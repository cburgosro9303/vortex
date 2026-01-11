# Historia 002: Endpoint GET /{app}/{profile}

## Contexto y Objetivo

Este es el endpoint principal de Vortex Config, equivalente al endpoint canonico de Spring Cloud Config Server. Permite a las aplicaciones cliente obtener su configuracion especificando el nombre de la aplicacion y el perfil activo.

El formato de respuesta debe ser 100% compatible con Spring Cloud Config para permitir migracion transparente de aplicaciones existentes.

**Ejemplo de uso:**
```bash
# Obtener configuracion de "myapp" con perfil "production"
curl http://localhost:8080/myapp/production

# Respuesta compatible Spring Cloud Config
{
  "name": "myapp",
  "profiles": ["production"],
  "label": null,
  "version": null,
  "propertySources": [...]
}
```

---

## Alcance

### In Scope

- Endpoint `GET /{app}/{profile}`
- Path extractors para `app` y `profile`
- Soporte de multiples profiles separados por coma
- Response type compatible con Spring Cloud Config
- Validacion basica de parametros
- Integracion con `ConfigMap` de `vortex-core`

### Out of Scope

- Soporte de label/branch (historia 003)
- Content negotiation (historia 004)
- Backend real de configuracion (usaremos mock)
- Cache de configuraciones

---

## Criterios de Aceptacion

- [ ] `GET /myapp/dev` retorna configuracion para app "myapp" y profile "dev"
- [ ] `GET /myapp/dev,local` soporta multiples profiles
- [ ] Response JSON es compatible con Spring Cloud Config
- [ ] `GET /` retorna 404 (no match)
- [ ] App y profile vacios retornan 400 Bad Request
- [ ] Tests unitarios cubren happy path y error cases

---

## Diseno Propuesto

### Estructura de Response (Spring Compatible)

```rust
// src/handlers/response.rs
use serde::Serialize;
use std::collections::HashMap;

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub name: String,
    pub profiles: Vec<String>,
    pub label: Option<String>,
    pub version: Option<String>,
    pub state: Option<String>,
    pub property_sources: Vec<PropertySourceResponse>,
}

#[derive(Debug, Serialize)]
pub struct PropertySourceResponse {
    pub name: String,
    pub source: HashMap<String, serde_json::Value>,
}
```

### Path Extractor

```rust
// src/extractors/path.rs
use axum::extract::Path;
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct ConfigPath {
    pub app: String,
    pub profile: String,
}

// En el handler
async fn get_config(Path(path): Path<ConfigPath>) -> impl IntoResponse {
    // path.app y path.profile disponibles
}
```

### Estructura de Modulos

```
crates/vortex-server/src/
├── handlers/
│   ├── mod.rs
│   ├── health.rs
│   ├── config.rs      # NUEVO: Handler de configuracion
│   └── response.rs    # NUEVO: Tipos de response
├── extractors/
│   ├── mod.rs         # NUEVO
│   └── path.rs        # NUEVO: Path extractors
└── error.rs           # NUEVO: Errores HTTP
```

---

## Pasos de Implementacion

### Paso 1: Definir Tipos de Response

```rust
// src/handlers/response.rs
use serde::Serialize;
use std::collections::HashMap;

/// Response compatible con Spring Cloud Config Server.
///
/// Este struct mapea exactamente al formato JSON que retorna
/// Spring Cloud Config para mantener compatibilidad.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    /// Nombre de la aplicacion
    pub name: String,

    /// Lista de profiles activos
    pub profiles: Vec<String>,

    /// Label (branch/tag) usado, null si no se especifico
    pub label: Option<String>,

    /// Version del commit (para Git backend)
    pub version: Option<String>,

    /// Estado adicional del config server
    pub state: Option<String>,

    /// Lista de property sources en orden de precedencia
    pub property_sources: Vec<PropertySourceResponse>,
}

/// Representa un archivo de configuracion individual.
#[derive(Debug, Clone, Serialize)]
pub struct PropertySourceResponse {
    /// Nombre/path del archivo de configuracion
    pub name: String,

    /// Propiedades como mapa clave-valor
    pub source: HashMap<String, serde_json::Value>,
}

impl ConfigResponse {
    /// Crea una respuesta vacia para una aplicacion y profiles.
    pub fn empty(name: impl Into<String>, profiles: Vec<String>) -> Self {
        Self {
            name: name.into(),
            profiles,
            label: None,
            version: None,
            state: None,
            property_sources: Vec::new(),
        }
    }
}
```

