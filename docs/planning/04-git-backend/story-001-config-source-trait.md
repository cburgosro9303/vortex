# Historia 001: Trait ConfigSource

## Contexto y Objetivo

ConfigSource es la abstraccion fundamental que permite a Vortex Config soportar multiples backends de configuracion (Git, filesystem, bases de datos, etc.). Esta historia define el trait que todos los backends deben implementar, estableciendo un contrato claro para la obtencion de configuraciones.

Para un desarrollador Java, este patron es similar a definir una interface `ConfigurationSource` que diferentes implementaciones (GitConfigSource, FileConfigSource, etc.) deben cumplir. La diferencia clave en Rust es que los traits pueden tener metodos async y tipos asociados.

## Alcance

### In Scope
- Definicion del trait `ConfigSource` con metodos async
- Tipos asociados para errores especificos del backend
- Metodo principal: `get_config(app, profiles, label)`
- Metodo de health check: `is_healthy()`
- Metodo de refresh: `refresh()`
- Struct `ConfigQuery` para encapsular parametros de busqueda

### Out of Scope
- Implementaciones concretas del trait (historias 002-005)
- Caching de configuraciones (epica futura)
- Composite sources (multiples backends)

## Criterios de Aceptacion

- [ ] `ConfigSource` trait definido con `async_trait`
- [ ] `ConfigQuery` struct para encapsular app/profiles/label
- [ ] Metodo `get_config` retorna `Result<PropertySource, Error>`
- [ ] Metodo `refresh` para forzar actualizacion del backend
- [ ] Metodo `is_healthy` para health checks
- [ ] Trait es object-safe para permitir `dyn ConfigSource`
- [ ] Documentacion con ejemplos de implementacion

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-git/src/source/trait.rs` - ConfigSource trait
- `vortex-git/src/source/mod.rs` - Re-exports
- `vortex-git/src/error.rs` - Tipos de error

### Interfaces

```rust
use async_trait::async_trait;
use vortex_core::PropertySource;

/// Query parameters for configuration retrieval
#[derive(Debug, Clone)]
pub struct ConfigQuery {
    pub application: String,
    pub profiles: Vec<String>,
    pub label: Option<String>,
}

/// Error type for configuration source operations
#[derive(Debug, thiserror::Error)]
pub enum ConfigSourceError {
    #[error("Configuration not found for {application}/{profiles:?}")]
    NotFound {
        application: String,
        profiles: Vec<String>,
    },
    #[error("Backend unavailable: {0}")]
    Unavailable(String),
    #[error("Invalid label: {0}")]
    InvalidLabel(String),
    #[error(transparent)]
    Internal(#[from] anyhow::Error),
}

/// Trait defining the contract for configuration backends
#[async_trait]
pub trait ConfigSource: Send + Sync {
    /// Retrieve configuration for the given query
    async fn get_config(
        &self,
        query: &ConfigQuery,
    ) -> Result<PropertySource, ConfigSourceError>;

    /// Force refresh of the backend (e.g., git pull)
    async fn refresh(&self) -> Result<(), ConfigSourceError>;

    /// Check if the backend is healthy and available
    async fn is_healthy(&self) -> bool;

    /// Get the name/identifier of this source
    fn name(&self) -> &str;
}
```

### Estructura Sugerida

```
crates/vortex-git/src/source/
├── mod.rs          # pub mod trait_def; pub use trait_def::*;
└── trait.rs        # ConfigSource trait definition
```

## Pasos de Implementacion

1. **Crear estructura de directorios**
   - Crear `crates/vortex-git/` con Cargo.toml
   - Crear `src/source/` directory
   - Crear `src/error.rs` para tipos de error

2. **Implementar ConfigQuery**
   - Definir struct con campos application, profiles, label
   - Implementar builder pattern para construccion fluida
   - Implementar `Default` para valores por defecto

3. **Implementar ConfigSourceError**
   - Usar `thiserror` para derivar `Error`
   - Variantes para casos comunes: NotFound, Unavailable, InvalidLabel
   - Variante `Internal` con `anyhow::Error` para errores inesperados

4. **Definir ConfigSource trait**
   - Usar `async_trait` macro para metodos async
   - Definir `get_config`, `refresh`, `is_healthy`, `name`
   - Asegurar que el trait es object-safe (`Send + Sync`)

5. **Agregar documentacion y tests**

## Conceptos de Rust Aprendidos

### async_trait Macro

En Rust estable, los traits no pueden tener metodos async directamente. La macro `async_trait` resuelve esto transformando los metodos async en metodos que retornan `Pin<Box<dyn Future>>`.

```rust
use async_trait::async_trait;
use std::future::Future;
use std::pin::Pin;

// SIN async_trait - asi se veria manualmente:
pub trait ConfigSourceManual {
    fn get_config<'a>(
        &'a self,
        query: &'a ConfigQuery,
    ) -> Pin<Box<dyn Future<Output = Result<PropertySource, Error>> + Send + 'a>>;
}

