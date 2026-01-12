use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::sync::Arc;
use tokio::runtime::Runtime;
use vortex_server::cache::{CacheConfig, CacheKey, ConfigCache};
use vortex_server::handlers::response::{ConfigResponse, PropertySourceResponse};

/// Crea un ConfigResponse de prueba con N propiedades
fn create_test_response(num_properties: usize) -> ConfigResponse {
    let mut source = std::collections::HashMap::new();
    for i in 0..num_properties {
        source.insert(
            format!("property.key.{}", i),
            serde_json::json!(format!("value-{}", i)),
        );
    }

    ConfigResponse {
        name: "test-app".to_string(),
        profiles: vec!["default".to_string()],
        label: Some("main".to_string()),
        version: Some("abc123".to_string()),
        state: None,
        property_sources: vec![PropertySourceResponse {
            name: "test-source".to_string(),
            source,
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
            std::hint::black_box(result)
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
            std::hint::black_box(result)
        });
    });
}

/// Benchmark: Cache insert
fn bench_cache_insert(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache = Arc::new(ConfigCache::new(CacheConfig::default()));
    let response = Arc::new(create_test_response(100));

    let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));

    c.bench_function("cache_insert", |b| {
        b.to_async(&rt).iter(|| {
            let cache = Arc::clone(&cache);
            let response = Arc::clone(&response);
            let counter = Arc::clone(&counter);
            async move {
                let count = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let key = CacheKey::new(format!("app-{}", count), "prod", "main");
                cache.insert(key, (*response).clone()).await;
            }
        });
    });
}

/// Benchmark: Cache insert con diferentes tamanos de response
fn bench_cache_insert_varying_sizes(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let mut group = c.benchmark_group("cache_insert_sizes");

    for size in [10, 100, 500, 1000].iter() {
        let cache = Arc::new(ConfigCache::new(CacheConfig::default()));
        let response = Arc::new(create_test_response(*size));

        group.throughput(Throughput::Elements(1));
        group.bench_with_input(BenchmarkId::from_parameter(size), size, |b, _size| {
            let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));
            b.to_async(&rt).iter(|| {
                let cache = Arc::clone(&cache);
                let response = Arc::clone(&response);
                let counter = Arc::clone(&counter);
                async move {
                    let count = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    let key = CacheKey::new(format!("app-{}", count), "prod", "main");
                    cache.insert(key, (*response).clone()).await;
                }
            });
        });
    }

    group.finish();
}

/// Benchmark: Cache invalidate
fn bench_cache_invalidate(c: &mut Criterion) {
    let rt = Runtime::new().unwrap();
    let cache = Arc::new(ConfigCache::new(CacheConfig::default()));
    let response = Arc::new(create_test_response(100));

    let counter = Arc::new(std::sync::atomic::AtomicU64::new(0));

    c.bench_function("cache_invalidate", |b| {
        b.to_async(&rt).iter(|| {
            let cache = Arc::clone(&cache);
            let response = Arc::clone(&response);
            let counter = Arc::clone(&counter);
            async move {
                let count = counter.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let key = CacheKey::new(format!("app-{}", count), "prod", "main");
                // Insert then invalidate
                cache.insert(key.clone(), (*response).clone()).await;
                cache.invalidate(&key).await;
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
            let key = CacheKey::new(format!("app-{}", i), "prod", "main");
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
                            let key = CacheKey::new(format!("app-{}", i % 1000), "prod", "main");
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
