# Historia 005: JSON Schema Validation

## Contexto y Objetivo

Ademas del control de acceso via PLAC, la gobernanza de configuraciones requiere validacion estructural. Esta historia implementa validacion de configuraciones contra JSON Schemas antes de servirlas a los clientes.

**Beneficios de Schema Validation:**
- **Contratos claros**: Los schemas documentan la estructura esperada
- **Deteccion temprana**: Errores de configuracion se detectan antes de llegar a produccion
- **Compatibilidad**: Asegurar que nuevas configuraciones no rompen clientes existentes
- **Documentacion**: Los schemas sirven como documentacion viva

Esta historia demuestra el uso del crate `jsonschema` para validacion en compile-time y runtime, patterns de carga de schemas, y manejo de errores de validacion.

---

## Alcance

### In Scope

- Carga de JSON Schemas desde archivos o embebidos
- Compilacion de schemas para validacion eficiente
- Validacion de configuraciones antes de retornar
- Errores de validacion descriptivos
- Registry de schemas por aplicacion/profile
- Cache de schemas compilados

### Out of Scope

- Generacion automatica de schemas desde tipos Rust
- UI para edicion de schemas
- Migracion automatica de configuraciones
- Validacion de schemas YAML (solo JSON Schema)

---

## Criterios de Aceptacion

- [ ] SchemaRegistry carga y cachea schemas JSON
- [ ] Schemas se asocian a patrones app/profile
- [ ] Validacion retorna errores detallados con path del campo
- [ ] Configuraciones validas pasan sin modificacion
- [ ] Configuraciones invalidas retornan 400 con detalles
- [ ] Modo "warn" permite pasar pero loguea warnings
- [ ] Tests cubren schemas validos e invalidos
- [ ] Performance: validacion < 5ms para configs tipicas

---

## Diseno Propuesto

### Arquitectura de Validacion

```
┌─────────────────────────────────────────────────────────────────────┐
│                          Schema Registry                             │
├─────────────────────────────────────────────────────────────────────┤
│                                                                      │
│  schemas/                                                            │
│  ├── payment-service.json      → Schema para payment-service/*     │
│  ├── user-service-prod.json    → Schema para user-service/prod     │
│  └── default.json              → Schema fallback                    │
│                                                                      │
│  ┌───────────────────────────────────────────────────────────────┐ │
│  │                    SchemaRegistry                              │ │
│  │                                                                │ │
│  │  schemas: HashMap<Pattern, CompiledSchema>                     │ │
│  │                                                                │ │
│  │  + load_from_directory(path)                                  │ │
│  │  + get_schema(app, profile) -> Option<&CompiledSchema>        │ │
│  │  + validate(app, profile, config) -> ValidationResult         │ │
│  │                                                                │ │
│  └───────────────────────────────────────────────────────────────┘ │
│                                                                      │
└───────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ validate
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                         Configuration JSON                           │
│                                                                      │
│  {                                                                   │
│    "server.port": 8080,                                             │
│    "database.url": "postgres://...",                                │
│    "database.pool_size": "not a number"  ← Error!                  │
│  }                                                                   │
│                                                                      │
└───────────────────────────────────────────────────────────────────────┘
                                    │
                                    │ result
                                    ▼
┌─────────────────────────────────────────────────────────────────────┐
│                        ValidationResult                              │
│                                                                      │
│  Err([                                                               │
│    ValidationError {                                                 │
│      path: "/database.pool_size",                                   │
│      message: "expected integer, found string",                     │
│      schema_path: "/properties/database.pool_size/type"             │
│    }                                                                 │
│  ])                                                                  │
│                                                                      │
└─────────────────────────────────────────────────────────────────────┘
```

### Estructura de Archivos

```
crates/vortex-governance/src/
├── schema/
│   ├── mod.rs           # Re-exports
│   ├── registry.rs      # SchemaRegistry
│   ├── loader.rs        # Schema loading
│   ├── validator.rs     # Validation logic
│   └── error.rs         # Validation errors
└── ...
```

---

## Pasos de Implementacion

### Paso 1: Definir Errores de Validacion

```rust
// src/schema/error.rs
use std::path::PathBuf;
use thiserror::Error;

/// Errors that can occur during schema operations.
#[derive(Debug, Error)]
pub enum SchemaError {
    #[error("Failed to load schema from {path}: {source}")]
    LoadError {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },

    #[error("Invalid JSON in schema {path}: {message}")]
    InvalidJson {
        path: PathBuf,
        message: String,
    },

    #[error("Invalid JSON Schema in {path}: {message}")]
    InvalidSchema {
        path: PathBuf,
        message: String,
    },

    #[error("No schema found for {app}/{profile}")]
    SchemaNotFound {
        app: String,
        profile: String,
    },
}

/// A single validation error.
#[derive(Debug, Clone)]
pub struct ValidationError {
    /// JSON Pointer path to the invalid field
    pub instance_path: String,

    /// Human-readable error message
    pub message: String,

    /// Path in the schema that caused the error
    pub schema_path: String,
}

impl std::fmt::Display for ValidationError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}: {}", self.instance_path, self.message)
    }
}

/// Result of validating a configuration.
#[derive(Debug)]
pub enum ValidationResult {
    /// Configuration is valid
    Valid,

    /// Configuration has validation errors
    Invalid(Vec<ValidationError>),

    /// No schema available for this app/profile
    NoSchema,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        matches!(self, ValidationResult::Valid)
    }

    pub fn errors(&self) -> Option<&[ValidationError]> {
        match self {
            ValidationResult::Invalid(errors) => Some(errors),
            _ => None,
        }
    }
}
```

