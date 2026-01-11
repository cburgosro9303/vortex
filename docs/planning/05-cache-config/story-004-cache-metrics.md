# Historia 004: Metricas de Cache

## Contexto y Objetivo

Las metricas son esenciales para entender el comportamiento del cache en produccion. Sin ellas, no podemos saber si el cache esta siendo efectivo, si el TTL es adecuado, o si necesitamos ajustar la capacidad.

Esta historia implementa la exposicion de metricas de cache en formato Prometheus:

1. **Hit/Miss counters**: Ratio de efectividad del cache
2. **Latency histograms**: Tiempo de respuesta para hits vs misses
3. **Size gauges**: Numero de entries y memoria estimada
4. **Eviction counters**: Entries removidas por TTL o capacidad

Para desarrolladores Java, esto es similar a Micrometer + Caffeine metrics, pero con la ventaja de que las metricas en Rust son zero-cost cuando no se usan.

---

## Alcance

### In Scope

- Integracion con crate `metrics` y `metrics-exporter-prometheus`
- Counters para hits, misses, evictions
- Histograms para latencias de operaciones
- Gauges para size y capacity
- Endpoint `/metrics` en formato Prometheus
- Dashboard basico de Grafana (opcional)

### Out of Scope

- Metricas distribuidas (multi-nodo)
- Push a Prometheus (solo pull)
- Metricas custom de negocio
- Alerting rules

---

## Criterios de Aceptacion

- [ ] `GET /metrics` retorna metricas en formato Prometheus
- [ ] Metricas incluyen: `cache_hits_total`, `cache_misses_total`
- [ ] Histogram `cache_operation_duration_seconds` con labels `operation`
- [ ] Gauge `cache_entries` con numero actual de entries
- [ ] Metricas actualizadas en tiempo real
- [ ] Bajo overhead (< 1% en latencia de requests)
- [ ] Tests de metricas pasan

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-server/src/
├── cache/
│   ├── mod.rs
│   ├── config_cache.rs
│   ├── invalidation.rs
│   └── metrics.rs        # Nuevo
├── handlers/
│   └── metrics.rs        # Endpoint /metrics
└── middleware/
    └── metrics.rs        # Request timing middleware
```

### Metricas Definidas

| Metrica | Tipo | Labels | Descripcion |
|---------|------|--------|-------------|
| `vortex_cache_hits_total` | Counter | - | Total de cache hits |
| `vortex_cache_misses_total` | Counter | - | Total de cache misses |
| `vortex_cache_evictions_total` | Counter | `reason` | Evictions por TTL/capacity |
| `vortex_cache_entries` | Gauge | - | Entries actuales en cache |
| `vortex_cache_operation_seconds` | Histogram | `operation` | Latencia de operaciones |
| `vortex_http_requests_total` | Counter | `method`, `path`, `status` | Requests HTTP |
| `vortex_http_request_duration_seconds` | Histogram | `method`, `path` | Latencia de requests |

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencias

```toml
# crates/vortex-server/Cargo.toml
[dependencies]
metrics = "0.22"
metrics-exporter-prometheus = "0.13"
```

### Paso 2: Inicializar Prometheus Exporter

```rust
// src/metrics/mod.rs
pub mod cache;
pub mod http;
pub mod setup;

pub use setup::init_metrics;
```

```rust
// src/metrics/setup.rs
use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use tracing::info;

/// Inicializa el sistema de metricas y retorna el handle para el endpoint.
pub fn init_metrics() -> PrometheusHandle {
    let builder = PrometheusBuilder::new();

    // Configurar buckets para histogramas
    let handle = builder
        .set_buckets(&[
            0.001, 0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0
        ])
        .expect("failed to set histogram buckets")
        .install_recorder()
        .expect("failed to install metrics recorder");

    info!("metrics system initialized");
    handle
}
```

### Paso 3: Implementar Cache Metrics

```rust
// src/cache/metrics.rs
use metrics::{counter, gauge, histogram};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

