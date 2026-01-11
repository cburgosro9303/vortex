# Historia 004: Modelo de Dominio Core

## Contexto y Objetivo

El modelo de dominio de Vortex Config define los tipos fundamentales que representan la configuracion de aplicaciones. Estos tipos son el corazon del servidor y seran utilizados por todos los demas componentes.

Inspirado en Spring Cloud Config, nuestro modelo incluye:
- **ConfigMap**: Conjunto de propiedades para una aplicacion
- **PropertySource**: Origen de configuracion con propiedades clave-valor
- **Application**: Identificador de aplicacion
- **Profile**: Perfil de ejecucion (dev, staging, prod)
- **Label**: Version/branch de configuracion (main, v1.0.0)

## Alcance

### In Scope
- Definir structs para ConfigMap, PropertySource
- Definir tipos para Application, Profile, Label
- Implementar traits basicos (Clone, Debug, PartialEq)
- Usar derive macros para Serialize/Deserialize
- Establecer visibilidad publica apropiada
- Documentacion inline completa

### Out of Scope
- Logica de negocio para cargar configuraciones
- Validacion compleja de datos
- Persistencia o serializacion a backends

## Criterios de Aceptacion

- [ ] Struct `ConfigMap` con campos: name, profiles, label, property_sources
- [ ] Struct `PropertySource` con campos: name, properties (HashMap)
- [ ] Tipos `Application`, `Profile`, `Label` como newtypes
- [ ] Todos los tipos implementan Debug, Clone, PartialEq
- [ ] Todos los tipos derivan Serialize, Deserialize
- [ ] Doc comments en todos los items publicos
- [ ] Al menos 5 tests unitarios para el modelo
- [ ] `cargo doc` genera sin warnings

## Diseno Propuesto

### Modulos Implicados

```
crates/vortex-core/src/
├── lib.rs              # Re-exports publicos
├── config.rs           # ConfigMap, PropertySource
├── environment.rs      # Application, Profile, Label
└── types.rs            # Type aliases y newtypes
```

### Interfaces Propuestas

```rust
// config.rs
pub struct ConfigMap { ... }
pub struct PropertySource { ... }

// environment.rs
pub struct Application(String);
pub struct Profile(String);
pub struct Label(String);

// Trait implementations
impl ConfigMap {
    pub fn new(name: impl Into<String>) -> Self;
    pub fn builder() -> ConfigMapBuilder;
    pub fn name(&self) -> &str;
    pub fn profiles(&self) -> &[Profile];
    pub fn label(&self) -> Option<&Label>;
    pub fn property_sources(&self) -> &[PropertySource];
    pub fn get_property(&self, key: &str) -> Option<&str>;
}
```

### Estructura Sugerida

```rust
// Ejemplo de ConfigMap completo
ConfigMap {
    name: Application("myapp"),
    profiles: vec![Profile("production")],
    label: Some(Label("main")),
    property_sources: vec![
        PropertySource {
            name: "application-production.yml".into(),
            properties: HashMap::from([
                ("server.port".into(), "8080".into()),
                ("database.url".into(), "postgres://...".into()),
            ]),
        },
        PropertySource {
            name: "application.yml".into(),
            properties: HashMap::from([
                ("server.port".into(), "8000".into()),
                ("app.name".into(), "My Application".into()),
            ]),
        },
    ],
}
```

## Pasos de Implementacion

### Paso 1: Crear modulo types.rs

```rust
// crates/vortex-core/src/types.rs

//! Common type definitions and newtypes for Vortex Config.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Application identifier.
///
/// Represents the name of an application whose configuration
/// is being managed. This is typically the service name.
///
/// # Example
///
/// ```
/// use vortex_core::Application;
///
/// let app = Application::new("payment-service");
/// assert_eq!(app.as_str(), "payment-service");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Application(String);

impl Application {
    /// Creates a new Application identifier.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the application name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Application {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Application {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Application {
    fn from(s: String) -> Self {
        Self(s)
    }
}
```

### Paso 2: Crear Profile y Label (similar pattern)

```rust
// Continuación de types.rs

/// Execution profile for configuration selection.
///
/// Profiles allow different configurations for different environments.
/// Common profiles: "default", "development", "staging", "production".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Profile(String);

impl Profile {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the default profile.
    pub fn default_profile() -> Self {
        Self::new("default")
    }
}

/// Configuration version or branch label.
///
/// Labels identify specific versions of configuration, typically
/// corresponding to Git branches or tags.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Label(String);

