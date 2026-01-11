# Historia 003: Endpoint GET /{app}/{profile}/{label}

## Contexto y Objetivo

Este endpoint extiende la API para soportar **labels**, que en el contexto de Spring Cloud Config representan branches, tags o commits de Git. El label permite a las aplicaciones obtener configuraciones de versiones especificas.

**Casos de uso:**
- Obtener configuracion de un branch especifico: `/myapp/dev/feature-x`
- Obtener configuracion de un tag de release: `/myapp/prod/v1.2.3`
- Rollback a configuracion anterior: `/myapp/prod/abc123` (commit hash)

Este endpoint es fundamental para:
- **Blue/Green deployments**: Diferentes versiones apuntando a diferentes labels
- **Canary releases**: Subset de instancias usando label experimental
- **Debugging**: Comparar configuraciones entre versiones

---

## Alcance

### In Scope

- Endpoint `GET /{app}/{profile}/{label}`
- Extension del path extractor para incluir label
- Manejo de label por defecto (cuando no se especifica)
- Sanitizacion de label (caracteres especiales en branches)
- Query parameters opcionales: `useDefaultLabel`

### Out of Scope

- Resolucion real de branches Git (sera implementado en Epica 04)
- Versionado de configuraciones
- Diff entre labels
- Cache por label

---

## Criterios de Aceptacion

- [ ] `GET /myapp/dev/main` retorna config con label "main"
- [ ] `GET /myapp/dev/feature%2Fx` soporta branches con `/` URL-encoded
- [ ] `GET /myapp/dev/v1.2.3` soporta tags semanticos
- [ ] Response incluye campo `label` con el valor especificado
- [ ] Labels vacios retornan 400 Bad Request
- [ ] Tests cubren casos de labels con caracteres especiales

---

## Diseno Propuesto

### Extension del Path Extractor

```rust
// src/extractors/path.rs

/// Extractor para rutas /{app}/{profile}/{label}
#[derive(Debug, Deserialize)]
pub struct AppProfileLabelPath {
    pub app: String,
    pub profile: String,
    pub label: String,
}

impl AppProfileLabelPath {
    /// Sanitiza el label para uso seguro.
    /// Decodifica URL encoding y normaliza.
    pub fn sanitized_label(&self) -> String {
        // feature%2Fx -> feature/x
        urlencoding::decode(&self.label)
            .unwrap_or(std::borrow::Cow::Borrowed(&self.label))
            .into_owned()
    }
}
```

### Query Parameters Opcionales

```rust
// src/extractors/query.rs
use serde::Deserialize;

#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ConfigQuery {
    /// Si true, usa el label por defecto cuando el especificado no existe
    #[serde(rename = "useDefaultLabel")]
    pub use_default_label: bool,
}
```

### Estructura de Modulos

```
crates/vortex-server/src/
├── extractors/
│   ├── mod.rs
│   ├── path.rs        # AppProfilePath + AppProfileLabelPath
│   └── query.rs       # NUEVO: Query extractors
└── handlers/
    └── config.rs      # Actualizado con handler de label
```

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencia urlencoding

```toml
# Cargo.toml
[dependencies]
urlencoding = "2.1"
```

### Paso 2: Extender Path Extractors

