# Historia 003: Lectura de Archivos de Configuracion

## Contexto y Objetivo

Una vez clonado el repositorio Git, necesitamos leer y parsear los archivos de configuracion. Esta historia implementa la logica para localizar archivos de configuracion segun las convenciones de Spring Cloud Config (application.yml, {app}.yml, {app}-{profile}.yml) y parsearlos en estructuras `ConfigMap`.

Para un desarrollador Java, esto es similar a como Spring Boot resuelve archivos de configuracion, pero con control explicito sobre el proceso de lectura y parsing, sin la "magia" de auto-configuracion.

## Alcance

### In Scope
- Resolucion de archivos por convencion Spring: `{app}.yml`, `{app}-{profile}.yml`
- Soporte de formatos: YAML, JSON, Properties
- Deteccion automatica de formato por extension
- Lectura asincrona de archivos con Tokio
- Merge de multiples archivos de configuracion
- Manejo de encoding UTF-8

### Out of Scope
- Archivos encriptados (epica de seguridad)
- Includes/imports entre archivos
- Watch de cambios en filesystem
- Archivos remotos (URLs)

## Criterios de Aceptacion

- [ ] Localiza archivos `application.yml` como base
- [ ] Localiza archivos `{app}.yml` para aplicacion especifica
- [ ] Localiza archivos `{app}-{profile}.yml` para profile especifico
- [ ] Soporta extensiones: `.yml`, `.yaml`, `.json`, `.properties`
- [ ] Merge correcto con prioridad: app-profile > app > application
- [ ] Error descriptivo si archivo existe pero no es parseable
- [ ] Operaciones de I/O no bloquean runtime async

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-git/src/reader/mod.rs` - Re-exports
- `vortex-git/src/reader/file.rs` - Lectura de archivos
- `vortex-git/src/reader/parser.rs` - Parsing y deteccion de formato
- `vortex-git/src/reader/resolver.rs` - Resolucion de paths

### Interfaces

```rust
/// Supported configuration file formats
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigFormat {
    Yaml,
    Json,
    Properties,
}

/// A configuration file with its content and metadata
#[derive(Debug)]
pub struct ConfigFile {
    pub path: PathBuf,
    pub format: ConfigFormat,
    pub content: String,
}

/// Resolver for configuration file paths
pub struct ConfigFileResolver {
    base_path: PathBuf,
}

impl ConfigFileResolver {
    /// Find all config files for given app/profiles
    pub fn resolve_files(
        &self,
        application: &str,
        profiles: &[String],
    ) -> Vec<PathBuf>;
}

/// Parser for configuration files
pub struct ConfigParser;

impl ConfigParser {
    /// Parse file content to ConfigMap
    pub fn parse(file: &ConfigFile) -> Result<ConfigMap, ConfigSourceError>;

    /// Detect format from file extension
    pub fn detect_format(path: &Path) -> Option<ConfigFormat>;
}
```

### Estructura Sugerida

```
crates/vortex-git/src/reader/
├── mod.rs          # pub mod file; pub mod parser; pub mod resolver;
├── file.rs         # Async file reading
├── parser.rs       # Format detection and parsing
└── resolver.rs     # Path resolution logic
```

## Pasos de Implementacion

1. **Implementar ConfigFormat enum**
   - Definir variantes para cada formato soportado
   - Metodo para detectar formato desde extension

2. **Implementar ConfigFileResolver**
   - Generar lista de posibles paths segun convenciones
   - Filtrar solo los que existen
   - Ordenar por prioridad

3. **Implementar lectura async de archivos**
   - Usar `tokio::fs::read_to_string`
   - Manejar errores de I/O

4. **Implementar ConfigParser**
   - Parser para YAML usando `serde_yaml`
   - Parser para JSON usando `serde_json`
   - Parser para Properties usando `java-properties`

5. **Implementar merge de configuraciones**
   - Usar merge de PropertySource de Epica 02

## Conceptos de Rust Aprendidos

### Path Handling en Rust

Rust tiene tipos dedicados para paths que son mas seguros que strings.

```rust
use std::path::{Path, PathBuf};

/// Resolver for configuration file paths following Spring conventions
pub struct ConfigFileResolver {
    base_path: PathBuf,
}

impl ConfigFileResolver {
    pub fn new(base_path: impl Into<PathBuf>) -> Self {
        Self {
            base_path: base_path.into(),
        }
    }

