# Epica 05: Cache con Moka y Configuracion del Servidor

## Objetivo

Implementar una capa de cache de alto rendimiento utilizando Moka y establecer el sistema de configuracion del servidor Vortex Config. Esta epica agrega:

1. **Cache Async**: Cache en memoria con TTL configurable usando Moka
2. **Invalidacion Inteligente**: Estrategias de invalidacion TTL, on-demand y pattern-based
3. **Configuracion del Servidor**: Sistema robusto para parsear configuracion YAML y variables de entorno
4. **Metricas de Cache**: Exposicion de hit/miss ratios y latencias para observabilidad
5. **Benchmarks**: Suite de benchmarks con Criterion para medir performance

El cache es fundamental para cumplir el KPI de latencia p99 < 10ms en configuraciones frecuentemente accedidas.

---

## Conceptos de Rust Cubiertos (Nivel Avanzado)

| Concepto | Historia | Comparacion con Java |
|----------|----------|---------------------|
| Arc (Atomic Reference Counting) | 001, 002, 004 | AtomicReference + shared ownership |
| Atomic* types (AtomicU64, etc.) | 001, 004 | AtomicLong, AtomicInteger |
| Tokio runtime configuration | 003, 005 | ExecutorService configuration |
| Channels (mpsc, oneshot, broadcast) | 002 | BlockingQueue, CompletableFuture |
| tokio::sync::Notify | 002 | CountDownLatch / Condition |
| Feature flags en Cargo | 003 | Maven profiles |
| Config crate | 003 | Spring @ConfigurationProperties |
| Metrics y Prometheus | 004 | Micrometer |
| Criterion benchmarks | 005 | JMH (Java Microbenchmark Harness) |
| Interior mutability (Mutex en async) | 001, 002 | synchronized blocks |

---

## Historias de Usuario

| # | Titulo | Descripcion | Puntos |
|---|--------|-------------|--------|
| 001 | [Integracion de Moka Cache](./story-001-moka-integration.md) | Cache async con TTL configurable | 5 |
| 002 | [Invalidacion de Cache](./story-002-invalidation.md) | Estrategias TTL, on-demand, pattern-based | 5 |
| 003 | [Configuracion del Servidor](./story-003-server-config.md) | Parsear YAML/env vars para vortex-server | 5 |
| 004 | [Metricas de Cache](./story-004-cache-metrics.md) | Hit/miss ratios y latencias con Prometheus | 3 |
| 005 | [Benchmarks de Performance](./story-005-benchmarks.md) | Criterion benchmarks para cache y serialization | 3 |

**Total**: 21 puntos de historia

---

## Dependencias

### Epicas Prerequisito

| Epica | Razon |
|-------|-------|
| 03 - HTTP Server | Servidor Axum funcionando con endpoints basicos |
| 04 - Git Backend | Backend de configuracion implementado para testear cache |

### Dependencias de Crates

```toml
[dependencies]
# Cache
moka = { version = "0.12", features = ["future"] }

# Configuracion
config = "0.14"

# Async runtime
tokio = { version = "1", features = ["full", "sync"] }

# Metricas
metrics = "0.22"
metrics-exporter-prometheus = "0.13"

# Serializacion
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# Observabilidad
tracing = "0.1"

# Errores
thiserror = "1"

[dev-dependencies]
# Benchmarking
criterion = { version = "0.5", features = ["async_tokio"] }
tokio-test = "0.4"

[[bench]]
name = "cache_benchmarks"
harness = false
```

---

## Criterios de Aceptacion

### Funcionales

- [ ] Cache Moka integrado con TTL configurable (default 5 minutos)
- [ ] Invalidacion on-demand por key individual
- [ ] Invalidacion por patron (glob patterns)
- [ ] Configuracion del servidor desde YAML y environment variables
- [ ] Variables de entorno sobreescriben valores YAML
- [ ] Metricas de cache expuestas en formato Prometheus
- [ ] Hit rate, miss rate, eviction count disponibles

### No Funcionales

- [ ] Cache hit latency p99 < 1ms
- [ ] Cache miss + backend fetch p99 < 50ms (dependiendo del backend)
- [ ] Memory footprint del cache configurable (max entries o max size)
- [ ] Invalidacion de 1000 entries < 100ms

### Compatibilidad

