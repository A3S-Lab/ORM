#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum MigrationError {
    #[error("migration version cannot be empty")]
    EmptyVersion,
    #[error("migration {version:?} has an empty name")]
    EmptyName { version: String },
    #[error("migration {version:?} has empty SQL")]
    EmptySql { version: String },
    #[error("migration version contains unsupported characters: {0:?}")]
    InvalidVersion(String),
    #[error("duplicate migration version: {0:?}")]
    DuplicateVersion(String),
    #[error(
        "migration {version:?} changed after it was applied (database {applied_checksum}, source {source_checksum})"
    )]
    ChecksumMismatch {
        version: String,
        applied_checksum: String,
        source_checksum: String,
    },
    #[error("database contains migration {0:?} that is absent from the source")]
    MissingSourceMigration(String),
}

#[derive(Debug, thiserror::Error)]
pub enum MigrationRunError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error(transparent)]
    Validation(#[from] MigrationError),
    #[error("migration backend failed: {0}")]
    Backend(E),
}
