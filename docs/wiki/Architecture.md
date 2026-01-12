# Architecture

Arquitectura interna de Vortex Config.

## Overview

Vortex Config sigue una arquitectura de capas (layered architecture) con separación clara de responsabilidades:

```
┌─────────────────────────────────────────┐
│       HTTP Layer (Axum)                 │
│  - Request handling                     │
│  - Content negotiation                  │
│  - Middleware (logging, metrics, etc.)  │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│         Cache Layer (Moka)              │
│  - In-memory cache                      │
│  - TTL/TTI policies                     │
│  - Invalidation strategies              │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│     ConfigSource Trait (Abstraction)    │
│  - Backend abstraction                  │
│  - Fetch, health check                  │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│        Git Backend (gix)                │
│  - Clone/pull repositories              │
│  - File resolution                      │
│  - Refresh scheduler                    │
└──────────────┬──────────────────────────┘
               │
┌──────────────▼──────────────────────────┐
│      File System / Git Repository       │
└─────────────────────────────────────────┘
```

---

## Crates Structure

### vortex-core

**Responsabilidad:** Tipos de dominio y lógica de negocio

**Módulos:**
- `config/` - ConfigMap, ConfigValue, PropertySource
- `format/` - Serializers (JSON, YAML, Properties)
- `merge/` - Deep merge strategies
- `error.rs` - Error types

**Dependencias:** serde, serde_json, serde_yaml, java-properties

### vortex-git

**Responsabilidad:** Backend Git

**Módulos:**
- `source/` - ConfigSource trait
- `repository/` - Git operations (clone, fetch, checkout)
- `reader/` - File reading and parsing
- `sync/` - Refresh scheduler

**Dependencias:** gix, tokio, vortex-core

### vortex-server

**Responsabilidad:** Servidor HTTP

**Módulos:**
- `handlers/` - Request handlers
- `extractors/` - Axum extractors
- `middleware/` - RequestId, Logging, Metrics
- `cache/` - Moka cache integration
- `config/` - Server configuration

**Dependencias:** axum, tokio, moka, vortex-core, vortex-git

### vortex-sources (Future)

**Responsabilidad:** Registry de backends

**Backends Planned:**
- S3
- PostgreSQL
- MySQL
- SQLite

---

## Request Flow

### GET /{app}/{profile}/{label}

1. **Request arrives** → Axum router
2. **Middleware chain:**
   - RequestIdLayer → Generate/propagate UUID
   - LoggingLayer → Log request
   - MetricsLayer → Record metrics
3. **Path extraction** → Extract app, profile, label
4. **Accept header** → Determine response format
5. **Cache lookup** → Check Moka cache
   - **Cache HIT** → Return cached response (skip to step 10)
   - **Cache MISS** → Continue
6. **Backend fetch** → Call GitBackend.fetch()
7. **File resolution** → Find YAML/JSON/Properties files
8. **Parse & merge** → Deep merge configurations
9. **Populate cache** → Store in Moka with TTL
10. **Content negotiation** → Serialize to JSON/YAML/Properties
11. **Response** → Return to client

**Latency Breakdown:**
- Cache hit: ~0.5ms
- Cache miss + Git fetch: ~30ms
- Cold start (first request): ~300ms

---

## Cache System

### Moka Cache

**Características:**
- **Async-native:** Tokio-friendly
- **TTL:** Time-to-live configurable
- **TTI:** Time-to-idle opcional
- **Eviction:** TinyLFU (mejor que LRU)
- **Thread-safe:** Lock-free internally

### Cache Keys

```rust
"{app}:{profile}:{label}:{format}"

// Ejemplos:
"myapp:dev:main:json"
"payment:prod:v1.0.0:yaml"
"order:staging:feature/new:properties"
```

### Invalidation

**TTL-based:**
- Expiración automática después de `VORTEX_CACHE_TTL_SECONDS`

**On-demand:**
- `DELETE /cache` → Limpiar todo
- `DELETE /cache/{app}` → Limpiar por app
- `DELETE /cache/{app}/{profile}` → Limpiar por app+profile
- `DELETE /cache/{app}/{profile}/{label}` → Limpiar específico

**Pattern-based (Future):**
- `DELETE /cache?pattern=myapp:*`

---

## Git Backend

### Components

**GitBackend:**
- Implementa trait `ConfigSource`
- Wraps `GitRepository` y `ConfigResolver`
- Maneja refresh automático

**GitRepository:**
- Clone, fetch, checkout
- Lock para thread-safety
- Timeout handling

**ConfigResolver:**
- Encuentra archivos de configuración
- Parsea YAML/JSON/Properties
- Aplica convenciones Spring

**RefreshScheduler:**
- Tokio spawn de background task
- Exponential backoff en fallos
- Notifica cambios

### File Resolution

**Orden de búsqueda:**

```
Para: GET /myapp/prod/main

Archivos buscados (menor a mayor prioridad):
1. application.yml
2. application-prod.yml
3. myapp.yml
4. myapp-prod.yml
```

**Merge:** Los archivos más específicos sobrescriben los generales.

### Refresh Cycle

```
1. Wait interval (default: 30s)
2. Git fetch origin
3. Compare local vs remote commit
4. If different:
   - Pull changes
   - Invalidate affected cache entries
   - Notify subscribers (future)
5. On failure:
   - Increment failure count
   - Apply exponential backoff
   - Retry after backoff period
6. Repeat
```

