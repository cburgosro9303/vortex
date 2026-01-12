//! Configuration file resolution following Spring Cloud Config conventions.

use std::path::{Path, PathBuf};

use tracing::debug;
use vortex_core::{ConfigMap, PropertySource};

use super::{ConfigFormat, ConfigParser};
use crate::error::ConfigSourceError;
use crate::source::ConfigQuery;

/// Resolves and reads configuration files from a repository.
///
/// Follows Spring Cloud Config file naming conventions:
/// - `application.yml` - Base configuration for all apps
/// - `application-{profile}.yml` - Profile-specific base config
/// - `{application}.yml` - Application-specific config
/// - `{application}-{profile}.yml` - Application + profile config
pub struct ConfigFileResolver {
    /// Base path of the repository.
    base_path: PathBuf,
    /// Search paths within the repository.
    search_paths: Vec<String>,
}

impl ConfigFileResolver {
    /// Creates a new file resolver.
    pub fn new(base_path: impl Into<PathBuf>, search_paths: Vec<String>) -> Self {
        Self {
            base_path: base_path.into(),
            search_paths,
        }
    }

    /// Resolves configuration for the given query.
    ///
    /// Returns property sources in order of precedence (highest first):
    /// 1. {app}-{profile}.yml (last profile has highest priority)
    /// 2. {app}.yml
    /// 3. application-{profile}.yml
    /// 4. application.yml
    pub fn resolve(&self, query: &ConfigQuery, label: &str) -> Result<Vec<PropertySource>, ConfigSourceError> {
        let mut sources = Vec::new();

        let effective_search_paths = if self.search_paths.is_empty() {
            vec!["".to_string()]
        } else {
            self.search_paths.clone()
        };

        for search_path in &effective_search_paths {
            let base = if search_path.is_empty() {
                self.base_path.clone()
            } else {
                self.base_path.join(search_path)
            };

            // 1. application.yml (lowest priority)
            if let Some(source) = self.try_read_config(&base, "application", None, label)? {
                sources.push(source);
            }

            // 2. application-{profile}.yml
            for profile in query.profiles() {
                if let Some(source) = self.try_read_config(&base, "application", Some(profile), label)? {
                    sources.push(source);
                }
            }

            // 3. {app}.yml
            if let Some(source) = self.try_read_config(&base, query.application(), None, label)? {
                sources.push(source);
            }

            // 4. {app}-{profile}.yml (highest priority)
            for profile in query.profiles() {
                if let Some(source) = self.try_read_config(&base, query.application(), Some(profile), label)? {
                    sources.push(source);
                }
            }
        }

        // Reverse so highest priority is first (Spring Cloud Config convention)
        sources.reverse();

        debug!(
            "Resolved {} property sources for {}",
            sources.len(),
            query
        );

        Ok(sources)
    }

    /// Tries to read a configuration file, returning None if not found.
    fn try_read_config(
        &self,
        base: &Path,
        name: &str,
        profile: Option<&str>,
        label: &str,
    ) -> Result<Option<PropertySource>, ConfigSourceError> {
        let filename = match profile {
            Some(p) => format!("{}-{}", name, p),
            None => name.to_string(),
        };

        // Try each supported format
        for format in ConfigFormat::all() {
            for ext in format.extensions() {
                let file_path = base.join(format!("{}.{}", filename, ext));

                if file_path.exists() {
                    debug!("Reading config file: {:?}", file_path);

                    let config = ConfigParser::parse_file(&file_path)?;
                    let source_name = self.make_source_name(&file_path, label);

                    return Ok(Some(PropertySource::new(source_name, config)));
                }
            }
        }

        Ok(None)
    }

    /// Creates a property source name following Spring Cloud Config conventions.
    fn make_source_name(&self, path: &Path, label: &str) -> String {
        let relative = path
            .strip_prefix(&self.base_path)
            .unwrap_or(path)
            .to_string_lossy();

        format!("git:{}:{}", label, relative)
    }

