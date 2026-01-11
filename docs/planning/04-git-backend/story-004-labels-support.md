# Historia 004: Soporte de Labels (Branches/Tags)

## Contexto y Objetivo

Los labels en Spring Cloud Config permiten servir configuraciones de diferentes versiones del repositorio Git. Un label puede ser un branch (`main`, `develop`), un tag (`v1.0.0`), o incluso un commit SHA. Esta historia implementa la capacidad de hacer checkout de diferentes referencias Git para servir configuraciones versionadas.

Para un desarrollador Java, esto es equivalente a como JGit hace checkout de refs, pero con el tipado fuerte de Rust y manejo explicito de los posibles estados del repositorio.

## Alcance

### In Scope
- Checkout de branches (`main`, `develop`, `feature/xxx`)
- Checkout de tags (`v1.0.0`, `release-2024.01`)
- Validacion de que el label existe
- Manejo de label por defecto (`main`)
- Restauracion al label por defecto si el solicitado no existe
- Cache del label actual para evitar checkouts innecesarios

### Out of Scope
- Checkout de commits especificos por SHA (simplificacion)
- Worktrees multiples (un label a la vez)
- Checkout sparse (archivos especificos)

## Criterios de Aceptacion

- [ ] Checkout de branch existente funciona
- [ ] Checkout de tag existente funciona
- [ ] Error descriptivo si label no existe
- [ ] Skip de checkout si ya estamos en el label correcto
- [ ] Label por defecto configurable
- [ ] Nombres de branch con `/` soportados (`feature/my-feature`)
- [ ] Operaciones de checkout en spawn_blocking

## Diseno Propuesto

### Modulos/Crates Implicados
- `vortex-git/src/repository/checkout.rs` - Operaciones de checkout
- `vortex-git/src/repository/refs.rs` - Resolucion de referencias

### Interfaces

```rust
/// Represents a Git reference (branch or tag)
#[derive(Debug, Clone, PartialEq)]
pub enum GitRef {
    Branch(String),
    Tag(String),
}

impl GitRef {
    /// Parse a label string into a GitRef
    /// Tries branch first, then tag
    pub fn parse(label: &str, repo: &gix::Repository) -> Result<Self, GitError>;

    /// Get the full ref name (e.g., "refs/heads/main")
    pub fn full_name(&self) -> String;
}

/// Result of a checkout operation
#[derive(Debug)]
pub struct CheckoutResult {
    pub label: String,
    pub ref_type: GitRef,
    pub commit_id: String,
    pub was_changed: bool,
}

/// Checkout a specific label in the repository
pub async fn checkout_label(
    repo_path: &Path,
    label: &str,
) -> Result<CheckoutResult, GitError>;

/// List available labels (branches and tags)
pub async fn list_labels(
    repo_path: &Path,
) -> Result<Vec<GitRef>, GitError>;
```

### Estructura Sugerida

```
crates/vortex-git/src/repository/
├── mod.rs
├── clone.rs
├── pull.rs
├── checkout.rs     # Checkout operations
└── refs.rs         # Reference resolution
```

## Pasos de Implementacion

1. **Implementar GitRef enum**
   - Variantes para Branch y Tag
   - Metodo parse que resuelve el tipo
   - Metodos helper para nombres completos

2. **Implementar resolucion de referencias**
   - Buscar primero como branch local
   - Luego como branch remoto
   - Finalmente como tag

3. **Implementar checkout**
   - Obtener HEAD actual
   - Si ya estamos en el label, skip
   - Reset al commit del label

4. **Implementar listado de labels**
   - Enumerar branches locales y remotos
   - Enumerar tags

5. **Tests con repositorio local**

## Conceptos de Rust Aprendidos

### Git References con gix

Las referencias en Git son punteros a commits. `gix` proporciona una API type-safe para trabajar con ellas.

