# Historia 001: Scaffold del Server Axum

## Contexto y Objetivo

Esta historia establece la base del servidor HTTP de Vortex Config. Implementaremos el scaffold minimo necesario para tener un servidor Axum funcionando con un endpoint de health check.

El health check es fundamental para:

- **Kubernetes**: Probes de liveness y readiness
- **Load balancers**: Verificacion de instancias saludables
- **Monitoring**: Verificacion basica de disponibilidad

Al completar esta historia, tendras un servidor HTTP funcional listo para agregar mas endpoints.

---

## Alcance

### In Scope

- Configuracion basica de Axum con Tokio runtime
- Endpoint `GET /health` retornando `{"status": "UP"}`
- Estructura inicial del crate `vortex-server`
- Graceful shutdown basico
- Tests unitarios del health endpoint

### Out of Scope

- Endpoints de configuracion (historia 002+)
- Middleware de logging (historia 005)
- Configuracion externa (puerto, host)
- TLS/HTTPS
- Metricas Prometheus

---

## Criterios de Aceptacion

- [ ] `GET /health` retorna status 200 con body `{"status": "UP"}`
- [ ] Content-Type de respuesta es `application/json`
- [ ] El servidor inicia en el puerto 8080 por defecto
- [ ] Graceful shutdown funciona con CTRL+C
- [ ] Tests pasan: `cargo test -p vortex-server`
- [ ] Sin warnings de clippy

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-server/
├── Cargo.toml
├── src/
│   ├── lib.rs           # Re-exports: run_server, HealthResponse
│   ├── server.rs        # Configuracion y startup del servidor
│   └── handlers/
│       ├── mod.rs       # pub mod health;
│       └── health.rs    # Handler del health check
└── tests/
    └── health_test.rs   # Tests de integracion
```

### Interfaces Principales

```rust
// src/lib.rs
pub use server::run_server;
pub use handlers::health::HealthResponse;

// src/server.rs
pub async fn run_server(addr: SocketAddr) -> Result<(), std::io::Error>;

// src/handlers/health.rs
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
}

pub async fn health_check() -> Json<HealthResponse>;
```

---

## Pasos de Implementacion

### Paso 1: Crear el Crate

```bash
# Desde la raiz del workspace
cargo new crates/vortex-server --lib
```

### Paso 2: Configurar Cargo.toml

```toml
[package]
name = "vortex-server"
version = "0.1.0"
edition = "2024"

[dependencies]
axum = "0.7"
tokio = { version = "1", features = ["full"] }
serde = { version = "1", features = ["derive"] }
serde_json = "1"
tracing = "0.1"

[dev-dependencies]
tower = { version = "0.4", features = ["util"] }
hyper = { version = "1", features = ["full"] }
http-body-util = "0.1"
```

### Paso 3: Implementar el Health Handler

```rust
// src/handlers/health.rs
use axum::Json;
use serde::Serialize;

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
}

impl Default for HealthResponse {
    fn default() -> Self {
        Self {
            status: "UP".to_string(),
        }
    }
}

pub async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse::default())
}
```

### Paso 4: Configurar el Router y Servidor

```rust
// src/server.rs
use std::net::SocketAddr;
use axum::{Router, routing::get};
use crate::handlers::health::health_check;

pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
}

pub async fn run_server(addr: SocketAddr) -> Result<(), std::io::Error> {
    let app = create_router();

    let listener = tokio::net::TcpListener::bind(addr).await?;
    tracing::info!("Server listening on {}", addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await
}

async fn shutdown_signal() {
    tokio::signal::ctrl_c()
        .await
        .expect("Failed to install CTRL+C handler");
    tracing::info!("Shutdown signal received");
}
```

### Paso 5: Configurar lib.rs

```rust
// src/lib.rs
pub mod handlers;
pub mod server;

pub use server::{create_router, run_server};
pub use handlers::health::HealthResponse;
```

### Paso 6: Crear Binary (opcional)

```rust
// src/bin/server.rs
use std::net::SocketAddr;
use vortex_server::run_server;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    tracing_subscriber::fmt::init();

    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    run_server(addr).await
}
```

---

## Conceptos de Rust Aprendidos

### 1. Async/Await y Tokio Runtime

En Rust, la programacion asincrona se maneja con `async/await`, similar a otros lenguajes pero con diferencias importantes.

**Rust:**

```rust
use tokio::net::TcpListener;