// CON async_trait - mucho mas ergonomico:
#[async_trait]
pub trait ConfigSource {
    async fn get_config(
        &self,
        query: &ConfigQuery,
    ) -> Result<PropertySource, ConfigSourceError>;
}

// Implementacion con async_trait
#[async_trait]
impl ConfigSource for MyBackend {
    async fn get_config(
        &self,
        query: &ConfigQuery,
    ) -> Result<PropertySource, ConfigSourceError> {
        // Puedes usar .await normalmente aqui
        let data = self.fetch_from_storage().await?;
        Ok(data)
    }
}
```

**Comparacion con Java:**

```java
// Java - CompletableFuture para async
public interface ConfigSource {
    CompletableFuture<PropertySource> getConfig(ConfigQuery query);
    CompletableFuture<Void> refresh();
    CompletableFuture<Boolean> isHealthy();
}

// Implementacion
public class GitConfigSource implements ConfigSource {
    @Override
    public CompletableFuture<PropertySource> getConfig(ConfigQuery query) {
        return CompletableFuture.supplyAsync(() -> {
            // fetch logic
            return new PropertySource();
        });
    }
}
```

### Trait Objects y Object Safety

Para poder usar `dyn ConfigSource` (similar a usar la interface como tipo en Java), el trait debe ser "object-safe". Esto significa ciertas restricciones.

```rust
use async_trait::async_trait;

// Object-safe trait - puede usarse como dyn ConfigSource
#[async_trait]
pub trait ConfigSource: Send + Sync {
    async fn get_config(&self, query: &ConfigQuery)
        -> Result<PropertySource, ConfigSourceError>;

    async fn refresh(&self) -> Result<(), ConfigSourceError>;

    async fn is_healthy(&self) -> bool;

    // Retorna &str, no String, para ser object-safe
    fn name(&self) -> &str;
}

// Uso como trait object (similar a Java interface)
async fn fetch_config(
    source: &dyn ConfigSource,  // Similar a ConfigSource source en Java
    query: &ConfigQuery,
) -> Result<PropertySource, ConfigSourceError> {
    if !source.is_healthy().await {
        return Err(ConfigSourceError::Unavailable(
            source.name().to_string()
        ));
    }
    source.get_config(query).await
}

// Almacenar multiples backends
struct ConfigService {
    // Box<dyn ...> es similar a List<ConfigSource> en Java
    sources: Vec<Box<dyn ConfigSource>>,
}

impl ConfigService {
    async fn get_from_first_available(
        &self,
        query: &ConfigQuery,
    ) -> Result<PropertySource, ConfigSourceError> {
        for source in &self.sources {
            if source.is_healthy().await {
                return source.get_config(query).await;
            }
        }
        Err(ConfigSourceError::Unavailable(
            "No healthy sources available".into()
        ))
    }
}
```

**Comparacion con Java:**

| Rust | Java |
|------|------|
| `&dyn ConfigSource` | `ConfigSource source` |
| `Box<dyn ConfigSource>` | `ConfigSource` (heap allocated) |
| `Vec<Box<dyn ConfigSource>>` | `List<ConfigSource>` |
| `Send + Sync` bound | Thread-safe by default (with caveats) |

### Lifetimes en Trait Methods

Cuando un metodo del trait toma referencias, Rust infiere lifetimes. Con `async_trait`, hay consideraciones especiales.

```rust
use async_trait::async_trait;

#[derive(Debug, Clone)]
pub struct ConfigQuery {
    pub application: String,
    pub profiles: Vec<String>,
    pub label: Option<String>,
}

impl ConfigQuery {
    /// Constructor con valores por defecto
    pub fn new(application: impl Into<String>) -> Self {
        Self {
            application: application.into(),
            profiles: vec!["default".to_string()],
            label: None,
        }
    }

    /// Builder pattern para profiles
    pub fn with_profiles(mut self, profiles: Vec<String>) -> Self {
        self.profiles = profiles;
        self
    }

