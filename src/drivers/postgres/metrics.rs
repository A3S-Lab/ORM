use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use super::PostgresRetryClass;

/// Eventual-consistency snapshot of the active PostgreSQL pool.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PostgresPoolStatus {
    pub generation: u64,
    pub max_size: usize,
    pub size: usize,
    pub available: usize,
    pub checked_out: usize,
    pub waiting: usize,
    pub saturated: bool,
}

impl PostgresPoolStatus {
    pub(crate) fn from_deadpool(status: deadpool_postgres::Status, generation: u64) -> Self {
        let checked_out = status.size.saturating_sub(status.available);
        let saturated =
            status.waiting > 0 || (status.max_size > 0 && checked_out >= status.max_size);
        Self {
            generation,
            max_size: status.max_size,
            size: status.size,
            available: status.available,
            checked_out,
            waiting: status.waiting,
            saturated,
        }
    }
}

/// Stable, label-free metrics suitable for exporting through an application
/// metrics system.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PostgresPoolMetricsSnapshot {
    pub pool: PostgresPoolStatus,
    pub acquisitions_in_flight: u64,
    pub acquisition_attempts: u64,
    pub acquisition_successes: u64,
    pub acquisition_failures: u64,
    pub acquisition_cancellations: u64,
    pub acquisition_latency_total: Duration,
    pub acquisition_latency_max: Duration,
    pub health_check_attempts: u64,
    pub health_check_successes: u64,
    pub health_check_failures: u64,
    pub health_check_latency_total: Duration,
    pub health_check_latency_max: Duration,
    pub operation_failures: u64,
    pub serialization_failures: u64,
    pub deadlock_failures: u64,
    pub lock_contention_failures: u64,
    pub failover_failures: u64,
    pub connection_failures: u64,
    pub pool_saturation_failures: u64,
    pub permanent_failures: u64,
    pub rotation_attempts: u64,
    pub rotation_successes: u64,
    pub rotation_failures: u64,
}

/// Result of an explicit PostgreSQL connection-health probe.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct PostgresPoolHealth {
    pub pool: PostgresPoolStatus,
    pub latency: Duration,
}

#[derive(Debug, Default)]
pub(crate) struct PostgresPoolMetrics {
    generation: AtomicU64,
    acquisitions_in_flight: AtomicU64,
    acquisition_attempts: AtomicU64,
    acquisition_successes: AtomicU64,
    acquisition_failures: AtomicU64,
    acquisition_cancellations: AtomicU64,
    acquisition_latency_total_nanos: AtomicU64,
    acquisition_latency_max_nanos: AtomicU64,
    health_check_attempts: AtomicU64,
    health_check_successes: AtomicU64,
    health_check_failures: AtomicU64,
    health_check_latency_total_nanos: AtomicU64,
    health_check_latency_max_nanos: AtomicU64,
    operation_failures: AtomicU64,
    serialization_failures: AtomicU64,
    deadlock_failures: AtomicU64,
    lock_contention_failures: AtomicU64,
    failover_failures: AtomicU64,
    connection_failures: AtomicU64,
    pool_saturation_failures: AtomicU64,
    permanent_failures: AtomicU64,
    rotation_attempts: AtomicU64,
    rotation_successes: AtomicU64,
    rotation_failures: AtomicU64,
}

impl PostgresPoolMetrics {
    pub(crate) fn generation(&self) -> u64 {
        self.generation.load(Ordering::Relaxed)
    }

