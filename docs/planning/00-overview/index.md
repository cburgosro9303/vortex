# Vortex Config — Planificación del Proyecto

## Mapa del Producto

**Vortex Config** es un servidor de configuración cloud-native escrito en Rust, diseñado como alternativa de alto rendimiento a Spring Cloud Config. Sus pilares fundamentales son:

- **Seguridad**: Control de acceso a nivel de propiedad (PLAC), firma criptográfica, integración con Vault
- **Performance**: Cold start < 500ms, footprint < 30MB, latencia p99 < 10ms
- **Observabilidad**: Tracing distribuido, métricas Prometheus, logs estructurados
- **Mantenibilidad**: Arquitectura modular en crates, testing exhaustivo, CI/CD enterprise

### Arquitectura de Alto Nivel

```
┌─────────────────────────────────────────────────────────────────────┐
│                         VORTEX CONFIG SERVER                        │
├─────────────────────────────────────────────────────────────────────┤
│  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐ │
│  │   Axum API  │  │  WebSocket  │  │    gRPC     │  │   Metrics   │ │
│  │   (REST)    │  │   (Push)    │  │  (Sidecar)  │  │ (Prometheus)│ │
│  └──────┬──────┘  └──────┬──────┘  └──────┬──────┘  └─────────────┘ │
│         │                │                │                          │
│  ┌──────┴────────────────┴────────────────┴──────┐                  │
│  │              GOVERNANCE LAYER                  │                  │
│  │  ┌────────┐  ┌────────┐  ┌────────┐           │                  │
│  │  │  PLAC  │  │ Schema │  │Complian│           │                  │
│  │  │ Engine │  │ Valid. │  │  ce    │           │                  │
│  │  └────────┘  └────────┘  └────────┘           │                  │
│  └───────────────────────────────────────────────┘                  │
│         │                                                            │
│  ┌──────┴───────────────────────────────────────┐                   │
│  │              FEATURE LAYER                    │                   │
│  │  ┌────────┐  ┌────────┐  ┌────────┐          │                   │
│  │  │ Flags  │  │Templat.│  │Rollouts│          │                   │
│  │  │ Engine │  │ (Tera) │  │ Canary │          │                   │
│  │  └────────┘  └────────┘  └────────┘          │                   │
│  └──────────────────────────────────────────────┘                   │
│         │                                                            │
│  ┌──────┴───────────────────────────────────────┐                   │
│  │                  CACHE LAYER                  │                   │
│  │              (Moka - Async Cache)             │                   │
│  └──────────────────────────────────────────────┘                   │
│         │                                                            │
│  ┌──────┴───────────────────────────────────────┐                   │
│  │              SOURCE LAYER (Backends)          │                   │
│  │  ┌────────┐  ┌────────┐  ┌────────┐          │                   │
│  │  │  Git   │  │   S3   │  │  SQL   │          │                   │
│  │  │ (gix)  │  │(aws-sdk│  │(SQLx)  │          │                   │
│  │  └────────┘  └────────┘  └────────┘          │                   │
│  └──────────────────────────────────────────────┘                   │
└─────────────────────────────────────────────────────────────────────┘
```

### Estructura del Workspace

```
vortex-config/
├── Cargo.toml                 # Workspace root
├── crates/
│   ├── vortex-core/           # Tipos compartidos, traits, ConfigMap
│   ├── vortex-server/         # Axum API, routing, middleware
│   ├── vortex-sources/        # Implementaciones de ConfigSource
│   │   ├── git/
│   │   ├── s3/
│   │   └── sql/
│   ├── vortex-governance/     # PLAC, Schema, Compliance
│   ├── vortex-features/       # Feature flags engine
│   ├── vortex-rollout/        # Canary/progressive deployment
│   ├── vortex-audit/          # Event sourcing, analytics
│   ├── vortex-secrets/        # Integración Vault/AWS SM
│   ├── vortex-templating/     # Tera integration
│   ├── vortex-federation/     # Multi-cluster sync
│   └── vortex-client/         # SDK para aplicaciones Rust
├── proto/                     # gRPC definitions
├── charts/                    # Helm charts
├── docker/
│   ├── Dockerfile.minimal     # Git-only
│   ├── Dockerfile.s3          # Con Litestream
│   └── Dockerfile.full        # Enterprise
└── tests/
    ├── integration/
    ├── compatibility/         # Tests contra Spring Boot client
    └── load/
```

---

