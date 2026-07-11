#![cfg(feature = "postgres")]

use a3s_orm::{
    count_all, insert_into, orm_table, select_from, Database, Executor, Migration, MigrationError,
    Migrator, PostgresDialect, PostgresExecutor, Query, SelectionExt, SqlArray,
};

orm_table! {
    struct Metric => "a3s_orm_metric" {
        id: i64 => "id",
        small_value: i16 => "small_value",
        count: i32 => "count",
        enabled: bool => "enabled",
        ratio: f32 => "ratio",
        score: f64 => "score",
        label: String => "label",
        payload: Vec<u8> => "payload",
        note: Option<String> => "note",
    }
}

orm_table! {
    struct ExtendedValue => "a3s_orm_extended_value" {
        id: uuid::Uuid => "id",
        metadata: serde_json::Value => "metadata",
        event_date: chrono::NaiveDate => "event_date",
        event_time: chrono::NaiveTime => "event_time",
        created_at: chrono::NaiveDateTime => "created_at",
        observed_at: chrono::DateTime<chrono::Utc> => "observed_at",
        amount: rust_decimal::Decimal => "amount",
        tags: SqlArray<String> => "tags",
        scores: SqlArray<Option<i32>> => "scores",
    }
}

fn insert_metric(id: i64, label: &str) -> a3s_orm::CompiledQuery {
    insert_into::<Metric>()
        .value(Metric::id(), id)
        .value(Metric::small_value(), 12)
        .value(Metric::count(), 34)
        .value(Metric::enabled(), true)
        .value(Metric::ratio(), 1.5)
        .value(Metric::score(), 2.5)
        .value(Metric::label(), label)
        .value(Metric::payload(), vec![1, 2, 3])
        .value(Metric::note(), None::<String>)
        .compile(&PostgresDialect)
        .unwrap()
}

orm_table! {
    struct NarrowMetric => "a3s_orm_metric" {
        small_value: i64 => "small_value",
    }
}

orm_table! {
    struct SelectedMetric => "selected_metric" {
        id: i64 => "id",
        label: String => "label",
    }
}