/// Registra las metricas de cache.
/// Llamar una vez al inicio para registrar las metricas.
pub fn register_cache_metrics() {
    // Describir metricas
    metrics::describe_counter!(
        "vortex_cache_hits_total",
        "Total number of cache hits"
    );
    metrics::describe_counter!(
        "vortex_cache_misses_total",
        "Total number of cache misses"
    );
    metrics::describe_counter!(
        "vortex_cache_evictions_total",
        "Total number of cache evictions"
    );
    metrics::describe_gauge!(
        "vortex_cache_entries",
        "Current number of entries in cache"
    );
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
        counter!("vortex_cache_evictions_total", "reason" => reason.to_string())
            .increment(1);
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
        ).record(duration.as_secs_f64());
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
        if total == 0.0 {
            0.0
        } else {
            hits / total
        }
    }
}

impl Default for CacheMetrics {
    fn default() -> Self {
        Self::new()
    }
}
```

### Paso 4: Integrar Metricas en ConfigCache

```rust
// src/cache/config_cache.rs (modificacion)
use crate::cache::metrics::CacheMetrics;

#[derive(Clone)]
pub struct ConfigCache {
    inner: Cache<CacheKey, Arc<ConfigResponse>>,
    metrics: CacheMetrics,
}

impl ConfigCache {
    pub fn new(config: CacheConfig) -> Self {
        let metrics = CacheMetrics::new();

        // Configurar listener para evictions
        let eviction_metrics = metrics.clone();
        let cache = Cache::builder()
            .max_capacity(config.max_capacity)
            .time_to_live(Duration::from_secs(config.ttl_seconds))
            .eviction_listener(move |_key, _value, cause| {
                let reason = match cause {
                    moka::notification::RemovalCause::Expired => "ttl",
                    moka::notification::RemovalCause::Size => "capacity",
                    moka::notification::RemovalCause::Explicit => "manual",
                    moka::notification::RemovalCause::Replaced => "replaced",
                };
                eviction_metrics.record_eviction(reason);
            })
            .build();

        Self {
            inner: cache,
            metrics,
        }
    }

    pub async fn get(&self, key: &CacheKey) -> Option<Arc<ConfigResponse>> {
        let start = Instant::now();

        let result = self.inner.get(key).await;

        if result.is_some() {
            self.metrics.record_hit();
        } else {
            self.metrics.record_miss();
        }

        self.metrics.record_operation_duration("get", start.elapsed());
        self.update_entry_gauge();

        result
    }

    pub async fn get_or_insert_with<F, Fut>(
        &self,
        key: CacheKey,
        init: F,
    ) -> Result<Arc<ConfigResponse>, CacheError>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<ConfigResponse, CacheError>>,
    {
        let start = Instant::now();

        // Verificar si existe primero
        if let Some(cached) = self.inner.get(&key).await {
            self.metrics.record_hit();
            self.metrics.record_operation_duration("get_or_insert_hit", start.elapsed());
            return Ok(cached);
        }

        self.metrics.record_miss();

        // Fetch desde backend
        let value = self.inner
            .try_get_with(key, async {
                let response = init().await?;
                Ok(Arc::new(response))
            })
            .await
            .map_err(|e| CacheError::FetchError(e.to_string()))?;

        self.metrics.record_operation_duration("get_or_insert_miss", start.elapsed());
        self.update_entry_gauge();

        Ok(value)
    }

    fn update_entry_gauge(&self) {
        self.metrics.update_entry_count(self.inner.entry_count());
    }

    /// Retorna las metricas para acceso externo
    pub fn metrics(&self) -> &CacheMetrics {
        &self.metrics
    }
}
```

### Paso 5: Implementar HTTP Metrics Middleware

```rust
// src/middleware/metrics.rs
use axum::{
    body::Body,
    extract::MatchedPath,
    http::Request,
    middleware::Next,
    response::Response,
};
use metrics::{counter, histogram};
use std::time::Instant;

