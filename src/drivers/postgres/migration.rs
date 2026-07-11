use async_trait::async_trait;

use crate::{
    pending_migrations, AppliedMigration, MigrationBackend, MigrationReport, PreparedMigration,
};

use super::{PostgresExecutor, PostgresMigrationError};

const MIGRATION_LOCK_ID: i64 = 0x4133_534f_524d;
const CREATE_TABLE: &str = "
    create table if not exists a3s_orm_migrations (
        version text primary key,
        name text not null,
        checksum text not null,
        applied_at timestamptz not null default now()
    )";

#[async_trait]
impl MigrationBackend for PostgresExecutor {
    type Error = PostgresMigrationError;

    async fn apply(
        &self,
        migrations: &[PreparedMigration],
    ) -> Result<MigrationReport, Self::Error> {
        let mut client = self.pool.get().await.map_err(crate::PostgresError::from)?;
        let transaction = client.transaction().await?;
        transaction
            .query_one("select pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK_ID])
            .await?;
        transaction.batch_execute(CREATE_TABLE).await?;
        let rows = transaction
            .query(
                "select version, checksum from a3s_orm_migrations order by version",
                &[],
            )
            .await?;
        let applied = rows
            .into_iter()
            .map(|row| AppliedMigration {
                version: row.get(0),
                checksum: row.get(1),
            })
            .collect::<Vec<_>>();
        let pending = pending_migrations(&applied, migrations)?;
        let mut versions = Vec::with_capacity(pending.len());
        for migration in pending {
            transaction
                .batch_execute(migration.up_sql())
                .await
                .map_err(|source| PostgresMigrationError::Apply {
                    version: migration.version().to_owned(),
                    source,
                })?;
            transaction
                .execute(
                    "insert into a3s_orm_migrations (version, name, checksum) values ($1, $2, $3)",
                    &[
                        &migration.version(),
                        &migration.name(),
                        &migration.checksum(),
                    ],
                )
                .await?;
            versions.push(migration.version().to_owned());
        }
        transaction.commit().await?;
        Ok(MigrationReport { applied: versions })
    }
}
