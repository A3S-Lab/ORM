<p align="center">
  <img src="https://raw.githubusercontent.com/A3S-Lab/ORM/main/assets/readme/hero.svg" width="100%" alt="A3S ORM turns typed Rust schemas and predicates into parameterized SQL for async PostgreSQL and SQLite execution">
</p>

<p align="center">
  <strong>Explicit queries. Compile-time constraints. Async PostgreSQL and SQLite.</strong>
</p>

<p align="center">
  <a href="https://github.com/A3S-Lab/ORM/actions/workflows/ci.yml"><img alt="CI status" src="https://github.com/A3S-Lab/ORM/actions/workflows/ci.yml/badge.svg?branch=main"></a>
  <a href="https://github.com/A3S-Lab/ORM/releases/latest"><img alt="Latest release" src="https://img.shields.io/github/v/release/A3S-Lab/ORM?display_name=tag&amp;sort=semver&amp;style=flat-square&amp;color=5b8cff"></a>
  <img alt="Rust 1.85 or newer" src="https://img.shields.io/badge/rust-1.85%2B-9b7bff?style=flat-square">
  <a href="LICENSE"><img alt="MIT license" src="https://img.shields.io/badge/license-MIT-0b1020?style=flat-square"></a>
</p>

<p align="center">
  <a href="https://a3s-lab.github.io/ORM/">Documentation</a> ·
  <a href="#the-contract">The contract</a> ·
  <a href="#quick-start">Quick start</a> ·
  <a href="#capability-map">Capabilities</a> ·
  <a href="#drivers-and-dialects">Drivers</a> ·
  <a href="#migrations">Migrations</a> ·
  <a href="#architecture">Architecture</a>
</p>

---