impl Label {
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the main/default label.
    pub fn main() -> Self {
        Self::new("main")
    }
}
```

### Paso 3: Crear config.rs

```rust
// crates/vortex-core/src/config.rs

//! Configuration structures for Vortex Config.

use crate::types::{Application, Label, Profile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A collection of configuration properties from a specific source.
///
/// PropertySource represents configuration loaded from a single file
/// or backend. Multiple PropertySources are combined to form a ConfigMap.
///
/// # Example
///
/// ```
/// use vortex_core::PropertySource;
/// use std::collections::HashMap;
///
/// let mut props = HashMap::new();
/// props.insert("server.port".to_string(), "8080".to_string());
///
/// let source = PropertySource::new("application.yml", props);
/// assert_eq!(source.get("server.port"), Some("8080"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertySource {
    /// Name of the source (typically filename or backend identifier)
    name: String,
    /// Key-value properties from this source
    properties: HashMap<String, String>,
}

impl PropertySource {
    /// Creates a new PropertySource with the given name and properties.
    pub fn new(
        name: impl Into<String>,
        properties: HashMap<String, String>,
    ) -> Self {
        Self {
            name: name.into(),
            properties,
        }
    }

    /// Returns the name of this property source.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference to the properties map.
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }

    /// Gets a property value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }

    /// Returns the number of properties.
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Returns true if there are no properties.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }
}

/// Complete configuration for an application.
///
/// ConfigMap aggregates configuration from multiple PropertySources
/// for a specific application, profile(s), and label combination.
/// PropertySources are ordered by precedence (first source wins).
///
/// # Example
///
/// ```
/// use vortex_core::{ConfigMap, Application, Profile};
///
/// let config = ConfigMap::builder()
///     .application(Application::new("myapp"))
///     .profile(Profile::new("production"))
///     .build();
///
/// assert_eq!(config.application().as_str(), "myapp");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigMap {
    /// Application this configuration belongs to
    application: Application,
    /// Active profiles
    profiles: Vec<Profile>,
    /// Configuration version/branch
    label: Option<Label>,
    /// Ordered list of property sources (first wins)
    property_sources: Vec<PropertySource>,
}

impl ConfigMap {
    /// Creates a new ConfigMap with the given application name.
    pub fn new(application: impl Into<Application>) -> Self {
        Self {
            application: application.into(),
            profiles: vec![Profile::default_profile()],
            label: None,
            property_sources: Vec::new(),
        }
    }

    /// Returns a builder for constructing a ConfigMap.
    pub fn builder() -> ConfigMapBuilder {
        ConfigMapBuilder::default()
    }

    /// Returns the application identifier.
    pub fn application(&self) -> &Application {
        &self.application
    }

    /// Returns the active profiles.
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    /// Returns the configuration label, if set.
    pub fn label(&self) -> Option<&Label> {
        self.label.as_ref()
    }

    /// Returns the property sources.
    pub fn property_sources(&self) -> &[PropertySource] {
        &self.property_sources
    }

    /// Gets a property value, searching through sources in order.
    ///
    /// Returns the value from the first PropertySource that contains
    /// the key, or None if not found in any source.
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.property_sources
            .iter()
            .find_map(|source| source.get(key))
    }

    /// Returns all unique property keys across all sources.
    pub fn property_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self
            .property_sources
            .iter()
            .flat_map(|s| s.properties().keys().map(|k| k.as_str()))
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

/// Builder for ConfigMap.
#[derive(Debug, Default)]
pub struct ConfigMapBuilder {
    application: Option<Application>,
    profiles: Vec<Profile>,
    label: Option<Label>,
    property_sources: Vec<PropertySource>,
}

impl ConfigMapBuilder {
    /// Sets the application identifier.
    pub fn application(mut self, app: impl Into<Application>) -> Self {
        self.application = Some(app.into());
        self
    }

    /// Adds a profile.
    pub fn profile(mut self, profile: impl Into<Profile>) -> Self {
        self.profiles.push(profile.into());
        self
    }