### Paso 2: Implementar Schema Loader

```rust
// src/schema/loader.rs
use std::path::{Path, PathBuf};
use std::fs;
use std::collections::HashMap;
use jsonschema::JSONSchema;
use serde_json::Value;
use tracing::{info, warn, debug};

use super::error::SchemaError;

/// Pattern for matching schemas to applications.
#[derive(Debug, Clone, Hash, Eq, PartialEq)]
pub struct SchemaPattern {
    /// Application name or glob pattern
    pub app: String,
    /// Profile name or glob pattern (None = all profiles)
    pub profile: Option<String>,
}

impl SchemaPattern {
    pub fn new(app: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            profile: None,
        }
    }

    pub fn with_profile(app: impl Into<String>, profile: impl Into<String>) -> Self {
        Self {
            app: app.into(),
            profile: Some(profile.into()),
        }
    }

    /// Check if this pattern matches the given app/profile.
    pub fn matches(&self, app: &str, profile: &str) -> bool {
        let app_matches = glob_match(&self.app, app);
        let profile_matches = self.profile
            .as_ref()
            .map(|p| glob_match(p, profile))
            .unwrap_or(true);

        app_matches && profile_matches
    }

    /// Calculate specificity for priority ordering.
    /// More specific patterns have higher scores.
    pub fn specificity(&self) -> u32 {
        let mut score = 0;

        // Exact app match scores higher than wildcard
        if !self.app.contains('*') {
            score += 100;
        }

        // Having a profile is more specific
        if let Some(profile) = &self.profile {
            score += 50;
            if !profile.contains('*') {
                score += 50;
            }
        }

        score
    }
}

/// A compiled JSON Schema ready for validation.
pub struct CompiledSchema {
    /// Original schema JSON
    pub schema_json: Value,
    /// Compiled schema for fast validation
    compiled: JSONSchema,
    /// Source file path
    pub source: PathBuf,
}

impl CompiledSchema {
    /// Compile a JSON Schema from a JSON value.
    pub fn compile(schema_json: Value, source: PathBuf) -> Result<Self, SchemaError> {
        let compiled = JSONSchema::compile(&schema_json)
            .map_err(|e| SchemaError::InvalidSchema {
                path: source.clone(),
                message: e.to_string(),
            })?;

        Ok(Self {
            schema_json,
            compiled,
            source,
        })
    }

    /// Validate a JSON value against this schema.
    pub fn validate(&self, instance: &Value) -> Vec<super::error::ValidationError> {
        let result = self.compiled.validate(instance);

        match result {
            Ok(_) => Vec::new(),
            Err(errors) => errors
                .map(|e| super::error::ValidationError {
                    instance_path: e.instance_path.to_string(),
                    message: e.to_string(),
                    schema_path: e.schema_path.to_string(),
                })
                .collect(),
        }
    }

    /// Check if a value is valid.
    pub fn is_valid(&self, instance: &Value) -> bool {
        self.compiled.is_valid(instance)
    }
}

/// Load schemas from a directory.
pub fn load_schemas_from_directory(
    dir: &Path,
) -> Result<HashMap<SchemaPattern, CompiledSchema>, SchemaError> {
    let mut schemas = HashMap::new();

    let entries = fs::read_dir(dir).map_err(|e| SchemaError::LoadError {
        path: dir.to_path_buf(),
        source: e,
    })?;

    for entry in entries {
        let entry = entry.map_err(|e| SchemaError::LoadError {
            path: dir.to_path_buf(),
            source: e,
        })?;

        let path = entry.path();

        // Skip non-JSON files
        if path.extension().map(|e| e != "json").unwrap_or(true) {
            continue;
        }

        // Parse filename to determine pattern
        let pattern = parse_schema_filename(&path)?;

        // Load and compile schema
        let schema = load_schema_file(&path)?;

        info!(
            pattern = ?pattern,
            path = %path.display(),
            "Loaded schema"
        );

        schemas.insert(pattern, schema);
    }

    Ok(schemas)
}

/// Parse schema filename to determine pattern.
///
/// Naming conventions:
/// - `app-name.json` -> matches app-name/*
/// - `app-name-profile.json` -> matches app-name/profile
/// - `default.json` -> fallback for all
fn parse_schema_filename(path: &Path) -> Result<SchemaPattern, SchemaError> {
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .ok_or_else(|| SchemaError::InvalidSchema {
            path: path.to_path_buf(),
            message: "Invalid filename".to_string(),
        })?;

    if stem == "default" {
        return Ok(SchemaPattern::new("*"));
    }

    // Check for app-profile pattern
    let parts: Vec<&str> = stem.rsplitn(2, '-').collect();

    if parts.len() == 2 && is_profile_name(parts[0]) {
        Ok(SchemaPattern::with_profile(parts[1], parts[0]))
    } else {
        Ok(SchemaPattern::new(stem))
    }
}

/// Check if a string looks like a profile name.
fn is_profile_name(s: &str) -> bool {
    matches!(s, "dev" | "development" | "test" | "staging" | "prod" | "production" | "local")
}

/// Load and compile a schema from file.
fn load_schema_file(path: &Path) -> Result<CompiledSchema, SchemaError> {
    let content = fs::read_to_string(path).map_err(|e| SchemaError::LoadError {
        path: path.to_path_buf(),
        source: e,
    })?;

    let schema_json: Value = serde_json::from_str(&content).map_err(|e| {
        SchemaError::InvalidJson {
            path: path.to_path_buf(),
            message: e.to_string(),
        }
    })?;

    CompiledSchema::compile(schema_json, path.to_path_buf())
}

/// Simple glob matching (supports * wildcard).
fn glob_match(pattern: &str, value: &str) -> bool {
    if pattern == "*" {
        return true;
    }

    if !pattern.contains('*') {
        return pattern == value;
    }

    // Simple prefix/suffix matching
    if pattern.starts_with('*') && pattern.ends_with('*') {
        let inner = &pattern[1..pattern.len()-1];
        return value.contains(inner);
    }

    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len()-1];
        return value.starts_with(prefix);
    }

    if pattern.starts_with('*') {
        let suffix = &pattern[1..];
        return value.ends_with(suffix);
    }

    // For more complex patterns, use glob-match crate
    glob_match::glob_match(pattern, value)
}
```

