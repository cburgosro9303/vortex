# Historia 005: Middleware de Logging y RequestId

## Contexto y Objetivo

El middleware de logging y request ID es fundamental para la observabilidad en produccion. Cada request debe tener un identificador unico que permita rastrear su flujo a traves del sistema, y todos los logs deben incluir este identificador.

**Beneficios:**
- **Debugging**: Correlacionar logs de un mismo request
- **Tracing distribuido**: Propagar ID entre servicios
- **Auditoria**: Rastrear quien hizo que y cuando
- **Metricas**: Medir latencia, errores, throughput

Esta historia implementa dos capas de Tower middleware:
1. **RequestId Layer**: Genera o propaga `X-Request-Id`
2. **Logging Layer**: Registra request/response con contexto

---

## Alcance

### In Scope

- Middleware que genera UUID v4 para cada request
- Propagacion de `X-Request-Id` si viene en el request
- Logging estructurado de requests (method, path, status, duration)
- Header `X-Request-Id` en todas las respuestas
- Integracion con `tracing` para spans

### Out of Scope

- Tracing distribuido completo (OpenTelemetry)
- Metricas Prometheus
- Rate limiting
- CORS
- Compression

---

## Criterios de Aceptacion

- [ ] Cada respuesta incluye header `X-Request-Id`
- [ ] Si request incluye `X-Request-Id`, se reutiliza
- [ ] Si no incluye, se genera UUID v4
- [ ] Logs incluyen request_id, method, path, status, duration
- [ ] Logs son estructurados (JSON-friendly)
- [ ] El span de tracing incluye request_id como campo

---

## Diseno Propuesto

### Arquitectura de Middleware

```
Request entrante
       │
       ▼
┌──────────────────┐
│  RequestId       │  ← Genera/propaga X-Request-Id
│  Layer           │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  Logging         │  ← Inicia span, mide duracion
│  Layer           │
└────────┬─────────┘
         │
         ▼
┌──────────────────┐
│  Axum Router     │  ← Handlers de negocio
└────────┬─────────┘
         │
         ▼
  Response saliente
  (con X-Request-Id)
```

### Estructura de Modulos

```
crates/vortex-server/src/
├── middleware/
│   ├── mod.rs           # Re-exports
│   ├── request_id.rs    # RequestId layer
│   └── logging.rs       # Logging layer
└── server.rs            # Aplicacion de layers
```

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencias

```toml
# Cargo.toml
[dependencies]
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "request-id", "propagate-header"] }
uuid = { version = "1", features = ["v4"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }
```

### Paso 2: Implementar RequestId Middleware

```rust
// src/middleware/request_id.rs
use axum::{
    http::{header::HeaderName, HeaderValue, Request, Response},
    body::Body,
};
use std::task::{Context, Poll};
use tower::{Layer, Service};
use uuid::Uuid;

/// Header name for request ID.
pub static REQUEST_ID_HEADER: HeaderName = HeaderName::from_static("x-request-id");

/// Layer that adds request ID to requests and responses.
#[derive(Clone, Default)]
pub struct RequestIdLayer;

impl<S> Layer<S> for RequestIdLayer {
    type Service = RequestIdMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        RequestIdMiddleware { inner }
    }
}

/// Middleware that ensures every request has a unique ID.
#[derive(Clone)]
pub struct RequestIdMiddleware<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for RequestIdMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, mut request: Request<Body>) -> Self::Future {
        // Get existing request ID or generate new one
        let request_id = request
            .headers()
            .get(&REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .map(String::from)
            .unwrap_or_else(|| Uuid::new_v4().to_string());

        // Add request ID to request headers (for handlers to access)
        request.headers_mut().insert(
            REQUEST_ID_HEADER.clone(),
            HeaderValue::from_str(&request_id).unwrap(),
        );

        // Store request ID for response
        let request_id_for_response = request_id.clone();

        let mut inner = self.inner.clone();

        Box::pin(async move {
            let mut response = inner.call(request).await?;

            // Add request ID to response headers
            response.headers_mut().insert(
                REQUEST_ID_HEADER.clone(),
                HeaderValue::from_str(&request_id_for_response).unwrap(),
            );

            Ok(response)
        })
    }
}
```

### Paso 3: Implementar Logging Middleware

