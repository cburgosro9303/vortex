//! Git repository operations using gix (pure Rust).

use std::path::Path;
use std::sync::Arc;

use gix::bstr::ByteSlice;
use gix::remote::fetch::Shallow;
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
/// Uses gix (pure Rust) for all Git operations - no system git required.
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
            },
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

        let result = tokio::task::spawn_blocking(move || Self::clone_blocking(&uri, &local_path))
            .await
            .map_err(|e| ConfigSourceError::git(format!("Clone task failed: {}", e)))?;

        match result {
            Ok(()) => {
                let mut state = state.write();
                *state = RepoState::Ready;
                info!("Repository cloned successfully");
                Ok(())
            },
            Err(e) => {
                let mut state = state.write();
                *state = RepoState::Error(e.to_string());
                Err(e)
            },
        }
    }

    /// Blocking clone operation using gix.
    fn clone_blocking(uri: &str, local_path: &Path) -> Result<(), ConfigSourceError> {
        // Create parent directories if needed
        if let Some(parent) = local_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        // Parse the URL
        let url = gix::url::parse(uri.into())
            .map_err(|e| ConfigSourceError::git(format!("Invalid URL: {}", e)))?;

        // Prepare the clone with shallow depth
        let mut prepare = gix::prepare_clone(url, local_path)
            .map_err(|e| ConfigSourceError::git(format!("Failed to prepare clone: {}", e)))?;

        // Configure shallow clone (depth 1)
        prepare = prepare.with_shallow(Shallow::DepthAtRemote(
            std::num::NonZeroU32::new(1).unwrap(),
        ));

        // Perform the fetch and checkout in one step
        let (mut checkout, _outcome) = prepare
            .fetch_then_checkout(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
            .map_err(|e| ConfigSourceError::git(format!("Clone failed: {}", e)))?;

        // Perform the worktree checkout
        let (_repo, _outcome) = checkout
            .main_worktree(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
            .map_err(|e| ConfigSourceError::git(format!("Checkout failed: {}", e)))?;

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

        let result = tokio::task::spawn_blocking(move || Self::fetch_blocking(&local_path))
            .await
            .map_err(|e| ConfigSourceError::git(format!("Fetch task failed: {}", e)))?;

        match result {
            Ok(()) => {
                let mut state = state.write();
                *state = RepoState::Ready;
                info!("Repository fetched successfully");
                Ok(())
            },
            Err(e) => {
                let mut state = state.write();
                *state = RepoState::Ready;
                warn!("Fetch failed: {}", e);
                Err(e)
            },
        }
    }

    /// Blocking fetch operation using gix.
    fn fetch_blocking(local_path: &Path) -> Result<(), ConfigSourceError> {
        let repo = gix::open(local_path)
            .map_err(|e| ConfigSourceError::git(format!("Failed to open repo: {}", e)))?;

        let remote = repo
            .find_default_remote(gix::remote::Direction::Fetch)
            .ok_or_else(|| ConfigSourceError::git("No default remote found"))?
            .map_err(|e| ConfigSourceError::git(format!("Failed to find remote: {}", e)))?;

        remote
            .connect(gix::remote::Direction::Fetch)
            .map_err(|e| ConfigSourceError::git(format!("Failed to connect: {}", e)))?
            .prepare_fetch(gix::progress::Discard, Default::default())
            .map_err(|e| ConfigSourceError::git(format!("Failed to prepare fetch: {}", e)))?
            .receive(gix::progress::Discard, &gix::interrupt::IS_INTERRUPTED)
            .map_err(|e| ConfigSourceError::git(format!("Fetch failed: {}", e)))?;

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

    /// Blocking checkout operation using gix.
    /// Note: For config reading, we only resolve the reference to get the commit ID.
    /// The actual worktree is already populated from clone, so we just track the reference.
    fn checkout_blocking(local_path: &Path, git_ref: &GitRef) -> Result<String, ConfigSourceError> {
        let repo = gix::open(local_path)
            .map_err(|e| ConfigSourceError::git(format!("Failed to open repo: {}", e)))?;

        // Resolve the reference to a commit ID
        let commit_id = match git_ref {
            GitRef::Branch(name) => {
                // Try local branch first, then remote
                let reference = repo
                    .find_reference(&format!("refs/heads/{}", name))
                    .or_else(|_| repo.find_reference(&format!("refs/remotes/origin/{}", name)))
                    .map_err(|_| ConfigSourceError::LabelNotFound(name.clone()))?;

                reference.into_fully_peeled_id().map_err(|e| {
                    ConfigSourceError::git(format!("Failed to peel reference: {}", e))
                })?
            },
            GitRef::Tag(name) => {
                let reference = repo
                    .find_reference(&format!("refs/tags/{}", name))
                    .map_err(|_| ConfigSourceError::LabelNotFound(name.clone()))?;

                reference.into_fully_peeled_id().map_err(|e| {
                    ConfigSourceError::git(format!("Failed to peel reference: {}", e))
                })?
            },
            GitRef::Commit(sha) => {
                let oid = gix::ObjectId::from_hex(sha.as_bytes())
                    .map_err(|_| ConfigSourceError::LabelNotFound(sha.clone()))?;

                // Verify the commit exists
                repo.find_object(oid)
                    .map_err(|_| ConfigSourceError::LabelNotFound(sha.clone()))?;

                return Ok(sha.clone());
            },
        };

        Ok(commit_id.to_string())
    }

    /// Gets the HEAD commit SHA.
    fn get_head_commit(local_path: &Path) -> Result<String, ConfigSourceError> {
        let repo = gix::open(local_path)
            .map_err(|e| ConfigSourceError::git(format!("Failed to open repo: {}", e)))?;

        let mut head = repo
            .head()
            .map_err(|e| ConfigSourceError::git(format!("Failed to get HEAD: {}", e)))?;

        let commit = head
            .peel_to_commit_in_place()
            .map_err(|e| ConfigSourceError::git(format!("Failed to peel HEAD: {}", e)))?;

        Ok(commit.id.to_string())
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

        tokio::task::spawn_blocking(move || -> Result<Vec<String>, ConfigSourceError> {
            let repo = gix::open(&local_path)
                .map_err(|e| ConfigSourceError::git(format!("Failed to open repo: {}", e)))?;

            let mut branches = Vec::new();

            if let Ok(refs) = repo.references() {
                // List local branches
                if let Ok(local) = refs.local_branches() {
                    for branch in local.flatten() {
                        if let Ok(name) = branch.name().as_bstr().to_str()
                            && let Some(short) = name.strip_prefix("refs/heads/")
                        {
                            branches.push(short.to_string());
                        }
                    }
                }

                // List remote branches
                if let Ok(remote) = refs.remote_branches() {
                    for branch in remote.flatten() {
                        if let Ok(name) = branch.name().as_bstr().to_str()
                            && let Some(short) = name.strip_prefix("refs/remotes/")
                        {
                            branches.push(short.to_string());
                        }
                    }
                }
            }

            Ok(branches)
        })
        .await
        .map_err(|e| ConfigSourceError::git(format!("Task failed: {}", e)))?
    }

    /// Lists available tags.
    pub async fn list_tags(&self) -> Result<Vec<String>, ConfigSourceError> {
        self.ensure_cloned().await?;

        let local_path = self.config.local_path().to_path_buf();

        tokio::task::spawn_blocking(move || -> Result<Vec<String>, ConfigSourceError> {
            let repo = gix::open(&local_path)
                .map_err(|e| ConfigSourceError::git(format!("Failed to open repo: {}", e)))?;

            let mut tags = Vec::new();

            if let Ok(refs) = repo.references()
                && let Ok(tag_refs) = refs.tags()
            {
                for tag in tag_refs.flatten() {
                    if let Ok(name) = tag.name().as_bstr().to_str()
                        && let Some(short) = name.strip_prefix("refs/tags/")
                    {
                        tags.push(short.to_string());
                    }
                }
            }

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
