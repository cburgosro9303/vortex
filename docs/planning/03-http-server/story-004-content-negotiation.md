# Historia 004: Content Negotiation

## Contexto y Objetivo

Content negotiation permite a los clientes solicitar el formato de respuesta preferido usando el header `Accept`. Spring Cloud Config Server soporta multiples formatos, y Vortex Config debe ser compatible.

**Formatos soportados:**
- `application/json` - JSON (por defecto)
- `application/x-yaml` o `text/yaml` - YAML
- `text/plain` - Properties (formato Java `.properties`)

**Ejemplos:**
```bash
# JSON (por defecto)
curl http://localhost:8080/myapp/dev
curl -H "Accept: application/json" http://localhost:8080/myapp/dev

# YAML
curl -H "Accept: application/x-yaml" http://localhost:8080/myapp/dev

# Properties
curl -H "Accept: text/plain" http://localhost:8080/myapp/dev
```

---

## Alcance

### In Scope

- Extractor para header `Accept`
- Serializacion a JSON, YAML, y Properties
- Content-Type correcto en respuesta
- Manejo de `Accept: */*` (usar JSON)
- Manejo de Accept invalido (400 Bad Request o default JSON)
- Endpoints alternativos: `/{app}-{profile}.yml`, `/{app}-{profile}.properties`

### Out of Scope

- Compresion (gzip, brotli)
- Accept con quality values (`Accept: application/json;q=0.9`)
- Formatos adicionales (TOML, XML, etc)

---

## Criterios de Aceptacion

- [ ] `Accept: application/json` retorna JSON con `Content-Type: application/json`
- [ ] `Accept: application/x-yaml` retorna YAML con `Content-Type: application/x-yaml`
- [ ] `Accept: text/yaml` tambien retorna YAML
- [ ] `Accept: text/plain` retorna formato `.properties`
- [ ] Sin header `Accept` retorna JSON
- [ ] `Accept: */*` retorna JSON
- [ ] `GET /{app}-{profile}.yml` retorna YAML directo
- [ ] `GET /{app}-{profile}.properties` retorna properties directo

---

## Diseno Propuesto

### Enum de Formatos

```rust
// src/extractors/accept.rs
#[derive(Debug, Clone, Copy, Default)]
pub enum OutputFormat {
    #[default]
    Json,
    Yaml,
    Properties,
}
```

### Extractor de Accept Header

```rust
// src/extractors/accept.rs
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts, StatusCode},
};

pub struct AcceptFormat(pub OutputFormat);

#[async_trait]
impl<S> FromRequestParts<S> for AcceptFormat
where
    S: Send + Sync,
{
    type Rejection = StatusCode;

    async fn from_request_parts(parts: &mut Parts, _state: &S) -> Result<Self, Self::Rejection> {
        let accept = parts
            .headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok());

        let format = OutputFormat::from_accept(accept);
        Ok(AcceptFormat(format))
    }
}
```

### Estructura de Modulos

```
crates/vortex-server/src/
├── extractors/
│   ├── mod.rs
│   ├── path.rs
│   ├── query.rs
│   └── accept.rs      # NUEVO: Accept header extractor
├── response/
│   ├── mod.rs         # NUEVO
│   ├── json.rs        # NUEVO: JSON serializer
│   ├── yaml.rs        # NUEVO: YAML serializer
│   └── properties.rs  # NUEVO: Properties serializer
└── handlers/
    └── config.rs      # Actualizado para usar formatos
```

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencias

```toml
# Cargo.toml
[dependencies]
serde_yaml = "0.9"
```

### Paso 2: Implementar OutputFormat

