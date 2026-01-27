//! UI state for collection management.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;

use vortex_application::ports::{slugify, CollectionTree, FolderTree};
use vortex_domain::persistence::{PersistenceCollection, PersistenceFolder, SavedRequest};

/// Represents a node in the UI tree view.
#[derive(Debug, Clone)]
pub struct TreeNode {
    /// Unique identifier.
    pub id: String,
    /// Display name.
    pub name: String,
    /// Type of node.
    pub node_type: TreeNodeType,
    /// Nesting depth for indentation.
    pub depth: u32,
    /// Whether the node is expanded (for folders/collections).
    pub expanded: bool,
    /// File path.
    pub path: PathBuf,
}

/// Type of tree node.
#[derive(Debug, Clone, PartialEq)]
pub enum TreeNodeType {
    /// A collection.
    Collection,
    /// A folder.
    Folder,
    /// A request with its HTTP method.
    Request {
        /// HTTP method name.
        method: String,
    },
}

/// State for the collection sidebar.
#[derive(Debug, Default)]
pub struct CollectionState {
    /// Currently loaded workspace path.
    pub workspace_path: Option<PathBuf>,
    /// Loaded collections indexed by path.
    pub collections: HashMap<PathBuf, CollectionData>,
    /// Currently selected item ID.
    pub selected_id: Option<String>,
    /// Expanded folder IDs.
    pub expanded_ids: HashSet<String>,
    /// Items with unsaved changes.
    pub dirty_ids: HashSet<String>,
}

/// Data for a loaded collection.
#[derive(Debug, Clone)]
pub struct CollectionData {
    /// The collection metadata.
    pub collection: PersistenceCollection,
    /// Requests at the collection root.
    pub requests: Vec<SavedRequest>,
    /// Folders in the collection.
    pub folders: Vec<FolderData>,
    /// Path to the collection directory.
    pub path: PathBuf,
}

/// Data for a folder.
#[derive(Debug, Clone)]
pub struct FolderData {
    /// The folder metadata.
    pub folder: PersistenceFolder,
    /// Requests in this folder.
    pub requests: Vec<SavedRequest>,
    /// Nested subfolders.
    pub subfolders: Vec<FolderData>,
    /// Path to the folder.
    pub path: PathBuf,
}

impl CollectionState {
    /// Creates a new empty state.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Loads a collection tree into the state.
    pub fn load_collection(&mut self, path: PathBuf, tree: CollectionTree) {
        let collection_data = CollectionData {
            collection: tree.collection.clone(),
            requests: tree.requests,
            folders: tree
                .folders
                .into_iter()
                .map(|f| Self::folder_tree_to_data(f, &path))
                .collect(),
            path: path.clone(),
        };

        // Auto-expand the collection
        self.expanded_ids.insert(tree.collection.id.clone());

        self.collections.insert(path, collection_data);
    }

    fn folder_tree_to_data(tree: FolderTree, base_path: &PathBuf) -> FolderData {
        let folder_path = base_path.join("requests").join(&tree.path);
        FolderData {
            folder: tree.folder,
            requests: tree.requests,
            subfolders: tree
                .subfolders
                .into_iter()
                .map(|f| Self::folder_tree_to_data(f, base_path))
                .collect(),
            path: folder_path,
        }
    }

    /// Flattens the collection tree into a list for UI rendering.
    #[must_use]
    pub fn flatten_tree(&self) -> Vec<TreeNode> {
        let mut nodes = Vec::new();

        for (path, data) in &self.collections {
            let collection_id = data.collection.id.clone();
            let expanded = self.expanded_ids.contains(&collection_id);

            nodes.push(TreeNode {
                id: collection_id.clone(),
                name: data.collection.name.clone(),
                node_type: TreeNodeType::Collection,
                depth: 0,
                expanded,
                path: path.clone(),
            });

            if expanded {
                // Add root-level requests
                for request in &data.requests {
                    nodes.push(TreeNode {
                        id: request.id.clone(),
                        name: request.name.clone(),
                        node_type: TreeNodeType::Request {
                            method: request.method.to_string(),
                        },
                        depth: 1,
                        expanded: false,
                        path: path
                            .join("requests")
                            .join(format!("{}.json", slugify(&request.name))),
                    });
                }

                // Add folders recursively
                self.flatten_folders(&data.folders, path, 1, &mut nodes);
            }
        }

        nodes
    }

