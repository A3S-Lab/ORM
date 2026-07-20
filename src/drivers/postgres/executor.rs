use std::future::Future;
use std::pin::Pin;
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Instant;

use async_trait::async_trait;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod, Runtime, Timeouts};
use tokio_postgres::NoTls;

use crate::{CompiledQuery, ExecuteResult, Executor, QueryResult, Transaction, TransactionManager};

use super::metrics::PostgresPoolMetrics;
use super::parameters::{encode, references};
use super::{
    PostgresError, PostgresMigrationOptions, PostgresPoolHealth, PostgresPoolMetricsSnapshot,
    PostgresPoolOptions, PostgresPoolStatus, PostgresRetryClass, PostgresRow, PostgresTlsOptions,
    PostgresTransaction, PostgresTransactionError, PostgresTransactionOptions,
};

#[derive(Clone)]
pub struct PostgresExecutor {
    pool: Arc<RwLock<Pool>>,
    metrics: Arc<PostgresPoolMetrics>,
    rotation: Arc<tokio::sync::Mutex<()>>,
    migration_options: PostgresMigrationOptions,
}

impl PostgresExecutor {
    pub fn from_pool(pool: Pool) -> Self {
        Self {
            pool: Arc::new(RwLock::new(pool)),
            metrics: Arc::new(PostgresPoolMetrics::default()),
            rotation: Arc::new(tokio::sync::Mutex::new(())),
            migration_options: PostgresMigrationOptions::default(),
        }
    }

    /// Build a bounded non-TLS pool. This is intended for local development or
    /// connections protected by a separate trusted transport.
    pub fn connect_no_tls(url: &str, max_size: usize) -> Result<Self, PostgresError> {
        Self::connect_no_tls_with(url, PostgresPoolOptions::new(max_size))
    }