### Paso 2: Definir Errores HTTP

```rust
// src/error.rs
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use serde::Serialize;

#[derive(Debug)]
pub enum AppError {
    /// Configuracion no encontrada
    NotFound { app: String, profile: String },

    /// Parametros invalidos
    BadRequest(String),

    /// Error interno
    Internal(String),
}

#[derive(Serialize)]
struct ErrorResponse {
    error: String,
    message: String,
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, error, message) = match self {
            AppError::NotFound { app, profile } => (
                StatusCode::NOT_FOUND,
                "Not Found",
                format!("Configuration not found for {}/{}", app, profile),
            ),
            AppError::BadRequest(msg) => (
                StatusCode::BAD_REQUEST,
                "Bad Request",
                msg,
            ),
            AppError::Internal(msg) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Internal Server Error",
                msg,
            ),
        };

        let body = Json(ErrorResponse {
            error: error.to_string(),
            message,
        });

        (status, body).into_response()
    }
}
```

### Paso 3: Implementar Path Extractor

```rust
// src/extractors/path.rs
use serde::Deserialize;

/// Extractor para rutas /{app}/{profile}
#[derive(Debug, Deserialize)]
pub struct AppProfilePath {
    pub app: String,
    pub profile: String,
}

impl AppProfilePath {
    /// Parsea el string de profiles separados por coma.
    ///
    /// # Ejemplo
    /// ```
    /// let path = AppProfilePath { app: "myapp".into(), profile: "dev,local".into() };
    /// assert_eq!(path.profiles(), vec!["dev", "local"]);
    /// ```
    pub fn profiles(&self) -> Vec<String> {
        self.profile
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Valida que los parametros no esten vacios.
    pub fn validate(&self) -> Result<(), String> {
        if self.app.trim().is_empty() {
            return Err("Application name cannot be empty".to_string());
        }
        if self.profile.trim().is_empty() {
            return Err("Profile cannot be empty".to_string());
        }
        Ok(())
    }
}
```

### Paso 4: Implementar Handler de Configuracion

```rust
// src/handlers/config.rs
use axum::{
    extract::Path,
    Json,
};
use std::collections::HashMap;
use tracing::instrument;

use crate::error::AppError;
use crate::extractors::path::AppProfilePath;
use crate::handlers::response::{ConfigResponse, PropertySourceResponse};

/// Handler para GET /{app}/{profile}
///
/// Retorna la configuracion para una aplicacion y perfil especificos.
/// Compatible con Spring Cloud Config Server.
#[instrument(skip_all, fields(app = %path.app, profile = %path.profile))]
pub async fn get_config(
    Path(path): Path<AppProfilePath>,
) -> Result<Json<ConfigResponse>, AppError> {
    // Validar parametros
    path.validate().map_err(AppError::BadRequest)?;

    let profiles = path.profiles();

    tracing::info!("Fetching config for {}/{:?}", path.app, profiles);

    // TODO: Integrar con ConfigSource real
    // Por ahora retornamos datos mock
    let response = create_mock_response(&path.app, profiles);

    Ok(Json(response))
}