```rust
// src/extractors/path.rs
use serde::Deserialize;

/// Extractor para rutas /{app}/{profile}
#[derive(Debug, Deserialize)]
pub struct AppProfilePath {
    pub app: String,
    pub profile: String,
}

/// Extractor para rutas /{app}/{profile}/{label}
#[derive(Debug, Deserialize)]
pub struct AppProfileLabelPath {
    pub app: String,
    pub profile: String,
    pub label: String,
}

impl AppProfilePath {
    pub fn profiles(&self) -> Vec<String> {
        self.profile
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

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

impl AppProfileLabelPath {
    pub fn profiles(&self) -> Vec<String> {
        self.profile
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect()
    }

    /// Decodifica y sanitiza el label.
    ///
    /// Los labels pueden contener caracteres URL-encoded:
    /// - `feature%2Fx` -> `feature/x`
    /// - `release%2Fv1.0` -> `release/v1.0`
    pub fn sanitized_label(&self) -> String {
        urlencoding::decode(&self.label)
            .map(|s| s.into_owned())
            .unwrap_or_else(|_| self.label.clone())
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.app.trim().is_empty() {
            return Err("Application name cannot be empty".to_string());
        }
        if self.profile.trim().is_empty() {
            return Err("Profile cannot be empty".to_string());
        }
        if self.label.trim().is_empty() {
            return Err("Label cannot be empty".to_string());
        }
        Ok(())
    }
}

// Conversion de AppProfileLabelPath a AppProfilePath
impl From<AppProfileLabelPath> for AppProfilePath {
    fn from(path: AppProfileLabelPath) -> Self {
        Self {
            app: path.app,
            profile: path.profile,
        }
    }
}
```

### Paso 3: Agregar Query Extractor

```rust
// src/extractors/query.rs
use serde::Deserialize;

/// Query parameters opcionales para endpoints de configuracion.
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ConfigQuery {
    /// Si true y el label no existe, usa el label por defecto (main/master).
    #[serde(rename = "useDefaultLabel")]
    pub use_default_label: bool,

    /// Forzar refresh del cache (bypass).
    #[serde(rename = "forceRefresh")]
    pub force_refresh: bool,
}
```

### Paso 4: Implementar Handler con Label

```rust
// src/handlers/config.rs
use axum::{
    extract::{Path, Query},
    Json,
};
use tracing::instrument;

use crate::error::AppError;
use crate::extractors::{
    path::{AppProfilePath, AppProfileLabelPath},
    query::ConfigQuery,
};
use crate::handlers::response::{ConfigResponse, PropertySourceResponse};

/// Handler para GET /{app}/{profile}
#[instrument(skip_all, fields(app = %path.app, profile = %path.profile))]
pub async fn get_config(
    Path(path): Path<AppProfilePath>,
) -> Result<Json<ConfigResponse>, AppError> {
    path.validate().map_err(AppError::BadRequest)?;

    let profiles = path.profiles();
    tracing::info!("Fetching config without label");

    let response = create_mock_response(&path.app, profiles, None);
    Ok(Json(response))
}

/// Handler para GET /{app}/{profile}/{label}
#[instrument(skip_all, fields(
    app = %path.app,
    profile = %path.profile,
    label = %path.label
))]
pub async fn get_config_with_label(
    Path(path): Path<AppProfileLabelPath>,
    Query(query): Query<ConfigQuery>,
) -> Result<Json<ConfigResponse>, AppError> {
    path.validate().map_err(AppError::BadRequest)?;

    let profiles = path.profiles();
    let label = path.sanitized_label();

    tracing::info!(
        use_default_label = query.use_default_label,
        "Fetching config with label"
    );

    // Validar caracteres peligrosos en label
    validate_label(&label)?;

    let response = create_mock_response(&path.app, profiles, Some(label));
    Ok(Json(response))
}

/// Valida que el label no contenga caracteres peligrosos.
fn validate_label(label: &str) -> Result<(), AppError> {
    // Prevenir path traversal
    if label.contains("..") {
        return Err(AppError::BadRequest(
            "Label cannot contain '..'".to_string()
        ));
    }

    // Prevenir caracteres de control
    if label.chars().any(|c| c.is_control()) {
        return Err(AppError::BadRequest(
            "Label cannot contain control characters".to_string()
        ));
    }

    Ok(())
}

fn create_mock_response(
    app: &str,
    profiles: Vec<String>,
    label: Option<String>,
) -> ConfigResponse {
    use std::collections::HashMap;

    let mut source = HashMap::new();
    source.insert(
        "server.port".to_string(),
        serde_json::Value::Number(8080.into()),
    );

    let source_name = match &label {
        Some(l) => format!("git:{}:config/{}.yml", l, app),
        None => format!("file:config/{}.yml", app),
    };

    ConfigResponse {
        name: app.to_string(),
        profiles,
        label,
        version: None,
        state: None,
        property_sources: vec![PropertySourceResponse {
            name: source_name,
            source,
        }],
    }
}
```

