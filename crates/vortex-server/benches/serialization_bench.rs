use criterion::{BenchmarkId, Criterion, Throughput, criterion_group, criterion_main};
use std::collections::HashMap;
use vortex_server::handlers::response::{ConfigResponse, PropertySourceResponse};

/// Crea un ConfigResponse de prueba con estructura anidada
fn create_nested_response(depth: usize, breadth: usize) -> ConfigResponse {
    fn create_nested_value(depth: usize, breadth: usize, prefix: &str) -> serde_json::Value {
        if depth == 0 {
            serde_json::json!(format!("value-{}", prefix))
        } else {
            let mut map = serde_json::Map::new();
            for i in 0..breadth {
                let key = format!("key-{}", i);
                let nested_prefix = format!("{}-{}", prefix, i);
                map.insert(key, create_nested_value(depth - 1, breadth, &nested_prefix));
            }
            serde_json::Value::Object(map)
        }
    }

    let mut source = HashMap::new();
    for i in 0..breadth {
        let key = format!("root-{}", i);
        source.insert(key, create_nested_value(depth, breadth, &format!("{}", i)));
    }

    ConfigResponse {
        name: "test-application".to_string(),
        profiles: vec!["production".to_string(), "cloud".to_string()],
        label: Some("main".to_string()),
        version: Some("abc123".to_string()),
        state: None,
        property_sources: vec![PropertySourceResponse {
            name: "test-source".to_string(),
            source,
        }],
    }
}

/// Crea un ConfigResponse plano con N propiedades
fn create_flat_response(num_properties: usize) -> ConfigResponse {
    let mut source = HashMap::new();
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
            name: "application.properties".to_string(),
            source,
        }],
    }
}

/// Benchmark: JSON serialization
fn bench_json_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_serialization");

    for (depth, breadth) in [(2, 5), (3, 5), (2, 10)].iter() {
        let response = create_nested_response(*depth, *breadth);
        let num_props = breadth.pow(*depth as u32 + 1);

        group.throughput(Throughput::Elements(num_props as u64));
        group.bench_with_input(
            BenchmarkId::new("depth_breadth", format!("{}x{}", depth, breadth)),
            &response,
            |b, response| {
                b.iter(|| {
                    let json = serde_json::to_string(response).unwrap();
                    std::hint::black_box(json)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: JSON serialization con pretty print
fn bench_json_serialization_pretty(c: &mut Criterion) {
    let response = create_flat_response(100);

    c.bench_function("json_serialization_pretty", |b| {
        b.iter(|| {
            let json = serde_json::to_string_pretty(&response).unwrap();
            std::hint::black_box(json)
        });
    });
}

/// Benchmark: YAML serialization
fn bench_yaml_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("yaml_serialization");

    for (depth, breadth) in [(2, 5), (3, 5), (2, 10)].iter() {
        let response = create_nested_response(*depth, *breadth);

        group.bench_with_input(
            BenchmarkId::new("depth_breadth", format!("{}x{}", depth, breadth)),
            &response,
            |b, response| {
                b.iter(|| {
                    let yaml = serde_yaml::to_string(response).unwrap();
                    std::hint::black_box(yaml)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: JSON deserialization
fn bench_json_deserialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("json_deserialization");

    for (depth, breadth) in [(2, 5), (3, 5), (2, 10)].iter() {
        let response = create_nested_response(*depth, *breadth);
        let json = serde_json::to_string(&response).unwrap();
        let json_bytes = json.as_bytes().to_vec();

        group.throughput(Throughput::Bytes(json_bytes.len() as u64));
        group.bench_with_input(
            BenchmarkId::new("depth_breadth", format!("{}x{}", depth, breadth)),
            &json_bytes,
            |b, json_bytes| {
                b.iter(|| {
                    let json_str = std::str::from_utf8(json_bytes).unwrap();
                    let value: serde_json::Value = serde_json::from_str(json_str).unwrap();
                    std::hint::black_box(value)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: ConfigResponse completo (como lo ve el cliente)
fn bench_config_response_serialization(c: &mut Criterion) {
    let mut group = c.benchmark_group("config_response");

    for num_sources in [1, 3, 5].iter() {
        let mut property_sources = Vec::new();
        for i in 0..*num_sources {
            let mut source = HashMap::new();
            for j in 0..100 {
                source.insert(
                    format!("key.{}.{}", i, j),
                    serde_json::json!(format!("value-{}-{}", i, j)),
                );
            }
            property_sources.push(PropertySourceResponse {
                name: format!("source-{}", i),
                source,
            });
        }

        let response = ConfigResponse {
            name: "my-application".to_string(),
            profiles: vec!["production".to_string(), "cloud".to_string()],
            label: Some("main".to_string()),
            version: Some("abc123".to_string()),
            state: None,
            property_sources,
        };

        group.bench_with_input(
            BenchmarkId::new("sources", num_sources),
            &response,
            |b, response| {
                b.iter(|| {
                    let json = serde_json::to_string(response).unwrap();
                    std::hint::black_box(json)
                });
            },
        );
    }

    group.finish();
}

/// Benchmark: Tamaño de serialización
fn bench_serialization_size(c: &mut Criterion) {
    let mut group = c.benchmark_group("serialization_size");

    for num_props in [10, 100, 500, 1000].iter() {
        let response = create_flat_response(*num_props);

        group.throughput(Throughput::Elements(*num_props as u64));
        group.bench_with_input(
            BenchmarkId::from_parameter(num_props),
            &response,
            |b, response| {
                b.iter(|| {
                    let json = serde_json::to_string(response).unwrap();
                    std::hint::black_box(json.len())
                });
            },
        );
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_json_serialization,
    bench_json_serialization_pretty,
    bench_yaml_serialization,
    bench_json_deserialization,
    bench_config_response_serialization,
    bench_serialization_size,
);

criterion_main!(benches);
