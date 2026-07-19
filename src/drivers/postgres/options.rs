use std::time::Duration;

const MAX_POSTGRES_TIMEOUT_MILLIS: u128 = i32::MAX as u128;
const NANOS_PER_MILLISECOND: u128 = 1_000_000;
pub(crate) const DEFAULT_MIGRATION_LOCK_ID: i64 = 0x4133_534f_524d;

/// PostgreSQL transaction isolation selected before application statements run.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PostgresIsolationLevel {
    #[default]
    ReadCommitted,
    RepeatableRead,
    Serializable,
}

/// Whether a PostgreSQL transaction may write data.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum PostgresTransactionAccessMode {
    #[default]
    ReadWrite,
    ReadOnly,
}

/// Typed PostgreSQL transaction settings applied transaction-locally.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PostgresTransactionOptions {
    isolation_level: PostgresIsolationLevel,
    access_mode: PostgresTransactionAccessMode,
    statement_timeout: Option<Duration>,
    lock_timeout: Option<Duration>,
    idle_in_transaction_timeout: Option<Duration>,
}

impl PostgresTransactionOptions {
    pub const fn new() -> Self {
        Self {
            isolation_level: PostgresIsolationLevel::ReadCommitted,
            access_mode: PostgresTransactionAccessMode::ReadWrite,
            statement_timeout: None,
            lock_timeout: None,
            idle_in_transaction_timeout: None,
        }
    }

    pub const fn with_isolation_level(mut self, isolation_level: PostgresIsolationLevel) -> Self {
        self.isolation_level = isolation_level;
        self
    }

    pub const fn with_access_mode(mut self, access_mode: PostgresTransactionAccessMode) -> Self {
        self.access_mode = access_mode;
        self
    }

    pub const fn with_statement_timeout(mut self, timeout: Duration) -> Self {
        self.statement_timeout = Some(timeout);
        self
    }

    pub const fn with_lock_timeout(mut self, timeout: Duration) -> Self {
        self.lock_timeout = Some(timeout);
        self
    }

    pub const fn with_idle_in_transaction_timeout(mut self, timeout: Duration) -> Self {
        self.idle_in_transaction_timeout = Some(timeout);
        self
    }

    pub const fn isolation_level(&self) -> PostgresIsolationLevel {
        self.isolation_level
    }

    pub const fn access_mode(&self) -> PostgresTransactionAccessMode {
        self.access_mode
    }

    pub const fn statement_timeout(&self) -> Option<Duration> {
        self.statement_timeout
    }

    pub const fn lock_timeout(&self) -> Option<Duration> {
        self.lock_timeout
    }

    pub const fn idle_in_transaction_timeout(&self) -> Option<Duration> {
        self.idle_in_transaction_timeout
    }

    pub fn validate(&self) -> Result<(), PostgresOptionsError> {
        validate_postgres_timeout("statement_timeout", self.statement_timeout)?;
        validate_postgres_timeout("lock_timeout", self.lock_timeout)?;
        validate_postgres_timeout(
            "idle_in_transaction_session_timeout",
            self.idle_in_transaction_timeout,
        )
    }

    pub(crate) const fn begin_sql(&self) -> &'static str {
        match (self.isolation_level, self.access_mode) {
            (PostgresIsolationLevel::ReadCommitted, PostgresTransactionAccessMode::ReadWrite) => {
                "BEGIN ISOLATION LEVEL READ COMMITTED READ WRITE"
            }
            (PostgresIsolationLevel::ReadCommitted, PostgresTransactionAccessMode::ReadOnly) => {
                "BEGIN ISOLATION LEVEL READ COMMITTED READ ONLY"
            }
            (PostgresIsolationLevel::RepeatableRead, PostgresTransactionAccessMode::ReadWrite) => {
                "BEGIN ISOLATION LEVEL REPEATABLE READ READ WRITE"
            }
            (PostgresIsolationLevel::RepeatableRead, PostgresTransactionAccessMode::ReadOnly) => {
                "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY"
            }
            (PostgresIsolationLevel::Serializable, PostgresTransactionAccessMode::ReadWrite) => {
                "BEGIN ISOLATION LEVEL SERIALIZABLE READ WRITE"
            }
            (PostgresIsolationLevel::Serializable, PostgresTransactionAccessMode::ReadOnly) => {
                "BEGIN ISOLATION LEVEL SERIALIZABLE READ ONLY"
            }
        }
    }
}

