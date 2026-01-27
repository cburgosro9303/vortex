//! Built-in dynamic variables
//!
//! These variables are prefixed with $ and generate new values on each resolution.

use chrono::Utc;
use rand::Rng;
use uuid::Uuid;

/// Information about a built-in variable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BuiltinInfo {
    /// Variable name (including $ prefix)
    pub name: &'static str,
    /// Human-readable description
    pub description: &'static str,
    /// Example output
    pub example: &'static str,
}

/// Generates values for built-in dynamic variables.
/// These variables are prefixed with $ and generate new values on each resolution.
pub struct BuiltinVariables;

impl BuiltinVariables {
    /// Resolves a built-in variable name to its value.
    /// Returns None if the name is not a recognized built-in.
    #[must_use]
    pub fn resolve(name: &str) -> Option<String> {
        match name {
            "$uuid" | "$randomUuid" => Some(Self::generate_uuid()),
            "$timestamp" => Some(Self::generate_timestamp()),
            "$isoTimestamp" => Some(Self::generate_iso_timestamp()),
            "$randomInt" => Some(Self::generate_random_int()),
            "$randomString" => Some(Self::generate_random_string()),
            "$randomEmail" => Some(Self::generate_random_email()),
            "$randomFirstName" => Some(Self::generate_random_first_name()),
            "$randomLastName" => Some(Self::generate_random_last_name()),
            "$randomAlphanumeric" => Some(Self::generate_random_alphanumeric()),
            "$randomBoolean" => Some(Self::generate_random_boolean()),
            "$date" => Some(Self::generate_date()),
            "$dateISO" => Some(Self::generate_date_iso()),
            _ => None,
        }
    }

    /// Returns whether the name is a valid built-in variable.
    #[must_use]
    pub fn is_builtin(name: &str) -> bool {
        Self::resolve(name).is_some() || Self::available().iter().any(|b| b.name == name)
    }

    /// Returns a list of all available built-in variable names with descriptions.
    #[must_use]
    pub fn available() -> Vec<BuiltinInfo> {
        vec![
            BuiltinInfo {
                name: "$uuid",
                description: "Random UUID v4",
                example: "550e8400-e29b-41d4-a716-446655440000",
            },
            BuiltinInfo {
                name: "$timestamp",
                description: "Unix timestamp in seconds",
                example: "1706284800",
            },
            BuiltinInfo {
                name: "$isoTimestamp",
                description: "ISO 8601 timestamp (UTC)",
                example: "2024-01-26T12:00:00+00:00",
            },
            BuiltinInfo {
                name: "$randomInt",
                description: "Random integer 0-1000",
                example: "427",
            },
            BuiltinInfo {
                name: "$randomString",
                description: "Random alphanumeric string (16 chars)",
                example: "aB3dE5fG7hI9jK1m",
            },
            BuiltinInfo {
                name: "$randomEmail",
                description: "Random email address",
                example: "abc12def@example.com",
            },
            BuiltinInfo {
                name: "$randomFirstName",
                description: "Random first name",
                example: "John",
            },
            BuiltinInfo {
                name: "$randomLastName",
                description: "Random last name",
                example: "Smith",
            },
            BuiltinInfo {
                name: "$randomAlphanumeric",
                description: "Random alphanumeric string (8 chars)",
                example: "aB3dE5fG",
            },
            BuiltinInfo {
                name: "$randomBoolean",
                description: "Random boolean (true/false)",
                example: "true",
            },
            BuiltinInfo {
                name: "$date",
                description: "Current date (YYYY-MM-DD)",
                example: "2024-01-26",
            },
            BuiltinInfo {
                name: "$dateISO",
                description: "Current date in ISO 8601 format",
                example: "2024-01-26T00:00:00Z",
            },
        ]
    }

    /// Generates a random UUID v4.
    fn generate_uuid() -> String {
        Uuid::new_v4().to_string()
    }

    /// Generates current Unix timestamp in seconds.
    fn generate_timestamp() -> String {
        Utc::now().timestamp().to_string()
    }

    /// Generates current timestamp in ISO 8601 format.
    fn generate_iso_timestamp() -> String {
        Utc::now().to_rfc3339()
    }

    /// Generates a random integer between 0 and 1000.
    fn generate_random_int() -> String {
        let mut rng = rand::rng();
        rng.random_range(0..=1000).to_string()
    }

    /// Generates a random 16-character alphanumeric string.
    fn generate_random_string() -> String {
        Self::random_alphanumeric_string(16)
    }

    /// Generates a random 8-character alphanumeric string.
    fn generate_random_alphanumeric() -> String {
        Self::random_alphanumeric_string(8)
    }

