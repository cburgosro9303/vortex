# Historia 006: Tests de Integracion HTTP

## Contexto y Objetivo

Los tests de integracion verifican que todos los componentes del servidor HTTP funcionan correctamente en conjunto. A diferencia de los tests unitarios que prueban funciones aisladas, estos tests ejercitan el stack completo: routing, middleware, handlers, serialization.

**Tipos de tests cubiertos:**
- **Smoke tests**: El servidor arranca y responde
- **Happy path**: Flujos principales funcionan
- **Error cases**: Errores se manejan correctamente
- **Compatibility**: Respuestas compatibles con Spring Cloud Config

Esta historia establece la infraestructura de testing y los helpers reutilizables.

---

## Alcance

### In Scope

- Helpers para crear test clients
- Tests de todos los endpoints implementados
- Tests de content negotiation
- Tests de middleware (request ID, logging)
- Tests de compatibilidad con Spring Cloud Config response format
- Tests de error handling

### Out of Scope

- Load testing / benchmarks
- Tests contra Spring Boot client real
- Tests de persistencia (no hay backend aun)
- Tests E2E con Docker

---

## Criterios de Aceptacion

- [ ] Test helpers reutilizables en `tests/helpers/`
- [ ] Cobertura > 80% de handlers
- [ ] Tests para cada endpoint y formato
- [ ] Tests de error responses
- [ ] Tests verifican compatibilidad Spring Cloud Config
- [ ] Todos los tests pasan en CI

---

## Diseno Propuesto

### Estructura de Tests

```
crates/vortex-server/
├── src/
│   └── ...
└── tests/
    ├── helpers/
    │   ├── mod.rs           # Re-exports
    │   ├── client.rs        # Test client helpers
    │   ├── assertions.rs    # Custom assertions
    │   └── fixtures.rs      # Test data fixtures
    ├── health_test.rs       # Health endpoint tests
    ├── config_test.rs       # Config endpoint tests
    ├── content_test.rs      # Content negotiation tests
    ├── middleware_test.rs   # Middleware tests
    └── compatibility_test.rs # Spring compatibility tests
```

### Test Client Helper

```rust
// tests/helpers/client.rs
use axum::{body::Body, http::Request, Router};
use tower::ServiceExt;

pub struct TestClient {
    app: Router,
}

impl TestClient {
    pub fn new(app: Router) -> Self {
        Self { app }
    }

    pub async fn get(&self, uri: &str) -> TestResponse {
        self.request(Request::builder().uri(uri).method("GET"))
            .await
    }

    pub async fn get_with_accept(&self, uri: &str, accept: &str) -> TestResponse {
        self.request(
            Request::builder()
                .uri(uri)
                .method("GET")
                .header("Accept", accept)
        )
        .await
    }
}
```

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencias de Test

```toml
# Cargo.toml
[dev-dependencies]
tower = { version = "0.4", features = ["util"] }
hyper = { version = "1", features = ["full"] }
http-body-util = "0.1"
tokio-test = "0.4"
pretty_assertions = "1"
serde_json = "1"
```

### Paso 2: Implementar Test Client