/// Bounded Deadpool settings used by the built-in PostgreSQL constructors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostgresPoolOptions {
    max_size: usize,
    wait_timeout: Option<Duration>,
    create_timeout: Option<Duration>,
    recycle_timeout: Option<Duration>,
}

impl PostgresPoolOptions {
    pub const fn new(max_size: usize) -> Self {
        Self {
            max_size,
            wait_timeout: Some(Duration::from_secs(30)),
            create_timeout: Some(Duration::from_secs(30)),
            recycle_timeout: Some(Duration::from_secs(5)),
        }
    }

    pub const fn with_wait_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.wait_timeout = timeout;
        self
    }

    pub const fn with_create_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.create_timeout = timeout;
        self
    }

    pub const fn with_recycle_timeout(mut self, timeout: Option<Duration>) -> Self {
        self.recycle_timeout = timeout;
        self
    }

    pub const fn max_size(&self) -> usize {
        self.max_size
    }

    pub const fn wait_timeout(&self) -> Option<Duration> {
        self.wait_timeout
    }

    pub const fn create_timeout(&self) -> Option<Duration> {
        self.create_timeout
    }

    pub const fn recycle_timeout(&self) -> Option<Duration> {
        self.recycle_timeout
    }

    pub fn validate(&self) -> Result<(), PostgresOptionsError> {
        if self.max_size == 0 {
            return Err(PostgresOptionsError::EmptyPool);
        }
        validate_nonzero_timeout("pool wait timeout", self.wait_timeout)?;
        validate_nonzero_timeout("pool create timeout", self.create_timeout)?;
        validate_nonzero_timeout("pool recycle timeout", self.recycle_timeout)
    }
}

impl Default for PostgresPoolOptions {
    fn default() -> Self {
        Self::new(16)
    }
}

/// PostgreSQL migration-lock behavior.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PostgresMigrationOptions {
    advisory_lock_id: i64,
    lock_timeout: Duration,
}

impl PostgresMigrationOptions {
    pub const fn new() -> Self {
        Self {
            advisory_lock_id: DEFAULT_MIGRATION_LOCK_ID,
            lock_timeout: Duration::from_secs(30),
        }
    }

    pub const fn with_advisory_lock_id(mut self, advisory_lock_id: i64) -> Self {
        self.advisory_lock_id = advisory_lock_id;
        self
    }

    pub const fn with_lock_timeout(mut self, lock_timeout: Duration) -> Self {
        self.lock_timeout = lock_timeout;
        self
    }

    pub const fn advisory_lock_id(&self) -> i64 {
        self.advisory_lock_id
    }

    pub const fn lock_timeout(&self) -> Duration {
        self.lock_timeout
    }

    pub fn validate(&self) -> Result<(), PostgresOptionsError> {
        validate_nonzero_timeout("migration lock timeout", Some(self.lock_timeout))?;
        validate_postgres_timeout("lock_timeout", Some(self.lock_timeout))
    }
}

