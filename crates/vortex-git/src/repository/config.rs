//! Git backend configuration.

use std::path::PathBuf;
use std::time::Duration;

use serde::{Deserialize, Serialize};

/// Configuration for the Git backend.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitBackendConfig {
    /// The Git repository URI (HTTPS or SSH).
    uri: String,

    /// Local path where the repository will be cloned.
    local_path: PathBuf,

    /// Default branch/tag to use when not specified.
    #[serde(default = "default_label")]
    default_label: String,

    /// Search paths within the repository (relative to root).
    #[serde(default)]
    search_paths: Vec<String>,

    /// Clone timeout duration.
    #[serde(default = "default_clone_timeout", with = "humantime_serde")]
    clone_timeout: Duration,

    /// Fetch timeout duration.
    #[serde(default = "default_fetch_timeout", with = "humantime_serde")]
    fetch_timeout: Duration,

    /// Whether to force clone if repository exists.
    #[serde(default)]
    force_pull: bool,

    /// Whether to delete untracked files on checkout.
    #[serde(default = "default_true")]
    clean_on_checkout: bool,

    /// Username for authentication (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    username: Option<String>,

    /// Password or token for authentication (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    password: Option<String>,

    /// SSH private key path (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    private_key: Option<PathBuf>,

    /// SSH private key passphrase (optional).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    passphrase: Option<String>,

    /// Whether to skip SSL verification (not recommended).
    #[serde(default)]
    skip_ssl_verification: bool,
}

fn default_label() -> String {
    "main".to_string()
}

fn default_clone_timeout() -> Duration {
    Duration::from_secs(120)
}

fn default_fetch_timeout() -> Duration {
    Duration::from_secs(30)
}

fn default_true() -> bool {
    true
}

impl GitBackendConfig {
    /// Creates a new builder for GitBackendConfig.
    pub fn builder() -> GitBackendConfigBuilder {
        GitBackendConfigBuilder::default()
    }

    /// Returns the repository URI.
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Returns the local path for the cloned repository.
    pub fn local_path(&self) -> &PathBuf {
        &self.local_path
    }

    /// Returns the default label (branch/tag).
    pub fn default_label(&self) -> &str {
        &self.default_label
    }

    /// Returns the search paths within the repository.
    pub fn search_paths(&self) -> &[String] {
        &self.search_paths
    }

    /// Returns the clone timeout.
    pub fn clone_timeout(&self) -> Duration {
        self.clone_timeout
    }

    /// Returns the fetch timeout.
    pub fn fetch_timeout(&self) -> Duration {
        self.fetch_timeout
    }

    /// Returns whether to force pull on existing repository.
    pub fn force_pull(&self) -> bool {
        self.force_pull
    }

    /// Returns whether to clean untracked files on checkout.
    pub fn clean_on_checkout(&self) -> bool {
        self.clean_on_checkout
    }

    /// Returns the username for authentication.
    pub fn username(&self) -> Option<&str> {
        self.username.as_deref()
    }

    /// Returns the password/token for authentication.
    pub fn password(&self) -> Option<&str> {
        self.password.as_deref()
    }

    /// Returns the SSH private key path.
    pub fn private_key(&self) -> Option<&PathBuf> {
        self.private_key.as_ref()
    }

    /// Returns the SSH private key passphrase.
    pub fn passphrase(&self) -> Option<&str> {
        self.passphrase.as_deref()
    }

    /// Returns whether to skip SSL verification.
    pub fn skip_ssl_verification(&self) -> bool {
        self.skip_ssl_verification
    }

    /// Returns effective search paths (defaults to root if empty).
    pub fn effective_search_paths(&self) -> Vec<&str> {
        if self.search_paths.is_empty() {
            vec![""]
        } else {
            self.search_paths.iter().map(|s| s.as_str()).collect()
        }
    }
}

/// Builder for GitBackendConfig.
#[derive(Debug, Default)]
pub struct GitBackendConfigBuilder {
    uri: Option<String>,
    local_path: Option<PathBuf>,
    default_label: Option<String>,
    search_paths: Vec<String>,
    clone_timeout: Option<Duration>,
    fetch_timeout: Option<Duration>,
    force_pull: bool,
    clean_on_checkout: bool,
    username: Option<String>,
    password: Option<String>,
    private_key: Option<PathBuf>,
    passphrase: Option<String>,
    skip_ssl_verification: bool,
}

impl GitBackendConfigBuilder {
    /// Sets the Git repository URI.
    pub fn uri(mut self, uri: impl Into<String>) -> Self {
        self.uri = Some(uri.into());
        self
    }

    /// Sets the local path for cloning.
    pub fn local_path(mut self, path: impl Into<PathBuf>) -> Self {
        self.local_path = Some(path.into());
        self
    }

    /// Sets the default label (branch/tag).
    pub fn default_label(mut self, label: impl Into<String>) -> Self {
        self.default_label = Some(label.into());
        self
    }