    /// Sets the label.
    pub fn label(mut self, label: impl Into<Label>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Adds a property source.
    pub fn property_source(mut self, source: PropertySource) -> Self {
        self.property_sources.push(source);
        self
    }

    /// Builds the ConfigMap.
    ///
    /// # Panics
    ///
    /// Panics if application is not set.
    pub fn build(self) -> ConfigMap {
        ConfigMap {
            application: self
                .application
                .expect("Application must be set"),
            profiles: if self.profiles.is_empty() {
                vec![Profile::default_profile()]
            } else {
                self.profiles
            },
            label: self.label,
            property_sources: self.property_sources,
        }
    }
}
```

### Paso 4: Actualizar lib.rs

```rust
// crates/vortex-core/src/lib.rs

//! Vortex Core - Domain types and traits for Vortex Config.
//!
//! This crate provides the foundational types for the Vortex Config
//! server, a cloud-native configuration management system.
//!
//! # Main Types
//!
//! - [`ConfigMap`]: Complete configuration for an application
//! - [`PropertySource`]: Single source of configuration properties
//! - [`Application`]: Application identifier
//! - [`Profile`]: Execution profile (dev, prod, etc.)
//! - [`Label`]: Configuration version/branch
//!
//! # Example
//!
//! ```
//! use vortex_core::{ConfigMap, PropertySource, Application, Profile};
//! use std::collections::HashMap;
//!
//! // Create a property source
//! let mut props = HashMap::new();
//! props.insert("server.port".to_string(), "8080".to_string());
//! let source = PropertySource::new("application.yml", props);
//!
//! // Build a ConfigMap
//! let config = ConfigMap::builder()
//!     .application(Application::new("myapp"))
//!     .profile(Profile::new("production"))
//!     .property_source(source)
//!     .build();
//!
//! assert_eq!(config.get_property("server.port"), Some("8080"));
//! ```

mod config;
mod types;

// Re-export public types
pub use config::{ConfigMap, ConfigMapBuilder, PropertySource};
pub use types::{Application, Label, Profile};

/// Returns the crate version.
pub fn version() -> &'static str {
    env!("CARGO_PKG_VERSION")
}
```

## Conceptos de Rust Aprendidos

### Structs y Campos

En Rust, los structs son similares a las clases de Java pero sin herencia. Son tipos de datos compuestos.

```rust
// Struct con campos nombrados (más común)
pub struct PropertySource {
    name: String,           // Campo privado por defecto
    pub properties: HashMap<String, String>,  // Campo público
}

// Tuple struct (para newtypes)
pub struct Application(String);  // Un solo campo sin nombre

// Unit struct (sin datos)
pub struct EmptyMarker;

// Struct con lifetime (para referencias)
pub struct ConfigRef<'a> {
    name: &'a str,  // Referencia con lifetime 'a
}
```

**Comparación con Java:**

```java
// Java - Clase típica
public class PropertySource {
    private String name;
    private Map<String, String> properties;

    // Constructor, getters, setters, equals, hashCode, toString...
}

// Java 16+ - Record (más similar a Rust struct)
public record PropertySource(
    String name,
    Map<String, String> properties
) {}
```

| Rust Struct | Java Class/Record |
|-------------|-------------------|
| Campos privados por defecto | Campos accesibles según modificador |
| No herencia | Herencia de clase |
| `impl` separado | Métodos en la clase |
| `#[derive(...)]` | Lombok / IDE generation |

### Enums y Pattern Matching

Los enums de Rust son mucho más poderosos que los de Java - pueden contener datos.

```rust
/// Configuration value that can be different types.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]  // Serializa sin tag de variante
pub enum ConfigValue {
    /// String value
    String(String),
    /// Integer value
    Integer(i64),
    /// Boolean value
    Boolean(bool),
    /// List of values
    List(Vec<ConfigValue>),
    /// Nested map
    Map(HashMap<String, ConfigValue>),
    /// Null/missing value
    Null,
}

impl ConfigValue {
    /// Attempts to get the value as a string.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Attempts to get the value as an integer.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Returns true if this is a null value.
    pub fn is_null(&self) -> bool {
        matches!(self, ConfigValue::Null)
    }
}

// Uso con match (exhaustivo - debe cubrir todas las variantes)
fn process_value(value: &ConfigValue) -> String {
    match value {
        ConfigValue::String(s) => s.clone(),
        ConfigValue::Integer(i) => i.to_string(),
        ConfigValue::Boolean(b) => b.to_string(),
        ConfigValue::List(items) => {
            format!("[{} items]", items.len())
        }
        ConfigValue::Map(map) => {
            format!("{{{} keys}}", map.len())
        }
        ConfigValue::Null => "null".to_string(),
    }
}
```

**Comparación con Java:**