### Paso 3: Implementar SchemaRegistry

```rust
// src/schema/registry.rs
use std::path::Path;
use std::collections::HashMap;
use std::sync::Arc;
use parking_lot::RwLock;
use serde_json::Value;
use tracing::{info, debug, warn};

use super::loader::{SchemaPattern, CompiledSchema, load_schemas_from_directory};
use super::error::{SchemaError, ValidationResult, ValidationError};

/// Registry of JSON Schemas for configuration validation.
///
/// The registry manages loading, caching, and matching schemas
/// to application/profile combinations.
pub struct SchemaRegistry {
    /// Loaded schemas indexed by pattern
    schemas: HashMap<SchemaPattern, CompiledSchema>,

    /// Cached pattern lookups for performance
    lookup_cache: RwLock<HashMap<(String, String), Option<SchemaPattern>>>,

    /// Validation mode
    mode: ValidationMode,
}

/// Validation mode determines behavior on invalid configs.
#[derive(Debug, Clone, Copy, Default)]
pub enum ValidationMode {
    /// Reject invalid configurations with 400
    #[default]
    Strict,
    /// Allow but log warnings
    Warn,
    /// Skip validation entirely
    Disabled,
}

impl SchemaRegistry {
    /// Create an empty registry.
    pub fn new() -> Self {
        Self {
            schemas: HashMap::new(),
            lookup_cache: RwLock::new(HashMap::new()),
            mode: ValidationMode::default(),
        }
    }

    /// Create registry with validation mode.
    pub fn with_mode(mode: ValidationMode) -> Self {
        Self {
            schemas: HashMap::new(),
            lookup_cache: RwLock::new(HashMap::new()),
            mode,
        }
    }

    /// Load schemas from a directory.
    pub fn load_from_directory(path: &Path) -> Result<Self, SchemaError> {
        let schemas = load_schemas_from_directory(path)?;

        info!(
            schema_count = schemas.len(),
            path = %path.display(),
            "Schema registry initialized"
        );

        Ok(Self {
            schemas,
            lookup_cache: RwLock::new(HashMap::new()),
            mode: ValidationMode::default(),
        })
    }

    /// Set the validation mode.
    pub fn set_mode(&mut self, mode: ValidationMode) {
        self.mode = mode;
    }

    /// Get the best matching schema for an app/profile.
    pub fn get_schema(&self, app: &str, profile: &str) -> Option<&CompiledSchema> {
        // Check cache first
        let cache_key = (app.to_string(), profile.to_string());

        {
            let cache = self.lookup_cache.read();
            if let Some(cached) = cache.get(&cache_key) {
                return cached.as_ref().and_then(|p| self.schemas.get(p));
            }
        }

        // Find best matching pattern
        let best_match = self.schemas
            .keys()
            .filter(|p| p.matches(app, profile))
            .max_by_key(|p| p.specificity());

        // Cache the result
        {
            let mut cache = self.lookup_cache.write();
            cache.insert(cache_key, best_match.cloned());
        }

        best_match.and_then(|p| self.schemas.get(p))
    }

    /// Validate a configuration.
    pub fn validate(
        &self,
        app: &str,
        profile: &str,
        config: &Value,
    ) -> ValidationResult {
        // Check if validation is disabled
        if matches!(self.mode, ValidationMode::Disabled) {
            return ValidationResult::Valid;
        }

        // Find schema
        let schema = match self.get_schema(app, profile) {
            Some(s) => s,
            None => {
                debug!(app = %app, profile = %profile, "No schema found");
                return ValidationResult::NoSchema;
            }
        };

        // Validate
        let errors = schema.validate(config);

        if errors.is_empty() {
            debug!(app = %app, profile = %profile, "Configuration is valid");
            ValidationResult::Valid
        } else {
            if matches!(self.mode, ValidationMode::Warn) {
                warn!(
                    app = %app,
                    profile = %profile,
                    error_count = errors.len(),
                    "Configuration has validation warnings"
                );
            }
            ValidationResult::Invalid(errors)
        }
    }

    /// Get number of loaded schemas.
    pub fn schema_count(&self) -> usize {
        self.schemas.len()
    }

    /// Check if a schema exists for app/profile.
    pub fn has_schema(&self, app: &str, profile: &str) -> bool {
        self.get_schema(app, profile).is_some()
    }

    /// Clear the lookup cache (call after adding/removing schemas).
    pub fn clear_cache(&self) {
        let mut cache = self.lookup_cache.write();
        cache.clear();
    }

    /// Add a schema programmatically.
    pub fn add_schema(
        &mut self,
        pattern: SchemaPattern,
        schema: CompiledSchema,
    ) {
        self.schemas.insert(pattern, schema);
        self.clear_cache();
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::new()
    }
}
```

