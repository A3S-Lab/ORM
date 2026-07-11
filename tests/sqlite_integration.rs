#![cfg(feature = "sqlite")]

use a3s_orm::{
    count, delete_from, insert_into, orm_table, select_from, update_table, Database, InsertRow,
    SelectionExt, SqliteDialect, SqliteExecutor, Value,
};

orm_table! {
    pub struct Person => "person" {
        id: i64 => "id",
        name: String => "name",
        age: i32 => "age",
        nickname: Option<String> => "nickname",
    }
}

orm_table! {
    struct Adult => "adult" {
        id: i64 => "id",
        name: String => "name",
    }
}

orm_table! {
    struct PersonWithNarrowAge => "person" {
        age: i8 => "age",
    }
}

#[tokio::test]
async fn executes_crud_against_real_sqlite() {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    executor
        .execute_schema(
            "create table person (\
             id integer primary key autoincrement, \
             name text not null, \
             age integer not null, \
             nickname text)",
        )
        .await
        .unwrap();
    let database = Database::new(SqliteDialect, executor);

    database
        .execute(
            insert_into::<Person>()
                .value(Person::name(), "Ada")
                .value(Person::age(), 36),
        )
        .await
        .unwrap();
    database
        .execute(
            insert_into::<Person>()
                .value(Person::name(), "Grace")
                .value(Person::age(), 40),
        )
        .await
        .unwrap();

    let rows = database
        .fetch_all(
            select_from::<Person>()
                .select((Person::id(), Person::name()))
                .filter(Person::age().gte(40)),
        )
        .await
        .unwrap()
        .rows;
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get(1), Some(&Value::String("Grace".to_string())));

    let updated = database
        .execute(
            update_table::<Person>()
                .set(Person::age(), 41)
                .filter(Person::name().eq("Grace")),
        )
        .await
        .unwrap();
    assert_eq!(updated.rows_affected, 1);

    let deleted = database
        .execute(delete_from::<Person>().filter(Person::name().eq("Ada")))
        .await
        .unwrap();
    assert_eq!(deleted.rows_affected, 1);
}

#[tokio::test]
async fn decodes_selected_columns_into_the_query_output_type() {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    executor
        .execute_schema(
            "create table person (id integer primary key, name text not null, age integer not null, nickname text)",
        )
        .await
        .unwrap();
    let database = Database::new(SqliteDialect, executor);
    database
        .execute(
            insert_into::<Person>()
                .value(Person::id(), 1)
                .value(Person::name(), "Ada")
                .value(Person::age(), 36)
                .value(Person::nickname(), None::<String>),
        )
        .await
        .unwrap();

    let rows = database
        .fetch_all_as(select_from::<Person>().select((
            Person::id(),
            Person::name(),
            Person::nickname(),
        )))
        .await
        .unwrap()
        .rows;
    assert_eq!(rows, vec![(1_i64, "Ada".to_owned(), None)]);
}

#[tokio::test]
async fn reports_integer_overflow_with_the_column_index() {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    executor
        .execute_schema(
            "create table person (age integer not null); insert into person values (1000)",
        )
        .await
        .unwrap();
    let database = Database::new(SqliteDialect, executor);

    let error = database
        .fetch_all_as(select_from::<PersonWithNarrowAge>().select(PersonWithNarrowAge::age()))
        .await
        .unwrap_err();
    assert!(error.to_string().contains("column 0"));
    assert!(error.to_string().contains("i8"));
}

#[tokio::test]
async fn executes_cte_and_bound_subquery_parameters() {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    executor
        .execute_schema(
            "create table person (
                id integer primary key,
                name text not null,
                age integer not null,
                nickname text
             );
             insert into person values (1, 'Ada', 36, null);
             insert into person values (2, 'Grace', 40, null)",
        )
        .await
        .unwrap();
    let database = Database::new(SqliteDialect, executor);

    let adult_cte = select_from::<Person>()
        .select((Person::id(), Person::name()))
        .filter(Person::age().gte(18))
        .as_cte::<Adult>();
    let eligible_ids = select_from::<Person>()
        .select(Person::id())
        .filter(Person::age().gte(40));
    let rows = database
        .fetch_all_as(
            select_from::<Adult>()
                .with(adult_cte)
                .select(Adult::name())
                .filter(Adult::id().in_subquery(eligible_ids)),
        )
        .await
        .unwrap()
        .rows;
    assert_eq!(rows, vec!["Grace".to_owned()]);

    let grouped = database
        .fetch_all_as(
            select_from::<Person>()
                .select((Person::age(), count(Person::id()).alias("people")))
                .group_by(Person::age())
                .having(count(Person::id()).gte(1))
                .order_by(Person::age(), a3s_orm::OrderDirection::Asc),
        )
        .await
        .unwrap()
        .rows;
    assert_eq!(grouped, vec![(36, 1_i64), (40, 1_i64)]);
}

#[tokio::test]
async fn executes_multi_row_insert_and_upsert() {
    let executor = SqliteExecutor::open_in_memory().await.unwrap();
    executor
        .execute_schema(
            "create table person (
                id integer primary key,
                name text not null,
                age integer not null,
                nickname text
             )",
        )
        .await
        .unwrap();
    let database = Database::new(SqliteDialect, executor);
    let inserted = database
        .fetch_all_as(
            insert_into::<Person>()
                .rows([
                    InsertRow::new()
                        .value(Person::id(), 1)
                        .value(Person::name(), "Ada")
                        .value(Person::age(), 36),
                    InsertRow::new()
                        .value(Person::id(), 2)
                        .value(Person::name(), "Grace")
                        .value(Person::age(), 40),
                ])
                .returning(Person::id()),
        )
        .await
        .unwrap()
        .rows;
    assert_eq!(inserted, vec![1, 2]);

    database
        .execute(
            insert_into::<Person>()
                .rows([
                    InsertRow::new()
                        .value(Person::id(), 1)
                        .value(Person::name(), "Ada updated")
                        .value(Person::age(), 37),
                    InsertRow::new()
                        .value(Person::id(), 3)
                        .value(Person::name(), "Linus")
                        .value(Person::age(), 55),
                ])
                .on_conflict(Person::id())
                .do_update_from_excluded(Person::name())
                .do_update_from_excluded(Person::age()),
        )
        .await
        .unwrap();
    let rows = database
        .fetch_all_as(
            select_from::<Person>()
                .select((Person::id(), Person::name(), Person::age()))
                .order_by(Person::id(), a3s_orm::OrderDirection::Asc),
        )
        .await
        .unwrap()
        .rows;
    assert_eq!(
        rows,
        vec![
            (1, "Ada updated".to_owned(), 37),
            (2, "Grace".to_owned(), 40),
            (3, "Linus".to_owned(), 55),
        ]
    );
}