    /// Adds a search path.
    pub fn search_path(mut self, path: impl Into<String>) -> Self {
        self.search_paths.push(path.into());
        self
    }

    /// Sets the search paths.
    pub fn search_paths(mut self, paths: Vec<impl Into<String>>) -> Self {
        self.search_paths = paths.into_iter().map(Into::into).collect();
        self
    }

    /// Sets the clone timeout.
    pub fn clone_timeout(mut self, timeout: Duration) -> Self {
        self.clone_timeout = Some(timeout);
        self
    }

    /// Sets the fetch timeout.
    pub fn fetch_timeout(mut self, timeout: Duration) -> Self {
        self.fetch_timeout = Some(timeout);
        self
    }

    /// Sets whether to force pull.
    pub fn force_pull(mut self, force: bool) -> Self {
        self.force_pull = force;
        self
    }

    /// Sets whether to clean on checkout.
    pub fn clean_on_checkout(mut self, clean: bool) -> Self {
        self.clean_on_checkout = clean;
        self
    }

    /// Sets basic authentication credentials.
    pub fn basic_auth(mut self, username: impl Into<String>, password: impl Into<String>) -> Self {
        self.username = Some(username.into());
        self.password = Some(password.into());
        self
    }

    /// Sets SSH authentication.
    pub fn ssh_auth(mut self, private_key: impl Into<PathBuf>) -> Self {
        self.private_key = Some(private_key.into());
        self
    }

    /// Sets the SSH key passphrase.
    pub fn passphrase(mut self, passphrase: impl Into<String>) -> Self {
        self.passphrase = Some(passphrase.into());
        self
    }

    /// Sets whether to skip SSL verification.
    pub fn skip_ssl_verification(mut self, skip: bool) -> Self {
        self.skip_ssl_verification = skip;
        self
    }

    /// Builds the configuration.
    ///
    /// # Errors
    ///
    /// Returns an error if required fields are missing.
    pub fn build(self) -> Result<GitBackendConfig, &'static str> {
        let uri = self.uri.ok_or("uri is required")?;
        let local_path = self.local_path.ok_or("local_path is required")?;

        Ok(GitBackendConfig {
            uri,
            local_path,
            default_label: self.default_label.unwrap_or_else(default_label),
            search_paths: self.search_paths,
            clone_timeout: self.clone_timeout.unwrap_or_else(default_clone_timeout),
            fetch_timeout: self.fetch_timeout.unwrap_or_else(default_fetch_timeout),
            force_pull: self.force_pull,
            clean_on_checkout: self.clean_on_checkout,
            username: self.username,
            password: self.password,
            private_key: self.private_key,
            passphrase: self.passphrase,
            skip_ssl_verification: self.skip_ssl_verification,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_builder_minimal() {
        let config = GitBackendConfig::builder()
            .uri("https://github.com/org/repo.git")
            .local_path("/tmp/repo")
            .build()
            .unwrap();

        assert_eq!(config.uri(), "https://github.com/org/repo.git");
        assert_eq!(config.local_path(), &PathBuf::from("/tmp/repo"));
        assert_eq!(config.default_label(), "main");
    }

    #[test]
    fn test_builder_full() {
        let config = GitBackendConfig::builder()
            .uri("https://github.com/org/repo.git")
            .local_path("/tmp/repo")
            .default_label("develop")
            .search_paths(vec!["config", "shared"])
            .clone_timeout(Duration::from_secs(60))
            .fetch_timeout(Duration::from_secs(15))
            .force_pull(true)
            .basic_auth("user", "token")
            .build()
            .unwrap();

        assert_eq!(config.default_label(), "develop");
        assert_eq!(config.search_paths(), &["config", "shared"]);
        assert_eq!(config.clone_timeout(), Duration::from_secs(60));
        assert!(config.force_pull());
        assert_eq!(config.username(), Some("user"));
        assert_eq!(config.password(), Some("token"));
    }

    #[test]
    fn test_builder_missing_uri() {
        let result = GitBackendConfig::builder().local_path("/tmp/repo").build();

        assert!(result.is_err());
    }

    #[test]
    fn test_effective_search_paths() {
        let config = GitBackendConfig::builder()
            .uri("https://github.com/org/repo.git")
            .local_path("/tmp/repo")
            .build()
            .unwrap();

        assert_eq!(config.effective_search_paths(), vec![""]);

        let config = GitBackendConfig::builder()
            .uri("https://github.com/org/repo.git")
            .local_path("/tmp/repo")
            .search_paths(vec!["config"])
            .build()
            .unwrap();

        assert_eq!(config.effective_search_paths(), vec!["config"]);
    }
}

mod humantime_serde {
    use serde::{self, Deserialize, Deserializer, Serializer};
    use std::time::Duration;

    pub fn serialize<S>(duration: &Duration, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_u64(duration.as_secs())
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<Duration, D::Error>
    where
        D: Deserializer<'de>,
    {
        let secs = u64::deserialize(deserializer)?;
        Ok(Duration::from_secs(secs))
    }
}