/// Crea una respuesta mock para desarrollo.
/// Sera reemplazada por integracion con ConfigSource.
fn create_mock_response(app: &str, profiles: Vec<String>) -> ConfigResponse {
    let mut source = HashMap::new();
    source.insert(
        "server.port".to_string(),
        serde_json::Value::Number(8080.into()),
    );
    source.insert(
        "spring.application.name".to_string(),
        serde_json::Value::String(app.to_string()),
    );

    ConfigResponse {
        name: app.to_string(),
        profiles: profiles.clone(),
        label: None,
        version: None,
        state: None,
        property_sources: vec![PropertySourceResponse {
            name: format!("file:config/{}-{}.yml", app, profiles.first().unwrap_or(&"default".to_string())),
            source,
        }],
    }
}
```

### Paso 5: Actualizar Router

```rust
// src/server.rs
use axum::{Router, routing::get};
use crate::handlers::{health::health_check, config::get_config};

pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        .route("/:app/:profile", get(get_config))
}
```

---

## Conceptos de Rust Aprendidos

### 1. Path Extractors en Axum

Los extractors son el equivalente Rust a las anotaciones `@PathVariable` de Spring.

**Rust (Axum):**
```rust
use axum::extract::Path;
use serde::Deserialize;

// El struct define los parametros esperados
#[derive(Deserialize)]
struct ConfigPath {
    app: String,
    profile: String,
}

// Path<T> extrae y deserializa automaticamente
async fn get_config(Path(path): Path<ConfigPath>) -> impl IntoResponse {
    // path.app contiene el valor de :app
    // path.profile contiene el valor de :profile
    format!("App: {}, Profile: {}", path.app, path.profile)
}

// Registro de ruta con placeholders
Router::new().route("/:app/:profile", get(get_config))
```

**Comparacion con Spring:**
```java
@GetMapping("/{app}/{profile}")
public ConfigResponse getConfig(
    @PathVariable String app,
    @PathVariable String profile
) {
    // app y profile inyectados por Spring
    return configService.getConfig(app, profile);
}
```

**Diferencias clave:**
| Aspecto | Axum | Spring |
|---------|------|--------|
| Definicion | Struct + Deserialize | Anotaciones |
| Validacion | Compile-time | Runtime |
| Binding | Automatico via serde | Reflection |
| Errores | Tipo Result | Excepciones |

### 2. Trait IntoResponse

Axum usa el trait `IntoResponse` para convertir cualquier tipo en una respuesta HTTP.

**Rust:**
```rust
use axum::response::{IntoResponse, Response};
use axum::http::StatusCode;

// Cualquier tipo que implemente IntoResponse puede ser retornado
async fn handler() -> impl IntoResponse {
    "Hello World" // &str implementa IntoResponse
}

// Puedes retornar tuplas (status, body)
async fn with_status() -> impl IntoResponse {
    (StatusCode::CREATED, "Resource created")
}

// O implementar IntoResponse para tipos custom
impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let status = match &self {
            AppError::NotFound { .. } => StatusCode::NOT_FOUND,
            AppError::BadRequest(_) => StatusCode::BAD_REQUEST,
            AppError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        };

        let body = Json(ErrorBody::from(&self));
        (status, body).into_response()
    }
}
```

**Comparacion con Spring:**
```java
// Spring usa ResponseEntity o @ResponseStatus
@GetMapping("/resource")
public ResponseEntity<Resource> getResource() {
    return ResponseEntity
        .status(HttpStatus.OK)
        .body(resource);
}

// O excepciones con @ResponseStatus
@ResponseStatus(HttpStatus.NOT_FOUND)
public class ResourceNotFoundException extends RuntimeException {
    // ...
}
```

**Diferencias clave:**
- Rust: Tipo de retorno explicito con `Result<T, E>`
- Spring: Excepciones y ResponseEntity
- Axum: Composicion con tuplas `(StatusCode, Json<T>)`

### 3. Closures y Fn Traits

Las closures en Rust son similares a lambdas en Java pero con semantica de ownership.

**Rust:**
```rust
// Closure que captura por referencia (Fn)
let profiles: Vec<String> = self.profile
    .split(',')
    .map(|s| s.trim().to_string())  // |s| es el parametro
    .filter(|s| !s.is_empty())
    .collect();

