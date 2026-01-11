# Historia 005: Sistema de Errores con thiserror

## Contexto y Objetivo

El manejo de errores en Rust es fundamentalmente diferente a Java. En lugar de excepciones que se propagan implicitamente, Rust usa tipos `Result<T, E>` que fuerzan al programador a manejar los errores explicitamente.

Esta historia establece la jerarquia de errores del dominio de Vortex Config usando la libreria `thiserror`, que simplifica la creacion de tipos de error idiomaticos.

Para un desarrollador Java, piensa en esto como crear una jerarquia de excepciones custom, pero donde el compilador te obliga a manejar cada posible error.

## Alcance

### In Scope
- Definir enum `VortexError` con variantes para cada tipo de error
- Implementar `std::error::Error` via thiserror
- Crear type alias `Result<T>` para el proyecto
- Manejar errores de configuracion, validacion, y fuentes
- Documentar patrones de uso

### Out of Scope
- Errores de red/HTTP (futuras epicas)
- Logging/tracing de errores
- Metricas de errores

## Criterios de Aceptacion

- [ ] Enum `VortexError` con al menos 5 variantes
- [ ] Todas las variantes tienen mensajes descriptivos
- [ ] Type alias `Result<T>` definido
- [ ] Errores implementan `std::error::Error` via thiserror
- [ ] Errores soportan encadenamiento (`source()`)
- [ ] Tests para conversion y display de errores
- [ ] Documentacion de patrones de uso

## Diseno Propuesto

### Modulos Implicados

```
crates/vortex-core/src/
├── lib.rs          # Re-export de error types
├── error.rs        # VortexError, Result type alias
└── ...
```

### Jerarquia de Errores

```
VortexError
├── ConfigNotFound          # Configuración no existe
├── InvalidApplication      # Nombre de app inválido
├── InvalidProfile          # Perfil inválido
├── InvalidLabel            # Label inválido
├── PropertyNotFound        # Propiedad no encontrada
├── ParseError              # Error parseando configuración
├── SourceError             # Error de backend/fuente
└── ValidationError         # Error de validación
```

### Interfaces Propuestas

```rust
// error.rs

use thiserror::Error;

#[derive(Debug, Error)]
pub enum VortexError {
    #[error("Configuration not found: {application}/{profile}/{label}")]
    ConfigNotFound {
        application: String,
        profile: String,
        label: Option<String>,
    },

    #[error("Invalid application name: {0}")]
    InvalidApplication(String),

    // ... más variantes
}

/// Result type alias for Vortex operations.
pub type Result<T> = std::result::Result<T, VortexError>;
```

## Pasos de Implementacion

### Paso 1: Agregar dependencia thiserror

```toml
# crates/vortex-core/Cargo.toml
[dependencies]
thiserror = "1.0"
```

### Paso 2: Crear error.rs

```rust
// crates/vortex-core/src/error.rs

//! Error types for Vortex Config.
//!
//! This module defines the error hierarchy used throughout
//! the Vortex Config system. All errors implement the standard
//! `std::error::Error` trait via `thiserror`.
//!
//! # Error Handling Philosophy
//!
//! Vortex follows Rust's explicit error handling approach:
//! - Functions that can fail return `Result<T, VortexError>`
//! - Errors are values, not control flow
//! - Errors should be handled at appropriate boundaries
//!
//! # Example
//!
//! ```
//! use vortex_core::{Result, VortexError};
//!
//! fn get_config(app: &str) -> Result<String> {
//!     if app.is_empty() {
//!         return Err(VortexError::InvalidApplication(
//!             "Application name cannot be empty".into()
//!         ));
//!     }
//!     Ok(format!("Config for {}", app))
//! }
//!
//! match get_config("myapp") {
//!     Ok(config) => println!("Got config: {}", config),
//!     Err(e) => eprintln!("Error: {}", e),
//! }
//! ```

use std::io;
use thiserror::Error;

