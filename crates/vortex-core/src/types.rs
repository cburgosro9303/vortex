//! Common type definitions and newtypes for Vortex Config.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Application identifier.
///
/// Represents the name of an application whose configuration
/// is being managed. This is typically the service name.
///
/// # Example
///
/// ```
/// use vortex_core::Application;
///
/// let app = Application::new("payment-service");
/// assert_eq!(app.as_str(), "payment-service");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Application(String);

impl Application {
    /// Creates a new Application identifier.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the application name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Application {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl From<&str> for Application {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Application {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Execution profile for configuration selection.
///
/// Profiles allow different configurations for different environments.
/// Common profiles: "default", "development", "staging", "production".
///
/// # Example
///
/// ```
/// use vortex_core::Profile;
///
/// let profile = Profile::new("production");
/// assert_eq!(profile.as_str(), "production");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Profile(String);

impl Profile {
    /// Creates a new Profile with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the profile name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the default profile.
    pub fn default_profile() -> Self {
        Self::new("default")
    }
}

impl From<&str> for Profile {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Profile {
    fn from(s: String) -> Self {
        Self(s)
    }
}

/// Configuration version or branch label.
///
/// Labels identify specific versions of configuration, typically
/// corresponding to Git branches or tags.
///
/// # Example
///
/// ```
/// use vortex_core::Label;
///
/// let label = Label::new("v1.0.0");
/// assert_eq!(label.as_str(), "v1.0.0");
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Label(String);

impl Label {
    /// Creates a new Label with the given name.
    pub fn new(name: impl Into<String>) -> Self {
        Self(name.into())
    }

    /// Returns the label name as a string slice.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Returns the main/default label.
    pub fn main() -> Self {
        Self::new("main")
    }
}

impl From<&str> for Label {
    fn from(s: &str) -> Self {
        Self::new(s)
    }
}

impl From<String> for Label {
    fn from(s: String) -> Self {
        Self(s)
    }
}
