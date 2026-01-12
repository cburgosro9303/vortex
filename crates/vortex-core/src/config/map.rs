use crate::config::value::ConfigValue;
use crate::error::{Result, VortexError};
use indexmap::IndexMap;
use serde::{Deserialize, Serialize};

/// A configuration map that holds key-value pairs with support for nested structures.
///
/// This struct wraps an `IndexMap<String, ConfigValue>` to provide specialized
/// methods for configuration handling, such as dot-notation access and
/// format conversion (JSON/YAML).
///
/// We use `IndexMap` to ensure iteration order is deterministic (insertion order),
/// which is important for predictable text-based outputs.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct ConfigMap {
    #[serde(flatten)]
    inner: IndexMap<String, ConfigValue>,
}

impl ConfigMap {
    /// Creates a new empty configuration map.
    pub fn new() -> Self {
        Self {
            inner: IndexMap::new(),
        }
    }

    /// Creates a ConfigMap from an existing IndexMap.
    pub fn from_inner(inner: IndexMap<String, ConfigValue>) -> Self {
        Self { inner }
    }

    /// Returns a reference to the internal map.
    pub fn as_inner(&self) -> &IndexMap<String, ConfigValue> {
        &self.inner
    }

     /// Returns a mutable reference to the internal map.
    pub fn as_inner_mut(&mut self) -> &mut IndexMap<String, ConfigValue> {
        &mut self.inner
    }

    /// Returns true if the map contains no elements.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns the number of elements in the map.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Inserts a key-value pair into the map.
    pub fn insert(&mut self, key: impl Into<String>, value: impl Into<ConfigValue>) {
        self.inner.insert(key.into(), value.into());
    }

    /// Retrieves a value by key, supporting dot notation for nested access.
    ///
    /// # Example
    /// ```
    /// # use vortex_core::{ConfigMap, ConfigValue};
    /// let mut map = ConfigMap::new();
    /// // Assuming map has {"server": {"port": 8080}}
    /// // map.get("server.port") returns Some(&ConfigValue::Integer(8080))
    /// ```
    pub fn get(&self, path: &str) -> Option<&ConfigValue> {
        if path.is_empty() {
             return None;
        }

        // Fast path for simple keys
        if !path.contains('.') {
            return self.inner.get(path);
        }

        // Recursive lookup for dot notation
        let parts: Vec<&str> = path.split('.').collect();
        let mut current_value = self.inner.get(parts[0])?;

        for part in &parts[1..] {
            match current_value {
                ConfigValue::Object(map) => {
                    current_value = map.get(*part)?;
                }
                _ => return None,
            }
        }

        Some(current_value)
    }

    /// Parses a JSON string into a ConfigMap.
    pub fn from_json(json: &str) -> Result<Self> {
        serde_json::from_str(json).map_err(|e| VortexError::parse_error("json_source", e.to_string()))
    }

    /// Serializes the map to a JSON string (pretty printed).
    pub fn to_json(&self) -> Result<String> {
        serde_json::to_string_pretty(self).map_err(|e| VortexError::parse_error("json_target", e.to_string()))
    }

    /// Parses a YAML string into a ConfigMap.
    pub fn from_yaml(yaml: &str) -> Result<Self> {
        serde_yaml::from_str(yaml).map_err(|e| VortexError::parse_error("yaml_source", e.to_string()))
    }

    /// Serializes the map to a YAML string.
    pub fn to_yaml(&self) -> Result<String> {
        serde_yaml::to_string(self).map_err(|e| VortexError::parse_error("yaml_target", e.to_string()))
    }
}

// Implement From<IndexMap>
impl From<IndexMap<String, ConfigValue>> for ConfigMap {
    fn from(map: IndexMap<String, ConfigValue>) -> Self {
        ConfigMap { inner: map }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_nested_access() {
        let json = r#"
        {
            "server": {
                "port": 8080,
                "host": "localhost",
                "admin": {
                    "enabled": true
                }
            }
        }
        "#;
        let config = ConfigMap::from_json(json).unwrap();

        assert_eq!(config.get("server.port").unwrap().as_i64(), Some(8080));
        assert_eq!(config.get("server.host").unwrap().as_str(), Some("localhost"));
        assert_eq!(config.get("server.admin.enabled").unwrap().as_bool(), Some(true));
        
        // Non-existent
        assert_eq!(config.get("server.ssl"), None);
        assert_eq!(config.get("server.port.sub"), None); // port is integer, not object
    }

    #[test]
    fn test_yaml_roundtrip() {
        let mut map = ConfigMap::new();
        map.insert("key", "value");
        map.insert("num", 100);

        let yaml = map.to_yaml().unwrap();
        let from_yaml = ConfigMap::from_yaml(&yaml).unwrap();

        assert_eq!(map, from_yaml);
    }
}
