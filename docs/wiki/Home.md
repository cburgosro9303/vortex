# Vortex Config - Wiki

Bienvenido a la wiki de **Vortex Config**, un servidor de configuración cloud-native de alto rendimiento escrito en Rust.

## Navegación Rápida

### 🚀 Primeros Pasos
- **[Getting Started](Getting-Started.md)** - Instalación y primer uso
- **[Configuration](Configuration.md)** - Configurar el servidor
- **[Deployment](Deployment.md)** - Despliegue en producción

### 📚 Documentación Técnica
- **[Architecture](Architecture.md)** - Arquitectura del proyecto
- **[API Reference](API-Reference.md)** - Referencia completa de la API REST
- **[Cache System](Cache-System.md)** - Sistema de cache con Moka

### 👨‍💻 Desarrollo
- **[Development Guide](Development.md)** - Guía para contribuidores
- **[Testing Strategy](Testing-Strategy.md)** - Estrategia de testing
- **[Rust Concepts](Rust-Concepts.md)** - Conceptos de Rust aplicados

### 🔧 Backends
- **[Git Backend](backends/Git-Backend.md)** - Backend de repositorios Git
- **[Future Backends](backends/Future-Backends.md)** - S3, SQL (planificados)

---

## ¿Qué es Vortex Config?

Vortex Config es un **servidor de configuración** diseñado para aplicaciones cloud-native, compatible con Spring Cloud Config Server. Permite centralizar y gestionar configuraciones de múltiples aplicaciones y entornos desde repositorios Git.

### Características Principales

- **🚀 Alto Rendimiento:** Cold start < 500ms, latencia p99 < 10ms
- **💾 Pequeño Footprint:** Imagen Docker ~37MB, memoria idle ~20MB
- **🔄 Compatible Spring:** Drop-in replacement para Spring Cloud Config
- **⚡ Cache Inteligente:** Cache async con Moka, TTL configurable
- **📊 Observabilidad:** Métricas Prometheus, logging estructurado, tracing
- **🔒 Seguro:** Tipos seguros con Rust, sin GC pauses, memory-safe
- **🐳 Cloud-Native:** Docker, Kubernetes-ready, 12-factor compliant

### Estado Actual

| Épica | Estado | Descripción |
|-------|--------|-------------|
| Epic 1: Foundation | ✅ 100% | Workspace, CI/CD, domain model |
| Epic 2: Core Types | ✅ 100% | ConfigMap, serialización, formats |
| Epic 3: HTTP Server | ✅ 100% | Axum, endpoints, middleware |
| Epic 4: Git Backend | ✅ 100% | Clone, fetch, refresh, auth |
| Epic 5: Cache | ✅ 100% | Moka cache, invalidation, metrics |
| Epic 6: Multi-Backend | 📋 Planificado | S3, SQL backends |
| Epic 7: Governance | 📋 Planificado | PLAC, schema validation |
| Epic 8: Real-time | 📋 Planificado | WebSockets, push updates |
| Epic 9: Advanced | 📋 Planificado | Feature flags, templating |
| Epic 10: Enterprise | 📋 Planificado | Canary, drift, federation |

**Completitud Global:** 50% (5/10 épicas)

---

## Quick Start

### Instalación con Docker

```bash
docker run -d \
  -p 8888:8888 \
  -e GIT_URI=https://github.com/your-org/config-repo.git \
  -e GIT_DEFAULT_LABEL=main \
  --name vortex-config \
  vortex-config:latest
```

### Primera Request

```bash
# Health check
curl http://localhost:8888/health

# Obtener configuración
curl http://localhost:8888/myapp/production
```

### Ejemplo de Respuesta

```json
{
  "name": "myapp",
  "profiles": ["production"],
  "label": "main",
  "version": "abc123",
  "state": null,
  "propertySources": [
    {
      "name": "git:main:myapp-production.yml",
      "source": {
        "server.port": 8080,
        "database.url": "jdbc:postgresql://..."
      }
    }
  ]
}
```

---