```rust
// src/extractors/accept.rs
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::{header, request::Parts},
    response::{IntoResponse, Response},
};

/// Formatos de salida soportados.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    #[default]
    Json,
    Yaml,
    Properties,
}

impl OutputFormat {
    /// Determina el formato basado en el header Accept.
    pub fn from_accept(accept: Option<&str>) -> Self {
        match accept {
            None => Self::Json,
            Some(accept) => {
                let accept = accept.to_lowercase();

                if accept.contains("application/x-yaml")
                    || accept.contains("text/yaml")
                    || accept.contains("application/yaml")
                {
                    Self::Yaml
                } else if accept.contains("text/plain") {
                    Self::Properties
                } else {
                    // Default to JSON for application/json, */*, or unknown
                    Self::Json
                }
            }
        }
    }

    /// Retorna el Content-Type correspondiente.
    pub fn content_type(&self) -> &'static str {
        match self {
            Self::Json => "application/json",
            Self::Yaml => "application/x-yaml",
            Self::Properties => "text/plain; charset=utf-8",
        }
    }
}

/// Extractor que parsea el header Accept.
pub struct AcceptFormat(pub OutputFormat);

#[async_trait]
impl<S> FromRequestParts<S> for AcceptFormat
where
    S: Send + Sync,
{
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        let accept = parts
            .headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok());

        Ok(AcceptFormat(OutputFormat::from_accept(accept)))
    }
}
```

### Paso 3: Implementar Serializadores

```rust
// src/response/mod.rs
pub mod json;
pub mod yaml;
pub mod properties;

use crate::extractors::accept::OutputFormat;
use crate::handlers::response::ConfigResponse;
use axum::response::{IntoResponse, Response};

/// Serializa ConfigResponse al formato especificado.
pub fn serialize_config(
    config: &ConfigResponse,
    format: OutputFormat,
) -> Result<Response, SerializeError> {
    match format {
        OutputFormat::Json => json::to_response(config),
        OutputFormat::Yaml => yaml::to_response(config),
        OutputFormat::Properties => properties::to_response(config),
    }
}

#[derive(Debug, thiserror::Error)]
pub enum SerializeError {
    #[error("JSON serialization failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("YAML serialization failed: {0}")]
    Yaml(#[from] serde_yaml::Error),

    #[error("Properties serialization failed: {0}")]
    Properties(String),
}

impl IntoResponse for SerializeError {
    fn into_response(self) -> Response {
        use axum::http::StatusCode;
        (StatusCode::INTERNAL_SERVER_ERROR, self.to_string()).into_response()
    }
}
```

```rust
// src/response/json.rs
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Serialize;

pub fn to_response<T: Serialize>(data: &T) -> Result<Response, super::SerializeError> {
    let body = serde_json::to_string_pretty(data)?;

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/json")],
        body,
    )
        .into_response())
}
```

```rust
// src/response/yaml.rs
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use serde::Serialize;

pub fn to_response<T: Serialize>(data: &T) -> Result<Response, super::SerializeError> {
    let body = serde_yaml::to_string(data)?;

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "application/x-yaml")],
        body,
    )
        .into_response())
}
```

```rust
// src/response/properties.rs
use axum::{
    http::{header, StatusCode},
    response::{IntoResponse, Response},
};
use crate::handlers::response::ConfigResponse;

/// Convierte ConfigResponse a formato .properties de Java.
///
/// Ejemplo de salida:
/// ```properties
/// server.port=8080
/// spring.application.name=myapp
/// ```
pub fn to_response(config: &ConfigResponse) -> Result<Response, super::SerializeError> {
    let mut output = String::new();

    // Agregar comentario con metadata
    output.push_str(&format!("# Application: {}\n", config.name));
    output.push_str(&format!("# Profiles: {}\n", config.profiles.join(",")));
    if let Some(ref label) = config.label {
        output.push_str(&format!("# Label: {}\n", label));
    }
    output.push('\n');

    // Iterar property sources (en orden inverso para precedencia correcta)
    for ps in config.property_sources.iter().rev() {
        output.push_str(&format!("# Source: {}\n", ps.name));

        for (key, value) in &ps.source {
            let value_str = json_value_to_properties_string(value);
            // Escapar caracteres especiales en key
            let escaped_key = escape_properties_key(key);
            output.push_str(&format!("{}={}\n", escaped_key, value_str));
        }
        output.push('\n');
    }

    Ok((
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        output,
    )
        .into_response())
}