```rust
// src/middleware/logging.rs
use axum::{
    http::{Method, Request, Response, Uri},
    body::Body,
};
use std::{
    task::{Context, Poll},
    time::Instant,
};
use tower::{Layer, Service};
use tracing::{info, info_span, Instrument};

use super::request_id::REQUEST_ID_HEADER;

/// Layer that logs requests and responses.
#[derive(Clone, Default)]
pub struct LoggingLayer;

impl<S> Layer<S> for LoggingLayer {
    type Service = LoggingMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        LoggingMiddleware { inner }
    }
}

/// Middleware that logs request/response details.
#[derive(Clone)]
pub struct LoggingMiddleware<S> {
    inner: S,
}

impl<S> Service<Request<Body>> for LoggingMiddleware<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = std::pin::Pin<
        Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>
    >;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let start = Instant::now();
        let method = request.method().clone();
        let uri = request.uri().clone();
        let path = uri.path().to_string();

        // Extract request ID (should have been set by RequestIdMiddleware)
        let request_id = request
            .headers()
            .get(&REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        // Create span with request context
        let span = info_span!(
            "http_request",
            request_id = %request_id,
            method = %method,
            path = %path,
        );

        let mut inner = self.inner.clone();

        Box::pin(
            async move {
                info!("Request started");

                let response = inner.call(request).await?;

                let status = response.status().as_u16();
                let duration = start.elapsed();

                info!(
                    status = status,
                    duration_ms = duration.as_millis() as u64,
                    "Request completed"
                );

                Ok(response)
            }
            .instrument(span)
        )
    }
}
```

### Paso 4: Crear Modulo de Middleware

```rust
// src/middleware/mod.rs
mod request_id;
mod logging;

pub use request_id::{RequestIdLayer, RequestIdMiddleware, REQUEST_ID_HEADER};
pub use logging::{LoggingLayer, LoggingMiddleware};

use tower::ServiceBuilder;

/// Crea el stack de middleware estandar.
///
/// El orden es importante:
/// 1. RequestId (primero, para que logging tenga el ID)
/// 2. Logging (segundo, para medir duracion total)
pub fn create_middleware_stack() -> tower::ServiceBuilder<
    tower::layer::util::Stack<LoggingLayer, tower::layer::util::Stack<RequestIdLayer, tower::layer::util::Identity>>
> {
    ServiceBuilder::new()
        .layer(RequestIdLayer)
        .layer(LoggingLayer)
}
```

### Paso 5: Aplicar Middleware al Router

```rust
// src/server.rs
use axum::Router;
use crate::middleware::{RequestIdLayer, LoggingLayer};
use tower::ServiceBuilder;

pub fn create_router() -> Router {
    let middleware = ServiceBuilder::new()
        .layer(RequestIdLayer)
        .layer(LoggingLayer);

    Router::new()
        .route("/health", get(health_check))
        .route("/:app/:profile/:label", get(get_config_with_label))
        .route("/:app/:profile", get(get_config))
        .layer(middleware)
}
```

### Paso 6: Configurar Tracing Subscriber

```rust
// src/lib.rs o main.rs
use tracing_subscriber::{
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
    EnvFilter,
};

pub fn init_tracing() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info,tower_http=debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().json())  // JSON format para produccion
        .init();
}

// Para desarrollo, usar formato legible:
pub fn init_tracing_dev() {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("debug"));

    tracing_subscriber::registry()
        .with(filter)
        .with(fmt::layer().pretty())
        .init();
}
```

### Paso 7: Extractor para Request ID en Handlers

```rust
// src/extractors/request_id.rs
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};

use crate::middleware::REQUEST_ID_HEADER;

/// Extractor para obtener el request ID en handlers.
pub struct RequestId(pub String);

#[async_trait]
impl<S> FromRequestParts<S> for RequestId
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let id = parts
            .headers
            .get(&REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("unknown")
            .to_string();

        Ok(RequestId(id))
    }
}

// Uso en handlers
async fn my_handler(RequestId(id): RequestId) -> String {
    format!("Your request ID is: {}", id)
}
```

---

## Conceptos de Rust Aprendidos

### 1. Tower Middleware y Service Trait

Tower es el estandar de facto para middleware en el ecosistema async de Rust.

**Rust:**
```rust
use tower::{Layer, Service};
use std::task::{Context, Poll};

// Un Layer es una fabrica de Services
pub struct MyLayer;

impl<S> Layer<S> for MyLayer {
    type Service = MyMiddleware<S>;

    fn layer(&self, inner: S) -> Self::Service {
        MyMiddleware { inner }
    }
}

// Un Service procesa requests
pub struct MyMiddleware<S> {
    inner: S,
}

impl<S, Request> Service<Request> for MyMiddleware<S>
where
    S: Service<Request>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;

    // Ready check (backpressure)
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    // Procesar request
    fn call(&mut self, request: Request) -> Self::Future {
        // Pre-processing
        self.inner.call(request)
        // Post-processing en el Future
    }
}
```