    /// Builder pattern para label
    pub fn with_label(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Obtener profiles como slice (borrowing)
    pub fn profiles(&self) -> &[String] {
        &self.profiles
    }

    /// Obtener label con default
    pub fn label_or_default(&self) -> &str {
        self.label.as_deref().unwrap_or("main")
    }
}

// Uso del builder pattern
fn example_query() {
    let query = ConfigQuery::new("myapp")
        .with_profiles(vec!["prod".into(), "aws".into()])
        .with_label("v1.0.0");

    println!("App: {}", query.application);
    println!("Profiles: {:?}", query.profiles());
    println!("Label: {}", query.label_or_default());
}
```

**Comparacion con Java (Builder Pattern):**

```java
// Java con Builder
public class ConfigQuery {
    private final String application;
    private final List<String> profiles;
    private final String label;

    private ConfigQuery(Builder builder) {
        this.application = builder.application;
        this.profiles = builder.profiles;
        this.label = builder.label;
    }

    public static Builder builder(String application) {
        return new Builder(application);
    }

    public static class Builder {
        private String application;
        private List<String> profiles = List.of("default");
        private String label;

        public Builder(String application) {
            this.application = application;
        }

        public Builder profiles(List<String> profiles) {
            this.profiles = profiles;
            return this;
        }

        public Builder label(String label) {
            this.label = label;
            return this;
        }

        public ConfigQuery build() {
            return new ConfigQuery(this);
        }
    }
}
```

### thiserror para Errores Tipados

`thiserror` genera automaticamente implementaciones de `Error` y `Display`.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ConfigSourceError {
    // Mensaje con interpolacion de campos
    #[error("Configuration not found for {application}/{profiles:?}")]
    NotFound {
        application: String,
        profiles: Vec<String>,
    },

    // Mensaje simple con campo
    #[error("Backend unavailable: {0}")]
    Unavailable(String),

    // Validacion de label
    #[error("Invalid label '{label}': {reason}")]
    InvalidLabel {
        label: String,
        reason: String,
    },

    // Wrap de errores internos con #[from] para conversion automatica
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),

    // Error generico para casos inesperados
    #[error(transparent)]  // Usa el Display del error interno
    Internal(#[from] anyhow::Error),
}

// thiserror genera automaticamente:
// - impl std::fmt::Display for ConfigSourceError
// - impl std::error::Error for ConfigSourceError
// - impl From<std::io::Error> for ConfigSourceError
// - impl From<anyhow::Error> for ConfigSourceError

// Uso con ? operator
async fn load_config(path: &str) -> Result<String, ConfigSourceError> {
    // std::io::Error se convierte automaticamente a ConfigSourceError::Io
    let content = tokio::fs::read_to_string(path).await?;
    Ok(content)
}
```

**Comparacion con Java:**

```java
// Java - excepciones checked/unchecked
public class ConfigSourceException extends Exception {
    private final String application;
    private final List<String> profiles;

    public ConfigSourceException(String application, List<String> profiles) {
        super(String.format("Configuration not found for %s/%s",
            application, profiles));
        this.application = application;
        this.profiles = profiles;
    }
}

// Subclases para diferentes casos
public class BackendUnavailableException extends ConfigSourceException { }
public class InvalidLabelException extends ConfigSourceException { }
```

## Riesgos y Errores Comunes

### 1. Olvidar Send + Sync en el trait bound

```rust
// ERROR: No es thread-safe, no puede usarse con Tokio
#[async_trait]
pub trait ConfigSource {
    async fn get_config(&self, query: &ConfigQuery) -> Result<PropertySource>;
}

// CORRECTO: Send + Sync permite uso en contextos multi-thread
#[async_trait]
pub trait ConfigSource: Send + Sync {
    async fn get_config(&self, query: &ConfigQuery) -> Result<PropertySource>;
}
```

### 2. Trait no object-safe por metodos genericos

```rust
// ERROR: Metodo generico hace el trait no object-safe
pub trait ConfigSource {
    fn parse<T: DeserializeOwned>(&self, data: &str) -> T;
}

// CORRECTO: Usar tipos concretos o asociados
pub trait ConfigSource {
    fn parse(&self, data: &str) -> ConfigMap;
}
```

### 3. Retornar Self en metodos

```rust
// ERROR: Self hace el trait no object-safe
pub trait ConfigSource {
    fn clone_source(&self) -> Self;
}

// CORRECTO: Retornar Box<dyn ConfigSource>
pub trait ConfigSource: Send + Sync {
    fn clone_source(&self) -> Box<dyn ConfigSource>;
}
```

## Pruebas

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_query_new() {
        let query = ConfigQuery::new("myapp");

        assert_eq!(query.application, "myapp");
        assert_eq!(query.profiles, vec!["default"]);
        assert!(query.label.is_none());
    }

