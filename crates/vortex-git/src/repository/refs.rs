//! Git reference types.

use std::fmt;

/// A Git reference (branch, tag, or commit).
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum GitRef {
    /// A branch reference (e.g., "main", "develop").
    Branch(String),

    /// A tag reference (e.g., "v1.0.0").
    Tag(String),

    /// A commit SHA (full or abbreviated).
    Commit(String),
}

impl GitRef {
    /// Creates a branch reference.
    pub fn branch(name: impl Into<String>) -> Self {
        Self::Branch(name.into())
    }

    /// Creates a tag reference.
    pub fn tag(name: impl Into<String>) -> Self {
        Self::Tag(name.into())
    }

    /// Creates a commit reference.
    pub fn commit(sha: impl Into<String>) -> Self {
        Self::Commit(sha.into())
    }

    /// Parses a label string into a GitRef.
    ///
    /// This attempts to determine the type based on the format:
    /// - 40-character hex string → Commit
    /// - Starts with "refs/tags/" or "tags/" → Tag
    /// - Otherwise → Branch (default)
    pub fn parse(label: &str) -> Self {
        let label = label.trim();

        // Check for commit SHA (40 hex chars)
        if label.len() == 40 && label.chars().all(|c| c.is_ascii_hexdigit()) {
            return Self::Commit(label.to_string());
        }

        // Check for tag reference
        if let Some(tag_name) = label.strip_prefix("refs/tags/") {
            return Self::Tag(tag_name.to_string());
        }
        if let Some(tag_name) = label.strip_prefix("tags/") {
            return Self::Tag(tag_name.to_string());
        }

        // Check for explicit branch reference
        if let Some(branch_name) = label.strip_prefix("refs/heads/") {
            return Self::Branch(branch_name.to_string());
        }

        // Default to branch
        Self::Branch(label.to_string())
    }

    /// Returns the reference name without prefix.
    pub fn name(&self) -> &str {
        match self {
            Self::Branch(name) | Self::Tag(name) | Self::Commit(name) => name,
        }
    }

    /// Returns the full Git reference path.
    pub fn full_ref(&self) -> String {
        match self {
            Self::Branch(name) => format!("refs/heads/{}", name),
            Self::Tag(name) => format!("refs/tags/{}", name),
            Self::Commit(sha) => sha.clone(),
        }
    }

    /// Returns true if this is a branch reference.
    pub fn is_branch(&self) -> bool {
        matches!(self, Self::Branch(_))
    }

    /// Returns true if this is a tag reference.
    pub fn is_tag(&self) -> bool {
        matches!(self, Self::Tag(_))
    }

    /// Returns true if this is a commit reference.
    pub fn is_commit(&self) -> bool {
        matches!(self, Self::Commit(_))
    }

    /// Validates the reference name.
    ///
    /// Returns an error message if the name is invalid.
    pub fn validate(&self) -> Result<(), &'static str> {
        let name = self.name();

        if name.is_empty() {
            return Err("reference name cannot be empty");
        }

        if name.starts_with('/') || name.ends_with('/') {
            return Err("reference name cannot start or end with '/'");
        }

        if name.contains("..") {
            return Err("reference name cannot contain '..'");
        }

        if name.contains("//") {
            return Err("reference name cannot contain '//'");
        }

        // Check for invalid characters
        for c in name.chars() {
            if c.is_control()
                || c == ' '
                || c == '~'
                || c == '^'
                || c == ':'
                || c == '?'
                || c == '*'
                || c == '['
            {
                return Err("reference name contains invalid characters");
            }
        }

        Ok(())
    }
}

impl fmt::Display for GitRef {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Branch(name) => write!(f, "{}", name),
            Self::Tag(name) => write!(f, "tags/{}", name),
            Self::Commit(sha) => write!(f, "{}", &sha[..8.min(sha.len())]),
        }
    }
}

impl From<&str> for GitRef {
    fn from(s: &str) -> Self {
        Self::parse(s)
    }
}

impl From<String> for GitRef {
    fn from(s: String) -> Self {
        Self::parse(&s)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_branch() {
        assert_eq!(GitRef::parse("main"), GitRef::Branch("main".to_string()));
        assert_eq!(
            GitRef::parse("feature/test"),
            GitRef::Branch("feature/test".to_string())
        );
        assert_eq!(
            GitRef::parse("refs/heads/develop"),
            GitRef::Branch("develop".to_string())
        );
    }

    #[test]
    fn test_parse_tag() {
        assert_eq!(
            GitRef::parse("refs/tags/v1.0.0"),
            GitRef::Tag("v1.0.0".to_string())
        );
        assert_eq!(
            GitRef::parse("tags/v2.0.0"),
            GitRef::Tag("v2.0.0".to_string())
        );
    }

    #[test]
    fn test_parse_commit() {
        let sha = "a1b2c3d4e5f6789012345678901234567890abcd";
        assert_eq!(GitRef::parse(sha), GitRef::Commit(sha.to_string()));
    }

    #[test]
    fn test_full_ref() {
        assert_eq!(GitRef::branch("main").full_ref(), "refs/heads/main");
        assert_eq!(GitRef::tag("v1.0.0").full_ref(), "refs/tags/v1.0.0");

        let sha = "a1b2c3d4";
        assert_eq!(GitRef::commit(sha).full_ref(), sha);
    }

    #[test]
    fn test_display() {
        assert_eq!(GitRef::branch("main").to_string(), "main");
        assert_eq!(GitRef::tag("v1.0.0").to_string(), "tags/v1.0.0");
        assert_eq!(GitRef::commit("a1b2c3d4e5f6").to_string(), "a1b2c3d4");
    }

    #[test]
    fn test_validate() {
        assert!(GitRef::branch("main").validate().is_ok());
        assert!(GitRef::branch("feature/test").validate().is_ok());
        assert!(GitRef::tag("v1.0.0").validate().is_ok());

        assert!(GitRef::branch("").validate().is_err());
        assert!(GitRef::branch("/main").validate().is_err());
        assert!(GitRef::branch("main/").validate().is_err());
        assert!(GitRef::branch("main..branch").validate().is_err());
        assert!(GitRef::branch("main branch").validate().is_err());
    }

    #[test]
    fn test_is_methods() {
        let branch = GitRef::branch("main");
        assert!(branch.is_branch());
        assert!(!branch.is_tag());
        assert!(!branch.is_commit());

        let tag = GitRef::tag("v1.0.0");
        assert!(!tag.is_branch());
        assert!(tag.is_tag());
        assert!(!tag.is_commit());

        let commit = GitRef::commit("abc123");
        assert!(!commit.is_branch());
        assert!(!commit.is_tag());
        assert!(commit.is_commit());
    }
}
