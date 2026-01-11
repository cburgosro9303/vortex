# Epica 06: Persistencia Multi-Backend (S3/SQL)

## Objetivo

Implementar backends de almacenamiento alternativos para Vortex Config que permitan deployments cloud-native flexibles. Esta epica extiende el sistema de configuracion mas alla del backend Git, agregando soporte para Amazon S3 y bases de datos SQL (PostgreSQL, MySQL, SQLite) mediante SQLx.

Los backends adicionales permiten:
- **S3**: Almacenamiento economico y escalable para configuraciones en AWS/MinIO
- **SQL**: Integracion con infraestructura de base de datos existente
- **Compositor**: Combinar multiples fuentes con prioridades configurables

Esta epica es fundamental para organizaciones que prefieren no depender de Git para almacenamiento de configuraciones, o que necesitan integrarse con sistemas existentes.

---

## Conceptos de Rust Cubiertos (Nivel Avanzado)

| Concepto | Historia | Comparacion con Java |
|----------|----------|---------------------|
| Associated Types | 001, 004 | Generics en interfaces |
| Generic Bounds (where clauses) | 004, 006 | Bounded type parameters |
| Async Streams | 001, 002 | Reactive Streams (Flux) |
| Feature Flags | 005 | Maven profiles |
| SQLx compile-time verification | 004 | No equivalente directo |
| Conditional Compilation | 005 | #ifdef (C), profiles |
| Trait Objects (dyn Trait) | 006 | Interface references |
| Strategy Pattern en Rust | 006 | Strategy pattern clasico |
| Testcontainers | 007 | Testcontainers-java |

---

## Historias de Usuario

| # | Titulo | Descripcion | Puntos |
|---|--------|-------------|--------|
| 001 | [Backend S3 - Lectura](./story-001-s3-read.md) | Leer configuraciones desde S3 | 5 |
| 002 | [Backend S3 - Listing y Versionado](./story-002-s3-versioning.md) | Listar y versionar configs en S3 | 5 |
| 003 | [Schema SQL para Configuraciones](./story-003-sql-schema.md) | Disenar tablas y migrations con SQLx | 3 |
| 004 | [Backend SQL con SQLx](./story-004-sqlx-backend.md) | Implementar ConfigSource para PostgreSQL | 8 |
| 005 | [Soporte Multi-Database](./story-005-multi-database.md) | Abstraer para MySQL, SQLite | 5 |
| 006 | [Backend Compositor](./story-006-compositor.md) | Combinar backends con prioridades | 5 |
| 007 | [Tests de Integracion Multi-Backend](./story-007-backend-tests.md) | Testcontainers para PostgreSQL y LocalStack | 5 |

**Total**: 36 puntos de historia

---

## Dependencias

### Epicas Prerequisito

| Epica | Razon |
|-------|-------|
| 01 - Foundation | Workspace configurado, toolchain, CI basico |
| 02 - Core Types | ConfigMap, PropertySource, trait ConfigSource |
| 04 - Git Backend | Trait ConfigSource definido, patrones establecidos |

### Dependencias de Crates

```toml
[dependencies]
# AWS S3
aws-sdk-s3 = "1.0"
aws-config = "1.0"
aws-credential-types = "1.0"

# SQL con SQLx
sqlx = { version = "0.8", features = ["runtime-tokio", "postgres", "mysql", "sqlite"] }

# Async utilities
tokio = { version = "1", features = ["full"] }
tokio-stream = "0.1"
futures = "0.3"
async-trait = "0.1"

# Serializacion
serde = { version = "1", features = ["derive"] }
serde_json = "1"

# Observabilidad
tracing = "0.1"

# Errores
thiserror = "1"

[dev-dependencies]
# Testing con containers
testcontainers = "0.18"
testcontainers-modules = { version = "0.6", features = ["postgres", "localstack"] }
tokio-test = "0.4"
```

### Feature Flags del Crate

```toml
[features]
default = ["git"]
git = ["git2"]
s3 = ["aws-sdk-s3", "aws-config"]
postgres = ["sqlx/postgres"]
mysql = ["sqlx/mysql"]
sqlite = ["sqlx/sqlite"]
sql = ["postgres"]  # Default SQL backend
all-backends = ["git", "s3", "postgres", "mysql", "sqlite"]
```

---

## Criterios de Aceptacion

### Funcionales

- [ ] Backend S3 puede leer configuraciones desde buckets S3/MinIO
- [ ] Backend S3 soporta versionado de objetos S3
- [ ] Backend S3 lista todas las configuraciones disponibles
- [ ] Schema SQL soporta configuraciones versionadas con metadata
- [ ] Backend SQLx implementa ConfigSource completo para PostgreSQL
- [ ] Soporte para MySQL y SQLite con feature flags
- [ ] Backend Compositor combina multiples backends con prioridades
- [ ] Configuraciones de backends superiores sobrescriben inferiores

### No Funcionales