// Closure que captura por valor (FnOnce)
let app_name = "myapp".to_string();
let make_response = move || {
    // `app_name` se mueve dentro de la closure
    ConfigResponse::empty(app_name, vec![])
};

// Closure que modifica captura (FnMut)
let mut count = 0;
let mut counter = || {
    count += 1;  // Modifica `count`
    count
};
```

**Comparacion con Java:**
```java
// Lambda en Java
List<String> profiles = Arrays.stream(this.profile.split(","))
    .map(String::trim)
    .filter(s -> !s.isEmpty())
    .collect(Collectors.toList());

// Las lambdas en Java capturan por referencia (effectively final)
String appName = "myapp";
Supplier<ConfigResponse> makeResponse = () -> {
    return ConfigResponse.empty(appName);
    // appName debe ser effectively final
};
```

**Los tres traits Fn:**
| Trait | Captura | Llamadas | Equivalente Java |
|-------|---------|----------|-----------------|
| `Fn` | `&self` | Multiples | Stateless lambda |
| `FnMut` | `&mut self` | Multiples | Lambda con estado |
| `FnOnce` | `self` | Una vez | Lambda que consume |

### 4. El Operador ? con Result

El operador `?` simplifica el manejo de errores propagando automaticamente.

**Rust:**
```rust
use crate::error::AppError;

async fn get_config(
    Path(path): Path<AppProfilePath>,
) -> Result<Json<ConfigResponse>, AppError> {
    // ? convierte el error y lo propaga si falla
    path.validate().map_err(AppError::BadRequest)?;

    // Equivalente sin ?:
    // match path.validate() {
    //     Ok(()) => {},
    //     Err(msg) => return Err(AppError::BadRequest(msg)),
    // }

    let config = fetch_config(&path.app).await?;
    Ok(Json(config))
}

// map_err transforma el tipo de error
fn validate(&self) -> Result<(), String> {
    if self.app.is_empty() {
        return Err("App cannot be empty".to_string());
    }
    Ok(())
}
```

**Comparacion con Java:**
```java
public ConfigResponse getConfig(String app, String profile)
        throws ValidationException, ConfigNotFoundException {
    // Propagacion manual con throws
    validate(app, profile);

    return fetchConfig(app);
}

// O con Optional/checked conversion
public Optional<ConfigResponse> getConfig(String app, String profile) {
    return validate(app, profile)
        .flatMap(v -> fetchConfig(app));
}
```

---

## Riesgos y Errores Comunes

### 1. Orden de Rutas en Router

```rust
// MAL: La ruta especifica nunca matchea
Router::new()
    .route("/:app/:profile", get(get_config))  // Captura todo!
    .route("/health", get(health_check))       // Nunca llega aqui

// BIEN: Rutas especificas primero
Router::new()
    .route("/health", get(health_check))       // Match exacto primero
    .route("/:app/:profile", get(get_config))  // Wildcard despues
```

### 2. Olvidar #[serde(rename_all)]

```rust
// MAL: JSON con snake_case (no compatible Spring)
#[derive(Serialize)]
pub struct ConfigResponse {
    pub property_sources: Vec<...>,  // Sera "property_sources"
}

// BIEN: camelCase para compatibilidad
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResponse {
    pub property_sources: Vec<...>,  // Sera "propertySources"
}
```

### 3. No Validar Input

```rust
// MAL: Confiar en el input
async fn get_config(Path(path): Path<AppProfilePath>) -> Json<ConfigResponse> {
    // path.app podria estar vacio!
    fetch_config(&path.app).await
}

