# Configuration Guide

Guía completa de configuración de Vortex Config Server.

## Variables de Entorno

### Servidor HTTP

| Variable | Default | Descripción |
|----------|---------|-------------|
| `VORTEX_HOST` | `0.0.0.0` | Host donde escucha el servidor |
| `VORTEX_PORT` | `8888` | Puerto del servidor HTTP |

### Git Backend

| Variable | Default | Descripción |
|----------|---------|-------------|
| `GIT_URI` | *requerido* | URL del repositorio Git |
| `GIT_LOCAL_PATH` | `/var/lib/vortex/repos` | Path local para clonar repos |
| `GIT_DEFAULT_LABEL` | `main` | Branch por defecto |
| `GIT_SEARCH_PATHS` | `` | Paths de búsqueda (CSV) |
| `GIT_USERNAME` | `` | Usuario para autenticación |
| `GIT_PASSWORD` | `` | Password/token para autenticación |
| `GIT_CLONE_TIMEOUT_SECS` | `120` | Timeout para clone |
| `GIT_FETCH_TIMEOUT_SECS` | `30` | Timeout para fetch |
| `GIT_FORCE_PULL` | `false` | Forzar pull en repo existente |

### Cache

| Variable | Default | Descripción |
|----------|---------|-------------|
| `VORTEX_CACHE_ENABLED` | `true` | Activar/desactivar cache |
| `VORTEX_CACHE_TTL_SECONDS` | `300` | TTL del cache (5 minutos) |
| `VORTEX_CACHE_MAX_CAPACITY` | `10000` | Capacidad máxima (entries) |
| `VORTEX_CACHE_TTI_SECONDS` | `` | Time-to-idle (opcional) |

### Git Refresh

| Variable | Default | Descripción |
|----------|---------|-------------|
| `GIT_REFRESH_ENABLED` | `true` | Activar refresh automático |
| `GIT_REFRESH_INTERVAL_SECS` | `30` | Intervalo de refresh |
| `GIT_REFRESH_MAX_FAILURES` | `3` | Fallos antes de backoff |
| `GIT_REFRESH_BACKOFF_MULTIPLIER` | `2.0` | Multiplicador de backoff |
| `GIT_REFRESH_MAX_BACKOFF_SECS` | `300` | Máximo backoff |

### Logging

| Variable | Default | Descripción |
|----------|---------|-------------|
| `RUST_LOG` | `info` | Nivel de logging (`error`, `warn`, `info`, `debug`, `trace`) |
| `RUST_LOG_FORMAT` | `json` | Formato de logs (`json` o `pretty`) |

---

## Archivo de Configuración (YAML)

Alternativamente, puedes usar un archivo YAML:

```yaml
# config/default.yaml

server:
  host: "0.0.0.0"
  port: 8888

git:
  uri: "https://github.com/your-org/config-repo.git"
  local_path: "/var/lib/vortex/repos"
  default_label: "main"
  search_paths:
    - "config"
    - "application"
  username: ""
  password: ""
  clone_timeout_secs: 120
  fetch_timeout_secs: 30
  force_pull: false

cache:
  enabled: true
  ttl_seconds: 300
  max_capacity: 10000
  tti_seconds: null

refresh:
  enabled: true
  interval_secs: 30
  max_failures: 3
  backoff_multiplier: 2.0
  max_backoff_secs: 300

logging:
  level: "info"
  format: "json"
```

**Usar con Docker:**

```bash
docker run -d \
  -p 8888:8888 \
  -v $(pwd)/config:/etc/vortex/config \
  vortex-config:latest
```

---

## Configuración de Git

### Repositorios Públicos

```bash
docker run -d \
  -p 8888:8888 \
  -e GIT_URI=https://github.com/spring-cloud-samples/config-repo.git \
  vortex-config:latest
```

### Repositorios Privados (HTTPS)

