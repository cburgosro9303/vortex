# Historia 002: Clone y Pull de Repositorios

## Contexto y Objetivo

Esta historia implementa las operaciones fundamentales de Git necesarias para el backend de configuracion: clonar repositorios remotos y mantenerlos actualizados con pull. Utilizamos la crate `gix` (gitoxide), una implementacion pura de Git en Rust que ofrece mejor performance y seguridad que bindings a libgit2.

Para un desarrollador Java, esto es equivalente a usar JGit para clonar un repositorio y hacer fetch/merge, pero con las garantias de seguridad de memoria de Rust y manejo explicito de errores.

## Alcance

### In Scope
- Clonar repositorios Git via HTTPS
- Pull (fetch + checkout) para actualizar repositorio existente
- Configuracion de directorio local para el repositorio
- Manejo de credenciales basico (usuario/password para HTTPS)
- Timeout configurable para operaciones de red
- Logging de operaciones Git

### Out of Scope
- Autenticacion SSH (epica futura)
- Shallow clones (optimizacion futura)
- Submodulos Git
- Git LFS

## Criterios de Aceptacion

- [ ] Clone de repositorio publico HTTPS funciona correctamente
- [ ] Clone de repositorio privado con credenciales funciona
- [ ] Pull actualiza repositorio existente sin re-clonar
- [ ] Deteccion de repositorio ya clonado (skip clone si existe)
- [ ] Timeout de 60 segundos por defecto para operaciones de red
- [ ] Errores descriptivos para casos comunes (repo no existe, sin acceso, etc.)
- [ ] Operaciones de Git ejecutadas en spawn_blocking para no bloquear runtime async

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-git/src/repository/mod.rs` - Re-exports
- `vortex-git/src/repository/clone.rs` - Operaciones de clone
- `vortex-git/src/repository/pull.rs` - Operaciones de pull
- `vortex-git/src/config.rs` - Configuracion del backend Git

### Interfaces

```rust
/// Configuration for Git backend
#[derive(Debug, Clone)]
pub struct GitBackendConfig {
    pub uri: String,
    pub local_path: PathBuf,
    pub default_label: String,
    pub timeout_seconds: u64,
    pub credentials: Option<GitCredentials>,
}

#[derive(Debug, Clone)]
pub struct GitCredentials {
    pub username: String,
    pub password: String,
}

/// Result of a clone operation
#[derive(Debug)]
pub struct CloneResult {
    pub local_path: PathBuf,
    pub head_commit: String,
    pub was_cloned: bool,  // false if already existed
}

/// Result of a pull operation
#[derive(Debug)]
pub struct PullResult {
    pub previous_commit: String,
    pub current_commit: String,
    pub had_changes: bool,
}
```

### Estructura Sugerida

```
crates/vortex-git/src/repository/
├── mod.rs          # pub mod clone; pub mod pull;
├── clone.rs        # clone_repository function
└── pull.rs         # pull_repository function
```

## Pasos de Implementacion

1. **Agregar dependencias en Cargo.toml**
   - `gix` con features necesarios
   - `tokio` para spawn_blocking
   - `anyhow` para manejo de errores

2. **Implementar GitBackendConfig**
   - Struct con campos de configuracion
   - Validacion de URI y path
   - Valores por defecto sensatos

3. **Implementar clone_repository**
   - Verificar si el directorio ya existe
   - Usar `gix::prepare_clone` para iniciar clone
   - Configurar credenciales si estan presentes
   - Ejecutar en spawn_blocking

4. **Implementar pull_repository**
   - Abrir repositorio existente con `gix::open`
   - Fetch del remote
   - Fast-forward del branch actual
   - Retornar si hubo cambios

5. **Agregar tests con repositorio de prueba**

## Conceptos de Rust Aprendidos

### gix Crate para Operaciones Git

`gix` es una implementacion moderna de Git en Rust puro. A diferencia de `git2` (bindings a libgit2), no requiere dependencias de C.

```rust
use gix::clone::PrepareFetch;
use gix::progress::Discard;
use std::path::Path;
use anyhow::{Context, Result};