/// Convierte un JSON value a string para .properties.
fn json_value_to_properties_string(value: &serde_json::Value) -> String {
    match value {
        serde_json::Value::Null => String::new(),
        serde_json::Value::Bool(b) => b.to_string(),
        serde_json::Value::Number(n) => n.to_string(),
        serde_json::Value::String(s) => escape_properties_value(s),
        serde_json::Value::Array(arr) => {
            // Arrays como lista separada por comas
            arr.iter()
                .map(|v| json_value_to_properties_string(v))
                .collect::<Vec<_>>()
                .join(",")
        }
        serde_json::Value::Object(_) => {
            // Objetos como JSON inline (no ideal, pero funcional)
            value.to_string()
        }
    }
}

/// Escapa caracteres especiales en keys de properties.
fn escape_properties_key(key: &str) -> String {
    key.replace('\\', "\\\\")
        .replace(':', "\\:")
        .replace('=', "\\=")
        .replace(' ', "\\ ")
}

/// Escapa caracteres especiales en values de properties.
fn escape_properties_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}
```

### Paso 4: Actualizar Handler

```rust
// src/handlers/config.rs
use axum::extract::Path;
use tracing::instrument;

use crate::error::AppError;
use crate::extractors::{
    accept::AcceptFormat,
    path::AppProfilePath,
};
use crate::handlers::response::ConfigResponse;
use crate::response::{serialize_config, SerializeError};

#[instrument(skip_all, fields(app = %path.app, profile = %path.profile))]
pub async fn get_config(
    Path(path): Path<AppProfilePath>,
    AcceptFormat(format): AcceptFormat,
) -> Result<axum::response::Response, AppError> {
    path.validate().map_err(AppError::BadRequest)?;

    let profiles = path.profiles();
    tracing::info!(?format, "Fetching config");

    let response = create_mock_response(&path.app, profiles, None);

    serialize_config(&response, format)
        .map_err(|e| AppError::Internal(e.to_string()))
}
```

### Paso 5: Endpoints con Extension

```rust
// src/handlers/config.rs

/// Handler para GET /{app}-{profile}.yml
/// Retorna configuracion directamente en YAML.
#[instrument(skip_all)]
pub async fn get_config_yaml(
    Path((app, profile)): Path<(String, String)>,
) -> Result<axum::response::Response, AppError> {
    // Parsear app-profile del path
    let (app, profile) = parse_app_profile(&app, &profile)?;

    let response = create_mock_response(&app, vec![profile], None);

    crate::response::yaml::to_response(&response)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Handler para GET /{app}-{profile}.properties
#[instrument(skip_all)]
pub async fn get_config_properties(
    Path((app, profile)): Path<(String, String)>,
) -> Result<axum::response::Response, AppError> {
    let (app, profile) = parse_app_profile(&app, &profile)?;

    let response = create_mock_response(&app, vec![profile], None);

    crate::response::properties::to_response(&response)
        .map_err(|e| AppError::Internal(e.to_string()))
}

/// Parsea el formato {app}-{profile} del path.
fn parse_app_profile(segment: &str, ext: &str) -> Result<(String, String), AppError> {
    // segment = "myapp-dev", ext = "yml"
    let full = if ext.is_empty() {
        segment.to_string()
    } else {
        segment.to_string()
    };

    // Buscar el ultimo guion (app puede tener guiones)
    match full.rfind('-') {
        Some(pos) => {
            let app = &full[..pos];
            let profile = &full[pos + 1..];
            Ok((app.to_string(), profile.to_string()))
        }
        None => Err(AppError::BadRequest(
            "Invalid format. Expected: {app}-{profile}".to_string(),
        )),
    }
}
```

### Paso 6: Actualizar Router

```rust
// src/server.rs
use axum::{Router, routing::get};
use crate::handlers::config::{
    get_config,
    get_config_with_label,
    get_config_yaml,
    get_config_properties,
};

pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        // Endpoints con extension (mas especificos primero)
        .route("/:app_profile.yml", get(get_config_yaml))
        .route("/:app_profile.properties", get(get_config_properties))
        // Endpoints estandar
        .route("/:app/:profile/:label", get(get_config_with_label))
        .route("/:app/:profile", get(get_config))
}
```

---

## Conceptos de Rust Aprendidos

### 1. Custom Extractors en Axum

Puedes crear extractors personalizados implementando `FromRequestParts` o `FromRequest`.

**Rust:**
```rust
use axum::{
    async_trait,
    extract::FromRequestParts,
    http::request::Parts,
};

