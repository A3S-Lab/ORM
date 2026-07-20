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
- Typed PostgreSQL isolation, read-only, statement, lock, and idle transaction controls.
- Bounded PostgreSQL pools with health, saturation, acquisition latency, and stable failure metrics.
- Verified rustls PostgreSQL connections with redacted in-memory certificate rotation.
- Typed PostgreSQL retry classification for serialization, deadlock, lock contention, failover, connection loss, pool saturation, and permanent errors.
- Deterministic SHA-256 migration validation and drift detection.
- Atomic, bounded-lock SQLite and PostgreSQL migration backends plus documented expand/contract deployments.
- Typed CTEs, scalar and `IN` subqueries, and correlated `EXISTS`.
- Selection aliases, basic aggregates, grouping, and having clauses.
- PostgreSQL UUID, JSON/JSONB, temporal, Numeric, and nullable array round trips.
- Multi-row inserts and PostgreSQL/SQLite conflict handling.
- Typed window functions, frames, and set operations.
- Explicit one/optional result cardinality and bound static raw queries.
- Typed source and join aliases.
- Configurable production-safe SQLite connection defaults.

## Next

- PostgreSQL enum, range, network, and custom-domain codecs.

## Later

- Typed schema-definition builders.
- Query transformation plugins.
- MySQL and additional runtime drivers.
- Compile-fail coverage for schema misuse.
- Broader parity work guided by concrete application needs.
