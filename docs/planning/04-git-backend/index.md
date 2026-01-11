# Epica 04: Git Backend con gix

## Objetivo

Implementar un backend de configuracion basado en repositorios Git utilizando la crate `gix`. Este backend permite almacenar configuraciones en repositorios Git, soportando operaciones de clone, pull, checkout de branches/tags, y lectura de archivos de configuracion en multiples formatos (YAML, JSON, Properties).

Esta epica transforma Vortex Config de un servidor de configuracion estatico a uno dinamico que puede sincronizarse automaticamente con repositorios Git, similar a Spring Cloud Config Server.

---

## Conceptos de Rust Cubiertos (Nivel Intermedio-Avanzado)

| Concepto | Historia | Comparacion con Java |
|----------|----------|---------------------|
| `async_trait` macro | 001 | Interfaces async (no nativo en Java) |
| Lifetimes en traits | 001, 003 | No aplica (GC en Java) |
| Mutex/RwLock | 005, 006 | ReentrantLock, ReadWriteLock |
| File I/O async | 002, 003 | NIO.2 con CompletableFuture |
| `anyhow` para errores | 002-006 | Exception chaining |
| Arc para shared state | 005 | Referencias compartidas (GC) |
| Tokio spawn/intervals | 005 | ScheduledExecutorService |
| tempfile para tests | 006 | JUnit @TempDir |
| gix crate | 002, 004 | JGit library |

---

## Historias de Usuario

| # | Titulo | Descripcion | Puntos |
|---|--------|-------------|--------|
| 001 | [Trait ConfigSource](./story-001-config-source-trait.md) | Abstraccion para backends de configuracion | 3 |
| 002 | [Clone y Pull de Repositorios](./story-002-clone-pull.md) | Clonar repos Git y mantenerlos actualizados | 5 |
| 003 | [Lectura de Archivos de Config](./story-003-file-reading.md) | Leer y parsear archivos YAML/Properties | 5 |
| 004 | [Soporte de Labels](./story-004-labels-support.md) | Checkout de branches y tags especificos | 5 |
| 005 | [Refresh y Sincronizacion](./story-005-refresh-sync.md) | Pull periodico y deteccion de cambios | 5 |
| 006 | [Tests con Repositorio Local](./story-006-git-tests.md) | Suite de tests usando repos Git de prueba | 3 |

**Total**: 26 puntos de historia

---

## Dependencias

### Epicas Prerequisito

| Epica | Razon |
|-------|-------|
| 02 - Core Types | ConfigMap, PropertySource, serializacion con serde |
| 03 - HTTP Server | Servidor Axum que consumira el backend Git |

### Dependencias de Crates

```toml
[dependencies]
# Git operations
gix = { version = "0.66", default-features = false, features = [
    "blocking-network-client",
    "blocking-http-transport-reqwest-rust-tls"
] }

# Async runtime
tokio = { version = "1", features = ["full", "sync"] }
async-trait = "0.1"

# File handling
tokio-util = { version = "0.7", features = ["io"] }

# Serialization (from Epic 02)
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"
java-properties = "2.0"

# Error handling
thiserror = "1"
anyhow = "1"

# Logging
tracing = "0.1"

# Utilities
parking_lot = "0.12"  # Faster Mutex/RwLock

[dev-dependencies]
tempfile = "3"
tokio-test = "0.4"
```

---

## Criterios de Aceptacion

### Funcionales

- [ ] `ConfigSource` trait define interfaz para cualquier backend
- [ ] Clone de repositorios Git publicos y privados (con credenciales)
- [ ] Pull automatico para mantener repositorio actualizado
- [ ] Lectura de archivos `.yml`, `.yaml`, `.json`, `.properties`
- [ ] Soporte de labels: branches (`main`, `develop`) y tags (`v1.0.0`)
- [ ] Resolucion de configuracion por application/profile/label
- [ ] Refresh periodico configurable (default: 30 segundos)
- [ ] Deteccion de cambios y notificacion

### No Funcionales

- [ ] Clone de repositorio < 30 segundos para repos de tamano medio
- [ ] Refresh incremental (pull) < 5 segundos
- [ ] Lectura de archivo de config < 10ms
- [ ] Thread-safe para acceso concurrente
- [ ] Memory footprint proporcional al tamano del repositorio