// El extractor encapsula la logica de parseo
pub struct AcceptFormat(pub OutputFormat);

#[async_trait]
impl<S> FromRequestParts<S> for AcceptFormat
where
    S: Send + Sync,
{
    // Tipo de error si la extraccion falla
    type Rejection = std::convert::Infallible;

    async fn from_request_parts(
        parts: &mut Parts,
        _state: &S,
    ) -> Result<Self, Self::Rejection> {
        // Acceso a headers, method, URI, etc
        let accept = parts
            .headers
            .get(header::ACCEPT)
            .and_then(|v| v.to_str().ok());

        Ok(AcceptFormat(OutputFormat::from_accept(accept)))
    }
}

// Uso en handler - automatico!
async fn handler(AcceptFormat(format): AcceptFormat) -> impl IntoResponse {
    // format ya esta parseado
}
```

**Comparacion con Spring:**
```java
// Spring usa HandlerMethodArgumentResolver
public class AcceptFormatResolver implements HandlerMethodArgumentResolver {

    @Override
    public boolean supportsParameter(MethodParameter parameter) {
        return parameter.getParameterType().equals(OutputFormat.class);
    }

    @Override
    public Object resolveArgument(
            MethodParameter parameter,
            ModelAndViewContainer mavContainer,
            NativeWebRequest webRequest,
            WebDataBinderFactory binderFactory) {

        String accept = webRequest.getHeader("Accept");
        return OutputFormat.fromAccept(accept);
    }
}

// Uso en controller
@GetMapping("/config")
public ResponseEntity<?> getConfig(OutputFormat format) {
    // ...
}
```

**Diferencias clave:**
| Aspecto | Axum | Spring |
|---------|------|--------|
| Implementacion | Trait `FromRequestParts` | Interface `HandlerMethodArgumentResolver` |
| Registro | Automatico por tipos | Manual en `WebMvcConfigurer` |
| Async | Nativo con async | Blocking |
| Type safety | Compile-time | Runtime |

### 2. Fn, FnMut, FnOnce - Los Traits de Closures

Las closures en Rust implementan uno o mas de estos traits segun como capturan variables.

**Rust:**
```rust
// Fn: Captura por referencia inmutable, puede llamarse multiples veces
let name = String::from("myapp");
let get_name = || &name;  // Solo lee `name`
println!("{}", get_name());
println!("{}", get_name()); // OK, puede llamarse otra vez

// FnMut: Captura por referencia mutable
let mut counter = 0;
let mut increment = || {
    counter += 1;  // Modifica `counter`
    counter
};
println!("{}", increment()); // 1
println!("{}", increment()); // 2

// FnOnce: Consume la captura, solo puede llamarse una vez
let name = String::from("myapp");
let consume = move || {
    drop(name);  // Consume `name`
};
consume();
// consume(); // ERROR: closure ya consumida

