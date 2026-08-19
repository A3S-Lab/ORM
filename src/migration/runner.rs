use std::collections::HashSet;

use super::{
    AppliedMigration, Migration, MigrationBackend, MigrationError, MigrationLedger,
    MigrationRunError, PreparedMigration,
};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct MigrationReport {
    pub applied: Vec<String>,
}

impl MigrationReport {
    pub fn is_up_to_date(&self) -> bool {
        self.applied.is_empty()
    }
}

pub struct Migrator<B> {
    backend: B,
}

impl<B> Migrator<B> {
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    pub fn backend(&self) -> &B {
        &self.backend
    }
}

impl<B: MigrationBackend> Migrator<B> {
    pub async fn run(
        &self,
        migrations: impl IntoIterator<Item = Migration>,
    ) -> Result<MigrationReport, MigrationRunError<B::Error>> {
        let migrations = prepare(migrations)?;
        self.backend
            .apply(&migrations)
            .await
            .map_err(MigrationRunError::Backend)
    }
}

impl<B: MigrationLedger> Migrator<B> {
    /// Verify that every supplied migration is present with its exact
    /// checksum, without locking or mutating the database.
    ///
    /// Additional database migrations are admitted so an older serving
    /// process can run during an expand-compatible rolling upgrade. Callers
    /// remain responsible for deciding which schema versions are compatible.
    pub async fn verify_required(
        &self,
        migrations: impl IntoIterator<Item = Migration>,
    ) -> Result<(), MigrationRunError<B::Error>> {
        let required = prepare(migrations)?;
        let applied = self
            .backend
            .applied_migrations()
            .await
            .map_err(MigrationRunError::Backend)?;
        verify_required(&applied, &required)?;
        Ok(())
    }
}

fn verify_required(
    applied: &[AppliedMigration],
    required: &[PreparedMigration],
) -> Result<(), MigrationError> {
    for required_migration in required {
        let Some(applied_migration) = applied
            .iter()
            .find(|migration| migration.version == required_migration.version())
        else {
            return Err(MigrationError::MissingAppliedMigration(
                required_migration.version().to_owned(),
            ));
        };
        if applied_migration.checksum != required_migration.checksum() {
            return Err(MigrationError::ChecksumMismatch {
                version: required_migration.version().to_owned(),
                applied_checksum: applied_migration.checksum.clone(),
                source_checksum: required_migration.checksum().to_owned(),
            });
        }
    }
    Ok(())
}

fn prepare(
    migrations: impl IntoIterator<Item = Migration>,
) -> Result<Vec<PreparedMigration>, MigrationError> {
    let mut migrations = migrations.into_iter().collect::<Vec<_>>();
    for migration in &migrations {
        validate(migration)?;
    }
    migrations.sort_by(|left, right| left.version().cmp(right.version()));
    let mut versions = HashSet::with_capacity(migrations.len());
    for migration in &migrations {
        if !versions.insert(migration.version().to_owned()) {
            return Err(MigrationError::DuplicateVersion(
                migration.version().to_owned(),
            ));
        }
    }
    Ok(migrations
        .into_iter()
        .map(PreparedMigration::prepare)
        .collect())
}

fn validate(migration: &Migration) -> Result<(), MigrationError> {
    if migration.version().is_empty() {
        return Err(MigrationError::EmptyVersion);
    }
    if !migration
        .version()
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'))
    {
        return Err(MigrationError::InvalidVersion(
            migration.version().to_owned(),
        ));
    }
    if migration.name().trim().is_empty() {
        return Err(MigrationError::EmptyName {
            version: migration.version().to_owned(),
        });
    }
    if migration.up_sql().trim().is_empty() {
        return Err(MigrationError::EmptySql {
            version: migration.version().to_owned(),
        });
    }
    Ok(())
}
