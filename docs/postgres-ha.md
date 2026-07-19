# PostgreSQL HA Controls

This document describes the PostgreSQL controls intended for concurrent,
rolling, and failover-prone deployments. The API makes policy explicit but
never retries an arbitrary transaction automatically.

## Bounded pool construction

`PostgresPoolOptions` configures connection count plus wait, create, and recycle
deadlines. The built-in constructors default to bounded 30-second wait/create
and five-second recycle timeouts.

```rust,no_run
use std::time::Duration;
use a3s_orm::{PostgresExecutor, PostgresPoolOptions};

# fn build() -> Result<PostgresExecutor, a3s_orm::PostgresError> {
let pool = PostgresPoolOptions::new(32)
    .with_wait_timeout(Some(Duration::from_secs(2)))
    .with_create_timeout(Some(Duration::from_secs(5)))
    .with_recycle_timeout(Some(Duration::from_secs(2)));
let executor = PostgresExecutor::connect_no_tls_with(
    "postgres://postgres@127.0.0.1/app",
    pool,
)?;
# Ok(executor)
# }
```

Use `connection()` instead of acquiring directly from `pool()` when low-level
client access is required and should be included in ORM acquisition metrics.
In 0.2, `pool()` returns a cheap clone of the active Deadpool generation rather
than a borrowed reference, and `from_pool` is no longer a `const fn`. A pool
clone retained across rotation continues to refer to the draining old
generation.

## Typed transaction semantics

`begin_with` and `transaction_with` apply transaction isolation, access mode,
and timeout settings before application statements execute. Settings use
transaction-local `set_config` calls and cannot leak to a later pool user.

```rust,no_run
use std::time::Duration;
use a3s_orm::{
    PostgresExecutor, PostgresIsolationLevel, PostgresTransactionAccessMode,
    PostgresTransactionOptions, Transaction,
};

# async fn run(executor: &PostgresExecutor) -> Result<(), a3s_orm::PostgresError> {
let options = PostgresTransactionOptions::new()
    .with_isolation_level(PostgresIsolationLevel::Serializable)
    .with_access_mode(PostgresTransactionAccessMode::ReadOnly)
    .with_statement_timeout(Duration::from_secs(5))
    .with_lock_timeout(Duration::from_millis(500))
    .with_idle_in_transaction_timeout(Duration::from_secs(10));
let transaction = executor.begin_with(options).await?;
// Execute typed reads through `transaction`.
transaction.commit().await?;
# Ok(())
# }
```

The timeout values are validated against PostgreSQL's integer GUC range.
`Duration::ZERO` is accepted for transaction settings because PostgreSQL
defines zero as an explicit timeout disable. A non-zero sub-millisecond
transaction timeout rounds up to one millisecond instead of silently disabling
the timeout. Pool and migration-lock deadlines reject zero.

## Pool health and metrics

`pool_status()` returns current capacity, checked-out connections, waiters, and
a saturation flag. `pool_metrics()` adds cumulative counters and bounded
latency aggregates:

- acquisition attempts, successes, failures, cancellations, total latency, and
  maximum latency;
- health-check attempts, successes, failures, total latency, and maximum
  latency;
- serialization, deadlock, lock-contention, failover, connection-loss,
  pool-saturation, and permanent failures;
- pool-rotation attempts, successes, failures, and active generation.

The snapshot contains no labels, URLs, hostnames, users, SQL, or credentials.
Applications should attach only their own bounded deployment identity when
exporting it. Do not use connection strings or arbitrary error messages as
metric labels.

`health_check()` includes a measured acquisition and `SELECT 1` round trip. A
successful probe reports the active pool generation and current status.

## Verified TLS and certificate rotation

`PostgresTlsOptions` accepts a PEM root bundle and an optional client
certificate/private-key pair. It rejects empty or invalid PEM, requires
`sslmode=require`, rejects Unix-socket hosts, verifies the server name through
rustls, redacts all PEM from `Debug`, and zeroizes the final private-key copy on
drop.

Read certificate files asynchronously in the application, then pass their
bytes to the typed options:

```rust,no_run
use a3s_orm::{PostgresExecutor, PostgresPoolOptions, PostgresTlsOptions};

# async fn read_secret(_path: &str) -> std::io::Result<Vec<u8>> { Ok(Vec::new()) }
# async fn connect() -> Result<PostgresExecutor, Box<dyn std::error::Error>> {
let ca = read_secret("/run/secrets/postgres-ca.pem").await?;
let client_certificate =
    read_secret("/run/secrets/postgres-client-cert.pem").await?;
let client_key = read_secret("/run/secrets/postgres-client-key.pem").await?;
let tls = PostgresTlsOptions::new(ca)
    .with_client_identity(client_certificate, client_key);
let executor = PostgresExecutor::connect_tls(
    "postgres://app@db.internal/app?sslmode=require",
    PostgresPoolOptions::new(32),
    &tls,
)?;
# Ok(executor)
# }
```

