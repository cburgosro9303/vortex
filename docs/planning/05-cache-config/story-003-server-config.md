# Historia 003: Configuracion del Servidor

## Contexto y Objetivo

Un servidor de configuracion necesita, ironicamente, su propia configuracion robusta. Esta historia implementa el sistema de configuracion de Vortex Server siguiendo principios de 12-Factor Apps:

1. **Configuracion desde archivos**: YAML para valores por defecto y por entorno
2. **Environment variables**: Sobreescriben valores de archivos para despliegues cloud
3. **Validacion al startup**: Fail-fast con mensajes claros de error
4. **Type-safety**: Deserializacion directa a structs de Rust

Para desarrolladores Java/Spring, esto es similar a `@ConfigurationProperties` + `application.yml` + profiles, pero sin reflection y con validacion en compile-time.

---

## Alcance

### In Scope

- Struct `ServerSettings` con toda la configuracion
- Carga desde `config/default.yaml`, `config/{env}.yaml`
- Override desde environment variables con prefijo `VORTEX_`
- Validacion de configuracion al startup
- Feature flags de Cargo para backends opcionales
- Tests de configuracion

### Out of Scope

- Hot reload de configuracion
- Configuracion remota (desde otro config server)
- Secrets management (Vault integration - epica futura)
- UI de configuracion

---

## Criterios de Aceptacion

- [ ] `ServerSettings` se carga desde YAML y env vars
- [ ] Environment variables con prefijo `VORTEX_` sobreescriben YAML
- [ ] Validacion de valores (port > 0, TTL > 0, etc.)
- [ ] Error descriptivo si configuracion invalida
- [ ] Feature flags para backends opcionales (`git`, `s3`, `sql`)
- [ ] Server inicia correctamente con configuracion minima
- [ ] Tests unitarios y de integracion pasan

---

## Diseno Propuesto

### Estructura de Modulos

```
crates/vortex-server/src/config/
├── mod.rs           # Re-exports
├── settings.rs      # ServerSettings struct
├── loader.rs        # Config loading logic
└── validation.rs    # Validation rules
```

### Interfaces Principales

```rust
/// Configuracion completa del servidor
#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    pub server: ServerConfig,
    pub cache: CacheSettings,
    pub backends: BackendsConfig,
    pub logging: LoggingConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
    pub graceful_shutdown_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CacheSettings {
    pub enabled: bool,
    pub ttl_seconds: u64,
    pub max_capacity: u64,
}
```

### Estructura de Archivos de Configuracion

```
config/
├── default.yaml     # Valores por defecto
├── development.yaml # Override para desarrollo
├── production.yaml  # Override para produccion
└── test.yaml        # Override para tests
```

---

## Pasos de Implementacion

### Paso 1: Agregar Dependencias

```toml
# crates/vortex-server/Cargo.toml
[dependencies]
config = "0.14"
serde = { version = "1", features = ["derive"] }

[features]
default = ["git-backend"]
git-backend = []
s3-backend = ["dep:aws-sdk-s3"]
sql-backend = ["dep:sqlx"]
```

### Paso 2: Definir Settings Structs