// En la practica, muchas closures implementan Fn
let values = vec![1, 2, 3];
let doubled: Vec<_> = values.iter()
    .map(|x| x * 2)  // Esta closure es Fn
    .collect();
```

**Uso con tipos genericos:**
```rust
// Aceptar cualquier closure que implemente Fn
fn apply_format<F>(value: &str, formatter: F) -> String
where
    F: Fn(&str) -> String,  // Trait bound
{
    formatter(value)
}

let result = apply_format("hello", |s| s.to_uppercase());
```

**Comparacion con Java:**
```java
// Java tiene interfaces funcionales equivalentes
// Fn -> Function<T, R>, Supplier<T>, Consumer<T>
// FnMut -> No hay equivalente directo (Java no tiene mutabilidad explicita)
// FnOnce -> No hay equivalente (Java no tiene ownership)

Function<String, String> formatter = s -> s.toUpperCase();
String result = formatter.apply("hello");
```

### 3. Pattern Matching con match

El `match` de Rust es mucho mas poderoso que el switch de Java.

**Rust:**
```rust
impl OutputFormat {
    pub fn from_accept(accept: Option<&str>) -> Self {
        match accept {
            // Pattern: None
            None => Self::Json,

            // Pattern: Some con binding
            Some(accept) => {
                let accept = accept.to_lowercase();

                // Match con guards (condiciones adicionales)
                if accept.contains("yaml") {
                    Self::Yaml
                } else if accept.contains("text/plain") {
                    Self::Properties
                } else {
                    Self::Json
                }
            }
        }
    }

    pub fn content_type(&self) -> &'static str {
        // Match en enums - debe ser exhaustivo
        match self {
            Self::Json => "application/json",
            Self::Yaml => "application/x-yaml",
            Self::Properties => "text/plain; charset=utf-8",
        }
    }
}

// Match con destructuring de structs
match config {
    ConfigResponse { name, profiles, .. } if profiles.is_empty() => {
        // Acceso directo a name y profiles
    }
    ConfigResponse { label: Some(l), .. } => {
        // Solo si label es Some
    }
    _ => {
        // Catch-all
    }
}
```

**Comparacion con Java (switch expressions):**
```java
// Java 17+ switch expressions
String contentType = switch (format) {
    case JSON -> "application/json";
    case YAML -> "application/x-yaml";
    case PROPERTIES -> "text/plain";
};

// Pero Java no puede hacer pattern matching en Option/structs facilmente
// Requiere instanceof + cast o pattern matching preview (Java 21+)
```

### 4. Trait Objects vs Generics

Puedes usar traits de dos formas: con generics (compile-time) o trait objects (runtime).

**Rust:**
```rust
use serde::Serialize;

// Generics: Monomorphization en compile-time (mas rapido, mas codigo)
pub fn to_json<T: Serialize>(data: &T) -> String {
    serde_json::to_string(data).unwrap()
}

// Trait object: Dynamic dispatch en runtime (menos codigo, un poco mas lento)
pub fn to_json_dyn(data: &dyn Serialize) -> String {
    // Nota: Serialize no es object-safe, esto es solo ejemplo conceptual
    unimplemented!()
}

// En la practica, para serializacion usamos generics
fn serialize_config<T: Serialize>(
    data: &T,
    format: OutputFormat,
) -> Result<String, SerializeError> {
    match format {
        OutputFormat::Json => Ok(serde_json::to_string_pretty(data)?),
        OutputFormat::Yaml => Ok(serde_yaml::to_string(data)?),
        OutputFormat::Properties => {
            // Properties requiere estructura especifica
            Err(SerializeError::Properties("Use specific function".into()))
        }
    }
}
```

---

## Riesgos y Errores Comunes

### 1. No Manejar Accept Malformado

```rust
// MAL: Panic si el header no es UTF-8 valido
let accept = parts.headers.get(header::ACCEPT)
    .unwrap()  // Panic si no existe
    .to_str()
    .unwrap();  // Panic si no es UTF-8