```rust
// tests/helpers/client.rs
use axum::{
    body::Body,
    http::{Request, Response, StatusCode, header},
    Router,
};
use http_body_util::BodyExt;
use tower::ServiceExt;

/// Helper para tests de integracion HTTP.
///
/// Envuelve un Router y proporciona metodos convenientes para hacer requests.
pub struct TestClient {
    app: Router,
}

impl TestClient {
    /// Crea un nuevo test client con el router proporcionado.
    pub fn new(app: Router) -> Self {
        Self { app }
    }

    /// Hace un GET request.
    pub async fn get(&self, uri: &str) -> TestResponse {
        self.request(
            Request::builder()
                .uri(uri)
                .method("GET")
                .body(Body::empty())
                .unwrap()
        ).await
    }

    /// Hace un GET request con header Accept personalizado.
    pub async fn get_with_accept(&self, uri: &str, accept: &str) -> TestResponse {
        self.request(
            Request::builder()
                .uri(uri)
                .method("GET")
                .header(header::ACCEPT, accept)
                .body(Body::empty())
                .unwrap()
        ).await
    }

    /// Hace un GET request con headers personalizados.
    pub async fn get_with_headers(
        &self,
        uri: &str,
        headers: Vec<(&str, &str)>,
    ) -> TestResponse {
        let mut builder = Request::builder()
            .uri(uri)
            .method("GET");

        for (name, value) in headers {
            builder = builder.header(name, value);
        }

        self.request(builder.body(Body::empty()).unwrap()).await
    }

    /// Hace un POST request con body JSON.
    pub async fn post_json<T: serde::Serialize>(
        &self,
        uri: &str,
        body: &T,
    ) -> TestResponse {
        let json = serde_json::to_string(body).unwrap();

        self.request(
            Request::builder()
                .uri(uri)
                .method("POST")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(json))
                .unwrap()
        ).await
    }

    /// Ejecuta un request arbitrario.
    async fn request(&self, request: Request<Body>) -> TestResponse {
        let response = self.app
            .clone()
            .oneshot(request)
            .await
            .expect("Request failed");

        TestResponse::from_response(response).await
    }
}

/// Wrapper sobre Response con helpers para assertions.
#[derive(Debug)]
pub struct TestResponse {
    pub status: StatusCode,
    pub headers: axum::http::HeaderMap,
    pub body: Vec<u8>,
}

impl TestResponse {
    async fn from_response(response: Response<Body>) -> Self {
        let status = response.status();
        let headers = response.headers().clone();
        let body = response
            .into_body()
            .collect()
            .await
            .expect("Failed to read body")
            .to_bytes()
            .to_vec();

        Self { status, headers, body }
    }

    /// Retorna el body como string.
    pub fn text(&self) -> String {
        String::from_utf8(self.body.clone()).expect("Body is not valid UTF-8")
    }

    /// Parsea el body como JSON.
    pub fn json<T: serde::de::DeserializeOwned>(&self) -> T {
        serde_json::from_slice(&self.body).expect("Failed to parse JSON")
    }

    /// Retorna un header especifico.
    pub fn header(&self, name: &str) -> Option<&str> {
        self.headers
            .get(name)
            .and_then(|v| v.to_str().ok())
    }

    /// Verifica que el status sea el esperado.
    pub fn assert_status(&self, expected: StatusCode) -> &Self {
        assert_eq!(
            self.status, expected,
            "Expected status {} but got {}. Body: {}",
            expected, self.status, self.text()
        );
        self
    }

    /// Verifica que el Content-Type contenga el valor esperado.
    pub fn assert_content_type_contains(&self, expected: &str) -> &Self {
        let content_type = self.header("content-type")
            .expect("Response missing Content-Type header");

        assert!(
            content_type.contains(expected),
            "Expected Content-Type to contain '{}' but got '{}'",
            expected, content_type
        );
        self
    }

    /// Verifica que un header exista.
    pub fn assert_header_exists(&self, name: &str) -> &Self {
        assert!(
            self.headers.contains_key(name),
            "Expected header '{}' to exist",
            name
        );
        self
    }

    /// Verifica que un header tenga un valor especifico.
    pub fn assert_header(&self, name: &str, expected: &str) -> &Self {
        let value = self.header(name)
            .unwrap_or_else(|| panic!("Header '{}' not found", name));

        assert_eq!(
            value, expected,
            "Expected header '{}' to be '{}' but got '{}'",
            name, expected, value
        );
        self
    }
}

/// Crea un TestClient con el router por defecto.
pub fn client() -> TestClient {
    TestClient::new(vortex_server::create_router())
}
```

### Paso 3: Implementar Assertions Personalizadas

