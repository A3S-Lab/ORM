#![cfg(feature = "postgres")]

use std::sync::Arc;
use std::time::Duration;

use a3s_orm::{
    sql_query, Executor, FromRow, Migration, Migrator, PostgresDialect, PostgresError,
    PostgresExecutor, PostgresIsolationLevel, PostgresMigrationError, PostgresMigrationOptions,
    PostgresPoolOptions, PostgresRetryClass, PostgresTlsOptions, PostgresTransaction,
    PostgresTransactionAccessMode, PostgresTransactionError, PostgresTransactionOptions, Query,
    Transaction,
};
use tokio::sync::Barrier;

const MIGRATION_LOCK_ID: i64 = 0x4133_534f_524e;

#[tokio::test]
async fn postgres_ha_controls_are_bounded_observable_and_retry_safe() {
    let Some(url) = std::env::var("A3S_ORM_POSTGRES_URL").ok() else {
        return;
    };

    let pool_options = PostgresPoolOptions::new(8)
        .with_wait_timeout(Some(Duration::from_secs(2)))
        .with_create_timeout(Some(Duration::from_secs(2)))
        .with_recycle_timeout(Some(Duration::from_secs(2)));
    let executor = PostgresExecutor::connect_no_tls_with(&url, pool_options).unwrap();
    reset_tables(&executor).await;

    transaction_options_are_applied_locally(&executor).await;
    abandoned_transactions_do_not_reenter_the_pool(&url).await;
    serializable_conflicts_are_typed(&executor).await;
    failover_and_disconnects_are_typed(&executor).await;
    pool_exhaustion_and_rotation_are_observable(&url).await;
    migration_contention_is_bounded(&url).await;
    expand_contract_window_supports_old_and_new_callers(&executor).await;
    tls_connection_and_rotation_are_verified().await;

    let metrics = executor.pool_metrics();
    assert!(metrics.acquisition_attempts >= metrics.acquisition_successes);
    assert!(metrics.serialization_failures >= 1);
    assert!(metrics.failover_failures >= 1);
    assert_eq!(metrics.pool.generation, 0);
}

async fn abandoned_transactions_do_not_reenter_the_pool(url: &str) {
    let executor = PostgresExecutor::connect_no_tls_with(
        url,
        PostgresPoolOptions::new(1)
            .with_wait_timeout(Some(Duration::from_secs(2)))
            .with_create_timeout(Some(Duration::from_secs(2)))
            .with_recycle_timeout(Some(Duration::from_secs(2))),
    )
    .unwrap();
    let transaction = executor
        .begin_with(
            PostgresTransactionOptions::new()
                .with_access_mode(PostgresTransactionAccessMode::ReadOnly),
        )
        .await
        .unwrap();
    let abandoned_backend_pid = fetch_text(&transaction, "select pg_backend_pid()::text").await;

    std::thread::spawn(move || drop(transaction))
        .join()
        .unwrap();

    let client = executor.connection().await.unwrap();
    let replacement_backend_pid = client
        .query_one("select pg_backend_pid()::text", &[])
        .await
        .unwrap()
        .get::<_, String>(0);
    let transaction_read_only = client
        .query_one("show transaction_read_only", &[])
        .await
        .unwrap()
        .get::<_, String>(0);
    assert_ne!(replacement_backend_pid, abandoned_backend_pid);
    assert_eq!(transaction_read_only, "off");
}

async fn tls_connection_and_rotation_are_verified() {
    let (Some(url), Some(ca_path)) = (
        std::env::var("A3S_ORM_POSTGRES_TLS_URL").ok(),
        std::env::var("A3S_ORM_POSTGRES_TLS_CA").ok(),
    ) else {
        return;
    };
    let ca_pem = tokio::fs::read(ca_path).await.unwrap();
    let tls_options = PostgresTlsOptions::new(ca_pem);
    tls_options.validate().unwrap();
    let pool_options = PostgresPoolOptions::new(2)
        .with_wait_timeout(Some(Duration::from_secs(2)))
        .with_create_timeout(Some(Duration::from_secs(2)))
        .with_recycle_timeout(Some(Duration::from_secs(2)));
    let executor = PostgresExecutor::connect_tls(&url, pool_options, &tls_options).unwrap();
    let health = executor.health_check().await.unwrap();
    assert_eq!(health.pool.checked_out, 0);
    assert!(!health.pool.saturated);
    assert_eq!(
        executor
            .rotate_tls(&url, pool_options, &tls_options)
            .await
            .unwrap(),
        1
    );
    let health = executor.health_check().await.unwrap();
    assert_eq!(health.pool.generation, 1);
    let metrics = executor.pool_metrics();
    assert_eq!(metrics.rotation_successes, 1);
    assert_eq!(metrics.rotation_failures, 0);
}

