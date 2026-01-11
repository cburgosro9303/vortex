# Epica 03: HTTP Server con Axum - API Spring Compatible

## Objetivo

Implementar un servidor HTTP utilizando Axum que exponga una API REST completamente compatible con Spring Cloud Config. El servidor debe soportar los endpoints canonicos `/{app}/{profile}` y `/{app}/{profile}/{label}`, con soporte para multiples formatos de respuesta (JSON, YAML, .properties).

Esta epica establece la capa de presentacion de Vortex Config, permitiendo que aplicaciones Spring Boot existentes puedan migrar sin cambios en su configuracion de cliente.

---

## Conceptos de Rust Cubiertos (Nivel Intermedio)

| Concepto | Historia | Comparacion con Java |
|----------|----------|---------------------|
| Traits (definicion e implementacion) | 001, 002 | Interfaces |
| Generics basicos | 001-003 | Generics `<T>` |
| impl blocks | Todas | Methods en class |
| Closures | 002-004 | Lambdas |
| Fn, FnMut, FnOnce | 004 | Functional interfaces |
| async/await | 001-006 | CompletableFuture |
| Axum Extractors | 002-004 | @PathVariable, @RequestBody |
| Tower middleware | 005 | Spring Interceptors/Filters |

---

## Historias de Usuario

| # | Titulo | Descripcion | Puntos |
|---|--------|-------------|--------|
| 001 | [Scaffold del Server Axum](./story-001-axum-scaffold.md) | Setup basico con health check | 3 |
| 002 | [Endpoint GET /{app}/{profile}](./story-002-app-profile-endpoint.md) | Ruta principal compatible Spring | 5 |
| 003 | [Endpoint GET /{app}/{profile}/{label}](./story-003-label-endpoint.md) | Soporte de labels (branches/tags) | 3 |
| 004 | [Content Negotiation](./story-004-content-negotiation.md) | JSON, YAML, .properties segun Accept | 5 |
| 005 | [Middleware de Logging y RequestId](./story-005-logging-middleware.md) | Tracing basico de requests | 3 |
| 006 | [Tests de Integracion HTTP](./story-006-integration-tests.md) | Tests con tower-test | 5 |

**Total**: 24 puntos de historia

---

## Dependencias

### Epicas Prerequisito

| Epica | Razon |
|-------|-------|
| 01 - Foundation | Workspace configurado, toolchain, CI basico |
| 02 - Core Types | ConfigMap, PropertySource, serializacion con serde |

### Dependencias de Crates

```toml
[dependencies]
# Framework HTTP
axum = "0.7"
tokio = { version = "1", features = ["full"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "request-id", "cors"] }

# Serializacion
serde = { version = "1", features = ["derive"] }
serde_json = "1"
serde_yaml = "0.9"

# Observabilidad
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter"] }

# Errores
thiserror = "1"
anyhow = "1"

# Utilidades
uuid = { version = "1", features = ["v4"] }

[dev-dependencies]
# Testing
tower = { version = "0.4", features = ["util"] }
hyper = { version = "1", features = ["full"] }
http-body-util = "0.1"
```

---

## Criterios de Aceptacion

### Funcionales

- [ ] `GET /health` retorna 200 OK con body `{"status": "UP"}`
- [ ] `GET /{app}/{profile}` retorna configuracion en formato Spring Cloud Config
- [ ] `GET /{app}/{profile}/{label}` soporta branches y tags
- [ ] Content negotiation funciona para JSON, YAML y .properties
- [ ] Headers `X-Request-Id` presentes en todas las respuestas
- [ ] Logs estructurados para cada request

### No Funcionales

- [ ] Tiempo de respuesta p99 < 10ms para configuraciones cacheadas
- [ ] El servidor inicia en < 500ms
- [ ] Memory footprint < 20MB en idle

### Compatibilidad Spring Cloud Config

- [ ] Response schema identico a Spring Cloud Config Server
- [ ] Mismo comportamiento de resolucion de profiles
- [ ] Soporte de multiple profiles separados por coma

---

## Definition of Done

- [ ] Codigo compila sin warnings (`cargo build --all-features`)
- [ ] Formateado con `cargo fmt`
- [ ] Sin errores de clippy (`cargo clippy -- -D warnings`)
- [ ] Tests unitarios pasan con cobertura > 80%
- [ ] Tests de integracion HTTP pasan
- [ ] Rustdoc para todas las APIs publicas
- [ ] Changelog actualizado
- [ ] Sin `unwrap()` en codigo de produccion
- [ ] Logs estructurados con tracing
- [ ] CI pipeline verde

---

## Riesgos y Mitigaciones

| Riesgo | Probabilidad | Impacto | Mitigacion |
|--------|--------------|---------|------------|
| Incompatibilidad con Spring Boot client | Media | Alto | Tests de compatibilidad con cliente real |
| Performance degradada por middleware | Baja | Medio | Benchmarks en cada historia |
| Complejidad en content negotiation | Media | Medio | Investigacion previa de Accept header parsing |
| Manejo incorrecto de errores async | Media | Alto | Uso de patron Result + ? operator |

---