```bash
docker run -d \
  -p 8888:8888 \
  -e GIT_URI=https://github.com/your-org/private-repo.git \
  -e GIT_USERNAME=your-username \
  -e GIT_PASSWORD=your-token \
  vortex-config:latest
```

**GitHub Personal Access Token:**

1. Ir a GitHub → Settings → Developer settings → Personal access tokens
2. Generar nuevo token con scopes: `repo`
3. Usar el token como `GIT_PASSWORD`

### SSH (Futuro)

Soporte para SSH keys está planificado para Epic 6.

---

## Configuración de Cache

### Cache Deshabilitado

Para debugging o desarrollo, puedes deshabilitar el cache:

```bash
docker run -d \
  -p 8888:8888 \
  -e VORTEX_CACHE_ENABLED=false \
  -e GIT_URI=... \
  vortex-config:latest
```

### Cache Agresivo (baja frecuencia de cambios)

```bash
docker run -d \
  -p 8888:8888 \
  -e VORTEX_CACHE_TTL_SECONDS=3600 \
  -e GIT_REFRESH_INTERVAL_SECS=300 \
  -e GIT_URI=... \
  vortex-config:latest
```

### Cache Conservador (alta frecuencia de cambios)

```bash
docker run -d \
  -p 8888:8888 \
  -e VORTEX_CACHE_TTL_SECONDS=30 \
  -e GIT_REFRESH_INTERVAL_SECS=10 \
  -e GIT_URI=... \
  vortex-config:latest
```

---

## Configuración de Logging

### Logging Estructurado (JSON)

```bash
export RUST_LOG=info
export RUST_LOG_FORMAT=json
```

**Output:**
```json
{"timestamp":"2026-01-12T10:30:00Z","level":"INFO","target":"vortex_server","message":"Server started","fields":{"host":"0.0.0.0","port":8888}}
```

### Logging Pretty (Desarrollo)

```bash
export RUST_LOG=debug
export RUST_LOG_FORMAT=pretty
```

**Output:**
```
2026-01-12T10:30:00Z  INFO vortex_server: Server started host=0.0.0.0 port=8888
```

### Niveles de Log Específicos

```bash
export RUST_LOG=vortex_server=debug,vortex_git=trace,vortex_core=info
```

---

## Configuración de Kubernetes

### ConfigMap

```yaml
apiVersion: v1
kind: ConfigMap
metadata:
  name: vortex-config
data:
  GIT_URI: "https://github.com/your-org/config-repo.git"
  GIT_DEFAULT_LABEL: "main"
  VORTEX_CACHE_TTL_SECONDS: "300"
  RUST_LOG: "info"
```

### Secret (Credenciales Git)

```yaml
apiVersion: v1
kind: Secret
metadata:
  name: vortex-git-credentials
type: Opaque
stringData:
  GIT_USERNAME: your-username
  GIT_PASSWORD: your-token
```

### Deployment

```yaml
apiVersion: apps/v1
kind: Deployment
metadata:
  name: vortex-config
spec:
  replicas: 2
  template:
    spec:
      containers:
      - name: vortex-config
        image: vortex-config:latest
        envFrom:
        - configMapRef:
            name: vortex-config
        env:
        - name: GIT_USERNAME
          valueFrom:
            secretKeyRef:
              name: vortex-git-credentials
              key: GIT_USERNAME
        - name: GIT_PASSWORD
          valueFrom:
            secretKeyRef:
              name: vortex-git-credentials
              key: GIT_PASSWORD
```

---

## Configuración Avanzada

### Search Paths

Buscar configuraciones en subdirectorios específicos:

```bash
export GIT_SEARCH_PATHS="config,application,services"
```

Estructura del repo:
```
config-repo/
├── config/
│   ├── myapp.yml
│   └── myapp-dev.yml
├── application/
│   └── shared.yml
└── services/
    └── payment.yml
```

### Multiple Profiles

Soporta múltiples profiles separados por coma:

