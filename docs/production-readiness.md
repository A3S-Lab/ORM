# Production Readiness

This document defines the production-supported scope of `a3s-orm`. It does not claim complete Kysely feature parity.

## Supported deployments

- SQLite through the bundled Tokio-safe single-connection executor.
- PostgreSQL through the bundled Deadpool executor and a caller-selected TLS pool.
- Compile-only PostgreSQL, SQLite, and MySQL SQL generation.
- Custom runtimes implementing the public `Executor` contract.

## Enforced guarantees

- Runtime values are represented as parameters and are not interpolated into generated SQL.
- Schema columns constrain inserted, updated, filtered, and decoded Rust values.
- Typed one-row and optional-row APIs reject unexpected cardinality.
- SQLite and PostgreSQL scoped transactions roll back on operation errors and Tokio task cancellation.
- PostgreSQL transaction isolation, access mode, and statement/lock/idle timeouts are validated and applied transaction-locally.
- Built-in PostgreSQL pools have bounded wait/create/recycle controls plus label-free health, saturation, latency, failure, and rotation metrics.
- PostgreSQL errors classify serialization, deadlock, lock contention, failover, connection loss, pool saturation, and permanent failures without automatically replaying writes.
- Rustls PostgreSQL pools validate PEM roots and client identity, redact credential-bearing configuration, and health-gate atomic certificate rotation.
- SQLite savepoint cleanup blocks later outer-transaction work until cleanup finishes.
- Migrations are ordered, checksummed, bounded-lock, atomic, and reject modified or missing applied versions.
- PostgreSQL integer and array conversions are range checked.
- CI tests no-default, individual extended-type, PostgreSQL-only, and all-feature builds.
- CI runs compile-fail doctests, strict Clippy, warning-free rustdoc, Rust 1.85 MSRV, cargo-audit, SQLite integration tests, and PostgreSQL 17 integration tests.
- CI measures all-feature line coverage against real SQLite and PostgreSQL databases and rejects changes below 90%.

## Deployment checklist

1. Use `PostgresExecutor::connect_tls` with the deployment CA and optional client identity. `connect_no_tls` is intended for local or separately secured connections.
2. Set pool size and bounded acquisition/creation/recycle deadlines through `PostgresPoolOptions`.
3. Review `SqliteOptions` for the workload. File databases default to WAL, foreign-key enforcement, and a five-second busy timeout.
4. Configure the PostgreSQL advisory-lock identity/deadline, run expand migrations before serving new code, and defer contract migrations until old versions drain.
5. Use typed builders by default. Restrict `sql_query` text to reviewed static SQL and bind all runtime values.
6. Pin and audit the application lockfile in addition to the crate's CI audit.
7. Export label-free `pool_metrics()` values with bounded deployment labels, and apply retries only to idempotent operations under an application deadline.

## Current limitations

- The bundled SQLite executor serializes work on one connection; it is not a multi-connection SQLite pool.
- The bundled TLS path uses caller-supplied in-memory PEM material; applications own certificate retrieval and rotation scheduling.
- Retry classification does not automatically retry transactions or resolve commit ambiguity.
- Set-operation operands with their own CTE, ordering, limit, or offset are rejected until portable parenthesized operands are implemented.
- Migrations are forward-only. Automated down migrations are deliberately not provided.
- MySQL has a compiler but no bundled runtime driver.
- Typed DDL builders, query plugins, custom PostgreSQL domain codecs, and schema code generation are not yet included.
- Table and CTE alias markers are declared by the user, so their declared column shapes must match the aliased source.

These limitations are API boundaries, not silent fallbacks: unsupported clauses and values return explicit errors.