```java
// Java - Sealed classes (Java 17+) como equivalente
public sealed interface ConfigValue permits
    StringValue, IntegerValue, BooleanValue, ListValue, MapValue, NullValue {
}

public record StringValue(String value) implements ConfigValue {}
public record IntegerValue(long value) implements ConfigValue {}
// ... etc

// Pattern matching en Java 21+
String result = switch (value) {
    case StringValue(String s) -> s;
    case IntegerValue(long i) -> String.valueOf(i);
    // ...
};
```

| Rust Enum | Java Sealed Classes |
|-----------|---------------------|
| `enum Foo { A, B(i32) }` | `sealed interface + records` |
| `match` | `switch` con pattern matching |
| Exhaustivo por defecto | Exhaustivo con sealed |
| Un archivo | Múltiples archivos/clases |

### Derive Macros

Los derive macros generan implementaciones automáticas de traits.

```rust
// Derive básicos
#[derive(Debug)]       // Genera fmt::Debug (como toString() para debug)
#[derive(Clone)]       // Genera Clone (deep copy)
#[derive(PartialEq)]   // Genera == y !=
#[derive(Eq)]          // Marca como igualdad total (requiere PartialEq)
#[derive(Hash)]        // Genera Hash (para usar en HashMap keys)
#[derive(Default)]     // Genera Default::default()

// Derive de serde (serialización)
#[derive(Serialize, Deserialize)]

// Combinados típicos para un tipo de dominio
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Application(String);

// Con atributos de serde
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]  // JSON con camelCase
pub struct ConfigResponse {
    #[serde(rename = "app")]        // Campo específico
    application: Application,

    #[serde(skip_serializing_if = "Option::is_none")]
    label: Option<Label>,

    #[serde(default)]               // Usa Default si falta
    profiles: Vec<Profile>,
}
```

**Comparación con Java/Lombok:**

| Rust Derive | Lombok Annotation |
|-------------|-------------------|
| `#[derive(Debug)]` | `@ToString` |
| `#[derive(Clone)]` | Implementación manual / `@Builder` |
| `#[derive(PartialEq)]` | `@EqualsAndHashCode` |
| `#[derive(Default)]` | Constructor sin args |
| `#[derive(Serialize)]` | Jackson annotations |

### Pub Visibility

```rust
// Archivo: src/lib.rs

// Módulo público - puede ser accedido desde fuera del crate
pub mod config;

// Módulo privado - solo accesible dentro de este crate
mod internal;

// Re-export público - Application disponible como vortex_core::Application
pub use config::Application;

// Archivo: src/config.rs

// Struct público con campos privados
pub struct ConfigMap {
    name: String,           // privado
    pub label: Label,       // público
    pub(crate) cache: Cache, // visible en todo el crate
    pub(super) temp: String, // visible en módulo padre
}

impl ConfigMap {
    // Método público
    pub fn new(name: String) -> Self { ... }

    // Método privado
    fn validate(&self) -> bool { ... }

    // Visible solo en este crate
    pub(crate) fn internal_state(&self) -> &State { ... }
}
```

**Niveles de visibilidad:**

| Rust | Java | Descripción |
|------|------|-------------|
| (ninguno) | private | Solo en el módulo actual |
| `pub` | public | Accesible desde cualquier lugar |
| `pub(crate)` | package-private | Solo en el crate actual |
| `pub(super)` | - | En el módulo padre |
| `pub(in path)` | - | En un path específico |

## Riesgos y Errores Comunes

### Error 1: Olvidar re-exportar tipos

```rust
// MAL: El usuario no puede acceder a Application
// lib.rs
mod types;

// El usuario tendría que hacer:
use vortex_core::types::Application;  // ERROR: types es privado

// BIEN: Re-exportar
pub mod types;
// o mejor:
mod types;
pub use types::Application;
```

### Error 2: Clone vs Copy

```rust
// MAL: Asumir que todo se puede copiar
let app = Application::new("test");
let app2 = app;  // app se mueve, ya no es válido
println!("{}", app);  // ERROR: use of moved value

// BIEN: Clonar explícitamente
let app = Application::new("test");
let app2 = app.clone();  // Copia explícita
println!("{}", app);  // OK
```

### Error 3: Comparación de structs sin PartialEq

```rust
// MAL: Sin derive
struct Config { name: String }
let c1 = Config { name: "a".into() };
let c2 = Config { name: "a".into() };
assert_eq!(c1, c2);  // ERROR: PartialEq not implemented

// BIEN: Con derive
#[derive(PartialEq)]
struct Config { name: String }
```

### Error 4: HashMap key sin Hash

