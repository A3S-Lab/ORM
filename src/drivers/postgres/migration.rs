use async_trait::async_trait;

use crate::{
    pending_migrations, AppliedMigration, MigrationBackend, MigrationError, MigrationReport,
    PreparedMigration,
};

use super::options::postgres_timeout_value;
use super::{PostgresError, PostgresExecutor, PostgresMigrationError, PostgresOptionsError};

const CREATE_TABLE: &str = "
    create table if not exists a3s_orm_migrations (
        version text primary key,
        name text not null,
        checksum text not null,
        applied_at timestamptz not null default now()
    )";

impl PostgresExecutor {
    fn migration_options_error(&self, source: PostgresOptionsError) -> PostgresMigrationError {
        let error = PostgresError::from(source);
        self.record_error(&error);
        PostgresMigrationError::Driver(error)
    }

    fn migration_database_error(&self, source: tokio_postgres::Error) -> PostgresMigrationError {
        self.record_database_failure(&source);
        PostgresMigrationError::Database(source)
    }

    fn migration_definition_error(&self, source: MigrationError) -> PostgresMigrationError {
        self.record_retry_class(super::PostgresRetryClass::Permanent);
        PostgresMigrationError::Migration(source)
    }

    fn migration_apply_error(
        &self,
        version: &str,
        source: tokio_postgres::Error,
    ) -> PostgresMigrationError {
        self.record_database_failure(&source);
        PostgresMigrationError::Apply {
            version: version.to_owned(),
            source,
        }
    }
}

#[async_trait]
impl MigrationBackend for PostgresExecutor {
    type Error = PostgresMigrationError;

    async fn apply(
        &self,
        migrations: &[PreparedMigration],
    ) -> Result<MigrationReport, Self::Error> {
        let options = self.migration_options();
        options
            .validate()
            .map_err(|source| self.migration_options_error(source))?;
        let lock_timeout = postgres_timeout_value("lock_timeout", options.lock_timeout())
            .map_err(|source| self.migration_options_error(source))?;
        let mut client = self.acquire().await?;
        let transaction = client
            .transaction()
            .await
            .map_err(|source| self.migration_database_error(source))?;
        transaction
            .execute(
                "select set_config('lock_timeout', $1, true)",
                &[&lock_timeout],
            )
            .await
            .map_err(|source| self.migration_database_error(source))?;
        if let Err(source) = transaction
            .query_one(
                "select pg_advisory_xact_lock($1)",
                &[&options.advisory_lock_id()],
            )
            .await
        {
            if source.code() == Some(&tokio_postgres::error::SqlState::LOCK_NOT_AVAILABLE) {
                self.record_database_failure(&source);
                return Err(PostgresMigrationError::LockTimeout {
                    timeout: options.lock_timeout(),
                    source,
                });
            }
            return Err(self.migration_database_error(source));
        }
        transaction
            .batch_execute(CREATE_TABLE)
            .await
            .map_err(|source| self.migration_database_error(source))?;
        let rows = transaction
            .query(
                "select version, checksum from a3s_orm_migrations order by version",
                &[],
            )
            .await
            .map_err(|source| self.migration_database_error(source))?;
        let applied = rows
            .into_iter()
            .map(|row| AppliedMigration {
                version: row.get(0),
                checksum: row.get(1),
            })
            .collect::<Vec<_>>();
        let pending = pending_migrations(&applied, migrations)
            .map_err(|source| self.migration_definition_error(source))?;
        let mut versions = Vec::with_capacity(pending.len());
        for migration in pending {
            transaction
                .batch_execute(migration.up_sql())
                .await
                .map_err(|source| self.migration_apply_error(migration.version(), source))?;
            transaction
                .execute(
                    "insert into a3s_orm_migrations (version, name, checksum) values ($1, $2, $3)",
                    &[
                        &migration.version(),
                        &migration.name(),
                        &migration.checksum(),
                    ],
                )
                .await
                .map_err(|source| self.migration_database_error(source))?;
            versions.push(migration.version().to_owned());
        }
        transaction
            .commit()
            .await
            .map_err(|source| self.migration_database_error(source))?;
        Ok(MigrationReport { applied: versions })
    }
}