    /// Lists all configuration files in the repository.
    pub fn list_config_files(&self) -> Result<Vec<PathBuf>, ConfigSourceError> {
        let mut files = Vec::new();

        let effective_search_paths = if self.search_paths.is_empty() {
            vec!["".to_string()]
        } else {
            self.search_paths.clone()
        };

        for search_path in &effective_search_paths {
            let base = if search_path.is_empty() {
                self.base_path.clone()
            } else {
                self.base_path.join(search_path)
            };

            if !base.exists() {
                continue;
            }

            self.find_config_files(&base, &mut files)?;
        }

        Ok(files)
    }

    /// Recursively finds configuration files.
    fn find_config_files(&self, dir: &Path, files: &mut Vec<PathBuf>) -> Result<(), ConfigSourceError> {
        if !dir.is_dir() {
            return Ok(());
        }

        for entry in std::fs::read_dir(dir)? {
            let entry = entry?;
            let path = entry.path();

            if path.is_file() {
                if ConfigFormat::from_path(&path).is_some() {
                    files.push(path);
                }
            } else if path.is_dir() {
                // Skip hidden directories and common non-config directories
                let name = path.file_name().and_then(|n| n.to_str()).unwrap_or("");
                if !name.starts_with('.') && name != "node_modules" && name != "target" {
                    self.find_config_files(&path, files)?;
                }
            }
        }

        Ok(())
    }

    /// Reads a specific configuration file.
    pub fn read_file(&self, path: &Path) -> Result<ConfigMap, ConfigSourceError> {
        let full_path = self.base_path.join(path);
        ConfigParser::parse_file(&full_path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn create_test_repo() -> TempDir {
        let dir = TempDir::new().unwrap();

        // Create application.yml
        fs::write(
            dir.path().join("application.yml"),
            "server:\n  port: 8080\n",
        )
        .unwrap();

        // Create application-dev.yml
        fs::write(
            dir.path().join("application-dev.yml"),
            "server:\n  port: 8081\nlogging:\n  level: DEBUG\n",
        )
        .unwrap();

        // Create myapp.yml
        fs::write(
            dir.path().join("myapp.yml"),
            "app:\n  name: myapp\n",
        )
        .unwrap();

        // Create myapp-dev.yml
        fs::write(
            dir.path().join("myapp-dev.yml"),
            "app:\n  debug: true\n",
        )
        .unwrap();

        dir
    }

    #[test]
    fn test_resolve_basic() {
        let dir = create_test_repo();
        let resolver = ConfigFileResolver::new(dir.path(), vec![]);

        let query = ConfigQuery::new("myapp", vec!["dev"]);
        let sources = resolver.resolve(&query, "main").unwrap();

        // Should have 4 sources in order of precedence
        assert_eq!(sources.len(), 4);

        // First should be myapp-dev.yml (highest priority)
        assert!(sources[0].name.contains("myapp-dev"));

        // Last should be application.yml (lowest priority)
        assert!(sources[3].name.contains("application.yml"));
    }

    #[test]
    fn test_resolve_no_profile() {
        let dir = create_test_repo();
        let resolver = ConfigFileResolver::new(dir.path(), vec![]);

        let query = ConfigQuery::new("myapp", vec![] as Vec<String>);
        let sources = resolver.resolve(&query, "main").unwrap();

        // Should have 2 sources: myapp.yml and application.yml
        assert_eq!(sources.len(), 2);
    }

    #[test]
    fn test_resolve_unknown_app() {
        let dir = create_test_repo();
        let resolver = ConfigFileResolver::new(dir.path(), vec![]);

        let query = ConfigQuery::new("unknown", vec!["dev"]);
        let sources = resolver.resolve(&query, "main").unwrap();

        // Should only have application files
        assert_eq!(sources.len(), 2);
        assert!(sources.iter().all(|s| !s.name.contains("unknown")));
    }

    #[test]
    fn test_list_config_files() {
        let dir = create_test_repo();
        let resolver = ConfigFileResolver::new(dir.path(), vec![]);

        let files = resolver.list_config_files().unwrap();
        assert_eq!(files.len(), 4);
    }

    #[test]
    fn test_source_name_format() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("test.yml"), "key: value").unwrap();

        let resolver = ConfigFileResolver::new(dir.path(), vec![]);
        let query = ConfigQuery::new("test", vec![] as Vec<String>);
        let sources = resolver.resolve(&query, "main").unwrap();

        assert_eq!(sources.len(), 1);
        assert!(sources[0].name.starts_with("git:main:"));
    }
}
