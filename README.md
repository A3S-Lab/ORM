# A3S ORM

<p align="center">
  <strong>Type-Safe SQL for Rust</strong>
</p>

<p align="center">
  <em>Build explicit, immutable queries and execute them with async PostgreSQL or SQLite drivers</em>
</p>

<p align="center">
  <a href="#overview">Overview</a> •
  <a href="#features">Features</a> •
  <a href="#quick-start">Quick Start</a> •
  <a href="#database-drivers">Database Drivers</a> •
  <a href="#architecture">Architecture</a> •
  <a href="#development">Development</a>
</p>

---

## Overview

**A3S ORM** is a type-safe SQL query builder for Rust, inspired by
[Kysely](https://kysely.dev/). Rust table definitions constrain columns,
values, and decoded results at compile time. Queries compile into SQL plus
bound parameters and execute through an async, driver-neutral interface.

Despite the name, this is not an Active Record framework. Records do not own
persistence behavior, queries stay explicit, and runtime values are never
interpolated into generated SQL.

### Basic usage

```rust
use a3s_orm::{orm_table, select_from, OrderDirection, PostgresDialect, Query};

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
    .order_by(Person::name(), OrderDirection::Asc)
    .limit(20)
    .compile(&PostgresDialect)?;

assert_eq!(
    query.sql,
    "select \"person\".\"id\", \"person\".\"name\" from \"person\" where (\"person\".\"age\" >= $1) order by \"person\".\"name\" asc limit $2"
);
# Ok::<(), a3s_orm::Error>(())
```

## Features

- **Typed Schema**: Catch invalid columns, values, and assignments at compile time
- **Immutable Queries**: Build SELECT, INSERT, UPDATE, and DELETE statements explicitly
- **Safe Parameters**: Keep runtime values out of generated SQL
- **Advanced SQL**: Use joins, CTEs, subqueries, aggregates, windows, and set operations
- **Typed Results**: Decode scalar, tuple, nullable, array, and extended database values
- **Async Drivers**: Run non-blocking SQLite and pooled PostgreSQL operations on Tokio
- **Safe Transactions**: Roll back scoped work on errors and task cancellation
- **PostgreSQL HA Controls**: Select transaction semantics, bound pool waits, classify retryable failures, observe health, and rotate verified TLS pools
- **Migrations**: Apply locked, atomic, checksummed migrations
- **Extensible Runtime**: Add another database through the public `Executor` contract

### Support matrix

| Capability | PostgreSQL | SQLite | MySQL |
| --- | :---: | :---: | :---: |
| SQL compilation | Yes | Yes | Yes |
| Bundled async driver | Yes | Yes | No |
| `RETURNING` | Yes | Yes | Rejected |
| `ON CONFLICT` | Yes | Yes | Rejected |
| Transactions | Yes | Yes | — |
| Locked migrations | Advisory lock | `BEGIN IMMEDIATE` | — |
| UUID, JSON, temporal, decimal, arrays | Yes | SQLite-native subset | — |

MySQL support currently means SQL generation only; it does not imply a bundled
runtime driver. See [Production Readiness](docs/production-readiness.md) for the
precise supported scope and limitations.

## Quick Start

### Installation

Pin the released Git tag:

```toml
[dependencies]
a3s-orm = { git = "https://github.com/A3S-Lab/ORM", tag = "v0.2.0" }
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

SQLite is enabled by default. For a compile-only query builder without a
bundled driver:

```toml
a3s-orm = { git = "https://github.com/A3S-Lab/ORM", tag = "v0.2.0", default-features = false }
```

For PostgreSQL:

```toml
a3s-orm = { git = "https://github.com/A3S-Lab/ORM", tag = "v0.2.0", default-features = false, features = ["postgres"] }
```

The `postgres` feature includes UUID, JSON/JSONB, Chrono date/time types,
`rust_decimal::Decimal`, and `SqlArray<T>`.

### Insert, update, and delete

```rust
# use a3s_orm::{delete_from, insert_into, orm_table, update_table, PostgresDialect, Query};
# orm_table! { struct Person => "person" { id: i64 => "id", name: String => "name" } }
let insert = insert_into::<Person>()
    .value(Person::id(), 1)
    .value(Person::name(), "Ada")
    .returning(Person::id())
    .compile(&PostgresDialect)?;

let update = update_table::<Person>()
    .set(Person::name(), "Ada Lovelace")
    .filter(Person::id().eq(1))
    .compile(&PostgresDialect)?;

let delete = delete_from::<Person>()
    .filter(Person::id().eq(1))
    .compile(&PostgresDialect)?;
# Ok::<(), a3s_orm::Error>(())
```

Multi-row inserts use typed `InsertRow<T>` values. PostgreSQL and SQLite also
support conflict targets, `DO NOTHING`, bound updates, and values from the
`excluded` row.

### Typed results

A selection determines its Rust output type. `fetch_all_as`, `fetch_optional_as`,
and `fetch_one_as` decode that type and enforce the requested cardinality.
Checked integer conversion reports overflow with the result-column index.

For SQL outside the typed AST, `sql_query::<Output>` accepts reviewed static
SQL while runtime data enters through `bind`.

## Database Drivers

### SQLite

```rust,no_run
use a3s_orm::{Database, SqliteDialect, SqliteExecutor};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let executor = SqliteExecutor::open("app.db").await?;
let database = Database::new(SqliteDialect, executor);
# let _ = database;
# Ok(())
# }
```

File databases default to WAL journaling, foreign-key enforcement, and a
five-second busy timeout. `SqliteExecutor::open_with_options` allows each policy
to be changed. In-memory databases use memory journaling.

The driver serializes access to its connection without blocking Tokio. Scoped
transactions and nested savepoints retain the connection gate until cancellation
cleanup completes.

### PostgreSQL

```rust,no_run
use a3s_orm::{Database, PostgresDialect, PostgresExecutor};

# async fn example() -> Result<(), Box<dyn std::error::Error>> {
let executor = PostgresExecutor::connect_no_tls(
    "postgres://postgres:postgres@127.0.0.1/app",
    16,
)?;
let database = Database::new(PostgresDialect, executor);
# let _ = database;
# Ok(())
# }
```

`connect_no_tls` is intended for local or separately secured connections.
Production applications can use `connect_tls` with in-memory
`PostgresTlsOptions`, then atomically install verified replacement certificate
material through `rotate_tls`.

`PostgresTransactionOptions` selects isolation, read-only mode, and
transaction-local statement, lock, and idle timeouts. `PostgresPoolOptions`
bounds pool acquisition/creation/recycling. Stable label-free snapshots expose
pool saturation, acquisition latency, health, failure classes, and certificate
pool generations. See [PostgreSQL HA Controls](docs/postgres-ha.md) for the
complete deployment and retry contract.

## Migrations

Migrations are ordered by version, checksummed with SHA-256, and recorded in
`a3s_orm_migrations`. Re-running an unchanged set is a no-op. Modifying or
removing an applied migration is an error.

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

SQLite coordinates migrators through its connection gate and
`BEGIN IMMEDIATE`. PostgreSQL uses a transaction-scoped advisory lock with a
bounded configurable deadline. The migration SQL and history entry commit
atomically. Production rolling deployments should follow the documented
expand/migrate/verify/contract phases.

## Architecture

The query API does not depend on a database client:

```text
typed schema + expressions
          │
    immutable query AST
          │
     dialect compiler
          │
      CompiledQuery
          │
 async Executor / driver
```

Source is split by responsibility under `compiler/`, `query/`, `drivers/`, and
`migration/`. See [Architecture](docs/architecture.md) for module ownership and
extension points.

## Development

The integration suite executes SQL against real databases. SQLite tests use
actual in-memory and temporary file databases. PostgreSQL tests run against
PostgreSQL 17 services and exercise schema creation, prepared queries, typed
round trips, migrations, transactions, rollback, cancellation cleanup,
concurrent serializable writers, pool exhaustion, failover-like disconnects,
migration contention, mixed-version expand/contract compatibility, and
generated-CA TLS rotation.

CI runs the full feature matrix with `cargo llvm-cov` and fails when line
coverage falls below 90%.

```bash
cargo fmt --all -- --check
cargo test --no-default-features
cargo test --all-features
cargo clippy --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc --all-features --no-deps
```

To run PostgreSQL integration tests locally:

```bash
A3S_ORM_POSTGRES_URL=postgres://postgres:postgres@127.0.0.1:5432/a3s_orm \
  cargo test --all-features
```

See [Roadmap](docs/roadmap.md) for planned schema builders, plugins, additional
codecs, code generation, and the MySQL runtime driver.

## License

MIT