// BIEN: Validar antes de usar
async fn get_config(
    Path(path): Path<AppProfilePath>,
) -> Result<Json<ConfigResponse>, AppError> {
    path.validate().map_err(AppError::BadRequest)?;
    Ok(Json(fetch_config(&path.app).await?))
}
```

---

## Pruebas

### Tests del Path Extractor

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn profiles_parses_single_profile() {
        let path = AppProfilePath {
            app: "myapp".to_string(),
            profile: "dev".to_string(),
        };
        assert_eq!(path.profiles(), vec!["dev"]);
    }

    #[test]
    fn profiles_parses_multiple_profiles() {
        let path = AppProfilePath {
            app: "myapp".to_string(),
            profile: "dev,local,custom".to_string(),
        };
        assert_eq!(path.profiles(), vec!["dev", "local", "custom"]);
    }

    #[test]
    fn profiles_trims_whitespace() {
        let path = AppProfilePath {
            app: "myapp".to_string(),
            profile: " dev , local ".to_string(),
        };
        assert_eq!(path.profiles(), vec!["dev", "local"]);
    }

    #[test]
    fn profiles_filters_empty() {
        let path = AppProfilePath {
            app: "myapp".to_string(),
            profile: "dev,,local".to_string(),
        };
        assert_eq!(path.profiles(), vec!["dev", "local"]);
    }

    #[test]
    fn validate_rejects_empty_app() {
        let path = AppProfilePath {
            app: "".to_string(),
            profile: "dev".to_string(),
        };
        assert!(path.validate().is_err());
    }

    #[test]
    fn validate_rejects_empty_profile() {
        let path = AppProfilePath {
            app: "myapp".to_string(),
            profile: "".to_string(),
        };
        assert!(path.validate().is_err());
    }
}
```

### Tests de Integracion HTTP

```rust
// tests/config_test.rs
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;
use vortex_server::create_router;

#[tokio::test]
async fn get_config_returns_200_for_valid_path() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_config_returns_correct_app_name() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/payment-service/production")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(config["name"], "payment-service");
    assert_eq!(config["profiles"][0], "production");
}

#[tokio::test]
async fn get_config_supports_multiple_profiles() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev,local")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
    let profiles = config["profiles"].as_array().unwrap();
    assert_eq!(profiles.len(), 2);
    assert_eq!(profiles[0], "dev");
    assert_eq!(profiles[1], "local");
}

#[tokio::test]
async fn get_config_returns_json_content_type() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
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
async fn get_config_has_property_sources() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert!(config["propertySources"].is_array());
}
```

---

## Observabilidad

### Tracing con instrument

```rust
use tracing::instrument;

#[instrument(skip_all, fields(app = %path.app, profile = %path.profile))]
pub async fn get_config(
    Path(path): Path<AppProfilePath>,
) -> Result<Json<ConfigResponse>, AppError> {
    tracing::info!("Fetching configuration");

    // El span incluye app y profile automaticamente
    let config = fetch_config(&path.app, &path.profiles()).await?;

    tracing::info!(
        property_sources = config.property_sources.len(),
        "Configuration fetched successfully"
    );

    Ok(Json(config))
}
```

### Logs Esperados

```
INFO get_config{app="myapp" profile="dev"}: Fetching configuration
INFO get_config{app="myapp" profile="dev"}: Configuration fetched successfully property_sources=1
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `src/handlers/response.rs` - Tipos de respuesta Spring-compatible
2. `src/handlers/config.rs` - Handler GET /{app}/{profile}
3. `src/extractors/mod.rs` - Modulo de extractors
4. `src/extractors/path.rs` - AppProfilePath extractor
5. `src/error.rs` - Errores HTTP tipados
6. `src/server.rs` - Router actualizado
7. `tests/config_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server

# Ejecutar servidor
cargo run -p vortex-server

# Probar endpoint
curl http://localhost:8080/myapp/dev | jq
# {
#   "name": "myapp",
#   "profiles": ["dev"],
#   "label": null,
#   "version": null,
#   "state": null,
#   "propertySources": [...]
# }

curl http://localhost:8080/myapp/dev,production | jq '.profiles'
# ["dev", "production"]
```
