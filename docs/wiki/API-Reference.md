# API Reference

Documentación completa de la API REST de Vortex Config.

## Base URL

```
http://localhost:8888
```

---

## Endpoints

### Health Check

Verificar el estado del servidor.

```http
GET /health
```

**Response (200 OK):**
```json
{
  "status": "UP"
}
```

---

### Get Configuration

Obtener configuración para una aplicación y profile.

```http
GET /{application}/{profile}
GET /{application}/{profile}/{label}
```

**Path Parameters:**

| Parámetro | Tipo | Descripción | Ejemplo |
|-----------|------|-------------|---------|
| `application` | string | Nombre de la aplicación | `myapp` |
| `profile` | string | Profile o múltiples separados por coma | `dev` o `dev,local` |
| `label` | string | Branch, tag o commit (opcional, default: main) | `main`, `v1.0.0`, `feature%2Fbranch` |

**Query Parameters:**

| Parámetro | Tipo | Descripción | Ejemplo |
|-----------|------|-------------|---------|
| `format` | string | Formato de respuesta: `json`, `yaml`, `properties` | `?format=yaml` |

**Request Headers:**

| Header | Valores | Descripción |
|--------|---------|-------------|
| `Accept` | `application/json` (default)<br>`application/x-yaml`<br>`text/yaml`<br>`text/plain` | Formato de respuesta |
| `X-Request-Id` | UUID | ID de request opcional (se genera si no se provee) |

**Response (200 OK):**

```json
{
  "name": "myapp",
  "profiles": ["dev"],
  "label": "main",
  "version": "abc123def456",
  "state": null,
  "propertySources": [
    {
      "name": "git:main:myapp-dev.yml",
      "source": {
        "server.port": 8081,
        "database.url": "jdbc:postgresql://localhost:5432/myapp"
      }
    },
    {
      "name": "git:main:myapp.yml",
      "source": {
        "server.port": 8080
      }
    },
    {
      "name": "git:main:application.yml",
      "source": {
        "logging.level": "INFO"
      }
    }
  ]
}
```

**Response Headers:**

```
X-Request-Id: 01234567-89ab-cdef-0123-456789abcdef
Content-Type: application/json
```

**Error Responses:**

| Status | Body | Descripción |
|--------|------|-------------|
| 404 | `{"error":"Configuration not found"}` | No se encontró configuración |
| 500 | `{"error":"Internal server error"}` | Error interno |

---

### Clear Cache

Invalidar cache selectivamente.

```http
DELETE /cache
DELETE /cache/{application}
DELETE /cache/{application}/{profile}
DELETE /cache/{application}/{profile}/{label}
```

**Path Parameters:**

| Parámetro | Tipo | Descripción |
|-----------|------|-------------|
| `application` | string | Nombre de la aplicación (opcional) |
| `profile` | string | Profile (opcional) |
| `label` | string | Branch/tag (opcional) |

**Response (200 OK):**

```json
{
  "cleared": 15,
  "message": "Cache cleared successfully"
}
```

**Ejemplos:**

```bash
# Limpiar todo el cache
curl -X DELETE http://localhost:8888/cache

# Limpiar cache de una app
curl -X DELETE http://localhost:8888/cache/myapp

# Limpiar cache de app + profile
curl -X DELETE http://localhost:8888/cache/myapp/dev

# Limpiar cache específico
curl -X DELETE http://localhost:8888/cache/myapp/dev/main
```

---

### Get Metrics

Obtener métricas en formato Prometheus.

```http
GET /metrics
```

**Response (200 OK):**

```prometheus
# HELP vortex_cache_hits_total Cache hit count
# TYPE vortex_cache_hits_total counter
vortex_cache_hits_total 1234

# HELP vortex_cache_misses_total Cache miss count
# TYPE vortex_cache_misses_total counter
vortex_cache_misses_total 56

# HELP vortex_cache_evictions_total Cache eviction count
# TYPE vortex_cache_evictions_total counter
vortex_cache_evictions_total 12

# HELP vortex_cache_size Current cache size
# TYPE vortex_cache_size gauge
vortex_cache_size 45

# HELP vortex_http_requests_total HTTP request count
# TYPE vortex_http_requests_total counter
vortex_http_requests_total{method="GET",status="200"} 890
vortex_http_requests_total{method="GET",status="404"} 10
```

