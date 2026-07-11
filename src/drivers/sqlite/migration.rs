use async_trait::async_trait;
use tokio_rusqlite::rusqlite;

use crate::{
    pending_migrations, AppliedMigration, MigrationBackend, MigrationReport, PreparedMigration,
};

use super::{SqliteExecutor, SqliteMigrationError};

const CREATE_TABLE: &str = "
    create table if not exists a3s_orm_migrations (
        version text primary key,
        name text not null,
        checksum text not null,
        applied_at text not null default current_timestamp
    )";

#[async_trait]
impl MigrationBackend for SqliteExecutor {
    type Error = SqliteMigrationError;

    async fn apply(
        &self,
        migrations: &[PreparedMigration],
    ) -> Result<MigrationReport, Self::Error> {
        let _guard = self.transaction_lock.lock().await;
        let migrations = migrations.to_vec();
        let outcome = self
            .connection
            .call(move |connection| {
                connection.execute_batch("BEGIN IMMEDIATE")?;
                let result = migrate(connection, &migrations);
                match result {
                    Ok(Ok(report)) => {
                        connection.execute_batch("COMMIT")?;
                        Ok(Ok(report))
                    }
                    Ok(Err(error)) => {
                        let _ = connection.execute_batch("ROLLBACK");
                        Ok(Err(error))
                    }
                    Err(error) => {
                        let _ = connection.execute_batch("ROLLBACK");
                        Err(error)
                    }
                }
            })
            .await
            .map_err(crate::SqliteError::from)?;
        outcome.map_err(|error| match error {
            MigrationFailure::Validation(error) => SqliteMigrationError::Migration(error),
            MigrationFailure::Apply { version, source } => {
                SqliteMigrationError::Apply { version, source }
            }
        })
    }
}

enum MigrationFailure {
    Validation(crate::MigrationError),
    Apply {
        version: String,
        source: rusqlite::Error,
    },
}

fn migrate(
    connection: &rusqlite::Connection,
    migrations: &[PreparedMigration],
) -> rusqlite::Result<Result<MigrationReport, MigrationFailure>> {
    connection.execute_batch(CREATE_TABLE)?;
    let mut statement =
        connection.prepare("select version, checksum from a3s_orm_migrations order by version")?;
    let applied = statement
        .query_map([], |row| {
            Ok(AppliedMigration {
                version: row.get(0)?,
                checksum: row.get(1)?,
            })
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    let pending = match pending_migrations(&applied, migrations) {
        Ok(pending) => pending,
        Err(error) => return Ok(Err(MigrationFailure::Validation(error))),
    };
    let mut versions = Vec::with_capacity(pending.len());
    for migration in pending {
        if let Err(source) = connection.execute_batch(migration.up_sql()) {
            return Ok(Err(MigrationFailure::Apply {
                version: migration.version().to_owned(),
                source,
            }));
        }
        connection.execute(
            "insert into a3s_orm_migrations (version, name, checksum) values (?1, ?2, ?3)",
            rusqlite::params![migration.version(), migration.name(), migration.checksum()],
        )?;
        versions.push(migration.version().to_owned());
    }
    Ok(Ok(MigrationReport { applied: versions }))
}