    pub(crate) fn next_generation(&self) -> u64 {
        self.generation
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |generation| {
                Some(generation.saturating_add(1))
            })
            .unwrap_or_else(|generation| generation)
            .saturating_add(1)
    }

    pub(crate) fn start_acquisition(&self) -> AcquisitionMeasurement<'_> {
        saturating_add(&self.acquisition_attempts, 1);
        saturating_add(&self.acquisitions_in_flight, 1);
        AcquisitionMeasurement {
            metrics: self,
            started: Instant::now(),
            completed: false,
        }
    }

    pub(crate) fn record_health(&self, latency: Duration, succeeded: bool) {
        saturating_add(&self.health_check_attempts, 1);
        record_duration(
            &self.health_check_latency_total_nanos,
            &self.health_check_latency_max_nanos,
            latency,
        );
        if succeeded {
            saturating_add(&self.health_check_successes, 1);
        } else {
            saturating_add(&self.health_check_failures, 1);
        }
    }

    pub(crate) fn record_error(&self, class: PostgresRetryClass) {
        saturating_add(&self.operation_failures, 1);
        let counter = match class {
            PostgresRetryClass::SerializationConflict => &self.serialization_failures,
            PostgresRetryClass::Deadlock => &self.deadlock_failures,
            PostgresRetryClass::LockContention => &self.lock_contention_failures,
            PostgresRetryClass::Failover => &self.failover_failures,
            PostgresRetryClass::ConnectionLoss => &self.connection_failures,
            PostgresRetryClass::PoolSaturated => &self.pool_saturation_failures,
            PostgresRetryClass::Permanent => &self.permanent_failures,
        };
        saturating_add(counter, 1);
    }

    pub(crate) fn start_rotation(&self) {
        saturating_add(&self.rotation_attempts, 1);
    }

    pub(crate) fn finish_rotation(&self, succeeded: bool) {
        if succeeded {
            saturating_add(&self.rotation_successes, 1);
        } else {
            saturating_add(&self.rotation_failures, 1);
        }
    }

    pub(crate) fn snapshot(
        &self,
        deadpool_status: deadpool_postgres::Status,
    ) -> PostgresPoolMetricsSnapshot {
        PostgresPoolMetricsSnapshot {
            pool: PostgresPoolStatus::from_deadpool(deadpool_status, self.generation()),
            acquisitions_in_flight: load(&self.acquisitions_in_flight),
            acquisition_attempts: load(&self.acquisition_attempts),
            acquisition_successes: load(&self.acquisition_successes),
            acquisition_failures: load(&self.acquisition_failures),
            acquisition_cancellations: load(&self.acquisition_cancellations),
            acquisition_latency_total: duration(load(&self.acquisition_latency_total_nanos)),
            acquisition_latency_max: duration(load(&self.acquisition_latency_max_nanos)),
            health_check_attempts: load(&self.health_check_attempts),
            health_check_successes: load(&self.health_check_successes),
            health_check_failures: load(&self.health_check_failures),
            health_check_latency_total: duration(load(&self.health_check_latency_total_nanos)),
            health_check_latency_max: duration(load(&self.health_check_latency_max_nanos)),
            operation_failures: load(&self.operation_failures),
            serialization_failures: load(&self.serialization_failures),
            deadlock_failures: load(&self.deadlock_failures),
            lock_contention_failures: load(&self.lock_contention_failures),
            failover_failures: load(&self.failover_failures),
            connection_failures: load(&self.connection_failures),
            pool_saturation_failures: load(&self.pool_saturation_failures),
            permanent_failures: load(&self.permanent_failures),
            rotation_attempts: load(&self.rotation_attempts),
            rotation_successes: load(&self.rotation_successes),
            rotation_failures: load(&self.rotation_failures),
        }
    }
}

pub(crate) struct AcquisitionMeasurement<'a> {
    metrics: &'a PostgresPoolMetrics,
    started: Instant,
    completed: bool,
}

impl AcquisitionMeasurement<'_> {
    pub(crate) fn finish(mut self, succeeded: bool) {
        self.completed = true;
        self.metrics
            .finish_acquisition(self.started.elapsed(), succeeded, false);
    }
}

impl Drop for AcquisitionMeasurement<'_> {
    fn drop(&mut self) {
        if !self.completed {
            self.metrics
                .finish_acquisition(self.started.elapsed(), false, true);
        }
    }
}

impl PostgresPoolMetrics {
    fn finish_acquisition(&self, latency: Duration, succeeded: bool, cancelled: bool) {
        saturating_sub(&self.acquisitions_in_flight, 1);
        record_duration(
            &self.acquisition_latency_total_nanos,
            &self.acquisition_latency_max_nanos,
            latency,
        );
        if succeeded {
            saturating_add(&self.acquisition_successes, 1);
        } else {
            saturating_add(&self.acquisition_failures, 1);
        }
        if cancelled {
            saturating_add(&self.acquisition_cancellations, 1);
        }
    }
}

fn record_duration(total: &AtomicU64, maximum: &AtomicU64, value: Duration) {
    let nanos = value.as_nanos().min(u128::from(u64::MAX)) as u64;
    saturating_add(total, nanos);
    maximum.fetch_max(nanos, Ordering::Relaxed);
}

fn duration(nanos: u64) -> Duration {
    Duration::from_nanos(nanos)
}

fn load(value: &AtomicU64) -> u64 {
    value.load(Ordering::Relaxed)
}

fn saturating_add(value: &AtomicU64, amount: u64) {
    let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_add(amount))
    });
}

fn saturating_sub(value: &AtomicU64, amount: u64) {
    let _ = value.fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
        Some(current.saturating_sub(amount))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_checked_out_and_saturation_without_labels() {
        let status = PostgresPoolStatus::from_deadpool(
            deadpool_postgres::Status {
                max_size: 4,
                size: 4,
                available: 0,
                waiting: 2,
            },
            3,
        );
        assert_eq!(status.checked_out, 4);
        assert!(status.saturated);
        assert_eq!(status.generation, 3);
    }

    #[test]
    fn cancelled_acquisition_does_not_leave_in_flight_gauge_stuck() {
        let metrics = PostgresPoolMetrics::default();
        drop(metrics.start_acquisition());
        let snapshot = metrics.snapshot(deadpool_postgres::Status {
            max_size: 1,
            size: 0,
            available: 0,
            waiting: 0,
        });
        assert_eq!(snapshot.acquisitions_in_flight, 0);
        assert_eq!(snapshot.acquisition_failures, 1);
        assert_eq!(snapshot.acquisition_cancellations, 1);
    }

    #[test]
    fn generation_saturates_instead_of_wrapping() {
        let metrics = PostgresPoolMetrics::default();
        metrics.generation.store(u64::MAX, Ordering::Relaxed);
        assert_eq!(metrics.next_generation(), u64::MAX);
        assert_eq!(metrics.generation(), u64::MAX);
    }
}
