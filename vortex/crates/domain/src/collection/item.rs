//! Collection item types

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::request::RequestSpec;

/// A folder containing requests and other folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Folder {
    /// Unique identifier
    pub id: Uuid,
    /// Folder name
    pub name: String,
    /// Items in this folder
    #[serde(default)]
    pub items: Vec<CollectionItem>,
}

impl Folder {
    /// Creates a new empty folder.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            id: Uuid::now_v7(),
            name: name.into(),
            items: Vec::new(),
        }
    }
}

/// An item in a collection (either a folder or a request).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CollectionItem {
    /// A folder containing other items
    Folder(Folder),
    /// A request specification
    Request(RequestSpec),
}

impl CollectionItem {
    /// Returns the ID of this item.
    #[must_use]
    pub const fn id(&self) -> Uuid {
        match self {
            Self::Folder(f) => f.id,
            Self::Request(r) => r.id,
        }
    }

    /// Returns the name of this item.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::Folder(f) => &f.name,
            Self::Request(r) => &r.name,
        }
    }
}

/// A collection of requests organized in folders.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Collection {
    /// Schema version for migration support
    pub schema: u32,
    /// Unique identifier
    pub id: Uuid,
    /// Collection name
    pub name: String,
    /// Optional description
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// Items in this collection
    #[serde(default)]
    pub items: Vec<CollectionItem>,
}

impl Collection {
    /// Current schema version.
    pub const SCHEMA_VERSION: u32 = 1;

    /// Creates a new empty collection.
    #[must_use]
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            schema: Self::SCHEMA_VERSION,
            id: Uuid::now_v7(),
            name: name.into(),
            description: None,
            items: Vec::new(),
        }
    }

    /// Adds an item to the collection root.
    pub fn add_item(&mut self, item: CollectionItem) {
        self.items.push(item);
    }

    /// Returns the total number of requests in the collection (recursive).
    #[must_use]
    pub fn request_count(&self) -> usize {
        fn count_in_items(items: &[CollectionItem]) -> usize {
            items.iter().fold(0, |acc, item| {
                acc + match item {
                    CollectionItem::Request(_) => 1,
                    CollectionItem::Folder(f) => count_in_items(&f.items),
                }
            })
        }
        count_in_items(&self.items)
    }
}

impl Default for Collection {
    fn default() -> Self {
        Self::new("New Collection")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_creation() {
        let collection = Collection::new("My API");
        assert_eq!(collection.name, "My API");
        assert_eq!(collection.schema, Collection::SCHEMA_VERSION);
        assert!(collection.items.is_empty());
    }

    #[test]
    fn test_request_count() {
        let mut collection = Collection::new("Test");

        // Add a request at root
        collection.add_item(CollectionItem::Request(RequestSpec::new("Request 1")));

        // Add a folder with requests
        let mut folder = Folder::new("Users");
        folder
            .items
            .push(CollectionItem::Request(RequestSpec::new("Get Users")));
        folder
            .items
            .push(CollectionItem::Request(RequestSpec::new("Create User")));
        collection.add_item(CollectionItem::Folder(folder));

        assert_eq!(collection.request_count(), 3);
    }
}