// Las funciones async retornan un Future
pub async fn run_server(addr: SocketAddr) -> Result<(), std::io::Error> {
    // await "desempaqueta" el Future y espera su resultado
    let listener = TcpListener::bind(addr).await?;

    // El servidor corre indefinidamente
    axum::serve(listener, app).await
}

// El runtime se configura con el macro #[tokio::main]
#[tokio::main]
async fn main() {
    run_server("0.0.0.0:8080".parse().unwrap()).await.unwrap();
}
```

**Comparacion con Java (CompletableFuture):**

```java
// Java requiere encadenar callbacks o usar virtual threads
public CompletableFuture<Void> runServer(int port) {
    return HttpServer.create()
        .port(port)
        .bindNow()
        .onDispose()
        .toFuture();
}

// Con Virtual Threads (Java 21+)
public void runServer(int port) throws Exception {
    try (var executor = Executors.newVirtualThreadPerTaskExecutor()) {
        var server = HttpServer.create(new InetSocketAddress(port), 0);
        server.start();
    }
}
```

**Diferencias clave:**

| Aspecto | Rust (Tokio) | Java |
|---------|--------------|------|
| Runtime | Explicito (`#[tokio::main]`) | Implicito (JVM) |
| Futures | Lazy (no ejecutan hasta await) | Eager (ejecutan inmediatamente) |
| Cancelacion | Automatica al dropear Future | Manual con `cancel()` |
| Overhead | Zero-cost abstractions | Overhead de objetos |

### 2. Axum Router y Handlers

Axum usa un sistema de routing type-safe sin macros.

**Rust:**

```rust
use axum::{Router, routing::get, Json};
use serde::Serialize;

#[derive(Serialize)]
struct HealthResponse {
    status: String,
}

// Los handlers son funciones async que retornan algo que implemente IntoResponse
async fn health_check() -> Json<HealthResponse> {
    Json(HealthResponse { status: "UP".to_string() })
}

// El router se construye con un builder pattern
fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/api/v1/users", get(list_users).post(create_user))
}
```

**Comparacion con Spring Boot:**

```java
@RestController
public class HealthController {

    @GetMapping("/health")
    public HealthResponse healthCheck() {
        return new HealthResponse("UP");
    }
}

// El routing se configura con anotaciones
@GetMapping("/api/v1/users")
public List<User> listUsers() { ... }

@PostMapping("/api/v1/users")
public User createUser(@RequestBody UserDto dto) { ... }
```

**Diferencias clave:**

| Aspecto | Axum | Spring |
|---------|------|--------|
| Routing | Explicito con `Router::new()` | Implicito con anotaciones |
| Type safety | Compile-time | Runtime (reflection) |
| Discovery | Manual | Component scanning |
| Startup | Instantaneo | Lento (classpath scan) |

### 3. Serde y Derive Macros

Serde es el estandar para serializacion en Rust, similar a Jackson en Java.

**Rust:**

```rust
use serde::{Serialize, Deserialize};

// derive genera automaticamente la implementacion
#[derive(Debug, Serialize, Deserialize)]
pub struct HealthResponse {
    pub status: String,

    // Atributos para personalizar serializacion
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,

    #[serde(rename = "responseTime")]
    pub response_time_ms: u64,
}

// Uso automatico con Json<T>
async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "UP".to_string(),
        details: None,
        response_time_ms: 5,
    })
}
```

**Comparacion con Jackson:**

```java
public class HealthResponse {
    private String status;

    @JsonInclude(JsonInclude.Include.NON_NULL)
    private String details;

    @JsonProperty("responseTime")
    private long responseTimeMs;

    // Getters, setters, constructors...
}
```

**Diferencias clave:**

- Rust genera codigo en compile-time (zero runtime reflection)
- Serde es mas rapido que Jackson en benchmarks
- `#[derive]` elimina boilerplate como `@Data` de Lombok

### 4. Result y Operador ?

El manejo de errores en Rust es explicito con `Result<T, E>`.

**Rust:**