```rust
use gix::refs::Category;
use gix::Repository;
use std::path::Path;
use anyhow::{Context, Result};

/// Represents a Git reference
#[derive(Debug, Clone, PartialEq)]
pub enum GitRef {
    Branch(String),
    Tag(String),
}

impl GitRef {
    /// Try to resolve a label to a GitRef
    pub fn resolve(label: &str, repo: &Repository) -> Result<Self> {
        // Intentar como branch local primero
        let local_ref = format!("refs/heads/{}", label);
        if repo.find_reference(&local_ref).is_ok() {
            return Ok(GitRef::Branch(label.to_string()));
        }

        // Intentar como branch remoto (origin)
        let remote_ref = format!("refs/remotes/origin/{}", label);
        if repo.find_reference(&remote_ref).is_ok() {
            return Ok(GitRef::Branch(label.to_string()));
        }

        // Intentar como tag
        let tag_ref = format!("refs/tags/{}", label);
        if repo.find_reference(&tag_ref).is_ok() {
            return Ok(GitRef::Tag(label.to_string()));
        }

        anyhow::bail!("Label '{}' not found as branch or tag", label)
    }

    /// Get the full reference name
    pub fn full_ref_name(&self) -> String {
        match self {
            GitRef::Branch(name) => format!("refs/heads/{}", name),
            GitRef::Tag(name) => format!("refs/tags/{}", name),
        }
    }

    /// Get the short name (without refs/heads or refs/tags prefix)
    pub fn short_name(&self) -> &str {
        match self {
            GitRef::Branch(name) => name,
            GitRef::Tag(name) => name,
        }
    }
}
```

**Comparacion con Java (JGit):**

```java
// Java con JGit
public class GitRef {
    public enum RefType { BRANCH, TAG }

    private final String name;
    private final RefType type;

    public static GitRef resolve(String label, Repository repo) throws IOException {
        // Intentar como branch
        Ref ref = repo.findRef("refs/heads/" + label);
        if (ref != null) {
            return new GitRef(label, RefType.BRANCH);
        }

        // Intentar como tag
        ref = repo.findRef("refs/tags/" + label);
        if (ref != null) {
            return new GitRef(label, RefType.TAG);
        }

        throw new RefNotFoundException("Label not found: " + label);
    }
}
```

### Checkout con gix

El checkout en gix es mas explicito que en otras librerias Git, requiriendo varios pasos.

```rust
use gix::{Repository, ObjectId};
use gix::refs::transaction::PreviousValue;
use std::path::Path;
use anyhow::{Context, Result};
use tokio::task::spawn_blocking;

/// Result of a checkout operation
#[derive(Debug)]
pub struct CheckoutResult {
    pub label: String,
    pub ref_type: GitRef,
    pub commit_id: String,
    pub was_changed: bool,
}

/// Checkout a label (branch or tag) in the repository
pub async fn checkout_label(
    repo_path: &Path,
    label: &str,
) -> Result<CheckoutResult> {
    let path = repo_path.to_owned();
    let label = label.to_string();

    spawn_blocking(move || checkout_label_blocking(&path, &label))
        .await
        .context("Checkout task panicked")?
}

fn checkout_label_blocking(repo_path: &Path, label: &str) -> Result<CheckoutResult> {
    let repo = gix::open(repo_path)
        .context("Failed to open repository")?;

    // Resolver el label a una referencia
    let git_ref = GitRef::resolve(label, &repo)
        .with_context(|| format!("Failed to resolve label '{}'", label))?;

    // Obtener el commit ID del label
    let reference = repo.find_reference(&git_ref.full_ref_name())
        .context("Failed to find reference")?;

    let commit_id = reference
        .peel_to_commit()
        .context("Failed to peel to commit")?
        .id();

    // Obtener HEAD actual
    let head = repo.head()
        .context("Failed to get HEAD")?;

    let current_commit = head
        .peel_to_commit_in_place()
        .ok()
        .map(|c| c.id());

    // Si ya estamos en el commit correcto, no hacer nada
    if current_commit == Some(commit_id) {
        return Ok(CheckoutResult {
            label: label.to_string(),
            ref_type: git_ref,
            commit_id: commit_id.to_string(),
            was_changed: false,
        });
    }

    // Realizar el checkout
    // NOTA: gix no tiene checkout completo aun, usamos workaround
    // En produccion, considerar git2 o ejecutar git CLI
    perform_checkout(&repo, &commit_id)?;

    Ok(CheckoutResult {
        label: label.to_string(),
        ref_type: git_ref,
        commit_id: commit_id.to_string(),
        was_changed: true,
    })
}

/// Perform the actual checkout (simplified)
fn perform_checkout(repo: &Repository, commit_id: &ObjectId) -> Result<()> {
    // gix checkout aun esta en desarrollo
    // Por ahora, usamos un enfoque simplificado:
    // 1. Update HEAD to point to the commit
    // 2. Reset the index and working tree

    // Esto es una simplificacion - en produccion usar git CLI o git2
    tracing::warn!(
        commit = %commit_id,
        "Performing simplified checkout - consider using git2 for full support"
    );

    Ok(())
}
```

