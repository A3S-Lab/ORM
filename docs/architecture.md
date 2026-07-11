# Architecture

`a3s-orm` is organized around a one-way dependency flow from typed query construction to execution.

## Module ownership

- `schema` defines table identity and references.
- `expression` defines typed columns, predicates, and ordering.
- `query` owns immutable statement builders, split by SQL statement kind.
- `ast` is the internal representation shared by builders and compilers. It is not public API.
- `compiler` turns the AST into SQL and bound parameters. `dialect` owns identifier quoting, placeholders, and feature flags.
- `executor` defines driver-neutral async execution and the `Database` facade.
- `drivers/<driver>` owns database-client adaptation, row representation, and driver-specific errors.
- `value` is the common parameter and untyped result-value boundary.

The compiler never opens a connection, and drivers never need to understand typed builder state. This allows compile-only use and keeps new runtime integrations local to `drivers`.

## Safety boundaries

Identifiers originate from schema metadata and are validated and quoted by the dialect. Application values are represented as `Value` parameters and are never interpolated into SQL. `execute_schema` on the SQLite driver is deliberately marked as trusted SQL because DDL cannot be represented by the current typed builders.

## Extension rules

A new dialect implements `Dialect`. A new runtime implements `Executor`; driver-specific row and error types remain inside its driver module. New SQL constructs should first extend the AST, then the relevant statement builder, and finally the compiler. Large compiler concerns should be split into statement and expression modules as the supported grammar grows.
