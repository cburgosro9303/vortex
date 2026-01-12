//! Git repository operations.

use std::path::Path;
use std::process::Command;
use std::sync::Arc;

use parking_lot::RwLock;
use tracing::{debug, info, warn};

use super::{GitBackendConfig, GitRef};
use crate::error::ConfigSourceError;

/// State of the repository.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepoState {
    /// Repository has not been cloned yet.
    NotCloned,
    /// Repository is currently being cloned.
    Cloning,
    /// Repository is ready for use.
    Ready,
    /// Repository is being updated.
    Updating,
    /// Repository encountered an error.
    Error(String),
}

/// A Git repository wrapper for configuration management.
///
/// Uses the system's `git` command for operations to ensure maximum compatibility.
pub struct GitRepository {
    config: GitBackendConfig,
    state: Arc<RwLock<RepoState>>,
    current_ref: Arc<RwLock<Option<GitRef>>>,
}

impl GitRepository {
    /// Creates a new GitRepository instance.
    pub fn new(config: GitBackendConfig) -> Self {
        let state = if config.local_path().join(".git").exists() {
            RepoState::Ready
        } else {
            RepoState::NotCloned
        };

        Self {
            config,
            state: Arc::new(RwLock::new(state)),
            current_ref: Arc::new(RwLock::new(None)),
        }
    }

    /// Returns the current repository state.
    pub fn state(&self) -> RepoState {
        self.state.read().clone()
    }

    /// Returns the configuration.
    pub fn config(&self) -> &GitBackendConfig {
        &self.config
    }

    /// Returns the local repository path.
    pub fn local_path(&self) -> &Path {
        self.config.local_path()
    }

    /// Returns the current checked out reference.
    pub fn current_ref(&self) -> Option<GitRef> {
        self.current_ref.read().clone()
    }

    /// Ensures the repository is cloned and ready.
    pub async fn ensure_cloned(&self) -> Result<(), ConfigSourceError> {
        let state = self.state();

        match state {
            RepoState::Ready => {
                debug!("Repository already cloned at {:?}", self.local_path());
                Ok(())
            }
            RepoState::NotCloned => self.clone_repo().await,
            RepoState::Cloning | RepoState::Updating => Err(ConfigSourceError::Refreshing),
            RepoState::Error(msg) => Err(ConfigSourceError::unavailable(msg)),
        }
    }

    /// Clones the repository.
    async fn clone_repo(&self) -> Result<(), ConfigSourceError> {
        {
            let mut state = self.state.write();
            *state = RepoState::Cloning;
        }

        let uri = self.config.uri().to_string();
        let local_path = self.config.local_path().to_path_buf();
        let state = Arc::clone(&self.state);

        info!("Cloning repository from {} to {:?}", uri, local_path);

        let result = tokio::task::spawn_blocking(move || {
            Self::clone_blocking(&uri, &local_path)
        })
        .await
        .map_err(|e| ConfigSourceError::git(format!("Clone task failed: {}", e)))?;

        match result {
            Ok(()) => {
                let mut state = state.write();
                *state = RepoState::Ready;
                info!("Repository cloned successfully");
                Ok(())
            }
            Err(e) => {
                let mut state = state.write();
                *state = RepoState::Error(e.to_string());
                Err(e)
            }
        }
    }