/// Middleware que registra metricas HTTP para cada request.
pub async fn http_metrics_middleware(
    matched_path: Option<MatchedPath>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let start = Instant::now();
    let method = request.method().to_string();
    let path = matched_path
        .map(|p| p.as_str().to_string())
        .unwrap_or_else(|| request.uri().path().to_string());

    let response = next.run(request).await;

    let status = response.status().as_u16().to_string();
    let duration = start.elapsed();

    // Registrar metricas
    counter!(
        "vortex_http_requests_total",
        "method" => method.clone(),
        "path" => path.clone(),
        "status" => status
    ).increment(1);

    histogram!(
        "vortex_http_request_duration_seconds",
        "method" => method,
        "path" => path
    ).record(duration.as_secs_f64());

    response
}

/// Registra las metricas HTTP
pub fn register_http_metrics() {
    metrics::describe_counter!(
        "vortex_http_requests_total",
        "Total number of HTTP requests"
    );
    metrics::describe_histogram!(
        "vortex_http_request_duration_seconds",
        "HTTP request duration in seconds"
    );
}
```

### Paso 6: Crear Endpoint /metrics

```rust
// src/handlers/metrics.rs
use axum::response::IntoResponse;
use metrics_exporter_prometheus::PrometheusHandle;

/// Handler para el endpoint /metrics
pub async fn metrics_handler(
    prometheus: axum::extract::State<PrometheusHandle>,
) -> impl IntoResponse {
    prometheus.render()
}
```

### Paso 7: Registrar Rutas y Middleware

```rust
// src/server.rs (modificacion)
use axum::middleware;
use crate::middleware::metrics::http_metrics_middleware;
use crate::handlers::metrics::metrics_handler;
use crate::metrics::{init_metrics, cache::register_cache_metrics, http::register_http_metrics};

pub fn create_app(state: AppState) -> Router {
    // Inicializar metricas
    let prometheus_handle = init_metrics();
    register_cache_metrics();
    register_http_metrics();

    Router::new()
        // Rutas de negocio
        .route("/health", get(health_check))
        .route("/{app}/{profile}", get(get_config))
        .route("/{app}/{profile}/{label}", get(get_config_with_label))
        // Ruta de metricas
        .route("/metrics", get(metrics_handler))
        // Middleware de metricas
        .layer(middleware::from_fn(http_metrics_middleware))
        .with_state(state)
        .with_state(prometheus_handle)
}
```

---

## Conceptos de Rust Aprendidos

### 1. Atomic Types (AtomicU64, AtomicUsize)

Los tipos atomicos permiten operaciones thread-safe sin locks, ideales para contadores de alto rendimiento.

**Rust:**
```rust
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

// AtomicU64 para contador thread-safe
struct Metrics {
    hits: AtomicU64,
}

impl Metrics {
    fn new() -> Self {
        Self {
            hits: AtomicU64::new(0),
        }
    }

    fn record_hit(&self) {
        // fetch_add retorna el valor anterior y suma atomicamente
        self.hits.fetch_add(1, Ordering::Relaxed);
    }

    fn get_hits(&self) -> u64 {
        self.hits.load(Ordering::Relaxed)
    }
}

// Orderings:
// - Relaxed: Sin garantias de orden, maximo rendimiento
// - Acquire/Release: Para sincronizar con otras operaciones
// - SeqCst: Orden total, mas lento pero mas seguro
```

**Comparacion con Java:**
```java
import java.util.concurrent.atomic.AtomicLong;

class Metrics {
    private final AtomicLong hits = new AtomicLong(0);

    void recordHit() {
        hits.incrementAndGet();
    }