```rust
// src/config/settings.rs
use serde::Deserialize;
use std::path::PathBuf;

/// Configuracion completa del servidor Vortex Config.
#[derive(Debug, Clone, Deserialize)]
pub struct ServerSettings {
    /// Configuracion del servidor HTTP
    pub server: ServerConfig,
    /// Configuracion del cache
    pub cache: CacheSettings,
    /// Configuracion de backends
    pub backends: BackendsConfig,
    /// Configuracion de logging
    pub logging: LoggingConfig,
    /// Configuracion de metricas
    #[serde(default)]
    pub metrics: MetricsConfig,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ServerConfig {
    /// Host para bind (default: 0.0.0.0)
    #[serde(default = "default_host")]
    pub host: String,

    /// Puerto HTTP (default: 8080)
    #[serde(default = "default_port")]
    pub port: u16,

    /// Timeout para graceful shutdown en segundos
    #[serde(default = "default_shutdown_timeout")]
    pub graceful_shutdown_timeout_secs: u64,

    /// Numero de workers (default: num_cpus)
    pub workers: Option<usize>,
}

fn default_host() -> String {
    "0.0.0.0".to_string()
}

fn default_port() -> u16 {
    8080
}

fn default_shutdown_timeout() -> u64 {
    30
}

#[derive(Debug, Clone, Deserialize)]
pub struct CacheSettings {
    /// Habilitar cache (default: true)
    #[serde(default = "default_true")]
    pub enabled: bool,

    /// TTL en segundos (default: 300 = 5 minutos)
    #[serde(default = "default_ttl")]
    pub ttl_seconds: u64,

    /// Capacidad maxima de entries (default: 10000)
    #[serde(default = "default_capacity")]
    pub max_capacity: u64,

    /// Time-to-idle en segundos (opcional)
    pub tti_seconds: Option<u64>,
}

fn default_true() -> bool {
    true
}

fn default_ttl() -> u64 {
    300
}

fn default_capacity() -> u64 {
    10_000
}

#[derive(Debug, Clone, Deserialize)]
pub struct BackendsConfig {
    /// Configuracion de Git backend
    #[cfg(feature = "git-backend")]
    pub git: Option<GitBackendConfig>,

    /// Configuracion de S3 backend
    #[cfg(feature = "s3-backend")]
    pub s3: Option<S3BackendConfig>,

    /// Configuracion de SQL backend
    #[cfg(feature = "sql-backend")]
    pub sql: Option<SqlBackendConfig>,
}

#[cfg(feature = "git-backend")]
#[derive(Debug, Clone, Deserialize)]
pub struct GitBackendConfig {
    /// URI del repositorio
    pub uri: String,
    /// Branch/tag por defecto
    #[serde(default = "default_label")]
    pub default_label: String,
    /// Directorio local para clone
    pub clone_dir: Option<PathBuf>,
    /// Intervalo de refresh en segundos
    pub refresh_interval_secs: Option<u64>,
}

fn default_label() -> String {
    "main".to_string()
}

#[cfg(feature = "s3-backend")]
#[derive(Debug, Clone, Deserialize)]
pub struct S3BackendConfig {
    pub bucket: String,
    pub region: Option<String>,
    pub prefix: Option<String>,
}

#[cfg(feature = "sql-backend")]
#[derive(Debug, Clone, Deserialize)]
pub struct SqlBackendConfig {
    pub url: String,
    pub max_connections: Option<u32>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct LoggingConfig {
    /// Nivel de log (trace, debug, info, warn, error)
    #[serde(default = "default_log_level")]
    pub level: String,
    /// Formato (json, pretty)
    #[serde(default = "default_log_format")]
    pub format: String,
}

fn default_log_level() -> String {
    "info".to_string()
}

fn default_log_format() -> String {
    "json".to_string()
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct MetricsConfig {
    /// Habilitar endpoint de metricas
    #[serde(default)]
    pub enabled: bool,
    /// Puerto para metricas (separado del principal)
    pub port: Option<u16>,
    /// Path del endpoint
    #[serde(default = "default_metrics_path")]
    pub path: String,
}

fn default_metrics_path() -> String {
    "/metrics".to_string()
}
```

### Paso 3: Implementar Config Loader