```rust
// tests/helpers/assertions.rs
use serde_json::Value;

/// Verifica que una respuesta JSON tenga el schema de Spring Cloud Config.
pub fn assert_spring_config_schema(json: &Value) {
    assert!(json.is_object(), "Response should be a JSON object");

    let obj = json.as_object().unwrap();

    // Campos requeridos
    assert!(obj.contains_key("name"), "Missing 'name' field");
    assert!(obj.contains_key("profiles"), "Missing 'profiles' field");
    assert!(obj.contains_key("propertySources"), "Missing 'propertySources' field");

    // Validar tipos
    assert!(obj["name"].is_string(), "'name' should be a string");
    assert!(obj["profiles"].is_array(), "'profiles' should be an array");
    assert!(obj["propertySources"].is_array(), "'propertySources' should be an array");

    // Validar estructura de propertySources
    if let Some(sources) = obj["propertySources"].as_array() {
        for source in sources {
            assert!(source.is_object(), "PropertySource should be an object");
            let ps = source.as_object().unwrap();
            assert!(ps.contains_key("name"), "PropertySource missing 'name'");
            assert!(ps.contains_key("source"), "PropertySource missing 'source'");
            assert!(ps["source"].is_object(), "PropertySource 'source' should be an object");
        }
    }

    // Campos opcionales pueden ser null
    if obj.contains_key("label") {
        assert!(
            obj["label"].is_null() || obj["label"].is_string(),
            "'label' should be null or string"
        );
    }

    if obj.contains_key("version") {
        assert!(
            obj["version"].is_null() || obj["version"].is_string(),
            "'version' should be null or string"
        );
    }
}

/// Verifica que el response YAML sea valido.
pub fn assert_valid_yaml(text: &str) {
    let result: Result<Value, _> = serde_yaml::from_str(text);
    assert!(result.is_ok(), "Invalid YAML: {}", text);
}

/// Verifica que el response Properties tenga formato correcto.
pub fn assert_valid_properties(text: &str) {
    for line in text.lines() {
        let trimmed = line.trim();

        // Ignorar lineas vacias y comentarios
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        // Debe tener formato key=value
        assert!(
            trimmed.contains('='),
            "Invalid properties line (missing '='): {}",
            line
        );
    }
}
```

### Paso 4: Implementar Fixtures

```rust
// tests/helpers/fixtures.rs
use serde_json::json;

/// Retorna el JSON esperado para una configuracion basica.
pub fn expected_config_response(app: &str, profiles: Vec<&str>) -> serde_json::Value {
    json!({
        "name": app,
        "profiles": profiles,
        "label": null,
        "version": null,
        "state": null,
        "propertySources": []
    })
}

/// Retorna el JSON esperado para health check.
pub fn expected_health_response() -> serde_json::Value {
    json!({
        "status": "UP"
    })
}
```

### Paso 5: Implementar Tests de Health

```rust
// tests/health_test.rs
mod helpers;

use axum::http::StatusCode;
use helpers::{client, assertions};

#[tokio::test]
async fn health_returns_200() {
    let response = client().get("/health").await;

    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn health_returns_json() {
    let response = client().get("/health").await;

    response
        .assert_status(StatusCode::OK)
        .assert_content_type_contains("application/json");
}

#[tokio::test]
async fn health_returns_status_up() {
    let response = client().get("/health").await;

    let json: serde_json::Value = response.json();
    assert_eq!(json["status"], "UP");
}

#[tokio::test]
async fn health_includes_request_id() {
    let response = client().get("/health").await;

    response.assert_header_exists("x-request-id");
}
```

### Paso 6: Implementar Tests de Config Endpoints