- [ ] Conexiones S3 con retry automatico y backoff exponencial
- [ ] Connection pooling para bases de datos SQL
- [ ] Queries SQL verificados en compile-time con SQLx
- [ ] Memory footprint < 50MB por backend activo
- [ ] Latencia p99 < 100ms para lecturas SQL cacheadas

### Seguridad

- [ ] Credenciales AWS via IAM roles o environment
- [ ] Conexiones SQL via TLS en produccion
- [ ] No logging de credenciales o datos sensibles
- [ ] Validacion de nombres de bucket/tabla contra injection

---

## Definition of Done

- [ ] Codigo compila sin warnings (`cargo build --all-features`)
- [ ] Compila con cada feature flag individual
- [ ] Formateado con `cargo fmt`
- [ ] Sin errores de clippy (`cargo clippy -- -D warnings`)
- [ ] Tests unitarios con cobertura > 80%
- [ ] Tests de integracion con Testcontainers pasan
- [ ] Rustdoc para todas las APIs publicas
- [ ] Ejemplos de configuracion documentados
- [ ] Changelog actualizado
- [ ] Sin `unwrap()` en codigo de produccion
- [ ] Logs estructurados con tracing
- [ ] CI pipeline verde con todos los feature flags

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Inconsistencia entre backends | Media | Alto | Interface comun estricta, tests de conformidad |
| Latencia variable en S3 | Alta | Medio | Caching agresivo, timeouts configurables |
| Schema SQL migrations complejas | Media | Medio | Migraciones incrementales, rollback plan |
| Feature flags combinaciones invalidas | Baja | Alto | Tests de compilacion para cada combinacion |
| Credentials leakage en logs | Baja | Critico | Redaction automatica, code review |
| Testcontainers flaky en CI | Media | Bajo | Retries, health checks robustos |

---

## Decisiones Arquitectonicas (ADRs)

### ADR-001: SQLx sobre otros ORMs

**Estado**: Aceptado

**Contexto**: Necesitamos interactuar con bases de datos SQL de forma segura y performante.

**Decision**: Usar SQLx como capa de acceso a datos.

**Razones**:
- Verificacion de queries en compile-time
- Soporte nativo async/await
- Multiples databases con mismo codigo
- No es ORM - queries SQL explicitas
- Excelente integracion con Tokio

**Alternativas consideradas**:
- Diesel: ORM completo pero sincrono
- SeaORM: Async pero mas abstraccion
- tokio-postgres: Solo PostgreSQL

### ADR-002: Feature Flags para Backends

**Estado**: Aceptado

**Contexto**: No todos los usuarios necesitan todos los backends, y cada uno agrega dependencias.

**Decision**: Usar feature flags de Cargo para habilitar backends selectivamente.

**Razones**:
- Reduce tamano del binario
- Evita dependencias innecesarias
- Compile-time configuration
- Patron comun en ecosistema Rust

**Ejemplo**:
```toml
# Solo S3
vortex-config = { version = "0.1", features = ["s3"] }

# Solo PostgreSQL
vortex-config = { version = "0.1", features = ["postgres"] }

# Todos los backends
vortex-config = { version = "0.1", features = ["all-backends"] }
```

### ADR-003: Compositor Pattern para Multi-Backend

**Estado**: Aceptado

**Contexto**: Necesitamos combinar configuraciones de multiples fuentes con prioridades.

**Decision**: Implementar un Backend Compositor usando Strategy pattern.

**Razones**:
- Flexibilidad en orden de precedencia
- Cada backend es independiente
- Facil de extender
- Testeable unitariamente

**Diagrama**:
```
┌──────────────────────────────────────────────────┐
│              CompositeConfigSource               │
├──────────────────────────────────────────────────┤
│  backends: Vec<(Priority, Box<dyn ConfigSource>)>│
├──────────────────────────────────────────────────┤
│  + get_config() -> merges all sources by priority│
│  + add_backend(priority, source)                 │
│  + remove_backend(name)                          │
└──────────────────────────────────────────────────┘
           │
           │ contiene
           ▼
    ┌──────────────┬──────────────┬──────────────┐
    │ GitBackend   │ S3Backend    │ SqlBackend   │
    │ priority: 10 │ priority: 20 │ priority: 30 │
    └──────────────┴──────────────┴──────────────┘

Merge strategy: Higher priority wins for conflicts
```

### ADR-004: Testcontainers para Integration Tests

**Estado**: Aceptado

**Contexto**: Necesitamos probar contra servicios reales (PostgreSQL, S3) sin infraestructura externa.

**Decision**: Usar testcontainers-rs para tests de integracion.

**Razones**:
- Tests reproducibles
- No requiere infraestructura externa
- Containers efimeros y aislados
- Soporte para PostgreSQL, MySQL, LocalStack

---

## Reglas Estrictas

1. **Todos los backends implementan ConfigSource**: Sin excepciones, mismo contrato
2. **Feature flags son aditivos**: Nunca romper compilacion por feature faltante
3. **Queries SQL verificados**: Usar macros `sqlx::query!` para compile-time checks
4. **No hardcodear credenciales**: Siempre via environment o IAM
5. **Async everywhere**: No operaciones bloqueantes en backends
6. **Graceful degradation**: Si un backend falla, log y continuar con otros
7. **Connection pooling obligatorio**: No crear conexiones por request
8. **Tests con containers**: No mocks para tests de integracion SQL/S3

