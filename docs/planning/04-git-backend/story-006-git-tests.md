# Historia 006: Tests con Repositorio Local

## Contexto y Objetivo

Para garantizar la calidad del backend Git, necesitamos una suite de tests robusta que pueda ejecutarse sin dependencias externas (repositorios remotos). Esta historia implementa infraestructura de testing usando repositorios Git locales creados dinamicamente, permitiendo tests rapidos, reproducibles y aislados.

Para un desarrollador Java, esto es similar a usar `@TempDir` de JUnit 5 para crear directorios temporales, pero con helpers especificos para crear repositorios Git de prueba. La crate `tempfile` proporciona directorios temporales que se limpian automaticamente.

## Alcance

### In Scope
- Test fixtures para crear repositorios Git de prueba
- Helpers para crear commits, branches, tags
- Tests de integracion para clone, pull, checkout
- Tests de lectura de configuracion
- Tests del scheduler de refresh
- Documentacion de como agregar nuevos tests

### Out of Scope
- Tests de performance/benchmark (epica futura)
- Tests de carga con multiples clientes
- Tests end-to-end con servidor HTTP

## Criterios de Aceptacion

- [ ] Test fixtures reutilizables para crear repos
- [ ] Tests cubren happy path y error cases
- [ ] Tests son independientes y aislados
- [ ] Tests limpian recursos automaticamente
- [ ] Cobertura > 80% del codigo del backend Git
- [ ] Tests pueden ejecutarse en CI sin red externa
- [ ] Documentacion de como escribir nuevos tests

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-git/tests/helpers/mod.rs` - Test utilities
- `vortex-git/tests/helpers/repo.rs` - Repository fixtures
- `vortex-git/tests/helpers/config.rs` - Config file fixtures
- `vortex-git/tests/clone_tests.rs` - Clone tests
- `vortex-git/tests/checkout_tests.rs` - Checkout tests
- `vortex-git/tests/reader_tests.rs` - Config reader tests
- `vortex-git/tests/integration_tests.rs` - Full integration tests

### Interfaces

```rust
/// Builder for creating test Git repositories
pub struct TestRepoBuilder {
    path: PathBuf,
    branches: Vec<BranchSpec>,
    tags: Vec<TagSpec>,
    files: Vec<FileSpec>,
}

impl TestRepoBuilder {
    pub fn new() -> Self;
    pub fn with_branch(self, name: &str) -> Self;
    pub fn with_tag(self, name: &str) -> Self;
    pub fn with_file(self, path: &str, content: &str) -> Self;
    pub fn build(self) -> Result<TestRepo, Error>;
}

/// A test repository that cleans up automatically
pub struct TestRepo {
    pub path: PathBuf,
    _temp_dir: TempDir,  // Dropped when TestRepo drops
}

impl TestRepo {
    pub fn commit(&self, message: &str) -> Result<String, Error>;
    pub fn create_branch(&self, name: &str) -> Result<(), Error>;
    pub fn create_tag(&self, name: &str) -> Result<(), Error>;
}
```

### Estructura Sugerida

```
crates/vortex-git/tests/
├── helpers/
│   ├── mod.rs          # pub mod repo; pub mod config;
│   ├── repo.rs         # TestRepoBuilder, TestRepo
│   └── config.rs       # Config file helpers
├── clone_tests.rs
├── pull_tests.rs
├── checkout_tests.rs
├── reader_tests.rs
├── refresh_tests.rs
└── integration_tests.rs
```

## Pasos de Implementacion

1. **Crear modulo de helpers**
   - Estructura de directorios
   - Re-exports en mod.rs

2. **Implementar TestRepoBuilder**
   - Builder pattern para configurar repo
   - Metodo build() que inicializa el repo

3. **Implementar TestRepo**
   - Wrapper sobre tempfile::TempDir
   - Metodos para operaciones Git comunes

4. **Escribir tests de cada componente**
   - Clone/pull tests
   - Checkout tests
   - Reader tests
   - Integration tests

5. **Configurar CI**
   - Asegurar que tests corren sin red

## Conceptos de Rust Aprendidos

### tempfile para Directorios Temporales

La crate `tempfile` crea directorios que se eliminan automaticamente cuando salen del scope (RAII pattern).

```rust
use tempfile::TempDir;
use std::path::PathBuf;
use std::process::Command;
use anyhow::{Context, Result};