    long getHits() {
        return hits.get();
    }
}
```

**Diferencias clave:**

| Aspecto | AtomicU64 (Rust) | AtomicLong (Java) |
|---------|------------------|-------------------|
| Memory ordering | Explicito | Siempre SeqCst (mas lento) |
| Signed/Unsigned | Unsigned por defecto | Siempre signed |
| Performance | Optimo con Relaxed | No configurable |
| Overflow | Wrap around definido | Wrap around definido |

### 2. Metrics Crate Patterns

El crate `metrics` usa un patron declarativo para registrar metricas.

**Rust:**
```rust
use metrics::{counter, gauge, histogram, describe_counter};

// Describir metrica (opcional pero recomendado)
describe_counter!("my_counter", "Description of counter");

// Incrementar counter
counter!("my_counter").increment(1);

// Counter con labels
counter!(
    "http_requests_total",
    "method" => "GET",
    "status" => "200"
).increment(1);

// Gauge (valor que puede subir o bajar)
gauge!("active_connections").set(42.0);
gauge!("active_connections").increment(1.0);
gauge!("active_connections").decrement(1.0);

// Histogram (distribucion de valores)
histogram!("request_duration_seconds").record(0.123);

// Con labels dinamicos
fn record_request(method: &str, path: &str, duration: f64) {
    histogram!(
        "http_request_duration_seconds",
        "method" => method.to_string(),
        "path" => path.to_string()
    ).record(duration);
}
```

**Comparacion con Java (Micrometer):**
```java
import io.micrometer.core.instrument.*;

// Counter
Counter counter = Counter.builder("my_counter")
    .description("Description of counter")
    .register(registry);
counter.increment();

// Counter con tags
Counter.builder("http_requests_total")
    .tag("method", "GET")
    .tag("status", "200")
    .register(registry)
    .increment();

// Gauge
Gauge.builder("active_connections", connectionPool, Pool::getActiveCount)
    .register(registry);

// Timer (equivalente a histogram para duraciones)
Timer timer = Timer.builder("request_duration")
    .register(registry);
timer.record(Duration.ofMillis(123));
```

### 3. Eviction Listeners en Moka

Moka permite registrar callbacks cuando entries son removidas.

**Rust:**
```rust
use moka::future::Cache;
use moka::notification::RemovalCause;

let metrics = CacheMetrics::new();
let metrics_clone = metrics.clone();

let cache: Cache<String, String> = Cache::builder()
    .max_capacity(1000)
    .time_to_live(Duration::from_secs(300))
    // Listener llamado en cada eviction
    .eviction_listener(move |key, value, cause| {
        match cause {
            RemovalCause::Expired => {
                // Entry expiro por TTL
                metrics_clone.record_eviction("ttl");
            }
            RemovalCause::Size => {
                // Cache lleno, entry removida por LFU
                metrics_clone.record_eviction("capacity");
            }
            RemovalCause::Explicit => {
                // Removida manualmente con invalidate()
                metrics_clone.record_eviction("manual");
            }
            RemovalCause::Replaced => {
                // Valor reemplazado por insert()
                metrics_clone.record_eviction("replaced");
            }
        }
    })
    .build();
```

**Comparacion con Java (Caffeine):**
```java
Cache<String, String> cache = Caffeine.newBuilder()
    .maximumSize(1000)
    .expireAfterWrite(5, TimeUnit.MINUTES)
    .evictionListener((key, value, cause) -> {
        switch (cause) {
            case EXPIRED -> metrics.recordEviction("ttl");
            case SIZE -> metrics.recordEviction("capacity");
            case EXPLICIT -> metrics.recordEviction("manual");
            case REPLACED -> metrics.recordEviction("replaced");
        }
    })
    .build();
```

### 4. Prometheus Exposition Format

El formato Prometheus es texto plano con formato especifico.

```prometheus
# HELP vortex_cache_hits_total Total number of cache hits
# TYPE vortex_cache_hits_total counter
vortex_cache_hits_total 12345

