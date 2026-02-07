//! User settings persistence.
//!
//! Stores user settings in the platform-specific config directory:
//! - Linux/macOS: ~/.config/vortex/settings.json
//! - Windows: %APPDATA%/vortex/settings.json

use std::path::PathBuf;

use tokio::fs;
use vortex_domain::UserSettings;

use crate::serialization::{from_json_bytes, to_json_stable_bytes, SerializationError};

/// Error type for settings operations.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    /// IO error during file operations.
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// Serialization/deserialization error.
    #[error("Serialization error: {0}")]
    Serialization(#[from] SerializationError),

    /// Could not determine config directory.
    #[error("Could not determine config directory")]
    NoConfigDir,
}

/// Repository for user settings persistence.
#[derive(Debug, Clone, Default)]
pub struct SettingsRepository;

impl SettingsRepository {
    /// Creates a new settings repository.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Returns the path to the Vortex config directory.
    fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("vortex"))
    }

    /// Returns the path to the settings file.
    fn settings_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("settings.json"))
    }

    /// Loads user settings from disk.
    ///
    /// Returns default settings if the file doesn't exist.
    pub async fn load(&self) -> Result<UserSettings, SettingsError> {
        let Some(path) = Self::settings_path() else {
            return Ok(UserSettings::default());
        };

        if !path.exists() {
            return Ok(UserSettings::default());
        }

        let content = fs::read(&path).await?;
        let settings = from_json_bytes(&content)?;
        Ok(settings)
    }

    /// Saves user settings to disk.
    pub async fn save(&self, settings: &UserSettings) -> Result<(), SettingsError> {
        let Some(config_dir) = Self::config_dir() else {
            return Err(SettingsError::NoConfigDir);
        };

        let Some(path) = Self::settings_path() else {
            return Err(SettingsError::NoConfigDir);
        };

        // Ensure config directory exists
        fs::create_dir_all(&config_dir).await?;

        let content = to_json_stable_bytes(settings)?;
        fs::write(&path, content).await?;

        Ok(())
    }

    /// Returns the path where settings are stored, if available.
    #[must_use]
    pub fn get_settings_path() -> Option<PathBuf> {
        Self::settings_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn settings_path_is_valid() {
        let path = SettingsRepository::settings_path();
        // Path should exist on most systems
        if let Some(p) = path {
            assert!(p.ends_with("vortex/settings.json"));
        }
    }

    #[tokio::test]
    async fn load_returns_default_when_no_file() {
        // This test depends on the file not existing
        // In a real test environment, we'd use a temp directory
        let repo = SettingsRepository::new();
        let result = repo.load().await;
        assert!(result.is_ok());
    }
}