/// Clone a Git repository to local path
pub fn clone_repository(
    url: &str,
    local_path: &Path,
) -> Result<gix::Repository> {
    // PrepareFetch configura el clone pero no lo ejecuta aun
    let mut prepare = gix::prepare_clone(url, local_path)
        .context("Failed to prepare clone")?;

    // Configure fetch options
    prepare = prepare.with_shallow(gix::remote::fetch::Shallow::NoChange);

    // Execute the clone with progress tracking disabled
    let (mut checkout, outcome) = prepare
        .fetch_then_checkout(Discard, &gix::interrupt::IS_INTERRUPTED)
        .context("Failed to fetch repository")?;

    // Checkout the main worktree
    let (repo, _) = checkout
        .main_worktree(Discard, &gix::interrupt::IS_INTERRUPTED)
        .context("Failed to checkout worktree")?;

    tracing::info!(
        url = %url,
        path = %local_path.display(),
        "Repository cloned successfully"
    );

    Ok(repo)
}
```

**Comparacion con Java (JGit):**

```java
// Java con JGit
import org.eclipse.jgit.api.Git;
import org.eclipse.jgit.api.CloneCommand;

public Repository cloneRepository(String url, File localPath) throws Exception {
    CloneCommand clone = Git.cloneRepository()
        .setURI(url)
        .setDirectory(localPath)
        .setCloneAllBranches(true);

    try (Git git = clone.call()) {
        return git.getRepository();
    }
}
```

### Async File I/O con Tokio spawn_blocking

Las operaciones de `gix` son blocking. Para no bloquear el runtime async de Tokio, usamos `spawn_blocking`.

```rust
use tokio::task::spawn_blocking;
use std::path::PathBuf;
use anyhow::{Context, Result};

pub struct GitRepository {
    local_path: PathBuf,
    url: String,
}

impl GitRepository {
    /// Clone repository - runs blocking Git operations in separate thread
    pub async fn clone(url: String, local_path: PathBuf) -> Result<Self> {
        let path_clone = local_path.clone();
        let url_clone = url.clone();

        // spawn_blocking mueve la operacion a un thread pool separado
        // para no bloquear el runtime async de Tokio
        let repo = spawn_blocking(move || {
            clone_repository(&url_clone, &path_clone)
        })
        .await
        .context("Task join error")?  // Error si el task panic
        .context("Clone failed")?;    // Error del clone

        Ok(Self { local_path, url })
    }

    /// Pull latest changes
    pub async fn pull(&self) -> Result<PullResult> {
        let path = self.local_path.clone();

        spawn_blocking(move || {
            pull_repository(&path)
        })
        .await
        .context("Task join error")?
        .context("Pull failed")
    }
}

// Uso async
async fn setup_repository() -> Result<GitRepository> {
    let repo = GitRepository::clone(
        "https://github.com/org/config-repo.git".into(),
        PathBuf::from("/var/vortex/repos/config"),
    ).await?;

    // Esto no bloqueara otras tasks async
    repo.pull().await?;

    Ok(repo)
}
```

**Comparacion con Java:**

```java
// Java - ExecutorService para operaciones blocking
public class GitRepository {
    private final ExecutorService executor = Executors.newCachedThreadPool();

    public CompletableFuture<Repository> cloneAsync(String url, Path localPath) {
        return CompletableFuture.supplyAsync(() -> {
            try {
                return cloneRepository(url, localPath.toFile());
            } catch (Exception e) {
                throw new CompletionException(e);
            }
        }, executor);
    }
}
```

### Error Handling con anyhow y Context

`anyhow` permite agregar contexto a errores, facilitando el debugging. Es ideal para codigo de aplicacion (vs codigo de libreria donde `thiserror` es mejor).

```rust
use anyhow::{anyhow, bail, Context, Result};
use std::path::Path;

pub fn clone_or_open(url: &str, local_path: &Path) -> Result<gix::Repository> {
    // Validar inputs primero
    if url.is_empty() {
        // bail! es shorthand para return Err(anyhow!(...))
        bail!("Repository URL cannot be empty");
    }

    if local_path.exists() {
        // Intentar abrir repositorio existente
        let repo = gix::open(local_path)
            .with_context(|| format!(
                "Failed to open existing repository at {}",
                local_path.display()
            ))?;

        tracing::info!(
            path = %local_path.display(),
            "Opened existing repository"
        );

        return Ok(repo);
    }

    // Crear directorio padre si no existe
    if let Some(parent) = local_path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!(
                "Failed to create parent directory: {}",
                parent.display()
            ))?;
    }

    // Clone nuevo repositorio
    clone_repository(url, local_path)
        .with_context(|| format!(
            "Failed to clone {} to {}",
            url,
            local_path.display()
        ))
}

