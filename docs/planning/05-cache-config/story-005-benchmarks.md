# Historia 005: Benchmarks de Performance

## Contexto y Objetivo

Vortex Config tiene KPIs de performance estrictos: latencia p99 < 10ms y cold start < 500ms. Para garantizar estos objetivos y detectar regresiones, necesitamos una suite de benchmarks automatizados.

Esta historia implementa benchmarks usando Criterion, el framework de benchmarking estandar en Rust:

1. **Cache benchmarks**: Latencia de get/insert con diferentes cargas
2. **Serialization benchmarks**: JSON, YAML, Properties rendering
3. **End-to-end benchmarks**: Request completo con cache hit/miss
4. **Memory benchmarks**: Uso de memoria bajo diferentes cargas

Para desarrolladores Java, Criterion es similar a JMH (Java Microbenchmark Harness), pero con integracion nativa en Cargo y analisis estadistico automatico.

---

## Alcance

### In Scope

- Benchmarks de cache (get, insert, invalidate)
- Benchmarks de serializacion (JSON, YAML, Properties)
- Benchmarks de HTTP handlers
- Integracion con CI para detectar regresiones
- Documentacion de resultados baseline

### Out of Scope

- Load testing (muchos clientes concurrentes)
- Stress testing (hasta el limite)
- Benchmarks de backends externos (Git, S3)
- Profiling detallado (flamegraphs)

---

## Criterios de Aceptacion

- [ ] `cargo bench` ejecuta todos los benchmarks
- [ ] Benchmarks de cache cubren get, insert, invalidate
- [ ] Benchmarks de serializacion cubren JSON, YAML, Properties
- [ ] CI falla si hay regresion > 10% en benchmarks criticos
- [ ] Resultados documentados como baseline
- [ ] Benchmarks reproducibles en diferentes maquinas

---

## Diseno Propuesto

### Estructura de Archivos

```
crates/vortex-server/
├── Cargo.toml
├── benches/
│   ├── cache_bench.rs        # Benchmarks de cache
│   ├── serialization_bench.rs # Benchmarks de serializacion
│   └── http_bench.rs         # Benchmarks de handlers
└── src/
    └── ...
```

### Configuracion en Cargo.toml

```toml
[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio", "html_reports"] }
tokio = { version = "1", features = ["rt-multi-thread", "macros"] }

[[bench]]
name = "cache_bench"
harness = false

[[bench]]
name = "serialization_bench"
harness = false

[[bench]]
name = "http_bench"
harness = false
```

---

## Pasos de Implementacion

### Paso 1: Configurar Criterion

```toml
# crates/vortex-server/Cargo.toml
[dev-dependencies]
criterion = { version = "0.5", features = ["async_tokio", "html_reports"] }

[[bench]]
name = "cache_bench"
harness = false

[[bench]]
name = "serialization_bench"
harness = false

[[bench]]
name = "http_bench"
harness = false
```

### Paso 2: Implementar Cache Benchmarks