/// Main error type for Vortex Config operations.
///
/// This enum covers all error conditions that can occur when
/// working with configuration in Vortex. Each variant includes
/// context information to help diagnose the issue.
#[derive(Debug, Error)]
pub enum VortexError {
    /// Configuration was not found for the given coordinates.
    #[error(
        "Configuration not found for application '{application}', \
         profile '{profile}', label '{}'",
        label.as_deref().unwrap_or("default")
    )]
    ConfigNotFound {
        /// Application name that was requested
        application: String,
        /// Profile that was requested
        profile: String,
        /// Label (version) that was requested, if any
        label: Option<String>,
    },

    /// Application name is invalid or empty.
    #[error("Invalid application name: {reason}")]
    InvalidApplication {
        /// The invalid name provided
        name: String,
        /// Why it's invalid
        reason: String,
    },

    /// Profile name is invalid.
    #[error("Invalid profile name '{name}': {reason}")]
    InvalidProfile {
        /// The invalid profile name
        name: String,
        /// Why it's invalid
        reason: String,
    },

    /// Label (version/branch) is invalid.
    #[error("Invalid label '{name}': {reason}")]
    InvalidLabel {
        /// The invalid label
        name: String,
        /// Why it's invalid
        reason: String,
    },

    /// A required property was not found.
    #[error("Property '{key}' not found in configuration")]
    PropertyNotFound {
        /// The key that was requested
        key: String,
    },

    /// Error parsing configuration content.
    #[error("Failed to parse configuration from '{source}': {message}")]
    ParseError {
        /// Source of the configuration (filename, URL, etc.)
        source: String,
        /// Description of the parse error
        message: String,
        /// Underlying error, if any
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Error accessing a configuration source/backend.
    #[error("Source error for '{source_name}': {message}")]
    SourceError {
        /// Name of the source that failed
        source_name: String,
        /// Description of what went wrong
        message: String,
        /// Underlying error
        #[source]
        cause: Option<Box<dyn std::error::Error + Send + Sync>>,
    },

    /// Validation error for configuration values.
    #[error("Validation error: {message}")]
    ValidationError {
        /// Field that failed validation
        field: String,
        /// Description of the validation failure
        message: String,
    },

    /// I/O error occurred.
    #[error("I/O error: {0}")]
    Io(#[from] io::Error),

    /// Generic internal error.
    #[error("Internal error: {0}")]
    Internal(String),
}

impl VortexError {
    // Convenience constructors

    /// Creates a ConfigNotFound error.
    pub fn config_not_found(
        application: impl Into<String>,
        profile: impl Into<String>,
        label: Option<String>,
    ) -> Self {
        Self::ConfigNotFound {
            application: application.into(),
            profile: profile.into(),
            label,
        }
    }

    /// Creates an InvalidApplication error.
    pub fn invalid_application(
        name: impl Into<String>,
        reason: impl Into<String>,
    ) -> Self {
        Self::InvalidApplication {
            name: name.into(),
            reason: reason.into(),
        }
    }

    /// Creates a PropertyNotFound error.
    pub fn property_not_found(key: impl Into<String>) -> Self {
        Self::PropertyNotFound { key: key.into() }
    }

    /// Creates a ParseError.
    pub fn parse_error(
        source: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::ParseError {
            source: source.into(),
            message: message.into(),
            cause: None,
        }
    }

    /// Creates a ParseError with a cause.
    pub fn parse_error_with_cause<E>(
        source: impl Into<String>,
        message: impl Into<String>,
        cause: E,
    ) -> Self
    where
        E: std::error::Error + Send + Sync + 'static,
    {
        Self::ParseError {
            source: source.into(),
            message: message.into(),
            cause: Some(Box::new(cause)),
        }
    }

    /// Creates a SourceError.
    pub fn source_error(
        source_name: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::SourceError {
            source_name: source_name.into(),
            message: message.into(),
            cause: None,
        }
    }

    /// Creates a ValidationError.
    pub fn validation_error(
        field: impl Into<String>,
        message: impl Into<String>,
    ) -> Self {
        Self::ValidationError {
            field: field.into(),
            message: message.into(),
        }
    }

    /// Returns true if this error indicates the config was not found.
    pub fn is_not_found(&self) -> bool {
        matches!(self, Self::ConfigNotFound { .. })
    }

    /// Returns true if this is a validation error.
    pub fn is_validation_error(&self) -> bool {
        matches!(self, Self::ValidationError { .. })
    }
}

/// Type alias for Results with VortexError.
///
/// Use this type for all Vortex operations that can fail.
///
/// # Example
///
/// ```
/// use vortex_core::Result;
///
/// fn process_config(name: &str) -> Result<()> {
///     // Implementation
///     Ok(())
/// }
/// ```
pub type Result<T> = std::result::Result<T, VortexError>;
```

### Paso 3: Actualizar lib.rs

```rust
// Agregar al lib.rs existente

mod error;

pub use error::{Result, VortexError};
```

### Paso 4: Integrar con modelo de dominio

```rust
// Ejemplo de uso en config.rs

use crate::error::{Result, VortexError};

impl ConfigMap {
    /// Gets a required property, returning error if not found.
    pub fn require_property(&self, key: &str) -> Result<&str> {
        self.get_property(key)
            .ok_or_else(|| VortexError::property_not_found(key))
    }
}

impl Application {
    /// Creates a new Application, validating the name.
    pub fn try_new(name: impl Into<String>) -> Result<Self> {
        let name = name.into();
        if name.is_empty() {
            return Err(VortexError::invalid_application(
                &name,
                "name cannot be empty",
            ));
        }
        if name.contains('/') || name.contains('\\') {
            return Err(VortexError::invalid_application(
                &name,
                "name cannot contain path separators",
            ));
        }
        Ok(Self(name))
    }
}
```

## Conceptos de Rust Aprendidos

### Result y Option

En Rust, los errores se manejan con tipos, no con excepciones.

```rust
// Result<T, E> - Operación que puede fallar
enum Result<T, E> {
    Ok(T),   // Éxito con valor de tipo T
    Err(E),  // Error con valor de tipo E
}

// Option<T> - Valor que puede estar ausente
enum Option<T> {
    Some(T), // Hay un valor
    None,    // No hay valor
}

// Uso básico
fn divide(a: i32, b: i32) -> Result<i32, String> {
    if b == 0 {
        Err("Division by zero".to_string())
    } else {
        Ok(a / b)
    }
}

fn find_user(id: u32) -> Option<User> {
    if id == 0 {
        None
    } else {
        Some(User { id, name: "Test".into() })
    }
}

// Manejo con match
let result = divide(10, 2);
match result {
    Ok(value) => println!("Result: {}", value),
    Err(e) => println!("Error: {}", e),
}

// Manejo con if let (cuando solo te interesa un caso)
if let Some(user) = find_user(1) {
    println!("Found: {}", user.name);
}
```

**Comparación con Java:**

```java
// Java - Excepciones
public int divide(int a, int b) throws ArithmeticException {
    if (b == 0) throw new ArithmeticException("Division by zero");
    return a / b;
}

// Java - Optional (solo para ausencia de valor)
public Optional<User> findUser(int id) {
    return id == 0 ? Optional.empty() : Optional.of(new User(id));
}
```

| Rust | Java |
|------|------|
| `Result<T, E>` | Checked exceptions / `throws` |
| `Option<T>` | `Optional<T>` |
| `Ok(value)` | `return value` |
| `Err(e)` | `throw new Exception(e)` |
| Match exhaustivo | try-catch |
| Siempre explícito | Excepciones pueden propagarse implícitamente |

### El Operador ?

El operador `?` es azúcar sintáctico para propagar errores:

```rust
// Sin operador ?
fn read_config_verbose(path: &str) -> Result<Config, VortexError> {
    let content = match std::fs::read_to_string(path) {
        Ok(c) => c,
        Err(e) => return Err(VortexError::Io(e)),
    };

    let config = match parse_config(&content) {
        Ok(c) => c,
        Err(e) => return Err(e),
    };

    Ok(config)
}

// Con operador ? (idiomático)
fn read_config(path: &str) -> Result<Config, VortexError> {
    let content = std::fs::read_to_string(path)?;  // Propaga error si falla
    let config = parse_config(&content)?;           // Propaga error si falla
    Ok(config)
}

// El ? hace esto automáticamente:
// 1. Si el Result es Ok(v), extrae v y continúa
// 2. Si el Result es Err(e), convierte e al tipo de error
//    de la función y hace return Err(e)
```

**Importante:** El operador `?` solo funciona en funciones que retornan `Result` o `Option`.

```rust
// BIEN: Función retorna Result
fn process() -> Result<(), VortexError> {
    let data = get_data()?;
    Ok(())
}

// MAL: main no retorna Result por defecto
fn main() {
    let data = get_data()?;  // ERROR: cannot use ? in fn that returns ()
}

// BIEN: main puede retornar Result
fn main() -> Result<(), VortexError> {
    let data = get_data()?;
    Ok(())
}
```

### thiserror para Definir Errores

`thiserror` es una macro que genera implementaciones de `std::error::Error`:

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigError {
    // Mensaje simple
    #[error("Configuration file not found")]
    NotFound,

    // Mensaje con interpolación de campos
    #[error("Invalid configuration key: {key}")]
    InvalidKey { key: String },

    // Mensaje con formato custom
    #[error("Parse error at line {line}: {message}")]
    ParseError { line: usize, message: String },

    // Encadenar errores con #[source]
    #[error("I/O error reading config")]
    Io {
        path: String,
        #[source]  // Implementa source() para error chaining
        cause: std::io::Error,
    },

    // Conversión automática con #[from]
    #[error("JSON parse error")]
    Json(#[from] serde_json::Error),  // Implementa From<serde_json::Error>

    // Tipo transparente (usa el Display del tipo interno)
    #[error(transparent)]
    Other(#[from] anyhow::Error),
}
```

**Lo que thiserror genera:**

```rust
// #[derive(Error)] genera:
impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Usa el mensaje de #[error("...")]
    }
}

impl std::error::Error for ConfigError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        // Retorna el campo marcado con #[source]
    }
}

// #[from] genera:
impl From<serde_json::Error> for ConfigError {
    fn from(err: serde_json::Error) -> Self {
        ConfigError::Json(err)
    }
}
```

### Patrones de Error Handling

```rust
// Patrón 1: Convertir Option a Result
fn get_required(config: &ConfigMap, key: &str) -> Result<&str> {
    config
        .get_property(key)
        .ok_or_else(|| VortexError::property_not_found(key))
}

// Patrón 2: Mapear errores
fn load_config(path: &str) -> Result<ConfigMap> {
    std::fs::read_to_string(path)
        .map_err(|e| VortexError::source_error("filesystem", e.to_string()))?;
    // ...
}

// Patrón 3: Combinar múltiples Results
fn validate_all(items: Vec<Item>) -> Result<Vec<ValidItem>> {
    items
        .into_iter()
        .map(|item| validate(item))  // Cada uno retorna Result
        .collect()  // Vec<Result<T, E>> -> Result<Vec<T>, E>
}

// Patrón 4: Proporcionar contexto
fn process_config(app: &str, profile: &str) -> Result<Config> {
    let config = fetch_config(app, profile).map_err(|e| {
        VortexError::SourceError {
            source_name: "config-server".into(),
            message: format!("Failed to fetch config for {}/{}", app, profile),
            cause: Some(Box::new(e)),
        }
    })?;
    Ok(config)
}

// Patrón 5: Early return con ?
fn complex_operation() -> Result<Output> {
    let a = step_one()?;      // Falla temprano si hay error
    let b = step_two(a)?;
    let c = step_three(b)?;
    Ok(c)
}
```

**Comparación con Java:**

```java
// Java equivalente (más verboso)
public Config processConfig(String app, String profile) throws VortexException {
    try {
        return fetchConfig(app, profile);
    } catch (FetchException e) {
        throw new VortexException(
            String.format("Failed to fetch config for %s/%s", app, profile),
            e  // cause
        );
    }
}
```

## Riesgos y Errores Comunes

### Error 1: Usar unwrap() en producción

```rust
// MAL: Panic en producción si falla
let config = load_config()?;
let port = config.get_property("port").unwrap();

// BIEN: Manejar la ausencia
let port = config.get_property("port")
    .ok_or_else(|| VortexError::property_not_found("port"))?;

// O con valor por defecto
let port = config.get_property("port").unwrap_or("8080");
```

### Error 2: Perder información de errores

```rust
// MAL: Se pierde el error original
fn process() -> Result<()> {
    let _ = risky_operation();  // Error ignorado silenciosamente
    Ok(())
}

// MAL: Mensaje genérico sin contexto
fn process() -> Result<()> {
    risky_operation().map_err(|_| VortexError::Internal("Something failed".into()))?;
    Ok(())
}

// BIEN: Preservar contexto
fn process() -> Result<()> {
    risky_operation().map_err(|e| {
        VortexError::parse_error_with_cause("config.yml", "Failed to parse", e)
    })?;
    Ok(())
}
```

### Error 3: Panic en lugar de Result

```rust
// MAL: Panic es para bugs, no para errores esperados
impl Application {
    pub fn new(name: &str) -> Self {
        if name.is_empty() {
            panic!("Name cannot be empty");  // NO
        }
        Self(name.to_string())
    }
}

// BIEN: Retornar Result para errores esperados
impl Application {
    pub fn try_new(name: &str) -> Result<Self> {
        if name.is_empty() {
            return Err(VortexError::invalid_application(name, "cannot be empty"));
        }
        Ok(Self(name.to_string()))
    }
}
```

### Error 4: No implementar source()

```rust
// MAL: Información de causa perdida
#[derive(Debug, Error)]
pub enum MyError {
    #[error("Parse failed")]
    Parse(serde_json::Error),  // No se puede acceder al error original
}

// BIEN: Usar #[source] o #[from]
#[derive(Debug, Error)]
pub enum MyError {
    #[error("Parse failed")]
    Parse(#[source] serde_json::Error),  // error.source() retorna el error JSON
}
```

## Pruebas

### Unit Tests

```rust
// crates/vortex-core/src/error.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_not_found_display() {
        let error = VortexError::config_not_found("myapp", "prod", Some("v1.0".into()));
        let msg = format!("{}", error);

        assert!(msg.contains("myapp"));
        assert!(msg.contains("prod"));
        assert!(msg.contains("v1.0"));
    }

    #[test]
    fn test_config_not_found_without_label() {
        let error = VortexError::config_not_found("myapp", "prod", None);
        let msg = format!("{}", error);

        assert!(msg.contains("default"));  // Label por defecto
    }

    #[test]
    fn test_property_not_found() {
        let error = VortexError::property_not_found("database.url");

        assert!(matches!(error, VortexError::PropertyNotFound { .. }));
        assert!(format!("{}", error).contains("database.url"));
    }

    #[test]
    fn test_io_error_conversion() {
        let io_error = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found"
        );
        let vortex_error: VortexError = io_error.into();

        assert!(matches!(vortex_error, VortexError::Io(_)));
    }

    #[test]
    fn test_error_source_chain() {
        let io_error = std::io::Error::new(
            std::io::ErrorKind::PermissionDenied,
            "access denied"
        );
        let parse_error = VortexError::parse_error_with_cause(
            "config.yml",
            "Could not read file",
            io_error,
        );

        // Verificar que source() está implementado
        assert!(parse_error.source().is_some());
    }

    #[test]
    fn test_is_not_found() {
        let not_found = VortexError::config_not_found("app", "dev", None);
        let parse_error = VortexError::parse_error("file", "bad format");

        assert!(not_found.is_not_found());
        assert!(!parse_error.is_not_found());
    }

    #[test]
    fn test_result_with_question_mark() {
        fn inner() -> Result<()> {
            Err(VortexError::Internal("test".into()))
        }

        fn outer() -> Result<String> {
            inner()?;  // Propaga el error
            Ok("success".into())
        }

        assert!(outer().is_err());
    }
}
```

### Integration Tests

```rust
// crates/vortex-core/tests/error_handling.rs

use vortex_core::{Application, ConfigMap, Result, VortexError};

#[test]
fn test_validation_workflow() {
    fn validate_and_process(app_name: &str) -> Result<String> {
        let app = Application::try_new(app_name)?;
        Ok(format!("Processed: {}", app))
    }

    // Valid case
    assert!(validate_and_process("myapp").is_ok());

    // Invalid case
    let result = validate_and_process("");
    assert!(result.is_err());

    if let Err(VortexError::InvalidApplication { name, reason }) = result {
        assert!(name.is_empty());
        assert!(reason.contains("empty"));
    } else {
        panic!("Expected InvalidApplication error");
    }
}

#[test]
fn test_error_context_preservation() {
    fn load_config() -> Result<ConfigMap> {
        // Simular un error de parsing
        Err(VortexError::parse_error(
            "application.yml",
            "Invalid YAML syntax at line 10",
        ))
    }

    let result = load_config();
    assert!(result.is_err());

    let error = result.unwrap_err();
    let message = format!("{}", error);

    assert!(message.contains("application.yml"));
    assert!(message.contains("line 10"));
}
```

## Entregable Final

### PR debe incluir:

1. **Archivos nuevos/modificados:**
   - `crates/vortex-core/src/error.rs` (nuevo)
   - `crates/vortex-core/src/lib.rs` (actualizado con exports)
   - `crates/vortex-core/Cargo.toml` (agregar thiserror)

2. **Tests:**
   - Tests unitarios para cada variante de error
   - Tests de conversión y display
   - Tests de encadenamiento de errores

3. **Documentación:**
   - Doc comments con ejemplos
   - Guía de patrones de uso en código

### Checklist de Revisión

- [ ] Enum VortexError tiene al menos 5 variantes útiles
- [ ] Todos los mensajes de error son descriptivos
- [ ] #[source] usado para error chaining donde aplica
- [ ] Type alias Result<T> definido y exportado
- [ ] Constructores de conveniencia para errores comunes
- [ ] Tests cubren display, conversión y source()
- [ ] Doc comments con ejemplos ejecutables
- [ ] No hay warnings de clippy
- [ ] Integración con modelo de dominio (métodos que usan Result)

---

**Navegación:** [Anterior: Domain Model](./story-004-domain-model.md) | [Volver al índice](./index.md) | [Siguiente épica: Storage Backends](../02-storage/index.md)
