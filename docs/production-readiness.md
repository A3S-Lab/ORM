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
- SQLite savepoint cleanup blocks later outer-transaction work until cleanup finishes.
- Migrations are ordered, checksummed, locked, atomic, and reject modified or missing applied versions.
- PostgreSQL integer and array conversions are range checked.
- CI tests no-default, individual extended-type, PostgreSQL-only, and all-feature builds.
- CI runs compile-fail doctests, strict Clippy, warning-free rustdoc, Rust 1.85 MSRV, cargo-audit, SQLite integration tests, and PostgreSQL 17 integration tests.
- CI measures all-feature line coverage against real SQLite and PostgreSQL databases and rejects changes below 90%.

## Deployment checklist

1. Construct PostgreSQL pools with the TLS connector and certificate policy required by the deployment. `connect_no_tls` is intended for local or separately secured connections.
2. Set pool sizes and timeouts on the Deadpool pool passed to `PostgresExecutor::from_pool`.
3. Review `SqliteOptions` for the workload. File databases default to WAL, foreign-key enforcement, and a five-second busy timeout.
4. Run migrations before serving traffic and treat checksum mismatch as an operational incident rather than editing migration history.
5. Use typed builders by default. Restrict `sql_query` text to reviewed static SQL and bind all runtime values.
6. Pin and audit the application lockfile in addition to the crate's CI audit.

## Current limitations

- The bundled SQLite executor serializes work on one connection; it is not a multi-connection SQLite pool.
- PostgreSQL TLS policy is supplied by the application rather than selected by this crate.
- Set-operation operands with their own CTE, ordering, limit, or offset are rejected until portable parenthesized operands are implemented.
- Migrations are forward-only. Automated down migrations are deliberately not provided.
- MySQL has a compiler but no bundled runtime driver.
- Typed DDL builders, query plugins, custom PostgreSQL domain codecs, and schema code generation are not yet included.
- Table and CTE alias markers are declared by the user, so their declared column shapes must match the aliased source.

These limitations are API boundaries, not silent fallbacks: unsupported clauses and values return explicit errors.
