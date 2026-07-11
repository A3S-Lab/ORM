# Roadmap

The project is being developed incrementally. Completed items describe implemented behavior, not full Kysely compatibility.

## Available

- Typed table and column declarations.
- Type-checked bound column values.
- Immutable CRUD query builders and joins.
- PostgreSQL, SQLite, and MySQL compilation.
- Driver-neutral async execution.
- Tokio-safe SQLite execution and integration tests.

## Next

- Typed row decoding and column aliases.
- Transactions with driver-backed commit and rollback.
- Multi-row inserts and conflict handling.
- Functions, aggregates, grouping, and having clauses.
- Subqueries, common table expressions, and set operations.
- Safe raw SQL fragments with bound parameters.
- PostgreSQL runtime driver.

## Later

- Schema and migration APIs with migration locking.
- Query transformation plugins.
- MySQL and additional runtime drivers.
- Compile-fail coverage for schema misuse.
- Broader parity work guided by concrete application needs.
