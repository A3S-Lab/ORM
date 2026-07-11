use sha2::{Digest, Sha256};

use super::MigrationError;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Migration {
    version: String,
    name: String,
    up_sql: String,
}

impl Migration {
    pub fn new(
        version: impl Into<String>,
        name: impl Into<String>,
        up_sql: impl Into<String>,
    ) -> Self {
        Self {
            version: version.into(),
            name: name.into(),
            up_sql: up_sql.into(),
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn up_sql(&self) -> &str {
        &self.up_sql
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedMigration {
    version: String,
    name: String,
    up_sql: String,
    checksum: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: String,
    pub checksum: String,
}

impl PreparedMigration {
    pub(crate) fn prepare(migration: Migration) -> Self {
        let mut digest = Sha256::new();
        digest.update(migration.up_sql.as_bytes());
        let checksum = hex_encode(&digest.finalize());
        Self {
            version: migration.version,
            name: migration.name,
            up_sql: migration.up_sql,
            checksum,
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn up_sql(&self) -> &str {
        &self.up_sql
    }

    pub fn checksum(&self) -> &str {
        &self.checksum
    }
}

fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        encoded.push(HEX[(byte >> 4) as usize] as char);
        encoded.push(HEX[(byte & 0x0f) as usize] as char);
    }
    encoded
}

pub fn pending_migrations<'a>(
    applied: &[AppliedMigration],
    source: &'a [PreparedMigration],
) -> Result<Vec<&'a PreparedMigration>, MigrationError> {
    for applied_migration in applied {
        let Some(source_migration) = source
            .iter()
            .find(|migration| migration.version() == applied_migration.version)
        else {
            return Err(MigrationError::MissingSourceMigration(
                applied_migration.version.clone(),
            ));
        };
        if source_migration.checksum() != applied_migration.checksum {
            return Err(MigrationError::ChecksumMismatch {
                version: applied_migration.version.clone(),
                applied_checksum: applied_migration.checksum.clone(),
                source_checksum: source_migration.checksum().to_owned(),
            });
        }
    }
    Ok(source
        .iter()
        .filter(|migration| {
            !applied
                .iter()
                .any(|applied| applied.version == migration.version())
        })
        .collect())
}
