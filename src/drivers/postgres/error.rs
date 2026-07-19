use std::error::Error as _;
use std::time::Duration;

use deadpool_postgres::{PoolError, TimeoutType};
use tokio_postgres::error::SqlState;

use super::{PostgresOptionsError, PostgresTlsError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PostgresRetryClass {
    SerializationConflict,
    Deadlock,
    LockContention,
    Failover,
    ConnectionLoss,
    PoolSaturated,
    Permanent,
}

impl PostgresRetryClass {
    pub const fn is_retryable(self) -> bool {
        !matches!(self, Self::Permanent)
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PostgresError {
    #[error("could not get PostgreSQL connection from pool: {0}")]
    Pool(#[from] deadpool_postgres::PoolError),
    #[error("PostgreSQL operation failed: {0}")]
    Database(#[from] tokio_postgres::Error),
    #[error("invalid PostgreSQL connection string: {0}")]
    Configuration(#[source] tokio_postgres::Error),
    #[error(transparent)]
    Options(#[from] PostgresOptionsError),
    #[error(transparent)]
    Tls(#[from] PostgresTlsError),
    #[error("could not build PostgreSQL connection pool: {0}")]
    PoolBuild(#[source] deadpool_postgres::BuildError),
    #[error("replacement PostgreSQL pool must be a distinct pool generation")]
    RotationUsesActivePool,
    #[error("could not apply transaction-local PostgreSQL setting {setting}: {source}")]
    TransactionSetup {
        setting: &'static str,
        #[source]
        source: tokio_postgres::Error,
    },
    #[error(
        "could not apply transaction-local PostgreSQL setting {setting} ({source}); rollback also failed ({rollback})"
    )]
    TransactionSetupAndRollback {
        setting: &'static str,
        source: tokio_postgres::Error,
        rollback: tokio_postgres::Error,
    },
    #[error("parameter value {value} is outside the range of PostgreSQL {target}")]
    IntegerOverflow { value: String, target: &'static str },
    #[error("query has {values} values but PostgreSQL inferred {parameters} parameters")]
    ParameterCount { values: usize, parameters: usize },
    #[error("PostgreSQL returned unsupported column type {0}")]
    UnsupportedType(String),
    #[error("cannot encode {actual} as an element of PostgreSQL {target}[]")]
    ArrayElementType {
        actual: &'static str,
        target: &'static str,
    },
    #[error("cannot encode PostgreSQL array element {index}: {source}")]
    ArrayElement {
        index: usize,
        source: Box<PostgresError>,
    },
    #[error("PostgreSQL transaction no longer owns its pooled connection")]
    TransactionClosed,
}

impl PostgresError {
    pub fn retry_class(&self) -> PostgresRetryClass {
        match self {
            Self::Pool(error) => classify_pool(error),
            Self::Database(error) => classify_database(error),
            Self::TransactionSetup { source, .. } => classify_database(source),
            Self::TransactionSetupAndRollback { source, .. } => classify_database(source),
            Self::Configuration(_)
            | Self::Options(_)
            | Self::Tls(_)
            | Self::PoolBuild(_)
            | Self::RotationUsesActivePool
            | Self::IntegerOverflow { .. }
            | Self::ParameterCount { .. }
            | Self::UnsupportedType(_)
            | Self::ArrayElementType { .. }
            | Self::ArrayElement { .. }
            | Self::TransactionClosed => PostgresRetryClass::Permanent,
        }
    }

    pub fn is_retryable(&self) -> bool {
        self.retry_class().is_retryable()
    }
}

#[derive(Debug, thiserror::Error)]
pub enum PostgresTransactionError<E>
where
    E: std::error::Error + Send + Sync + 'static,
{
    #[error("could not start PostgreSQL transaction: {0}")]
    Begin(#[source] PostgresError),
    #[error("transaction operation failed: {0}")]
    Operation(#[source] E),
    #[error("could not commit PostgreSQL transaction: {0}")]
    Commit(#[source] PostgresError),
    #[error("transaction operation failed ({operation}) and rollback failed ({rollback})")]
    OperationAndRollback {
        operation: E,
        rollback: PostgresError,
    },
}

impl PostgresTransactionError<PostgresError> {
    pub fn retry_class(&self) -> PostgresRetryClass {
        match self {
            Self::Begin(error) | Self::Commit(error) => error.retry_class(),
            Self::Operation(error) => error.retry_class(),
            Self::OperationAndRollback { operation, .. } => operation.retry_class(),
        }
    }

    pub fn is_retryable(&self) -> bool {
        self.retry_class().is_retryable()
    }
}

#[derive(Debug, thiserror::Error)]
#[non_exhaustive]
pub enum PostgresMigrationError {
    #[error(transparent)]
    Driver(#[from] PostgresError),
    #[error(transparent)]
    Database(#[from] tokio_postgres::Error),
    #[error(transparent)]
    Migration(#[from] crate::MigrationError),
    #[error("PostgreSQL migration {version:?} failed: {source}")]
    Apply {
        version: String,
        #[source]
        source: tokio_postgres::Error,
    },
    #[error("PostgreSQL migration lock was not acquired within {timeout:?}: {source}")]
    LockTimeout {
        timeout: Duration,
        #[source]
        source: tokio_postgres::Error,
    },
}

impl PostgresMigrationError {
    pub fn retry_class(&self) -> PostgresRetryClass {
        match self {
            Self::Driver(error) => error.retry_class(),
            Self::Database(error) | Self::Apply { source: error, .. } => classify_database(error),
            Self::LockTimeout { .. } => PostgresRetryClass::LockContention,
            Self::Migration(_) => PostgresRetryClass::Permanent,
        }
    }

    pub fn is_retryable(&self) -> bool {
        self.retry_class().is_retryable()
    }
}

fn classify_pool(error: &PoolError) -> PostgresRetryClass {
    match error {
        PoolError::Timeout(TimeoutType::Wait) => PostgresRetryClass::PoolSaturated,
        PoolError::Timeout(TimeoutType::Create | TimeoutType::Recycle) | PoolError::Closed => {
            PostgresRetryClass::ConnectionLoss
        }
        PoolError::Backend(error) => classify_database(error),
        PoolError::PostCreateHook(_) | PoolError::NoRuntimeSpecified => {
            PostgresRetryClass::Permanent
        }
    }
}

pub(crate) fn classify_database(error: &tokio_postgres::Error) -> PostgresRetryClass {
    let Some(code) = error.code() else {
        return if error.is_closed() || has_io_source(error) {
            PostgresRetryClass::ConnectionLoss
        } else {
            PostgresRetryClass::Permanent
        };
    };
    if *code == SqlState::T_R_SERIALIZATION_FAILURE {
        return PostgresRetryClass::SerializationConflict;
    }
    if *code == SqlState::T_R_DEADLOCK_DETECTED {
        return PostgresRetryClass::Deadlock;
    }
    if *code == SqlState::LOCK_NOT_AVAILABLE {
        return PostgresRetryClass::LockContention;
    }
    if matches!(
        *code,
        SqlState::ADMIN_SHUTDOWN | SqlState::CRASH_SHUTDOWN | SqlState::CANNOT_CONNECT_NOW
    ) {
        return PostgresRetryClass::Failover;
    }
    if code.code().starts_with("08") {
        return PostgresRetryClass::ConnectionLoss;
    }
    PostgresRetryClass::Permanent
}

fn has_io_source(error: &tokio_postgres::Error) -> bool {
    let mut source = error.source();
    while let Some(current) = source {
        if current.downcast_ref::<std::io::Error>().is_some() {
            return true;
        }
        source = current.source();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retry_classes_have_explicit_retry_policy() {
        assert!(PostgresRetryClass::SerializationConflict.is_retryable());
        assert!(PostgresRetryClass::Deadlock.is_retryable());
        assert!(PostgresRetryClass::LockContention.is_retryable());
        assert!(PostgresRetryClass::Failover.is_retryable());
        assert!(PostgresRetryClass::ConnectionLoss.is_retryable());
        assert!(PostgresRetryClass::PoolSaturated.is_retryable());
        assert!(!PostgresRetryClass::Permanent.is_retryable());
    }

    #[test]
    fn configuration_error_never_echoes_connection_credentials() {
        let marker = "postgres://user:private-password-marker@[unterminated-host/database";
        let source = marker.parse::<tokio_postgres::Config>().unwrap_err();
        let error = PostgresError::Configuration(source);
        let rendered = format!("{error:?} {error}");
        assert!(!rendered.contains("private-password-marker"));
        assert_eq!(error.retry_class(), PostgresRetryClass::Permanent);
    }

    #[test]
    fn wrapper_and_pool_errors_preserve_typed_retry_classes() {
        fn saturated() -> PostgresError {
            PostgresError::Pool(PoolError::Timeout(TimeoutType::Wait))
        }

        assert_eq!(
            PostgresTransactionError::Begin(saturated()).retry_class(),
            PostgresRetryClass::PoolSaturated
        );
        assert_eq!(
            PostgresTransactionError::Commit(saturated()).retry_class(),
            PostgresRetryClass::PoolSaturated
        );
        assert!(PostgresTransactionError::Operation(saturated()).is_retryable());
        assert_eq!(
            PostgresTransactionError::OperationAndRollback {
                operation: PostgresError::ParameterCount {
                    values: 2,
                    parameters: 1,
                },
                rollback: saturated(),
            }
            .retry_class(),
            PostgresRetryClass::Permanent
        );
        assert_eq!(
            PostgresTransactionError::OperationAndRollback {
                operation: saturated(),
                rollback: PostgresError::ParameterCount {
                    values: 2,
                    parameters: 1,
                },
            }
            .retry_class(),
            PostgresRetryClass::PoolSaturated
        );

        assert_eq!(
            PostgresMigrationError::Driver(saturated()).retry_class(),
            PostgresRetryClass::PoolSaturated
        );
        assert!(
            !PostgresMigrationError::Migration(crate::MigrationError::EmptyVersion).is_retryable()
        );
        assert_eq!(
            PostgresError::Pool(PoolError::Closed).retry_class(),
            PostgresRetryClass::ConnectionLoss
        );
        assert_eq!(
            PostgresError::Pool(PoolError::NoRuntimeSpecified).retry_class(),
            PostgresRetryClass::Permanent
        );
    }
}