impl Default for PostgresMigrationOptions {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
#[non_exhaustive]
pub enum PostgresOptionsError {
    #[error("PostgreSQL pool max_size must be greater than zero")]
    EmptyPool,
    #[error("{setting} must be greater than zero when configured")]
    ZeroTimeout { setting: &'static str },
    #[error("{setting} is {millis}ms, exceeding PostgreSQL's supported maximum of {max_millis}ms")]
    TimeoutTooLarge {
        setting: &'static str,
        millis: u128,
        max_millis: u128,
    },
}

pub(crate) fn postgres_timeout_value(
    setting: &'static str,
    timeout: Duration,
) -> Result<String, PostgresOptionsError> {
    validate_postgres_timeout(setting, Some(timeout))?;
    Ok(format!("{}ms", postgres_timeout_millis(timeout)))
}

fn validate_postgres_timeout(
    setting: &'static str,
    timeout: Option<Duration>,
) -> Result<(), PostgresOptionsError> {
    let Some(timeout) = timeout else {
        return Ok(());
    };
    let millis = postgres_timeout_millis(timeout);
    if millis > MAX_POSTGRES_TIMEOUT_MILLIS {
        return Err(PostgresOptionsError::TimeoutTooLarge {
            setting,
            millis,
            max_millis: MAX_POSTGRES_TIMEOUT_MILLIS,
        });
    }
    Ok(())
}

fn postgres_timeout_millis(timeout: Duration) -> u128 {
    timeout.as_nanos().div_ceil(NANOS_PER_MILLISECOND)
}

fn validate_nonzero_timeout(
    setting: &'static str,
    timeout: Option<Duration>,
) -> Result<(), PostgresOptionsError> {
    if timeout.is_some_and(|timeout| timeout.is_zero()) {
        return Err(PostgresOptionsError::ZeroTimeout { setting });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn transaction_begin_sql_is_static_and_complete() {
        let cases = [
            (
                PostgresIsolationLevel::ReadCommitted,
                PostgresTransactionAccessMode::ReadWrite,
                "BEGIN ISOLATION LEVEL READ COMMITTED READ WRITE",
            ),
            (
                PostgresIsolationLevel::ReadCommitted,
                PostgresTransactionAccessMode::ReadOnly,
                "BEGIN ISOLATION LEVEL READ COMMITTED READ ONLY",
            ),
            (
                PostgresIsolationLevel::RepeatableRead,
                PostgresTransactionAccessMode::ReadWrite,
                "BEGIN ISOLATION LEVEL REPEATABLE READ READ WRITE",
            ),
            (
                PostgresIsolationLevel::RepeatableRead,
                PostgresTransactionAccessMode::ReadOnly,
                "BEGIN ISOLATION LEVEL REPEATABLE READ READ ONLY",
            ),
            (
                PostgresIsolationLevel::Serializable,
                PostgresTransactionAccessMode::ReadWrite,
                "BEGIN ISOLATION LEVEL SERIALIZABLE READ WRITE",
            ),
            (
                PostgresIsolationLevel::Serializable,
                PostgresTransactionAccessMode::ReadOnly,
                "BEGIN ISOLATION LEVEL SERIALIZABLE READ ONLY",
            ),
        ];
        for (isolation, access, expected) in cases {
            let options = PostgresTransactionOptions::new()
                .with_isolation_level(isolation)
                .with_access_mode(access);
            assert_eq!(options.isolation_level(), isolation);
            assert_eq!(options.access_mode(), access);
            assert_eq!(options.begin_sql(), expected);
        }
        assert_eq!(PostgresPoolOptions::default().max_size(), 16);
    }

    #[test]
    fn options_reject_invalid_pool_and_timeout_bounds() {
        assert_eq!(
            PostgresPoolOptions::new(0).validate(),
            Err(PostgresOptionsError::EmptyPool)
        );
        assert_eq!(
            PostgresPoolOptions::new(1)
                .with_wait_timeout(Some(Duration::ZERO))
                .validate(),
            Err(PostgresOptionsError::ZeroTimeout {
                setting: "pool wait timeout"
            })
        );
        assert!(matches!(
            PostgresTransactionOptions::new()
                .with_statement_timeout(Duration::from_millis(i32::MAX as u64 + 1))
                .validate(),
            Err(PostgresOptionsError::TimeoutTooLarge {
                setting: "statement_timeout",
                ..
            })
        ));
        assert_eq!(
            postgres_timeout_value("statement_timeout", Duration::from_nanos(1)).unwrap(),
            "1ms"
        );
        assert!(matches!(
            PostgresTransactionOptions::new()
                .with_statement_timeout(
                    Duration::from_millis(i32::MAX as u64) + Duration::from_nanos(1)
                )
                .validate(),
            Err(PostgresOptionsError::TimeoutTooLarge {
                setting: "statement_timeout",
                millis,
                ..
            }) if millis == i32::MAX as u128 + 1
        ));
    }
}