## Roadmap por Fases

### Fase 1 — MVP Drop-in (Épicas 1-4)

**Objetivo**: Servidor funcional compatible con Spring Cloud Config usando Git como backend.

| Épica | Nombre | Entregable Principal |
| ------- | -------- | --------------------- |
| 01 | Foundation | Workspace, toolchain, CI básico |
| 02 | Core Types | ConfigMap, PropertySource, serialización |
| 03 | HTTP Server | API REST compatible Spring Cloud Config |
| 04 | Git Backend | Backend Git con gix |

**Criterios de éxito Fase 1**:

- [ ] `GET /{app}/{profile}` retorna JSON compatible Spring
- [ ] `GET /{app}/{profile}/{label}` soporta branches/tags
- [ ] Formatos: JSON, YAML, .properties
- [ ] Cold start < 500ms
- [ ] Tests de compatibilidad con Spring Boot client

### Fase 2 — Persistencia & Cloud (Épicas 5-6)

**Objetivo**: Backends alternativos para deployments serverless y cloud-native.

| Épica | Nombre | Entregable Principal |
|-------|--------|---------------------|
| 05 | Cache & Config | Cache Moka, configuración del servidor |
| 06 | Multi-Backend | Backends S3 y SQL (PostgreSQL, MySQL, SQLite) |

**Criterios de éxito Fase 2**:

- [ ] Cache con TTL configurable y métricas
- [ ] Backend S3 funcional con versionado
- [ ] Backend SQL con migrations SQLx
- [ ] Compositor de backends con prioridades

### Fase 3 — Governance Core (Épicas 7-8)

**Objetivo**: Control de acceso granular y actualizaciones en tiempo real.

| Épica | Nombre | Entregable Principal |
|-------|--------|---------------------|
| 07 | Governance | PLAC engine, schema validation |
| 08 | Realtime | WebSockets, diff semántico, push de cambios |

**Criterios de éxito Fase 3**:

- [ ] Políticas PLAC aplicadas por request
- [ ] Acciones: deny, redact, mask, warn
- [ ] Validación de schemas JSON
- [ ] WebSocket push con diff semántico
- [ ] Reconexión automática de clientes

### Fase 4 — Advanced Features (Épica 9)

**Objetivo**: Capacidades diferenciadoras vs Spring Cloud Config.

| Épica | Nombre | Entregable Principal |
|-------|--------|---------------------|
| 09 | Advanced Features | Feature flags, templating Tera, compliance engine |

**Criterios de éxito Fase 4**:

- [ ] Feature flags con targeting y porcentajes
- [ ] Templating dinámico con funciones built-in
- [ ] Motor de compliance (PCI-DSS, SOC2)
- [ ] Reportes de violaciones

### Fase 5 — Enterprise & Operations (Épica 10)

**Objetivo**: Production-readiness para deployments enterprise.

| Épica | Nombre | Entregable Principal |
|-------|--------|---------------------|
| 10 | Enterprise | Canary rollouts, drift detection, federation |

**Criterios de éxito Fase 5**:

- [ ] Rollouts progresivos con métricas de éxito
- [ ] Detección de drift con remediación
- [ ] SDK de heartbeat para clientes
- [ ] Multi-cluster federation
- [ ] Helm charts production-ready

---

## Lista de Épicas en Orden Recomendado

| # | Slug | Nombre | Historias | Dependencias |
|---|------|--------|-----------|--------------|
| 01 | foundation | Foundation - Proyecto Base y Toolchain | 5 | Ninguna |
| 02 | core-types | Core Types y Serialización | 5 | 01 |
| 03 | http-server | HTTP Server con Axum | 6 | 01, 02 |
| 04 | git-backend | Git Backend con gix | 6 | 02, 03 |
| 05 | cache-config | Cache con Moka y Configuración | 5 | 03, 04 |
| 06 | multi-backend | Persistencia Multi-Backend (S3/SQL) | 7 | 04 |
| 07 | governance | Governance - PLAC y Schema Validation | 6 | 03 |
| 08 | realtime | Real-time y WebSockets | 5 | 03, 05 |
| 09 | advanced-features | Features Avanzadas | 7 | 07 |
| 10 | enterprise | Enterprise - Canary, Drift, Federation | 6 | 05, 07, 08 |

---

## Estrategia de Aprendizaje Rust (Currículo)

Este proyecto está diseñado para un desarrollador senior en Java/Spring que está aprendiendo Rust. El currículo está integrado en las épicas de forma progresiva.