    pub fn connect_no_tls_with(
        url: &str,
        pool_options: PostgresPoolOptions,
    ) -> Result<Self, PostgresError> {
        pool_options.validate()?;
        let config = tokio_postgres::Config::from_str(url).map_err(PostgresError::Configuration)?;
        let manager = Manager::from_config(
            config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Verified,
            },
        );
        Ok(Self::from_pool(build_pool(manager, pool_options)?))
    }

    /// Build a rustls-backed pool from in-memory certificate material.
    ///
    /// The URL must use `sslmode=require`. Root and client certificate
    /// verification is performed before a pool is constructed.
    pub fn connect_tls(
        url: &str,
        pool_options: PostgresPoolOptions,
        tls_options: &PostgresTlsOptions,
    ) -> Result<Self, PostgresError> {
        pool_options.validate()?;
        let config = tokio_postgres::Config::from_str(url).map_err(PostgresError::Configuration)?;
        tls_options.validate_connection_config(&config)?;
        let manager = Manager::from_config(
            config,
            tls_options.connector()?,
            ManagerConfig {
                recycling_method: RecyclingMethod::Verified,
            },
        );
        Ok(Self::from_pool(build_pool(manager, pool_options)?))
    }

    pub fn with_migration_options(
        mut self,
        migration_options: PostgresMigrationOptions,
    ) -> Result<Self, PostgresError> {
        migration_options.validate()?;
        self.migration_options = migration_options;
        Ok(self)
    }

    /// Return a clone of the active pool. A clone retained across rotation
    /// continues to refer to the drained old generation.
    pub fn pool(&self) -> Pool {
        self.pool
            .read()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    pub fn pool_status(&self) -> PostgresPoolStatus {
        PostgresPoolStatus::from_deadpool(self.pool().status(), self.metrics.generation())
    }

    pub fn pool_metrics(&self) -> PostgresPoolMetricsSnapshot {
        self.metrics.snapshot(self.pool().status())
    }

    pub const fn migration_options(&self) -> PostgresMigrationOptions {
        self.migration_options
    }

    /// Acquire a measured connection from the active pool generation.
    pub async fn connection(&self) -> Result<deadpool_postgres::Client, PostgresError> {
        self.acquire().await
    }

    /// Probe pool acquisition and a live server round trip.
    pub async fn health_check(&self) -> Result<PostgresPoolHealth, PostgresError> {
        let started = Instant::now();
        let client = match self.acquire().await {
            Ok(client) => client,
            Err(error) => {
                self.metrics.record_health(started.elapsed(), false);
                return Err(error);
            }
        };
        if let Err(source) = client.simple_query("SELECT 1").await {
            let error = PostgresError::Database(source);
            self.record_error(&error);
            self.metrics.record_health(started.elapsed(), false);
            return Err(error);
        }
        drop(client);
        let latency = started.elapsed();
        self.metrics.record_health(latency, true);
        Ok(PostgresPoolHealth {
            pool: self.pool_status(),
            latency,
        })
    }

    /// Health-check and atomically install a replacement pool, then close the
    /// old generation to new acquisitions. Existing checked-out clients may
    /// finish naturally.
    pub async fn rotate_pool(&self, candidate: Pool) -> Result<u64, PostgresError> {
        let _rotation = self.rotation.lock().await;
        self.metrics.start_rotation();
        let active = self.pool();
        if std::ptr::eq(active.manager(), candidate.manager()) {
            self.metrics.finish_rotation(false);
            let error = PostgresError::RotationUsesActivePool;
            self.record_error(&error);
            return Err(error);
        }
        let client = match candidate.get().await {
            Ok(client) => client,
            Err(source) => {
                candidate.close();
                self.metrics.finish_rotation(false);
                let error = PostgresError::Pool(source);
                self.record_error(&error);
                return Err(error);
            }
        };
        if let Err(source) = client.simple_query("SELECT 1").await {
            drop(client);
            candidate.close();
            self.metrics.finish_rotation(false);
            let error = PostgresError::Database(source);
            self.record_error(&error);
            return Err(error);
        }
        drop(client);

        let old = {
            let mut active = self
                .pool
                .write()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            std::mem::replace(&mut *active, candidate)
        };
        let generation = self.metrics.next_generation();
        old.close();
        self.metrics.finish_rotation(true);
        Ok(generation)
    }

    /// Build, validate, health-check, and install new TLS certificate material.
    pub async fn rotate_tls(
        &self,
        url: &str,
        pool_options: PostgresPoolOptions,
        tls_options: &PostgresTlsOptions,
    ) -> Result<u64, PostgresError> {
        pool_options.validate()?;
        let config = tokio_postgres::Config::from_str(url).map_err(PostgresError::Configuration)?;
        tls_options.validate_connection_config(&config)?;
        let manager = Manager::from_config(
            config,
            tls_options.connector()?,
            ManagerConfig {
                recycling_method: RecyclingMethod::Verified,
            },
        );
        self.rotate_pool(build_pool(manager, pool_options)?).await
    }

    pub async fn begin_with(
        &self,
        options: PostgresTransactionOptions,
    ) -> Result<PostgresTransaction, PostgresError> {
        options.validate().map_err(|source| {
            let error = PostgresError::Options(source);
            self.record_error(&error);
            error
        })?;
        let client = self.acquire().await?;
        if let Err(source) = client.batch_execute(options.begin_sql()).await {
            let error = PostgresError::Database(source);
            self.record_error(&error);
            return Err(error);
        }
        for (setting, timeout) in [
            ("statement_timeout", options.statement_timeout()),
            ("lock_timeout", options.lock_timeout()),
            (
                "idle_in_transaction_session_timeout",
                options.idle_in_transaction_timeout(),
            ),
        ] {
            let Some(timeout) = timeout else {
                continue;
            };
            let value = super::options::postgres_timeout_value(setting, timeout)?;
            if let Err(source) = client
                .execute("SELECT set_config($1, $2, true)", &[&setting, &value])
                .await
            {
                let error = match client.batch_execute("ROLLBACK").await {
                    Ok(()) => PostgresError::TransactionSetup { setting, source },
                    Err(rollback) => PostgresError::TransactionSetupAndRollback {
                        setting,
                        source,
                        rollback,
                    },
                };
                self.record_error(&error);
                return Err(error);
            }
        }
        Ok(PostgresTransaction::new(client, Arc::clone(&self.metrics)))
    }

    pub async fn transaction_with<T, E, F>(
        &self,
        options: PostgresTransactionOptions,
        operation: F,
    ) -> Result<T, PostgresTransactionError<E>>
    where
        T: Send,
        E: std::error::Error + Send + Sync + 'static,
        F: for<'a> FnOnce(
            &'a PostgresTransaction,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>,
    {
        let transaction = self
            .begin_with(options)
            .await
            .map_err(PostgresTransactionError::Begin)?;
        match operation(&transaction).await {
            Ok(value) => {
                transaction
                    .commit()
                    .await
                    .map_err(PostgresTransactionError::Commit)?;
                Ok(value)
            }
            Err(operation) => match transaction.rollback().await {
                Ok(()) => Err(PostgresTransactionError::Operation(operation)),
                Err(rollback) => Err(PostgresTransactionError::OperationAndRollback {
                    operation,
                    rollback,
                }),
            },
        }
    }

    pub async fn transaction<T, E, F>(&self, operation: F) -> Result<T, PostgresTransactionError<E>>
    where
        T: Send,
        E: std::error::Error + Send + Sync + 'static,
        F: for<'a> FnOnce(
            &'a PostgresTransaction,
        ) -> Pin<Box<dyn Future<Output = Result<T, E>> + Send + 'a>>,
    {
        self.transaction_with(PostgresTransactionOptions::default(), operation)
            .await
    }

    pub(crate) async fn acquire(&self) -> Result<deadpool_postgres::Client, PostgresError> {
        let pool = self.pool();
        let measurement = self.metrics.start_acquisition();
        match pool.get().await {
            Ok(client) => {
                measurement.finish(true);
                Ok(client)
            }
            Err(source) => {
                measurement.finish(false);
                let error = PostgresError::Pool(source);
                self.record_error(&error);
                Err(error)
            }
        }
    }

    pub(crate) fn record_error(&self, error: &PostgresError) {
        self.record_retry_class(error.retry_class());
    }

    pub(crate) fn record_retry_class(&self, class: PostgresRetryClass) {
        self.metrics.record_error(class);
    }

    pub(crate) fn record_database_failure(&self, error: &tokio_postgres::Error) {
        self.record_retry_class(super::error::classify_database(error));
    }
}