### Paso 4: Integrar con Middleware

```rust
// src/schema/middleware.rs
use axum::{
    body::Body,
    http::{Request, Response, StatusCode},
    response::IntoResponse,
    Json,
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::{Layer, Service};
use std::task::{Context, Poll};
use std::pin::Pin;
use std::future::Future;

use super::registry::{SchemaRegistry, ValidationMode};
use super::error::ValidationResult;

/// Layer that validates responses against JSON Schemas.
#[derive(Clone)]
pub struct SchemaValidationLayer {
    registry: Arc<SchemaRegistry>,
}

impl SchemaValidationLayer {
    pub fn new(registry: Arc<SchemaRegistry>) -> Self {
        Self { registry }
    }
}

impl<S> Layer<S> for SchemaValidationLayer {
    type Service = SchemaValidationService<S>;

    fn layer(&self, inner: S) -> Self::Service {
        SchemaValidationService {
            inner,
            registry: Arc::clone(&self.registry),
        }
    }
}

/// Service that validates configuration responses.
#[derive(Clone)]
pub struct SchemaValidationService<S> {
    inner: S,
    registry: Arc<SchemaRegistry>,
}

impl<S> Service<Request<Body>> for SchemaValidationService<S>
where
    S: Service<Request<Body>, Response = Response<Body>> + Clone + Send + 'static,
    S::Future: Send + 'static,
{
    type Response = Response<Body>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;

    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }

    fn call(&mut self, request: Request<Body>) -> Self::Future {
        let registry = Arc::clone(&self.registry);
        let mut inner = self.inner.clone();

        // Extract app/profile from path
        let path = request.uri().path().to_string();
        let (app, profile) = extract_app_profile(&path);

        Box::pin(async move {
            // Call inner service
            let response = inner.call(request).await?;

            // Only validate successful responses
            if !response.status().is_success() {
                return Ok(response);
            }

            // Read response body
            let (parts, body) = response.into_parts();
            let bytes = match axum::body::to_bytes(body, usize::MAX).await {
                Ok(b) => b,
                Err(_) => return Ok(Response::from_parts(parts, Body::empty())),
            };

            // Parse as JSON
            let config: Value = match serde_json::from_slice(&bytes) {
                Ok(v) => v,
                Err(_) => {
                    // Not JSON, return as-is
                    return Ok(Response::from_parts(parts, Body::from(bytes)));
                }
            };

            // Validate
            let result = registry.validate(&app, &profile, &config);

            match result {
                ValidationResult::Valid | ValidationResult::NoSchema => {
                    Ok(Response::from_parts(parts, Body::from(bytes)))
                }
                ValidationResult::Invalid(errors) => {
                    // Return validation error response
                    let error_response = json!({
                        "error": "Configuration Validation Failed",
                        "errors": errors.iter().map(|e| {
                            json!({
                                "path": e.instance_path,
                                "message": e.message
                            })
                        }).collect::<Vec<_>>()
                    });

                    Ok((
                        StatusCode::BAD_REQUEST,
                        Json(error_response),
                    ).into_response())
                }
            }
        })
    }
}

fn extract_app_profile(path: &str) -> (String, String) {
    let segments: Vec<&str> = path.split('/').filter(|s| !s.is_empty()).collect();
    let app = segments.get(0).unwrap_or(&"unknown").to_string();
    let profile = segments.get(1).unwrap_or(&"default").to_string();
    (app, profile)
}
```

### Paso 5: Crear Builder para Schemas Programaticos

```rust
// src/schema/builder.rs
use serde_json::{json, Value};
use super::loader::CompiledSchema;
use super::error::SchemaError;
use std::path::PathBuf;

/// Builder for creating JSON Schemas programmatically.
///
/// # Example
/// ```
/// let schema = SchemaBuilder::new()
///     .property("server.port", PropertyType::Integer)
///         .minimum(1024)
///         .maximum(65535)
///         .required()
///     .property("database.url", PropertyType::String)
///         .format("uri")
///         .required()
///     .property("feature.enabled", PropertyType::Boolean)
///         .default(false)
///     .build()?;
/// ```
pub struct SchemaBuilder {
    properties: Vec<PropertyDefinition>,
    required: Vec<String>,
    additional_properties: bool,
}