#[tokio::test]
async fn executes_typed_queries_against_postgres_pool() {
    let Some(url) = std::env::var("A3S_ORM_POSTGRES_URL").ok() else {
        return;
    };
    let executor = PostgresExecutor::connect_no_tls(&url, 4).unwrap();
    let client = executor.pool().get().await.unwrap();
    client
        .batch_execute(
            "drop table if exists a3s_orm_metric;
             drop table if exists a3s_orm_extended_value;
             drop table if exists a3s_orm_migration_probe;
             drop table if exists a3s_orm_rollback_probe;
             drop table if exists a3s_orm_migrations;
             create table a3s_orm_metric (
                id bigint primary key,
                small_value smallint not null,
                count integer not null,
                enabled boolean not null,
                ratio real not null,
                score double precision not null,
                label text not null,
                payload bytea not null,
                note text
             );
             create table a3s_orm_extended_value (
                id uuid primary key,
                metadata jsonb not null,
                event_date date not null,
                event_time time not null,
                created_at timestamp not null,
                observed_at timestamptz not null,
                amount numeric not null,
                tags text[] not null,
                scores integer[] not null
             )",
        )
        .await
        .unwrap();
    drop(client);

    let migration_set = || {
        vec![
            Migration::new(
                "001",
                "create migration probe",
                "create table a3s_orm_migration_probe (id bigint primary key)",
            ),
            Migration::new(
                "002",
                "seed migration probe",
                "insert into a3s_orm_migration_probe (id) values (1)",
            ),
        ]
    };
    let left = Migrator::new(executor.clone());
    let right = Migrator::new(executor.clone());
    let (left, right) = tokio::join!(left.run(migration_set()), right.run(migration_set()));
    assert_eq!(
        left.unwrap().applied.len() + right.unwrap().applied.len(),
        2
    );

    let drift = Migrator::new(executor.clone())
        .run([
            Migration::new("001", "changed", "select 1"),
            migration_set().remove(1),
        ])
        .await
        .unwrap_err();
    assert!(matches!(
        drift,
        a3s_orm::migration::MigrationRunError::Backend(a3s_orm::PostgresMigrationError::Migration(
            MigrationError::ChecksumMismatch { .. }
        ))
    ));

    let failed = Migrator::new(executor.clone())
        .run([
            migration_set().remove(0),
            migration_set().remove(1),
            Migration::new(
                "003",
                "broken migration",
                "create table a3s_orm_rollback_probe (id bigint); invalid sql",
            ),
        ])
        .await;
    assert!(failed.is_err());
    let client = executor.pool().get().await.unwrap();
    let table: Option<String> = client
        .query_one(
            "select to_regclass('public.a3s_orm_rollback_probe')::text",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert!(table.is_none());
    drop(client);

    let database = Database::new(PostgresDialect, executor);
    database
        .execute(
            insert_into::<Metric>()
                .value(Metric::id(), 1)
                .value(Metric::small_value(), 12)
                .value(Metric::count(), 34)
                .value(Metric::enabled(), true)
                .value(Metric::ratio(), 1.5)
                .value(Metric::score(), 2.5)
                .value(Metric::label(), "production")
                .value(Metric::payload(), vec![1, 2, 3])
                .value(Metric::note(), None::<String>),
        )
        .await
        .unwrap();

    let rows = database
        .fetch_all_as(select_from::<Metric>().select((
            Metric::id(),
            Metric::small_value(),
            Metric::count(),
            Metric::enabled(),
            Metric::label(),
            Metric::note(),
        )))
        .await
        .unwrap()
        .rows;
    assert_eq!(
        rows,
        vec![(1_i64, 12_i16, 34_i32, true, "production".to_owned(), None)]
    );

    let selected = select_from::<Metric>()
        .select((Metric::id(), Metric::label()))
        .filter(Metric::count().gte(30))
        .as_cte::<SelectedMetric>();
    let eligible = select_from::<Metric>()
        .select(Metric::id())
        .filter(Metric::small_value().gt(10));
    let rows = database
        .fetch_all_as(
            select_from::<SelectedMetric>()
                .with(selected)
                .select(SelectedMetric::label())
                .filter(SelectedMetric::id().in_subquery(eligible)),
        )
        .await
        .unwrap()
        .rows;
    assert_eq!(rows, vec!["production".to_owned()]);

    let total = database
        .fetch_all_as(select_from::<Metric>().select(count_all().alias("metric_count")))
        .await
        .unwrap()
        .rows;
    assert_eq!(total, vec![1_i64]);

    let extended_id = uuid::Uuid::parse_str("018f3f56-8d4a-7c2a-9f13-5ab3d245d701").unwrap();
    let metadata = serde_json::json!({"kind": "production", "attempt": 2});
    let event_date = chrono::NaiveDate::from_ymd_opt(2026, 7, 12).unwrap();
    let event_time = chrono::NaiveTime::from_hms_opt(14, 30, 45).unwrap();
    let created_at = event_date.and_time(event_time);
    let observed_at = created_at.and_utc();
    let amount = rust_decimal::Decimal::new(123456, 3);
    database
        .execute(
            insert_into::<ExtendedValue>()
                .value(ExtendedValue::id(), extended_id)
                .value(ExtendedValue::metadata(), metadata.clone())
                .value(ExtendedValue::event_date(), event_date)
                .value(ExtendedValue::event_time(), event_time)
                .value(ExtendedValue::created_at(), created_at)
                .value(ExtendedValue::observed_at(), observed_at)
                .value(ExtendedValue::amount(), amount)
                .value(
                    ExtendedValue::tags(),
                    SqlArray::from(vec!["rust".to_owned(), "postgres".to_owned()]),
                )
                .value(
                    ExtendedValue::scores(),
                    SqlArray::from(vec![Some(10), None, Some(30)]),
                ),
        )
        .await
        .unwrap();
    let scalar_values = database
        .fetch_all_as(select_from::<ExtendedValue>().select((
            ExtendedValue::id(),
            ExtendedValue::metadata(),
            ExtendedValue::event_date(),
            ExtendedValue::event_time(),
            ExtendedValue::created_at(),
            ExtendedValue::observed_at(),
            ExtendedValue::amount(),
        )))
        .await
        .unwrap()
        .rows;
    assert_eq!(
        scalar_values,
        vec![(
            extended_id,
            metadata,
            event_date,
            event_time,
            created_at,
            observed_at,
            amount,
        )]
    );
    let array_values = database
        .fetch_all_as(
            select_from::<ExtendedValue>().select((ExtendedValue::tags(), ExtendedValue::scores())),
        )
        .await
        .unwrap()
        .rows;
    assert_eq!(
        array_values,
        vec![(
            SqlArray::from(vec!["rust".to_owned(), "postgres".to_owned()]),
            SqlArray::from(vec![Some(10), None, Some(30)]),
        )]
    );

    let error = database
        .execute(insert_into::<NarrowMetric>().value(NarrowMetric::small_value(), i64::MAX))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("smallint"));

    let executor = database.executor();
    executor
        .transaction(|transaction| {
            Box::pin(async move {
                transaction
                    .execute(&insert_metric(2, "committed"))
                    .await
                    .unwrap();
                Ok::<_, std::io::Error>(())
            })
        })
        .await
        .unwrap();
    let error = executor
        .transaction(|transaction| {
            Box::pin(async move {
                transaction
                    .execute(&insert_metric(3, "rolled back"))
                    .await
                    .unwrap();
                Err::<(), _>(std::io::Error::other("reject transaction"))
            })
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("reject transaction"));

    let rows = database
        .fetch_all_as(select_from::<Metric>().select(Metric::id()))
        .await
        .unwrap()
        .rows;
    assert_eq!(rows, vec![1, 2]);
}