```rust
// MAL: Sin Hash no puede ser key
#[derive(PartialEq, Eq)]
struct AppKey { name: String }

let mut map = HashMap::new();
map.insert(AppKey { name: "a".into() }, 1);  // ERROR

// BIEN: Derivar Hash
#[derive(PartialEq, Eq, Hash)]
struct AppKey { name: String }
```

## Pruebas

### Unit Tests

```rust
// crates/vortex-core/src/config.rs

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_source_creation() {
        let mut props = HashMap::new();
        props.insert("key".to_string(), "value".to_string());

        let source = PropertySource::new("test.yml", props);

        assert_eq!(source.name(), "test.yml");
        assert_eq!(source.get("key"), Some("value"));
        assert_eq!(source.get("missing"), None);
    }

    #[test]
    fn test_config_map_builder() {
        let config = ConfigMap::builder()
            .application("myapp")
            .profile(Profile::new("prod"))
            .label(Label::new("v1.0"))
            .build();

        assert_eq!(config.application().as_str(), "myapp");
        assert_eq!(config.profiles().len(), 1);
        assert!(config.label().is_some());
    }

    #[test]
    fn test_property_lookup_precedence() {
        let source1 = PropertySource::new(
            "high-priority",
            HashMap::from([("key".into(), "from-source1".into())]),
        );
        let source2 = PropertySource::new(
            "low-priority",
            HashMap::from([("key".into(), "from-source2".into())]),
        );

        let config = ConfigMap::builder()
            .application("test")
            .property_source(source1)
            .property_source(source2)
            .build();

        // First source wins
        assert_eq!(config.get_property("key"), Some("from-source1"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let config = ConfigMap::builder()
            .application("myapp")
            .profile(Profile::new("production"))
            .build();

        let json = serde_json::to_string(&config).unwrap();
        let restored: ConfigMap = serde_json::from_str(&json).unwrap();

        assert_eq!(config, restored);
    }

    #[test]
    fn test_application_display() {
        let app = Application::new("payment-service");
        assert_eq!(format!("{}", app), "payment-service");
    }
}
```

### Integration Tests

```rust
// crates/vortex-core/tests/domain_tests.rs

use vortex_core::{Application, ConfigMap, Label, Profile, PropertySource};
use std::collections::HashMap;

#[test]
fn test_complete_config_workflow() {
    // Simulate loading configuration from multiple sources
    let app_props = PropertySource::new(
        "application.yml",
        HashMap::from([
            ("server.port".into(), "8000".into()),
            ("app.name".into(), "Default App".into()),
        ]),
    );

    let profile_props = PropertySource::new(
        "application-production.yml",
        HashMap::from([
            ("server.port".into(), "8080".into()),
            ("database.pool.size".into(), "20".into()),
        ]),
    );

    let config = ConfigMap::builder()
        .application(Application::new("myapp"))
        .profile(Profile::new("production"))
        .label(Label::new("main"))
        .property_source(profile_props)  // Higher priority
        .property_source(app_props)       // Lower priority
        .build();

    // Profile-specific value takes precedence
    assert_eq!(config.get_property("server.port"), Some("8080"));

    // Falls back to default
    assert_eq!(config.get_property("app.name"), Some("Default App"));

    // Profile-specific only
    assert_eq!(config.get_property("database.pool.size"), Some("20"));
}
```

## Entregable Final

### PR debe incluir:

1. **Archivos nuevos/modificados:**
   - `crates/vortex-core/src/lib.rs` (actualizado)
   - `crates/vortex-core/src/types.rs`
   - `crates/vortex-core/src/config.rs`

2. **Tests:**
   - Al menos 5 tests unitarios
   - 1 test de integración

3. **Documentación:**
   - Doc comments en todos los items públicos
   - Ejemplos ejecutables en doc comments

### Checklist de Revisión

- [ ] Todos los tipos públicos documentados
- [ ] Derive macros apropiados (Debug, Clone, PartialEq, Serialize, Deserialize)
- [ ] Visibilidad mínima necesaria (no exponer internals)
- [ ] Tests cubren casos básicos y edge cases
- [ ] `cargo doc --open` genera documentación correcta
- [ ] Ejemplos en doc comments compilan (`cargo test --doc`)
- [ ] No hay warnings de clippy
- [ ] Serialización JSON funciona correctamente

---

**Navegación:** [Anterior: CI Pipeline](./story-003-ci-pipeline.md) | [Volver al índice](./index.md) | [Siguiente: Error Handling](./story-005-error-handling.md)