### Paso 5: Actualizar Router

```rust
// src/server.rs
use axum::{Router, routing::get};
use crate::handlers::{
    health::health_check,
    config::{get_config, get_config_with_label},
};

pub fn create_router() -> Router {
    Router::new()
        .route("/health", get(health_check))
        // Rutas con mas segmentos primero
        .route("/:app/:profile/:label", get(get_config_with_label))
        .route("/:app/:profile", get(get_config))
}
```

---

## Conceptos de Rust Aprendidos

### 1. Optional Query Parameters con Default

Axum permite extraer query parameters opcionales usando `#[serde(default)]`.

**Rust:**
```rust
use axum::extract::Query;
use serde::Deserialize;

// #[serde(default)] usa Default::default() si el campo no esta presente
#[derive(Debug, Deserialize, Default)]
#[serde(default)]
pub struct ConfigQuery {
    #[serde(rename = "useDefaultLabel")]
    pub use_default_label: bool,  // Default: false

    pub limit: Option<u32>,       // Default: None
}

async fn handler(Query(query): Query<ConfigQuery>) -> String {
    // GET /config -> use_default_label = false, limit = None
    // GET /config?useDefaultLabel=true -> use_default_label = true
    // GET /config?limit=10 -> limit = Some(10)
    format!("{:?}", query)
}
```

**Comparacion con Spring:**
```java
@GetMapping("/config")
public ConfigResponse getConfig(
    // required = false con defaultValue
    @RequestParam(required = false, defaultValue = "false")
    boolean useDefaultLabel,

    // Optional para nullables
    @RequestParam Optional<Integer> limit
) {
    // ...
}
```

**Diferencias clave:**
| Aspecto | Axum/Serde | Spring |
|---------|-----------|--------|
| Default values | `#[serde(default)]` | `defaultValue` |
| Nullable | `Option<T>` | `Optional<T>` o nullable |
| Renaming | `#[serde(rename)]` | `@RequestParam(name)` |
| Validation | Compile-time types | Runtime validation |

### 2. From Trait para Conversiones

El trait `From` permite conversiones explicitas entre tipos.

**Rust:**
```rust
// Definir conversion de AppProfileLabelPath a AppProfilePath
impl From<AppProfileLabelPath> for AppProfilePath {
    fn from(path: AppProfileLabelPath) -> Self {
        Self {
            app: path.app,
            profile: path.profile,
        }
    }
}

// Uso automatico con .into()
let label_path = AppProfileLabelPath {
    app: "myapp".into(),
    profile: "dev".into(),
    label: "main".into(),
};

// Conversion explicita
let path: AppProfilePath = label_path.into();

// O usando From directamente
let path = AppProfilePath::from(label_path);
```

**Comparacion con Java:**
```java
// Java usa constructores o factory methods
public class AppProfilePath {
    public static AppProfilePath from(AppProfileLabelPath labelPath) {
        return new AppProfilePath(labelPath.getApp(), labelPath.getProfile());
    }
}

// O conversion en el mismo constructor
public AppProfilePath(AppProfileLabelPath labelPath) {
    this.app = labelPath.getApp();
    this.profile = labelPath.getProfile();
}
```

**Ventajas del trait From:**
- `Into<T>` se implementa automaticamente si implementas `From<T>`
- Funciona con `?` operator para conversiones de error
- Se integra con el sistema de tipos (generics)

### 3. Cow (Clone on Write)

`Cow` evita clonaciones innecesarias cuando el dato puede ser borrowed o owned.