/// A test Git repository that cleans up automatically
pub struct TestRepo {
    path: PathBuf,
    _temp_dir: TempDir,  // Underscore porque no lo usamos directamente
}

impl TestRepo {
    /// Create a new empty test repository
    pub fn new() -> Result<Self> {
        let temp_dir = TempDir::new()
            .context("Failed to create temp directory")?;

        let path = temp_dir.path().to_path_buf();

        // Initialize git repo
        git_command(&path, &["init"])?;

        // Configure git (necesario para commits)
        git_command(&path, &["config", "user.email", "test@example.com"])?;
        git_command(&path, &["config", "user.name", "Test User"])?;

        Ok(Self {
            path,
            _temp_dir: temp_dir,
        })
    }

    /// Get the path to the repository
    pub fn path(&self) -> &PathBuf {
        &self.path
    }

    /// Create a file and commit it
    pub fn commit_file(
        &self,
        filename: &str,
        content: &str,
        message: &str,
    ) -> Result<String> {
        // Write file
        let file_path = self.path.join(filename);
        if let Some(parent) = file_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&file_path, content)?;

        // Stage and commit
        git_command(&self.path, &["add", filename])?;
        git_command(&self.path, &["commit", "-m", message])?;

        // Get commit hash
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(&self.path)
            .output()?;

        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    }
}

// Cuando TestRepo sale del scope, TempDir se dropea
// y el directorio se elimina automaticamente
fn example() {
    let repo = TestRepo::new().unwrap();
    // ... usar repo ...
}  // Directorio eliminado aqui

