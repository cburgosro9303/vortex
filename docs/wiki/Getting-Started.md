# Getting Started

Esta guía te ayudará a instalar y usar Vortex Config en minutos.

## Prerequisitos

- **Docker** (recomendado) o **Rust 1.92+**
- Un repositorio Git con archivos de configuración (o usar uno de prueba)

---

## Opción 1: Docker (Recomendado)

### 1. Ejecutar con Docker

```bash
docker run -d \
  -p 8888:8888 \
  -e GIT_URI=https://github.com/spring-cloud-samples/config-repo.git \
  -e GIT_DEFAULT_LABEL=main \
  -e RUST_LOG=info \
  --name vortex-config \
  vortex-config:latest
```

### 2. Verificar que esté corriendo

```bash
# Health check
curl http://localhost:8888/health
# {"status":"UP"}
```

### 3. Obtener tu primera configuración

```bash
curl http://localhost:8888/foo/dev
```

**Respuesta esperada:**

```json
{
  "name": "foo",
  "profiles": ["dev"],
  "label": "main",
  "version": "abc123",
  "propertySources": [
    {
      "name": "git:main:foo-dev.yml",
      "source": {
        "property1": "value1"
      }
    }
  ]
}
```

---

## Opción 2: Docker Compose

### 1. Crear docker-compose.yml

```yaml
version: '3.8'

services:
  vortex-config:
    image: vortex-config:latest
    ports:
      - "8888:8888"
    environment:
      - GIT_URI=https://github.com/your-org/config-repo.git
      - GIT_DEFAULT_LABEL=main
      - GIT_USERNAME=${GIT_USERNAME}
      - GIT_PASSWORD=${GIT_PASSWORD}
      - VORTEX_CACHE_ENABLED=true
      - VORTEX_CACHE_TTL_SECONDS=300
      - RUST_LOG=info
    volumes:
      - vortex-repos:/var/lib/vortex/repos
    restart: unless-stopped
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8888/health"]
      interval: 30s
      timeout: 10s
      retries: 3

volumes:
  vortex-repos:
```

### 2. Ejecutar

```bash
docker-compose up -d
```

### 3. Ver logs

```bash
docker-compose logs -f vortex-config
```

---

## Opción 3: Compilar desde Fuentes

### 1. Clonar el repositorio

```bash
git clone https://github.com/cburgosro9303/vortex-config.git
cd vortex-config
```

### 2. Compilar el proyecto

```bash
# Debug build (más rápido, menos optimizado)
cargo build --workspace

# Release build (optimizado)
cargo build --workspace --release
```

### 3. Ejecutar tests (opcional)

```bash
cargo test --workspace
```

### 4. Ejecutar el servidor

```bash
# Configurar variables de entorno
export GIT_URI=https://github.com/spring-cloud-samples/config-repo.git
export GIT_DEFAULT_LABEL=main
export RUST_LOG=info

# Ejecutar
cargo run --bin vortex-server
```

---

## Estructura del Repositorio de Configuración

Vortex Config sigue las convenciones de Spring Cloud Config para la estructura de archivos:

```
config-repo/
├── application.yml              # Configuración base para todas las apps
├── application-dev.yml          # Configuración dev para todas las apps
├── application-prod.yml         # Configuración prod para todas las apps
├── myapp.yml                    # Configuración específica de 'myapp'
├── myapp-dev.yml                # Configuración 'myapp' en dev
├── myapp-prod.yml               # Configuración 'myapp' en prod
└── other-service.yml            # Otra aplicación
```

### Prioridad de Archivos

Cuando se solicita `GET /myapp/dev`, los archivos se cargan en este orden (de menor a mayor prioridad):

1. `application.yml`
2. `application-dev.yml`
3. `myapp.yml`
4. `myapp-dev.yml`

Los valores de archivos más específicos sobrescriben los más genéricos.

---

## Ejemplo: Crear tu Primer Repositorio

### 1. Crear repositorio Git

```bash
mkdir config-repo
cd config-repo
git init
```

### 2. Crear archivo de configuración base

```yaml
# application.yml
server:
  port: 8080

logging:
  level: INFO
```

### 3. Crear configuración específica para 'myapp'

```yaml
# myapp-dev.yml
server:
  port: 8081

database:
  url: jdbc:postgresql://localhost:5432/myapp_dev
  username: dev_user
  password: dev_pass

logging:
  level: DEBUG
```

```yaml
# myapp-prod.yml
server:
  port: 8080

database:
  url: jdbc:postgresql://prod-db:5432/myapp
  username: ${DB_USER}
  password: ${DB_PASSWORD}

logging:
  level: WARN
```

### 4. Commit y push

```bash
git add .
git commit -m "Initial configuration"
git remote add origin https://github.com/your-user/config-repo.git
git push -u origin main
```