### Ref Validation y Sanitization

Es importante validar que los labels no contengan caracteres peligrosos.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum LabelError {
    #[error("Label cannot be empty")]
    Empty,

    #[error("Label contains invalid characters: {0}")]
    InvalidCharacters(String),

    #[error("Label cannot start with '-'")]
    StartsWithDash,

    #[error("Label cannot contain '..'")]
    ContainsDoubleDot,

    #[error("Label is too long (max 255 characters)")]
    TooLong,
}

/// Validate and sanitize a label string
pub fn validate_label(label: &str) -> Result<&str, LabelError> {
    // Check for empty
    if label.is_empty() {
        return Err(LabelError::Empty);
    }

    // Check length
    if label.len() > 255 {
        return Err(LabelError::TooLong);
    }

    // Check for dangerous patterns
    if label.starts_with('-') {
        return Err(LabelError::StartsWithDash);
    }

    if label.contains("..") {
        return Err(LabelError::ContainsDoubleDot);
    }

    // Check for invalid characters
    // Git refs can contain: a-z, A-Z, 0-9, -, _, /, .
    let invalid_chars: Vec<char> = label
        .chars()
        .filter(|c| !c.is_alphanumeric() && !['-', '_', '/', '.'].contains(c))
        .collect();

    if !invalid_chars.is_empty() {
        return Err(LabelError::InvalidCharacters(
            invalid_chars.into_iter().collect()
        ));
    }

    Ok(label)
}

// Uso
fn example() -> Result<(), LabelError> {
    validate_label("main")?;           // OK
    validate_label("feature/login")?;  // OK
    validate_label("v1.0.0")?;         // OK
    validate_label("")?;               // Error: Empty
    validate_label("../etc/passwd")?;  // Error: ContainsDoubleDot
    validate_label("-dangerous")?;     // Error: StartsWithDash

    Ok(())
}
```

**Comparacion con Java:**

```java
// Java - validation con Pattern
public class LabelValidator {
    private static final Pattern VALID_LABEL = Pattern.compile(
        "^[a-zA-Z0-9][a-zA-Z0-9/_.-]*$"
    );

    public static void validate(String label) throws LabelException {
        if (label == null || label.isEmpty()) {
            throw new LabelException("Label cannot be empty");
        }
        if (label.length() > 255) {
            throw new LabelException("Label too long");
        }
        if (label.contains("..")) {
            throw new LabelException("Label cannot contain '..'");
        }
        if (!VALID_LABEL.matcher(label).matches()) {
            throw new LabelException("Label contains invalid characters");
        }
    }
}
```

### Listado de Referencias

```rust
use gix::Repository;
use anyhow::Result;

/// List all available labels (branches and tags)
pub fn list_labels(repo: &Repository) -> Result<Vec<GitRef>> {
    let mut labels = Vec::new();

    // Listar branches locales
    for reference in repo.references()? {
        let reference = reference?;
        let name = reference.name().as_bstr().to_string();

        if name.starts_with("refs/heads/") {
            let branch_name = name.strip_prefix("refs/heads/")
                .unwrap()
                .to_string();
            labels.push(GitRef::Branch(branch_name));
        } else if name.starts_with("refs/tags/") {
            let tag_name = name.strip_prefix("refs/tags/")
                .unwrap()
                .to_string();
            labels.push(GitRef::Tag(tag_name));
        }
    }

    // Ordenar: branches primero, luego tags, alfabeticamente
    labels.sort_by(|a, b| {
        match (a, b) {
            (GitRef::Branch(a), GitRef::Branch(b)) => a.cmp(b),
            (GitRef::Tag(a), GitRef::Tag(b)) => a.cmp(b),
            (GitRef::Branch(_), GitRef::Tag(_)) => std::cmp::Ordering::Less,
            (GitRef::Tag(_), GitRef::Branch(_)) => std::cmp::Ordering::Greater,
        }
    });

    Ok(labels)
}