/// Helper para ejecutar comandos git
fn git_command(path: &PathBuf, args: &[&str]) -> Result<()> {
    let output = Command::new("git")
        .args(args)
        .current_dir(path)
        .output()
        .context("Failed to execute git command")?;

    if !output.status.success() {
        anyhow::bail!(
            "Git command failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }

    Ok(())
}
```

**Comparacion con Java (JUnit 5):**

```java
// Java con JUnit 5
class GitTests {
    @TempDir
    Path tempDir;

    private Path repoPath;

    @BeforeEach
    void setUp() throws Exception {
        repoPath = tempDir.resolve("repo");
        Files.createDirectories(repoPath);

        // git init
        new ProcessBuilder("git", "init")
            .directory(repoPath.toFile())
            .start()
            .waitFor();
    }

    // tempDir se limpia automaticamente despues del test
}
```

### Test Fixtures con Builder Pattern

```rust
use std::collections::HashMap;

/// Specification for a branch
#[derive(Debug, Clone)]
pub struct BranchSpec {
    pub name: String,
    pub files: HashMap<String, String>,
}

/// Builder for test repositories
pub struct TestRepoBuilder {
    initial_files: HashMap<String, String>,
    branches: Vec<BranchSpec>,
    tags: Vec<String>,
}

impl TestRepoBuilder {
    pub fn new() -> Self {
        Self {
            initial_files: HashMap::new(),
            branches: Vec::new(),
            tags: Vec::new(),
        }
    }

    /// Add a file to the initial commit
    pub fn with_file(mut self, path: &str, content: &str) -> Self {
        self.initial_files.insert(path.to_string(), content.to_string());
        self
    }

    /// Add a branch with specific files
    pub fn with_branch(mut self, name: &str, files: HashMap<String, String>) -> Self {
        self.branches.push(BranchSpec {
            name: name.to_string(),
            files,
        });
        self
    }

    /// Add a tag at current commit
    pub fn with_tag(mut self, name: &str) -> Self {
        self.tags.push(name.to_string());
        self
    }

    /// Build the test repository
    pub fn build(self) -> Result<TestRepo> {
        let repo = TestRepo::new()?;

        // Create initial commit
        for (path, content) in &self.initial_files {
            let file_path = repo.path.join(path);
            if let Some(parent) = file_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::write(&file_path, content)?;
        }

        if !self.initial_files.is_empty() {
            git_command(&repo.path, &["add", "."])?;
            git_command(&repo.path, &["commit", "-m", "Initial commit"])?;
        }

        // Create tags
        for tag in &self.tags {
            git_command(&repo.path, &["tag", tag])?;
        }

        // Create branches
        for branch in &self.branches {
            git_command(&repo.path, &["checkout", "-b", &branch.name])?;

            for (path, content) in &branch.files {
                let file_path = repo.path.join(path);
                std::fs::write(&file_path, content)?;
            }

            if !branch.files.is_empty() {
                git_command(&repo.path, &["add", "."])?;
                git_command(&repo.path, &[
                    "commit", "-m",
                    &format!("Changes for {}", branch.name)
                ])?;
            }
        }

        // Return to main branch
        git_command(&repo.path, &["checkout", "main"]).ok();

        Ok(repo)
    }
}

// Uso
fn create_test_repo() -> Result<TestRepo> {
    TestRepoBuilder::new()
        .with_file("application.yml", "server:\n  port: 8080")
        .with_file("myapp.yml", "app:\n  name: myapp")
        .with_tag("v1.0.0")
        .with_branch("develop", {
            let mut files = HashMap::new();
            files.insert(
                "application.yml".to_string(),
                "server:\n  port: 9090".to_string()
            );
            files
        })
        .build()
}
```

### Integration Tests en Rust

Los tests de integracion en Rust van en el directorio `tests/` y se compilan como binarios separados.

```rust
// tests/integration_tests.rs

mod helpers;

use helpers::repo::TestRepoBuilder;
use vortex_git::{GitBackend, ConfigQuery};
use std::collections::HashMap;

/// Test the complete flow: clone -> read config -> checkout -> read again
#[tokio::test]
async fn test_full_config_flow() {
    // Setup: Create test repo with branches
    let test_repo = TestRepoBuilder::new()
        .with_file("application.yml", "server:\n  port: 8080\n  env: default")
        .with_file("myapp.yml", "app:\n  name: myapp")
        .with_file("myapp-prod.yml", "server:\n  port: 80\n  env: prod")
        .with_tag("v1.0.0")
        .with_branch("develop", {
            let mut files = HashMap::new();
            files.insert(
                "application.yml".to_string(),
                "server:\n  port: 9090\n  env: develop".to_string()
            );
            files
        })
        .build()
        .expect("Failed to create test repo");

    // Create backend pointing to test repo
    let backend = GitBackend::new(GitBackendConfig::new(
        test_repo.path().to_string_lossy(),
        tempfile::tempdir().unwrap().path(),
    )).await.expect("Failed to create backend");

    // Test 1: Get config from main branch
    let query = ConfigQuery::new("myapp")
        .with_profiles(vec!["prod".into()]);

    let config = backend
        .get_config(&query)
        .await
        .expect("Failed to get config");

    // Should merge application.yml + myapp.yml + myapp-prod.yml
    assert_eq!(
        config.source().get("server.port").unwrap().as_i64(),
        Some(80)
    );
    assert_eq!(
        config.source().get("server.env").unwrap().as_str(),
        Some("prod")
    );
    assert_eq!(
        config.source().get("app.name").unwrap().as_str(),
        Some("myapp")
    );

    // Test 2: Get config from develop branch
    let query_develop = ConfigQuery::new("myapp")
        .with_label("develop");

    let config_develop = backend
        .get_config(&query_develop)
        .await
        .expect("Failed to get develop config");

    assert_eq!(
        config_develop.source().get("server.port").unwrap().as_i64(),
        Some(9090)
    );
    assert_eq!(
        config_develop.source().get("server.env").unwrap().as_str(),
        Some("develop")
    );
}

#[tokio::test]
async fn test_config_not_found() {
    let test_repo = TestRepoBuilder::new()
        .with_file("application.yml", "default: value")
        .build()
        .expect("Failed to create test repo");

    let backend = GitBackend::new(GitBackendConfig::new(
        test_repo.path().to_string_lossy(),
        tempfile::tempdir().unwrap().path(),
    )).await.expect("Failed to create backend");

    // Query for non-existent app with strict mode
    let query = ConfigQuery::new("nonexistent-app")
        .with_profiles(vec!["prod".into()]);

    // Should still return application.yml as fallback
    let result = backend.get_config(&query).await;
    assert!(result.is_ok()); // Returns default config
}

#[tokio::test]
async fn test_invalid_label() {
    let test_repo = TestRepoBuilder::new()
        .with_file("application.yml", "value: test")
        .build()
        .expect("Failed to create test repo");

    let backend = GitBackend::new(GitBackendConfig::new(
        test_repo.path().to_string_lossy(),
        tempfile::tempdir().unwrap().path(),
    )).await.expect("Failed to create backend");

    let query = ConfigQuery::new("myapp")
        .with_label("nonexistent-branch");

    let result = backend.get_config(&query).await;
    assert!(result.is_err());

    let error = result.unwrap_err();
    // Error should mention the invalid label
    assert!(error.to_string().contains("nonexistent-branch")
        || matches!(error, ConfigSourceError::InvalidLabel { .. }));
}
```

### Test Organization y Modularidad

```rust
// tests/helpers/mod.rs
pub mod repo;
pub mod config;

// Re-export commonly used items
pub use repo::{TestRepo, TestRepoBuilder};
pub use config::ConfigFileHelper;

// tests/helpers/config.rs
use std::path::Path;

/// Helper para crear archivos de configuracion de prueba
pub struct ConfigFileHelper;

impl ConfigFileHelper {
    /// Create a typical Spring-style application.yml
    pub fn spring_application_yml() -> &'static str {
        r#"
spring:
  application:
    name: test-app
  datasource:
    url: jdbc:postgresql://localhost/testdb
    username: testuser

server:
  port: 8080

management:
  endpoints:
    web:
      exposure:
        include: health,info
"#
    }

    /// Create a production profile config
    pub fn prod_profile_yml() -> &'static str {
        r#"
server:
  port: 80

spring:
  datasource:
    url: jdbc:postgresql://prod-db/proddb

logging:
  level:
    root: WARN
"#
    }

    /// Write config files to a directory
    pub fn write_standard_configs(dir: &Path) -> std::io::Result<()> {
        std::fs::write(
            dir.join("application.yml"),
            Self::spring_application_yml()
        )?;
        std::fs::write(
            dir.join("application-prod.yml"),
            Self::prod_profile_yml()
        )?;
        Ok(())
    }
}

