#![cfg(feature = "postgres")]

use a3s_orm::{
    insert_into, orm_table, select_from, Database, Executor, PostgresDialect, PostgresExecutor,
    Query,
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
             )",
        )
        .await
        .unwrap();
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