struct PropertyDefinition {
    name: String,
    property_type: PropertyType,
    format: Option<String>,
    minimum: Option<i64>,
    maximum: Option<i64>,
    min_length: Option<usize>,
    max_length: Option<usize>,
    pattern: Option<String>,
    enum_values: Option<Vec<Value>>,
    default: Option<Value>,
}

#[derive(Clone, Copy)]
pub enum PropertyType {
    String,
    Integer,
    Number,
    Boolean,
    Array,
    Object,
}

impl PropertyType {
    fn as_str(&self) -> &'static str {
        match self {
            PropertyType::String => "string",
            PropertyType::Integer => "integer",
            PropertyType::Number => "number",
            PropertyType::Boolean => "boolean",
            PropertyType::Array => "array",
            PropertyType::Object => "object",
        }
    }
}

impl SchemaBuilder {
    pub fn new() -> Self {
        Self {
            properties: Vec::new(),
            required: Vec::new(),
            additional_properties: true,
        }
    }

    /// Start defining a new property.
    pub fn property(
        mut self,
        name: impl Into<String>,
        property_type: PropertyType,
    ) -> PropertyBuilder {
        PropertyBuilder {
            schema_builder: self,
            definition: PropertyDefinition {
                name: name.into(),
                property_type,
                format: None,
                minimum: None,
                maximum: None,
                min_length: None,
                max_length: None,
                pattern: None,
                enum_values: None,
                default: None,
            },
        }
    }

    /// Disallow additional properties not defined in schema.
    pub fn no_additional_properties(mut self) -> Self {
        self.additional_properties = false;
        self
    }

    /// Build the schema.
    pub fn build(self) -> Result<CompiledSchema, SchemaError> {
        let mut properties = serde_json::Map::new();

        for prop in &self.properties {
            let mut prop_schema = serde_json::Map::new();
            prop_schema.insert("type".to_string(), json!(prop.property_type.as_str()));

            if let Some(format) = &prop.format {
                prop_schema.insert("format".to_string(), json!(format));
            }
            if let Some(min) = prop.minimum {
                prop_schema.insert("minimum".to_string(), json!(min));
            }
            if let Some(max) = prop.maximum {
                prop_schema.insert("maximum".to_string(), json!(max));
            }
            if let Some(min_len) = prop.min_length {
                prop_schema.insert("minLength".to_string(), json!(min_len));
            }
            if let Some(max_len) = prop.max_length {
                prop_schema.insert("maxLength".to_string(), json!(max_len));
            }
            if let Some(pattern) = &prop.pattern {
                prop_schema.insert("pattern".to_string(), json!(pattern));
            }
            if let Some(values) = &prop.enum_values {
                prop_schema.insert("enum".to_string(), json!(values));
            }
            if let Some(default) = &prop.default {
                prop_schema.insert("default".to_string(), default.clone());
            }

            properties.insert(prop.name.clone(), Value::Object(prop_schema));
        }

        let schema_json = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": properties,
            "required": self.required,
            "additionalProperties": self.additional_properties
        });

        CompiledSchema::compile(schema_json, PathBuf::from("<programmatic>"))
    }
}

impl Default for SchemaBuilder {
    fn default() -> Self {
        Self::new()
    }
}

/// Builder for individual properties.
pub struct PropertyBuilder {
    schema_builder: SchemaBuilder,
    definition: PropertyDefinition,
}

impl PropertyBuilder {
    /// Mark this property as required.
    pub fn required(mut self) -> Self {
        self.schema_builder.required.push(self.definition.name.clone());
        self
    }

    /// Set format (e.g., "uri", "email", "date-time").
    pub fn format(mut self, format: impl Into<String>) -> Self {
        self.definition.format = Some(format.into());
        self
    }

    /// Set minimum value for numbers.
    pub fn minimum(mut self, min: i64) -> Self {
        self.definition.minimum = Some(min);
        self
    }

    /// Set maximum value for numbers.
    pub fn maximum(mut self, max: i64) -> Self {
        self.definition.maximum = Some(max);
        self
    }

    /// Set minimum length for strings.
    pub fn min_length(mut self, len: usize) -> Self {
        self.definition.min_length = Some(len);
        self
    }

    /// Set maximum length for strings.
    pub fn max_length(mut self, len: usize) -> Self {
        self.definition.max_length = Some(len);
        self
    }

    /// Set regex pattern for strings.
    pub fn pattern(mut self, pattern: impl Into<String>) -> Self {
        self.definition.pattern = Some(pattern.into());
        self
    }

    /// Set allowed enum values.
    pub fn enum_values(mut self, values: Vec<Value>) -> Self {
        self.definition.enum_values = Some(values);
        self
    }

    /// Set default value.
    pub fn default(mut self, value: Value) -> Self {
        self.definition.default = Some(value);
        self
    }