// tests/reader_tests.rs
mod helpers;

use helpers::{TestRepoBuilder, ConfigFileHelper};
use vortex_git::reader::{ConfigFileResolver, ConfigParser};

#[tokio::test]
async fn test_resolve_standard_config_files() {
    let test_repo = TestRepoBuilder::new()
        .with_file("application.yml", ConfigFileHelper::spring_application_yml())
        .with_file("application-prod.yml", ConfigFileHelper::prod_profile_yml())
        .with_file("myapp.yml", "app:\n  custom: value")
        .with_file("myapp-prod.yml", "app:\n  env: production")
        .build()
        .unwrap();

    let resolver = ConfigFileResolver::new(test_repo.path());

    // Test resolution for myapp with prod profile
    let files = resolver.resolve_files("myapp", &["prod".into()]);

    assert_eq!(files.len(), 4);
    assert!(files[0].ends_with("application.yml"));
    assert!(files[1].ends_with("myapp.yml"));
    assert!(files[2].ends_with("application-prod.yml"));
    assert!(files[3].ends_with("myapp-prod.yml"));
}

#[test]
fn test_parse_yaml_config() {
    let content = r#"
server:
  port: 8080
  host: localhost
database:
  pool:
    size: 10
"#;

    let file = ConfigFile {
        path: PathBuf::from("test.yml"),
        format: ConfigFormat::Yaml,
        content: content.to_string(),
    };

    let config = ConfigParser::parse(&file).unwrap();

    assert_eq!(config.get("server.port").unwrap().as_i64(), Some(8080));
    assert_eq!(config.get("server.host").unwrap().as_str(), Some("localhost"));
    assert_eq!(config.get("database.pool.size").unwrap().as_i64(), Some(10));
}
```

## Riesgos y Errores Comunes

### 1. Tests que dependen de orden de ejecucion

```rust
// ERROR: Test modifica estado global
static mut COUNTER: u32 = 0;