---

## Concurrency & Thread Safety

### Shared State

**Arc<T>:** Shared ownership
- `Arc<GitBackend>`
- `Arc<ConfigCache>`
- `Arc<AppState>`

**RwLock<T>:** Read-write lock
- `RwLock<GitRepository>` (in GitBackend)
- Multiple readers, single writer

**Atomic types:**
- Cache metrics counters
- Request counters

### Async Runtime

**Tokio:**
- Multi-threaded work-stealing scheduler
- Default: # cores threads
- All I/O operations are async

**Blocking operations:**
- Git operations (gix) → `spawn_blocking`
- File I/O → async via tokio

---

## Error Handling

### Error Types

**VortexError (vortex-core):**
```rust
pub enum VortexError {
    InvalidApplication(String),
    ConfigNotFound { app, profile },
    ParseError(SourceError),
    SourceError(String),
    IoError(io::Error),
}
```

**HTTP Error Mapping:**
- `ConfigNotFound` → 404 Not Found
- `ParseError` → 500 Internal Server Error
- `IoError` → 500 Internal Server Error

### Error Context

Uso de `anyhow` para agregar contexto:

```rust
.context("Failed to fetch configuration")?
```

---

## Observability

### Logging

**Structured logging con tracing:**

```rust
#[instrument(skip(self), fields(app = %app, profile = %profile))]
async fn fetch_config(&self, app: &str, profile: &str) {
    info!("fetching configuration");
    // ...
}
```

**Log levels:**
- `ERROR`: Errores críticos
- `WARN`: Warnings (ej: cache miss frecuente)
- `INFO`: Operaciones importantes
- `DEBUG`: Debugging detallado
- `TRACE`: Todo

### Metrics

**Prometheus format:**

```
vortex_cache_hits_total
vortex_cache_misses_total
vortex_cache_evictions_total
vortex_cache_size
vortex_http_requests_total{method,status}
```

### Tracing (Future)

Integración con distributed tracing:
- OpenTelemetry
- Jaeger
- Zipkin

---

## Performance Characteristics

### Cold Start

**< 500ms:**
1. Parse configuration (< 50ms)
2. Setup Tokio runtime (< 100ms)
3. Initialize Axum server (< 200ms)
4. Git clone (paralelo) (< 30s)

### Request Latency

**Cache hit: < 1ms**
- Cache lookup: ~0.3ms
- Serialization: ~0.2ms

**Cache miss: < 50ms**
- Git file read: ~10ms
- Parse YAML: ~5ms
- Deep merge: ~2ms
- Serialization: ~3ms
- Cache populate: ~1ms

### Memory Footprint

**Idle: ~20MB**
- Binary: ~10MB
- Runtime: ~5MB
- Cache (empty): ~2MB
- Overhead: ~3MB

**With 10k cached configs: ~150MB**

---

## Design Patterns

### Trait-based Abstraction

```rust
#[async_trait]
pub trait ConfigSource: Send + Sync {
    async fn fetch(&self, query: &ConfigQuery) -> Result<ConfigResult>;
    fn default_label(&self) -> &str;
}
```

**Benefits:**
- Testability (mocks)
- Extensibility (new backends)
- Composition

### Builder Pattern

```rust
let config = GitBackendConfig::builder()
    .uri("https://...")
    .default_label("main")
    .build()?;
```

### Repository Pattern

GitBackend encapsula toda la lógica Git, exponiendo interface simple.

### Middleware/Filter Pattern

Axum Tower layers para cross-cutting concerns.

---

## Security Considerations

### Git Credentials

**Actualmente soportado:**
- HTTPS con usuario/password
- Personal Access Tokens

**Future:**
- SSH keys
- OAuth tokens
- Credential rotation

### API Security

**Recomendaciones:**
- Kubernetes Network Policies
- Service Mesh (mTLS)
- API Gateway (rate limiting, auth)

### Secrets

**NO almacenar en configuración:**
- Database passwords
- API keys
- Certificates

**Usar:**
- HashiCorp Vault (future integration)
- Kubernetes Secrets
- AWS Secrets Manager (future)

---

## Future Architecture

### Epic 6: Multi-Backend

```
┌──────────────────────────────┐
│     Backend Compositor       │
├──────────────────────────────┤
│  - Priority-based selection  │
│  - Fallback strategies       │
│  - Health checks             │
└──┬─────────┬─────────┬────────┘
   │         │         │
┌──▼───┐ ┌──▼───┐ ┌──▼───┐
│ Git  │ │  S3  │ │ SQL  │
└──────┘ └──────┘ └──────┘
```

### Epic 7: Governance

```
┌──────────────────────────────┐
│      PLAC Middleware         │
├──────────────────────────────┤
│  - Property-level ACL        │
│  - Actions: deny/mask/redact │
│  - Policy engine             │
└──────────────────────────────┘
```

### Epic 8: Real-time

```
┌──────────────────────────────┐
│     WebSocket Handler        │
├──────────────────────────────┤
│  - Subscribe to changes      │
│  - Push updates              │
│  - Semantic diff             │
└──────────────────────────────┘
```

---

## Próximos Pasos

- **[Development Guide](Development.md)** - Contribuir al proyecto
- **[Testing Strategy](Testing-Strategy.md)** - Estrategia de testing
- **[Rust Concepts](Rust-Concepts.md)** - Conceptos de Rust aplicados