// El error resultante tiene toda la cadena de contexto:
// "Failed to clone https://... to /path: Failed to fetch: network error"
```

**Comparacion con Java (Exception Chaining):**

```java
// Java - exception chaining
public Repository cloneOrOpen(String url, Path localPath) throws Exception {
    if (url == null || url.isEmpty()) {
        throw new IllegalArgumentException("Repository URL cannot be empty");
    }

    if (Files.exists(localPath)) {
        try {
            return Git.open(localPath.toFile()).getRepository();
        } catch (IOException e) {
            throw new RepositoryException(
                "Failed to open existing repository at " + localPath, e);
        }
    }

    try {
        Files.createDirectories(localPath.getParent());
    } catch (IOException e) {
        throw new RepositoryException(
            "Failed to create parent directory: " + localPath.getParent(), e);
    }

    try {
        return cloneRepository(url, localPath.toFile());
    } catch (Exception e) {
        throw new RepositoryException(
            String.format("Failed to clone %s to %s", url, localPath), e);
    }
}
```

### Configuracion con Builder Pattern

```rust
use std::path::PathBuf;
use std::time::Duration;

#[derive(Debug, Clone)]
pub struct GitBackendConfig {
    pub uri: String,
    pub local_path: PathBuf,
    pub default_label: String,
    pub timeout: Duration,
    pub credentials: Option<GitCredentials>,
}

#[derive(Debug, Clone)]
pub struct GitCredentials {
    pub username: String,
    pub password: String,
}

impl GitBackendConfig {
    /// Create new config with required fields
    pub fn new(uri: impl Into<String>, local_path: impl Into<PathBuf>) -> Self {
        Self {
            uri: uri.into(),
            local_path: local_path.into(),
            default_label: "main".to_string(),
            timeout: Duration::from_secs(60),
            credentials: None,
        }
    }

    /// Set default branch/tag label
    pub fn with_default_label(mut self, label: impl Into<String>) -> Self {
        self.default_label = label.into();
        self
    }

    /// Set network timeout
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Set HTTPS credentials
    pub fn with_credentials(
        mut self,
        username: impl Into<String>,
        password: impl Into<String>,
    ) -> Self {
        self.credentials = Some(GitCredentials {
            username: username.into(),
            password: password.into(),
        });
        self
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        if self.uri.is_empty() {
            return Err("URI cannot be empty".into());
        }
        if !self.uri.starts_with("https://") && !self.uri.starts_with("git@") {
            return Err("URI must be HTTPS or SSH".into());
        }
        Ok(())
    }
}

// Uso
fn create_config() -> GitBackendConfig {
    GitBackendConfig::new(
        "https://github.com/org/config.git",
        "/var/vortex/repos/config"
    )
    .with_default_label("main")
    .with_timeout(Duration::from_secs(120))
    .with_credentials("user", "token")
}
```

## Riesgos y Errores Comunes

### 1. Bloquear el runtime async con operaciones Git

```rust
// ERROR: Bloquea el runtime de Tokio
async fn bad_clone(url: &str, path: &Path) -> Result<()> {
    let repo = gix::prepare_clone(url, path)?
        .fetch_then_checkout(...)?;  // BLOCKING!
    Ok(())
}

// CORRECTO: Usar spawn_blocking
async fn good_clone(url: String, path: PathBuf) -> Result<()> {
    spawn_blocking(move || {
        gix::prepare_clone(&url, &path)?
            .fetch_then_checkout(...)
    }).await??;
    Ok(())
}
```

### 2. No manejar el caso de repositorio existente

```rust
// ERROR: Falla si el directorio ya existe
pub fn clone(url: &str, path: &Path) -> Result<()> {
    gix::prepare_clone(url, path)?;  // Error si path existe
    Ok(())
}

