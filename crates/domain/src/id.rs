//! ID generation utilities.

use uuid::Uuid;

/// Generates a new UUID v4 as a string.
///
/// This is the standard ID format for all Vortex entities.
#[must_use]
pub fn generate_id() -> String {
    Uuid::now_v7().to_string()
}

/// Generates a new UUID v7 as a string.
///
/// UUID v7 includes timestamp information and is sortable.
#[must_use]
pub fn generate_id_v7() -> String {
    Uuid::now_v7().to_string()
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_id_format() {
        let id = generate_id();
        // UUID v4 format: 8-4-4-4-12 = 36 chars
        assert_eq!(id.len(), 36);
        assert!(Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn test_generate_id_uniqueness() {
        let id1 = generate_id();
        let id2 = generate_id();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_generate_id_v7_format() {
        let id = generate_id_v7();
        assert_eq!(id.len(), 36);
        assert!(Uuid::parse_str(&id).is_ok());
    }

    #[test]
    fn test_generate_id_v7_uniqueness() {
        let id1 = generate_id_v7();
        let id2 = generate_id_v7();
        assert_ne!(id1, id2);
    }
}
