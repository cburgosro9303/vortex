//! JSON serialization helpers for deterministic output.

use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::ser::{PrettyFormatter, Serializer};
use std::io;

/// Error type for serialization operations.
#[derive(Debug, thiserror::Error)]
pub enum SerializationError {
    /// JSON serialization failed.
    #[error("JSON serialization failed: {0}")]
    Serialize(#[from] serde_json::Error),

    /// JSON deserialization failed.
    #[error("JSON deserialization failed: {0}")]
    Deserialize(serde_json::Error),

    /// UTF-8 encoding error.
    #[error("UTF-8 encoding error: {0}")]
    Utf8(#[from] std::string::FromUtf8Error),

    /// IO error.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),
}

/// Serializes a value to deterministic JSON.
///
/// Output format:
/// - 2-space indentation
/// - Trailing newline
/// - Keys sorted alphabetically (requires BTreeMap in source types)
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_json_stable<T: Serialize>(value: &T) -> Result<String, SerializationError> {
    let mut buffer = Vec::new();
    let formatter = PrettyFormatter::with_indent(b"  ");
    let mut serializer = Serializer::with_formatter(&mut buffer, formatter);
    value.serialize(&mut serializer)?;

    let mut json = String::from_utf8(buffer)?;
    json.push('\n'); // Trailing newline
    Ok(json)
}

/// Serializes a value to deterministic JSON bytes.
///
/// Same as `to_json_stable` but returns bytes for direct file writing.
///
/// # Errors
///
/// Returns an error if serialization fails.
pub fn to_json_stable_bytes<T: Serialize>(value: &T) -> Result<Vec<u8>, SerializationError> {
    let json = to_json_stable(value)?;
    Ok(json.into_bytes())
}

/// Deserializes JSON from a string.
///
/// Handles both pretty-printed and minified JSON.
///
/// # Errors
///
/// Returns an error if the JSON is invalid or doesn't match the expected type.
pub fn from_json<T: DeserializeOwned>(json: &str) -> Result<T, SerializationError> {
    serde_json::from_str(json).map_err(SerializationError::Deserialize)
}

/// Deserializes JSON from bytes.
///
/// Handles both pretty-printed and minified JSON.
///
/// # Errors
///
/// Returns an error if the JSON is invalid or doesn't match the expected type.
pub fn from_json_bytes<T: DeserializeOwned>(bytes: &[u8]) -> Result<T, SerializationError> {
    serde_json::from_slice(bytes).map_err(SerializationError::Deserialize)
}

/// Validates that JSON can be parsed without deserializing to a specific type.
///
/// Useful for schema validation before attempting typed deserialization.
///
/// # Errors
///
/// Returns an error if the JSON is invalid.
pub fn validate_json(json: &str) -> Result<serde_json::Value, SerializationError> {
    serde_json::from_str(json).map_err(SerializationError::Deserialize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[test]
    fn test_stable_serialization_has_trailing_newline() {
        let mut map = BTreeMap::new();
        map.insert("z_key", "value1");
        map.insert("a_key", "value2");

        let json = to_json_stable(&map).expect("serialization should work");
        assert!(json.ends_with('\n'));
    }

    #[test]
    fn test_stable_serialization_uses_two_space_indent() {
        let mut map = BTreeMap::new();
        map.insert("key", "value");

        let json = to_json_stable(&map).expect("serialization should work");
        assert!(json.contains("  \"key\""));
    }

    #[test]
    fn test_btreemap_keys_are_sorted() {
        let mut map = BTreeMap::new();
        map.insert("zebra", 1);
        map.insert("apple", 2);
        map.insert("mango", 3);

        let json = to_json_stable(&map).expect("serialization should work");
        let apple_pos = json.find("apple").expect("apple should be in json");
        let mango_pos = json.find("mango").expect("mango should be in json");
        let zebra_pos = json.find("zebra").expect("zebra should be in json");

        assert!(apple_pos < mango_pos);
        assert!(mango_pos < zebra_pos);
    }

    #[test]
    fn test_roundtrip_serialization() {
        let mut original = BTreeMap::new();
        original.insert("key".to_string(), "value".to_string());

        let json = to_json_stable(&original).expect("serialization should work");
        let restored: BTreeMap<String, String> =
            from_json(&json).expect("deserialization should work");

        assert_eq!(original, restored);
    }

    #[test]
    fn test_from_json_bytes() {
        let json = r#"{"name": "test"}"#;
        let result: serde_json::Value =
            from_json_bytes(json.as_bytes()).expect("deserialization should work");
        assert_eq!(result["name"], "test");
    }

    #[test]
    fn test_validate_json_valid() {
        let result = validate_json(r#"{"valid": true}"#);
        assert!(result.is_ok());
    }

    #[test]
    fn test_validate_json_invalid() {
        let result = validate_json(r#"{"invalid": }"#);
        assert!(result.is_err());
    }
}
