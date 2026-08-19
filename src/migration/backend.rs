use async_trait::async_trait;

use super::{AppliedMigration, MigrationReport, PreparedMigration};

/// Reads the migration ledger without creating or changing database objects.
///
/// Serving processes use this boundary to prove that their required schema is
/// already present while retaining a database role that has no DDL authority.
#[async_trait]
pub trait MigrationLedger: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn applied_migrations(&self) -> Result<Vec<AppliedMigration>, Self::Error>;
}

/// Applies an already validated migration set.
///
/// Implementations must serialize concurrent migrators, verify previously
/// applied checksums, and atomically record every migration they apply.
#[async_trait]
pub trait MigrationBackend: Send + Sync {
    type Error: std::error::Error + Send + Sync + 'static;

    async fn apply(&self, migrations: &[PreparedMigration])
        -> Result<MigrationReport, Self::Error>;
}
