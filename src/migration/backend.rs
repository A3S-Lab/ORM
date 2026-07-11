use async_trait::async_trait;

use super::{MigrationReport, PreparedMigration};

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
