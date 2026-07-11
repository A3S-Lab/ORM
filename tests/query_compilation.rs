use a3s_orm::expression::Selection;
use a3s_orm::{
    count, delete_from, exists, insert_into, orm_table, select_from, update_table, OrderDirection,
    PostgresDialect, Query, SelectionExt, SqliteDialect, Value,
};

orm_table! {
    pub struct Person => "person" {
        id: i64 => "id",
        name: String => "name",
        age: i32 => "age",
        manager_id: Option<i64> => "manager_id",
    }
}

struct InvalidScalarSelection;

impl Selection for InvalidScalarSelection {
    type Output = i64;

    fn expressions(self) -> Vec<a3s_orm::Expression> {
        vec![Person::id().expression(), Person::manager_id().expression()]
    }
}

orm_table! {
    pub struct Adult => "adult" {
        id: i64 => "id",
        name: String => "name",
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

#[test]
fn compiles_scalar_subqueries_exists_and_continuous_parameters() {
    let adults = select_from::<Person>()
        .select(Person::id())
        .filter(Person::age().gte(18));
    let pets = select_from::<Pet>()
        .select(Pet::id())
        .filter(Pet::owner_id().eq_column(Person::id()));
    let query = select_from::<Person>()
        .select(Person::name())
        .filter(Person::name().like("A%"))
        .filter(Person::id().in_subquery(adults))
        .filter(exists(pets))
        .compile(&PostgresDialect)
        .unwrap();

    assert_eq!(
        query.sql,
        "select \"person\".\"name\" from \"person\" where ((\"person\".\"name\" like $1) and (\"person\".\"id\" in (select \"person\".\"id\" from \"person\" where (\"person\".\"age\" >= $2))) and exists (select \"pet\".\"id\" from \"pet\" where (\"pet\".\"owner_id\" = \"person\".\"id\")))"
    );
    assert_eq!(
        query.parameters,
        vec![Value::String("A%".to_owned()), Value::I64(18)]
    );
}

#[test]
fn compiles_ctes_before_main_query_and_rejects_invalid_shapes() {
    let adult_cte = select_from::<Person>()
        .select((Person::id(), Person::name()))
        .filter(Person::age().gte(18))
        .as_cte::<Adult>();
    let query = select_from::<Adult>()
        .with(adult_cte)
        .select(Adult::name())
        .filter(Adult::id().gt(10))
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "with \"adult\" as (select \"person\".\"id\", \"person\".\"name\" from \"person\" where (\"person\".\"age\" >= $1)) select \"adult\".\"name\" from \"adult\" where (\"adult\".\"id\" > $2)"
    );
    assert_eq!(query.parameters, vec![Value::I64(18), Value::I64(10)]);

    let invalid_scalar = select_from::<Person>()
        .select(Person::id())
        .filter(Person::id().eq_subquery(select_from::<Person>().select(InvalidScalarSelection)))
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(invalid_scalar.to_string().contains("exactly one"));

    let duplicate = select_from::<Adult>()
        .with(
            select_from::<Person>()
                .select(Person::id())
                .as_cte::<Adult>(),
        )
        .with(
            select_from::<Person>()
                .select(Person::id())
                .as_cte::<Adult>(),
        )
        .select(Adult::id())
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(duplicate.to_string().contains("duplicate"));
}

#[test]
fn select_replaces_the_previous_projection_to_preserve_output_type() {
    let query = select_from::<Person>()
        .select(Person::id())
        .select(Person::name())
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(query.sql, "select \"person\".\"name\" from \"person\"");
}

#[test]
fn compiles_aliased_aggregates_grouping_and_having() {
    let query = select_from::<Person>()
        .select((Person::age(), count(Person::id()).alias("person_count")))
        .group_by(Person::age())
        .having(count(Person::id()).gt(1))
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "select \"person\".\"age\", \"count\"(\"person\".\"id\") as \"person_count\" from \"person\" group by \"person\".\"age\" having (\"count\"(\"person\".\"id\") > $1)"
    );
    assert_eq!(query.parameters, vec![Value::I64(1)]);
}