```rust
// benches/cache_bench.rs
use criterion::{
    black_box, criterion_group, criterion_main,
    BenchmarkId, Criterion, Throughput,
};
use std::sync::Arc;
use std::time::Duration;
use tokio::runtime::Runtime;
use vortex_server::cache::{CacheConfig, CacheKey, ConfigCache};
use vortex_server::handlers::response::ConfigResponse;

/// Crea un ConfigResponse de prueba con N propiedades
fn create_test_response(num_properties: usize) -> ConfigResponse {
    let mut properties = std::collections::HashMap::new();
    for i in 0..num_properties {
        properties.insert(format!("key.{}", i), format!("value-{}", i));
    }

    ConfigResponse {
        name: "test-app".to_string(),
        profiles: vec!["default".to_string()],
        label: "main".to_string(),
        version: None,
        state: None,
        property_sources: vec![vortex_core::PropertySource {
            name: "test".to_string(),
            source: properties,
        }],
    }
}

/// Benchmark: Cache get (hit)
fn bench_cache_get_hit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();

    let cache = ConfigCache::new(CacheConfig::default());
    let key = CacheKey::new("myapp", "prod", "main");
    let response = create_test_response(100);

    // Pre-populate cache
    rt.block_on(async {
        cache.insert(key.clone(), response).await;
    });

    c.bench_function("cache_get_hit", |b| {
        b.to_async(&rt).iter(|| async {
            let result = cache.get(&key).await;
            black_box(result)
        });
    });
}

/// Benchmark: Cache get (miss)
fn bench_cache_get_miss(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache = ConfigCache::new(CacheConfig::default());

    c.bench_function("cache_get_miss", |b| {
        b.to_async(&rt).iter(|| async {
            let key = CacheKey::new("nonexistent", "app", "main");
            let result = cache.get(&key).await;
            black_box(result)
        });
    });
}

/// Benchmark: Cache insert
fn bench_cache_insert(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache = ConfigCache::new(CacheConfig::default());
    let response = create_test_response(100);

    let mut counter = 0u64;

    c.bench_function("cache_insert", |b| {
        b.to_async(&rt).iter(|| {
            counter += 1;
            let key = CacheKey::new(
                &format!("app-{}", counter),
                "prod",
                "main"
            );
            let response = response.clone();
            async move {
                cache.insert(key, response).await;
            }
        });
    });
}

/// Benchmark: Cache insert con diferentes tamanos de response
fn bench_cache_insert_varying_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("cache_insert_sizes");

    for size in [10, 100, 500, 1000].iter() {
        let cache = ConfigCache::new(CacheConfig::default());
        let response = create_test_response(*size);

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, _size| {
                let mut counter = 0u64;
                b.to_async(&rt).iter(|| {
                    counter += 1;
                    let key = CacheKey::new(
                        &format!("app-{}", counter),
                        "prod",
                        "main"
                    );
                    let response = response.clone();
                    async move {
                        cache.insert(key, response).await;
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Cache invalidate
fn bench_cache_invalidate(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache = ConfigCache::new(CacheConfig::default());
    let response = create_test_response(100);

    let mut counter = 0u64;

    c.bench_function("cache_invalidate", |b| {
        b.to_async(&rt).iter(|| {
            counter += 1;
            let key = CacheKey::new(
                &format!("app-{}", counter),
                "prod",
                "main"
            );
            let response = response.clone();
            async move {
                // Insert then invalidate
                cache.insert(key.clone(), response).await;
                cache.invalidate(&key);
            }
        });
    });
}

/// Benchmark: Concurrencia - multiples gets simultaneos
fn bench_cache_concurrent_gets(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache = Arc::new(ConfigCache::new(CacheConfig::default()));

    // Pre-populate con 1000 entries
    rt.block_on(async {
        for i in 0..1000 {
            let key = CacheKey::new(&format!("app-{}", i), "prod", "main");
            cache.insert(key, create_test_response(50)).await;
        }
    });

    c.bench_function("cache_concurrent_gets_100", |b| {
        b.to_async(&rt).iter(|| {
            let cache = Arc::clone(&cache);
            async move {
                let handles: Vec<_> = (0..100)
                    .map(|i| {
                        let cache = Arc::clone(&cache);
                        tokio::spawn(async move {
                            let key = CacheKey::new(
                                &format!("app-{}", i % 1000),
                                "prod",
                                "main"
                            );
                            cache.get(&key).await
                        })
                    })
                    .collect();

                for handle in handles {
                    let _ = handle.await;
                }
            }
        });
    });
}

criterion_group!(
    benches,
    bench_cache_get_hit,
    bench_cache_get_miss,
    bench_cache_insert,
    bench_cache_insert_varying_sizes,
    bench_cache_invalidate,
    bench_cache_concurrent_gets,
);

criterion_main!(benches);
```

### Paso 3: Implementar Serialization Benchmarks

