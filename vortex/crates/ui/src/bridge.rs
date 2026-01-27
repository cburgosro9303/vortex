//! UI Bridge Module
//!
//! Defines the communication protocol between the Slint UI thread
//! and the async Tokio runtime.

use std::path::PathBuf;

use vortex_domain::RequestState;

/// Commands sent from UI to the async runtime.
#[derive(Debug, Clone)]
pub enum UiCommand {
    /// User clicked Send button or pressed Enter.
    SendRequest,

    /// User clicked Cancel button.
    CancelRequest,

    /// User wants to create a new workspace.
    CreateWorkspace { path: PathBuf, name: String },

    /// User wants to open an existing workspace.
    OpenWorkspace { path: PathBuf },

    /// User wants to close the current workspace.
    CloseWorkspace,

    /// User wants to create a new request.
    CreateRequest {
        collection_path: PathBuf,
        name: String,
    },

    /// User wants to save the current collection.
    SaveCollection,

    /// User selected an item in the tree.
    ItemSelected { id: String, path: PathBuf },

    /// User double-clicked an item (load request into editor).
    ItemDoubleClicked { id: String, path: PathBuf },

    /// User toggled a folder expansion.
    ToggleFolder { id: String },
}

/// A tree item for UI display.
#[derive(Debug, Clone)]
pub struct TreeItemData {
    pub id: String,
    pub name: String,
    pub item_type: String,
    pub method: String,
    pub depth: i32,
    pub expanded: bool,
    pub path: String,
}

/// Updates sent from async runtime to the UI.
#[derive(Debug, Clone)]
pub enum UiUpdate {
    /// Update the request state (Idle/Loading/Success/Error).
    State(RequestState),

    /// Update the elapsed time display during loading.
    ElapsedTime(String),

    /// Update the workspace path.
    WorkspacePath(String),

    /// Update the collection tree items.
    CollectionItems(Vec<TreeItemData>),

    /// Show an error message.
    Error { title: String, message: String },

    /// Update saving state.
    SavingState(bool),

    /// Load a request into the editor.
    LoadRequest {
        url: String,
        method: i32,
        body: String,
    },
}