    fn flatten_folders(
        &self,
        folders: &[FolderData],
        _base_path: &PathBuf,
        depth: u32,
        nodes: &mut Vec<TreeNode>,
    ) {
        for folder_data in folders {
            let folder_id = folder_data.folder.id.clone();
            let expanded = self.expanded_ids.contains(&folder_id);

            nodes.push(TreeNode {
                id: folder_id.clone(),
                name: folder_data.folder.name.clone(),
                node_type: TreeNodeType::Folder,
                depth,
                expanded,
                path: folder_data.path.clone(),
            });

            if expanded {
                // Add folder's requests
                for request in &folder_data.requests {
                    nodes.push(TreeNode {
                        id: request.id.clone(),
                        name: request.name.clone(),
                        node_type: TreeNodeType::Request {
                            method: request.method.to_string(),
                        },
                        depth: depth + 1,
                        expanded: false,
                        path: folder_data
                            .path
                            .join(format!("{}.json", slugify(&request.name))),
                    });
                }

                // Recurse into subfolders
                self.flatten_folders(&folder_data.subfolders, &folder_data.path, depth + 1, nodes);
            }
        }
    }

    /// Toggles the expanded state of a folder or collection.
    pub fn toggle_expanded(&mut self, id: &str) {
        if self.expanded_ids.contains(id) {
            self.expanded_ids.remove(id);
        } else {
            self.expanded_ids.insert(id.to_string());
        }
    }

    /// Marks an item as having unsaved changes.
    pub fn mark_dirty(&mut self, id: &str) {
        self.dirty_ids.insert(id.to_string());
    }

    /// Clears the dirty flag for an item.
    pub fn mark_clean(&mut self, id: &str) {
        self.dirty_ids.remove(id);
    }

    /// Returns whether any items have unsaved changes.
    #[must_use]
    pub fn has_unsaved_changes(&self) -> bool {
        !self.dirty_ids.is_empty()
    }

    /// Clears all collections and resets state.
    pub fn clear(&mut self) {
        self.collections.clear();
        self.selected_id = None;
        self.expanded_ids.clear();
        self.dirty_ids.clear();
    }

    /// Finds a request by its ID across all collections.
    #[must_use]
    pub fn find_request(&self, id: &str) -> Option<(&SavedRequest, PathBuf)> {
        for (_path, data) in &self.collections {
            // Check root requests
            for request in &data.requests {
                if request.id == id {
                    let request_path = data
                        .path
                        .join("requests")
                        .join(format!("{}.json", slugify(&request.name)));
                    return Some((request, request_path));
                }
            }

            // Check folder requests recursively
            if let Some(result) = Self::find_request_in_folders(&data.folders, id) {
                return Some(result);
            }
        }
        None
    }

    fn find_request_in_folders<'a>(
        folders: &'a [FolderData],
        id: &str,
    ) -> Option<(&'a SavedRequest, PathBuf)> {
        for folder in folders {
            for request in &folder.requests {
                if request.id == id {
                    let request_path = folder
                        .path
                        .join(format!("{}.json", slugify(&request.name)));
                    return Some((request, request_path));
                }
            }

            if let Some(result) = Self::find_request_in_folders(&folder.subfolders, id) {
                return Some(result);
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collection_state_new() {
        let state = CollectionState::new();
        assert!(state.workspace_path.is_none());
        assert!(state.collections.is_empty());
    }

    #[test]
    fn test_toggle_expanded() {
        let mut state = CollectionState::new();
        let id = "test-id";

        assert!(!state.expanded_ids.contains(id));

        state.toggle_expanded(id);
        assert!(state.expanded_ids.contains(id));

        state.toggle_expanded(id);
        assert!(!state.expanded_ids.contains(id));
    }

    #[test]
    fn test_dirty_tracking() {
        let mut state = CollectionState::new();
        let id = "test-id";

        assert!(!state.has_unsaved_changes());

        state.mark_dirty(id);
        assert!(state.has_unsaved_changes());

        state.mark_clean(id);
        assert!(!state.has_unsaved_changes());
    }
}