// BIEN: Usar Option y defaults
let accept = parts.headers.get(header::ACCEPT)
    .and_then(|v| v.to_str().ok());  // None si falla
```

### 2. Content-Type Incorrecto

```rust
// MAL: Retornar YAML con Content-Type JSON
let yaml = serde_yaml::to_string(&data)?;
Json(yaml)  // Content-Type sera application/json!

// BIEN: Usar Content-Type explicito
(
    StatusCode::OK,
    [(header::CONTENT_TYPE, "application/x-yaml")],
    yaml,
).into_response()
```

### 3. Properties con Nested Objects

```rust
// Los .properties no soportan objetos anidados nativamente
// MAL: Convertir directamente
let value = json!({"nested": {"key": "value"}});
// Resultado: nested.key={"key":"value"}  // No es lo esperado

// BIEN: Flatten o serializar como JSON string
fn flatten_json(prefix: &str, value: &Value, output: &mut HashMap<String, String>) {
    match value {
        Value::Object(map) => {
            for (k, v) in map {
                let new_prefix = if prefix.is_empty() {
                    k.clone()
                } else {
                    format!("{}.{}", prefix, k)
                };
                flatten_json(&new_prefix, v, output);
            }
        }
        _ => {
            output.insert(prefix.to_string(), value.to_string());
        }
    }
}
```

---

## Pruebas

### Tests del OutputFormat

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn from_accept_returns_json_for_none() {
        assert_eq!(OutputFormat::from_accept(None), OutputFormat::Json);
    }

    #[test]
    fn from_accept_returns_json_for_application_json() {
        assert_eq!(
            OutputFormat::from_accept(Some("application/json")),
            OutputFormat::Json
        );
    }

    #[test]
    fn from_accept_returns_yaml_for_application_yaml() {
        assert_eq!(
            OutputFormat::from_accept(Some("application/x-yaml")),
            OutputFormat::Yaml
        );
        assert_eq!(
            OutputFormat::from_accept(Some("text/yaml")),
            OutputFormat::Yaml
        );
        assert_eq!(
            OutputFormat::from_accept(Some("application/yaml")),
            OutputFormat::Yaml
        );
    }

    #[test]
    fn from_accept_returns_properties_for_text_plain() {
        assert_eq!(
            OutputFormat::from_accept(Some("text/plain")),
            OutputFormat::Properties
        );
    }

    #[test]
    fn from_accept_is_case_insensitive() {
        assert_eq!(
            OutputFormat::from_accept(Some("APPLICATION/JSON")),
            OutputFormat::Json
        );
        assert_eq!(
            OutputFormat::from_accept(Some("Application/X-YAML")),
            OutputFormat::Yaml
        );
    }

    #[test]
    fn from_accept_returns_json_for_wildcard() {
        assert_eq!(
            OutputFormat::from_accept(Some("*/*")),
            OutputFormat::Json
        );
    }

    #[test]
    fn content_type_matches_format() {
        assert_eq!(OutputFormat::Json.content_type(), "application/json");
        assert_eq!(OutputFormat::Yaml.content_type(), "application/x-yaml");
        assert!(OutputFormat::Properties.content_type().contains("text/plain"));
    }
}
```

### Tests de Integracion HTTP