### Compatibilidad Spring Cloud Config

- [ ] Misma estructura de directorios: `/{application}-{profile}.yml`
- [ ] Soporte de archivos compartidos: `application.yml`
- [ ] Resolucion de labels compatible con Spring

---

## Definition of Done

- [ ] Codigo compila sin warnings (`cargo build --all-features`)
- [ ] Formateado con `cargo fmt`
- [ ] Sin errores de clippy (`cargo clippy -- -D warnings`)
- [ ] Tests unitarios pasan con cobertura > 80%
- [ ] Tests de integracion con repositorio Git local
- [ ] Rustdoc para todas las APIs publicas
- [ ] Changelog actualizado
- [ ] Sin `unwrap()` en codigo de produccion
- [ ] Logs estructurados con tracing
- [ ] CI pipeline verde

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| gix API inestable | Media | Alto | Pinear version, wrapper interno |
| Timeouts en clone de repos grandes | Media | Medio | Timeout configurable, clone shallow |
| Race conditions en refresh | Media | Alto | RwLock con estrategia clara |
| Autenticacion SSH compleja | Alta | Medio | Empezar con HTTPS, SSH como feature |
| Memory leaks con repos grandes | Baja | Alto | Profiling, limites configurables |

---

## Decisiones Arquitectonicas (ADRs)

### ADR-004: gix como libreria Git

**Estado**: Aceptado

**Contexto**: Necesitamos una libreria Git nativa en Rust para operaciones de clone, pull, y checkout.

**Decision**: Usar `gix` (anteriormente gitoxide) como libreria Git.

**Razones**:
- Implementacion pura en Rust (sin dependencia de libgit2/C)
- Activamente mantenido por el autor de git2-rs
- Mejor performance que git2 en muchas operaciones
- API mas idiomatica para Rust

**Alternativas consideradas**:
- git2-rs: Binding a libgit2, mas maduro pero requiere C toolchain
- Ejecutar git CLI: Simple pero menos control y parsing de output

### ADR-005: Estrategia de Sincronizacion

**Estado**: Aceptado

**Contexto**: Debemos mantener el repositorio local sincronizado con el remoto.

**Decision**: Pull periodico con intervalo configurable + refresh on-demand.

**Razones**:
- Simple de implementar y debuggear
- Predecible en terminos de carga de red
- Compatible con repos sin webhook support

**Alternativas consideradas**:
- Webhooks: Requiere endpoint publico, mas complejo
- Long polling: Mayor complejidad, beneficio marginal

### ADR-006: Manejo de Estado Compartido

**Estado**: Aceptado

**Contexto**: Multiples requests pueden leer configuracion mientras ocurre un refresh.

**Decision**: Usar `RwLock` para permitir lecturas concurrentes, escrituras exclusivas.

**Razones**:
- Lecturas son mucho mas frecuentes que escrituras
- RwLock permite maxima concurrencia en lecturas
- parking_lot::RwLock es mas rapido que std

---

## Reglas Estrictas

1. **No bloquear el runtime async**: Todas las operaciones de I/O deben ser async o ejecutarse en `spawn_blocking`
2. **RwLock discipline**: Nunca mantener lock mientras se hace I/O
3. **Errores contextuales**: Usar `anyhow::Context` para agregar contexto a errores
4. **Clone defensivo**: Siempre clonar datos antes de liberar locks
5. **Timeouts en operaciones de red**: Todas las operaciones Git deben tener timeout
6. **Tests con repos efimeros**: Usar tempfile para crear repos de prueba

---

## Estructura del Crate

```
crates/vortex-git/
├── Cargo.toml
├── src/
│   ├── lib.rs                 # Re-exports publicos
│   ├── error.rs               # GitBackendError
│   ├── source/
│   │   ├── mod.rs
│   │   ├── trait.rs           # ConfigSource trait
│   │   └── git.rs             # GitConfigSource implementation
│   ├── repository/
│   │   ├── mod.rs
│   │   ├── clone.rs           # Clone operations
│   │   ├── pull.rs            # Pull/fetch operations
│   │   └── checkout.rs        # Branch/tag checkout
│   ├── reader/
│   │   ├── mod.rs
│   │   ├── file.rs            # File reading
│   │   └── parser.rs          # Format detection and parsing
│   ├── sync/
│   │   ├── mod.rs
│   │   ├── refresh.rs         # Periodic refresh
│   │   └── state.rs           # Shared state management
│   └── config.rs              # GitBackendConfig
└── tests/
    ├── clone_tests.rs
    ├── pull_tests.rs
    ├── checkout_tests.rs
    ├── reader_tests.rs
    └── helpers/
        └── mod.rs             # Test utilities, temp repos
```