```rust
// benches/serialization_bench.rs
use criterion::{
    black_box, criterion_group, criterion_main,
    BenchmarkId, Criterion, Throughput,
};
use vortex_core::{ConfigMap, ConfigValue, PropertySource};
use vortex_server::response::format::{JsonFormatter, YamlFormatter, PropertiesFormatter};

/// Crea un ConfigMap de prueba con N propiedades anidadas
fn create_test_config(depth: usize, breadth: usize) -> ConfigMap {
    fn create_nested(depth: usize, breadth: usize, prefix: &str) -> ConfigValue {
        if depth == 0 {
            ConfigValue::String(format!("value-{}", prefix))
        } else {
            let mut map = indexmap::IndexMap::new();
            for i in 0..breadth {
                let key = format!("key-{}", i);
                let nested_prefix = format!("{}-{}", prefix, i);
                map.insert(key, create_nested(depth - 1, breadth, &nested_prefix));
            }
            ConfigValue::Object(map)
        }
    }

    let mut config = ConfigMap::new();
    for i in 0..breadth {
        let key = format!("root-{}", i);
        config.insert(key, create_nested(depth, breadth, &format!("{}", i)));
    }
    config
}

/// Benchmark: JSON serialization
fn bench_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    for (depth, breadth) in [(2, 5), (3, 5), (2, 10), (3, 10)].iter() {
        let config = create_test_config(*depth, *breadth);
        let num_props = breadth.pow(*depth as u32 + 1);

        group.throughput(Throughput::Elements(num_props as u64));
        group.bench_with_input(
            BenchmarkId::new("depth_breadth", format!("{}x{}", depth, breadth)),
            &config,
            |b, config| {
                b.iter(|| {
                    let json = serde_json::to_string(config).unwrap();
                    black_box(json)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: YAML serialization
fn bench_yaml_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("yaml_serialization");

    for (depth, breadth) in [(2, 5), (3, 5), (2, 10)].iter() {
        let config = create_test_config(*depth, *breadth);

        group.bench_with_input(
            BenchmarkId::new("depth_breadth", format!("{}x{}", depth, breadth)),
            &config,
            |b, config| {
                b.iter(|| {
                    let yaml = serde_yaml::to_string(config).unwrap();
                    black_box(yaml)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Properties serialization (flat key=value)
fn bench_properties_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("properties_serialization");

    for num_props in [50, 100, 500, 1000].iter() {
        let config = create_flat_config(*num_props);

        group.throughput(Throughput::Elements(*num_props as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_props),
            &config,
            |b, config| {
                b.iter(|| {
                    let props = PropertiesFormatter::format(config);
                    black_box(props)
                });
            },
        );
    }

    group.finish();
}

fn create_flat_config(num_props: usize) -> ConfigMap {
    let mut config = ConfigMap::new();
    for i in 0..num_props {
        config.insert(
            format!("property.key.{}", i),
            ConfigValue::String(format!("value-{}", i)),
        );
    }
    config
}

/// Benchmark: JSON deserialization
fn bench_json_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_deserialization");

    for (depth, breadth) in [(2, 5), (3, 5), (2, 10)].iter() {
        let config = create_test_config(*depth, *breadth);
        let json = serde_json::to_string(&config).unwrap();

        group.throughput(Throughput::Bytes(json.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("depth_breadth", format!("{}x{}", depth, breadth)),
            &json,
            |b, json| {
                b.iter(|| {
                    let config: ConfigMap = serde_json::from_str(json).unwrap();
                    black_box(config)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: ConfigResponse completo (como lo ve el cliente)
fn bench_config_response_serialization(c: &mut Criterion) {
    use vortex_server::handlers::response::ConfigResponse;

    let mut group = c.benchmark_group("config_response");

    for num_sources in [1, 3, 5].iter() {
        let response = ConfigResponse {
            name: "my-application".to_string(),
            profiles: vec!["production".to_string(), "cloud".to_string()],
            label: "main".to_string(),
            version: Some("abc123".to_string()),
            state: None,
            property_sources: (0..*num_sources)
                .map(|i| PropertySource {
                    name: format!("source-{}", i),
                    source: (0..100)
                        .map(|j| (format!("key.{}.{}", i, j), format!("value-{}-{}", i, j)))
                        .collect(),
                })
                .collect(),
        };

        group.bench_with_input(
            BenchmarkId::new("sources", num_sources),
            &response,
            |b, response| {
                b.iter(|| {
                    let json = serde_json::to_string(response).unwrap();
                    black_box(json)
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_json_serialization,
    bench_yaml_serialization,
    bench_properties_serialization,
    bench_json_deserialization,
    bench_config_response_serialization,
);

criterion_main!(benches);
```

### Paso 4: Implementar HTTP Benchmarks