    /// Finish this property and continue with schema builder.
    pub fn property(
        mut self,
        name: impl Into<String>,
        property_type: PropertyType,
    ) -> PropertyBuilder {
        self.schema_builder.properties.push(self.definition);
        self.schema_builder.property(name, property_type)
    }

    /// Build the schema.
    pub fn build(mut self) -> Result<CompiledSchema, SchemaError> {
        self.schema_builder.properties.push(self.definition);
        self.schema_builder.build()
    }
}
```

---

## Conceptos de Rust Aprendidos

### 1. jsonschema Crate

El crate `jsonschema` proporciona validacion de JSON Schema en Rust.

**Rust:**
```rust
use jsonschema::JSONSchema;
use serde_json::json;

// Compilar schema una vez
let schema_json = json!({
    "type": "object",
    "properties": {
        "port": { "type": "integer", "minimum": 1024 }
    },
    "required": ["port"]
});

let compiled = JSONSchema::compile(&schema_json)
    .expect("Invalid schema");

// Validar multiples instancias
let valid = json!({ "port": 8080 });
let invalid = json!({ "port": "not a number" });

assert!(compiled.is_valid(&valid));
assert!(!compiled.is_valid(&invalid));

// Obtener errores detallados
let result = compiled.validate(&invalid);
if let Err(errors) = result {
    for error in errors {
        println!("Path: {}", error.instance_path);
        println!("Error: {}", error);
    }
}
```

**Java (networknt/json-schema-validator):**
```java
import com.networknt.schema.*;

ObjectMapper mapper = new ObjectMapper();

// Parse schema
JsonNode schemaNode = mapper.readTree(schemaJson);
JsonSchema schema = JsonSchemaFactory
    .getInstance(SpecVersion.VersionFlag.V202012)
    .getSchema(schemaNode);

// Validate
JsonNode data = mapper.readTree(instanceJson);
Set<ValidationMessage> errors = schema.validate(data);

if (errors.isEmpty()) {
    System.out.println("Valid!");
} else {
    for (ValidationMessage error : errors) {
        System.out.println(error.getPath() + ": " + error.getMessage());
    }
}
```

### 2. Patron de Cache con RwLock

Cachear resultados de busquedas costosas.

**Rust:**
```rust
use parking_lot::RwLock;
use std::collections::HashMap;

pub struct SchemaRegistry {
    schemas: HashMap<Pattern, Schema>,
    // Cache de lookups para evitar busqueda repetida
    cache: RwLock<HashMap<(String, String), Option<Pattern>>>,
}

impl SchemaRegistry {
    pub fn get_schema(&self, app: &str, profile: &str) -> Option<&Schema> {
        let key = (app.to_string(), profile.to_string());

        // Intentar leer del cache (read lock, permite concurrencia)
        {
            let cache = self.cache.read();
            if let Some(cached) = cache.get(&key) {
                return cached.as_ref().and_then(|p| self.schemas.get(p));
            }
        }

        // Cache miss - buscar el patron
        let pattern = self.find_matching_pattern(app, profile);

        // Actualizar cache (write lock, exclusivo)
        {
            let mut cache = self.cache.write();
            cache.insert(key, pattern.clone());
        }

        pattern.and_then(|p| self.schemas.get(&p))
    }
}
```

**Java:**
```java
public class SchemaRegistry {
    private final Map<Pattern, Schema> schemas;
    private final ConcurrentMap<Pair<String,String>, Optional<Pattern>> cache
        = new ConcurrentHashMap<>();

    public Schema getSchema(String app, String profile) {
        var key = Pair.of(app, profile);

        // computeIfAbsent es atomico
        Optional<Pattern> pattern = cache.computeIfAbsent(key, k ->
            Optional.ofNullable(findMatchingPattern(app, profile))
        );

        return pattern.map(schemas::get).orElse(null);
    }
}
```

### 3. Builder Pattern con Chaining Fluido

Builder con metodos que se encadenan de forma natural.

**Rust:**
```rust
// PropertyBuilder permite encadenar y luego volver a SchemaBuilder
pub struct PropertyBuilder {
    schema_builder: SchemaBuilder,  // Toma ownership
    definition: PropertyDefinition,
}

impl PropertyBuilder {
    // Metodos que modifican y retornan Self
    pub fn required(mut self) -> Self {
        self.schema_builder.required.push(self.definition.name.clone());
        self
    }

    pub fn minimum(mut self, min: i64) -> Self {
        self.definition.minimum = Some(min);
        self
    }

    // Permite cambiar a otra property
    pub fn property(mut self, name: &str, typ: PropertyType) -> PropertyBuilder {
        // Guarda la property actual
        self.schema_builder.properties.push(self.definition);
        // Inicia nueva property
        self.schema_builder.property(name, typ)
    }

    // Termina y construye
    pub fn build(mut self) -> Result<Schema, Error> {
        self.schema_builder.properties.push(self.definition);
        self.schema_builder.build()
    }
}

// Uso fluido
let schema = SchemaBuilder::new()
    .property("port", PropertyType::Integer)
        .minimum(1024)
        .maximum(65535)
        .required()
    .property("host", PropertyType::String)
        .default(json!("localhost"))
    .property("tls", PropertyType::Boolean)
        .default(json!(false))
    .build()?;