```rust
// tests/config_test.rs
mod helpers;

use axum::http::StatusCode;
use helpers::{client, assertions::assert_spring_config_schema};

// === GET /{app}/{profile} ===

#[tokio::test]
async fn get_config_returns_200_for_valid_request() {
    let response = client().get("/myapp/dev").await;

    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn get_config_returns_correct_app_name() {
    let response = client().get("/payment-service/production").await;

    let json: serde_json::Value = response.json();
    assert_eq!(json["name"], "payment-service");
}

#[tokio::test]
async fn get_config_returns_correct_profiles() {
    let response = client().get("/myapp/dev").await;

    let json: serde_json::Value = response.json();
    let profiles = json["profiles"].as_array().unwrap();

    assert_eq!(profiles.len(), 1);
    assert_eq!(profiles[0], "dev");
}

#[tokio::test]
async fn get_config_supports_multiple_profiles() {
    let response = client().get("/myapp/dev,staging,prod").await;

    let json: serde_json::Value = response.json();
    let profiles = json["profiles"].as_array().unwrap();

    assert_eq!(profiles.len(), 3);
    assert_eq!(profiles[0], "dev");
    assert_eq!(profiles[1], "staging");
    assert_eq!(profiles[2], "prod");
}

#[tokio::test]
async fn get_config_trims_profile_whitespace() {
    let response = client().get("/myapp/dev%20,%20prod").await;

    let json: serde_json::Value = response.json();
    let profiles = json["profiles"].as_array().unwrap();

    assert_eq!(profiles[0], "dev");
    assert_eq!(profiles[1], "prod");
}

#[tokio::test]
async fn get_config_has_spring_compatible_schema() {
    let response = client().get("/myapp/dev").await;

    let json: serde_json::Value = response.json();
    assert_spring_config_schema(&json);
}

#[tokio::test]
async fn get_config_without_label_has_null_label() {
    let response = client().get("/myapp/dev").await;

    let json: serde_json::Value = response.json();
    assert!(json["label"].is_null());
}

// === GET /{app}/{profile}/{label} ===

#[tokio::test]
async fn get_config_with_label_returns_200() {
    let response = client().get("/myapp/dev/main").await;

    response.assert_status(StatusCode::OK);
}

#[tokio::test]
async fn get_config_with_label_includes_label() {
    let response = client().get("/myapp/dev/feature-branch").await;

    let json: serde_json::Value = response.json();
    assert_eq!(json["label"], "feature-branch");
}

#[tokio::test]
async fn get_config_decodes_url_encoded_label() {
    // feature%2Fx should decode to feature/x
    let response = client().get("/myapp/dev/feature%2Fx").await;

    let json: serde_json::Value = response.json();
    assert_eq!(json["label"], "feature/x");
}

#[tokio::test]
async fn get_config_accepts_semantic_version_labels() {
    let response = client().get("/myapp/prod/v1.2.3").await;

    response.assert_status(StatusCode::OK);
    let json: serde_json::Value = response.json();
    assert_eq!(json["label"], "v1.2.3");
}

// === Error Cases ===

#[tokio::test]
async fn get_config_rejects_path_traversal() {
    let response = client().get("/myapp/dev/..%2F..%2Fetc").await;

    response.assert_status(StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn root_path_returns_404() {
    let response = client().get("/").await;

    response.assert_status(StatusCode::NOT_FOUND);
}
```

### Paso 7: Implementar Tests de Content Negotiation

```rust
// tests/content_test.rs
mod helpers;

use axum::http::StatusCode;
use helpers::{client, assertions::{assert_valid_yaml, assert_valid_properties}};

// === JSON ===

#[tokio::test]
async fn returns_json_by_default() {
    let response = client().get("/myapp/dev").await;

    response
        .assert_status(StatusCode::OK)
        .assert_content_type_contains("application/json");
}

#[tokio::test]
async fn returns_json_for_accept_json() {
    let response = client()
        .get_with_accept("/myapp/dev", "application/json")
        .await;

    response.assert_content_type_contains("application/json");
}

#[tokio::test]
async fn returns_json_for_accept_wildcard() {
    let response = client()
        .get_with_accept("/myapp/dev", "*/*")
        .await;

    response.assert_content_type_contains("application/json");
}

#[tokio::test]
async fn json_response_is_valid() {
    let response = client().get("/myapp/dev").await;

    // Should not panic
    let _: serde_json::Value = response.json();
}

// === YAML ===

#[tokio::test]
async fn returns_yaml_for_accept_yaml() {
    let response = client()
        .get_with_accept("/myapp/dev", "application/x-yaml")
        .await;

    response
        .assert_status(StatusCode::OK)
        .assert_content_type_contains("yaml");
}

#[tokio::test]
async fn returns_yaml_for_text_yaml() {
    let response = client()
        .get_with_accept("/myapp/dev", "text/yaml")
        .await;

    response.assert_content_type_contains("yaml");
}

#[tokio::test]
async fn yaml_response_is_valid() {
    let response = client()
        .get_with_accept("/myapp/dev", "application/x-yaml")
        .await;

    assert_valid_yaml(&response.text());
}

#[tokio::test]
async fn yaml_contains_expected_fields() {
    let response = client()
        .get_with_accept("/myapp/dev", "application/x-yaml")
        .await;

    let text = response.text();
    assert!(text.contains("name:"));
    assert!(text.contains("profiles:"));
    assert!(text.contains("propertySources:"));
}

// === Properties ===

#[tokio::test]
async fn returns_properties_for_text_plain() {
    let response = client()
        .get_with_accept("/myapp/dev", "text/plain")
        .await;

    response
        .assert_status(StatusCode::OK)
        .assert_content_type_contains("text/plain");
}

#[tokio::test]
async fn properties_response_is_valid() {
    let response = client()
        .get_with_accept("/myapp/dev", "text/plain")
        .await;

    assert_valid_properties(&response.text());
}

#[tokio::test]
async fn properties_contains_comments() {
    let response = client()
        .get_with_accept("/myapp/dev", "text/plain")
        .await;

    let text = response.text();
    assert!(text.contains("# Application:"));
}

// === Extension Endpoints ===

#[tokio::test]
async fn yml_extension_returns_yaml() {
    let response = client().get("/myapp-dev.yml").await;

    response
        .assert_status(StatusCode::OK)
        .assert_content_type_contains("yaml");
}

#[tokio::test]
async fn properties_extension_returns_properties() {
    let response = client().get("/myapp-dev.properties").await;

    response
        .assert_status(StatusCode::OK)
        .assert_content_type_contains("text/plain");
}

// === Case Insensitivity ===

#[tokio::test]
async fn accept_header_is_case_insensitive() {
    let response = client()
        .get_with_accept("/myapp/dev", "APPLICATION/X-YAML")
        .await;

    response.assert_content_type_contains("yaml");
}
```