```rust
// tests/content_negotiation_test.rs
use axum::{body::Body, http::{header, Request, StatusCode}};
use tower::ServiceExt;
use vortex_server::create_router;

#[tokio::test]
async fn returns_json_by_default() {
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

    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();

    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn returns_json_for_accept_json() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
                .header(header::ACCEPT, "application/json")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();

    assert!(content_type.contains("application/json"));
}

#[tokio::test]
async fn returns_yaml_for_accept_yaml() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
                .header(header::ACCEPT, "application/x-yaml")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();

    assert!(content_type.contains("yaml"));

    // Verificar que el body es YAML valido
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();
    assert!(body_str.contains("name:"));  // YAML usa : en vez de :
}

#[tokio::test]
async fn returns_properties_for_accept_text_plain() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev")
                .header(header::ACCEPT, "text/plain")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();

    assert!(content_type.contains("text/plain"));

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    let body_str = String::from_utf8(body.to_vec()).unwrap();

    // Properties format usa key=value
    assert!(body_str.contains("="));
    // Y comentarios con #
    assert!(body_str.contains("#"));
}

#[tokio::test]
async fn yml_extension_endpoint_returns_yaml() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp-dev.yml")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();

    assert!(content_type.contains("yaml"));
}

#[tokio::test]
async fn properties_extension_endpoint_returns_properties() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp-dev.properties")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);

    let content_type = response.headers()
        .get(header::CONTENT_TYPE)
        .unwrap()
        .to_str()
        .unwrap();

    assert!(content_type.contains("text/plain"));
}
```

### Tests del Serializador Properties

```rust
#[cfg(test)]
mod properties_tests {
    use super::*;

    #[test]
    fn escapes_special_characters_in_value() {
        assert_eq!(escape_properties_value("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_properties_value("tab\there"), "tab\\there");
        assert_eq!(escape_properties_value("back\\slash"), "back\\\\slash");
    }

    #[test]
    fn escapes_special_characters_in_key() {
        assert_eq!(escape_properties_key("key:with:colons"), "key\\:with\\:colons");
        assert_eq!(escape_properties_key("key=with=equals"), "key\\=with\\=equals");
        assert_eq!(escape_properties_key("key with spaces"), "key\\ with\\ spaces");
    }

    #[test]
    fn converts_json_values_correctly() {
        use serde_json::json;

        assert_eq!(json_value_to_properties_string(&json!(null)), "");
        assert_eq!(json_value_to_properties_string(&json!(true)), "true");
        assert_eq!(json_value_to_properties_string(&json!(42)), "42");
        assert_eq!(json_value_to_properties_string(&json!("hello")), "hello");
        assert_eq!(json_value_to_properties_string(&json!([1, 2, 3])), "1,2,3");
    }
}
```

---

## Observabilidad

### Logging del Formato

```rust
#[instrument(skip_all, fields(app = %path.app, profile = %path.profile, format = ?format))]
pub async fn get_config(
    Path(path): Path<AppProfilePath>,
    AcceptFormat(format): AcceptFormat,
) -> Result<axum::response::Response, AppError> {
    tracing::info!("Processing request");
    // ...
}
```

### Metricas (Futuro)

```rust
// Contador por formato
counter!("config_requests_total", "format" => format.to_string()).increment(1);
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `Cargo.toml` - Agregar `serde_yaml`
2. `src/extractors/accept.rs` - NUEVO: OutputFormat y AcceptFormat
3. `src/extractors/mod.rs` - Re-export accept module
4. `src/response/mod.rs` - NUEVO: Modulo de serializacion
5. `src/response/json.rs` - NUEVO: JSON serializer
6. `src/response/yaml.rs` - NUEVO: YAML serializer
7. `src/response/properties.rs` - NUEVO: Properties serializer
8. `src/handlers/config.rs` - Actualizado con content negotiation
9. `src/server.rs` - Rutas con extension
10. `tests/content_negotiation_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server

# Probar formatos
curl http://localhost:8080/myapp/dev | head -5
# {
#   "name": "myapp",
#   ...

curl -H "Accept: application/x-yaml" http://localhost:8080/myapp/dev | head -5
# name: myapp
# profiles:
# ...

curl -H "Accept: text/plain" http://localhost:8080/myapp/dev | head -5
# # Application: myapp
# # Profiles: dev
# server.port=8080

# Endpoints con extension
curl http://localhost:8080/myapp-dev.yml | head -3
curl http://localhost:8080/myapp-dev.properties | head -3
```
