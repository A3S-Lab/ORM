# Architecture

`a3s-orm` is organized around a one-way dependency flow from typed query construction to execution.

## Module ownership

- `schema` defines table identity and references.
- `expression` defines typed columns, predicates, and ordering.
- `function` owns typed aggregate expressions without coupling them to statement builders.
- `query` owns immutable statement builders, split by SQL statement kind.
- `ast` is the internal representation shared by builders and compilers. It is not public API.
- `compiler` turns the AST into SQL and bound parameters. `dialect` owns identifier quoting, placeholders, and feature flags.
- `compiler/validation` enforces statement structure before SQL is emitted, including multi-row column identity and conflict ownership.
- `decode` converts driver-neutral row values into inferred query output types, with checked numeric conversion.
- `executor` defines driver-neutral async execution, transaction contracts, and the `Database` facade.
- `drivers/<driver>` owns database-client adaptation, row representation, and driver-specific errors.
- `value` is the common parameter and untyped result-value boundary.

The compiler never opens a connection, and drivers never need to understand typed builder state. This allows compile-only use and keeps new runtime integrations local to `drivers`.

CTEs and subqueries remain AST nodes until dialect compilation. They share one compiler parameter accumulator, so PostgreSQL placeholders stay globally ordered across CTEs, outer predicates, and nested queries. CTE names, selection aliases, and function identifiers pass through the same identifier validation and quoting as schema identifiers.

Multi-row inserts store rows separately in the AST. Compilation verifies identical column ordering before flattening values into the shared parameter accumulator. Conflict assignments distinguish bound values from references to the `excluded` row; neither path interpolates application data. Dialects advertise conflict support explicitly, so unsupported MySQL syntax fails rather than being approximated.

## SQLite transaction isolation

Every `SqliteExecutor` clone shares a connection-level transaction gate. Normal execution acquires the gate for one operation; a transaction owns it from `BEGIN IMMEDIATE` through `commit` or `rollback`. Transaction statements use an internal unlocked path so they cannot deadlock themselves. Other clones wait at the gate and cannot interleave statements with the active transaction.

`SqliteExecutor::transaction` is the recommended application API. It commits on success, rolls back operation errors, and reports rollback failure without discarding the original operation error. Dropping an incomplete transaction schedules rollback and transfers the connection gate into that cleanup task. This makes Tokio task cancellation safe while a runtime is active. Explicit transactions remain available for infrastructure code that needs manual lifetime control.

Nested work uses `SqliteTransaction::savepoint`. A savepoint owns a second operation gate within its outer transaction. If its future is cancelled, cleanup retains that gate until `ROLLBACK TO SAVEPOINT` and `RELEASE SAVEPOINT` finish, so subsequent outer-transaction statements cannot race with cleanup.

## PostgreSQL execution

`PostgresExecutor` owns a Deadpool connection pool and uses its per-connection prepared-statement cache. Parameters are encoded after PostgreSQL has inferred their target types, which permits checked conversion of the common Rust integer representation into `smallint`, `integer`, or `bigint`. Rows are converted into the same driver-neutral values used by typed decoding.

Extended values remain explicit variants across the entire path: UUID, JSON, date/time, timestamp, timestamp with time zone, Decimal, and arrays are never converted to display strings by the PostgreSQL driver. `SqlArray<T>` separates SQL arrays from the `Vec<u8>` bytea representation. Array parameters are converted against the server-inferred element type with indexed conversion errors, and nullable array elements remain nullable during round trips.

`from_pool` accepts pools constructed with deployment-specific TLS connectors. `connect_no_tls` is a convenience for local development and explicitly does not choose a production TLS policy. Transactions retain one pooled connection from `BEGIN` through completion; cancellation cleanup retains that connection until rollback finishes.

## Migrations

The `migration` module validates definitions, sorts versions deterministically, and computes SHA-256 checksums before invoking a backend. The backend contract performs reconciliation and execution as one locked operation.

SQLite uses the executor's shared connection gate plus `BEGIN IMMEDIATE`. PostgreSQL uses `pg_advisory_xact_lock` on the same transaction that executes migrations. Both backends create the history table, compare every applied checksum, execute pending SQL, and insert history rows within one transaction. Database errors therefore cannot leave schema changes recorded only partially.

## Safety boundaries

Identifiers originate from schema metadata and are validated and quoted by the dialect. Application values are represented as `Value` parameters and are never interpolated into SQL. `execute_schema` on the SQLite driver is deliberately marked as trusted SQL because DDL cannot be represented by the current typed builders.

## Extension rules

A new dialect implements `Dialect`. A new runtime implements `Executor`; driver-specific row and error types remain inside its driver module. New SQL constructs should first extend the AST, then the relevant statement builder, and finally the compiler. Large compiler concerns should be split into statement and expression modules as the supported grammar grows.