---

## Formatos de Respuesta

### JSON (Default)

```bash
curl http://localhost:8888/myapp/dev
```

```json
{
  "name": "myapp",
  "profiles": ["dev"],
  "propertySources": [...]
}
```

### YAML

```bash
curl -H "Accept: application/x-yaml" http://localhost:8888/myapp/dev
```

```yaml
name: myapp
profiles:
  - dev
label: main
version: abc123
propertySources:
  - name: git:main:myapp-dev.yml
    source:
      server.port: 8081
```

### Properties

```bash
curl -H "Accept: text/plain" http://localhost:8888/myapp/dev
```

```properties
server.port=8081
database.url=jdbc:postgresql://localhost:5432/myapp
logging.level=DEBUG
```

---

## Ejemplos de Uso

### Ejemplo 1: Obtener configuración de desarrollo

```bash
curl http://localhost:8888/myapp/dev | jq
```

### Ejemplo 2: Múltiples profiles

```bash
curl http://localhost:8888/myapp/dev,local | jq
```

Orden de prioridad: `local` > `dev` > base

### Ejemplo 3: Branch específico

```bash
# Tag
curl http://localhost:8888/myapp/prod/v1.0.0

# Branch
curl http://localhost:8888/myapp/dev/develop

# Feature branch (URL encoded)
curl http://localhost:8888/myapp/dev/feature%2Fnew-cache
```

### Ejemplo 4: Format en query parameter

```bash
curl "http://localhost:8888/myapp/dev?format=yaml"
curl "http://localhost:8888/myapp/dev?format=properties"
```

### Ejemplo 5: Con custom request ID

```bash
curl -H "X-Request-Id: my-custom-id-123" \
     http://localhost:8888/myapp/dev
```

El response incluirá el mismo `X-Request-Id`.

---

## Rate Limiting

Actualmente no hay rate limiting implementado. En producción, se recomienda usar un API Gateway (Nginx, Kong, etc.) para rate limiting.

---

## Authentication

Actualmente no hay autenticación en el servidor (solo autenticación Git). Para securing the API, se recomienda:

1. **Kubernetes Network Policies** - Restringir acceso a pods específicos
2. **Service Mesh (Istio/Linkerd)** - mTLS entre servicios
3. **API Gateway** - Autenticación/autorización centralizada

---

## Errores Comunes

### 404 Not Found

**Causa:** No se encontró configuración para la aplicación/profile solicitado

**Solución:**
1. Verificar que los archivos existan en el repositorio Git
2. Verificar el nombre de la aplicación
3. Verificar el formato de los archivos (YAML/JSON/Properties)

### 500 Internal Server Error

**Causa:** Error al acceder al repositorio Git o parsear configuración

**Solución:**
1. Verificar logs del servidor
2. Verificar conectividad al repositorio Git
3. Verificar sintaxis de archivos YAML/JSON

### 503 Service Unavailable

**Causa:** Servidor no puede acceder al backend (Git down, network issue)

**Solución:**
1. Verificar conectividad de red
2. Verificar que el repositorio Git esté disponible
3. Revisar configuración de GIT_URI

---

## Response Schema

### ConfigResponse

```typescript
interface ConfigResponse {
  name: string;              // Nombre de la aplicación
  profiles: string[];        // Lista de profiles
  label: string;             // Branch/tag usado
  version?: string;          // Commit hash (Git)
  state?: string;            // Estado adicional (opcional)
  propertySources: PropertySource[];
}
```

### PropertySource

```typescript
interface PropertySource {
  name: string;              // Nombre del source (ej: "git:main:app.yml")
  source: Record<string, any>; // Mapa de propiedades
}
```

---

## Postman Collection

Importar la colección de Postman para probar la API:

```bash
docs/vortex-config.postman_collection.json
```

---

## OpenAPI Specification

La especificación OpenAPI 3.0 estará disponible en futuras versiones:

```
GET /api-docs
GET /swagger-ui
```

---

## Próximos Pasos

- **[Configuration](Configuration.md)** - Configurar el servidor
- **[Getting Started](Getting-Started.md)** - Guía de inicio
- **[Deployment](Deployment.md)** - Despliegue en producción
