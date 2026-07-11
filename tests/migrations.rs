use std::sync::Mutex;

use a3s_orm::{
    Migration, MigrationBackend, MigrationError, MigrationReport, Migrator, PreparedMigration,
};
use async_trait::async_trait;

#[derive(Default)]
struct RecordingBackend {
    migrations: Mutex<Vec<PreparedMigration>>,
}

#[derive(Debug, thiserror::Error)]
#[error("recording backend failed")]
struct RecordingError;

#[async_trait]
impl MigrationBackend for RecordingBackend {
    type Error = RecordingError;

    async fn apply(
        &self,
        migrations: &[PreparedMigration],
    ) -> Result<MigrationReport, Self::Error> {
        self.migrations
            .lock()
            .unwrap()
            .extend_from_slice(migrations);
        Ok(MigrationReport {
            applied: migrations
                .iter()
                .map(|migration| migration.version().to_owned())
                .collect(),
        })
    }
}

#[tokio::test]
async fn validates_sorts_and_checksums_migrations_before_backend_execution() {
    let migrator = Migrator::new(RecordingBackend::default());
    let report = migrator
        .run([
            Migration::new("002", "second", "create table second (id integer)"),
            Migration::new("001", "first", "create table first (id integer)"),
        ])
        .await
        .unwrap();
    assert_eq!(report.applied, ["001", "002"]);
    let recorded = migrator.backend().migrations.lock().unwrap();
    assert_eq!(recorded[0].version(), "001");
    assert_eq!(recorded[0].checksum().len(), 64);
    assert_eq!(
        recorded[0].checksum(),
        "10e0a2282e051aa0de3c957c7ef36d748cea0a244fed443abb5917b55d7a8384"
    );
    assert_ne!(recorded[0].checksum(), recorded[1].checksum());
}

#[tokio::test]
async fn rejects_invalid_and_duplicate_versions_without_calling_backend() {
    let migrator = Migrator::new(RecordingBackend::default());
    let error = migrator
        .run([
            Migration::new("001", "first", "select 1"),
            Migration::new("001", "duplicate", "select 2"),
        ])
        .await
        .unwrap_err();
    assert!(matches!(
        error,
        a3s_orm::migration::MigrationRunError::Validation(
            MigrationError::DuplicateVersion(version)
        ) if version == "001"
    ));
    assert!(migrator.backend().migrations.lock().unwrap().is_empty());
}