```

### 4. Glob Matching Simple

Implementar glob matching basico.

**Rust:**
```rust
/// Simple glob matching (supports * wildcard).
fn glob_match(pattern: &str, value: &str) -> bool {
    // Caso especial: * matchea todo
    if pattern == "*" {
        return true;
    }

    // Sin wildcards: comparacion exacta
    if !pattern.contains('*') {
        return pattern == value;
    }

    // *algo* - contains
    if pattern.starts_with('*') && pattern.ends_with('*') && pattern.len() > 2 {
        let inner = &pattern[1..pattern.len()-1];
        return value.contains(inner);
    }

    // algo* - starts with
    if pattern.ends_with('*') {
        let prefix = &pattern[..pattern.len()-1];
        return value.starts_with(prefix);
    }

    // *algo - ends with
    if pattern.starts_with('*') {
        let suffix = &pattern[1..];
        return value.ends_with(suffix);
    }

    // Patrones mas complejos: usar crate glob-match
    glob_match::glob_match(pattern, value)
}
```

---

## Riesgos y Errores Comunes

### 1. Schema Demasiado Estricto

```json
// MAL: Rechaza configuraciones validas pero no previstas
{
    "type": "object",
    "properties": {
        "known.property": { "type": "string" }
    },
    "additionalProperties": false  // Rechaza cualquier otra propiedad!
}

// BIEN: Permitir propiedades adicionales
{
    "type": "object",
    "properties": {
        "known.property": { "type": "string" }
    },
    "additionalProperties": true  // Default, permite extension
}
```

### 2. No Cachear Schema Compilado

```rust
// MAL: Compilar schema en cada request
fn validate(&self, config: &Value) -> bool {
    let schema_json = load_schema_json();
    let compiled = JSONSchema::compile(&schema_json).unwrap();  // Costoso!
    compiled.is_valid(config)
}

// BIEN: Compilar una vez, reutilizar
struct CompiledSchema {
    compiled: JSONSchema,  // Pre-compilado
}

impl CompiledSchema {
    fn validate(&self, config: &Value) -> bool {
        self.compiled.is_valid(config)  // Rapido
    }
}
```

### 3. Errores Sin Contexto

```rust
// MAL: Error generico
fn validate(&self, config: &Value) -> Result<(), &'static str> {
    if !self.schema.is_valid(config) {
        return Err("Invalid configuration");
    }
    Ok(())
}

// BIEN: Errores detallados
fn validate(&self, config: &Value) -> ValidationResult {
    let errors: Vec<_> = self.schema.validate(config)
        .err()
        .map(|e| e.map(|err| ValidationError {
            path: err.instance_path.to_string(),
            message: err.to_string(),
        }).collect())
        .unwrap_or_default();

    if errors.is_empty() {
        ValidationResult::Valid
    } else {
        ValidationResult::Invalid(errors)
    }
}
```

### 4. Bloquear en Validacion de Schemas Grandes

```rust
// MAL: Validacion sincrona de schema grande
async fn handler(/* ... */) -> Response {
    let config = get_config().await;
    // Bloquea el thread del executor!
    let result = schema.validate(&config);
    // ...
}

// BIEN: Usar spawn_blocking para CPU-intensive
async fn handler(/* ... */) -> Response {
    let config = get_config().await;
    let schema = Arc::clone(&self.schema);

    let result = tokio::task::spawn_blocking(move || {
        schema.validate(&config)
    }).await?;
    // ...
}
```

---

## Pruebas

### Tests de Validacion

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn create_test_schema() -> CompiledSchema {
        let schema_json = json!({
            "$schema": "https://json-schema.org/draft/2020-12/schema",
            "type": "object",
            "properties": {
                "server.port": {
                    "type": "integer",
                    "minimum": 1024,
                    "maximum": 65535
                },
                "database.url": {
                    "type": "string",
                    "format": "uri"
                },
                "feature.enabled": {
                    "type": "boolean"
                }
            },
            "required": ["server.port"]
        });

        CompiledSchema::compile(schema_json, PathBuf::from("test.json")).unwrap()
    }

    #[test]
    fn test_valid_config() {
        let schema = create_test_schema();
        let config = json!({
            "server.port": 8080,
            "database.url": "postgres://localhost/db",
            "feature.enabled": true
        });

        assert!(schema.is_valid(&config));
        assert!(schema.validate(&config).is_empty());
    }

    #[test]
    fn test_missing_required_field() {
        let schema = create_test_schema();
        let config = json!({
            "database.url": "postgres://localhost/db"
        });

        let errors = schema.validate(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.message.contains("required")));
    }

    #[test]
    fn test_wrong_type() {
        let schema = create_test_schema();
        let config = json!({
            "server.port": "not a number"
        });

        let errors = schema.validate(&config);
        assert!(!errors.is_empty());
        assert!(errors.iter().any(|e| e.instance_path.contains("port")));
    }

    #[test]
    fn test_value_out_of_range() {
        let schema = create_test_schema();

        let too_low = json!({ "server.port": 80 });
        let too_high = json!({ "server.port": 70000 });

        assert!(!schema.is_valid(&too_low));
        assert!(!schema.is_valid(&too_high));
    }

    #[test]
    fn test_additional_properties_allowed() {
        let schema = create_test_schema();
        let config = json!({
            "server.port": 8080,
            "custom.property": "allowed"
        });

        assert!(schema.is_valid(&config));
    }
}
```