#[async_trait]
impl TransactionManager for PostgresExecutor {
    type Transaction = PostgresTransaction;

    async fn begin(&self) -> Result<Self::Transaction, Self::Error> {
        self.begin_with(PostgresTransactionOptions::default()).await
    }
}

#[async_trait]
impl Executor for PostgresExecutor {
    type Row = PostgresRow;
    type Error = PostgresError;

    async fn execute(&self, query: &CompiledQuery) -> Result<ExecuteResult, Self::Error> {
        let client = self.acquire().await?;
        let statement = client.prepare_cached(&query.sql).await.map_err(|source| {
            let error = PostgresError::Database(source);
            self.record_error(&error);
            error
        })?;
        let values = encode(&query.parameters, statement.params())
            .inspect_err(|error| self.record_error(error))?;
        let parameters = references(&values);
        let rows_affected = client
            .execute(&statement, &parameters)
            .await
            .map_err(|source| {
                let error = PostgresError::Database(source);
                self.record_error(&error);
                error
            })?;
        Ok(ExecuteResult { rows_affected })
    }

    async fn fetch_all(
        &self,
        query: &CompiledQuery,
    ) -> Result<QueryResult<Self::Row>, Self::Error> {
        let client = self.acquire().await?;
        let statement = client.prepare_cached(&query.sql).await.map_err(|source| {
            let error = PostgresError::Database(source);
            self.record_error(&error);
            error
        })?;
        let values = encode(&query.parameters, statement.params())
            .inspect_err(|error| self.record_error(error))?;
        let parameters = references(&values);
        let rows = client
            .query(&statement, &parameters)
            .await
            .map_err(|source| {
                let error = PostgresError::Database(source);
                self.record_error(&error);
                error
            })?
            .into_iter()
            .map(PostgresRow::decode)
            .collect::<Result<Vec<_>, _>>()
            .inspect_err(|error| self.record_error(error))?;
        Ok(QueryResult { rows })
    }
}

fn build_pool(manager: Manager, options: PostgresPoolOptions) -> Result<Pool, PostgresError> {
    let timeouts = Timeouts {
        wait: options.wait_timeout(),
        create: options.create_timeout(),
        recycle: options.recycle_timeout(),
    };
    Pool::builder(manager)
        .max_size(options.max_size())
        .timeouts(timeouts)
        .runtime(Runtime::Tokio1)
        .build()
        .map_err(PostgresError::PoolBuild)
}
