use a3s_orm::expression::Selection;
use a3s_orm::{
    bound, cast, coalesce, count, delete_from, dense_rank, exists, insert_into, least, lock_table,
    max, min, not, orm_table, rank, row_number, scalar_subquery, select_from, select_from_as,
    sql_function, sql_query, update_table, Dialect, InsertRow, OrderDirection, PostgresDialect,
    PostgresTableLockMode, Query, SelectionExt, SqliteDialect, Value, WindowBoundary,
    WindowFrameUnits,
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
    pub struct PersonAlias => "p" {
        id: i64 => "id",
        name: String => "name",
    }
}

orm_table! {
    pub struct PetAlias => "pt" {
        owner_id: i64 => "owner_id",
        name: String => "name",
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
    pub struct UpdateCandidate => "update_candidate" {
        id: i64 => "id",
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
fn compiles_postgres_row_locking_and_rejects_unsupported_dialects() {
    let query = select_from::<Person>()
        .select(Person::id())
        .filter(Person::age().gte(18))
        .order_by(Person::id(), OrderDirection::Asc)
        .limit(10)
        .for_update_of::<Person>()
        .skip_locked()
        .compile(&PostgresDialect)
        .unwrap();

    assert_eq!(
        query.sql,
        "select \"person\".\"id\" from \"person\" where (\"person\".\"age\" >= $1) order by \"person\".\"id\" asc limit $2 for update of \"person\" skip locked"
    );
    assert_eq!(query.parameters, vec![Value::I64(18), Value::U64(10)]);

    let no_wait = select_from::<Person>()
        .select(Person::id())
        .for_no_key_update()
        .no_wait()
        .compile(&PostgresDialect)
        .unwrap();
    assert!(no_wait.sql.ends_with("for no key update nowait"));

    let shared = select_from::<Person>()
        .select(Person::id())
        .inner_join::<Pet>(Person::id().eq_column(Pet::owner_id()))
        .for_share_of::<Person>()
        .for_share_of::<Pet>()
        .compile(&PostgresDialect)
        .unwrap();
    assert!(shared.sql.ends_with("for share of \"person\", \"pet\""));

    let key_shared = select_from::<Person>()
        .select(Person::id())
        .for_key_share()
        .compile(&PostgresDialect)
        .unwrap();
    assert!(key_shared.sql.ends_with("for key share"));

    let unsupported = select_from::<Person>()
        .select(Person::id())
        .for_update()
        .compile(&SqliteDialect)
        .unwrap_err();
    assert!(unsupported
        .to_string()
        .contains("does not support select row locking"));

    let invalid_target = select_from::<Person>()
        .select(Person::id())
        .for_update_of::<Pet>()
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(invalid_target.to_string().contains("lock target"));

    let aliases = select_from_as::<Person, PersonAlias>()
        .select(PersonAlias::id())
        .inner_join::<Pet>(PersonAlias::id().eq_column(Pet::owner_id()))
        .for_update_of::<PersonAlias>()
        .for_update_of::<Pet>()
        .for_update_of::<Pet>()
        .compile(&PostgresDialect)
        .unwrap();
    assert!(aliases.sql.ends_with("for update of \"p\", \"pet\""));

    let implicit_update = select_from::<Person>()
        .select(Person::id())
        .skip_locked()
        .compile(&PostgresDialect)
        .unwrap();
    assert!(implicit_update.sql.ends_with("for update skip locked"));

    let set_operation = select_from::<Person>()
        .select(Person::id())
        .union(select_from::<Person>().select(Person::id()))
        .for_update()
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(set_operation
        .to_string()
        .contains("cannot be combined with set operations"));

    let locked_operand = select_from::<Person>()
        .select(Person::id())
        .union(
            select_from::<Person>()
                .select(Person::id())
                .for_update()
                .no_wait(),
        )
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(locked_operand
        .to_string()
        .contains("operands with CTE, ordering, limit, offset, or row locking"));
}

#[test]
fn compiles_typed_postgres_table_locks_and_rejects_unsupported_dialects() {
    let query = lock_table::<Person>(PostgresTableLockMode::ShareRowExclusive)
        .no_wait()
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "lock table \"person\" in share row exclusive mode nowait"
    );
    assert!(query.parameters.is_empty());

    let unsupported = lock_table::<Person>(PostgresTableLockMode::AccessExclusive)
        .compile(&SqliteDialect)
        .unwrap_err();
    assert!(unsupported
        .to_string()
        .contains("does not support table locking"));

    for (mode, sql_mode) in [
        (PostgresTableLockMode::AccessShare, "access share"),
        (PostgresTableLockMode::RowShare, "row share"),
        (PostgresTableLockMode::RowExclusive, "row exclusive"),
        (
            PostgresTableLockMode::ShareUpdateExclusive,
            "share update exclusive",
        ),
        (PostgresTableLockMode::Share, "share"),
        (
            PostgresTableLockMode::ShareRowExclusive,
            "share row exclusive",
        ),
        (PostgresTableLockMode::Exclusive, "exclusive"),
        (PostgresTableLockMode::AccessExclusive, "access exclusive"),
    ] {
        let compiled = lock_table::<Person>(mode)
            .compile(&PostgresDialect)
            .unwrap();
        assert_eq!(
            compiled.sql,
            format!("lock table \"person\" in {sql_mode} mode")
        );
    }
}

#[test]
fn compiles_nested_typed_functions_casts_and_negation() {
    struct JsonPath;

    let json_path = cast::<String, JsonPath>(
        cast::<String, String>(bound::<String>("$.secrets[*] ? (@.version < 3)"), "text"),
        "jsonpath",
    );
    let predicate = sql_function::<bool>(
        "jsonb_path_exists",
        [Person::name().expression(), json_path.expression()],
    )
    .eq(true);
    let handled = select_from::<Pet>()
        .select(Pet::id())
        .filter(Pet::owner_id().eq_column(Person::id()));
    let query = select_from::<Person>()
        .select(Person::id())
        .filter(predicate)
        .filter(not(exists(handled)))
        .compile(&PostgresDialect)
        .unwrap();

    assert_eq!(
        query.sql,
        "select \"person\".\"id\" from \"person\" where ((\"jsonb_path_exists\"(\"person\".\"name\", cast(cast($1 as \"text\") as \"jsonpath\")) = $2) and not (exists (select \"pet\".\"id\" from \"pet\" where (\"pet\".\"owner_id\" = \"person\".\"id\"))))"
    );
    assert_eq!(
        query.parameters,
        vec![
            Value::String("$.secrets[*] ? (@.version < 3)".to_owned()),
            Value::Bool(true),
        ]
    );

    let invalid_function = select_from::<Person>()
        .select(sql_function::<i64>("invalid function", []))
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(invalid_function.to_string().contains("identifier"));

    let invalid_cast = select_from::<Person>()
        .select(cast::<String, String>(
            bound::<String>("value"),
            "text; drop table person",
        ))
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(invalid_cast.to_string().contains("SQL type"));
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
fn compiles_multi_row_insert_and_conflict_updates() {
    let insert = insert_into::<Person>()
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
        .on_conflict(Person::id())
        .do_update_from_excluded(Person::name())
        .do_update(Person::age(), 41)
        .returning(Person::id())
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        insert.sql,
        "insert into \"person\" (\"id\", \"name\", \"age\") values ($1, $2, $3), ($4, $5, $6) on conflict (\"id\") do update set \"name\" = excluded.\"name\", \"age\" = $7 returning \"person\".\"id\""
    );
    assert_eq!(
        insert.parameters,
        vec![
            Value::I64(1),
            Value::String("Ada".to_owned()),
            Value::I64(36),
            Value::I64(2),
            Value::String("Grace".to_owned()),
            Value::I64(40),
            Value::I64(41),
        ]
    );

    let do_nothing = insert_into::<Person>()
        .value(Person::id(), 1)
        .on_conflict(Person::id())
        .do_nothing()
        .compile(&SqliteDialect)
        .unwrap();
    assert_eq!(
        do_nothing.sql,
        "insert into \"person\" (\"id\") values (?) on conflict (\"id\") do nothing"
    );
}

#[test]
fn rejects_invalid_insert_rows_and_unsupported_conflicts() {
    let inconsistent = insert_into::<Person>()
        .rows([
            InsertRow::new()
                .value(Person::id(), 1)
                .value(Person::name(), "Ada"),
            InsertRow::new()
                .value(Person::name(), "Grace")
                .value(Person::id(), 2),
        ])
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(inconsistent.to_string().contains("differ"));

    let duplicate = insert_into::<Person>()
        .row(
            InsertRow::new()
                .value(Person::id(), 1)
                .value(Person::id(), 2),
        )
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(duplicate.to_string().contains("more than once"));

    let incomplete_conflict = insert_into::<Person>()
        .value(Person::id(), 1)
        .do_nothing()
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(incomplete_conflict.to_string().contains("target"));

    let mysql = insert_into::<Person>()
        .value(Person::id(), 1)
        .on_conflict(Person::id())
        .do_nothing()
        .compile(&a3s_orm::MysqlDialect)
        .unwrap_err();
    assert!(mysql.to_string().contains("does not support on conflict"));
}

#[test]
fn compiles_typed_set_operations_with_continuous_parameters() {
    let query = select_from::<Person>()
        .select(Person::name())
        .filter(Person::age().gte(18))
        .union_all(
            select_from::<Pet>()
                .select(Pet::name())
                .filter(Pet::name().like("A%")),
        )
        .except(
            select_from::<Person>()
                .select(Person::name())
                .filter(Person::age().lt(21)),
        )
        .limit(50)
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "select \"person\".\"name\" from \"person\" where (\"person\".\"age\" >= $1) union all select \"pet\".\"name\" from \"pet\" where (\"pet\".\"name\" like $2) except select \"person\".\"name\" from \"person\" where (\"person\".\"age\" < $3) limit $4"
    );
    assert_eq!(
        query.parameters,
        vec![
            Value::I64(18),
            Value::String("A%".to_owned()),
            Value::I64(21),
            Value::U64(50),
        ]
    );

    let unsupported = select_from::<Person>()
        .select(Person::name())
        .union(
            select_from::<Pet>()
                .select(Pet::name())
                .order_by(Pet::name(), OrderDirection::Asc),
        )
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(unsupported.to_string().contains("set-operation"));
}

#[test]
fn compiles_typed_window_functions_and_frames() {
    let query = select_from::<Person>()
        .select((
            Person::name(),
            row_number()
                .partition_by(Person::manager_id())
                .order_by(Person::age(), OrderDirection::Desc)
                .alias("position"),
            count(Person::id())
                .over()
                .partition_by(Person::manager_id())
                .order_by(Person::age(), OrderDirection::Asc)
                .frame(
                    WindowFrameUnits::Rows,
                    WindowBoundary::UnboundedPreceding,
                    WindowBoundary::CurrentRow,
                )
                .alias("running_count"),
        ))
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "select \"person\".\"name\", \"row_number\"() over (partition by \"person\".\"manager_id\" order by \"person\".\"age\" desc) as \"position\", \"count\"(\"person\".\"id\") over (partition by \"person\".\"manager_id\" order by \"person\".\"age\" asc rows between unbounded preceding and current row) as \"running_count\" from \"person\""
    );

    let invalid = select_from::<Person>()
        .select(row_number().frame(
            WindowFrameUnits::Rows,
            WindowBoundary::UnboundedFollowing,
            WindowBoundary::CurrentRow,
        ))
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(invalid.to_string().contains("window frame"));
}