/// Check if a label exists
pub fn label_exists(repo: &Repository, label: &str) -> bool {
    GitRef::resolve(label, repo).is_ok()
}
```

## Riesgos y Errores Comunes

### 1. No validar labels antes de usar

```rust
// ERROR: Usa el label directamente
async fn get_config(label: &str) -> Result<Config> {
    checkout_label(repo_path, label).await?;  // Peligroso si label es malicioso
    // ...
}

// CORRECTO: Validar primero
async fn get_config(label: &str) -> Result<Config> {
    let validated_label = validate_label(label)
        .map_err(|e| ConfigSourceError::InvalidLabel {
            label: label.to_string(),
            reason: e.to_string(),
        })?;

    checkout_label(repo_path, validated_label).await?;
    // ...
}
```

### 2. Race condition en checkout concurrente

```rust
// ERROR: Multiples requests pueden hacer checkout simultaneamente
async fn handle_request(label: &str) -> Result<Config> {
    checkout_label(repo_path, label).await?;  // Race condition!
    read_config().await
}

// CORRECTO: Usar lock para serializar checkouts
struct GitBackend {
    checkout_lock: tokio::sync::Mutex<()>,
}

impl GitBackend {
    async fn handle_request(&self, label: &str) -> Result<Config> {
        let _guard = self.checkout_lock.lock().await;
        checkout_label(repo_path, label).await?;
        read_config().await
    }
}
```

### 3. No restaurar al label por defecto

```rust
// ERROR: Si checkout falla, estado indefinido
async fn get_config(label: &str) -> Result<Config> {
    checkout_label(repo_path, label).await?;
    read_config().await  // Si falla aqui, quedamos en label incorrecto
}

// MEJOR: Manejar estado consistente
struct GitBackend {
    current_label: RwLock<String>,
    default_label: String,
}

