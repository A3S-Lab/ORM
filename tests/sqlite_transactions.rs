#![cfg(feature = "sqlite")]

use std::time::Duration;

use a3s_orm::{
    insert_into, orm_table, select_from, Executor, Query, SqliteDialect, SqliteExecutor,
    Transaction, TransactionManager,
};

orm_table! {
    pub struct Account => "account" {
        id: i64 => "id",
        balance: i64 => "balance",
    }
}

async fn executor() -> SqliteExecutor {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    executor
        .execute_schema("create table account (id integer primary key, balance integer not null)")
        .await
        .unwrap();
    executor
}

fn insert(id: i64) -> a3s_orm::CompiledQuery {
    insert_into::<Account>()
        .value(Account::id(), id)
        .value(Account::balance(), 100)
        .compile(&SqliteDialect)
        .unwrap()
}

fn select_all() -> a3s_orm::CompiledQuery {
    select_from::<Account>()
        .select(Account::id())
        .compile(&SqliteDialect)
        .unwrap()
}

#[tokio::test]
async fn commits_and_rolls_back_explicitly() {
    let executor = executor().await;

    let transaction = executor.begin().await.unwrap();
    transaction.execute(&insert(1)).await.unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(
        executor.fetch_all(&select_all()).await.unwrap().rows.len(),
        1
    );

    let transaction = executor.begin().await.unwrap();
    transaction.execute(&insert(2)).await.unwrap();
    transaction.rollback().await.unwrap();
    assert_eq!(
        executor.fetch_all(&select_all()).await.unwrap().rows.len(),
        1
    );
}

#[tokio::test]
async fn excludes_queries_from_other_executor_clones() {
    let executor = executor().await;
    let transaction = executor.begin().await.unwrap();
    transaction.execute(&insert(1)).await.unwrap();

    let concurrent_executor = executor.clone();
    let query = select_all();
    let concurrent = tokio::spawn(async move { concurrent_executor.fetch_all(&query).await });

    tokio::time::sleep(Duration::from_millis(20)).await;
    assert!(!concurrent.is_finished());

    transaction.commit().await.unwrap();
    let result = tokio::time::timeout(Duration::from_secs(1), concurrent)
        .await
        .expect("query remained blocked after commit")
        .unwrap()
        .unwrap();
    assert_eq!(result.rows.len(), 1);
}
