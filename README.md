# a3s-orm

`a3s-orm` is a type-safe SQL query builder for Rust, inspired by Kysely. It models tables and columns in Rust, builds immutable queries, compiles them for a SQL dialect, and executes them through an async driver.

It is not an Active Record framework: records do not own persistence behavior, queries remain explicit, and application values are always passed as bound parameters.

## Why a3s-orm

- Catch table, column, and value-type mistakes at compile time.
- Keep query construction independent from database drivers.
- Inspect or transport compiled SQL without opening a connection.
- Run SQLite operations without blocking the Tokio runtime.
- Decode selected columns into their inferred Rust tuple types.
- Keep transactions isolated from concurrent users of a cloned SQLite executor.
- Add drivers without coupling them to the query API.

## Quick start

```toml
[dependencies]
a3s-orm = { git = "https://github.com/A3S-Lab/ORM" }
```

Define the schema and build a query:

```rust
use a3s_orm::{orm_table, select_from, PostgresDialect, Query};

orm_table! {
    pub struct Person => "person" {
        id: i64 => "id",
        name: String => "name",
        age: i32 => "age",
    }
}

let query = select_from::<Person>()
    .select((Person::id(), Person::name()))
    .filter(Person::age().gte(18))
    .order_by(Person::name().asc())
    .limit(20);

let compiled = query.compile(&PostgresDialect)?;
assert_eq!(
    compiled.sql,
    "SELECT \"person\".\"id\", \"person\".\"name\" FROM \"person\" WHERE \"person\".\"age\" >= $1 ORDER BY \"person\".\"name\" ASC LIMIT $2"
);
# Ok::<(), a3s_orm::Error>(())
```

Execute against SQLite:

```rust,no_run
use a3s_orm::{Database, SqliteDialect, SqliteExecutor};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let executor = SqliteExecutor::open("app.db").await?;
let database = Database::new(SqliteDialect, executor);
// database.execute(query).await?;
# Ok(())
# }
```

Use the scoped transaction API for application work. It commits on success
and rolls back on an operation error. If its Tokio task is cancelled, the
transaction retains the connection gate until its fallback rollback finishes.

Disable the default SQLite driver for compile-only or custom-driver use:

```toml
a3s-orm = { git = "https://github.com/A3S-Lab/ORM", default-features = false }
```

## Current capabilities

| Area | Support |
| --- | --- |
| Typed tables, columns, and bound values | Yes |
| Immutable SELECT, INSERT, UPDATE, DELETE builders | Yes |
| Filters, boolean expressions, ordering, pagination | Yes |
| Inner, left, right, and full joins | Yes |
| PostgreSQL, SQLite, and MySQL SQL compilation | Yes |
| Async executor abstraction | Yes |
| Non-blocking SQLite driver | Yes |
| Pooled PostgreSQL driver with prepared statement cache | Yes |
| Cancellation-safe scoped PostgreSQL transactions | Yes |
| Typed scalar, tuple, nullable, and checked integer decoding | Yes |
| Cancellation-safe scoped SQLite transactions | Yes |
| Nested SQLite savepoints with cancellation cleanup | Yes |
| Locked, checksummed SQLite and PostgreSQL migrations | Yes |
| CTEs, scalar/IN subqueries, and correlated EXISTS | Yes |
| Selection aliases, aggregates, GROUP BY, and HAVING | Yes |
| Plugins and window functions | Planned |
| MySQL runtime driver | Planned |

MySQL compilation intentionally rejects `RETURNING`, which that dialect does not support. Dialect support does not imply that a runtime driver is bundled.

## Migrations

Migrations are sorted by version, checksummed with SHA-256, and recorded in `a3s_orm_migrations`. Re-running an unchanged set is a no-op; changing or removing an applied migration is an error.

```rust,no_run
use a3s_orm::{Migration, Migrator, SqliteExecutor};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let executor = SqliteExecutor::open("app.db").await?;
let report = Migrator::new(executor)
    .run([Migration::new(
        "001",
        "create people",
        "create table person (id integer primary key, name text not null)",
    )])
    .await?;
println!("applied: {:?}", report.applied);
# Ok(())
# }
```

SQLite serializes migrators with its shared connection gate and `BEGIN IMMEDIATE`. PostgreSQL uses a transaction-scoped advisory lock. Migration SQL and its history row commit atomically.

## Architecture

The crate separates responsibilities so the public query API does not depend on a database client:

```text
typed schema + expressions
          |
    immutable query AST
          |
    dialect compiler
          |
     CompiledQuery
          |
 async Executor / driver
```

See [Architecture](docs/architecture.md) for module ownership and extension points.

## Status

This is an early foundation, not a claim of feature parity with Kysely. The roadmap prioritizes window functions, set operations, plugins, broader PostgreSQL type support, and a MySQL runtime driver. See [Roadmap](docs/roadmap.md).

## Development

```bash
cargo fmt --all -- --check
cargo test --all-features
cargo test --no-default-features
cargo clippy --all-targets --all-features -- -D warnings
```

## License

MIT