## Decisiones Arquitectonicas (ADRs)

### ADR-001: Axum como Framework HTTP

**Estado**: Aceptado

**Contexto**: Necesitamos un framework HTTP async para Rust que sea performante y ergonomico.

**Decision**: Usar Axum 0.7+ como framework HTTP.

**Razones**:
- Desarrollado por el equipo de Tokio
- Excelente integracion con Tower (middleware)
- Type-safe extractors
- Macro-free routing
- Comunidad activa

**Alternativas consideradas**:
- Actix-web: Mas features pero API menos ergonomica
- Warp: Buen framework pero menos mantenido
- Rocket: Requiere nightly para algunas features

### ADR-002: Estructura de Response Compatible Spring

**Estado**: Aceptado

**Contexto**: Debemos ser drop-in replacement de Spring Cloud Config Server.

**Decision**: Implementar exactamente el mismo schema de respuesta JSON.

```json
{
  "name": "application",
  "profiles": ["default"],
  "label": "main",
  "version": null,
  "state": null,
  "propertySources": [
    {
      "name": "file:config/application.yml",
      "source": {
        "server.port": 8080,
        "spring.application.name": "demo"
      }
    }
  ]
}
```

### ADR-003: Tower para Middleware

**Estado**: Aceptado

**Contexto**: Necesitamos middleware para logging, request-id, CORS, etc.

**Decision**: Usar Tower layers para todos los middleware.

**Razones**:
- Estandar de facto en ecosistema Rust async
- Composable y testeable
- Axum tiene integracion nativa
- Reutilizable entre servicios

---

## Reglas Estrictas

1. **No unwrap() en produccion**: Usar `?` operator o `expect()` con mensaje descriptivo
2. **Todos los endpoints son async**: No bloquear el runtime de Tokio
3. **Errores tipados**: Usar `thiserror` para errores de dominio
4. **Extractors validados**: Usar extractors con validacion integrada
5. **Tests para cada endpoint**: Minimo tests de happy path y error cases
6. **Logs con contexto**: Usar `tracing::instrument` en handlers

---

## Estructura del Crate

```
crates/vortex-server/
├── Cargo.toml
├── src/
│   ├── lib.rs              # Re-exports publicos
│   ├── error.rs            # Tipos de error HTTP
│   ├── server.rs           # Configuracion del servidor
│   ├── handlers/
│   │   ├── mod.rs
│   │   ├── health.rs       # Health check
│   │   ├── config.rs       # Endpoints de configuracion
│   │   └── response.rs     # Tipos de respuesta
│   ├── extractors/
│   │   ├── mod.rs
│   │   ├── path.rs         # Path extractors
│   │   └── accept.rs       # Accept header extractor
│   ├── middleware/
│   │   ├── mod.rs
│   │   ├── request_id.rs   # Request ID layer
│   │   └── logging.rs      # Logging layer
│   └── response/
│       ├── mod.rs
│       ├── json.rs         # JSON serializer
│       ├── yaml.rs         # YAML serializer
│       └── properties.rs   # Properties serializer
└── tests/
    ├── health_test.rs
    ├── config_test.rs
    └── helpers/
        └── mod.rs          # Test utilities
```

---

## Diagrama de Flujo de Request

```
                         ┌─────────────────┐
                         │   HTTP Request  │
                         └────────┬────────┘
                                  │
                         ┌────────▼────────┐
                         │  RequestId      │
                         │  Middleware     │
                         └────────┬────────┘
                                  │
                         ┌────────▼────────┐
                         │  Logging        │
                         │  Middleware     │
                         └────────┬────────┘
                                  │
                         ┌────────▼────────┐
                         │  Axum Router    │
                         └────────┬────────┘
                                  │
           ┌──────────────────────┼──────────────────────┐
           │                      │                      │
   ┌───────▼───────┐     ┌───────▼───────┐     ┌───────▼───────┐
   │   /health     │     │ /{app}/{prof} │     │/{app}/{p}/{l} │
   └───────┬───────┘     └───────┬───────┘     └───────┬───────┘
           │                      │                      │
           │             ┌────────▼────────┐             │
           │             │  Path Extractor │◄────────────┘
           │             └────────┬────────┘
           │                      │
           │             ┌────────▼────────┐
           │             │ Accept Extractor│
           │             └────────┬────────┘
           │                      │
           │             ┌────────▼────────┐
           │             │ Config Handler  │
           │             └────────┬────────┘
           │                      │
           │             ┌────────▼────────┐
           │             │Content Negotiate│
           │             └────────┬────────┘
           │                      │
           └──────────────────────┼──────────────────────┘
                                  │
                         ┌────────▼────────┐
                         │  HTTP Response  │
                         └─────────────────┘
```

---

## Changelog

| Version | Fecha | Cambios |
|---------|-------|---------|
| 0.1.0 | 2025-01-XX | Creacion inicial de la epica |

---

## Referencias

- [Axum Documentation](https://docs.rs/axum)
- [Tower Documentation](https://docs.rs/tower)
- [Spring Cloud Config API](https://docs.spring.io/spring-cloud-config/docs/current/reference/html/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