async fn reset_tables(executor: &PostgresExecutor) {
    executor
        .connection()
        .await
        .unwrap()
        .batch_execute(
            "drop table if exists a3s_orm_migrations;
             drop table if exists a3s_orm_ha_counter;
             drop table if exists a3s_orm_ha_contract;
             create table a3s_orm_ha_counter (
                 id bigint primary key,
                 value bigint not null
             );
             insert into a3s_orm_ha_counter (id, value) values (1, 0);
             create table a3s_orm_ha_contract (
                 id bigint primary key,
                 value text not null
             )",
        )
        .await
        .unwrap();
}

async fn transaction_options_are_applied_locally(executor: &PostgresExecutor) {
    let permanent_before = executor.pool_metrics().permanent_failures;
    let Err(error) = executor
        .begin_with(
            PostgresTransactionOptions::new()
                .with_statement_timeout(Duration::from_millis(i32::MAX as u64 + 1)),
        )
        .await
    else {
        panic!("oversized transaction timeout should be rejected");
    };
    assert!(matches!(error, PostgresError::Options(_)));
    assert_eq!(
        executor.pool_metrics().permanent_failures,
        permanent_before + 1
    );

    let options = PostgresTransactionOptions::new()
        .with_isolation_level(PostgresIsolationLevel::Serializable)
        .with_access_mode(PostgresTransactionAccessMode::ReadOnly)
        .with_statement_timeout(Duration::from_millis(250))
        .with_lock_timeout(Duration::from_millis(125))
        .with_idle_in_transaction_timeout(Duration::from_secs(2));
    let transaction = executor.begin_with(options).await.unwrap();

    assert_eq!(
        fetch_text(&transaction, "show transaction_isolation").await,
        "serializable"
    );
    assert_eq!(
        fetch_text(&transaction, "show transaction_read_only").await,
        "on"
    );
    assert_eq!(
        fetch_text(&transaction, "show statement_timeout").await,
        "250ms"
    );
    assert_eq!(fetch_text(&transaction, "show lock_timeout").await, "125ms");
    assert_eq!(
        fetch_text(&transaction, "show idle_in_transaction_session_timeout").await,
        "2s"
    );

    let write = sql_query::<()>("update a3s_orm_ha_counter set value = value + 1 where id = 1")
        .compile(&PostgresDialect)
        .unwrap();
    let error = transaction.execute(&write).await.unwrap_err();
    assert_eq!(error.retry_class(), PostgresRetryClass::Permanent);
    transaction.rollback().await.unwrap();
}

