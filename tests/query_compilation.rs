use a3s_orm::{
    delete_from, insert_into, orm_table, select_from, update_table, OrderDirection,
    PostgresDialect, Query, SqliteDialect, Value,
};

orm_table! {
    pub struct Person => "person" {
        id: i64 => "id",
        name: String => "name",
        age: i32 => "age",
        manager_id: Option<i64> => "manager_id",
    }
}

orm_table! {
    pub struct Pet => "pet" {
        id: i64 => "id",
        owner_id: i64 => "owner_id",
        name: String => "name",
    }
}

#[test]
fn compiles_typed_select_join_filter_order_and_pagination() {
    let query = select_from::<Person>()
        .select((Person::id(), Person::name(), Pet::name()))
        .inner_join::<Pet>(Person::id().eq_column(Pet::owner_id()))
        .filter(Person::age().gte(18))
        .filter(Person::name().like("A%"))
        .order_by(Person::name(), OrderDirection::Asc)
        .limit(10)
        .offset(20)
        .compile(&PostgresDialect)
        .unwrap();

    assert_eq!(
        query.sql,
        "select \"person\".\"id\", \"person\".\"name\", \"pet\".\"name\" from \"person\" inner join \"pet\" on (\"person\".\"id\" = \"pet\".\"owner_id\") where ((\"person\".\"age\" >= $1) and (\"person\".\"name\" like $2)) order by \"person\".\"name\" asc limit $3 offset $4"
    );
    assert_eq!(
        query.parameters,
        vec![
            Value::I64(18),
            Value::String("A%".to_string()),
            Value::U64(10),
            Value::U64(20),
        ]
    );
}

#[test]
fn compiles_insert_update_and_delete_without_interpolating_values() {
    let insert = insert_into::<Person>()
        .value(Person::name(), "Robert'); drop table person; --")
        .value(Person::age(), 42)
        .returning(Person::id())
        .compile(&SqliteDialect)
        .unwrap();
    assert_eq!(
        insert.sql,
        "insert into \"person\" (\"name\", \"age\") values (?, ?) returning \"person\".\"id\""
    );
    assert!(!insert.sql.contains("drop table"));

    let update = update_table::<Person>()
        .set(Person::name(), "Ada")
        .filter(Person::id().eq(7))
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        update.sql,
        "update \"person\" set \"name\" = $1 where (\"person\".\"id\" = $2)"
    );

    let delete = delete_from::<Person>()
        .filter(Person::manager_id().is_null())
        .returning((Person::id(), Person::name()))
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        delete.sql,
        "delete from \"person\" where \"person\".\"manager_id\" is null returning \"person\".\"id\", \"person\".\"name\""
    );
}

#[test]
fn rejects_incomplete_queries_and_unsupported_returning() {
    assert!(select_from::<Person>().compile(&PostgresDialect).is_err());
    assert!(insert_into::<Person>().compile(&PostgresDialect).is_err());
    assert!(update_table::<Person>().compile(&PostgresDialect).is_err());

    let error = insert_into::<Person>()
        .value(Person::name(), "Ada")
        .returning(Person::id())
        .compile(&a3s_orm::MysqlDialect)
        .unwrap_err();
    assert!(error.to_string().contains("does not support returning"));
}