### Nivel Básico (Épicas 1-2)

| Concepto | Épica | Historia | Comparación con Java |
|----------|-------|----------|---------------------|
| Toolchain (rustup, cargo) | 01 | 001, 002 | Similar a Maven/Gradle |
| Workspace multi-crate | 01 | 001 | Multi-module Maven project |
| Módulos y visibilidad | 01 | 001, 004 | Packages y access modifiers |
| Tipos primitivos | 01 | 004 | Tipos primitivos + wrappers |
| Structs | 01, 02 | 004, 001 | Classes sin herencia |
| Enums con datos | 01, 02 | 004, 001 | Sealed classes + records (Java 17+) |
| Pattern matching (match) | 01, 02 | 004, 001 | Switch expressions (Java 17+) |
| Result<T, E> | 01, 02 | 005, 001 | Checked exceptions / Optional |
| Option<T> | 02 | 001, 002 | Optional<T> |
| Ownership básico | 02 | 001, 002 | Sin equivalente (GC en Java) |
| Borrowing (&, &mut) | 02 | 001, 002 | Sin equivalente |
| Clone vs Copy | 02 | 001 | .clone() en Java |
| Derive macros | 02 | 001, 002 | Lombok @Data |
| Serde (Serialize/Deserialize) | 02 | 001-004 | Jackson annotations |

### Nivel Intermedio (Épicas 3-4)

| Concepto | Épica | Historia | Comparación con Java |
|----------|-------|----------|---------------------|
| Traits (definición) | 03, 04 | 001, 001 | Interfaces |
| Traits (implementación) | 03, 04 | Todas | implements Interface |
| Generics básicos | 03 | 001-003 | Generics <T> |
| impl blocks | 03, 04 | Todas | Methods en class |
| Closures | 03 | 002-004 | Lambdas |
| Fn, FnMut, FnOnce | 03 | 004 | Functional interfaces |
| async/await intro | 03 | 001-006 | CompletableFuture |
| Extractors (Axum) | 03 | 002-004 | @PathVariable, @RequestBody |
| Lifetimes básicos | 04 | 001, 003 | Sin equivalente |
| async_trait | 04 | 001 | Sin equivalente |
| Mutex/RwLock | 04 | 005 | synchronized / ReentrantLock |
| File I/O | 04 | 003 | java.nio.file |

### Nivel Avanzado (Épicas 5-8)

| Concepto | Épica | Historia | Comparación con Java |
|----------|-------|----------|---------------------|
| Arc (atomic reference counting) | 05 | 001-004 | AtomicReference |
| Atomic* types | 05 | 001 | AtomicInteger, etc. |
| Tokio runtime | 05 | 001-005 | ExecutorService |
| Channels (mpsc, oneshot) | 05 | 002, 003 | BlockingQueue |
| Feature flags (Cargo) | 06 | 005 | Maven profiles |
| Associated types | 06 | 004, 006 | Sin equivalente directo |
| Generic bounds complejos | 06 | 004-006 | Bounded wildcards |
| SQLx y migrations | 06 | 003-005 | Flyway/Liquibase + JPA |
| Macros declarativas | 07 | 002 | Sin equivalente |
| Builder pattern | 07 | 001, 003 | Builder pattern |
| Async streams | 08 | 001-003 | Reactive Streams |
| Pin y Unpin | 08 | 003 | Sin equivalente |
| Broadcast channels | 08 | 002 | PublishSubject (RxJava) |

### Nivel Enterprise (Épicas 9-10)

| Concepto | Épica | Historia | Comparación con Java |
|----------|-------|----------|---------------------|
| Arquitectura hexagonal | 09 | Todas | Clean Architecture |
| Plugin architecture | 09 | 001-003 | SPI / ServiceLoader |
| Template engines (Tera) | 09 | 004, 005 | Thymeleaf / Freemarker |
| Rule engines | 09 | 006 | Drools |
| gRPC (tonic) | 10 | 005 | gRPC-java |
| Consistent hashing | 10 | 001 | Guava Hashing |
| Observabilidad (tracing) | 10 | 006 | Micrometer + Sleuth |
| Graceful shutdown | 10 | 006 | @PreDestroy |

---

## Definition of Done (Global)

Una historia se considera **Done** cuando cumple TODOS estos criterios:

### Código