**Comparacion con Spring Interceptors:**
```java
// Spring HandlerInterceptor
public class LoggingInterceptor implements HandlerInterceptor {

    @Override
    public boolean preHandle(
            HttpServletRequest request,
            HttpServletResponse response,
            Object handler) {
        // Pre-processing
        return true;
    }

    @Override
    public void afterCompletion(
            HttpServletRequest request,
            HttpServletResponse response,
            Object handler,
            Exception ex) {
        // Post-processing
    }
}

// Registro en WebMvcConfigurer
@Override
public void addInterceptors(InterceptorRegistry registry) {
    registry.addInterceptor(new LoggingInterceptor());
}
```

**Diferencias clave:**
| Aspecto | Tower (Rust) | Spring Interceptor |
|---------|--------------|-------------------|
| Modelo | Funcional (Service trait) | OOP (Interface) |
| Async | Nativo | Blocking o WebFlux |
| Composicion | `layer(a).layer(b)` | `registry.addInterceptor()` |
| Backpressure | `poll_ready` | No integrado |
| Type safety | Compile-time | Runtime |

### 2. Pin y Async Futures

Cuando retornamos Futures desde closures, necesitamos `Pin<Box<dyn Future>>`.

**Rust:**
```rust
use std::pin::Pin;
use std::future::Future;

impl<S> Service<Request<Body>> for MyMiddleware<S>
where
    S: Service<Request<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    // El Future debe ser:
    // - Pin: Para garantizar que no se mueve en memoria
    // - Box: Para type erasure (no sabemos el tipo concreto)
    // - dyn Future: Trait object
    // - Send: Puede moverse entre threads
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let mut inner = self.inner.clone();

        // Box::pin crea el Pin<Box<...>>
        Box::pin(async move {
            // async block que captura `inner`
            inner.call(request).await
        })
    }
}
```

**Por que Pin?**
Los Futures en Rust pueden tener auto-referencias (un campo que apunta a otro campo del mismo struct). Si el Future se mueve en memoria, estas referencias quedarian invalidas. `Pin` garantiza que el Future no se movera.

**Comparacion conceptual con Java:**
```java
// Java CompletableFuture no tiene este problema
// porque la JVM maneja referencias con GC
CompletableFuture<Response> future = CompletableFuture.supplyAsync(() -> {
    // Este closure puede capturar variables libremente
    return processRequest(request);
});
```

### 3. Instrument Trait de Tracing

El trait `Instrument` permite adjuntar spans a Futures.

**Rust:**
```rust
use tracing::{info_span, Instrument};

async fn my_async_function() {
    // Crear span
    let span = info_span!("my_operation", key = "value");

    // Ejecutar future dentro del span
    async {
        // Todo log aqui incluira el span context
        tracing::info!("Doing work");
    }
    .instrument(span)
    .await;
}

// En middleware
fn call(&mut self, request: Request<Body>) -> Self::Future {
    let span = info_span!(
        "http_request",
        method = %request.method(),
        path = %request.uri().path(),
    );

    Box::pin(
        async move {
            // Logs dentro de este async block incluyen el span
            tracing::info!("Processing");
            let response = inner.call(request).await?;
            tracing::info!(status = %response.status(), "Done");
            Ok(response)
        }
        .instrument(span)  // <-- Adjunta el span al Future
    )
}
```

**Comparacion con SLF4J MDC:**
```java
// Java usa MDC (Mapped Diagnostic Context)
MDC.put("requestId", requestId);
try {
    logger.info("Processing request");
    Response response = processRequest(request);
    logger.info("Request completed");
    return response;
} finally {
    MDC.remove("requestId");
}
```

**Diferencias:**
- Rust: `Instrument` es type-safe y se integra con async
- Java: MDC es thread-local, problemas con async
- Rust: Spans son jerarquicos (parent-child)
- Java: MDC es plano (key-value)

### 4. Clone + Send + 'static Bounds

Los bounds comunes en middleware async.

