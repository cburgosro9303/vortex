//! Request history persistence.
//!
//! Stores request history in the platform-specific config directory:
//! - Linux/macOS: ~/.config/vortex/history.json
//! - Windows: %APPDATA%/vortex/history.json

use std::path::PathBuf;

use tokio::fs;
use vortex_domain::RequestHistory;

use crate::serialization::{SerializationError, from_json_bytes, to_json_stable_bytes};

/// Error type for history operations.
#[derive(Debug, thiserror::Error)]
pub enum HistoryError {
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

/// Repository for request history persistence.
#[derive(Debug, Clone, Default)]
pub struct HistoryRepository;

impl HistoryRepository {
    /// Creates a new history repository.
    #[must_use]
    pub const fn new() -> Self {
        Self
    }

    /// Returns the path to the Vortex config directory.
    fn config_dir() -> Option<PathBuf> {
        dirs::config_dir().map(|p| p.join("vortex"))
    }

    /// Returns the path to the history file.
    fn history_path() -> Option<PathBuf> {
        Self::config_dir().map(|p| p.join("history.json"))
    }

    /// Loads request history from disk.
    ///
    /// Returns empty history if the file doesn't exist.
    pub async fn load(&self) -> Result<RequestHistory, HistoryError> {
        let Some(path) = Self::history_path() else {
            return Ok(RequestHistory::new(100));
        };

        if !path.exists() {
            return Ok(RequestHistory::new(100));
        }

        let content = fs::read(&path).await?;
        let history = from_json_bytes(&content)?;
        Ok(history)
    }

    /// Saves request history to disk.
    pub async fn save(&self, history: &RequestHistory) -> Result<(), HistoryError> {
        let Some(config_dir) = Self::config_dir() else {
            return Err(HistoryError::NoConfigDir);
        };

        let Some(path) = Self::history_path() else {
            return Err(HistoryError::NoConfigDir);
        };

        // Ensure config directory exists
        fs::create_dir_all(&config_dir).await?;

        let content = to_json_stable_bytes(history)?;
        fs::write(&path, content).await?;

        Ok(())
    }

    /// Returns the path where history is stored, if available.
    #[must_use]
    pub fn get_history_path() -> Option<PathBuf> {
        Self::history_path()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn history_path_is_valid() {
        let path = HistoryRepository::history_path();
        if let Some(p) = path {
            assert!(p.ends_with("vortex/history.json"));
        }
    }

    #[tokio::test]
    async fn load_returns_empty_when_no_file() {
        let repo = HistoryRepository::new();
        let result = repo.load().await;
        assert!(result.is_ok());
    }
}
