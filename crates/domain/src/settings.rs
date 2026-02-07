//! User Settings Domain Model
//!
//! Defines user preferences for the Vortex API Client.

use serde::{Deserialize, Serialize};

/// Theme mode preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// Light mode theme.
    Light,
    /// Dark mode theme (default).
    #[default]
    Dark,
    /// Follow system theme preference.
    System,
}

impl ThemeMode {
    /// Returns true if dark mode should be used based on the preference.
    /// For System mode, this should be determined by the OS preference.
    #[must_use]
    pub fn is_dark(&self) -> bool {
        match self {
            Self::Light => false,
            Self::Dark => true,
            Self::System => true, // Default to dark for System until OS detection is implemented
        }
    }

    /// Convert to index for UI combo box.
    #[must_use]
    pub fn to_index(self) -> i32 {
        match self {
            Self::Light => 0,
            Self::Dark => 1,
            Self::System => 2,
        }
    }

    /// Create from UI combo box index.
    #[must_use]
    pub fn from_index(index: i32) -> Self {
        match index {
            0 => Self::Light,
            1 => Self::Dark,
            _ => Self::System,
        }
    }
}

/// Font scale preference.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "lowercase")]
pub enum FontScale {
    /// Small font scale (0.85x).
    Small,
    /// Medium font scale (1.0x, default).
    #[default]
    Medium,
    /// Large font scale (1.15x).
    Large,
}

impl FontScale {
    /// Returns the scale factor multiplier.
    #[must_use]
    pub fn factor(&self) -> f32 {
        match self {
            Self::Small => 0.85,
            Self::Medium => 1.0,
            Self::Large => 1.15,
        }
    }

    /// Convert to index for UI combo box.
    #[must_use]
    pub fn to_index(self) -> i32 {
        match self {
            Self::Small => 0,
            Self::Medium => 1,
            Self::Large => 2,
        }
    }

    /// Create from UI combo box index.
    #[must_use]
    pub fn from_index(index: i32) -> Self {
        match index {
            0 => Self::Small,
            2 => Self::Large,
            _ => Self::Medium,
        }
    }
}

/// User settings for the application.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UserSettings {
    /// Theme mode preference.
    #[serde(default)]
    pub theme: ThemeMode,

    /// Font scale preference.
    #[serde(default)]
    pub font_scale: FontScale,

    /// Whether the history panel is visible.
    #[serde(default = "default_history_visible")]
    pub history_visible: bool,

    /// Maximum number of history entries to keep.
    #[serde(default = "default_history_limit")]
    pub history_limit: usize,

    /// Sidebar width in pixels.
    #[serde(default = "default_sidebar_width")]
    pub sidebar_width: u32,
}

fn default_history_visible() -> bool {
    true
}

fn default_history_limit() -> usize {
    100
}

fn default_sidebar_width() -> u32 {
    280
}

impl Default for UserSettings {
    fn default() -> Self {
        Self {
            theme: ThemeMode::default(),
            font_scale: FontScale::default(),
            history_visible: default_history_visible(),
            history_limit: default_history_limit(),
            sidebar_width: default_sidebar_width(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_mode_is_dark() {
        assert!(!ThemeMode::Light.is_dark());
        assert!(ThemeMode::Dark.is_dark());
        assert!(ThemeMode::System.is_dark()); // Default behavior
    }

    #[test]
    fn font_scale_factors() {
        assert!((FontScale::Small.factor() - 0.85).abs() < f32::EPSILON);
        assert!((FontScale::Medium.factor() - 1.0).abs() < f32::EPSILON);
        assert!((FontScale::Large.factor() - 1.15).abs() < f32::EPSILON);
    }

    #[test]
    fn default_settings() {
        let settings = UserSettings::default();
        assert_eq!(settings.theme, ThemeMode::Dark);
        assert_eq!(settings.font_scale, FontScale::Medium);
        assert!(settings.history_visible);
        assert_eq!(settings.history_limit, 100);
        assert_eq!(settings.sidebar_width, 280);
    }
}