For rotation, load and validate the replacement material and call
`rotate_tls`. The replacement pool must complete a TLS connection and live
health probe before it is installed. Installation increments the generation
and closes the old pool to new acquisitions; already checked-out connections
may complete naturally.

```rust,no_run
# use a3s_orm::{PostgresExecutor, PostgresPoolOptions, PostgresTlsOptions};
# async fn rotate(
#     executor: &PostgresExecutor,
#     replacement: &PostgresTlsOptions,
# ) -> Result<(), a3s_orm::PostgresError> {
let generation = executor
    .rotate_tls(
        "postgres://app@db.internal/app?sslmode=require",
        PostgresPoolOptions::new(32),
        replacement,
    )
    .await?;
assert!(generation > 0);
# Ok(())
# }
```

The raw connection URL is parsed into the pool configuration but is never
emitted. The upstream parser's error types identify the invalid field without
echoing a potentially credential-bearing value.

## Retry classification

`PostgresError::retry_class()` distinguishes:

| Class | Typical SQLSTATE or source | Retryable |
| --- | --- | :---: |
| `SerializationConflict` | `40001` | Yes |
| `Deadlock` | `40P01` | Yes |
| `LockContention` | `55P03`, including configured lock deadlines | Yes |
| `Failover` | `57P01`, `57P02`, `57P03` | Yes |
| `ConnectionLoss` | SQLSTATE class `08`, closed connection, I/O failure | Yes |
| `PoolSaturated` | pool wait timeout | Yes |
| `Permanent` | validation, constraint, syntax, and unsupported-value errors | No |

`PostgresTransactionError<PostgresError>` and `PostgresMigrationError` expose
the same classification. A retryable result is only permission for
application-owned policy to consider a retry. The caller must still prove that
the operation is idempotent, apply a bounded attempt/deadline budget, and use
backoff. Commit ambiguity must be resolved from application identity or durable
receipts before replaying writes. When both an operation and its cleanup fail,
the primary operation's retry classification remains authoritative.

## Migration lock and expand/contract deployments

PostgreSQL migrations use a transaction-scoped advisory lock. The default lock
deadline is 30 seconds. Configure an application-specific lock identity and
deadline when multiple independent schemas share one database:

```rust,no_run
use std::time::Duration;
use a3s_orm::{PostgresExecutor, PostgresMigrationOptions};

# fn configure(executor: PostgresExecutor) -> Result<PostgresExecutor, a3s_orm::PostgresError> {
let executor = executor.with_migration_options(
    PostgresMigrationOptions::new()
        .with_advisory_lock_id(0x4150_505f_4442)
        .with_lock_timeout(Duration::from_secs(10)),
)?;
# Ok(executor)
# }
```

A lock deadline returns `PostgresMigrationError::LockTimeout` and is classified
as lock contention rather than pool saturation. Migration SQL and its history
row still commit atomically.

For mixed-version rolling deployments:

1. **Expand**: add nullable columns, additive tables/indexes, or compatible
   defaults. Do not rename or remove fields used by the old version.
2. **Migrate**: deploy code that can read old and new shapes, dual-write when
   required, and backfill in bounded batches outside the schema migration.
3. **Verify**: prove old and new application versions can both read and write
   while observing backfill and error metrics.
4. **Contract**: only after the old version is fully drained, stop compatibility
   writes and remove obsolete columns in a later migration.

Never combine an incompatible contract step with its expand step. Checksums
make migration history immutable; create a new forward migration for every
phase.

## Verification

The PostgreSQL 17 CI gate exercises:

- transaction-local isolation, read-only mode, and all three timeouts;
- deterministic concurrent serializable writers;
- pool exhaustion, acquisition/health metrics, and atomic pool rotation;
- SQLSTATE failover plus a terminated backend and unreachable TCP endpoint;
- bounded advisory-lock contention;
- an old/new expand-contract compatibility window;
- a generated short-lived CA, verified rustls connection, and live TLS pool
  rotation.

The live server credentials and CA are generated only inside the isolated CI
job and are not emitted to logs or metrics. The unit-test identity is public,
intentionally insecure fixture data assembled in memory and is never trusted by
the integration server.