#[test]
fn test_a() {
    unsafe { COUNTER += 1; }
    assert_eq!(unsafe { COUNTER }, 1);
}

#[test]
fn test_b() {
    // Puede fallar si test_a corre primero
    assert_eq!(unsafe { COUNTER }, 0);
}

// CORRECTO: Tests completamente aislados
#[test]
fn test_a() {
    let repo = TestRepo::new().unwrap();
    // Usar repo local, sin estado compartido
}

#[test]
fn test_b() {
    let repo = TestRepo::new().unwrap();
    // Completamente independiente
}
```

### 2. No limpiar recursos en caso de panic

```rust
// ERROR: Si el test falla, el temp dir puede no limpiarse
#[test]
fn bad_test() {
    let temp = std::env::temp_dir().join("my-test");
    std::fs::create_dir(&temp).unwrap();

    // Si esto falla...
    assert!(false);

    // ...esto nunca se ejecuta
    std::fs::remove_dir_all(&temp).unwrap();
}

// CORRECTO: TempDir se limpia automaticamente
#[test]
fn good_test() {
    let temp_dir = TempDir::new().unwrap();

    // Aunque falle...
    assert!(true);

}  // TempDir se limpia aqui, incluso si panic
```

### 3. Tests que requieren recursos externos

```rust
// ERROR: Requiere conexion a internet
#[test]
fn test_clone_github_repo() {
    // Fallara en CI sin red
    clone_repository("https://github.com/org/repo.git", path).unwrap();
}

// CORRECTO: Usar repositorio local
#[test]
fn test_clone_local_repo() {
    let source_repo = TestRepo::new().unwrap();
    source_repo.commit_file("test.txt", "content", "Initial").unwrap();

    // Clone del repo local - no requiere red
    let dest = TempDir::new().unwrap();
    clone_repository(
        &format!("file://{}", source_repo.path().display()),
        dest.path(),
    ).unwrap();
}
```

## Pruebas

### Test de los Helpers

```rust
#[cfg(test)]
mod helper_tests {
    use super::*;

    #[test]
    fn test_repo_builder_creates_valid_repo() {
        let repo = TestRepoBuilder::new()
            .with_file("test.txt", "content")
            .build()
            .unwrap();

        // Verify it's a git repo
        assert!(repo.path().join(".git").exists());

        // Verify file exists
        assert!(repo.path().join("test.txt").exists());
    }

    #[test]
    fn test_repo_builder_creates_branches() {
        let repo = TestRepoBuilder::new()
            .with_file("main.txt", "main content")
            .with_branch("develop", {
                let mut files = HashMap::new();
                files.insert("dev.txt".to_string(), "dev content".to_string());
                files
            })
            .build()
            .unwrap();

        // Verify branch exists
        let output = Command::new("git")
            .args(["branch", "-l"])
            .current_dir(repo.path())
            .output()
            .unwrap();

        let branches = String::from_utf8_lossy(&output.stdout);
        assert!(branches.contains("develop"));
    }