#[test]
fn compiles_trusted_raw_sql_with_only_bound_runtime_values() {
    let query = sql_query::<(i64, String)>("select id, name from person where age >= ")
        .bind(18)
        .append(" and name <> ")
        .bind("Robert'); drop table person; --")
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "select id, name from person where age >= $1 and name <> $2"
    );
    assert!(!query.sql.contains("drop table"));
    assert_eq!(
        query.parameters,
        vec![
            Value::I64(18),
            Value::String("Robert'); drop table person; --".to_owned()),
        ]
    );

    let empty = sql_query::<()>("   ")
        .compile(&PostgresDialect)
        .unwrap_err();
    assert!(empty.to_string().contains("cannot be empty"));
}

#[test]
fn compiles_typed_source_and_join_aliases() {
    let query = select_from_as::<Person, PersonAlias>()
        .select((PersonAlias::name(), PetAlias::name()))
        .inner_join_as::<Pet, PetAlias>(PersonAlias::id().eq_column(PetAlias::owner_id()))
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "select \"p\".\"name\", \"pt\".\"name\" from \"person\" as \"p\" inner join \"pet\" as \"pt\" on (\"p\".\"id\" = \"pt\".\"owner_id\")"
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
fn composes_column_comparisons_scalar_subqueries_and_expression_ordering() {
    let minimum_pet_id = scalar_subquery(
        select_from::<Pet>()
            .select(min(Pet::id()))
            .filter(Pet::owner_id().eq_column(Person::id())),
    );
    let deadline = least::<i64>([
        coalesce::<i64>([minimum_pet_id.expression(), Person::id().expression()]).expression(),
        Person::id().expression(),
    ]);
    let query = select_from::<Person>()
        .select(Person::id())
        .filter(Person::id().ne_column(Person::manager_id()))
        .filter(Person::age().lt_column(Person::age()))
        .order_by_expression(deadline, OrderDirection::Asc)
        .compile(&PostgresDialect)
        .unwrap();

    assert_eq!(
        query.sql,
        "select \"person\".\"id\" from \"person\" where ((\"person\".\"id\" <> \"person\".\"manager_id\") and (\"person\".\"age\" < \"person\".\"age\")) order by least(coalesce((select \"min\"(\"pet\".\"id\") from \"pet\" where (\"pet\".\"owner_id\" = \"person\".\"id\")), \"person\".\"id\"), \"person\".\"id\") asc"
    );
    assert!(query.parameters.is_empty());

    assert!(select_from::<Person>()
        .select(coalesce::<i64>([]))
        .compile(&PostgresDialect)
        .unwrap_err()
        .to_string()
        .contains("at least one"));
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
fn compiles_locking_cte_update_from_with_expression_assignment() {
    let candidates = select_from::<Person>()
        .select(Person::id())
        .filter(Person::age().gte(18))
        .order_by(Person::id(), OrderDirection::Asc)
        .limit(5)
        .for_update()
        .skip_locked()
        .as_cte::<UpdateCandidate>();
    let query = update_table::<Person>()
        .with(candidates)
        .set_expression(Person::age(), Person::age() + 1)
        .set(Person::name(), "leased")
        .from::<UpdateCandidate>()
        .filter(Person::id().eq_column(UpdateCandidate::id()))
        .returning((Person::id(), Person::age()))
        .compile(&PostgresDialect)
        .unwrap();

    assert_eq!(
        query.sql,
        "with \"update_candidate\" as (select \"person\".\"id\" from \"person\" where (\"person\".\"age\" >= $1) order by \"person\".\"id\" asc limit $2 for update skip locked) update \"person\" set \"age\" = (\"person\".\"age\" + $3), \"name\" = $4 from \"update_candidate\" where (\"person\".\"id\" = \"update_candidate\".\"id\") returning \"person\".\"id\", \"person\".\"age\""
    );
    assert_eq!(
        query.parameters,
        vec![
            Value::I64(18),
            Value::U64(5),
            Value::I64(1),
            Value::String("leased".to_owned()),
        ]
    );

    let unsupported = update_table::<Person>()
        .set(Person::name(), "leased")
        .from::<UpdateCandidate>()
        .filter(Person::id().eq_column(UpdateCandidate::id()))
        .compile(&a3s_orm::MysqlDialect)
        .unwrap_err();
    assert!(unsupported
        .to_string()
        .contains("does not support update from"));
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
fn compiles_remaining_predicates_joins_aggregates_and_set_operations() {
    let query = select_from::<Person>()
        .distinct()
        .select((min(Person::age()), max(Person::age())))
        .left_join::<Pet>(Person::id().eq_column(Pet::owner_id()))
        .right_join::<Pet>(Person::id().eq_column(Pet::owner_id()))
        .full_join::<Pet>(Person::id().eq_column(Pet::owner_id()))
        .filter(Person::age().ne(10).or(Person::age().gt(20)))
        .filter(Person::age().lte(80))
        .filter(Person::manager_id().is_not_null())
        .group_by(Person::manager_id())
        .having(count(Person::id()).gt(1))
        .having(count(Person::id()).lte(10))
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "select distinct \"min\"(\"person\".\"age\"), \"max\"(\"person\".\"age\") from \"person\" left join \"pet\" on (\"person\".\"id\" = \"pet\".\"owner_id\") right join \"pet\" on (\"person\".\"id\" = \"pet\".\"owner_id\") full join \"pet\" on (\"person\".\"id\" = \"pet\".\"owner_id\") where (((\"person\".\"age\" <> $1) or (\"person\".\"age\" > $2)) and (\"person\".\"age\" <= $3) and \"person\".\"manager_id\" is not null) group by \"person\".\"manager_id\" having ((\"count\"(\"person\".\"id\") > $4) and (\"count\"(\"person\".\"id\") <= $5))"
    );

    let set = select_from::<Person>()
        .select(Person::name())
        .intersect(select_from::<Pet>().select(Pet::name()))
        .compile(&SqliteDialect)
        .unwrap();
    assert_eq!(
        set.sql,
        "select \"person\".\"name\" from \"person\" intersect select \"pet\".\"name\" from \"pet\""
    );
}

#[test]
fn compiles_all_window_units_boundaries_and_rank_functions() {
    let query = select_from::<Person>()
        .select((
            rank()
                .frame(
                    WindowFrameUnits::Range,
                    WindowBoundary::Preceding(2),
                    WindowBoundary::Following(3),
                )
                .alias("ranked"),
            dense_rank()
                .frame(
                    WindowFrameUnits::Groups,
                    WindowBoundary::CurrentRow,
                    WindowBoundary::UnboundedFollowing,
                )
                .alias("dense"),
        ))
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "select \"rank\"() over (range between 2 preceding and 3 following) as \"ranked\", \"dense_rank\"() over (groups between current row and unbounded following) as \"dense\" from \"person\""
    );
}

#[test]
fn exposes_schema_and_dialect_metadata_and_select_all() {
    let column = Person::name();
    assert_eq!(column.table_name(), "person");
    assert_eq!(column.name(), "name");
    assert_eq!(a3s_orm::TableRef::<Person>::new().name(), "person");
    assert_eq!(a3s_orm::TableRef::<Person>::default().name(), "person");

    assert_eq!(PostgresDialect.name(), "PostgreSQL");
    assert_eq!(PostgresDialect.identifier_quote(), '"');
    assert_eq!(PostgresDialect.placeholder(3), "$3");
    assert!(PostgresDialect.supports_returning());
    assert!(PostgresDialect.supports_on_conflict());
    assert!(PostgresDialect.supports_update_from());
    assert_eq!(SqliteDialect.name(), "SQLite");
    assert_eq!(SqliteDialect.placeholder(3), "?");
    assert!(SqliteDialect.supports_update_from());
    assert_eq!(a3s_orm::MysqlDialect.name(), "MySQL");
    assert_eq!(a3s_orm::MysqlDialect.identifier_quote(), '`');
    assert!(!a3s_orm::MysqlDialect.supports_returning());
    assert!(!a3s_orm::MysqlDialect.supports_on_conflict());
    assert!(!a3s_orm::MysqlDialect.supports_update_from());

    assert_eq!(
        select_from::<Person>()
            .select_all()
            .compile(&SqliteDialect)
            .unwrap()
            .sql,
        "select \"person\".* from \"person\""
    );
}

#[test]
fn compiles_all_typed_aggregate_comparisons() {
    let query = select_from::<Person>()
        .select(count(Person::id()))
        .having(count(Person::id()).eq(1))
        .having(count(Person::id()).ne(2))
        .having(count(Person::id()).gte(3))
        .having(count(Person::id()).lt(4))
        .compile(&PostgresDialect)
        .unwrap();
    assert_eq!(
        query.sql,
        "select \"count\"(\"person\".\"id\") from \"person\" having ((\"count\"(\"person\".\"id\") = $1) and (\"count\"(\"person\".\"id\") <> $2) and (\"count\"(\"person\".\"id\") >= $3) and (\"count\"(\"person\".\"id\") < $4))"
    );
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
