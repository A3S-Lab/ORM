use std::time::Duration;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SqliteJournalMode {
    Delete,
    Truncate,
    Persist,
    Memory,
    Wal,
    Off,
}

impl SqliteJournalMode {
    pub(crate) const fn as_sql(self) -> &'static str {
        match self {
            Self::Delete => "DELETE",
            Self::Truncate => "TRUNCATE",
            Self::Persist => "PERSIST",
            Self::Memory => "MEMORY",
            Self::Wal => "WAL",
            Self::Off => "OFF",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SqliteOptions {
    pub busy_timeout: Duration,
    pub foreign_keys: bool,
    pub journal_mode: SqliteJournalMode,
}

impl Default for SqliteOptions {
    fn default() -> Self {
        Self {
            busy_timeout: Duration::from_secs(5),
            foreign_keys: true,
            journal_mode: SqliteJournalMode::Wal,
        }
    }
}

impl SqliteOptions {
    pub(crate) fn in_memory() -> Self {
        Self {
            journal_mode: SqliteJournalMode::Memory,
            ..Self::default()
        }
    }
}