# HELP vortex_cache_operation_seconds Time spent on cache operations
# TYPE vortex_cache_operation_seconds histogram
vortex_cache_operation_seconds_bucket{operation="get",le="0.001"} 8000
vortex_cache_operation_seconds_bucket{operation="get",le="0.005"} 10000
vortex_cache_operation_seconds_bucket{operation="get",le="0.01"} 11000
vortex_cache_operation_seconds_bucket{operation="get",le="+Inf"} 12345
vortex_cache_operation_seconds_sum{operation="get"} 45.678
vortex_cache_operation_seconds_count{operation="get"} 12345
```

---

## Riesgos y Errores Comunes

### 1. Labels con alta cardinalidad

```rust
// MAL: User ID como label = millones de series temporales
counter!(
    "requests_total",
    "user_id" => user_id  // NUNCA hacer esto!
).increment(1);

// BIEN: Solo labels con valores limitados
counter!(
    "requests_total",
    "method" => method,   // GET, POST, PUT, DELETE, ...
    "status" => status    // 200, 400, 500, ...
).increment(1);
```

### 2. Olvidar clonar metrics para closures

```rust
// MAL: No compila porque metrics no puede moverse a closure
let metrics = CacheMetrics::new();
cache.eviction_listener(move |_, _, cause| {
    metrics.record_eviction(cause);  // ERROR: metrics movido!
});
cache.other_method(&metrics);  // ERROR: metrics ya movido

// BIEN: Clonar antes de mover
let metrics = CacheMetrics::new();
let metrics_for_listener = metrics.clone();
cache.eviction_listener(move |_, _, cause| {
    metrics_for_listener.record_eviction(cause);
});
cache.other_method(&metrics);  // OK: tenemos nuestra copia
```

### 3. Metricas no registradas

```rust
// MAL: Metrica usada pero no descrita
counter!("my_counter").increment(1);
// Funciona pero no tendra HELP text en output

// BIEN: Describir primero
describe_counter!("my_counter", "What this counter measures");
counter!("my_counter").increment(1);
```

### 4. Blocking en hot path

```rust
// MAL: Operaciones costosas en el path de metricas
fn record_request(&self, request: &Request) {
    let serialized = serde_json::to_string(request).unwrap();  // Costoso!
    counter!("requests", "body" => serialized).increment(1);
}

// BIEN: Solo labels simples y baratos
fn record_request(&self, method: &str, path: &str) {
    counter!("requests", "method" => method, "path" => path).increment(1);
}
```

---

## Pruebas

### Tests Unitarios

```rust
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
        // Verificar que se registro algo > 10ms
    }

    #[tokio::test]
    async fn test_cache_records_hits_and_misses() {
        let cache = ConfigCache::new(CacheConfig::default());
        let key = CacheKey::new("app", "prod", "main");

        // Miss
        let _ = cache.get(&key).await;

        // Insert and hit
        cache.insert(key.clone(), ConfigResponse::default()).await;
        let _ = cache.get(&key).await;

        let metrics = cache.metrics();
        assert_eq!(metrics.hits.load(Ordering::Relaxed), 1);
        assert_eq!(metrics.misses.load(Ordering::Relaxed), 1);
    }
}
```

### Tests de Integracion

```rust
#[tokio::test]
async fn test_metrics_endpoint() {
    let app = create_test_app().await;

    // Hacer algunos requests para generar metricas
    for _ in 0..10 {
        let _ = app
            .clone()
            .oneshot(Request::get("/myapp/prod").body(Body::empty()).unwrap())
            .await;
    }

    // Verificar endpoint de metricas
    let response = app
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let body = to_string(response).await;

    // Verificar que contiene metricas esperadas
    assert!(body.contains("vortex_cache_hits_total"));
    assert!(body.contains("vortex_http_requests_total"));
}

