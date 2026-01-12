//! Configuration query types.

use serde::{Deserialize, Serialize};

/// A query for fetching configuration from a source.
///
/// This follows Spring Cloud Config conventions where configuration
/// is organized by application name, profiles, and optional label.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct ConfigQuery {
    /// The application name (e.g., "myapp").
    application: String,

    /// The active profiles (e.g., ["dev", "local"]).
    profiles: Vec<String>,

    /// The label (branch, tag, or commit). None means use default.
    label: Option<String>,
}

impl ConfigQuery {
    /// Creates a new configuration query.
    ///
    /// # Arguments
    ///
    /// * `application` - The application name
    /// * `profiles` - The active profiles
    ///
    /// # Example
    ///
    /// ```
    /// use vortex_git::ConfigQuery;
    ///
    /// let query = ConfigQuery::new("myapp", vec!["dev", "local"]);
    /// assert_eq!(query.application(), "myapp");
    /// assert_eq!(query.profiles(), &["dev", "local"]);
    /// ```
    pub fn new(application: impl Into<String>, profiles: Vec<impl Into<String>>) -> Self {
        Self {
            application: application.into(),
            profiles: profiles.into_iter().map(Into::into).collect(),
            label: None,
        }
    }

    /// Creates a new configuration query with a label.
    ///
    /// # Example
    ///
    /// ```
    /// use vortex_git::ConfigQuery;
    ///
    /// let query = ConfigQuery::with_label("myapp", vec!["prod"], "v1.0.0");
    /// assert_eq!(query.label(), Some("v1.0.0"));
    /// ```
    pub fn with_label(
        application: impl Into<String>,
        profiles: Vec<impl Into<String>>,
        label: impl Into<String>,
    ) -> Self {
        Self {
            application: application.into(),
            profiles: profiles.into_iter().map(Into::into).collect(),
            label: Some(label.into()),
        }
    }

    /// Returns the application name.
    pub fn application(&self) -> &str {
        &self.application
    }

    /// Returns the active profiles.
    pub fn profiles(&self) -> &[String] {
        &self.profiles
    }

    /// Returns the label (branch/tag/commit) if specified.
    pub fn label(&self) -> Option<&str> {
        self.label.as_deref()
    }

    /// Sets the label for this query.
    pub fn set_label(&mut self, label: impl Into<String>) {
        self.label = Some(label.into());
    }

    /// Returns a new query with the specified label.
    pub fn with_label_set(mut self, label: impl Into<String>) -> Self {
        self.label = Some(label.into());
        self
    }

    /// Returns the effective label, using the provided default if none is set.
    pub fn effective_label<'a>(&'a self, default: &'a str) -> &'a str {
        self.label.as_deref().unwrap_or(default)
    }
}

impl std::fmt::Display for ConfigQuery {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}/{}", self.application, self.profiles.join(","))?;
        if let Some(label) = &self.label {
            write!(f, "/{}", label)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_query() {
        let query = ConfigQuery::new("myapp", vec!["dev"]);
        assert_eq!(query.application(), "myapp");
        assert_eq!(query.profiles(), &["dev"]);
        assert_eq!(query.label(), None);
    }

    #[test]
    fn test_query_with_label() {
        let query = ConfigQuery::with_label("myapp", vec!["prod", "cloud"], "main");
        assert_eq!(query.application(), "myapp");
        assert_eq!(query.profiles(), &["prod", "cloud"]);
        assert_eq!(query.label(), Some("main"));
    }

    #[test]
    fn test_effective_label() {
        let query = ConfigQuery::new("myapp", vec!["dev"]);
        assert_eq!(query.effective_label("main"), "main");

        let query = ConfigQuery::with_label("myapp", vec!["dev"], "develop");
        assert_eq!(query.effective_label("main"), "develop");
    }

    #[test]
    fn test_display() {
        let query = ConfigQuery::new("myapp", vec!["dev", "local"]);
        assert_eq!(query.to_string(), "myapp/dev,local");

        let query = ConfigQuery::with_label("myapp", vec!["prod"], "v1.0.0");
        assert_eq!(query.to_string(), "myapp/prod/v1.0.0");
    }

    #[test]
    fn test_with_label_set() {
        let query = ConfigQuery::new("myapp", vec!["dev"]).with_label_set("feature/test");
        assert_eq!(query.label(), Some("feature/test"));
    }
}