**Rust:**
```rust
use std::borrow::Cow;

pub fn sanitized_label(&self) -> String {
    // urlencoding::decode retorna Cow<str>
    // - Si no hay cambios: Cow::Borrowed(&str) - sin copia
    // - Si hubo decode: Cow::Owned(String) - nueva string
    urlencoding::decode(&self.label)
        .map(|cow| cow.into_owned())  // Convierte a String
        .unwrap_or_else(|_| self.label.clone())
}

// Ejemplo de uso directo de Cow
fn process_name(name: &str) -> Cow<str> {
    if name.contains(' ') {
        // Necesitamos modificar: crear owned String
        Cow::Owned(name.replace(' ', "_"))
    } else {
        // Sin cambios: solo borrow
        Cow::Borrowed(name)
    }
}

let name1 = process_name("hello");      // Cow::Borrowed
let name2 = process_name("hello world"); // Cow::Owned
```

**Por que no existe en Java:**
Java no tiene el concepto de borrowing. Strings son siempre referencias a objetos en heap, y la JVM maneja la memoria. El equivalente mas cercano seria usar `String.intern()` para strings iguales, pero no es el mismo concepto.

### 4. Validacion de Seguridad en Input

Es crucial validar input del usuario antes de usarlo.

**Rust:**
```rust
/// Valida que el label no contenga patrones peligrosos.
fn validate_label(label: &str) -> Result<(), AppError> {
    // Path traversal
    if label.contains("..") {
        return Err(AppError::BadRequest(
            "Label cannot contain '..'".to_string()
        ));
    }

    // Caracteres de control (newlines, etc)
    if label.chars().any(|c| c.is_control()) {
        return Err(AppError::BadRequest(
            "Label cannot contain control characters".to_string()
        ));
    }

    // Longitud maxima
    if label.len() > 256 {
        return Err(AppError::BadRequest(
            "Label too long (max 256 chars)".to_string()
        ));
    }

    Ok(())
}
```

**Comparacion con Spring (Bean Validation):**
```java
public class LabelPath {
    @NotBlank
    @Size(max = 256)
    @Pattern(regexp = "^[^.]{2}.*$", message = "Cannot contain ..")
    private String label;
}
```

---

## Riesgos y Errores Comunes

### 1. Path Traversal via Label

```rust
// PELIGROSO: El label podria contener "../../../etc/passwd"
async fn bad_handler(Path(path): Path<AppProfileLabelPath>) {
    let file = format!("configs/{}/{}", path.app, path.label);
    std::fs::read_to_string(file); // Path traversal!
}

// SEGURO: Validar y sanitizar
async fn safe_handler(Path(path): Path<AppProfileLabelPath>) -> Result<...> {
    validate_label(&path.label)?;  // Rechaza ".."
    let label = path.sanitized_label();
    // Usar label sanitizado
}
```

### 2. URL Encoding Incorrecto

```rust
// MAL: Asumir que el label ya esta decoded
let label = path.label;  // Podria ser "feature%2Fx"

// BIEN: Siempre decodificar
let label = path.sanitized_label();  // "feature/x"
```

### 3. Orden de Rutas con Path Variables

```rust
// MAL: La ruta de 2 segmentos captura requests para 3 segmentos
Router::new()
    .route("/:app/:profile", get(get_config))
    .route("/:app/:profile/:label", get(get_config_with_label))

// En realidad funciona en Axum, pero es buena practica ordenar
// de mas especifico a menos especifico
Router::new()
    .route("/:app/:profile/:label", get(get_config_with_label))
    .route("/:app/:profile", get(get_config))
```

---

## Pruebas