```rust
// benches/http_bench.rs
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use criterion::{black_box, criterion_group, criterion_main, Criterion};
use std::sync::Arc;
use tokio::runtime::Runtime;
use tower::ServiceExt;
use vortex_server::{create_router, AppState, cache::{ConfigCache, CacheConfig}};

/// Crea app de test con cache pre-poblado
async fn create_test_app() -> axum::Router {
    let cache = ConfigCache::new(CacheConfig::default());

    // Pre-popular cache
    for app in ["app1", "app2", "app3"] {
        for profile in ["dev", "prod"] {
            let key = vortex_server::cache::CacheKey::new(app, profile, "main");
            cache.insert(key, create_test_response()).await;
        }
    }

    let state = AppState::new_with_cache(cache);
    create_router(state)
}

fn create_test_response() -> vortex_server::handlers::response::ConfigResponse {
    vortex_server::handlers::response::ConfigResponse {
        name: "test-app".to_string(),
        profiles: vec!["default".to_string()],
        label: "main".to_string(),
        version: None,
        state: None,
        property_sources: vec![],
    }
}

/// Benchmark: GET /health
fn bench_health_endpoint(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let app = rt.block_on(create_test_app());

    c.bench_function("http_health", |b| {
        b.to_async(&rt).iter(|| {
            let app = app.clone();
            async move {
                let response = app
                    .oneshot(
                        Request::get("/health")
                            .body(Body::empty())
                            .unwrap()
                    )
                    .await
                    .unwrap();

                assert_eq!(response.status(), StatusCode::OK);
                black_box(response)
            }
        });
    });
}

/// Benchmark: GET /{app}/{profile} (cache hit)
fn bench_get_config_cache_hit(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let app = rt.block_on(create_test_app());

    c.bench_function("http_get_config_hit", |b| {
        b.to_async(&rt).iter(|| {
            let app = app.clone();
            async move {
                let response = app
                    .oneshot(
                        Request::get("/app1/prod")
                            .body(Body::empty())
                            .unwrap()
                    )
                    .await
                    .unwrap();

                assert_eq!(response.status(), StatusCode::OK);
                black_box(response)
            }
        });
    });
}

/// Benchmark: Content negotiation (diferentes formatos)
fn bench_content_negotiation(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let app = rt.block_on(create_test_app());

    let mut group = c.benchmark_group("http_content_negotiation");

    for accept in ["application/json", "application/x-yaml", "text/plain"] {
        group.bench_with_input(
            criterion::BenchmarkId::new("format", accept),
            &accept,
            |b, accept| {
                let app = app.clone();
                b.to_async(&rt).iter(|| {
                    let app = app.clone();
                    async move {
                        let response = app
                            .oneshot(
                                Request::get("/app1/prod")
                                    .header("Accept", *accept)
                                    .body(Body::empty())
                                    .unwrap()
                            )
                            .await
                            .unwrap();

                        black_box(response)
                    }
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Concurrencia - multiples requests simultaneos
fn bench_concurrent_requests(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let app = Arc::new(rt.block_on(create_test_app()));

    c.bench_function("http_concurrent_50", |b| {
        b.to_async(&rt).iter(|| {
            let app = Arc::clone(&app);
            async move {
                let handles: Vec<_> = (0..50)
                    .map(|i| {
                        let app = (*app).clone();
                        tokio::spawn(async move {
                            let app_name = format!("app{}", (i % 3) + 1);
                            let profile = if i % 2 == 0 { "prod" } else { "dev" };

                            app.oneshot(
                                Request::get(&format!("/{}/{}", app_name, profile))
                                    .body(Body::empty())
                                    .unwrap()
                            )
                            .await
                        })
                    })
                    .collect();

                for handle in handles {
                    let _ = handle.await;
                }
            }
        });
    });
}

criterion_group!(
    benches,
    bench_health_endpoint,
    bench_get_config_cache_hit,
    bench_content_negotiation,
    bench_concurrent_requests,
);

criterion_main!(benches);
```

### Paso 5: Configurar CI para Benchmarks