    /// Resolve all configuration files for an application and profiles
    /// Returns files in order of priority (lowest to highest)
    pub fn resolve_files(
        &self,
        application: &str,
        profiles: &[String],
    ) -> Vec<PathBuf> {
        let mut files = Vec::new();
        let extensions = ["yml", "yaml", "json", "properties"];

        // 1. Base: application.{ext}
        for ext in &extensions {
            let path = self.base_path.join(format!("application.{}", ext));
            if path.exists() {
                files.push(path);
                break;  // Solo el primer formato encontrado
            }
        }

        // 2. Application-specific: {app}.{ext}
        for ext in &extensions {
            let path = self.base_path.join(format!("{}.{}", application, ext));
            if path.exists() {
                files.push(path);
                break;
            }
        }

        // 3. Profile-specific: application-{profile}.{ext}
        for profile in profiles {
            for ext in &extensions {
                let path = self.base_path
                    .join(format!("application-{}.{}", profile, ext));
                if path.exists() {
                    files.push(path);
                    break;
                }
            }
        }

        // 4. App + Profile: {app}-{profile}.{ext}
        for profile in profiles {
            for ext in &extensions {
                let path = self.base_path
                    .join(format!("{}-{}.{}", application, profile, ext));
                if path.exists() {
                    files.push(path);
                    break;
                }
            }
        }

        files
    }
}
```

**Comparacion con Java:**

```java
// Java - Path handling con java.nio
public class ConfigFileResolver {
    private final Path basePath;

    public List<Path> resolveFiles(String application, List<String> profiles) {
        List<Path> files = new ArrayList<>();
        String[] extensions = {"yml", "yaml", "json", "properties"};

        // application.{ext}
        for (String ext : extensions) {
            Path path = basePath.resolve("application." + ext);
            if (Files.exists(path)) {
                files.add(path);
                break;
            }
        }

        // Similar for other patterns...
        return files;
    }
}
```

| Rust | Java |
|------|------|
| `PathBuf` (owned) | `Path` |
| `&Path` (borrowed) | `Path` |
| `path.join("foo")` | `path.resolve("foo")` |
| `path.exists()` | `Files.exists(path)` |
| `path.extension()` | `path.getFileName()` + parsing |

### Async File I/O con Tokio

Tokio proporciona versiones async de las operaciones de filesystem.

```rust
use tokio::fs;
use std::path::Path;
use anyhow::{Context, Result};

/// Read a configuration file asynchronously
pub async fn read_config_file(path: &Path) -> Result<ConfigFile> {
    // Verificar que el archivo existe
    if !path.exists() {
        anyhow::bail!("Configuration file not found: {}", path.display());
    }

    // Detectar formato desde extension
    let format = ConfigParser::detect_format(path)
        .ok_or_else(|| anyhow::anyhow!(
            "Unsupported file format: {}",
            path.display()
        ))?;

    // Leer contenido de forma async
    let content = fs::read_to_string(path)
        .await
        .with_context(|| format!(
            "Failed to read configuration file: {}",
            path.display()
        ))?;

    Ok(ConfigFile {
        path: path.to_path_buf(),
        format,
        content,
    })
}

/// Read multiple configuration files in parallel
pub async fn read_all_config_files(
    paths: Vec<PathBuf>,
) -> Vec<Result<ConfigFile>> {
    use futures::future::join_all;

    // Crear futures para cada archivo
    let futures: Vec<_> = paths
        .into_iter()
        .map(|path| async move {
            read_config_file(&path).await
        })
        .collect();

    // Ejecutar todos en paralelo
    join_all(futures).await
}
```

**Comparacion con Java (NIO.2 async):**

```java
// Java - AsynchronousFileChannel
public CompletableFuture<String> readConfigFileAsync(Path path) {
    return CompletableFuture.supplyAsync(() -> {
        try {
            return Files.readString(path);
        } catch (IOException e) {
            throw new CompletionException(e);
        }
    });
}

// Leer multiples archivos en paralelo
public List<CompletableFuture<String>> readAllConfigFiles(List<Path> paths) {
    return paths.stream()
        .map(this::readConfigFileAsync)
        .collect(Collectors.toList());
}
```

### Parsing con Deteccion de Formato

```rust
use std::path::Path;
use vortex_core::ConfigMap;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ConfigFormat {
    Yaml,
    Json,
    Properties,
}

pub struct ConfigParser;

impl ConfigParser {
    /// Detect format from file extension
    pub fn detect_format(path: &Path) -> Option<ConfigFormat> {
        // extension() retorna Option<&OsStr>
        let ext = path.extension()?.to_str()?;

        match ext.to_lowercase().as_str() {
            "yml" | "yaml" => Some(ConfigFormat::Yaml),
            "json" => Some(ConfigFormat::Json),
            "properties" => Some(ConfigFormat::Properties),
            _ => None,
        }
    }