- [ ] API de cache compatible con trait `ConfigSource`
- [ ] Configuracion compatible con 12-factor apps
- [ ] Metricas compatibles con Prometheus scraping

---

## Definition of Done

- [ ] Codigo compila sin warnings (`cargo build --all-features`)
- [ ] Formateado con `cargo fmt`
- [ ] Sin errores de clippy (`cargo clippy -- -D warnings`)
- [ ] Tests unitarios pasan con cobertura > 80%
- [ ] Tests de integracion con cache pasan
- [ ] Benchmarks documentados y reproducibles
- [ ] Rustdoc para todas las APIs publicas
- [ ] Changelog actualizado
- [ ] Sin `unwrap()` en codigo de produccion
- [ ] Metricas expuestas correctamente
- [ ] CI pipeline verde

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Memory leaks en cache de larga duracion | Baja | Alto | Configurar max_capacity y TTL, monitorear metricas |
| Race conditions en invalidacion | Media | Alto | Usar primitivas atomicas de Moka, tests de concurrencia |
| Configuracion incorrecta en produccion | Media | Alto | Validacion al startup, fail-fast con mensajes claros |
| Overhead de metricas en hot path | Baja | Medio | Benchmarks, metricas atomicas sin locks |
| Incompatibilidad de feature flags | Baja | Medio | Tests con diferentes combinaciones de features |

---

## Decisiones Arquitectonicas (ADRs)

### ADR-001: Moka como Cache Library

**Estado**: Aceptado

**Contexto**: Necesitamos un cache en memoria async-friendly con soporte para TTL y eviction policies.

**Decision**: Usar Moka 0.12+ como libreria de cache.

**Razones**:
- Cache async-native con soporte para Tokio
- TTL per-entry y time-to-idle configurables
- Size-based eviction con TinyLFU (mejor que LRU)
- Thread-safe sin locks explicitos
- Metricas built-in opcionales
- Mantenida activamente por el equipo de Rust

**Alternativas consideradas**:
- `cached`: Mas simple pero sin async nativo
- `lru`: Solo LRU, sin TTL built-in
- `dashmap` + manual TTL: Mas trabajo, propenso a errores

### ADR-002: Config Crate para Configuracion

**Estado**: Aceptado

**Contexto**: Necesitamos un sistema flexible para cargar configuracion de multiples fuentes.

**Decision**: Usar el crate `config` para gestion de configuracion.

**Razones**:
- Soporte para multiples formatos (YAML, TOML, JSON)
- Merge de multiples fuentes con prioridades
- Environment variables con prefijos configurables
- Deserializacion directa a structs con serde
- Ampliamente usado en el ecosistema Rust

**Estructura de configuracion**:

```yaml
# config/default.yaml
server:
  host: "0.0.0.0"
  port: 8080

cache:
  enabled: true
  ttl_seconds: 300
  max_capacity: 10000

backends:
  git:
    enabled: true
    uri: "file:///config-repo"
    default_label: "main"

logging:
  level: "info"
  format: "json"
```

### ADR-003: Prometheus para Metricas

**Estado**: Aceptado

**Contexto**: Necesitamos exponer metricas de cache para observabilidad.

**Decision**: Usar `metrics` crate con `metrics-exporter-prometheus`.

**Razones**:
- API ergonomica y performante
- Exporter Prometheus listo para produccion
- Compatible con ecosystem de observabilidad cloud-native
- Macros para registrar metricas sin boilerplate
- Bajo overhead en hot paths

---

## Reglas Estrictas

1. **Arc para estado compartido**: Todo estado compartido entre requests debe usar `Arc<T>`
2. **No bloquear el runtime**: Nunca usar `std::sync::Mutex` en codigo async, usar `tokio::sync::Mutex`
3. **Cache key consistency**: Keys de cache deben ser normalizadas (lowercase, sorted params)
4. **TTL explicito**: Siempre configurar TTL, nunca cache infinito por defecto
5. **Metricas no bloquean**: Metricas deben ser atomicas, nunca bloquear el hot path
6. **Fail-fast en config**: Errores de configuracion deben fallar al startup, no en runtime
7. **Benchmarks reproducibles**: Todos los benchmarks deben correr en CI

---

## Estructura del Crate