- [ ] Código compila sin warnings (`cargo build --all-features`)
- [ ] Formateado con `cargo fmt`
- [ ] Sin errores de clippy (`cargo clippy -- -D warnings`)
- [ ] Tests unitarios pasan (`cargo test`)
- [ ] Cobertura de tests > 80% para código nuevo
- [ ] Sin dependencias con vulnerabilidades conocidas (`cargo audit`)

### Documentación

- [ ] Rustdoc para APIs públicas (`///` comments)
- [ ] README actualizado si hay cambios de uso
- [ ] Changelog actualizado en el index de la épica

### Calidad

- [ ] Sin unwrap() en código de producción (usar expect() o ? operator)
- [ ] Errores tipados con thiserror (no strings)
- [ ] Logs estructurados con tracing
- [ ] Métricas relevantes expuestas

### Integración

- [ ] CI pipeline pasa
- [ ] Tests de integración pasan (si aplica)
- [ ] Revisión de código aprobada
- [ ] Merged a branch principal

---

## Convenciones del Proyecto

### Naming

| Elemento | Convención | Ejemplo |
|----------|------------|---------|
| Crates | kebab-case | vortex-core |
| Modules | snake_case | property_source |
| Types (struct, enum) | PascalCase | ConfigMap |
| Functions | snake_case | fetch_config |
| Constants | SCREAMING_SNAKE | MAX_CACHE_SIZE |
| Traits | PascalCase | ConfigSource |

### Estructura de Crates

```
crates/vortex-{name}/
├── Cargo.toml
├── src/
│   ├── lib.rs          # Re-exports públicos
│   ├── error.rs        # Tipos de error
│   ├── {feature}/      # Módulos por feature
│   │   ├── mod.rs
│   │   └── ...
│   └── tests/          # Tests de integración internos
└── tests/              # Tests de integración externos
```

### Manejo de Errores

```rust
// Usar thiserror para errores de dominio
#[derive(Debug, thiserror::Error)]
pub enum ConfigError {
    #[error("configuration not found: {app}/{profile}")]
    NotFound { app: String, profile: String },

    #[error("failed to parse configuration: {0}")]
    ParseError(#[from] serde_yaml::Error),
}

// Usar anyhow en capas de aplicación
pub async fn handle_request(...) -> anyhow::Result<Response> {
    // ...
}
```

### Logging y Tracing

```rust
use tracing::{info, warn, error, instrument};

#[instrument(skip(self), fields(app = %app, profile = %profile))]
pub async fn fetch_config(&self, app: &str, profile: &str) -> Result<ConfigMap> {
    info!("fetching configuration");
    // ...
}
```

---

## KPIs del Proyecto

| Métrica | Objetivo | Medición |
|---------|----------|----------|
| Cold start | < 500ms | CI benchmark |
| Memory footprint | < 30MB | Docker stats |
| Request latency p99 | < 10ms | Prometheus histogram |
| Config propagation | < 30s | Tracing |
| Build time (debug) | < 30s | CI metrics |
| Build time (release) | < 2min | CI metrics |
| Test coverage | > 80% | cargo-llvm-cov |

---

## Stack Tecnológico

### Core

| Categoría | Crate | Versión | Propósito |
|-----------|-------|---------|-----------|
| HTTP | axum | 0.7 | API REST |
| Async Runtime | tokio | 1.x | Runtime async |
| Serialization | serde | 1.x | JSON/YAML/Properties |
| Git | gix | 0.58 | Operaciones Git |
| Cache | moka | 0.12 | Cache async |
| Database | sqlx | 0.7 | SQL async |
| Tracing | tracing | 0.1 | Observabilidad |
| Errors | thiserror/anyhow | 1.x / 1.x | Manejo de errores |

### Avanzado

| Categoría | Crate | Versión | Propósito |
|-----------|-------|---------|-----------|
| Templating | tera | 1.19 | Config templates |
| Validation | jsonschema | 0.18 | Schema validation |
| Crypto | ring | 0.17 | Signing |
| Metrics | metrics-exporter-prometheus | 0.13 | Prometheus |
| gRPC | tonic | 0.11 | Federation |
| S3 | aws-sdk-s3 | 1.x | S3 backend |

---

## Referencias

- [PRD Completo](../PRD.md)
- [Rust Book](https://doc.rust-lang.org/book/)
- [Axum Documentation](https://docs.rs/axum)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Spring Cloud Config Reference](https://spring.io/projects/spring-cloud-config)