async fn serializable_conflicts_are_typed(executor: &PostgresExecutor) {
    let barrier = Arc::new(Barrier::new(2));
    let left = tokio::spawn(serializable_increment(
        executor.clone(),
        Arc::clone(&barrier),
    ));
    let right = tokio::spawn(serializable_increment(
        executor.clone(),
        Arc::clone(&barrier),
    ));
    let (left, right) = tokio::join!(left, right);
    let results = [left.unwrap(), right.unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    let error = results
        .into_iter()
        .find_map(Result::err)
        .expect("one serializable writer should be rejected");
    assert_eq!(
        error.retry_class(),
        PostgresRetryClass::SerializationConflict
    );
    assert!(error.is_retryable());

    let value = executor
        .connection()
        .await
        .unwrap()
        .query_one("select value from a3s_orm_ha_counter where id = 1", &[])
        .await
        .unwrap()
        .get::<_, i64>(0);
    assert_eq!(value, 1);
}

async fn serializable_increment(
    executor: PostgresExecutor,
    barrier: Arc<Barrier>,
) -> Result<(), PostgresTransactionError<PostgresError>> {
    executor
        .transaction_with(
            PostgresTransactionOptions::new()
                .with_isolation_level(PostgresIsolationLevel::Serializable)
                .with_statement_timeout(Duration::from_secs(2))
                .with_lock_timeout(Duration::from_secs(2)),
            move |transaction| {
                Box::pin(async move {
                    let read =
                        sql_query::<i64>("select value from a3s_orm_ha_counter where id = 1")
                            .compile(&PostgresDialect)
                            .unwrap();
                    transaction.fetch_all(&read).await?;
                    barrier.wait().await;
                    let write = sql_query::<()>(
                        "update a3s_orm_ha_counter set value = value + 1 where id = 1",
                    )
                    .compile(&PostgresDialect)
                    .unwrap();
                    transaction.execute(&write).await?;
                    Ok(())
                })
            },
        )
        .await
}

async fn failover_and_disconnects_are_typed(executor: &PostgresExecutor) {
    let simulated_failover = sql_query::<()>(
        "do $$ begin raise exception 'simulated failover' using errcode = '57P01'; end $$",
    )
    .compile(&PostgresDialect)
    .unwrap();
    let error = executor.execute(&simulated_failover).await.unwrap_err();
    assert_eq!(error.retry_class(), PostgresRetryClass::Failover);

    let victim = executor.connection().await.unwrap();
    let backend_pid = victim
        .query_one("select pg_backend_pid()", &[])
        .await
        .unwrap()
        .get::<_, i32>(0);
    let terminator = executor.connection().await.unwrap();
    assert!(terminator
        .query_one("select pg_terminate_backend($1)", &[&backend_pid])
        .await
        .unwrap()
        .get::<_, bool>(0));
    let source = victim.simple_query("select 1").await.unwrap_err();
    let error = PostgresError::Database(source);
    assert!(matches!(
        error.retry_class(),
        PostgresRetryClass::Failover | PostgresRetryClass::ConnectionLoss
    ));
    assert!(error.is_retryable());

    let unavailable = PostgresExecutor::connect_no_tls_with(
        "postgres://postgres:postgres@127.0.0.1:1/a3s_orm",
        PostgresPoolOptions::new(1)
            .with_wait_timeout(Some(Duration::from_millis(200)))
            .with_create_timeout(Some(Duration::from_millis(200)))
            .with_recycle_timeout(Some(Duration::from_millis(200))),
    )
    .unwrap();
    let error = unavailable.health_check().await.unwrap_err();
    assert_eq!(error.retry_class(), PostgresRetryClass::ConnectionLoss);
    assert!(error.is_retryable());
}

async fn pool_exhaustion_and_rotation_are_observable(url: &str) {
    let options = PostgresPoolOptions::new(1)
        .with_wait_timeout(Some(Duration::from_millis(100)))
        .with_create_timeout(Some(Duration::from_secs(2)))
        .with_recycle_timeout(Some(Duration::from_secs(2)));
    let executor = PostgresExecutor::connect_no_tls_with(url, options).unwrap();
    let held = executor.connection().await.unwrap();
    let error = executor.health_check().await.unwrap_err();
    assert_eq!(error.retry_class(), PostgresRetryClass::PoolSaturated);
    let metrics = executor.pool_metrics();
    assert_eq!(metrics.acquisition_attempts, 2);
    assert_eq!(metrics.acquisition_failures, 1);
    assert_eq!(metrics.health_check_failures, 1);
    assert_eq!(metrics.pool.checked_out, 1);
    assert!(metrics.pool.saturated);
    drop(held);
    executor.health_check().await.unwrap();

    let candidate = PostgresExecutor::connect_no_tls_with(url, options)
        .unwrap()
        .pool();
    assert_eq!(executor.rotate_pool(candidate).await.unwrap(), 1);
    let health = executor.health_check().await.unwrap();
    assert_eq!(health.pool.generation, 1);
    assert!(matches!(
        executor.rotate_pool(executor.pool()).await,
        Err(PostgresError::RotationUsesActivePool)
    ));

    let shared_candidate = PostgresExecutor::connect_no_tls_with(url, options)
        .unwrap()
        .pool();
    let (left, right) = tokio::join!(
        executor.rotate_pool(shared_candidate.clone()),
        executor.rotate_pool(shared_candidate),
    );
    let rotations = [left, right];
    assert_eq!(rotations.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        rotations
            .iter()
            .filter(|result| matches!(result, Err(PostgresError::RotationUsesActivePool)))
            .count(),
        1
    );
    assert_eq!(executor.health_check().await.unwrap().pool.generation, 2);

    let metrics = executor.pool_metrics();
    assert_eq!(metrics.rotation_attempts, 4);
    assert_eq!(metrics.rotation_successes, 2);
    assert_eq!(metrics.rotation_failures, 2);
}

async fn migration_contention_is_bounded(url: &str) {
    let options = PostgresPoolOptions::new(3)
        .with_wait_timeout(Some(Duration::from_secs(2)))
        .with_create_timeout(Some(Duration::from_secs(2)))
        .with_recycle_timeout(Some(Duration::from_secs(2)));
    let executor = PostgresExecutor::connect_no_tls_with(url, options)
        .unwrap()
        .with_migration_options(
            PostgresMigrationOptions::new()
                .with_advisory_lock_id(MIGRATION_LOCK_ID)
                .with_lock_timeout(Duration::from_millis(150)),
        )
        .unwrap();
    let holder = executor.connection().await.unwrap();
    holder.batch_execute("begin").await.unwrap();
    holder
        .query_one("select pg_advisory_xact_lock($1)", &[&MIGRATION_LOCK_ID])
        .await
        .unwrap();

    let error = Migrator::new(executor.clone())
        .run(Vec::<Migration>::new())
        .await
        .unwrap_err();
    let a3s_orm::migration::MigrationRunError::Backend(error) = error else {
        panic!("expected migration backend error");
    };
    assert!(matches!(error, PostgresMigrationError::LockTimeout { .. }));
    assert_eq!(error.retry_class(), PostgresRetryClass::LockContention);
    let metrics = executor.pool_metrics();
    assert_eq!(metrics.lock_contention_failures, 1);
    assert_eq!(metrics.pool_saturation_failures, 0);
    assert_eq!(metrics.permanent_failures, 0);
    holder.batch_execute("rollback").await.unwrap();

    Migrator::new(executor)
        .run(Vec::<Migration>::new())
        .await
        .unwrap();
}

async fn expand_contract_window_supports_old_and_new_callers(executor: &PostgresExecutor) {
    let client = executor.connection().await.unwrap();
    client
        .execute(
            "insert into a3s_orm_ha_contract (id, value) values ($1, $2)",
            &[&1_i64, &"before-expand"],
        )
        .await
        .unwrap();
    client
        .batch_execute("alter table a3s_orm_ha_contract add column value_v2 text")
        .await
        .unwrap();

    client
        .execute(
            "insert into a3s_orm_ha_contract (id, value) values ($1, $2)",
            &[&2_i64, &"old-writer"],
        )
        .await
        .unwrap();
    client
        .execute(
            "insert into a3s_orm_ha_contract (id, value, value_v2) values ($1, $2, $3)",
            &[&3_i64, &"new-writer", &"new-writer-v2"],
        )
        .await
        .unwrap();

    let old_values = client
        .query("select value from a3s_orm_ha_contract order by id", &[])
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    let new_values = client
        .query(
            "select coalesce(value_v2, value) from a3s_orm_ha_contract order by id",
            &[],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    assert_eq!(old_values, ["before-expand", "old-writer", "new-writer"]);
    assert_eq!(new_values, ["before-expand", "old-writer", "new-writer-v2"]);
}

async fn fetch_text(transaction: &PostgresTransaction, sql: &'static str) -> String {
    let query = sql_query::<String>(sql).compile(&PostgresDialect).unwrap();
    let result = transaction.fetch_all(&query).await.unwrap();
    String::from_row(&result.rows[0]).unwrap()
}