    #[test]
    fn test_config_query_builder() {
        let query = ConfigQuery::new("myapp")
            .with_profiles(vec!["prod".into(), "aws".into()])
            .with_label("v1.0.0");

        assert_eq!(query.application, "myapp");
        assert_eq!(query.profiles, vec!["prod", "aws"]);
        assert_eq!(query.label, Some("v1.0.0".to_string()));
    }

    #[test]
    fn test_config_query_label_or_default() {
        let query_without_label = ConfigQuery::new("app");
        assert_eq!(query_without_label.label_or_default(), "main");

        let query_with_label = ConfigQuery::new("app")
            .with_label("develop");
        assert_eq!(query_with_label.label_or_default(), "develop");
    }

    #[test]
    fn test_config_source_error_display() {
        let error = ConfigSourceError::NotFound {
            application: "myapp".into(),
            profiles: vec!["prod".into()],
        };

        assert!(error.to_string().contains("myapp"));
        assert!(error.to_string().contains("prod"));
    }

    #[test]
    fn test_error_from_io() {
        let io_error = std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "file not found"
        );
        let config_error: ConfigSourceError = io_error.into();

        assert!(matches!(config_error, ConfigSourceError::Io(_)));
    }
}
```

### Mock Implementation for Testing

```rust
#[cfg(test)]
mod mock_tests {
    use super::*;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::sync::Arc;

    /// Mock implementation for testing consumers of ConfigSource
    struct MockConfigSource {
        healthy: AtomicBool,
        config: PropertySource,
        name: String,
    }

    impl MockConfigSource {
        fn new(name: &str, config: PropertySource) -> Self {
            Self {
                healthy: AtomicBool::new(true),
                config,
                name: name.to_string(),
            }
        }

        fn set_healthy(&self, healthy: bool) {
            self.healthy.store(healthy, Ordering::SeqCst);
        }
    }

    #[async_trait]
    impl ConfigSource for MockConfigSource {
        async fn get_config(
            &self,
            _query: &ConfigQuery,
        ) -> Result<PropertySource, ConfigSourceError> {
            if !self.is_healthy().await {
                return Err(ConfigSourceError::Unavailable(
                    self.name.clone()
                ));
            }
            Ok(self.config.clone())
        }

        async fn refresh(&self) -> Result<(), ConfigSourceError> {
            Ok(())
        }

        async fn is_healthy(&self) -> bool {
            self.healthy.load(Ordering::SeqCst)
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_mock_source_healthy() {
        let source = MockConfigSource::new(
            "test",
            PropertySource::new("test", ConfigMap::new()),
        );

        assert!(source.is_healthy().await);

        let query = ConfigQuery::new("app");
        let result = source.get_config(&query).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn test_mock_source_unhealthy() {
        let source = MockConfigSource::new(
            "test",
            PropertySource::new("test", ConfigMap::new()),
        );
        source.set_healthy(false);

        assert!(!source.is_healthy().await);

        let query = ConfigQuery::new("app");
        let result = source.get_config(&query).await;
        assert!(matches!(
            result,
            Err(ConfigSourceError::Unavailable(_))
        ));
    }

    #[tokio::test]
    async fn test_trait_object() {
        let source: Box<dyn ConfigSource> = Box::new(
            MockConfigSource::new(
                "test",
                PropertySource::new("test", ConfigMap::new()),
            )
        );

        assert!(source.is_healthy().await);
        assert_eq!(source.name(), "test");
    }
}
```

## Observabilidad

```rust
use tracing::{instrument, info, warn};

#[async_trait]
impl ConfigSource for GitConfigSource {
    #[instrument(skip(self), fields(source = %self.name()))]
    async fn get_config(
        &self,
        query: &ConfigQuery,
    ) -> Result<PropertySource, ConfigSourceError> {
        info!(
            application = %query.application,
            profiles = ?query.profiles,
            label = ?query.label,
            "Fetching configuration"
        );

        // ... implementation
    }

    #[instrument(skip(self), fields(source = %self.name()))]
    async fn refresh(&self) -> Result<(), ConfigSourceError> {
        info!("Refreshing configuration source");
        // ... implementation
    }
}
```

## Entregable Final

- PR con:
  - `crates/vortex-git/Cargo.toml`
  - `crates/vortex-git/src/lib.rs`
  - `crates/vortex-git/src/error.rs`
  - `crates/vortex-git/src/source/mod.rs`
  - `crates/vortex-git/src/source/trait.rs`
  - Tests unitarios con mock implementation
  - Rustdoc para trait y todos los metodos
  - Ejemplo de implementacion en documentation

---

**Anterior**: [Indice de Epica 04](./index.md)
**Siguiente**: [Historia 002 - Clone y Pull de Repositorios](./story-002-clone-pull.md)
