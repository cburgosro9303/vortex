//! Cache metrics recording.

use metrics::{counter, gauge, histogram};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// Registra las metricas de cache.
/// Llamar una vez al inicio para registrar las metricas.
pub fn register_cache_metrics() {
    // Describir metricas
    metrics::describe_counter!("vortex_cache_hits_total", "Total number of cache hits");
    metrics::describe_counter!("vortex_cache_misses_total", "Total number of cache misses");
    metrics::describe_counter!(
        "vortex_cache_evictions_total",
        "Total number of cache evictions"
    );
    metrics::describe_gauge!("vortex_cache_entries", "Current number of entries in cache");
    metrics::describe_histogram!(
        "vortex_cache_operation_seconds",
        "Time spent on cache operations"
    );
}

/// Recorder de metricas de cache.
/// Usa atomic counters internos para maximo rendimiento.
#[derive(Debug, Clone)]
pub struct CacheMetrics {
    hits: Arc<AtomicU64>,
    misses: Arc<AtomicU64>,
}

impl CacheMetrics {
    pub fn new() -> Self {
        Self {
            hits: Arc::new(AtomicU64::new(0)),
            misses: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Registra un cache hit
    pub fn record_hit(&self) {
        self.hits.fetch_add(1, Ordering::Relaxed);
        counter!("vortex_cache_hits_total").increment(1);
    }

    /// Registra un cache miss
    pub fn record_miss(&self) {
        self.misses.fetch_add(1, Ordering::Relaxed);
        counter!("vortex_cache_misses_total").increment(1);
    }

    /// Registra una eviction
    pub fn record_eviction(&self, reason: &str) {
        counter!("vortex_cache_evictions_total", "reason" => reason.to_string()).increment(1);
    }

    /// Actualiza el gauge de entries
    pub fn update_entry_count(&self, count: u64) {
        gauge!("vortex_cache_entries").set(count as f64);
    }

    /// Registra la duracion de una operacion
    pub fn record_operation_duration(&self, operation: &str, duration: Duration) {
        histogram!(
            "vortex_cache_operation_seconds",
            "operation" => operation.to_string()
        )
        .record(duration.as_secs_f64());
    }

    /// Helper para medir tiempo de operacion
    pub fn time_operation<T, F: FnOnce() -> T>(&self, operation: &str, f: F) -> T {
        let start = Instant::now();
        let result = f();
        self.record_operation_duration(operation, start.elapsed());
        result
    }

    /// Calcula hit rate (para logging/debugging)
    pub fn hit_rate(&self) -> f64 {
        let hits = self.hits.load(Ordering::Relaxed) as f64;
        let misses = self.misses.load(Ordering::Relaxed) as f64;
        let total = hits + misses;
        if total == 0.0 { 0.0 } else { hits / total }
    }

    /// Retorna el numero de hits
    pub fn hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }

    /// Retorna el numero de misses
    pub fn misses(&self) -> u64 {
        self.misses.load(Ordering::Relaxed)
    }
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_metrics_hit_rate() {
        let metrics = CacheMetrics::new();

        // 3 hits, 1 miss = 75% hit rate
        metrics.record_hit();
        metrics.record_hit();
        metrics.record_hit();
        metrics.record_miss();

        let rate = metrics.hit_rate();
        assert!((rate - 0.75).abs() < 0.001);
    }

    #[test]
    fn test_operation_timing() {
        let metrics = CacheMetrics::new();

        let result = metrics.time_operation("test_op", || {
            std::thread::sleep(Duration::from_millis(10));
            42
        });

        assert_eq!(result, 42);
    }

    #[test]
    fn test_hit_miss_counters() {
        let metrics = CacheMetrics::new();

        assert_eq!(metrics.hits(), 0);
        assert_eq!(metrics.misses(), 0);

        metrics.record_hit();
        metrics.record_hit();
        metrics.record_miss();

        assert_eq!(metrics.hits(), 2);
        assert_eq!(metrics.misses(), 1);
    }
}