**A3S ORM** is a type-safe, executor-neutral SQL query builder for Rust,
inspired by [Kysely](https://kysely.dev/). Table declarations constrain
columns, values, assignments, and decoded results at compile time. Immutable
builders compile into SQL plus bound parameters, then execute through an async
driver-neutral interface.

Despite the name, this is not an Active Record framework. Records do not own
persistence behavior, queries remain visible, and runtime values are never
interpolated into generated SQL.

The [documentation website](https://a3s-lab.github.io/ORM/) provides complete
Chinese and English guides for v0.3.1, v0.3.0, and v0.2.0, including same-page
language and version switching.

## The contract

Define the schema once, compose with typed columns, and inspect the exact query
before it reaches a connection:

```rust
use a3s_orm::{orm_table, select_from, OrderDirection, PostgresDialect, Query};

orm_table! {
    pub struct Person => "person" {
        id: i64 => "id",
        name: String => "name",
        age: i32 => "age",
    }
}

fn main() -> Result<(), a3s_orm::Error> {
    let query = select_from::<Person>()
        .select((Person::id(), Person::name()))
        .filter(Person::age().gte(18))
        .order_by(Person::name(), OrderDirection::Asc)
        .limit(20)
        .compile(&PostgresDialect)?;

    println!("sql = {}", query.sql);
    println!("parameters = {:?}", query.parameters);
    assert_eq!(query.parameters.len(), 2);
    Ok(())
}
```

```text
sql = select "person"."id", "person"."name" from "person" where ("person"."age" >= $1) order by "person"."name" asc limit $2
parameters = [I64(18), U64(20)]
```

Column ownership and Rust value families are checked before compilation. The
dialect owns quoting, placeholders, and feature support; unsupported syntax is
rejected instead of approximated.

## Quick start

### Install

SQLite is the default runtime:

```toml
[dependencies]
a3s-orm = "0.3"
tokio = { version = "1", features = ["macros", "rt-multi-thread"] }
```

Enable the bundled PostgreSQL driver instead:

```toml
a3s-orm = { version = "0.3", default-features = false, features = ["postgres"] }
```

Or use the query builder and dialect compilers without a bundled runtime:

```toml
a3s-orm = { version = "0.3", default-features = false }
```

Version `0.2.1` is the maintenance backport for applications constrained to
the `0.2` compatibility line. New integrations should use `0.3`.

The `postgres` feature includes UUID, JSON/JSONB, Chrono date/time types,
`rust_decimal::Decimal`, and `SqlArray<T>`.

### Execute a typed SQLite round trip

The default feature is enough for a real in-memory database:

```rust
use a3s_orm::{
    insert_into, orm_table, select_from, Database, SqliteDialect, SqliteExecutor,
};

orm_table! {
    struct Person => "person" {
        id: i64 => "id",
        name: String => "name",
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = SqliteExecutor::open_in_memory().await?;
    executor
        .execute_schema(
            "create table person (id integer primary key, name text not null)",
        )
        .await?;

    let database = Database::new(SqliteDialect, executor);
    database
        .execute(
            insert_into::<Person>()
                .value(Person::id(), 1)
                .value(Person::name(), "Ada"),
        )
        .await?;

    let name: String = database
        .fetch_one_as(
            select_from::<Person>()
                .select(Person::name())
                .filter(Person::id().eq(1)),
        )
        .await?;

    assert_eq!(name, "Ada");
    Ok(())
}
```

## Capability map

- **Typed structure** — schema markers constrain columns, joins, filters,
  inserts, updates, and result decoding.
- **Composable SQL** — immutable SELECT, INSERT, UPDATE, and DELETE builders
  cover joins, CTEs, `UPDATE FROM`, typed expression assignments, subqueries,
  aggregates, windows, set operations, functions, casts, and conflict handling.
- **Explicit concurrency** — PostgreSQL row locks, table locks, advisory locks,
  transaction isolation, access mode, and timeouts remain typed operations.
- **Checked results** — scalar, tuple, nullable, array, UUID, JSON, temporal,
  and decimal values decode through checked conversions.
- **Cancellation-safe execution** — scoped SQLite and PostgreSQL transactions
  retain their connection until rollback cleanup completes.
- **Deterministic migrations** — ordered, checksummed migrations run atomically
  behind a bounded database lock.
- **Controlled escape hatch** — `sql_query::<Output>` accepts reviewed static
  SQL while dynamic values still enter through `bind`.

### PostgreSQL worker queues stay typed

Lock clauses, CTEs, update sources, and expression assignments are AST nodes
rather than appended SQL strings. That keeps candidate selection and lease
acquisition in one parameterized statement:

```rust
use a3s_orm::{
    orm_table, select_from, update_table, OrderDirection, PostgresDialect, Query,
};

orm_table! {
    struct Job => "jobs" {
        id: i64 => "id",
        state: String => "state",
        attempt_count: i32 => "attempt_count",
    }
}

orm_table! {
    struct JobCandidate => "job_candidate" {
        id: i64 => "id",
    }
}

fn main() -> Result<(), a3s_orm::Error> {
    let candidates = select_from::<Job>()
        .select(Job::id())
        .filter(Job::state().eq("ready"))
        .order_by(Job::id(), OrderDirection::Asc)
        .limit(1)
        .for_update_of::<Job>()
        .skip_locked()
        .as_cte::<JobCandidate>();
    let query = update_table::<Job>()
        .with(candidates)
        .set(Job::state(), "leased")
        .set_expression(Job::attempt_count(), Job::attempt_count() + 1)
        .from::<JobCandidate>()
        .filter(Job::id().eq_column(JobCandidate::id()))
        .returning((Job::id(), Job::attempt_count()))
        .compile(&PostgresDialect)?;

    assert!(query.sql.contains("for update of \"jobs\" skip locked"));
    assert!(query.sql.contains("update \"jobs\""));
    assert!(query.sql.contains("from \"job_candidate\""));
    Ok(())
}
```

Transaction-scoped `advisory_xact_lock(namespace, key)` covers logical
resources that do not have a row yet. Retry classification identifies
serialization, deadlock, lock contention, failover, connection loss, and pool
saturation without automatically replaying writes. See
[PostgreSQL HA Controls](docs/postgres-ha.md) for the complete contract.

## Drivers and dialects

| Capability | PostgreSQL | SQLite | MySQL |
| --- | :---: | :---: | :---: |
| SQL compilation | Yes | Yes | Yes |
| Bundled async driver | Yes | Yes | No |
| `RETURNING` | Yes | Yes | Rejected |
| `ON CONFLICT` | Yes | Yes | Rejected |
| `UPDATE FROM` | Yes | Yes | Rejected |
| Row and table locks | Yes | Rejected | Rejected |
| Transactions | Yes | Yes | — |
| Locked migrations | Advisory lock | `BEGIN IMMEDIATE` | — |
| UUID, JSON, temporal, decimal, arrays | Yes | SQLite-native subset | — |

**SQLite** uses a Tokio-safe single connection. File databases default to WAL,
foreign-key enforcement, and a five-second busy timeout. Nested savepoints and
scoped transactions prevent later work from racing cancellation cleanup.

**PostgreSQL** uses a bounded Deadpool pool with prepared-statement caching.
The driver exposes typed transaction policy, stable label-free health metrics,
retry classification, verified rustls connections, and health-gated atomic TLS
pool rotation. `connect_no_tls` is intended for local or separately secured
connections.

MySQL support currently means SQL generation only. It does not imply a bundled
runtime driver. Read [Production Readiness](docs/production-readiness.md) for
the precise deployment scope and limitations.

## Migrations

Migrations are sorted by version, checksummed with SHA-256, and recorded in
`a3s_orm_migrations`. Re-running an unchanged set is a no-op; modifying or
removing an applied migration is an error.

```rust
use a3s_orm::{Migration, Migrator, SqliteExecutor};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let executor = SqliteExecutor::open_in_memory().await?;
    let report = Migrator::new(executor)
        .run([Migration::new(
            "001",
            "create people",
            "create table person (id integer primary key, name text not null)",
        )])
        .await?;

    assert_eq!(report.applied, vec!["001"]);
    Ok(())
}
```

Serving processes that deliberately have no DDL authority use the same
manifest and ledger through `Migrator::verify_required`. That call performs no
table creation, lock acquisition, or ledger write: every supplied migration
must already exist with its exact checksum. Additional applied migrations are
admitted so an older binary can participate in an explicitly
expand-compatible rolling upgrade.

```rust
# use a3s_orm::{Migration, Migrator, SqliteExecutor};
# async fn admit(executor: SqliteExecutor) -> Result<(), Box<dyn std::error::Error>> {
Migrator::new(executor)
    .verify_required([Migration::new(
        "001",
        "create people",
        "create table person (id integer primary key, name text not null)",
    )])
    .await?;
# Ok(())
# }
```

SQLite coordinates migrators through its connection gate and
`BEGIN IMMEDIATE`. PostgreSQL uses a transaction-scoped advisory lock with a
bounded deadline. Migration SQL and its history entry commit atomically.

## Architecture

The query API does not depend on a database client:

```text
typed schema + expressions
          │
    immutable query AST
          │
     dialect compiler
          │
  SQL + bound parameters
          │
 async Executor / driver
```

The compiler never opens a connection, and drivers never need to understand
typed builder state. A new dialect implements `Dialect`; a new runtime
implements `Executor`. See [Architecture](docs/architecture.md) for module
ownership and extension rules.

## Production boundaries

The library makes unsupported behavior visible rather than silently falling
back:

- the bundled SQLite executor serializes work on one connection;
- MySQL has a compiler but no bundled runtime driver;
- migrations are forward-only;
- scalar function and cast result types are explicit caller assertions;
- typed DDL builders, query plugins, custom PostgreSQL domain codecs, and
  schema code generation are not included yet.

Review [Production Readiness](docs/production-readiness.md) before deployment,
[PostgreSQL HA Controls](docs/postgres-ha.md) for pool and failover policy, and
the [Roadmap](docs/roadmap.md) for planned work.

## Development

The test suite runs real SQLite databases and PostgreSQL 17 services. CI checks
the feature matrix, compile-fail doctests, Rust 1.85 MSRV, strict Clippy,
warning-free rustdoc, dependency advisories, and at least 90% line coverage.

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

The versioned bilingual documentation site lives in `website/`:

```bash
cd website
npm ci
npm run check
npm run build
npm run check:site
```

## License

[MIT License](https://github.com/A3S-Lab/ORM/blob/main/LICENSE)