---

## Estructura del Crate

```
crates/vortex-backends/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Re-exports, feature gates
│   ├── error.rs               # Tipos de error unificados
│   ├── traits.rs              # ConfigSource trait (re-export)
│   ├── s3/
│   │   ├── mod.rs             # S3 backend module
│   │   ├── client.rs          # S3 client wrapper
│   │   ├── config.rs          # S3 configuration
│   │   └── source.rs          # S3ConfigSource implementation
│   ├── sql/
│   │   ├── mod.rs             # SQL backend module
│   │   ├── schema.rs          # Table definitions
│   │   ├── migrations/        # SQLx migrations
│   │   ├── postgres.rs        # PostgreSQL implementation
│   │   ├── mysql.rs           # MySQL implementation
│   │   └── sqlite.rs          # SQLite implementation
│   └── composite/
│       ├── mod.rs             # Composite backend module
│       ├── compositor.rs      # CompositeConfigSource
│       └── priority.rs        # Priority ordering
├── migrations/
│   └── 20240101_initial.sql   # Initial schema
└── tests/
    ├── s3_test.rs             # S3 integration tests
    ├── postgres_test.rs       # PostgreSQL tests
    ├── mysql_test.rs          # MySQL tests
    ├── sqlite_test.rs         # SQLite tests
    ├── composite_test.rs      # Composite backend tests
    └── helpers/
        ├── mod.rs
        ├── containers.rs      # Testcontainers setup
        └── fixtures.rs        # Test data
```

---

## Diagrama de Arquitectura

```
                        ┌─────────────────────┐
                        │   Vortex Server     │
                        │   (HTTP Layer)      │
                        └──────────┬──────────┘
                                   │
                        ┌──────────▼──────────┐
                        │  CompositeSource    │
                        │  (Backend Manager)  │
                        └──────────┬──────────┘
                                   │
          ┌────────────────────────┼────────────────────────┐
          │                        │                        │
┌─────────▼─────────┐   ┌─────────▼─────────┐   ┌─────────▼─────────┐
│   GitBackend      │   │   S3Backend       │   │   SqlBackend      │
│   (Epica 04)      │   │   (Historia 1-2)  │   │   (Historia 3-5)  │
└─────────┬─────────┘   └─────────┬─────────┘   └─────────┬─────────┘
          │                       │                       │
┌─────────▼─────────┐   ┌─────────▼─────────┐   ┌─────────▼─────────┐
│   Git Repository  │   │   S3 Bucket       │   │   PostgreSQL      │
│   (local/remote)  │   │   (AWS/MinIO)     │   │   MySQL/SQLite    │
└───────────────────┘   └───────────────────┘   └───────────────────┘
```

---

## Flujo de Configuracion

```
Request: GET /payment-service/production

┌─────────────────────────────────────────────────────────────────┐
│                    CompositeConfigSource                         │
├─────────────────────────────────────────────────────────────────┤
│                                                                  │
│  1. Query all backends in parallel                               │
│     ┌─────────┐ ┌─────────┐ ┌─────────┐                        │
│     │   Git   │ │   S3    │ │   SQL   │                        │
│     │ pri:10  │ │ pri:20  │ │ pri:30  │                        │
│     └────┬────┘ └────┬────┘ └────┬────┘                        │
│          │           │           │                              │
│          ▼           ▼           ▼                              │
│     ┌─────────┐ ┌─────────┐ ┌─────────┐                        │
│     │ Result  │ │ Result  │ │ Result  │                        │
│     │ port=   │ │ port=   │ │ port=   │                        │
│     │ 8080    │ │ 9090    │ │ (none)  │                        │
│     └────┬────┘ └────┬────┘ └────┬────┘                        │
│          │           │           │                              │
│  2. Merge by priority (higher wins)                             │
│          └───────────┼───────────┘                              │
│                      ▼                                          │
│              ┌─────────────┐                                    │
│              │ Final:      │                                    │
│              │ port=9090   │  ← S3 wins (priority 20 > 10)     │
│              └─────────────┘                                    │
│                                                                  │
└─────────────────────────────────────────────────────────────────┘
```

---

## Changelog

| Version | Fecha | Cambios |
|---------|-------|---------|
| 0.1.0 | 2025-01-XX | Creacion inicial de la epica |

---

## Referencias

- [AWS SDK for Rust](https://docs.aws.amazon.com/sdk-for-rust/latest/dg/welcome.html)
- [SQLx Documentation](https://docs.rs/sqlx)
- [Testcontainers-rs](https://docs.rs/testcontainers)
- [Rust Feature Flags](https://doc.rust-lang.org/cargo/reference/features.html)
- [Spring Cloud Config - Backend Alternatives](https://docs.spring.io/spring-cloud-config/docs/current/reference/html/#_environment_repository)