// CORRECTO: Verificar y decidir
pub fn clone_or_open(url: &str, path: &Path) -> Result<gix::Repository> {
    if path.exists() {
        // Verificar que es un repo Git valido
        return gix::open(path)
            .context("Path exists but is not a valid Git repository");
    }
    clone_repository(url, path)
}
```

### 3. Credenciales en logs

```rust
// ERROR: Password en logs!
tracing::info!("Cloning with credentials: {:?}", config.credentials);

// CORRECTO: Ocultar credenciales sensibles
#[derive(Clone)]
pub struct GitCredentials {
    pub username: String,
    password: String,  // private
}

impl std::fmt::Debug for GitCredentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitCredentials")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}
```

### 4. No limpiar en caso de error parcial

```rust
// ERROR: Deja directorio parcialmente creado si clone falla
pub fn clone(url: &str, path: &Path) -> Result<()> {
    std::fs::create_dir_all(path)?;
    gix::prepare_clone(url, path)?
        .fetch_then_checkout(...)?;  // Si falla, path queda
    Ok(())
}

// CORRECTO: Cleanup en caso de error
pub fn clone(url: &str, path: &Path) -> Result<gix::Repository> {
    let created_path = !path.exists();
    if created_path {
        std::fs::create_dir_all(path)?;
    }

    match clone_repository_internal(url, path) {
        Ok(repo) => Ok(repo),
        Err(e) => {
            if created_path {
                // Best effort cleanup
                let _ = std::fs::remove_dir_all(path);
            }
            Err(e)
        }
    }
}
```

## Pruebas

### Unit Tests

```rust
#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_git_backend_config_builder() {
        let config = GitBackendConfig::new(
            "https://github.com/org/repo.git",
            "/tmp/repo"
        )
        .with_default_label("develop")
        .with_timeout(Duration::from_secs(30));

        assert_eq!(config.uri, "https://github.com/org/repo.git");
        assert_eq!(config.default_label, "develop");
        assert_eq!(config.timeout, Duration::from_secs(30));
        assert!(config.credentials.is_none());
    }

    #[test]
    fn test_config_with_credentials() {
        let config = GitBackendConfig::new("https://example.com/repo.git", "/tmp")
            .with_credentials("user", "pass123");

        let creds = config.credentials.unwrap();
        assert_eq!(creds.username, "user");
        // Password should be redacted in debug output
        let debug = format!("{:?}", creds);
        assert!(!debug.contains("pass123"));
        assert!(debug.contains("[REDACTED]"));
    }

    #[test]
    fn test_config_validation() {
        let valid = GitBackendConfig::new("https://github.com/org/repo.git", "/tmp");
        assert!(valid.validate().is_ok());

        let empty_uri = GitBackendConfig::new("", "/tmp");
        assert!(empty_uri.validate().is_err());

        let invalid_uri = GitBackendConfig::new("ftp://example.com/repo", "/tmp");
        assert!(invalid_uri.validate().is_err());
    }
}
```

### Integration Tests con Repositorio Real

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;

    /// Test con un repositorio publico pequeno
    /// NOTA: Requiere conexion a internet
    #[tokio::test]
    #[ignore]  // Ignorar por defecto, ejecutar con --ignored
    async fn test_clone_public_repository() {
        let temp_dir = TempDir::new().unwrap();
        let local_path = temp_dir.path().join("repo");

        // Usar un repo publico pequeno para tests
        let result = GitRepository::clone(
            "https://github.com/rust-lang/rust-by-example.git".into(),
            local_path.clone(),
        ).await;

        assert!(result.is_ok(), "Clone failed: {:?}", result.err());

        // Verificar que el directorio .git existe
        assert!(local_path.join(".git").exists());
    }

    #[tokio::test]
    #[ignore]
    async fn test_clone_then_pull() {
        let temp_dir = TempDir::new().unwrap();
        let local_path = temp_dir.path().join("repo");

        let repo = GitRepository::clone(
            "https://github.com/rust-lang/rust-by-example.git".into(),
            local_path,
        ).await.unwrap();

        // Pull no debe fallar si no hay cambios
        let pull_result = repo.pull().await;
        assert!(pull_result.is_ok());

        // Si no hubo cambios remotos, had_changes debe ser false
        // (asumiendo que el repo no cambio durante el test)
    }

    #[tokio::test]
    async fn test_clone_invalid_url() {
        let temp_dir = TempDir::new().unwrap();
        let local_path = temp_dir.path().join("repo");

        let result = GitRepository::clone(
            "https://github.com/nonexistent/repo-that-does-not-exist-12345.git".into(),
            local_path,
        ).await;

        assert!(result.is_err());
        let error = result.unwrap_err();
        // El error debe ser descriptivo
        assert!(error.to_string().to_lowercase().contains("fail")
            || error.to_string().to_lowercase().contains("error"));
    }
}
```