    #[test]
    fn test_repo_builder_creates_tags() {
        let repo = TestRepoBuilder::new()
            .with_file("version.txt", "1.0.0")
            .with_tag("v1.0.0")
            .build()
            .unwrap();

        let output = Command::new("git")
            .args(["tag", "-l"])
            .current_dir(repo.path())
            .output()
            .unwrap();

        let tags = String::from_utf8_lossy(&output.stdout);
        assert!(tags.contains("v1.0.0"));
    }

    #[test]
    fn test_temp_dir_cleanup() {
        let path: PathBuf;

        {
            let repo = TestRepo::new().unwrap();
            path = repo.path().clone();
            assert!(path.exists());
        }  // repo dropped here

        // Directory should be cleaned up
        assert!(!path.exists());
    }
}
```

### Coverage Report

```rust
// En CI, ejecutar con coverage
// cargo tarpaulin --out Html --output-dir coverage/

// Ejemplo de configuracion en Cargo.toml
// [package.metadata.tarpaulin]
// exclude = ["tests/*", "benches/*"]
```

## Observabilidad

```rust
// En tests, habilitar logs para debugging
#[tokio::test]
async fn test_with_logs() {
    // Inicializar subscriber para tests
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_max_level(tracing::Level::DEBUG)
        .try_init();

    // Ahora los logs apareceran en output del test
    tracing::info!("Starting test");

    let repo = TestRepo::new().unwrap();
    // ...
}

// Helper para tests
pub fn init_test_logging() {
    let _ = tracing_subscriber::fmt()
        .with_test_writer()
        .with_env_filter("vortex_git=debug")
        .try_init();
}
```

## Documentacion de Testing

```rust
//! # Testing the Git Backend
//!
//! This module provides utilities for testing the Git backend without
//! external dependencies.
//!
//! ## Quick Start
//!
//! ```rust
//! use vortex_git::testing::{TestRepoBuilder, TestRepo};
//!
//! #[tokio::test]
//! async fn my_test() {
//!     // Create a test repository
//!     let repo = TestRepoBuilder::new()
//!         .with_file("application.yml", "port: 8080")
//!         .build()
//!         .unwrap();
//!
//!     // Use the repository
//!     let backend = GitBackend::new(config).await.unwrap();
//!
//!     // Repository is cleaned up automatically when test ends
//! }
//! ```
//!
//! ## Creating Branches and Tags
//!
//! ```rust
//! let repo = TestRepoBuilder::new()
//!     .with_file("main.txt", "main branch content")
//!     .with_tag("v1.0.0")
//!     .with_branch("develop", hashmap!{
//!         "dev.txt" => "develop branch content"
//!     })
//!     .build()
//!     .unwrap();
//! ```
//!
//! ## Best Practices
//!
//! 1. Always use `TestRepo` or `TempDir` - never create files in fixed paths
//! 2. Each test should be independent - don't rely on other tests
//! 3. Use `#[tokio::test]` for async tests
//! 4. Enable logging with `init_test_logging()` for debugging
```

## Entregable Final

- PR con:
  - `crates/vortex-git/tests/helpers/mod.rs`
  - `crates/vortex-git/tests/helpers/repo.rs`
  - `crates/vortex-git/tests/helpers/config.rs`
  - `crates/vortex-git/tests/clone_tests.rs`
  - `crates/vortex-git/tests/pull_tests.rs`
  - `crates/vortex-git/tests/checkout_tests.rs`
  - `crates/vortex-git/tests/reader_tests.rs`
  - `crates/vortex-git/tests/refresh_tests.rs`
  - `crates/vortex-git/tests/integration_tests.rs`
  - Cobertura > 80%
  - Documentacion de testing
  - CI configurado para ejecutar tests

---

**Anterior**: [Historia 005 - Refresh y Sincronizacion](./story-005-refresh-sync.md)
**Siguiente**: [Epica 05 - (siguiente epica)](../05-xxx/index.md)