## Arquitectura de Alto Nivel

```
┌─────────────────────────────────────────────────────────┐
│                  HTTP Request (Axum)                    │
└────────────────────┬────────────────────────────────────┘
                     │
          ┌──────────▼──────────┐
          │   Middleware Layer  │
          │  - RequestId        │
          │  - Logging          │
          │  - Metrics          │
          └──────────┬──────────┘
                     │
          ┌──────────▼──────────┐
          │    Cache Layer      │
          │     (Moka)          │
          └──────────┬──────────┘
                     │
          ┌──────────▼──────────┐
          │  ConfigSource Trait │
          └──────────┬──────────┘
                     │
          ┌──────────▼──────────┐
          │   Git Backend       │
          │    (gix)            │
          └──────────┬──────────┘
                     │
          ┌──────────▼──────────┐
          │   Git Repository    │
          │  (YAML/JSON/Props)  │
          └─────────────────────┘
```

---

## Stack Tecnológico

### Core
- **Rust 1.92+** (edition 2024)
- **Tokio** - Async runtime
- **Axum 0.8** - HTTP framework
- **gix** - Pure Rust Git library
- **Moka 0.12** - Async cache
- **serde** - Serialization

### Observabilidad
- **tracing** - Structured logging
- **metrics** - Metrics collection
- **prometheus-exporter** - Prometheus format

---

## Casos de Uso

### 1. Configuración Centralizada
Centralizar configuraciones de microservicios en un repositorio Git:

```yaml
# config-repo/myapp-production.yml
server:
  port: 8080

database:
  url: jdbc:postgresql://prod-db:5432/myapp
  pool:
    max-size: 20
```

### 2. Múltiples Entornos
Gestionar configuraciones por entorno (dev, staging, production):

```bash
# Desarrollo
curl http://vortex:8888/myapp/dev

# Staging
curl http://vortex:8888/myapp/staging

# Producción
curl http://vortex:8888/myapp/production
```

### 3. Feature Branches
Probar configuraciones en branches específicos:

```bash
# Branch principal
curl http://vortex:8888/myapp/dev/main

# Feature branch
curl http://vortex:8888/myapp/dev/feature%2Fnew-feature
```

### 4. Spring Boot Integration
Integración transparente con aplicaciones Spring Boot existentes:

```yaml
# application.yml (Spring Boot client)
spring:
  application:
    name: myapp
  cloud:
    config:
      uri: http://vortex-config:8888
      profile: ${ENVIRONMENT}
      label: main
```

---

## Ventajas vs Spring Cloud Config

| Característica | Spring Cloud Config | Vortex Config |
|----------------|---------------------|---------------|
| **Cold Start** | 5-15s | < 500ms |
| **Memory** | ~200MB heap | < 30MB total |
| **Latency p99** | ~50ms | < 10ms (cached) |
| **Footprint** | ~150MB imagen | ~37MB imagen |
| **Async Native** | No (servlet) | Sí (Tokio) |
| **Cache Built-in** | Limitado | Avanzado (Moka) |
| **Métricas** | Básicas | Prometheus nativo |
| **Type Safety** | Runtime | Compile-time |

---

## Recursos Adicionales

### Documentación
- **[PRD Completo](../PRD.md)** - Product Requirements Document
- **[Planning](../planning/)** - Épicas y user stories
- **[Reviews](../reviews/)** - Retrospectivas de épicas

### Repositorio
- **GitHub:** https://github.com/cburgosro9303/vortex-config
- **Issues:** https://github.com/cburgosro9303/vortex-config/issues
- **Discussions:** https://github.com/cburgosro9303/vortex-config/discussions

### Referencias Externas
- [Rust Book](https://doc.rust-lang.org/book/)
- [Axum Documentation](https://docs.rs/axum)
- [Spring Cloud Config](https://spring.io/projects/spring-cloud-config)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)

---

## Licencia

Polyform Noncommercial License 1.0.0

Para uso comercial, contactar: [@cburgosro9303](https://github.com/cburgosro9303)
