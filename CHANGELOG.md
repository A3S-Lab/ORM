# Changelog

## 0.3.1 - 2026-08-19

- Added the read-only `MigrationLedger` boundary and
  `Migrator::verify_required` schema admission for serving processes without
  DDL authority.
- Added SQLite and PostgreSQL coverage for missing migrations, checksum drift,
  and expand-compatible ledgers with additional applied migrations.