```yaml
# .github/workflows/bench.yml
name: Benchmarks

on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  benchmark:
    runs-on: ubuntu-latest

    steps:
      - uses: actions/checkout@v4

      - name: Install Rust
        uses: dtolnay/rust-action@stable

      - name: Cache cargo
        uses: actions/cache@v4
        with:
          path: |
            ~/.cargo/registry
            ~/.cargo/git
            target
          key: ${{ runner.os }}-cargo-bench-${{ hashFiles('**/Cargo.lock') }}

      - name: Run benchmarks
        run: cargo bench --package vortex-server -- --save-baseline pr

      - name: Compare with main (on PR)
        if: github.event_name == 'pull_request'
        run: |
          git fetch origin main
          git checkout origin/main -- target/criterion || true
          cargo bench --package vortex-server -- --baseline main --save-baseline pr

      - name: Upload benchmark results
        uses: actions/upload-artifact@v4
        with:
          name: benchmark-results
          path: target/criterion

      - name: Check for regressions
        run: |
          # Script para detectar regresiones > 10%
          python3 scripts/check_benchmarks.py
```

### Paso 6: Script de Deteccion de Regresiones

```python
#!/usr/bin/env python3
# scripts/check_benchmarks.py
"""
Analiza resultados de Criterion y falla si hay regresiones > threshold.
"""

import json
import sys
from pathlib import Path

THRESHOLD_PERCENT = 10  # Regresion permitida

def main():
    criterion_dir = Path("target/criterion")

    if not criterion_dir.exists():
        print("No benchmark results found")
        return 0

    regressions = []

    for bench_dir in criterion_dir.iterdir():
        if not bench_dir.is_dir():
            continue

        estimates_file = bench_dir / "pr" / "estimates.json"
        baseline_file = bench_dir / "main" / "estimates.json"

        if not estimates_file.exists() or not baseline_file.exists():
            continue

        with open(estimates_file) as f:
            current = json.load(f)

        with open(baseline_file) as f:
            baseline = json.load(f)

        current_mean = current["mean"]["point_estimate"]
        baseline_mean = baseline["mean"]["point_estimate"]

        change_percent = ((current_mean - baseline_mean) / baseline_mean) * 100

        if change_percent > THRESHOLD_PERCENT:
            regressions.append({
                "benchmark": bench_dir.name,
                "baseline_ns": baseline_mean,
                "current_ns": current_mean,
                "change_percent": change_percent,
            })

    if regressions:
        print("Performance regressions detected!")
        print("=" * 60)
        for reg in regressions:
            print(f"  {reg['benchmark']}")
            print(f"    Baseline: {reg['baseline_ns'] / 1e6:.3f} ms")
            print(f"    Current:  {reg['current_ns'] / 1e6:.3f} ms")
            print(f"    Change:   +{reg['change_percent']:.1f}%")
            print()
        return 1

    print("No significant regressions detected")
    return 0

if __name__ == "__main__":
    sys.exit(main())
```

---

## Conceptos de Rust Aprendidos

### 1. Criterion Framework

Criterion es el framework de benchmarking mas usado en Rust, similar a JMH en Java.

**Rust (Criterion):**
```rust
use criterion::{black_box, criterion_group, criterion_main, Criterion};

// Benchmark simple
fn bench_simple(c: &mut Criterion) {
    c.bench_function("my_function", |b| {
        b.iter(|| {
            // Codigo a benchmarkear
            let result = expensive_operation();
            // black_box evita que el compilador optimice el codigo
            black_box(result)
        });
    });
}

// Benchmark con parametros
fn bench_with_params(c: &mut Criterion) {
    let mut group = c.benchmark_group("my_group");

    for size in [10, 100, 1000].iter() {
        group.bench_with_input(
            BenchmarkId::from_parameter(size),
            size,
            |b, &size| {
                b.iter(|| {
                    process_data(size)
                });
            },
        );
    }

    group.finish();
}

// Benchmark async
fn bench_async(c: &mut Criterion) {
    let rt = tokio::runtime::Runtime::new().unwrap();

    c.bench_function("async_operation", |b| {
        b.to_async(&rt).iter(|| async {
            async_operation().await
        });
    });
}

criterion_group!(benches, bench_simple, bench_with_params, bench_async);
criterion_main!(benches);
```