```rust
// src/config/loader.rs
use config::{Config, ConfigError, Environment, File};
use std::env;
use tracing::info;
use crate::config::settings::ServerSettings;
use crate::config::validation::validate_settings;

/// Error al cargar configuracion
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("failed to load configuration: {0}")]
    LoadError(#[from] ConfigError),

    #[error("configuration validation failed: {0}")]
    ValidationError(String),
}

/// Carga la configuracion del servidor desde multiples fuentes.
///
/// Orden de precedencia (mayor a menor):
/// 1. Environment variables (VORTEX_*)
/// 2. config/{env}.yaml (segun VORTEX_ENV o RUN_MODE)
/// 3. config/default.yaml
pub fn load_settings() -> Result<ServerSettings, SettingsError> {
    // Determinar entorno
    let run_mode = env::var("VORTEX_ENV")
        .or_else(|_| env::var("RUN_MODE"))
        .unwrap_or_else(|_| "development".to_string());

    info!(run_mode = %run_mode, "loading configuration");

    let settings = Config::builder()
        // Defaults desde archivo
        .add_source(
            File::with_name("config/default")
                .required(false)
        )
        // Override por entorno
        .add_source(
            File::with_name(&format!("config/{}", run_mode))
                .required(false)
        )
        // Environment variables con prefijo VORTEX_
        // VORTEX_SERVER__PORT=8081 -> server.port = 8081
        .add_source(
            Environment::with_prefix("VORTEX")
                .prefix_separator("_")
                .separator("__")
        )
        .build()?;

    // Deserializar a struct
    let settings: ServerSettings = settings.try_deserialize()?;

    // Validar
    validate_settings(&settings)?;

    info!("configuration loaded successfully");
    Ok(settings)
}

/// Carga configuracion desde un path especifico (para tests)
pub fn load_settings_from(path: &str) -> Result<ServerSettings, SettingsError> {
    let settings = Config::builder()
        .add_source(File::with_name(path))
        .build()?;

    let settings: ServerSettings = settings.try_deserialize()?;
    validate_settings(&settings)?;

    Ok(settings)
}
```

### Paso 4: Implementar Validacion

```rust
// src/config/validation.rs
use crate::config::settings::ServerSettings;
use crate::config::loader::SettingsError;

/// Valida la configuracion del servidor.
/// Retorna error si algun valor es invalido.
pub fn validate_settings(settings: &ServerSettings) -> Result<(), SettingsError> {
    validate_server(&settings.server)?;
    validate_cache(&settings.cache)?;
    validate_backends(&settings.backends)?;
    validate_logging(&settings.logging)?;

    Ok(())
}

fn validate_server(config: &crate::config::settings::ServerConfig) -> Result<(), SettingsError> {
    if config.port == 0 {
        return Err(SettingsError::ValidationError(
            "server.port must be greater than 0".to_string()
        ));
    }

    if config.graceful_shutdown_timeout_secs == 0 {
        return Err(SettingsError::ValidationError(
            "server.graceful_shutdown_timeout_secs must be greater than 0".to_string()
        ));
    }

    Ok(())
}

fn validate_cache(config: &crate::config::settings::CacheSettings) -> Result<(), SettingsError> {
    if config.enabled {
        if config.ttl_seconds == 0 {
            return Err(SettingsError::ValidationError(
                "cache.ttl_seconds must be greater than 0 when cache is enabled".to_string()
            ));
        }

        if config.max_capacity == 0 {
            return Err(SettingsError::ValidationError(
                "cache.max_capacity must be greater than 0 when cache is enabled".to_string()
            ));
        }

        if let Some(tti) = config.tti_seconds {
            if tti >= config.ttl_seconds {
                return Err(SettingsError::ValidationError(
                    "cache.tti_seconds should be less than ttl_seconds".to_string()
                ));
            }
        }
    }

    Ok(())
}

fn validate_backends(config: &crate::config::settings::BackendsConfig) -> Result<(), SettingsError> {
    let mut has_backend = false;

    #[cfg(feature = "git-backend")]
    if let Some(ref git) = config.git {
        has_backend = true;
        if git.uri.is_empty() {
            return Err(SettingsError::ValidationError(
                "backends.git.uri is required when git backend is configured".to_string()
            ));
        }
    }

    #[cfg(feature = "s3-backend")]
    if let Some(ref s3) = config.s3 {
        has_backend = true;
        if s3.bucket.is_empty() {
            return Err(SettingsError::ValidationError(
                "backends.s3.bucket is required when s3 backend is configured".to_string()
            ));
        }
    }

    #[cfg(feature = "sql-backend")]
    if let Some(ref sql) = config.sql {
        has_backend = true;
        if sql.url.is_empty() {
            return Err(SettingsError::ValidationError(
                "backends.sql.url is required when sql backend is configured".to_string()
            ));
        }
    }

    if !has_backend {
        return Err(SettingsError::ValidationError(
            "at least one backend must be configured".to_string()
        ));
    }

    Ok(())
}

fn validate_logging(config: &crate::config::settings::LoggingConfig) -> Result<(), SettingsError> {
    let valid_levels = ["trace", "debug", "info", "warn", "error"];
    if !valid_levels.contains(&config.level.to_lowercase().as_str()) {
        return Err(SettingsError::ValidationError(
            format!("logging.level must be one of: {:?}", valid_levels)
        ));
    }

    let valid_formats = ["json", "pretty"];
    if !valid_formats.contains(&config.format.to_lowercase().as_str()) {
        return Err(SettingsError::ValidationError(
            format!("logging.format must be one of: {:?}", valid_formats)
        ));
    }

    Ok(())
}
```