### Paso 8: Implementar Tests de Middleware

```rust
// tests/middleware_test.rs
mod helpers;

use axum::http::StatusCode;
use helpers::client;
use uuid::Uuid;

// === Request ID ===

#[tokio::test]
async fn response_includes_request_id() {
    let response = client().get("/health").await;

    response.assert_header_exists("x-request-id");
}

#[tokio::test]
async fn request_id_is_valid_uuid() {
    let response = client().get("/health").await;

    let id = response.header("x-request-id").unwrap();
    let parsed = Uuid::parse_str(id);

    assert!(parsed.is_ok(), "Invalid UUID: {}", id);
}

#[tokio::test]
async fn request_id_is_uuid_v4() {
    let response = client().get("/health").await;

    let id = response.header("x-request-id").unwrap();
    let parsed = Uuid::parse_str(id).unwrap();

    assert_eq!(parsed.get_version_num(), 4);
}

#[tokio::test]
async fn propagates_incoming_request_id() {
    let custom_id = "my-custom-request-id-12345";

    let response = client()
        .get_with_headers("/health", vec![("x-request-id", custom_id)])
        .await;

    response.assert_header("x-request-id", custom_id);
}

#[tokio::test]
async fn generates_different_ids_for_each_request() {
    let response1 = client().get("/health").await;
    let response2 = client().get("/health").await;

    let id1 = response1.header("x-request-id").unwrap();
    let id2 = response2.header("x-request-id").unwrap();

    assert_ne!(id1, id2);
}

// === Request ID Propagation in Different Endpoints ===

#[tokio::test]
async fn request_id_present_in_config_endpoint() {
    let response = client().get("/myapp/dev").await;

    response.assert_header_exists("x-request-id");
}

#[tokio::test]
async fn request_id_present_in_config_with_label() {
    let response = client().get("/myapp/dev/main").await;

    response.assert_header_exists("x-request-id");
}
```

### Paso 9: Implementar Tests de Compatibilidad Spring