### Tests Unitarios del Extractor

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sanitized_label_decodes_slash() {
        let path = AppProfileLabelPath {
            app: "app".into(),
            profile: "dev".into(),
            label: "feature%2Fx".into(),
        };
        assert_eq!(path.sanitized_label(), "feature/x");
    }

    #[test]
    fn sanitized_label_handles_plain_label() {
        let path = AppProfileLabelPath {
            app: "app".into(),
            profile: "dev".into(),
            label: "main".into(),
        };
        assert_eq!(path.sanitized_label(), "main");
    }

    #[test]
    fn validate_label_rejects_path_traversal() {
        assert!(validate_label("../secret").is_err());
        assert!(validate_label("config/../../etc").is_err());
    }

    #[test]
    fn validate_label_accepts_normal_labels() {
        assert!(validate_label("main").is_ok());
        assert!(validate_label("feature/new-feature").is_ok());
        assert!(validate_label("v1.2.3").is_ok());
        assert!(validate_label("release-2024.01").is_ok());
    }

    #[test]
    fn validate_label_rejects_control_chars() {
        assert!(validate_label("main\ninjection").is_err());
        assert!(validate_label("main\0null").is_err());
    }
}
```

### Tests de Integracion HTTP

```rust
// tests/config_label_test.rs
use axum::{body::Body, http::{Request, StatusCode}};
use tower::ServiceExt;
use vortex_server::create_router;

#[tokio::test]
async fn get_config_with_label_returns_200() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev/main")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_config_with_label_includes_label_in_response() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev/feature-branch")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(config["label"], "feature-branch");
}

#[tokio::test]
async fn get_config_without_label_has_null_label() {
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
    assert!(config["label"].is_null());
}

#[tokio::test]
async fn get_config_decodes_url_encoded_label() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev/feature%2Fawesome")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();

    let config: serde_json::Value = serde_json::from_slice(&body).unwrap();
    assert_eq!(config["label"], "feature/awesome");
}

#[tokio::test]
async fn get_config_with_query_params() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev/main?useDefaultLabel=true")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn get_config_rejects_path_traversal() {
    let app = create_router();

    let response = app
        .oneshot(
            Request::builder()
                .uri("/myapp/dev/..%2F..%2Fetc%2Fpasswd")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}
```

---

## Observabilidad

### Logging con Contexto

```rust
#[instrument(skip_all, fields(
    app = %path.app,
    profile = %path.profile,
    label = %path.label,
    label_sanitized = tracing::field::Empty
))]
pub async fn get_config_with_label(
    Path(path): Path<AppProfileLabelPath>,
    Query(query): Query<ConfigQuery>,
) -> Result<Json<ConfigResponse>, AppError> {
    let label = path.sanitized_label();

    // Agregar campo calculado al span
    tracing::Span::current().record("label_sanitized", &label.as_str());

    tracing::info!(
        use_default_label = query.use_default_label,
        "Fetching config with label"
    );

    // ...
}
```

### Ejemplo de Log Output

```
INFO get_config_with_label{
    app="myapp"
    profile="dev"
    label="feature%2Fx"
    label_sanitized="feature/x"
}: Fetching config with label use_default_label=false
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `Cargo.toml` - Agregar dependencia `urlencoding`
2. `src/extractors/path.rs` - Agregar `AppProfileLabelPath`
3. `src/extractors/query.rs` - NUEVO: `ConfigQuery`
4. `src/extractors/mod.rs` - Re-export query module
5. `src/handlers/config.rs` - Agregar `get_config_with_label`
6. `src/server.rs` - Agregar ruta `/:app/:profile/:label`
7. `tests/config_label_test.rs` - Tests de integracion

### Verificacion

```bash
# Compilar
cargo build -p vortex-server

# Tests
cargo test -p vortex-server

# Ejecutar servidor
cargo run -p vortex-server

# Probar endpoints
curl http://localhost:8080/myapp/dev | jq '.label'
# null

curl http://localhost:8080/myapp/dev/main | jq '.label'
# "main"

curl http://localhost:8080/myapp/dev/feature%2Fx | jq '.label'
# "feature/x"

curl "http://localhost:8080/myapp/dev/main?useDefaultLabel=true"
# 200 OK
```