#[tokio::test]
async fn test_metrics_include_labels() {
    let app = create_test_app().await;

    // Request especifico
    let _ = app
        .clone()
        .oneshot(
            Request::get("/myapp/production/main")
                .body(Body::empty())
                .unwrap()
        )
        .await;

    let response = app
        .oneshot(Request::get("/metrics").body(Body::empty()).unwrap())
        .await
        .unwrap();

    let body = to_string(response).await;

    // Verificar labels
    assert!(body.contains("method=\"GET\""));
    assert!(body.contains("path=\"/{app}/{profile}/{label}\""));
}
```

---

## Observabilidad

### Ejemplo de Output Prometheus

```prometheus
# HELP vortex_cache_hits_total Total number of cache hits
# TYPE vortex_cache_hits_total counter
vortex_cache_hits_total 12543

# HELP vortex_cache_misses_total Total number of cache misses
# TYPE vortex_cache_misses_total counter
vortex_cache_misses_total 1287

# HELP vortex_cache_evictions_total Total number of cache evictions
# TYPE vortex_cache_evictions_total counter
vortex_cache_evictions_total{reason="ttl"} 523
vortex_cache_evictions_total{reason="capacity"} 12
vortex_cache_evictions_total{reason="manual"} 45

# HELP vortex_cache_entries Current number of entries in cache
# TYPE vortex_cache_entries gauge
vortex_cache_entries 4521

# HELP vortex_cache_operation_seconds Time spent on cache operations
# TYPE vortex_cache_operation_seconds histogram
vortex_cache_operation_seconds_bucket{operation="get",le="0.001"} 11000
vortex_cache_operation_seconds_bucket{operation="get",le="0.005"} 12000
vortex_cache_operation_seconds_bucket{operation="get",le="+Inf"} 12543
vortex_cache_operation_seconds_sum{operation="get"} 15.234
vortex_cache_operation_seconds_count{operation="get"} 12543

# HELP vortex_http_requests_total Total number of HTTP requests
# TYPE vortex_http_requests_total counter
vortex_http_requests_total{method="GET",path="/{app}/{profile}",status="200"} 9876
vortex_http_requests_total{method="GET",path="/health",status="200"} 1234
vortex_http_requests_total{method="DELETE",path="/cache/{app}/{profile}/{label}",status="200"} 45
```

### Queries Prometheus Utiles

```promql
# Hit rate
sum(rate(vortex_cache_hits_total[5m])) /
(sum(rate(vortex_cache_hits_total[5m])) + sum(rate(vortex_cache_misses_total[5m])))

# Latencia p99 de requests
histogram_quantile(0.99, sum(rate(vortex_http_request_duration_seconds_bucket[5m])) by (le))

# Requests por segundo
sum(rate(vortex_http_requests_total[1m]))

# Evictions por minuto
sum(rate(vortex_cache_evictions_total[1m])) by (reason)
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `crates/vortex-server/src/metrics/mod.rs` - Re-exports
2. `crates/vortex-server/src/metrics/setup.rs` - Inicializacion Prometheus
3. `crates/vortex-server/src/cache/metrics.rs` - CacheMetrics
4. `crates/vortex-server/src/cache/config_cache.rs` - Integracion con metricas
5. `crates/vortex-server/src/middleware/metrics.rs` - HTTP metrics middleware
6. `crates/vortex-server/src/handlers/metrics.rs` - Endpoint /metrics
7. `crates/vortex-server/tests/metrics_test.rs` - Tests

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server metrics

# Ejecutar y verificar
cargo run -p vortex-server &
curl http://localhost:8080/metrics | grep vortex

# Load test rapido
for i in {1..100}; do curl -s http://localhost:8080/myapp/prod > /dev/null; done
curl http://localhost:8080/metrics | grep cache_hits
```

### Configuracion Prometheus

```yaml
# prometheus.yml
scrape_configs:
  - job_name: 'vortex-config'
    static_configs:
      - targets: ['localhost:8080']
    metrics_path: '/metrics'
    scrape_interval: 15s
```

---

**Anterior**: [Historia 003 - Configuracion del Servidor](./story-003-server-config.md)
**Siguiente**: [Historia 005 - Benchmarks de Performance](./story-005-benchmarks.md)