### Paso 5: Crear Archivo de Configuracion por Defecto

```yaml
# config/default.yaml
server:
  host: "0.0.0.0"
  port: 8080
  graceful_shutdown_timeout_secs: 30

cache:
  enabled: true
  ttl_seconds: 300
  max_capacity: 10000

backends:
  git:
    uri: "file:///config-repo"
    default_label: "main"

logging:
  level: "info"
  format: "json"

metrics:
  enabled: true
  path: "/metrics"
```

### Paso 6: Integrar en el Server

```rust
// src/main.rs o src/lib.rs
use crate::config::load_settings;
use std::net::SocketAddr;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Cargar configuracion primero
    let settings = load_settings()?;

    // Inicializar logging segun configuracion
    init_logging(&settings.logging);

    // Crear cache si esta habilitado
    let cache = if settings.cache.enabled {
        Some(ConfigCache::new(settings.cache.clone().into()))
    } else {
        None
    };

    // Crear app state
    let state = AppState::new(settings.clone(), cache);

    // Bind al address configurado
    let addr: SocketAddr = format!(
        "{}:{}",
        settings.server.host,
        settings.server.port
    ).parse()?;

    tracing::info!("starting server on {}", addr);

    run_server(addr, state).await
}
```

---

## Conceptos de Rust Aprendidos

### 1. Feature Flags en Cargo

Los feature flags permiten compilacion condicional, habilitando o deshabilitando codigo en compile-time.

**Rust (Cargo.toml):**
```toml
[features]
# Feature por defecto (se incluye si no se especifica otra cosa)
default = ["git-backend"]

# Features opcionales
git-backend = []
s3-backend = ["dep:aws-sdk-s3"]  # Activa dependencia opcional
sql-backend = ["dep:sqlx"]

# Feature que agrupa otros
full = ["git-backend", "s3-backend", "sql-backend"]

[dependencies]
# Dependencia siempre incluida
serde = "1"

# Dependencias opcionales (solo si feature activo)
aws-sdk-s3 = { version = "1", optional = true }
sqlx = { version = "0.7", optional = true }
```

**Uso en codigo:**
```rust
// Compilacion condicional con cfg
#[cfg(feature = "git-backend")]
pub mod git;

#[cfg(feature = "s3-backend")]
pub mod s3;

// En structs
pub struct BackendsConfig {
    #[cfg(feature = "git-backend")]
    pub git: Option<GitBackendConfig>,

    #[cfg(feature = "s3-backend")]
    pub s3: Option<S3BackendConfig>,
}

// En funciones
impl BackendsConfig {
    pub fn available_backends(&self) -> Vec<&str> {
        let mut backends = vec![];

        #[cfg(feature = "git-backend")]
        if self.git.is_some() {
            backends.push("git");
        }

        #[cfg(feature = "s3-backend")]
        if self.s3.is_some() {
            backends.push("s3");
        }

        backends
    }
}
```