```
crates/vortex-server/
├── Cargo.toml
├── src/
│   ├── lib.rs
│   ├── config/
│   │   ├── mod.rs           # Re-exports
│   │   ├── settings.rs      # ServerSettings struct
│   │   ├── loader.rs        # Config loading logic
│   │   └── validation.rs    # Config validation
│   ├── cache/
│   │   ├── mod.rs           # Re-exports
│   │   ├── config_cache.rs  # Moka cache wrapper
│   │   ├── invalidation.rs  # Invalidation strategies
│   │   ├── keys.rs          # Cache key generation
│   │   └── metrics.rs       # Cache metrics
│   ├── handlers/
│   │   └── ...
│   └── middleware/
│       └── ...
├── benches/
│   ├── cache_benchmarks.rs
│   └── serialization_benchmarks.rs
└── tests/
    ├── cache_integration.rs
    └── config_tests.rs
```

---

## Diagrama de Cache Layer

```
                    ┌─────────────────────────────────────┐
                    │           HTTP Request              │
                    └─────────────────┬───────────────────┘
                                      │
                    ┌─────────────────▼───────────────────┐
                    │         Cache Key Builder           │
                    │   (app + profile + label + format)  │
                    └─────────────────┬───────────────────┘
                                      │
                    ┌─────────────────▼───────────────────┐
                    │          Moka Cache                 │
                    │  ┌─────────────────────────────┐   │
                    │  │  Key: "myapp:prod:main:json" │   │
                    │  │  Value: Arc<ConfigResponse>  │   │
                    │  │  TTL: 300s                   │   │
                    │  └─────────────────────────────┘   │
                    └─────────────────┬───────────────────┘
                                      │
                         ┌────────────┴────────────┐
                         │                         │
                    ┌────▼────┐              ┌────▼────┐
                    │  HIT    │              │  MISS   │
                    └────┬────┘              └────┬────┘
                         │                         │
                         │                  ┌──────▼──────┐
                         │                  │   Backend   │
                         │                  │  (Git/S3)   │
                         │                  └──────┬──────┘
                         │                         │
                         │                  ┌──────▼──────┐
                         │                  │  Populate   │
                         │                  │   Cache     │
                         │                  └──────┬──────┘
                         │                         │
                    ┌────▼─────────────────────────▼────┐
                    │        Metrics Recording          │
                    │  (hit/miss counter, latency)      │
                    └─────────────────┬─────────────────┘
                                      │
                    ┌─────────────────▼───────────────────┐
                    │          HTTP Response              │
                    └─────────────────────────────────────┘
```

---

## Diagrama de Invalidacion

```
┌──────────────────────────────────────────────────────────────┐
│                   Invalidation Strategies                     │
├──────────────────────────────────────────────────────────────┤
│                                                               │
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
│  │  TTL-based  │  │  On-demand  │  │   Pattern-based     │  │
│  │             │  │             │  │                     │  │
│  │  Automatic  │  │  DELETE     │  │  DELETE             │  │
│  │  expiration │  │  /cache/key │  │  /cache?pattern=*   │  │
│  │  after TTL  │  │             │  │                     │  │
│  └──────┬──────┘  └──────┬──────┘  └──────────┬──────────┘  │
│         │                │                     │              │
│         └────────────────┼─────────────────────┘              │
│                          │                                    │
│                  ┌───────▼───────┐                           │
│                  │  Invalidator  │                           │
│                  │   Service     │                           │
│                  └───────┬───────┘                           │
│                          │                                    │
│                  ┌───────▼───────┐                           │
│                  │  Moka Cache   │                           │
│                  │  .invalidate()│                           │
│                  └───────┬───────┘                           │
│                          │                                    │
│                  ┌───────▼───────┐                           │
│                  │   Metrics     │                           │
│                  │  (evictions)  │                           │
│                  └───────────────┘                           │
│                                                               │
└──────────────────────────────────────────────────────────────┘
```

---

## Changelog

| Version | Fecha | Cambios |
|---------|-------|---------|
| 0.1.0 | 2025-01-XX | Creacion inicial de la epica |

---

## Referencias

- [Moka Documentation](https://docs.rs/moka)
- [Config Crate](https://docs.rs/config)
- [Metrics Crate](https://docs.rs/metrics)
- [Criterion Benchmarking](https://docs.rs/criterion)
- [Tokio Sync Primitives](https://docs.rs/tokio/latest/tokio/sync/index.html)
- [12-Factor App Config](https://12factor.net/config)
