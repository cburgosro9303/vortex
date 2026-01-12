use crate::config::map::ConfigMap;
use serde::{Deserialize, Serialize};

/// Represents a source of configuration properties.
///
/// A property source acts as a named container for a set of configuration properties
/// (represented by `ConfigMap`). It usually corresponds to a file (e.g., "application.yml"),
/// a git repository, or an environment variable set.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertySource {
    /// The name of the property source (e.g., "application.yml").
    pub name: String,
    
    /// The source origin details (e.g., URI, file path).
    /// Kept simple for now.
    #[serde(default)]
    pub origin: String,

    /// Priority of this source. Higher values take precedence.
    #[serde(default)]
    pub priority: i32,

    /// The actual configuration properties.
    pub config: ConfigMap,
}

impl PropertySource {
    /// Creates a new PropertySource.
    pub fn new(name: impl Into<String>, config: ConfigMap) -> Self {
        Self {
            name: name.into(),
            origin: String::new(),
            priority: 0,
            config,
        }
    }
}