    /// Parse configuration content based on format
    pub fn parse(file: &ConfigFile) -> Result<ConfigMap, ConfigSourceError> {
        match file.format {
            ConfigFormat::Yaml => Self::parse_yaml(&file.content, &file.path),
            ConfigFormat::Json => Self::parse_json(&file.content, &file.path),
            ConfigFormat::Properties => {
                Self::parse_properties(&file.content, &file.path)
            }
        }
    }

    fn parse_yaml(content: &str, path: &Path) -> Result<ConfigMap, ConfigSourceError> {
        serde_yaml::from_str(content)
            .map_err(|e| ConfigSourceError::ParseError {
                path: path.to_path_buf(),
                format: "YAML".into(),
                details: e.to_string(),
            })
    }

    fn parse_json(content: &str, path: &Path) -> Result<ConfigMap, ConfigSourceError> {
        serde_json::from_str(content)
            .map_err(|e| ConfigSourceError::ParseError {
                path: path.to_path_buf(),
                format: "JSON".into(),
                details: e.to_string(),
            })
    }

    fn parse_properties(
        content: &str,
        path: &Path,
    ) -> Result<ConfigMap, ConfigSourceError> {
        use java_properties::read;
        use std::io::Cursor;

        let cursor = Cursor::new(content.as_bytes());
        let props = read(cursor)
            .map_err(|e| ConfigSourceError::ParseError {
                path: path.to_path_buf(),
                format: "Properties".into(),
                details: e.to_string(),
            })?;

        // Convertir HashMap<String, String> a ConfigMap
        let mut config = ConfigMap::new();
        for (key, value) in props {
            config.insert(key, ConfigValue::String(value));
        }

        Ok(config)
    }
}
```

**Comparacion con Java:**

```java
// Java - Factory pattern para parsers
public interface ConfigParser {
    ConfigMap parse(String content) throws ParseException;
}

public class ConfigParserFactory {
    public static ConfigParser forFormat(ConfigFormat format) {
        return switch (format) {
            case YAML -> new YamlParser();
            case JSON -> new JsonParser();
            case PROPERTIES -> new PropertiesParser();
        };
    }
}

// Yaml parser con SnakeYAML
public class YamlParser implements ConfigParser {
    private final Yaml yaml = new Yaml();

    @Override
    public ConfigMap parse(String content) {
        Map<String, Object> data = yaml.load(content);
        return ConfigMap.fromMap(data);
    }
}
```

### Merge de Configuraciones

```rust
use vortex_core::{ConfigMap, PropertySource};

/// Merge multiple config files into a single PropertySource
pub async fn load_and_merge_configs(
    resolver: &ConfigFileResolver,
    application: &str,
    profiles: &[String],
) -> Result<PropertySource, ConfigSourceError> {
    // Resolver paths (ya ordenados por prioridad)
    let paths = resolver.resolve_files(application, profiles);

    if paths.is_empty() {
        return Err(ConfigSourceError::NotFound {
            application: application.to_string(),
            profiles: profiles.to_vec(),
        });
    }

    // Leer todos los archivos
    let mut merged = ConfigMap::new();

    for path in paths {
        let file = read_config_file(&path).await?;
        let config = ConfigParser::parse(&file)?;

        // Merge: archivos posteriores tienen mayor prioridad
        // y sobrescriben valores anteriores
        merged = merge_configs(merged, config);

        tracing::debug!(
            path = %path.display(),
            "Merged configuration file"
        );
    }

    // Crear PropertySource con nombre descriptivo
    let source_name = format!(
        "git:{}:{}",
        application,
        profiles.join(",")
    );

    Ok(PropertySource::new(source_name, merged))
}

/// Deep merge two ConfigMaps (right has priority)
fn merge_configs(base: ConfigMap, overlay: ConfigMap) -> ConfigMap {
    let mut result = base;

    for (key, value) in overlay.into_iter() {
        match (result.get(&key), &value) {
            // Si ambos son objetos, merge recursivo
            (Some(ConfigValue::Object(base_obj)), ConfigValue::Object(overlay_obj)) => {
                let merged_obj = merge_objects(base_obj.clone(), overlay_obj);
                result.insert(key, ConfigValue::Object(merged_obj));
            }
            // En cualquier otro caso, overlay gana
            _ => {
                result.insert(key, value);
            }
        }
    }

    result
}
```

## Riesgos y Errores Comunes

### 1. Path traversal vulnerability

```rust
// ERROR: Permite leer archivos fuera del repo
fn read_file(base: &Path, filename: &str) -> Result<String> {
    let path = base.join(filename);  // filename podria ser "../../../etc/passwd"
    std::fs::read_to_string(path)
}

