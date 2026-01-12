//! Configuration file format detection and handling.

use std::path::Path;

/// Supported configuration file formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ConfigFormat {
    /// YAML format (.yml, .yaml)
    Yaml,
    /// JSON format (.json)
    Json,
    /// Java Properties format (.properties)
    Properties,
}

impl ConfigFormat {
    /// Detects the format from a file path based on extension.
    ///
    /// # Returns
    ///
    /// `Some(format)` if the extension is recognized, `None` otherwise.
    pub fn from_path(path: &Path) -> Option<Self> {
        path.extension()
            .and_then(|ext| ext.to_str())
            .and_then(Self::from_extension)
    }

    /// Detects the format from a file extension string.
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "yml" | "yaml" => Some(Self::Yaml),
            "json" => Some(Self::Json),
            "properties" => Some(Self::Properties),
            _ => None,
        }
    }

    /// Returns the primary file extension for this format.
    pub fn extension(&self) -> &'static str {
        match self {
            Self::Yaml => "yml",
            Self::Json => "json",
            Self::Properties => "properties",
        }
    }

    /// Returns all file extensions for this format.
    pub fn extensions(&self) -> &'static [&'static str] {
        match self {
            Self::Yaml => &["yml", "yaml"],
            Self::Json => &["json"],
            Self::Properties => &["properties"],
        }
    }

    /// Returns the MIME type for this format.
    pub fn mime_type(&self) -> &'static str {
        match self {
            Self::Yaml => "application/x-yaml",
            Self::Json => "application/json",
            Self::Properties => "text/plain",
        }
    }

    /// Returns all supported formats.
    pub fn all() -> &'static [Self] {
        &[Self::Yaml, Self::Json, Self::Properties]
    }
}

impl std::fmt::Display for ConfigFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Yaml => write!(f, "YAML"),
            Self::Json => write!(f, "JSON"),
            Self::Properties => write!(f, "Properties"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_from_path() {
        assert_eq!(
            ConfigFormat::from_path(Path::new("config.yml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("config.yaml")),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("config.json")),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigFormat::from_path(Path::new("config.properties")),
            Some(ConfigFormat::Properties)
        );
        assert_eq!(ConfigFormat::from_path(Path::new("config.txt")), None);
        assert_eq!(ConfigFormat::from_path(Path::new("config")), None);
    }

    #[test]
    fn test_from_extension() {
        assert_eq!(
            ConfigFormat::from_extension("yml"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_extension("YML"),
            Some(ConfigFormat::Yaml)
        );
        assert_eq!(
            ConfigFormat::from_extension("json"),
            Some(ConfigFormat::Json)
        );
        assert_eq!(
            ConfigFormat::from_extension("properties"),
            Some(ConfigFormat::Properties)
        );
        assert_eq!(ConfigFormat::from_extension("txt"), None);
    }

    #[test]
    fn test_extension() {
        assert_eq!(ConfigFormat::Yaml.extension(), "yml");
        assert_eq!(ConfigFormat::Json.extension(), "json");
        assert_eq!(ConfigFormat::Properties.extension(), "properties");
    }

    #[test]
    fn test_extensions() {
        assert_eq!(ConfigFormat::Yaml.extensions(), &["yml", "yaml"]);
        assert_eq!(ConfigFormat::Json.extensions(), &["json"]);
        assert_eq!(ConfigFormat::Properties.extensions(), &["properties"]);
    }

    #[test]
    fn test_mime_type() {
        assert_eq!(ConfigFormat::Yaml.mime_type(), "application/x-yaml");
        assert_eq!(ConfigFormat::Json.mime_type(), "application/json");
        assert_eq!(ConfigFormat::Properties.mime_type(), "text/plain");
    }
}
