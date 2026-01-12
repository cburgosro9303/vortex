use indexmap::IndexMap;
use ordered_float::OrderedFloat;
use serde::{Deserialize, Serialize};

/// Represents a configuration value that can be of various types from JSON/YAML.
///
/// This enum is the core building block for dynamic configuration structures.
/// It supports recursive types (Arrays inside Objects, etc.) and uses `IndexMap`
/// to preserve key order, which is crucial for configuration predictability.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ConfigValue {
    /// Null value
    Null,
    /// Boolean value
    Bool(bool),
    /// Integer value (signed 64-bit)
    Integer(i64),
    /// Floating point value (wrapped in OrderedFloat for Eq support)
    Float(OrderedFloat<f64>),
    /// String value
    String(String),
    /// Array of values
    Array(Vec<ConfigValue>),
    /// Object (Map) of values
    Object(IndexMap<String, ConfigValue>),
}

impl ConfigValue {
    /// Returns true if the value is Null.
    pub fn is_null(&self) -> bool {
        matches!(self, ConfigValue::Null)
    }

    /// Returns the value as a bool if it matches.
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            ConfigValue::Bool(b) => Some(*b),
            _ => None,
        }
    }

    /// Returns the value as an i64 if it matches.
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            ConfigValue::Integer(i) => Some(*i),
            _ => None,
        }
    }

    /// Returns the value as an f64 if it matches (Integer or Float).
    pub fn as_f64(&self) -> Option<f64> {
        match self {
            ConfigValue::Float(f) => Some(f.into_inner()),
            ConfigValue::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    /// Returns the value as a str if it matches.
    pub fn as_str(&self) -> Option<&str> {
        match self {
            ConfigValue::String(s) => Some(s),
            _ => None,
        }
    }

    /// Returns the value as an array slice if it matches.
    pub fn as_array(&self) -> Option<&[ConfigValue]> {
        match self {
            ConfigValue::Array(arr) => Some(arr),
            _ => None,
        }
    }

    /// Returns the value as an object (IndexMap) if it matches.
    pub fn as_object(&self) -> Option<&IndexMap<String, ConfigValue>> {
        match self {
            ConfigValue::Object(map) => Some(map),
            _ => None,
        }
    }
}

// ==========================================
// From Conversions for Ergonomics
// ==========================================

impl From<bool> for ConfigValue {
    fn from(v: bool) -> Self {
        ConfigValue::Bool(v)
    }
}

impl From<i64> for ConfigValue {
    fn from(v: i64) -> Self {
        ConfigValue::Integer(v)
    }
}

impl From<i32> for ConfigValue {
    fn from(v: i32) -> Self {
        ConfigValue::Integer(v as i64)
    }
}

impl From<f64> for ConfigValue {
    fn from(v: f64) -> Self {
        ConfigValue::Float(OrderedFloat(v))
    }
}

impl From<String> for ConfigValue {
    fn from(v: String) -> Self {
        ConfigValue::String(v)
    }
}

impl From<&str> for ConfigValue {
    fn from(v: &str) -> Self {
        ConfigValue::String(v.to_string())
    }
}

impl<T: Into<ConfigValue>> From<Vec<T>> for ConfigValue {
    fn from(v: Vec<T>) -> Self {
        ConfigValue::Array(v.into_iter().map(Into::into).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_value_creation() {
        let v: ConfigValue = 42.into();
        assert_eq!(v, ConfigValue::Integer(42));
        assert_eq!(v.as_i64(), Some(42));
        assert_eq!(v.as_f64(), Some(42.0));

        let s: ConfigValue = "hello".into();
        assert_eq!(s.as_str(), Some("hello"));
    }

    #[test]
    fn test_serde_serialization() {
        let v: ConfigValue = vec![1, 2].into();
        let json = serde_json::to_string(&v).unwrap();
        assert_eq!(json, "[1,2]");
    }

    #[test]
    fn test_serde_deserialization() {
        let json = r#"{"key": "value", "num": 10.5}"#;
        // Directly deserialize into ConfigValue (as Object)
        let v: ConfigValue = serde_json::from_str(json).unwrap();
        
        if let ConfigValue::Object(map) = v {
            assert_eq!(map.get("key").unwrap().as_str(), Some("value"));
            assert_eq!(map.get("num").unwrap().as_f64(), Some(10.5));
        } else {
            panic!("Expected Object");
        }
    }
}