// CORRECTO: Validar que el path resultante esta dentro del base
fn read_file_safe(base: &Path, filename: &str) -> Result<String> {
    let path = base.join(filename);
    let canonical = path.canonicalize()?;
    let base_canonical = base.canonicalize()?;

    if !canonical.starts_with(&base_canonical) {
        anyhow::bail!(
            "Path traversal attempt detected: {}",
            filename
        );
    }

    std::fs::read_to_string(canonical)
}
```

### 2. No manejar archivos grandes

```rust
// ERROR: Lee todo en memoria, puede causar OOM
async fn read_file(path: &Path) -> Result<String> {
    tokio::fs::read_to_string(path).await?
}

// MEJOR: Verificar tamano primero
async fn read_file_safe(path: &Path, max_size: u64) -> Result<String> {
    let metadata = tokio::fs::metadata(path).await?;

    if metadata.len() > max_size {
        anyhow::bail!(
            "Configuration file too large: {} bytes (max: {})",
            metadata.len(),
            max_size
        );
    }

    tokio::fs::read_to_string(path).await.map_err(Into::into)
}
```

### 3. Encoding incorrecto

```rust
// ERROR: Asume UTF-8, falla silenciosamente con otros encodings
async fn read_file(path: &Path) -> Result<String> {
    tokio::fs::read_to_string(path).await?
}

// MEJOR: Ser explicito sobre el encoding esperado
async fn read_file_utf8(path: &Path) -> Result<String> {
    let bytes = tokio::fs::read(path).await?;

    String::from_utf8(bytes).map_err(|e| {
        anyhow::anyhow!(
            "File {} is not valid UTF-8: {}",
            path.display(),
            e
        )
    })
}
```

## Pruebas

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn create_test_files(dir: &Path) {
        // application.yml
        std::fs::write(
            dir.join("application.yml"),
            "server:\n  port: 8080"
        ).unwrap();

        // myapp.yml
        std::fs::write(
            dir.join("myapp.yml"),
            "app:\n  name: myapp"
        ).unwrap();

        // myapp-prod.yml
        std::fs::write(
            dir.join("myapp-prod.yml"),
            "server:\n  port: 80\napp:\n  env: production"
        ).unwrap();
    }

    #[test]
    fn test_detect_format() {
        assert_eq!(
            ConfigParser::detect_format(Path::new("config.yml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigParser::detect_format(Path::new("config.yaml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigParser::detect_format(Path::new("config.json")),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigParser::detect_format(Path::new("config.properties")),
            Some(ConfigFormat::Properties)
        );
        assert_eq!(
            ConfigParser::detect_format(Path::new("config.txt")),
            None
        );
    }

    #[test]
    fn test_resolve_files_order() {
        let temp_dir = TempDir::new().unwrap();
        create_test_files(temp_dir.path());

        let resolver = ConfigFileResolver::new(temp_dir.path());
        let files = resolver.resolve_files("myapp", &["prod".into()]);

        // Verificar orden: application.yml, myapp.yml, myapp-prod.yml
        assert_eq!(files.len(), 3);
        assert!(files[0].ends_with("application.yml"));
        assert!(files[1].ends_with("myapp.yml"));
        assert!(files[2].ends_with("myapp-prod.yml"));
    }

    #[test]
    fn test_parse_yaml() {
        let file = ConfigFile {
            path: PathBuf::from("test.yml"),
            format: ConfigFormat::Yaml,
            content: "server:\n  port: 8080\n  host: localhost".into(),
        };

        let config = ConfigParser::parse(&file).unwrap();

        assert_eq!(
            config.get("server.port").unwrap().as_i64(),
            Some(8080)
        );
        assert_eq!(
            config.get("server.host").unwrap().as_str(),
            Some("localhost")
        );
    }

    #[test]
    fn test_parse_json() {
        let file = ConfigFile {
            path: PathBuf::from("test.json"),
            format: ConfigFormat::Json,
            content: r#"{"database": {"url": "jdbc:postgresql://localhost/db"}}"#.into(),
        };

        let config = ConfigParser::parse(&file).unwrap();

        assert_eq!(
            config.get("database.url").unwrap().as_str(),
            Some("jdbc:postgresql://localhost/db")
        );
    }

    #[test]
    fn test_parse_properties() {
        let file = ConfigFile {
            path: PathBuf::from("test.properties"),
            format: ConfigFormat::Properties,
            content: "server.port=8080\nserver.host=localhost".into(),
        };

        let config = ConfigParser::parse(&file).unwrap();

        // Properties no crea estructura anidada automaticamente
        assert_eq!(
            config.get("server.port").unwrap().as_str(),
            Some("8080")
        );
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let file = ConfigFile {
            path: PathBuf::from("test.yml"),
            format: ConfigFormat::Yaml,
            content: "invalid: yaml: content:".into(),
        };

        let result = ConfigParser::parse(&file);
        assert!(result.is_err());

        let err = result.unwrap_err();
        assert!(matches!(err, ConfigSourceError::ParseError { .. }));
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;

    #[tokio::test]
    async fn test_load_and_merge_configs() {
        let temp_dir = TempDir::new().unwrap();
        let base = temp_dir.path();

        // Crear archivos con configuraciones que se solapan
        std::fs::write(
            base.join("application.yml"),
            "server:\n  port: 8080\n  timeout: 30"
        ).unwrap();

        std::fs::write(
            base.join("myapp.yml"),
            "server:\n  port: 9000\napp:\n  name: myapp"
        ).unwrap();

        std::fs::write(
            base.join("myapp-prod.yml"),
            "server:\n  port: 80"
        ).unwrap();

        let resolver = ConfigFileResolver::new(base);
        let result = load_and_merge_configs(
            &resolver,
            "myapp",
            &["prod".into()],
        ).await.unwrap();

        let config = result.source();

        // server.port debe ser 80 (de myapp-prod.yml, mayor prioridad)
        assert_eq!(config.get("server.port").unwrap().as_i64(), Some(80));

        // server.timeout debe ser 30 (de application.yml)
        assert_eq!(config.get("server.timeout").unwrap().as_i64(), Some(30));

        // app.name debe ser myapp (de myapp.yml)
        assert_eq!(config.get("app.name").unwrap().as_str(), Some("myapp"));
    }

    #[tokio::test]
    async fn test_no_config_files_found() {
        let temp_dir = TempDir::new().unwrap();
        let resolver = ConfigFileResolver::new(temp_dir.path());

        let result = load_and_merge_configs(
            &resolver,
            "nonexistent",
            &["default".into()],
        ).await;

        assert!(matches!(
            result,
            Err(ConfigSourceError::NotFound { .. })
        ));
    }
}
```

