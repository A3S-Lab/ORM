#![cfg(feature = "sqlite")]

use a3s_orm::{
    delete_from, insert_into, orm_table, select_from, update_table, Database, SqliteDialect,
    SqliteExecutor, Value,
};

orm_table! {
    pub struct Person => "person" {
        id: i64 => "id",
        name: String => "name",
        age: i32 => "age",
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
             age integer not null)",
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