**Compilacion con features:**
```bash
# Solo default features
cargo build

# Features especificos
cargo build --features "git-backend,s3-backend"

# Sin defaults + especificos
cargo build --no-default-features --features "s3-backend"

# Todos los features
cargo build --all-features
```

**Comparacion con Java (Maven profiles):**
```xml
<!-- pom.xml -->
<profiles>
    <profile>
        <id>git-backend</id>
        <activation>
            <activeByDefault>true</activeByDefault>
        </activation>
        <dependencies>
            <dependency>
                <groupId>org.eclipse.jgit</groupId>
                <artifactId>org.eclipse.jgit</artifactId>
            </dependency>
        </dependencies>
    </profile>
    <profile>
        <id>s3-backend</id>
        <dependencies>
            <dependency>
                <groupId>software.amazon.awssdk</groupId>
                <artifactId>s3</artifactId>
            </dependency>
        </dependencies>
    </profile>
</profiles>
```

```bash
mvn package -P git-backend,s3-backend
```

**Diferencias clave:**

| Aspecto | Cargo Features | Maven Profiles |
|---------|---------------|----------------|
| Granularidad | Compile-time, por linea | Build-time, por archivo |
| Codigo muerto | Eliminado completamente | Incluido (puede ser runtime) |
| Condiciones | `#[cfg(feature)]` | Sin condicionales en codigo |
| Dependencias | Activacion precisa | Todo o nada por perfil |

### 2. Environment Variables con Config Crate

El crate `config` permite cargar configuracion de multiples fuentes.

**Rust:**
```rust
use config::{Config, Environment, File};

let settings = Config::builder()
    // Carga archivo YAML
    .add_source(File::with_name("config/default"))
    // Carga archivo segun entorno
    .add_source(File::with_name(&format!("config/{}", env)))
    // Environment variables
    // VORTEX_SERVER__PORT=8081 -> server.port = 8081
    .add_source(
        Environment::with_prefix("VORTEX")
            .prefix_separator("_")   // VORTEX_*
            .separator("__")         // __ = nivel de anidacion
    )
    .build()?;

// Deserializa directamente a struct
let settings: ServerSettings = settings.try_deserialize()?;
```

**Mapeo de env vars:**
```bash
# Environment variables
export VORTEX_SERVER__PORT=8081
export VORTEX_CACHE__TTL_SECONDS=600
export VORTEX_BACKENDS__GIT__URI="https://github.com/repo"

# Se mapean a:
# server.port = 8081
# cache.ttl_seconds = 600
# backends.git.uri = "https://github.com/repo"
```

**Comparacion con Spring:**
```java
// application.yml
server:
  port: 8080
cache:
  ttl-seconds: 300

// @ConfigurationProperties
@ConfigurationProperties(prefix = "cache")
public class CacheProperties {
    private long ttlSeconds;
    // getters/setters
}

// Environment variables (Spring los mapea automaticamente)
// SERVER_PORT=8081
// CACHE_TTL_SECONDS=600
```

### 3. Serde Default Values

Serde permite especificar valores por defecto para campos opcionales.

**Rust:**
```rust
use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct CacheSettings {
    // Valor por defecto si no esta en config
    #[serde(default = "default_ttl")]
    pub ttl_seconds: u64,

    // Default usando Default trait
    #[serde(default)]
    pub enabled: bool,  // false por defecto

    // Default inline
    #[serde(default = "|| 10_000")]
    pub max_capacity: u64,
}

fn default_ttl() -> u64 {
    300
}

// Si el YAML no tiene cache.ttl_seconds, usa 300
// Si no tiene cache.enabled, usa false
```

**Comparacion con Java/Jackson:**
```java
public class CacheSettings {
    // Jackson no tiene defaults nativos
    // Opcion 1: Inicializar en declaracion
    private long ttlSeconds = 300;
    private boolean enabled = false;

    // Opcion 2: Usar @JsonSetter con @Value
    @JsonSetter
    public void setTtlSeconds(
        @Value("${cache.ttl-seconds:300}") long ttl
    ) {
        this.ttlSeconds = ttl;
    }
}
```