```bash
curl http://localhost:8888/myapp/dev,local,debug
```

Orden de prioridad (mayor a menor): `debug` > `local` > `dev` > base

---

## Ejemplos de Configuración

### Desarrollo Local

```bash
# .env
VORTEX_HOST=127.0.0.1
VORTEX_PORT=8888
GIT_URI=file:///Users/you/repos/config-repo
GIT_DEFAULT_LABEL=develop
VORTEX_CACHE_ENABLED=false
RUST_LOG=debug

# Ejecutar
docker-compose up -d
```

### Staging

```bash
# docker-compose.staging.yml
environment:
  - VORTEX_PORT=8888
  - GIT_URI=https://github.com/your-org/config-repo.git
  - GIT_DEFAULT_LABEL=staging
  - GIT_USERNAME=${GIT_USERNAME}
  - GIT_PASSWORD=${GIT_PASSWORD}
  - VORTEX_CACHE_TTL_SECONDS=300
  - GIT_REFRESH_INTERVAL_SECS=60
  - RUST_LOG=info
```

### Producción

```bash
# Kubernetes manifest
env:
  - name: VORTEX_PORT
    value: "8888"
  - name: GIT_URI
    value: "https://github.com/your-org/config-repo.git"
  - name: GIT_DEFAULT_LABEL
    value: "main"
  - name: GIT_USERNAME
    valueFrom:
      secretKeyRef:
        name: git-credentials
        key: username
  - name: GIT_PASSWORD
    valueFrom:
      secretKeyRef:
        name: git-credentials
        key: password
  - name: VORTEX_CACHE_TTL_SECONDS
    value: "600"
  - name: GIT_REFRESH_INTERVAL_SECS
    value: "120"
  - name: RUST_LOG
    value: "warn"

resources:
  requests:
    memory: "64Mi"
    cpu: "100m"
  limits:
    memory: "256Mi"
    cpu: "500m"
```

---

## Health Checks

### Docker

```yaml
healthcheck:
  test: ["CMD", "curl", "-f", "http://localhost:8888/health"]
  interval: 30s
  timeout: 10s
  retries: 3
  start_period: 10s
```

### Kubernetes

```yaml
livenessProbe:
  httpGet:
    path: /health
    port: 8888
  initialDelaySeconds: 5
  periodSeconds: 10
  timeoutSeconds: 5
  failureThreshold: 3

readinessProbe:
  httpGet:
    path: /health
    port: 8888
  initialDelaySeconds: 3
  periodSeconds: 5
  timeoutSeconds: 3
  failureThreshold: 2
```

---

## Troubleshooting

### No puede clonar el repositorio

**Error:** `Failed to clone repository`

**Soluciones:**
1. Verificar `GIT_URI` correcta
2. Verificar credenciales (usuario/password)
3. Verificar conectividad de red
4. Aumentar `GIT_CLONE_TIMEOUT_SECS`

### Cache no se invalida

**Error:** Cambios en Git no se reflejan

**Soluciones:**
1. Verificar `GIT_REFRESH_ENABLED=true`
2. Reducir `GIT_REFRESH_INTERVAL_SECS`
3. Invalidar cache manualmente: `curl -X DELETE http://localhost:8888/cache`
4. Verificar logs: `docker logs vortex-config`

### Alto uso de memoria

**Error:** Contenedor usando mucha memoria

**Soluciones:**
1. Reducir `VORTEX_CACHE_MAX_CAPACITY`
2. Reducir `VORTEX_CACHE_TTL_SECONDS`
3. Activar `VORTEX_CACHE_TTI_SECONDS` para eviction agresiva
4. Monitorear métricas: `curl http://localhost:8888/metrics`

---

## Próximos Pasos

- **[API Reference](API-Reference.md)** - Documentación completa de la API
- **[Deployment](Deployment.md)** - Guía de deployment en producción
- **[Architecture](Architecture.md)** - Entender la arquitectura interna
