//! Configuration source trait definition.

use async_trait::async_trait;

use super::{ConfigQuery, ConfigResult};
use crate::error::ConfigSourceError;

/// A source of configuration data.
///
/// This trait abstracts over different configuration backends (Git, S3, database, etc.)
/// allowing the server to fetch configuration without knowing the underlying storage.
///
/// # Implementors
///
/// - `GitBackend` - Fetches configuration from a Git repository
/// - (Future) `S3Backend` - Fetches configuration from S3
/// - (Future) `DatabaseBackend` - Fetches configuration from a database
///
/// # Example
///
/// ```ignore
/// use vortex_git::{ConfigSource, ConfigQuery, ConfigResult};
///
/// struct MySource;
///
/// #[async_trait]
/// impl ConfigSource for MySource {
///     async fn fetch(&self, query: &ConfigQuery) -> Result<ConfigResult, ConfigSourceError> {
///         // Implementation here
///     }
///
///     async fn health_check(&self) -> Result<(), ConfigSourceError> {
///         Ok(())
///     }
///
///     fn name(&self) -> &str {
///         "my-source"
///     }
/// }
/// ```
#[async_trait]
pub trait ConfigSource: Send + Sync {
    /// Fetches configuration for the given query.
    ///
    /// # Arguments
    ///
    /// * `query` - The configuration query containing application, profiles, and label
    ///
    /// # Returns
    ///
    /// A `ConfigResult` containing the resolved configuration, or an error.
    ///
    /// # Errors
    ///
    /// - `ConfigSourceError::ApplicationNotFound` if the application doesn't exist
    /// - `ConfigSourceError::LabelNotFound` if the branch/tag doesn't exist
    /// - `ConfigSourceError::SourceUnavailable` if the source is not accessible
    async fn fetch(&self, query: &ConfigQuery) -> Result<ConfigResult, ConfigSourceError>;

    /// Performs a health check on the configuration source.
    ///
    /// This should verify that the source is accessible and properly configured.
    ///
    /// # Returns
    ///
    /// `Ok(())` if the source is healthy, or an error describing the problem.
    async fn health_check(&self) -> Result<(), ConfigSourceError>;

    /// Returns the name of this configuration source.
    ///
    /// This is used for logging and identification purposes.
    fn name(&self) -> &str;

    /// Triggers a refresh of the configuration source.
    ///
    /// For Git sources, this typically means pulling the latest changes.
    /// The default implementation is a no-op for sources that don't support refresh.
    ///
    /// # Returns
    ///
    /// `Ok(())` on success, or an error if refresh fails.
    async fn refresh(&self) -> Result<(), ConfigSourceError> {
        Ok(())
    }

    /// Returns whether this source supports refresh operations.
    ///
    /// Sources that don't support refresh (e.g., static file sources)
    /// should return `false`.
    fn supports_refresh(&self) -> bool {
        false
    }

    /// Returns the default label for this source.
    ///
    /// For Git sources, this is typically "main" or "master".
    fn default_label(&self) -> &str {
        "main"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct MockSource {
        name: String,
    }

    #[async_trait]
    impl ConfigSource for MockSource {
        async fn fetch(&self, query: &ConfigQuery) -> Result<ConfigResult, ConfigSourceError> {
            let result = ConfigResult::new(
                query.application(),
                query.profiles().to_vec(),
                query.effective_label("main"),
            );
            Ok(result)
        }

        async fn health_check(&self) -> Result<(), ConfigSourceError> {
            Ok(())
        }

        fn name(&self) -> &str {
            &self.name
        }
    }

    #[tokio::test]
    async fn test_mock_source() {
        let source = MockSource {
            name: "mock".to_string(),
        };

        let query = ConfigQuery::new("myapp", vec!["dev"]);
        let result = source.fetch(&query).await.unwrap();

        assert_eq!(result.name(), "myapp");
        assert_eq!(result.profiles(), &["dev"]);
        assert_eq!(result.label(), "main");
    }

    #[tokio::test]
    async fn test_health_check() {
        let source = MockSource {
            name: "mock".to_string(),
        };

        assert!(source.health_check().await.is_ok());
    }

    #[tokio::test]
    async fn test_default_refresh() {
        let source = MockSource {
            name: "mock".to_string(),
        };

        assert!(!source.supports_refresh());
        assert!(source.refresh().await.is_ok());
    }
}