### 5. Configurar Vortex Config

```bash
docker run -d \
  -p 8888:8888 \
  -e GIT_URI=https://github.com/your-user/config-repo.git \
  -e GIT_DEFAULT_LABEL=main \
  --name vortex-config \
  vortex-config:latest
```

### 6. Obtener configuración

```bash
# Entorno dev
curl http://localhost:8888/myapp/dev | jq

# Entorno prod
curl http://localhost:8888/myapp/prod | jq
```

---

## Integración con Spring Boot

### 1. Agregar dependencia

```xml
<!-- pom.xml -->
<dependency>
    <groupId>org.springframework.cloud</groupId>
    <artifactId>spring-cloud-starter-config</artifactId>
</dependency>
```

### 2. Configurar bootstrap.yml

```yaml
# src/main/resources/bootstrap.yml
spring:
  application:
    name: myapp
  cloud:
    config:
      uri: http://vortex-config:8888
      profile: ${ENVIRONMENT:dev}
      label: main
      fail-fast: true
      retry:
        max-attempts: 6
        initial-interval: 1000
```

### 3. Usar propiedades

```java
@RestController
public class MyController {

    @Value("${database.url}")
    private String databaseUrl;

    @GetMapping("/config")
    public String getConfig() {
        return "Database URL: " + databaseUrl;
    }
}
```

### 4. Actualizar configuración en runtime

```bash
# Invalidar cache en Vortex
curl -X DELETE http://vortex:8888/cache/myapp/dev

# Refrescar en Spring Boot
curl -X POST http://localhost:8080/actuator/refresh
```

---

## Formatos Disponibles

### JSON (default)

```bash
curl http://localhost:8888/myapp/dev
curl -H "Accept: application/json" http://localhost:8888/myapp/dev
```

### YAML

```bash
curl -H "Accept: application/x-yaml" http://localhost:8888/myapp/dev
curl -H "Accept: text/yaml" http://localhost:8888/myapp/dev
```

### Properties

```bash
curl -H "Accept: text/plain" http://localhost:8888/myapp/dev
```

**Output:**
```
server.port=8081
database.url=jdbc:postgresql://localhost:5432/myapp_dev
logging.level=DEBUG
```

---

## Comandos Útiles

### Health Check

```bash
curl http://localhost:8888/health
```

### Métricas Prometheus

```bash
curl http://localhost:8888/metrics
```

### Obtener configuración con branch específico

```bash
# Branch main
curl http://localhost:8888/myapp/dev/main

# Tag específico
curl http://localhost:8888/myapp/prod/v1.0.0

# Feature branch (URL encoded)
curl http://localhost:8888/myapp/dev/feature%2Fnew-feature
```

### Invalidar Cache

```bash
# Invalidar todo el cache
curl -X DELETE http://localhost:8888/cache

# Invalidar por app
curl -X DELETE http://localhost:8888/cache/myapp

# Invalidar por app + profile
curl -X DELETE http://localhost:8888/cache/myapp/dev

# Invalidar específico
curl -X DELETE http://localhost:8888/cache/myapp/dev/main
```

---

## Troubleshooting

### El servidor no inicia

**Problema:** Error de conexión al repositorio Git

**Solución:**
```bash
# Verificar que GIT_URI esté correctamente configurado
docker logs vortex-config

# Verificar conectividad al repo
git ls-remote https://github.com/your-user/config-repo.git
```

### No encuentra la configuración

**Problema:** `404 Not Found` al solicitar configuración

**Solución:**
1. Verificar que los archivos existan en el repositorio
2. Verificar el nombre del archivo (`{app}.yml` o `{app}-{profile}.yml`)
3. Verificar el branch/label correcto

```bash
# Listar archivos en el repo
git clone https://github.com/your-user/config-repo.git
cd config-repo
ls -la
```

### Cache desactualizado

**Problema:** Los cambios en Git no se reflejan

**Solución:**
```bash
# Opción 1: Esperar el refresh automático (default: 30s)

# Opción 2: Invalidar cache manualmente
curl -X DELETE http://localhost:8888/cache/myapp/dev

# Opción 3: Reiniciar el servidor
docker restart vortex-config
```

---

## Próximos Pasos

- **[Configuration](Configuration.md)** - Configuración avanzada del servidor
- **[API Reference](API-Reference.md)** - Documentación completa de la API
- **[Deployment](Deployment.md)** - Despliegue en producción (Kubernetes)
- **[Architecture](Architecture.md)** - Entender la arquitectura interna

---

## Ejemplos Completos

Ver repositorio de ejemplos:
- [spring-cloud-samples/config-repo](https://github.com/spring-cloud-samples/config-repo) - Repositorio de configuración de ejemplo
- [vortex-config-examples](docs/examples/) - Ejemplos de integración