**Comparacion con Java (JMH):**
```java
import org.openjdk.jmh.annotations.*;
import java.util.concurrent.TimeUnit;

@BenchmarkMode(Mode.AverageTime)
@OutputTimeUnit(TimeUnit.NANOSECONDS)
@State(Scope.Benchmark)
public class MyBenchmark {

    @Benchmark
    public void benchSimple(Blackhole bh) {
        Object result = expensiveOperation();
        bh.consume(result);  // Equivalente a black_box
    }

    @Benchmark
    @OperationsPerInvocation(1000)
    public void benchWithParams(Blackhole bh) {
        for (int i = 0; i < 1000; i++) {
            bh.consume(processData(i));
        }
    }
}
```

**Diferencias clave:**

| Aspecto | Criterion (Rust) | JMH (Java) |
|---------|------------------|------------|
| Configuracion | Cargo.toml | Maven plugin + anotaciones |
| Warmup | Automatico | @Warmup annotation |
| Estadisticas | Automaticas, con outlier detection | Configurables |
| Output | HTML + JSON + CLI | CLI + JSON |
| Async | Soporte nativo | Requiere custom harness |

### 2. black_box y Optimizaciones del Compilador

`black_box` previene que el compilador optimice codigo "muerto".

**Rust:**
```rust
use criterion::black_box;

fn benchmark() {
    // MAL: El compilador puede eliminar esto
    let _ = expensive_computation();

    // MAL: El resultado no se usa, puede ser eliminado
    expensive_computation();

    // BIEN: black_box fuerza al compilador a mantener el codigo
    black_box(expensive_computation());

    // BIEN para inputs tambien
    let result = process(black_box(input));
    black_box(result);
}
```

**Comparacion con Java:**
```java
// JMH usa Blackhole
@Benchmark
public void benchmark(Blackhole bh) {
    Object result = expensiveComputation();
    bh.consume(result);  // Evita DCE (dead code elimination)
}
```

### 3. Throughput Measurements

Criterion permite medir throughput ademas de latencia.

**Rust:**
```rust
use criterion::Throughput;

fn bench_with_throughput(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization");

    let data = create_large_data();
    let bytes = data.len() as u64;

    // Medir bytes por segundo
    group.throughput(Throughput::Bytes(bytes));
    group.bench_function("serialize", |b| {
        b.iter(|| serialize(&data))
    });

    // O elementos por segundo
    group.throughput(Throughput::Elements(1000));
    group.bench_function("process_batch", |b| {
        b.iter(|| process_batch(1000))
    });

    group.finish();
}
```

### 4. Async Benchmarking

Criterion soporta benchmarks async con Tokio.

**Rust:**
```rust
use criterion::{criterion_group, Criterion};
use tokio::runtime::Runtime;

fn bench_async_operations(c: &mut Criterion) {
    // Crear runtime una vez
    let rt = Runtime::new().unwrap();

    // Benchmark async
    c.bench_function("async_get", |b| {
        // to_async convierte el bencher para async
        b.to_async(&rt).iter(|| async {
            let result = async_operation().await;
            black_box(result)
        });
    });

    // Con setup async
    c.bench_function("async_with_setup", |b| {
        b.to_async(&rt).iter_custom(|iters| async move {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                // Setup
                let data = prepare_data().await;

                // Medir solo esta parte
                let start = std::time::Instant::now();
                let _ = process_data(data).await;
                total += start.elapsed();
            }

            total
        });
    });
}
```

---

## Riesgos y Errores Comunes

### 1. Benchmark demasiado corto

```rust
// MAL: Operacion muy rapida, ruido domina la medicion
c.bench_function("too_fast", |b| {
    b.iter(|| {
        1 + 1  // Nanosegundos, resultado no confiable
    });
});

// BIEN: Batch de operaciones
c.bench_function("batched", |b| {
    b.iter(|| {
        let mut sum = 0;
        for i in 0..1000 {
            sum += black_box(i);
        }
        sum
    });
});
```

### 2. Estado compartido entre iteraciones

```rust
// MAL: Cache se llena en primeras iteraciones
c.bench_function("cache_get", |b| {
    let cache = ConfigCache::new();  // Mismo cache para todas las iteraciones

    b.iter(|| {
        cache.get(&key)  // Primera vez es miss, resto son hits
    });
});

// BIEN: Usar iter_with_setup para estado fresco
c.bench_function("cache_get_fresh", |b| {
    b.iter_with_setup(
        || {
            // Setup: crear cache fresco
            let cache = ConfigCache::new();
            // Pre-populate si queremos hit
            cache.insert(key.clone(), value.clone());
            cache
        },
        |cache| {
            // Benchmark
            cache.get(&key)
        }
    );
});
```