**Rust:**
```rust
impl<S> Service<Request<Body>> for MyMiddleware<S>
where
    S: Service<Request<Body>> + Clone + Send + 'static,
    //                           ^^^^^   ^^^^   ^^^^^^^
    //                           |       |      |
    //                           |       |      No referencias temporales
    //                           |       Puede enviarse entre threads
    //                           Puede clonarse (para mover a async block)
    S::Future: Send + 'static,
    //         ^^^^   ^^^^^^^
    //         |      |
    //         |      El Future no contiene referencias locales
    //         El Future puede ejecutarse en cualquier thread
{
    fn call(&mut self, request: Request<Body>) -> Self::Future {
        // Clonamos porque movemos `inner` al async block
        let mut inner = self.inner.clone();

        Box::pin(async move {
            // `inner` se movio aqui (requiere Clone)
            // El async block puede ejecutarse en otro thread (requiere Send)
            inner.call(request).await
        })
    }
}
```

**Por que necesitamos Clone?**
El `&mut self` en `call` no se puede mover al async block. Clonamos el service para tener una copia que podemos mover.

---

## Riesgos y Errores Comunes

### 1. Orden Incorrecto de Layers

```rust
// MAL: Logging antes de RequestId (no tendra el ID)
Router::new()
    .layer(LoggingLayer)    // No tiene request_id aun!
    .layer(RequestIdLayer)

// BIEN: RequestId primero (layers se aplican de abajo hacia arriba)
Router::new()
    .layer(RequestIdLayer)  // Se aplica primero al request
    .layer(LoggingLayer)    // Tiene acceso al request_id

// O usando ServiceBuilder (orden intuitivo)
ServiceBuilder::new()
    .layer(RequestIdLayer)  // Primero
    .layer(LoggingLayer)    // Segundo
```

### 2. No Propagar Request ID

```rust
// MAL: Generar siempre nuevo ID
let request_id = Uuid::new_v4().to_string();

// BIEN: Reutilizar si existe
let request_id = request
    .headers()
    .get(&REQUEST_ID_HEADER)
    .and_then(|v| v.to_str().ok())
    .map(String::from)
    .unwrap_or_else(|| Uuid::new_v4().to_string());
```

### 3. Bloquear en Middleware

```rust
// MAL: Operacion bloqueante
fn call(&mut self, request: Request<Body>) -> Self::Future {
    std::thread::sleep(Duration::from_millis(100)); // NUNCA!

    Box::pin(async move {
        // ...
    })
}

// BIEN: Usar async
fn call(&mut self, request: Request<Body>) -> Self::Future {
    Box::pin(async move {
        tokio::time::sleep(Duration::from_millis(100)).await;
        // ...
    })
}
```

### 4. Memory Leak con Spans

```rust
// MAL: Span nunca se cierra
let span = info_span!("request");
let _guard = span.enter();  // Mantiene span abierto
// Si el handler hace panic, el span queda abierto

// BIEN: Usar .instrument() que maneja el ciclo de vida
Box::pin(
    async move { ... }
        .instrument(span)  // Span se cierra cuando Future completa
)
```

---

## Pruebas

### Tests del RequestId Middleware

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use axum::{body::Body, http::Request};
    use tower::{ServiceBuilder, ServiceExt};

    async fn echo_request_id(request: Request<Body>) -> Response<Body> {
        let id = request
            .headers()
            .get(&REQUEST_ID_HEADER)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("none");

        Response::new(Body::from(id.to_string()))
    }

    #[tokio::test]
    async fn generates_request_id_when_missing() {
        let service = ServiceBuilder::new()
            .layer(RequestIdLayer)
            .service_fn(echo_request_id);

        let request = Request::builder()
            .uri("/test")
            .body(Body::empty())
            .unwrap();

        let response = service.oneshot(request).await.unwrap();

        // Response should have X-Request-Id header
        let id = response.headers().get(&REQUEST_ID_HEADER);
        assert!(id.is_some());

        // Should be valid UUID
        let id_str = id.unwrap().to_str().unwrap();
        assert!(uuid::Uuid::parse_str(id_str).is_ok());
    }

    #[tokio::test]
    async fn propagates_existing_request_id() {
        let service = ServiceBuilder::new()
            .layer(RequestIdLayer)
            .service_fn(echo_request_id);

        let request = Request::builder()
            .uri("/test")
            .header("X-Request-Id", "my-custom-id")
            .body(Body::empty())
            .unwrap();

        let response = service.oneshot(request).await.unwrap();

        let id = response
            .headers()
            .get(&REQUEST_ID_HEADER)
            .unwrap()
            .to_str()
            .unwrap();

        assert_eq!(id, "my-custom-id");
    }
}
```

### Tests de Integracion

```rust
// tests/middleware_test.rs
use axum::{body::Body, http::Request};
use tower::ServiceExt;
use vortex_server::create_router;