---

## Diagrama de Flujo del Backend Git

```
                    ┌─────────────────────────────────────────┐
                    │            HTTP Request                  │
                    │   GET /{app}/{profile}/{label}          │
                    └────────────────┬────────────────────────┘
                                     │
                    ┌────────────────▼────────────────────────┐
                    │          ConfigSource Trait              │
                    │    get_config(app, profile, label)      │
                    └────────────────┬────────────────────────┘
                                     │
                    ┌────────────────▼────────────────────────┐
                    │         GitConfigSource                  │
                    │                                          │
                    │  ┌──────────────────────────────────┐   │
                    │  │      RwLock<RepoState>           │   │
                    │  │  - current_commit: ObjectId      │   │
                    │  │  - local_path: PathBuf           │   │
                    │  │  - last_refresh: Instant         │   │
                    │  └──────────────────────────────────┘   │
                    └────────────────┬────────────────────────┘
                                     │
              ┌──────────────────────┼──────────────────────┐
              │                      │                      │
    ┌─────────▼─────────┐  ┌────────▼────────┐  ┌─────────▼─────────┐
    │   Clone/Pull      │  │    Checkout     │  │   Read Files      │
    │   Repository      │  │   Label/Branch  │  │   Parse Config    │
    └─────────┬─────────┘  └────────┬────────┘  └─────────┬─────────┘
              │                      │                      │
              │            ┌────────▼────────┐              │
              │            │   Local Repo    │◄─────────────┘
              │            │   .git folder   │
              │            └────────┬────────┘
              │                      │
              └──────────────────────┼──────────────────────┘
                                     │
                    ┌────────────────▼────────────────────────┐
                    │           File System                    │
                    │   /{app}.yml, /{app}-{profile}.yml      │
                    │   /application.yml                       │
                    └────────────────┬────────────────────────┘
                                     │
                    ┌────────────────▼────────────────────────┐
                    │        ConfigMap (from Epic 02)         │
                    └─────────────────────────────────────────┘

            ┌─────────────────────────────────────────────────┐
            │              Background Tasks                    │
            │                                                  │
            │  ┌────────────────────────────────────────────┐ │
            │  │  Tokio Spawn: Periodic Refresh             │ │
            │  │  - Every 30s (configurable)                │ │
            │  │  - git fetch + compare commits             │ │
            │  │  - Update RwLock<RepoState> if changed     │ │
            │  └────────────────────────────────────────────┘ │
            └─────────────────────────────────────────────────┘
```

---

## Patron de Resolucion de Archivos

Siguiendo la convencion de Spring Cloud Config:

```
Para request: GET /myapp/prod/v1.0.0

Archivos buscados (en orden de prioridad):
1. myapp-prod.yml        (app + profile especifico)
2. myapp.yml             (app sin profile)
3. application-prod.yml  (default app + profile)
4. application.yml       (default app)

Merge strategy: Ultimo archivo encontrado tiene mayor prioridad
```

---

## Changelog

| Version | Fecha | Cambios |
|---------|-------|---------|
| 0.1.0 | 2026-01-XX | Creacion inicial de la epica |

---

## Referencias

- [gix Documentation](https://docs.rs/gix)
- [Spring Cloud Config - Git Backend](https://docs.spring.io/spring-cloud-config/docs/current/reference/html/#_git_backend)
- [Tokio Sync Primitives](https://docs.rs/tokio/latest/tokio/sync/)
- [parking_lot Documentation](https://docs.rs/parking_lot)

---

**Anterior**: [Epica 03 - HTTP Server](../03-http-server/index.md)
**Siguiente**: [Historia 001 - Trait ConfigSource](./story-001-config-source-trait.md)
