# Roadmap

The project is being developed incrementally. Completed items describe implemented behavior, not full Kysely compatibility.

## Available

- Typed table and column declarations.
- Type-checked bound column values.
- Immutable CRUD query builders and joins.
- PostgreSQL, SQLite, and MySQL compilation.
- Driver-neutral async execution.
- Tokio-safe SQLite execution and integration tests.
- Typed result decoding for scalars, tuples, nullable values, and checked integers.
- Cancellation-safe SQLite transaction scopes that exclude concurrent executor clones.
- Nested SQLite savepoints with cancellation-safe cleanup.
- Pooled PostgreSQL execution with prepared statement caching.
- PostgreSQL scalar/null decoding and target-aware integer parameters.
- Cancellation-safe scoped PostgreSQL transactions.
- Deterministic SHA-256 migration validation and drift detection.
- Atomic, concurrency-locked SQLite and PostgreSQL migration backends.
- Typed CTEs, scalar and `IN` subqueries, and correlated `EXISTS`.
- Selection aliases, basic aggregates, grouping, and having clauses.
- PostgreSQL UUID, JSON/JSONB, temporal, Numeric, and nullable array round trips.
- Multi-row inserts and PostgreSQL/SQLite conflict handling.

## Next

- Window functions and set operations.
- Safe raw SQL fragments with bound parameters.
- PostgreSQL enum, range, network, and custom-domain codecs.

## Later

- Typed schema-definition builders.
- Query transformation plugins.
- MySQL and additional runtime drivers.
- Compile-fail coverage for schema misuse.
- Broader parity work guided by concrete application needs.
