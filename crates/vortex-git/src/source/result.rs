//! Configuration result types.

use serde::{Deserialize, Serialize};
use vortex_core::PropertySource;

/// The result of fetching configuration from a source.
///
/// This follows the Spring Cloud Config response format.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfigResult {
    /// The application name.
    name: String,

    /// The active profiles.
    profiles: Vec<String>,

    /// The resolved label (branch/tag/commit).
    label: String,

    /// The version (e.g., commit hash).
    version: Option<String>,

    /// Additional state information.
    state: Option<String>,

    /// The property sources in order of precedence (first = highest).
    property_sources: Vec<PropertySource>,
}

impl ConfigResult {
    /// Creates a new configuration result.
    pub fn new(name: impl Into<String>, profiles: Vec<String>, label: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            profiles,
            label: label.into(),
            version: None,
            state: None,
            property_sources: Vec::new(),
        }
    }

    /// Returns the application name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns the active profiles.
    pub fn profiles(&self) -> &[String] {
        &self.profiles
    }

    /// Returns the resolved label.
    pub fn label(&self) -> &str {
        &self.label
    }

    /// Returns the version (commit hash) if available.
    pub fn version(&self) -> Option<&str> {
        self.version.as_deref()
    }

    /// Returns the state if available.
    pub fn state(&self) -> Option<&str> {
        self.state.as_deref()
    }

    /// Returns the property sources.
    pub fn property_sources(&self) -> &[PropertySource] {
        &self.property_sources
    }

    /// Sets the version.
    pub fn set_version(&mut self, version: impl Into<String>) {
        self.version = Some(version.into());
    }

    /// Sets the state.
    pub fn set_state(&mut self, state: impl Into<String>) {
        self.state = Some(state.into());
    }

    /// Adds a property source.
    ///
    /// Sources added first have higher precedence.
    pub fn add_property_source(&mut self, source: PropertySource) {
        self.property_sources.push(source);
    }

    /// Adds multiple property sources.
    pub fn add_property_sources(&mut self, sources: impl IntoIterator<Item = PropertySource>) {
        self.property_sources.extend(sources);
    }

    /// Returns true if there are no property sources.
    pub fn is_empty(&self) -> bool {
        self.property_sources.is_empty()
    }

    /// Returns the number of property sources.
    pub fn len(&self) -> usize {
        self.property_sources.len()
    }

    /// Builder-style method to set version.
    pub fn with_version(mut self, version: impl Into<String>) -> Self {
        self.version = Some(version.into());
        self
    }

    /// Builder-style method to set state.
    pub fn with_state(mut self, state: impl Into<String>) -> Self {
        self.state = Some(state.into());
        self
    }

    /// Builder-style method to add property sources.
    pub fn with_property_sources(mut self, sources: Vec<PropertySource>) -> Self {
        self.property_sources = sources;
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_core::ConfigMap;

    #[test]
    fn test_new_result() {
        let result = ConfigResult::new("myapp", vec!["dev".to_string()], "main");
        assert_eq!(result.name(), "myapp");
        assert_eq!(result.profiles(), &["dev"]);
        assert_eq!(result.label(), "main");
        assert!(result.version().is_none());
        assert!(result.is_empty());
    }

    #[test]
    fn test_with_version() {
        let result = ConfigResult::new("myapp", vec!["dev".to_string()], "main")
            .with_version("abc123");
        assert_eq!(result.version(), Some("abc123"));
    }

    #[test]
    fn test_add_property_source() {
        let mut result = ConfigResult::new("myapp", vec!["dev".to_string()], "main");

        let source = PropertySource::new("git:main:config/myapp.yml", ConfigMap::new());
        result.add_property_source(source);

        assert_eq!(result.len(), 1);
        assert!(!result.is_empty());
    }

    #[test]
    fn test_serialization() {
        let result = ConfigResult::new("myapp", vec!["dev".to_string()], "main")
            .with_version("abc123");

        let json = serde_json::to_string(&result).unwrap();
        assert!(json.contains("\"name\":\"myapp\""));
        assert!(json.contains("\"propertySources\""));
    }
}
