//! Configuration file parsing.

use std::path::Path;

use vortex_core::format::properties::PropertiesFormat;
use vortex_core::format::FormatParser;
use vortex_core::ConfigMap;

use super::ConfigFormat;
use crate::error::ConfigSourceError;

/// Parser for configuration files.
pub struct ConfigParser;

impl ConfigParser {
    /// Parses configuration content based on the specified format.
    pub fn parse(content: &str, format: ConfigFormat) -> Result<ConfigMap, ConfigSourceError> {
        match format {
            ConfigFormat::Yaml => Self::parse_yaml(content),
            ConfigFormat::Json => Self::parse_json(content),
            ConfigFormat::Properties => Self::parse_properties(content),
        }
    }

    /// Parses configuration from a file, detecting format from extension.
    pub fn parse_file(path: &Path) -> Result<ConfigMap, ConfigSourceError> {
        let format = ConfigFormat::from_path(path).ok_or_else(|| {
            ConfigSourceError::UnsupportedFormat(
                path.extension()
                    .and_then(|e| e.to_str())
                    .unwrap_or("unknown")
                    .to_string(),
            )
        })?;

        let content = std::fs::read_to_string(path)?;
        Self::parse(&content, format).map_err(|e| match e {
            ConfigSourceError::Parse { reason, .. } => {
                ConfigSourceError::parse(path, reason)
            }
            other => other,
        })
    }

    /// Parses YAML content.
    fn parse_yaml(content: &str) -> Result<ConfigMap, ConfigSourceError> {
        ConfigMap::from_yaml(content)
            .map_err(|e| ConfigSourceError::parse("", e.to_string()))
    }

    /// Parses JSON content.
    fn parse_json(content: &str) -> Result<ConfigMap, ConfigSourceError> {
        ConfigMap::from_json(content)
            .map_err(|e| ConfigSourceError::parse("", e.to_string()))
    }

    /// Parses Java Properties content.
    fn parse_properties(content: &str) -> Result<ConfigMap, ConfigSourceError> {
        let parser = PropertiesFormat;
        parser
            .parse(content)
            .map_err(|e: vortex_core::VortexError| ConfigSourceError::parse("", e.to_string()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use vortex_core::ConfigValue;

    #[test]
    fn test_parse_yaml() {
        let yaml = r#"
server:
  port: 8080
  host: localhost
app:
  name: myapp
  debug: true
"#;

        let map = ConfigParser::parse(yaml, ConfigFormat::Yaml).unwrap();
        assert_eq!(
            map.get("server.port"),
            Some(&ConfigValue::Integer(8080))
        );
        assert_eq!(
            map.get("server.host"),
            Some(&ConfigValue::String("localhost".to_string()))
        );
        assert_eq!(
            map.get("app.name"),
            Some(&ConfigValue::String("myapp".to_string()))
        );
        assert_eq!(map.get("app.debug"), Some(&ConfigValue::Bool(true)));
    }

    #[test]
    fn test_parse_json() {
        let json = r#"{
            "server": {
                "port": 8080,
                "host": "localhost"
            },
            "app": {
                "name": "myapp"
            }
        }"#;

        let map = ConfigParser::parse(json, ConfigFormat::Json).unwrap();
        assert_eq!(
            map.get("server.port"),
            Some(&ConfigValue::Integer(8080))
        );
        assert_eq!(
            map.get("server.host"),
            Some(&ConfigValue::String("localhost".to_string()))
        );
    }

    #[test]
    fn test_parse_properties() {
        let props = r#"
# Comment
server.port=8080
server.host=localhost
app.name=myapp
app.debug=true
"#;

        let map = ConfigParser::parse(props, ConfigFormat::Properties).unwrap();
        // Properties parser creates nested structure, use dot notation access
        assert_eq!(
            map.get("server.port"),
            Some(&ConfigValue::String("8080".to_string()))
        );
        assert_eq!(
            map.get("server.host"),
            Some(&ConfigValue::String("localhost".to_string()))
        );
        assert_eq!(
            map.get("app.name"),
            Some(&ConfigValue::String("myapp".to_string()))
        );
    }

    #[test]
    fn test_parse_yaml_with_arrays() {
        let yaml = r#"
servers:
  - host: server1
    port: 8080
  - host: server2
    port: 8081
"#;

        let map = ConfigParser::parse(yaml, ConfigFormat::Yaml).unwrap();
        // Arrays are stored as ConfigValue::Array
        let servers = map.as_inner().get("servers");
        assert!(servers.is_some());
        if let Some(ConfigValue::Array(arr)) = servers {
            assert_eq!(arr.len(), 2);
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_parse_invalid_yaml() {
        let invalid = "key: [invalid";
        let result = ConfigParser::parse(invalid, ConfigFormat::Yaml);
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_invalid_json() {
        let invalid = "{ invalid }";
        let result = ConfigParser::parse(invalid, ConfigFormat::Json);
        assert!(result.is_err());
    }
}
