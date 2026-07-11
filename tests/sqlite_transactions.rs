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

#[tokio::test]
async fn scoped_transaction_commits_on_success_and_rolls_back_on_error() {
    let executor = executor().await;

    let value = executor
        .transaction(|transaction| {
            Box::pin(async move {
                transaction.execute(&insert(1)).await.unwrap();
                Ok::<_, std::io::Error>("committed")
            })
        })
        .await
        .unwrap();
    assert_eq!(value, "committed");

    let error = executor
        .transaction(|transaction| {
            Box::pin(async move {
                transaction.execute(&insert(2)).await.unwrap();
                Err::<(), _>(std::io::Error::other("reject operation"))
            })
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("reject operation"));
    assert_eq!(
        executor.fetch_all(&select_all()).await.unwrap().rows.len(),
        1
    );
}

#[tokio::test]
async fn aborting_scoped_transaction_rolls_back_and_releases_connection() {
    let executor = executor().await;
    let task_executor = executor.clone();
    let (inserted_tx, inserted_rx) = tokio::sync::oneshot::channel();

    let task = tokio::spawn(async move {
        task_executor
            .transaction(|transaction| {
                Box::pin(async move {
                    transaction.execute(&insert(1)).await.unwrap();
                    inserted_tx.send(()).unwrap();
                    std::future::pending::<Result<(), std::io::Error>>().await
                })
            })
            .await
    });
    inserted_rx.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());

    let result = tokio::time::timeout(Duration::from_secs(1), executor.fetch_all(&select_all()))
        .await
        .expect("connection remained locked after transaction task was aborted")
        .unwrap();
    assert!(result.rows.is_empty());
}

#[tokio::test]
async fn savepoint_releases_on_success_and_rolls_back_only_its_changes_on_error() {
    let executor = executor().await;
    executor
        .transaction(|transaction| {
            Box::pin(async move {
                transaction.execute(&insert(1)).await.unwrap();
                transaction
                    .savepoint(|savepoint| {
                        Box::pin(async move {
                            savepoint.execute(&insert(2)).await.unwrap();
                            Ok::<_, std::io::Error>(())
                        })
                    })
                    .await
                    .unwrap();
                let error = transaction
                    .savepoint(|savepoint| {
                        Box::pin(async move {
                            savepoint.execute(&insert(3)).await.unwrap();
                            Err::<(), _>(std::io::Error::other("discard nested work"))
                        })
                    })
                    .await
                    .unwrap_err();
                assert!(error.to_string().contains("discard nested work"));
                Ok::<_, std::io::Error>(())
            })
        })
        .await
        .unwrap();

    let rows = executor.fetch_all(&select_all()).await.unwrap().rows;
    assert_eq!(rows.len(), 2);
}

#[tokio::test]
async fn cancelled_savepoint_cleans_up_before_outer_transaction_continues() {
    let executor = executor().await;
    executor
        .transaction(|transaction| {
            Box::pin(async move {
                let timed_out = tokio::time::timeout(
                    Duration::from_millis(20),
                    transaction.savepoint(|savepoint| {
                        Box::pin(async move {
                            savepoint.execute(&insert(1)).await.unwrap();
                            std::future::pending::<Result<(), std::io::Error>>().await
                        })
                    }),
                )
                .await;
                assert!(timed_out.is_err());

                transaction.execute(&insert(2)).await.unwrap();
                Ok::<_, std::io::Error>(())
            })
        })
        .await
        .unwrap();

    let rows = executor.fetch_all(&select_all()).await.unwrap().rows;
    assert_eq!(rows.len(), 1);
}