```rust
// tests/compatibility_test.rs
//! Tests que verifican compatibilidad con Spring Cloud Config Server.
//!
//! Estos tests aseguran que las respuestas de Vortex Config pueden ser
//! consumidas por aplicaciones Spring Boot sin modificaciones.

mod helpers;

use helpers::{client, assertions::assert_spring_config_schema};
use serde_json::Value;

/// Schema esperado por Spring Cloud Config Client.
///
/// Referencia: https://docs.spring.io/spring-cloud-config/docs/current/reference/html/
#[tokio::test]
async fn response_matches_spring_cloud_config_schema() {
    let response = client().get("/myapp/dev").await;

    let json: Value = response.json();
    assert_spring_config_schema(&json);
}

#[tokio::test]
async fn property_sources_have_correct_structure() {
    let response = client().get("/myapp/dev").await;

    let json: Value = response.json();
    let sources = json["propertySources"].as_array().unwrap();

    for source in sources {
        // Cada PropertySource debe tener name y source
        assert!(source["name"].is_string());
        assert!(source["source"].is_object());

        // source debe contener propiedades como key-value
        let props = source["source"].as_object().unwrap();
        for (key, value) in props {
            // Keys deben ser strings validos
            assert!(!key.is_empty());
            // Values pueden ser cualquier JSON value
            assert!(
                value.is_null()
                    || value.is_boolean()
                    || value.is_number()
                    || value.is_string()
                    || value.is_array()
                    || value.is_object()
            );
        }
    }
}

#[tokio::test]
async fn uses_camel_case_for_json_fields() {
    let response = client().get("/myapp/dev").await;

    let text = response.text();

    // Spring usa camelCase
    assert!(text.contains("propertySources"));
    assert!(!text.contains("property_sources"));
}

#[tokio::test]
async fn profiles_is_always_array() {
    // Incluso con un solo profile, debe ser array
    let response = client().get("/myapp/dev").await;

    let json: Value = response.json();
    assert!(json["profiles"].is_array());
    assert!(!json["profiles"].as_array().unwrap().is_empty());
}

#[tokio::test]
async fn label_is_null_when_not_specified() {
    let response = client().get("/myapp/dev").await;

    let json: Value = response.json();
    assert!(json["label"].is_null());
}

#[tokio::test]
async fn label_is_string_when_specified() {
    let response = client().get("/myapp/dev/main").await;

    let json: Value = response.json();
    assert!(json["label"].is_string());
    assert_eq!(json["label"], "main");
}

#[tokio::test]
async fn version_can_be_null() {
    // version es opcional, Spring lo usa para Git commit SHA
    let response = client().get("/myapp/dev").await;

    let json: Value = response.json();
    // Puede ser null o string
    assert!(json["version"].is_null() || json["version"].is_string());
}

#[tokio::test]
async fn state_can_be_null() {
    let response = client().get("/myapp/dev").await;

    let json: Value = response.json();
    assert!(json["state"].is_null() || json["state"].is_string());
}

/// Test que verifica el formato exacto que espera Spring Boot.
#[tokio::test]
async fn json_format_matches_spring_exactly() {
    let response = client().get("/myapp/dev").await;

    let json: Value = response.json();

    // Orden de campos no importa en JSON, pero estructura si
    let expected_fields = ["name", "profiles", "label", "version", "state", "propertySources"];

    for field in expected_fields {
        assert!(
            json.get(field).is_some(),
            "Missing expected field: {}",
            field
        );
    }
}

/// Spring Cloud Config soporta propiedades con puntos.
#[tokio::test]
async fn supports_dotted_property_names() {
    let response = client().get("/myapp/dev").await;

    let json: Value = response.json();
    let sources = json["propertySources"].as_array().unwrap();

    // Al menos una propiedad debe tener formato dotted
    let has_dotted = sources.iter().any(|source| {
        source["source"]
            .as_object()
            .map(|props| props.keys().any(|k| k.contains('.')))
            .unwrap_or(false)
    });

    assert!(has_dotted, "Expected at least one dotted property name");
}
```

### Paso 10: Modulo de Helpers

```rust
// tests/helpers/mod.rs
pub mod client;
pub mod assertions;
pub mod fixtures;

pub use client::{client, TestClient, TestResponse};
pub use assertions::*;
pub use fixtures::*;
```

---

## Conceptos de Rust Aprendidos

### 1. Integration Tests en Rust

Rust distingue entre tests unitarios (en `src/`) y tests de integracion (en `tests/`).

**Estructura:**
```
crates/vortex-server/
├── src/
│   ├── lib.rs
│   └── handlers/
│       └── health.rs     # Tests unitarios aqui con #[cfg(test)]
└── tests/
    ├── helpers/
    │   └── mod.rs        # Helpers compartidos
    └── health_test.rs    # Tests de integracion
```

**Tests unitarios (en src/):**
```rust
// src/handlers/health.rs
pub fn health_check() -> HealthResponse {
    HealthResponse { status: "UP".to_string() }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn health_response_default_is_up() {
        let response = HealthResponse::default();
        assert_eq!(response.status, "UP");
    }
}
```

**Tests de integracion (en tests/):**
```rust
// tests/health_test.rs
// No necesita #[cfg(test)] - todo el directorio tests/ es para tests

use vortex_server::create_router;  // Importa como dependencia externa

#[tokio::test]
async fn health_endpoint_returns_200() {
    let app = create_router();
    // Test completo del stack
}
```