### Tests con Mock Repository

```rust
#[cfg(test)]
mod local_repo_tests {
    use super::*;
    use tempfile::TempDir;
    use std::process::Command;

    /// Crea un repositorio Git local para testing
    fn create_test_repo(dir: &Path) -> Result<()> {
        // git init
        Command::new("git")
            .args(["init"])
            .current_dir(dir)
            .output()
            .context("Failed to init repo")?;

        // Configurar user para commits
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(dir)
            .output()?;

        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(dir)
            .output()?;

        // Crear archivo y commit
        std::fs::write(dir.join("application.yml"), "server:\n  port: 8080")?;

        Command::new("git")
            .args(["add", "."])
            .current_dir(dir)
            .output()?;

        Command::new("git")
            .args(["commit", "-m", "Initial commit"])
            .current_dir(dir)
            .output()?;

        Ok(())
    }

    #[tokio::test]
    async fn test_open_local_repository() {
        let temp_dir = TempDir::new().unwrap();
        create_test_repo(temp_dir.path()).unwrap();

        // Abrir con gix
        let result = spawn_blocking({
            let path = temp_dir.path().to_owned();
            move || gix::open(&path)
        }).await.unwrap();

        assert!(result.is_ok());
    }
}
```

## Observabilidad

```rust
use tracing::{instrument, info, warn, error, Span};

impl GitRepository {
    #[instrument(
        skip(url, local_path),
        fields(
            url = %url,
            path = %local_path.display(),
            operation = "clone"
        )
    )]
    pub async fn clone(url: String, local_path: PathBuf) -> Result<Self> {
        info!("Starting repository clone");

        let start = std::time::Instant::now();
        let result = spawn_blocking({
            let url = url.clone();
            let path = local_path.clone();
            move || clone_repository(&url, &path)
        }).await?;

        match &result {
            Ok(_) => {
                info!(
                    duration_ms = %start.elapsed().as_millis(),
                    "Clone completed successfully"
                );
            }
            Err(e) => {
                error!(
                    duration_ms = %start.elapsed().as_millis(),
                    error = %e,
                    "Clone failed"
                );
            }
        }

        result.map(|_| Self { local_path, url })
    }

    #[instrument(skip(self), fields(path = %self.local_path.display()))]
    pub async fn pull(&self) -> Result<PullResult> {
        info!("Starting repository pull");

        let start = std::time::Instant::now();
        let result = spawn_blocking({
            let path = self.local_path.clone();
            move || pull_repository(&path)
        }).await?;

        match &result {
            Ok(pr) => {
                if pr.had_changes {
                    info!(
                        duration_ms = %start.elapsed().as_millis(),
                        previous_commit = %pr.previous_commit,
                        current_commit = %pr.current_commit,
                        "Pull completed with changes"
                    );
                } else {
                    info!(
                        duration_ms = %start.elapsed().as_millis(),
                        "Pull completed, no changes"
                    );
                }
            }
            Err(e) => {
                error!(
                    duration_ms = %start.elapsed().as_millis(),
                    error = %e,
                    "Pull failed"
                );
            }
        }

        result
    }
}
```

## Entregable Final

- PR con:
  - `crates/vortex-git/src/config.rs` - GitBackendConfig
  - `crates/vortex-git/src/repository/mod.rs`
  - `crates/vortex-git/src/repository/clone.rs`
  - `crates/vortex-git/src/repository/pull.rs`
  - Tests unitarios con mocks
  - Tests de integracion (marcados #[ignore])
  - Logging con tracing
  - Documentacion de API

---

**Anterior**: [Historia 001 - Trait ConfigSource](./story-001-config-source-trait.md)
**Siguiente**: [Historia 003 - Lectura de Archivos de Config](./story-003-file-reading.md)
