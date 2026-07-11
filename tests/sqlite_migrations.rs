#![cfg(feature = "sqlite")]

use a3s_orm::{
    orm_table, select_from, Database, Migration, MigrationError, Migrator, SqliteDialect,
    SqliteExecutor,
};

orm_table! {
    struct Widget => "widget" {
        id: i64 => "id",
    }
}

orm_table! {
    struct RollbackProbe => "rollback_probe" {
        id: i64 => "id",
    }
}

fn migrations() -> Vec<Migration> {
    vec![
        Migration::new(
            "001",
            "create widgets",
            "create table widget (id integer primary key)",
        ),
        Migration::new("002", "seed widgets", "insert into widget (id) values (1)"),
    ]
}

#[tokio::test]
async fn applies_once_and_reports_up_to_date() {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    let migrator = Migrator::new(executor.clone());
    let first = migrator.run(migrations()).await.unwrap();
    assert_eq!(first.applied, ["001", "002"]);
    let second = migrator.run(migrations()).await.unwrap();
    assert!(second.is_up_to_date());

    let database = Database::new(SqliteDialect, executor);
    let rows = database
        .fetch_all_as(select_from::<Widget>().select(Widget::id()))
        .await
        .unwrap()
        .rows;
    assert_eq!(rows, vec![1]);
}

#[tokio::test]
async fn detects_changed_and_missing_source_migrations() {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    let migrator = Migrator::new(executor);
    migrator.run(migrations()).await.unwrap();

    let changed = migrator
        .run([
            Migration::new(
                "001",
                "create widgets",
                "create table widget (id integer primary key, changed text)",
            ),
            migrations().remove(1),
        ])
        .await
        .unwrap_err();
    assert!(matches!(
        changed,
        a3s_orm::migration::MigrationRunError::Backend(a3s_orm::SqliteMigrationError::Migration(
            MigrationError::ChecksumMismatch { .. }
        ))
    ));

    let missing = migrator.run([migrations().remove(1)]).await.unwrap_err();
    assert!(matches!(
        missing,
        a3s_orm::migration::MigrationRunError::Backend(
            a3s_orm::SqliteMigrationError::Migration(
                MigrationError::MissingSourceMigration(version)
            )
        ) if version == "001"
    ));
}

#[tokio::test]
async fn failed_migration_rolls_back_schema_and_history() {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    let migrator = Migrator::new(executor.clone());
    let error = migrator
        .run([Migration::new(
            "001",
            "broken migration",
            "create table rollback_probe (id integer); invalid sql",
        )])
        .await;
    let error = error.unwrap_err();
    assert!(error.to_string().contains("001"));

    let database = Database::new(SqliteDialect, executor.clone());
    assert!(database
        .fetch_all(select_from::<RollbackProbe>().select(RollbackProbe::id()))
        .await
        .is_err());

    let repaired = migrator
        .run([Migration::new(
            "001",
            "broken migration",
            "create table rollback_probe (id integer)",
        )])
        .await
        .unwrap();
    assert_eq!(repaired.applied, ["001"]);
}

#[tokio::test]
async fn concurrent_migrators_serialize_on_the_shared_connection() {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    let left = Migrator::new(executor.clone());
    let right = Migrator::new(executor);
    let (left, right) = tokio::join!(left.run(migrations()), right.run(migrations()));
    let applied = left.unwrap().applied.len() + right.unwrap().applied.len();
    assert_eq!(applied, 2);
}