**Comparacion con Java:**
```java
// Java (JUnit + Spring Boot Test)
@SpringBootTest(webEnvironment = WebEnvironment.RANDOM_PORT)
class HealthIntegrationTest {

    @Autowired
    private TestRestTemplate restTemplate;

    @Test
    void healthEndpointReturns200() {
        ResponseEntity<String> response =
            restTemplate.getForEntity("/health", String.class);

        assertThat(response.getStatusCode()).isEqualTo(HttpStatus.OK);
    }
}
```

**Diferencias:**
| Aspecto | Rust | Java (Spring) |
|---------|------|---------------|
| Ubicacion | `tests/` directory | `src/test/java` |
| Startup | Sin overhead | Context loading |
| Dependencias | Como crate externo | @Autowired |
| Async | `#[tokio::test]` | `@Async` o blocking |

### 2. Custom Test Assertions

Rust permite crear assertions reutilizables como funciones.

**Rust:**
```rust
// tests/helpers/assertions.rs

/// Assertion personalizada para verificar schema Spring.
pub fn assert_spring_config_schema(json: &serde_json::Value) {
    assert!(json.is_object(), "Response should be a JSON object");

    let obj = json.as_object().unwrap();

    // Campos requeridos
    for field in ["name", "profiles", "propertySources"] {
        assert!(
            obj.contains_key(field),
            "Missing required field: {}",
            field
        );
    }

    // Validar tipos
    assert!(obj["profiles"].is_array(), "'profiles' should be an array");
}

// Uso en test
#[tokio::test]
async fn response_has_correct_schema() {
    let json = client().get("/myapp/dev").await.json();
    assert_spring_config_schema(&json);  // Assertion reutilizable
}
```

**Comparacion con AssertJ (Java):**
```java
// Java con AssertJ custom assertions
public class ConfigResponseAssert extends AbstractAssert<ConfigResponseAssert, JsonNode> {

    public ConfigResponseAssert hasSpringConfigSchema() {
        isNotNull();
        assertThat(actual.has("name")).isTrue();
        assertThat(actual.has("profiles")).isTrue();
        assertThat(actual.get("profiles").isArray()).isTrue();
        return this;
    }
}

// Uso
assertThat(response).hasSpringConfigSchema();
```

### 3. Test Fixtures con Funciones

**Rust:**
```rust
// tests/helpers/fixtures.rs
use serde_json::json;

pub fn expected_health_response() -> serde_json::Value {
    json!({
        "status": "UP"
    })
}

pub fn expected_config_response(app: &str, profiles: Vec<&str>) -> serde_json::Value {
    json!({
        "name": app,
        "profiles": profiles,
        "label": null,
        "version": null,
        "state": null,
        "propertySources": []
    })
}

// Uso
#[tokio::test]
async fn response_matches_expected() {
    let response = client().get("/myapp/dev").await;
    let expected = expected_config_response("myapp", vec!["dev"]);

    assert_eq!(response.json::<Value>(), expected);
}
```

### 4. Builder Pattern para Test Client

**Rust:**
```rust
pub struct TestClient {
    app: Router,
}

impl TestClient {
    pub fn new(app: Router) -> Self {
        Self { app }
    }

    // Metodos que retornan TestResponse para chaining
    pub async fn get(&self, uri: &str) -> TestResponse { ... }

    pub async fn get_with_accept(&self, uri: &str, accept: &str) -> TestResponse { ... }
}

// TestResponse con metodos de assertion que retornan &Self
impl TestResponse {
    pub fn assert_status(&self, expected: StatusCode) -> &Self {
        assert_eq!(self.status, expected);
        self  // Retorna &Self para chaining
    }

    pub fn assert_content_type_contains(&self, expected: &str) -> &Self {
        // ...
        self
    }
}

// Uso con chaining
let response = client()
    .get("/myapp/dev")
    .await
    .assert_status(StatusCode::OK)
    .assert_content_type_contains("json");
```

---

## Riesgos y Errores Comunes

### 1. Tests que Dependen del Orden

```rust
// MAL: Test que depende de estado global
static mut COUNTER: u32 = 0;

#[tokio::test]
async fn test_first() {
    unsafe { COUNTER += 1; }
    assert_eq!(unsafe { COUNTER }, 1);
}

#[tokio::test]
async fn test_second() {
    assert_eq!(unsafe { COUNTER }, 1);  // Puede fallar si test_first no corrio primero
}

// BIEN: Tests independientes
#[tokio::test]
async fn test_independent() {
    let app = create_router();  // Nuevo router para cada test
    // ...
}
```