### 4. Validacion de Configuracion

Rust no tiene validacion de beans integrada como Java, pero podemos implementarla facilmente.

**Rust (validacion manual):**
```rust
pub fn validate_settings(settings: &ServerSettings) -> Result<(), SettingsError> {
    // Validacion explicita
    if settings.server.port == 0 {
        return Err(SettingsError::ValidationError(
            "server.port must be greater than 0".into()
        ));
    }

    Ok(())
}
```

**Rust (con validator crate):**
```rust
use validator::Validate;

#[derive(Debug, Deserialize, Validate)]
pub struct ServerConfig {
    #[validate(range(min = 1, max = 65535))]
    pub port: u16,

    #[validate(length(min = 1))]
    pub host: String,
}

// Uso
let config: ServerConfig = load()?;
config.validate()?;  // Retorna Result con errores
```

**Comparacion con Java (Bean Validation):**
```java
public class ServerConfig {
    @Min(1)
    @Max(65535)
    private int port;

    @NotBlank
    private String host;
}

// Validacion automatica con Spring
@Validated
@ConfigurationProperties(prefix = "server")
public class ServerConfig { ... }
```

---

## Riesgos y Errores Comunes

### 1. Olvidar `required = false` en archivos de config

```rust
// MAL: Falla si el archivo no existe
.add_source(File::with_name("config/production"))

// BIEN: Archivo opcional
.add_source(
    File::with_name("config/production")
        .required(false)
)
```

### 2. Separador incorrecto en env vars

```rust
// Configuracion
Environment::with_prefix("VORTEX")
    .separator("__")  // Doble underscore para niveles

// MAL: Single underscore no funciona para niveles
export VORTEX_SERVER_PORT=8081  // No parseara server.port

// BIEN: Doble underscore
export VORTEX_SERVER__PORT=8081  // Correcto: server.port = 8081
```

### 3. Feature flags no testeados

```rust
// Si solo testeas con default features, puedes tener errores ocultos

// CI debe testear todas las combinaciones importantes
// .github/workflows/ci.yml:
# cargo test --no-default-features --features s3-backend
# cargo test --all-features
```

### 4. Validacion despues de uso

```rust
// MAL: Usar configuracion antes de validar
let settings = config.try_deserialize::<ServerSettings>()?;
start_server(&settings);  // Puede fallar con valores invalidos
validate(&settings)?;

// BIEN: Validar primero
let settings = config.try_deserialize::<ServerSettings>()?;
validate(&settings)?;
start_server(&settings);
```

---

## Pruebas

### Tests Unitarios

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use std::env;

    #[test]
    fn test_load_default_settings() {
        // Crear archivo temporal de config
        let yaml = r#"
server:
  port: 8080
cache:
  enabled: true
  ttl_seconds: 300
backends:
  git:
    uri: "file:///test"
logging:
  level: info
  format: json
"#;
        // ... escribir a archivo temporal y cargar
    }

    #[test]
    fn test_env_override() {
        env::set_var("VORTEX_SERVER__PORT", "9090");

        let settings = load_settings().unwrap();

        assert_eq!(settings.server.port, 9090);

        env::remove_var("VORTEX_SERVER__PORT");
    }

    #[test]
    fn test_validation_fails_on_zero_port() {
        let settings = ServerSettings {
            server: ServerConfig {
                port: 0,  // Invalido
                ..Default::default()
            },
            ..Default::default()
        };

        let result = validate_settings(&settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("port"));
    }

    #[test]
    fn test_validation_fails_without_backend() {
        let settings = ServerSettings {
            backends: BackendsConfig {
                #[cfg(feature = "git-backend")]
                git: None,
                #[cfg(feature = "s3-backend")]
                s3: None,
                #[cfg(feature = "sql-backend")]
                sql: None,
            },
            ..Default::default()
        };

        let result = validate_settings(&settings);
        assert!(result.is_err());
    }

    #[test]
    #[cfg(feature = "git-backend")]
    fn test_git_backend_validation() {
        let settings = ServerSettings {
            backends: BackendsConfig {
                git: Some(GitBackendConfig {
                    uri: "".to_string(),  // Invalido
                    default_label: "main".to_string(),
                    clone_dir: None,
                    refresh_interval_secs: None,
                }),
            },
            ..Default::default()
        };

        let result = validate_settings(&settings);
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("uri"));
    }
}
```

### Tests de Feature Flags

```rust
#[cfg(test)]
mod feature_tests {
    use super::*;