## Observabilidad

```rust
use tracing::{instrument, info, debug, warn};

impl ConfigFileResolver {
    #[instrument(skip(self), fields(base_path = %self.base_path.display()))]
    pub fn resolve_files(
        &self,
        application: &str,
        profiles: &[String],
    ) -> Vec<PathBuf> {
        let files = self.resolve_files_internal(application, profiles);

        info!(
            application = %application,
            profiles = ?profiles,
            files_found = files.len(),
            "Resolved configuration files"
        );

        for (i, file) in files.iter().enumerate() {
            debug!(
                priority = i,
                path = %file.display(),
                "Configuration file in resolution order"
            );
        }

        files
    }
}

pub async fn read_config_file(path: &Path) -> Result<ConfigFile> {
    let start = std::time::Instant::now();

    let result = read_config_file_internal(path).await;

    match &result {
        Ok(file) => {
            info!(
                path = %path.display(),
                format = ?file.format,
                size_bytes = file.content.len(),
                duration_ms = %start.elapsed().as_millis(),
                "Read configuration file"
            );
        }
        Err(e) => {
            warn!(
                path = %path.display(),
                error = %e,
                duration_ms = %start.elapsed().as_millis(),
                "Failed to read configuration file"
            );
        }
    }

    result
}
```

## Entregable Final

- PR con:
  - `crates/vortex-git/src/reader/mod.rs`
  - `crates/vortex-git/src/reader/file.rs`
  - `crates/vortex-git/src/reader/parser.rs`
  - `crates/vortex-git/src/reader/resolver.rs`
  - Tests unitarios para cada componente
  - Tests de integracion con archivos temporales
  - Logging con tracing
  - Documentacion de la convencion de nombres de archivos

---

**Anterior**: [Historia 002 - Clone y Pull](./story-002-clone-pull.md)
**Siguiente**: [Historia 004 - Soporte de Labels](./story-004-labels-support.md)