impl GitBackend {
    async fn get_config(&self, label: &str) -> Result<Config> {
        // Intentar checkout
        match checkout_label(repo_path, label).await {
            Ok(result) => {
                *self.current_label.write().await = label.to_string();
                read_config().await
            }
            Err(e) => {
                // Fallback al default si existe
                tracing::warn!(
                    label = %label,
                    error = %e,
                    "Label not found, using default"
                );
                checkout_label(repo_path, &self.default_label).await?;
                read_config().await
            }
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

    #[test]
    fn test_validate_label_valid() {
        assert!(validate_label("main").is_ok());
        assert!(validate_label("develop").is_ok());
        assert!(validate_label("feature/login").is_ok());
        assert!(validate_label("release-2024.01").is_ok());
        assert!(validate_label("v1.0.0").is_ok());
        assert!(validate_label("my_branch").is_ok());
    }

    #[test]
    fn test_validate_label_invalid() {
        assert!(matches!(
            validate_label(""),
            Err(LabelError::Empty)
        ));
        assert!(matches!(
            validate_label("-bad"),
            Err(LabelError::StartsWithDash)
        ));
        assert!(matches!(
            validate_label("../hack"),
            Err(LabelError::ContainsDoubleDot)
        ));
        assert!(matches!(
            validate_label("bad<chars>"),
            Err(LabelError::InvalidCharacters(_))
        ));
    }

    #[test]
    fn test_git_ref_full_name() {
        let branch = GitRef::Branch("main".to_string());
        assert_eq!(branch.full_ref_name(), "refs/heads/main");

        let tag = GitRef::Tag("v1.0.0".to_string());
        assert_eq!(tag.full_ref_name(), "refs/tags/v1.0.0");
    }

    #[test]
    fn test_git_ref_short_name() {
        let branch = GitRef::Branch("feature/login".to_string());
        assert_eq!(branch.short_name(), "feature/login");

        let tag = GitRef::Tag("v1.0.0".to_string());
        assert_eq!(tag.short_name(), "v1.0.0");
    }
}
```

### Integration Tests

```rust
#[cfg(test)]
mod integration_tests {
    use super::*;
    use tempfile::TempDir;
    use std::process::Command;

    /// Create a test repository with branches and tags
    fn setup_test_repo() -> TempDir {
        let temp_dir = TempDir::new().unwrap();
        let repo_path = temp_dir.path();

        // git init
        Command::new("git")
            .args(["init"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Configure git
        Command::new("git")
            .args(["config", "user.email", "test@test.com"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["config", "user.name", "Test"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Initial commit on main
        std::fs::write(
            repo_path.join("application.yml"),
            "version: 1.0"
        ).unwrap();
        Command::new("git")
            .args(["add", "."])
            .current_dir(repo_path)
            .output()
            .unwrap();
        Command::new("git")
            .args(["commit", "-m", "Initial"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create develop branch
        Command::new("git")
            .args(["branch", "develop"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Create a tag
        Command::new("git")
            .args(["tag", "v1.0.0"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Switch to develop and make changes
        Command::new("git")
            .args(["checkout", "develop"])
            .current_dir(repo_path)
            .output()
            .unwrap();
        std::fs::write(
            repo_path.join("application.yml"),
            "version: 2.0-SNAPSHOT"
        ).unwrap();
        Command::new("git")
            .args(["commit", "-am", "Develop version"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        // Back to main
        Command::new("git")
            .args(["checkout", "main"])
            .current_dir(repo_path)
            .output()
            .unwrap();

        temp_dir
    }

    #[tokio::test]
    async fn test_checkout_branch() {
        let temp_dir = setup_test_repo();

        let result = checkout_label(temp_dir.path(), "develop").await;
        assert!(result.is_ok());

        let checkout = result.unwrap();
        assert_eq!(checkout.label, "develop");
        assert!(matches!(checkout.ref_type, GitRef::Branch(_)));
    }

    #[tokio::test]
    async fn test_checkout_tag() {
        let temp_dir = setup_test_repo();

        let result = checkout_label(temp_dir.path(), "v1.0.0").await;
        assert!(result.is_ok());

        let checkout = result.unwrap();
        assert_eq!(checkout.label, "v1.0.0");
        assert!(matches!(checkout.ref_type, GitRef::Tag(_)));
    }

    #[tokio::test]
    async fn test_checkout_nonexistent_label() {
        let temp_dir = setup_test_repo();

        let result = checkout_label(temp_dir.path(), "nonexistent").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_list_labels() {
        let temp_dir = setup_test_repo();

        let repo = gix::open(temp_dir.path()).unwrap();
        let labels = list_labels(&repo).unwrap();

        // Should have main, develop branches and v1.0.0 tag
        let branch_names: Vec<_> = labels.iter()
            .filter_map(|l| match l {
                GitRef::Branch(name) => Some(name.as_str()),
                _ => None,
            })
            .collect();

        assert!(branch_names.contains(&"main"));
        assert!(branch_names.contains(&"develop"));

        let tag_names: Vec<_> = labels.iter()
            .filter_map(|l| match l {
                GitRef::Tag(name) => Some(name.as_str()),
                _ => None,
            })
            .collect();

        assert!(tag_names.contains(&"v1.0.0"));
    }
}
```

## Observabilidad

```rust
use tracing::{instrument, info, warn, debug};

#[instrument(skip(repo_path), fields(path = %repo_path.display()))]
pub async fn checkout_label(
    repo_path: &Path,
    label: &str,
) -> Result<CheckoutResult> {
    // Validar label
    let validated = validate_label(label).map_err(|e| {
        warn!(label = %label, error = %e, "Invalid label");
        e
    })?;

    info!(label = %validated, "Starting checkout");

    let result = checkout_label_internal(repo_path, validated).await;

    match &result {
        Ok(checkout) => {
            if checkout.was_changed {
                info!(
                    label = %checkout.label,
                    commit = %checkout.commit_id,
                    ref_type = ?checkout.ref_type,
                    "Checkout completed"
                );
            } else {
                debug!(
                    label = %checkout.label,
                    "Already on requested label, skipped checkout"
                );
            }
        }
        Err(e) => {
            warn!(label = %label, error = %e, "Checkout failed");
        }
    }

    result
}
```

## Entregable Final

- PR con:
  - `crates/vortex-git/src/repository/checkout.rs`
  - `crates/vortex-git/src/repository/refs.rs`
  - Actualizacion de `crates/vortex-git/src/repository/mod.rs`
  - Tests unitarios de validacion
  - Tests de integracion con repo local
  - Logging con tracing
  - Documentacion de labels soportados

---

**Anterior**: [Historia 003 - Lectura de Archivos](./story-003-file-reading.md)
**Siguiente**: [Historia 005 - Refresh y Sincronizacion](./story-005-refresh-sync.md)