```rust
use std::io;
use std::net::SocketAddr;

// El tipo de retorno explicita que puede fallar
pub async fn run_server(addr: SocketAddr) -> Result<(), io::Error> {
    // El operador ? propaga el error si falla
    let listener = tokio::net::TcpListener::bind(addr).await?;

    // Equivalente sin ?:
    // let listener = match tokio::net::TcpListener::bind(addr).await {
    //     Ok(l) => l,
    //     Err(e) => return Err(e),
    // };

    axum::serve(listener, app).await?;
    Ok(())
}
```

**Comparacion con Java:**

```java
// Java usa checked exceptions
public void runServer(int port) throws IOException {
    ServerSocket socket = new ServerSocket(port); // puede lanzar IOException
}

// O las envuelve en RuntimeException
public void runServer(int port) {
    try {
        ServerSocket socket = new ServerSocket(port);
    } catch (IOException e) {
        throw new RuntimeException(e);
    }
}
```

**Diferencias clave:**

| Aspecto | Rust Result | Java Exceptions |
|---------|-------------|-----------------|
| Tipo | Parte del tipo de retorno | Metadata separada |
| Forzado | Compile-time | Runtime |
| Propagacion | `?` operator | `throws` o try-catch |
| Performance | Zero-cost | Stack unwinding |

---

## Riesgos y Errores Comunes

### 1. Olvidar .await

```rust
// MAL: El Future nunca se ejecuta
async fn bad_example() {
    health_check(); // Compila pero no hace nada!
}

// BIEN: Usar .await
async fn good_example() {
    health_check().await;
}
```

### 2. Bloquear el Runtime

```rust
// MAL: std::thread::sleep bloquea el thread del runtime
async fn bad_sleep() {
    std::thread::sleep(Duration::from_secs(1)); // NUNCA hacer esto!
}

// BIEN: Usar tokio::time::sleep
async fn good_sleep() {
    tokio::time::sleep(Duration::from_secs(1)).await;
}
```

### 3. Panic en Handlers

```rust
// MAL: unwrap() puede causar panic
async fn bad_handler() -> Json<Value> {
    let data = some_fallible_operation().unwrap(); // Crash!
    Json(data)
}

// BIEN: Retornar Result o manejar el error
async fn good_handler() -> Result<Json<Value>, StatusCode> {
    let data = some_fallible_operation()
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(data))
}
```

---

## Pruebas

### Tests Unitarios

```rust
// tests/health_test.rs
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use tower::ServiceExt;
use vortex_server::create_router;

#[tokio::test]
async fn health_check_returns_200() {
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

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn health_check_returns_json() {
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

    let content_type = response
        .headers()
        .get("content-type")
        .unwrap()
        .to_str()
        .unwrap();

    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn health_check_body_contains_status_up() {
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

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let health: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(health["status"], "UP");
}
```

### Test del Response Type

```rust
#[test]
fn health_response_serializes_correctly() {
    use vortex_server::HealthResponse;

    let response = HealthResponse::default();
    let json = serde_json::to_string(&response).unwrap();

    assert_eq!(json, r#"{"status":"UP"}"#);
}
```

---

## Observabilidad

### Logging Basico

```rust
// En main.rs o al inicio del servidor
tracing_subscriber::fmt::init();

// En el servidor
tracing::info!("Server listening on {}", addr);
tracing::info!("Shutdown signal received");
```

### Metricas (Preparacion)

El health endpoint puede extenderse para incluir metricas basicas:

```rust
#[derive(Serialize)]
pub struct HealthResponse {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub uptime_seconds: Option<u64>,
}
```

---

## Entregable Final

### Archivos Creados

1. `crates/vortex-server/Cargo.toml` - Dependencias del crate
2. `crates/vortex-server/src/lib.rs` - Re-exports publicos
3. `crates/vortex-server/src/server.rs` - Configuracion del servidor
4. `crates/vortex-server/src/handlers/mod.rs` - Modulo de handlers
5. `crates/vortex-server/src/handlers/health.rs` - Health check handler
6. `crates/vortex-server/tests/health_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server

# Clippy
cargo clippy -p vortex-server -- -D warnings

# Ejecutar (si hay binary)
cargo run -p vortex-server

# Verificar endpoint
curl http://localhost:8080/health
# {"status":"UP"}
```

### Ejemplo de Uso

```rust
use std::net::SocketAddr;
use vortex_server::run_server;

#[tokio::main]
async fn main() -> Result<(), std::io::Error> {
    let addr = SocketAddr::from(([0, 0, 0, 0], 8080));
    run_server(addr).await
}
```