#[tokio::test]
async fn response_includes_request_id_header() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert!(response.headers().contains_key("x-request-id"));
}

#[tokio::test]
async fn propagates_incoming_request_id() {
    let app = create_router();
    let custom_id = "test-request-123";

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .header("x-request-id", custom_id)
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let returned_id = response
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap();

    assert_eq!(returned_id, custom_id);
}

#[tokio::test]
async fn generates_uuid_when_no_request_id() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/health")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let id = response
        .headers()
        .get("x-request-id")
        .unwrap()
        .to_str()
        .unwrap();

    // Verify it's a valid UUID v4
    let parsed = uuid::Uuid::parse_str(id);
    assert!(parsed.is_ok());
    assert_eq!(parsed.unwrap().get_version_num(), 4);
}
```

### Tests del Logging (Captura de Logs)

```rust
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::util::SubscriberInitExt;

#[tokio::test]
async fn logs_request_details() {
    // Capturar logs en test
    let (tx, rx) = std::sync::mpsc::channel();

    let subscriber = tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer()
            .with_writer(move || TestWriter(tx.clone())));

    let _guard = tracing::subscriber::set_default(subscriber);

    let app = create_router();

    let _ = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
                .body(Body::empty())
                .unwrap()
        )
        .await;

    // Verificar que se logueo
    let logs: Vec<_> = rx.try_iter().collect();
    let log_output = logs.join("");

    assert!(log_output.contains("http_request"));
    assert!(log_output.contains("/myapp/dev"));
}
```

---

## Observabilidad

### Formato de Logs (Produccion - JSON)

```json
{
  "timestamp": "2024-01-15T10:30:45.123Z",
  "level": "INFO",
  "target": "vortex_server::middleware::logging",
  "span": {
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "method": "GET",
    "path": "/myapp/dev"
  },
  "message": "Request started"
}

{
  "timestamp": "2024-01-15T10:30:45.125Z",
  "level": "INFO",
  "target": "vortex_server::middleware::logging",
  "span": {
    "request_id": "550e8400-e29b-41d4-a716-446655440000",
    "method": "GET",
    "path": "/myapp/dev"
  },
  "fields": {
    "status": 200,
    "duration_ms": 2
  },
  "message": "Request completed"
}
```

### Formato de Logs (Desarrollo - Pretty)

```
  2024-01-15T10:30:45.123Z  INFO http_request{request_id=550e8400... method=GET path=/myapp/dev}: Request started
  2024-01-15T10:30:45.125Z  INFO http_request{request_id=550e8400... method=GET path=/myapp/dev}: Request completed status=200 duration_ms=2
```

### Metricas Expuestas (Futuro)

```rust
// Preparacion para Prometheus
use metrics::{counter, histogram};

// En logging middleware
histogram!("http_request_duration_seconds", duration.as_secs_f64());
counter!("http_requests_total", 1, "method" => method, "status" => status);
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `Cargo.toml` - Agregar tower, tower-http, uuid, tracing-subscriber
2. `src/middleware/mod.rs` - NUEVO: Modulo de middleware
3. `src/middleware/request_id.rs` - NUEVO: RequestId layer
4. `src/middleware/logging.rs` - NUEVO: Logging layer
5. `src/extractors/request_id.rs` - NUEVO: RequestId extractor
6. `src/server.rs` - Aplicar middleware stack
7. `src/lib.rs` - Funcion init_tracing
8. `tests/middleware_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server

# Ejecutar con logging
RUST_LOG=debug cargo run -p vortex-server

# Verificar headers
curl -v http://localhost:8080/health 2>&1 | grep -i x-request-id
# < x-request-id: 550e8400-e29b-41d4-a716-446655440000

# Propagar request ID
curl -H "X-Request-Id: my-custom-id" http://localhost:8080/health -v 2>&1 | grep x-request-id
# < x-request-id: my-custom-id

# Ver logs estructurados
RUST_LOG=info cargo run -p vortex-server 2>&1 | jq '.'
```

### Ejemplo de Log Output

```bash
$ curl http://localhost:8080/myapp/dev

# En el servidor:
INFO http_request{request_id=abc123 method=GET path=/myapp/dev}: Request started
INFO http_request{request_id=abc123 method=GET path=/myapp/dev}: Fetching config
INFO http_request{request_id=abc123 method=GET path=/myapp/dev}: Request completed status=200 duration_ms=3
```