### 3. No usar black_box

```rust
// MAL: Compilador puede optimizar todo
c.bench_function("optimized_away", |b| {
    b.iter(|| {
        let result = complex_computation();
        // result no se usa, puede ser eliminado
    });
});

// BIEN: black_box previene optimizaciones
c.bench_function("real_measurement", |b| {
    b.iter(|| {
        black_box(complex_computation())
    });
});
```

### 4. Benchmarks no reproducibles

```rust
// MAL: Depende de datos aleatorios
c.bench_function("random_data", |b| {
    b.iter(|| {
        let data = generate_random_data();  // Diferente cada vez!
        process(data)
    });
});

// BIEN: Datos deterministicos
c.bench_function("deterministic", |b| {
    let data = generate_test_data(42);  // Seed fijo

    b.iter(|| {
        process(black_box(&data))
    });
});
```

---

## Pruebas

### Verificar que Benchmarks Compilan

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_test_response() {
        let response = create_test_response(100);
        assert_eq!(response.property_sources.len(), 1);
        assert_eq!(response.property_sources[0].source.len(), 100);
    }

    #[test]
    fn test_create_test_config() {
        let config = create_test_config(2, 3);
        // depth=2, breadth=3 should create 3^3 = 27 leaf values
        assert!(!config.is_empty());
    }
}
```

### Test de Regresion Manual

```bash
# Establecer baseline
cargo bench --package vortex-server -- --save-baseline main

# Hacer cambios...

# Comparar con baseline
cargo bench --package vortex-server -- --baseline main

# Ver reporte HTML
open target/criterion/report/index.html
```

---

## Observabilidad

### Ejemplo de Output de Criterion

```
cache_get_hit           time:   [245.12 ns 246.34 ns 247.89 ns]
                        thrpt:  [4.0340 Melem/s 4.0594 Melem/s 4.0795 Melem/s]

cache_get_miss          time:   [89.234 ns 90.123 ns 91.456 ns]
                        thrpt:  [10.934 Melem/s 11.096 Melem/s 11.206 Melem/s]

cache_insert/10         time:   [1.2345 us 1.2456 us 1.2567 us]
cache_insert/100        time:   [4.5678 us 4.6789 us 4.7890 us]
cache_insert/1000       time:   [45.678 us 46.789 us 47.890 us]

Benchmarking cache_concurrent_gets_100: Warming up for 3.0000 s
cache_concurrent_gets_100
                        time:   [1.2345 ms 1.2456 ms 1.2567 ms]
```

### Reportes HTML

Criterion genera reportes HTML interactivos en `target/criterion/report/index.html`:
- Graficos de distribucion
- Comparacion con baselines
- Deteccion de outliers
- Regresiones/mejoras

---

## Entregable Final

### Archivos Creados

1. `crates/vortex-server/benches/cache_bench.rs`
2. `crates/vortex-server/benches/serialization_bench.rs`
3. `crates/vortex-server/benches/http_bench.rs`
4. `.github/workflows/bench.yml`
5. `scripts/check_benchmarks.py`

### Verificacion

```bash
# Ejecutar todos los benchmarks
cargo bench --package vortex-server

# Solo cache benchmarks
cargo bench --package vortex-server -- cache

# Con output minimo
cargo bench --package vortex-server -- --quiet

# Guardar baseline
cargo bench --package vortex-server -- --save-baseline v1.0

# Comparar con baseline
cargo bench --package vortex-server -- --baseline v1.0
```

### Resultados Baseline Esperados

| Benchmark | Target | Metrica |
|-----------|--------|---------|
| cache_get_hit | < 500 ns | Latencia |
| cache_get_miss | < 200 ns | Latencia |
| cache_insert | < 2 us | Latencia |
| json_serialization (100 props) | < 50 us | Latencia |
| http_get_config_hit | < 100 us | Latencia |
| http_concurrent_50 | < 5 ms | Latencia total |

---

**Anterior**: [Historia 004 - Metricas de Cache](./story-004-cache-metrics.md)
**Siguiente**: [Indice de Epica 06 - Multi-Backend](../06-multi-backend/index.md)