    /// Helper to generate random alphanumeric string of given length.
    fn random_alphanumeric_string(len: usize) -> String {
        const CHARSET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
        let mut rng = rand::rng();
        (0..len)
            .map(|_| {
                let idx = rng.random_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }

    /// Generates a random email address.
    fn generate_random_email() -> String {
        let random_part = Self::random_alphanumeric_string(8).to_lowercase();
        format!("{random_part}@example.com")
    }

    /// Generates a random first name.
    fn generate_random_first_name() -> String {
        const FIRST_NAMES: &[&str] = &[
            "James", "Mary", "John", "Patricia", "Robert", "Jennifer", "Michael", "Linda",
            "William", "Elizabeth", "David", "Barbara", "Richard", "Susan", "Joseph", "Jessica",
            "Thomas", "Sarah", "Charles", "Karen", "Emma", "Olivia", "Liam", "Noah", "Ava",
        ];
        let mut rng = rand::rng();
        let idx = rng.random_range(0..FIRST_NAMES.len());
        FIRST_NAMES[idx].to_string()
    }

    /// Generates a random last name.
    fn generate_random_last_name() -> String {
        const LAST_NAMES: &[&str] = &[
            "Smith", "Johnson", "Williams", "Brown", "Jones", "Garcia", "Miller", "Davis",
            "Rodriguez", "Martinez", "Hernandez", "Lopez", "Gonzalez", "Wilson", "Anderson",
            "Thomas", "Taylor", "Moore", "Jackson", "Martin", "Lee", "Perez", "Thompson",
        ];
        let mut rng = rand::rng();
        let idx = rng.random_range(0..LAST_NAMES.len());
        LAST_NAMES[idx].to_string()
    }

    /// Generates a random boolean string.
    fn generate_random_boolean() -> String {
        let mut rng = rand::rng();
        if rng.random_bool(0.5) {
            "true".to_string()
        } else {
            "false".to_string()
        }
    }

    /// Generates current date in YYYY-MM-DD format.
    fn generate_date() -> String {
        Utc::now().format("%Y-%m-%d").to_string()
    }

    /// Generates current date in ISO 8601 format.
    fn generate_date_iso() -> String {
        Utc::now().format("%Y-%m-%dT00:00:00Z").to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_uuid_generation() {
        let uuid = BuiltinVariables::resolve("$uuid").expect("Should resolve $uuid");
        assert!(Uuid::parse_str(&uuid).is_ok());
    }

    #[test]
    fn test_random_uuid_alias() {
        let uuid = BuiltinVariables::resolve("$randomUuid").expect("Should resolve $randomUuid");
        assert!(Uuid::parse_str(&uuid).is_ok());
    }

    #[test]
    fn test_timestamp_generation() {
        let ts = BuiltinVariables::resolve("$timestamp").expect("Should resolve $timestamp");
        let parsed: i64 = ts.parse().expect("Should be valid integer");
        assert!(parsed > 0);
    }

    #[test]
    fn test_iso_timestamp_generation() {
        let ts = BuiltinVariables::resolve("$isoTimestamp").expect("Should resolve $isoTimestamp");
        // Should contain T and timezone info
        assert!(ts.contains('T'));
        assert!(ts.contains('+') || ts.contains('Z'));
    }

    #[test]
    fn test_random_int_generation() {
        let int_str =
            BuiltinVariables::resolve("$randomInt").expect("Should resolve $randomInt");
        let int_val: i32 = int_str.parse().expect("Should be valid integer");
        assert!((0..=1000).contains(&int_val));
    }

    #[test]
    fn test_random_string_generation() {
        let s =
            BuiltinVariables::resolve("$randomString").expect("Should resolve $randomString");
        assert_eq!(s.len(), 16);
        assert!(s.chars().all(|c| c.is_alphanumeric()));
    }

    #[test]
    fn test_random_email_generation() {
        let email =
            BuiltinVariables::resolve("$randomEmail").expect("Should resolve $randomEmail");
        assert!(email.contains('@'));
        assert!(email.ends_with("@example.com"));
    }

    #[test]
    fn test_random_first_name() {
        let name = BuiltinVariables::resolve("$randomFirstName")
            .expect("Should resolve $randomFirstName");
        assert!(!name.is_empty());
        assert!(name.chars().all(|c| c.is_alphabetic()));
    }

    #[test]
    fn test_random_last_name() {
        let name =
            BuiltinVariables::resolve("$randomLastName").expect("Should resolve $randomLastName");
        assert!(!name.is_empty());
        assert!(name.chars().all(|c| c.is_alphabetic()));
    }

    #[test]
    fn test_random_boolean() {
        let bool_str =
            BuiltinVariables::resolve("$randomBoolean").expect("Should resolve $randomBoolean");
        assert!(bool_str == "true" || bool_str == "false");
    }

    #[test]
    fn test_date_generation() {
        let date = BuiltinVariables::resolve("$date").expect("Should resolve $date");
        // Should be in YYYY-MM-DD format
        assert_eq!(date.len(), 10);
        assert!(date.contains('-'));
    }

    #[test]
    fn test_unknown_builtin() {
        assert!(BuiltinVariables::resolve("$unknown").is_none());
    }

    #[test]
    fn test_non_builtin() {
        assert!(BuiltinVariables::resolve("regular_var").is_none());
    }

    #[test]
    fn test_is_builtin() {
        assert!(BuiltinVariables::is_builtin("$uuid"));
        assert!(BuiltinVariables::is_builtin("$timestamp"));
        assert!(!BuiltinVariables::is_builtin("regular_var"));
        assert!(!BuiltinVariables::is_builtin("$nonexistent"));
    }

    #[test]
    fn test_available_list() {
        let available = BuiltinVariables::available();
        assert!(!available.is_empty());
        assert!(available.iter().any(|b| b.name == "$uuid"));
        assert!(available.iter().any(|b| b.name == "$timestamp"));
    }

    #[test]
    fn test_uniqueness() {
        // Generate multiple UUIDs and ensure they're different
        let uuid1 = BuiltinVariables::resolve("$uuid").expect("Should resolve");
        let uuid2 = BuiltinVariables::resolve("$uuid").expect("Should resolve");
        // Note: While theoretically possible to get the same UUID, it's astronomically unlikely
        assert_ne!(uuid1, uuid2);
    }
}