    /// Blocking clone operation using git command.
    fn clone_blocking(uri: &str, local_path: &Path) -> Result<(), ConfigSourceError> {
        // Create parent directories if needed
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let output = Command::new("git")
            .args(["clone", "--depth", "1", uri])
            .arg(local_path)
            .output()
            .map_err(|e| ConfigSourceError::git(format!("Failed to execute git clone: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ConfigSourceError::git(format!("Clone failed: {}", stderr)));
        }

        Ok(())
    }

    /// Fetches the latest changes from the remote.
    pub async fn fetch(&self) -> Result<(), ConfigSourceError> {
        self.ensure_cloned().await?;

        {
            let mut state = self.state.write();
            if *state == RepoState::Updating {
                return Err(ConfigSourceError::Refreshing);
            }
            *state = RepoState::Updating;
        }

        let local_path = self.config.local_path().to_path_buf();
        let state = Arc::clone(&self.state);

        info!("Fetching updates for repository at {:?}", local_path);

        let result = tokio::task::spawn_blocking(move || {
            Self::fetch_blocking(&local_path)
        })
        .await
        .map_err(|e| ConfigSourceError::git(format!("Fetch task failed: {}", e)))?;

        match result {
            Ok(()) => {
                let mut state = state.write();
                *state = RepoState::Ready;
                info!("Repository fetched successfully");
                Ok(())
            }
            Err(e) => {
                let mut state = state.write();
                *state = RepoState::Ready;
                warn!("Fetch failed: {}", e);
                Err(e)
            }
        }
    }

    /// Blocking fetch operation.
    fn fetch_blocking(local_path: &Path) -> Result<(), ConfigSourceError> {
        let output = Command::new("git")
            .args(["fetch", "--all", "--prune"])
            .current_dir(local_path)
            .output()
            .map_err(|e| ConfigSourceError::git(format!("Failed to execute git fetch: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ConfigSourceError::git(format!("Fetch failed: {}", stderr)));
        }

        Ok(())
    }

    /// Checks out a specific reference (branch, tag, or commit).
    pub async fn checkout(&self, git_ref: &GitRef) -> Result<String, ConfigSourceError> {
        self.ensure_cloned().await?;

        git_ref
            .validate()
            .map_err(|e| ConfigSourceError::LabelNotFound(e.to_string()))?;

        let local_path = self.config.local_path().to_path_buf();
        let git_ref_clone = git_ref.clone();
        let current_ref = Arc::clone(&self.current_ref);

        debug!("Checking out {} in {:?}", git_ref, local_path);

        let commit_id = tokio::task::spawn_blocking(move || {
            Self::checkout_blocking(&local_path, &git_ref_clone)
        })
        .await
        .map_err(|e| ConfigSourceError::git(format!("Checkout task failed: {}", e)))??;

        {
            let mut current = current_ref.write();
            *current = Some(git_ref.clone());
        }

        Ok(commit_id)
    }

    /// Blocking checkout operation.
    fn checkout_blocking(local_path: &Path, git_ref: &GitRef) -> Result<String, ConfigSourceError> {
        match git_ref {
            GitRef::Branch(name) => {
                // Try to checkout the branch, falling back to origin/branch
                let output = Command::new("git")
                    .args(["checkout", name])
                    .current_dir(local_path)
                    .output()
                    .map_err(|e| ConfigSourceError::git(format!("Checkout failed: {}", e)))?;

                if !output.status.success() {
                    // Try origin/branch
                    let origin_ref = format!("origin/{}", name);
                    let output = Command::new("git")
                        .args(["checkout", "-B", name, &origin_ref])
                        .current_dir(local_path)
                        .output()
                        .map_err(|e| ConfigSourceError::git(format!("Checkout failed: {}", e)))?;

                    if !output.status.success() {
                        return Err(ConfigSourceError::LabelNotFound(name.clone()));
                    }
                }
            }
            GitRef::Tag(name) => {
                let tag_ref = format!("tags/{}", name);
                let output = Command::new("git")
                    .args(["checkout", &tag_ref])
                    .current_dir(local_path)
                    .output()
                    .map_err(|e| ConfigSourceError::git(format!("Checkout failed: {}", e)))?;

                if !output.status.success() {
                    return Err(ConfigSourceError::LabelNotFound(name.clone()));
                }
            }
            GitRef::Commit(sha) => {
                let output = Command::new("git")
                    .args(["checkout", sha])
                    .current_dir(local_path)
                    .output()
                    .map_err(|e| ConfigSourceError::git(format!("Checkout failed: {}", e)))?;

                if !output.status.success() {
                    return Err(ConfigSourceError::LabelNotFound(sha.clone()));
                }
            }
        }

        // Get the current commit SHA
        Self::get_head_commit(local_path)
    }

    /// Gets the HEAD commit SHA.
    fn get_head_commit(local_path: &Path) -> Result<String, ConfigSourceError> {
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(local_path)
            .output()
            .map_err(|e| ConfigSourceError::git(format!("Failed to get HEAD: {}", e)))?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(ConfigSourceError::git(format!("Failed to get HEAD: {}", stderr)));
        }

        let sha = String::from_utf8_lossy(&output.stdout).trim().to_string();
        Ok(sha)
    }

    /// Returns the current HEAD commit ID.
    pub async fn head_commit(&self) -> Result<String, ConfigSourceError> {
        self.ensure_cloned().await?;

        let local_path = self.config.local_path().to_path_buf();

        tokio::task::spawn_blocking(move || Self::get_head_commit(&local_path))
            .await
            .map_err(|e| ConfigSourceError::git(format!("Task failed: {}", e)))?
    }

    /// Checks if the repository exists locally.
    pub fn exists_locally(&self) -> bool {
        self.config.local_path().join(".git").exists()
    }

    /// Lists available branches.
    pub async fn list_branches(&self) -> Result<Vec<String>, ConfigSourceError> {
        self.ensure_cloned().await?;

        let local_path = self.config.local_path().to_path_buf();

        tokio::task::spawn_blocking(move || {
            let output = Command::new("git")
                .args(["branch", "-a", "--format=%(refname:short)"])
                .current_dir(&local_path)
                .output()
                .map_err(|e| ConfigSourceError::git(format!("Failed to list branches: {}", e)))?;

            if !output.status.success() {
                return Err(ConfigSourceError::git("Failed to list branches"));
            }

            let branches: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            Ok(branches)
        })
        .await
        .map_err(|e| ConfigSourceError::git(format!("Task failed: {}", e)))?
    }

    /// Lists available tags.
    pub async fn list_tags(&self) -> Result<Vec<String>, ConfigSourceError> {
        self.ensure_cloned().await?;

        let local_path = self.config.local_path().to_path_buf();

        tokio::task::spawn_blocking(move || {
            let output = Command::new("git")
                .args(["tag", "-l"])
                .current_dir(&local_path)
                .output()
                .map_err(|e| ConfigSourceError::git(format!("Failed to list tags: {}", e)))?;

            if !output.status.success() {
                return Err(ConfigSourceError::git("Failed to list tags"));
            }

            let tags: Vec<String> = String::from_utf8_lossy(&output.stdout)
                .lines()
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();

            Ok(tags)
        })
        .await
        .map_err(|e| ConfigSourceError::git(format!("Task failed: {}", e)))?
    }
}

impl std::fmt::Debug for GitRepository {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GitRepository")
            .field("uri", &self.config.uri())
            .field("local_path", &self.config.local_path())
            .field("state", &self.state())
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_repository() {
        let config = GitBackendConfig::builder()
            .uri("https://github.com/test/repo.git")
            .local_path("/tmp/nonexistent-vortex-test")
            .build()
            .unwrap();

        let repo = GitRepository::new(config);
        assert_eq!(repo.state(), RepoState::NotCloned);
    }

    #[test]
    fn test_exists_locally() {
        let config = GitBackendConfig::builder()
            .uri("https://github.com/test/repo.git")
            .local_path("/tmp/nonexistent-vortex-test")
            .build()
            .unwrap();

        let repo = GitRepository::new(config);
        assert!(!repo.exists_locally());
    }
}