### Tests del Registry

```rust
#[cfg(test)]
mod registry_tests {
    use super::*;
    use tempfile::TempDir;
    use std::fs;

    fn create_test_registry() -> (TempDir, SchemaRegistry) {
        let dir = TempDir::new().unwrap();

        // Create test schemas
        let payment_schema = json!({
            "type": "object",
            "properties": {
                "payment.gateway": { "type": "string" }
            }
        });
        fs::write(
            dir.path().join("payment-service.json"),
            serde_json::to_string(&payment_schema).unwrap()
        ).unwrap();

        let default_schema = json!({
            "type": "object"
        });
        fs::write(
            dir.path().join("default.json"),
            serde_json::to_string(&default_schema).unwrap()
        ).unwrap();

        let registry = SchemaRegistry::load_from_directory(dir.path()).unwrap();
        (dir, registry)
    }

    #[test]
    fn test_specific_schema_match() {
        let (_dir, registry) = create_test_registry();

        let schema = registry.get_schema("payment-service", "prod");
        assert!(schema.is_some());
    }

    #[test]
    fn test_fallback_to_default() {
        let (_dir, registry) = create_test_registry();

        let schema = registry.get_schema("unknown-service", "dev");
        assert!(schema.is_some());  // Falls back to default.json
    }

    #[test]
    fn test_schema_specificity() {
        let dir = TempDir::new().unwrap();

        // More specific schema
        let prod_schema = json!({
            "type": "object",
            "properties": {
                "special": { "type": "string" }
            }
        });
        fs::write(
            dir.path().join("myapp-prod.json"),
            serde_json::to_string(&prod_schema).unwrap()
        ).unwrap();

        // General schema
        let general_schema = json!({
            "type": "object"
        });
        fs::write(
            dir.path().join("myapp.json"),
            serde_json::to_string(&general_schema).unwrap()
        ).unwrap();

        let registry = SchemaRegistry::load_from_directory(dir.path()).unwrap();

        // prod should get specific schema
        let prod = registry.get_schema("myapp", "prod").unwrap();
        assert!(prod.schema_json["properties"]["special"].is_object());

        // dev should get general schema
        let dev = registry.get_schema("myapp", "dev").unwrap();
        assert!(dev.schema_json["properties"].is_null());
    }
}
```

---

## Seguridad

- **No exponer schemas**: Los schemas no deben ser accesibles via API
- **Validar schemas**: Validar que los schemas cargados son validos JSON Schema
- **Limit de tamano**: Limitar tamano de configuraciones a validar
- **Timeout**: Implementar timeout en validacion de schemas complejos
- **Error messages**: No revelar estructura interna en errores de validacion

---

## Entregable Final

### Archivos Creados

1. `src/schema/mod.rs` - Re-exports
2. `src/schema/error.rs` - Tipos de error
3. `src/schema/loader.rs` - Carga de schemas
4. `src/schema/registry.rs` - SchemaRegistry
5. `src/schema/middleware.rs` - Layer de validacion
6. `src/schema/builder.rs` - SchemaBuilder

### Ejemplo de Schema

```json
// schemas/payment-service.json
{
    "$schema": "https://json-schema.org/draft/2020-12/schema",
    "$id": "https://vortex.example.com/schemas/payment-service",
    "title": "Payment Service Configuration",
    "description": "Schema for payment-service configuration",
    "type": "object",
    "properties": {
        "server.port": {
            "type": "integer",
            "minimum": 1024,
            "maximum": 65535,
            "default": 8080
        },
        "payment.gateway.url": {
            "type": "string",
            "format": "uri"
        },
        "payment.gateway.timeout": {
            "type": "integer",
            "minimum": 1000,
            "maximum": 30000,
            "default": 5000
        },
        "payment.retry.max_attempts": {
            "type": "integer",
            "minimum": 0,
            "maximum": 10,
            "default": 3
        },
        "payment.currencies": {
            "type": "array",
            "items": {
                "type": "string",
                "pattern": "^[A-Z]{3}$"
            },
            "default": ["USD", "EUR"]
        }
    },
    "required": ["payment.gateway.url"]
}
```

### Verificacion

```bash
# Compilar
cargo build -p vortex-governance

# Tests
cargo test -p vortex-governance -- schema

# Validar schema manualmente
cargo run -p vortex-governance -- validate-schema \
    --schema schemas/payment-service.json \
    --config config/payment-service/prod.json

# Output esperado (valido):
# Schema: schemas/payment-service.json
# Config: config/payment-service/prod.json
# Result: VALID

# Output esperado (invalido):
# Schema: schemas/payment-service.json
# Config: config/payment-service/prod.json
# Result: INVALID
# Errors:
#   - /server.port: expected integer, found string
#   - /payment.gateway.url: missing required property
```