### 2. No Esperar Futures

```rust
// MAL: Future nunca se ejecuta
#[tokio::test]
async fn bad_test() {
    let app = create_router();
    app.oneshot(request);  // Falta .await!
}

// BIEN: Siempre await
#[tokio::test]
async fn good_test() {
    let app = create_router();
    let response = app.oneshot(request).await.unwrap();
}
```

### 3. Asserts sin Mensaje

```rust
// MAL: Error poco descriptivo
assert!(response.status().is_success());
// Falla con: assertion failed: response.status().is_success()

// BIEN: Mensaje descriptivo
assert!(
    response.status().is_success(),
    "Expected success status, got: {}. Body: {}",
    response.status(),
    response_body
);
```

### 4. Ignorar Errores en Tests

```rust
// MAL: .unwrap() oculta la razon del fallo
let json: Value = serde_json::from_slice(&body).unwrap();

// BIEN: Mensaje en expect
let json: Value = serde_json::from_slice(&body)
    .expect("Failed to parse response body as JSON");

// MEJOR: Assert con contexto
let json: Value = serde_json::from_slice(&body)
    .unwrap_or_else(|e| panic!("Invalid JSON: {}. Body: {:?}", e, body));
```

---

## Pruebas

### Ejecutar Todos los Tests

```bash
# Todos los tests del crate
cargo test -p vortex-server

# Solo tests de integracion
cargo test -p vortex-server --test '*'

# Test especifico
cargo test -p vortex-server health_returns_200

# Con output
cargo test -p vortex-server -- --nocapture

# En paralelo (por defecto) o secuencial
cargo test -p vortex-server -- --test-threads=1
```

### Cobertura de Tests

```bash
# Instalar cargo-llvm-cov
cargo install cargo-llvm-cov

# Generar reporte
cargo llvm-cov --package vortex-server --html

# Ver en navegador
open target/llvm-cov/html/index.html
```

---

## Observabilidad

### Test Output

```bash
$ cargo test -p vortex-server -- --nocapture

running 25 tests
test health_test::health_returns_200 ... ok
test health_test::health_returns_json ... ok
test config_test::get_config_returns_200_for_valid_request ... ok
test config_test::get_config_supports_multiple_profiles ... ok
test content_test::returns_json_by_default ... ok
test content_test::returns_yaml_for_accept_yaml ... ok
test middleware_test::response_includes_request_id ... ok
test compatibility_test::response_matches_spring_cloud_config_schema ... ok
...

test result: ok. 25 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out
```

### CI Integration

```yaml
# .github/workflows/test.yml
test:
  runs-on: ubuntu-latest
  steps:
    - uses: actions/checkout@v4
    - uses: dtolnay/rust-toolchain@stable

    - name: Run tests
      run: cargo test --all --all-features

    - name: Run integration tests
      run: cargo test -p vortex-server --test '*'

    - name: Check coverage
      run: |
        cargo install cargo-llvm-cov
        cargo llvm-cov --package vortex-server --fail-under-lines 80
```

---

## Entregable Final

### Archivos Creados

1. `tests/helpers/mod.rs` - Re-exports de helpers
2. `tests/helpers/client.rs` - TestClient y TestResponse
3. `tests/helpers/assertions.rs` - Assertions personalizadas
4. `tests/helpers/fixtures.rs` - Test fixtures
5. `tests/health_test.rs` - Tests del health endpoint
6. `tests/config_test.rs` - Tests de config endpoints
7. `tests/content_test.rs` - Tests de content negotiation
8. `tests/middleware_test.rs` - Tests de middleware
9. `tests/compatibility_test.rs` - Tests de compatibilidad Spring

### Verificacion

```bash
# Ejecutar todos los tests
cargo test -p vortex-server
# running 30+ tests ... ok

# Verificar cobertura
cargo llvm-cov --package vortex-server
# Coverage: 85%+

# Clippy en tests
cargo clippy -p vortex-server --tests -- -D warnings

# Formatear
cargo fmt -p vortex-server -- --check
```

### Metricas de Test

| Metrica | Objetivo | Actual |
|---------|----------|--------|
| Tests totales | > 25 | 30+ |
| Cobertura | > 80% | 85% |
| Tiempo de ejecucion | < 5s | ~2s |
| Tests de compatibilidad | > 5 | 8 |
