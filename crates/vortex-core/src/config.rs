//! Configuration structures for Vortex Config.

use crate::types::{Application, Label, Profile};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

/// A collection of configuration properties from a specific source.
///
/// PropertySource represents configuration loaded from a single file
/// or backend. Multiple PropertySources are combined to form a ConfigMap.
///
/// # Example
///
/// ```
/// use vortex_core::PropertySource;
/// use std::collections::HashMap;
///
/// let mut props = HashMap::new();
/// props.insert("server.port".to_string(), "8080".to_string());
///
/// let source = PropertySource::new("application.yml", props);
/// assert_eq!(source.get("server.port"), Some("8080"));
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PropertySource {
    /// Name of the source (typically filename or backend identifier)
    name: String,
    /// Key-value properties from this source
    properties: HashMap<String, String>,
}

impl PropertySource {
    /// Creates a new PropertySource with the given name and properties.
    pub fn new(name: impl Into<String>, properties: HashMap<String, String>) -> Self {
        Self {
            name: name.into(),
            properties,
        }
    }

    /// Returns the name of this property source.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Returns a reference to the properties map.
    pub fn properties(&self) -> &HashMap<String, String> {
        &self.properties
    }

    /// Gets a property value by key.
    pub fn get(&self, key: &str) -> Option<&str> {
        self.properties.get(key).map(|s| s.as_str())
    }

    /// Returns the number of properties.
    pub fn len(&self) -> usize {
        self.properties.len()
    }

    /// Returns true if there are no properties.
    pub fn is_empty(&self) -> bool {
        self.properties.is_empty()
    }
}

/// Complete configuration for an application.
///
/// ConfigMap aggregates configuration from multiple PropertySources
/// for a specific application, profile(s), and label combination.
/// PropertySources are ordered by precedence (first source wins).
///
/// # Example
///
/// ```
/// use vortex_core::{ConfigMap, Application, Profile};
///
/// let config = ConfigMap::builder()
///     .application(Application::new("myapp"))
///     .profile(Profile::new("production"))
///     .build();
///
/// assert_eq!(config.application().as_str(), "myapp");
/// ```
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ConfigMap {
    /// Application this configuration belongs to
    application: Application,
    /// Active profiles
    profiles: Vec<Profile>,
    /// Configuration version/branch
    label: Option<Label>,
    /// Ordered list of property sources (first wins)
    property_sources: Vec<PropertySource>,
}

impl ConfigMap {
    /// Creates a new ConfigMap with the given application name.
    pub fn new(application: impl Into<Application>) -> Self {
        Self {
            application: application.into(),
            profiles: vec![Profile::default_profile()],
            label: None,
            property_sources: Vec::new(),
        }
    }

    /// Returns a builder for constructing a ConfigMap.
    pub fn builder() -> ConfigMapBuilder {
        ConfigMapBuilder::default()
    }

    /// Returns the application identifier.
    pub fn application(&self) -> &Application {
        &self.application
    }

    /// Returns the active profiles.
    pub fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    /// Returns the configuration label, if set.
    pub fn label(&self) -> Option<&Label> {
        self.label.as_ref()
    }

    /// Returns the property sources.
    pub fn property_sources(&self) -> &[PropertySource] {
        &self.property_sources
    }

    /// Gets a property value, searching through sources in order.
    ///
    /// Returns the value from the first PropertySource that contains
    /// the key, or None if not found in any source.
    pub fn get_property(&self, key: &str) -> Option<&str> {
        self.property_sources
            .iter()
            .find_map(|source| source.get(key))
    }

    /// Returns all unique property keys across all sources.
    pub fn property_keys(&self) -> Vec<&str> {
        let mut keys: Vec<&str> = self
            .property_sources
            .iter()
            .flat_map(|s| s.properties().keys().map(|k| k.as_str()))
            .collect();
        keys.sort();
        keys.dedup();
        keys
    }
}

/// Builder for ConfigMap.
#[derive(Debug, Default)]
pub struct ConfigMapBuilder {
    application: Option<Application>,
    profiles: Vec<Profile>,
    label: Option<Label>,
    property_sources: Vec<PropertySource>,
}

impl ConfigMapBuilder {
    /// Sets the application identifier.
    pub fn application(mut self, app: impl Into<Application>) -> Self {
        self.application = Some(app.into());
        self
    }

    /// Adds a profile.
    pub fn profile(mut self, profile: impl Into<Profile>) -> Self {
        self.profiles.push(profile.into());
        self
    }

    /// Sets the label.
    pub fn label(mut self, label: impl Into<Label>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Adds a property source.
    pub fn property_source(mut self, source: PropertySource) -> Self {
        self.property_sources.push(source);
        self
    }

    /// Builds the ConfigMap.
    ///
    /// # Panics
    ///
    /// Panics if application is not set.
    pub fn build(self) -> ConfigMap {
        ConfigMap {
            application: self.application.expect("Application must be set"),
            profiles: if self.profiles.is_empty() {
                vec![Profile::default_profile()]
            } else {
                self.profiles
            },
            label: self.label,
            property_sources: self.property_sources,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_property_source_creation() {
        let mut props = HashMap::new();
        props.insert("key".to_string(), "value".to_string());

        let source = PropertySource::new("test.yml", props);

        assert_eq!(source.name(), "test.yml");
        assert_eq!(source.get("key"), Some("value"));
        assert_eq!(source.get("missing"), None);
    }

    #[test]
    fn test_config_map_builder() {
        let config = ConfigMap::builder()
            .application("myapp")
            .profile(Profile::new("prod"))
            .label(Label::new("v1.0"))
            .build();

        assert_eq!(config.application().as_str(), "myapp");
        assert_eq!(config.profiles().len(), 1);
        assert!(config.label().is_some());
    }

    #[test]
    fn test_property_lookup_precedence() {
        let source1 = PropertySource::new(
            "high-priority",
            HashMap::from([("key".into(), "from-source1".into())]),
        );
        let source2 = PropertySource::new(
            "low-priority",
            HashMap::from([("key".into(), "from-source2".into())]),
        );

        let config = ConfigMap::builder()
            .application("test")
            .property_source(source1)
            .property_source(source2)
            .build();

        // First source wins
        assert_eq!(config.get_property("key"), Some("from-source1"));
    }

    #[test]
    fn test_serialization_roundtrip() {
        let config = ConfigMap::builder()
            .application("myapp")
            .profile(Profile::new("production"))
            .build();

        let json = serde_json::to_string(&config).unwrap();
        let restored: ConfigMap = serde_json::from_str(&json).unwrap();

        assert_eq!(config, restored);
    }

    #[test]
    fn test_application_display() {
        let app = Application::new("payment-service");
        assert_eq!(format!("{}", app), "payment-service");
    }
}