    #[test]
    #[cfg(feature = "git-backend")]
    fn test_git_feature_enabled() {
        let backends = BackendsConfig::default();
        // git field debe existir
        let _ = backends.git;
    }

    #[test]
    #[cfg(not(feature = "s3-backend"))]
    fn test_s3_feature_disabled() {
        // Este test solo corre si s3-backend esta deshabilitado
        // Verificar que el codigo compila sin S3
    }
}
```

---

## Observabilidad

### Logging de Configuracion (sin secrets)

```rust
impl ServerSettings {
    /// Log de configuracion para debugging (sin valores sensibles)
    pub fn log_summary(&self) {
        tracing::info!(
            server.port = self.server.port,
            server.host = %self.server.host,
            cache.enabled = self.cache.enabled,
            cache.ttl = self.cache.ttl_seconds,
            logging.level = %self.logging.level,
            "configuration loaded"
        );

        #[cfg(feature = "git-backend")]
        if let Some(ref git) = self.backends.git {
            tracing::info!(
                backend = "git",
                uri = %git.uri,
                label = %git.default_label,
                "git backend configured"
            );
        }
    }
}
```

### Endpoint de Configuracion (opcional)

```rust
// GET /admin/config (solo valores no sensibles)
pub async fn get_config_summary(
    State(state): State<AppState>,
) -> Json<ConfigSummary> {
    Json(ConfigSummary {
        server_port: state.settings.server.port,
        cache_enabled: state.settings.cache.enabled,
        cache_ttl: state.settings.cache.ttl_seconds,
        backends: state.settings.backends.available_backends(),
    })
}
```

---

## Entregable Final

### Archivos Creados/Modificados

1. `crates/vortex-server/src/config/mod.rs` - Re-exports del modulo
2. `crates/vortex-server/src/config/settings.rs` - Structs de configuracion
3. `crates/vortex-server/src/config/loader.rs` - Logica de carga
4. `crates/vortex-server/src/config/validation.rs` - Validacion
5. `config/default.yaml` - Configuracion por defecto
6. `config/development.yaml` - Override para desarrollo
7. `config/production.yaml` - Override para produccion
8. `crates/vortex-server/Cargo.toml` - Features actualizados
9. `crates/vortex-server/tests/config_test.rs` - Tests

### Verificacion

```bash
# Compilar con diferentes features
cargo build -p vortex-server
cargo build -p vortex-server --all-features
cargo build -p vortex-server --no-default-features --features s3-backend

# Tests
cargo test -p vortex-server config

# Test con env vars
VORTEX_SERVER__PORT=9090 cargo test -p vortex-server

# Ejecutar con configuracion custom
VORTEX_ENV=production cargo run -p vortex-server
```

### Ejemplo de Uso

```yaml
# config/production.yaml
server:
  port: 8443
  graceful_shutdown_timeout_secs: 60

cache:
  ttl_seconds: 600
  max_capacity: 50000

backends:
  git:
    uri: "https://github.com/company/config-repo"
    default_label: "release"
    refresh_interval_secs: 30

logging:
  level: "warn"
  format: "json"

metrics:
  enabled: true
  port: 9090
```

```bash
# Ejecutar en produccion
VORTEX_ENV=production \
VORTEX_BACKENDS__GIT__URI="https://token@github.com/company/config-repo" \
cargo run --release -p vortex-server
```

---

**Anterior**: [Historia 002 - Invalidacion de Cache](./story-002-invalidation.md)
**Siguiente**: [Historia 004 - Metricas de Cache](./story-004-cache-metrics.md)
