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

    // --- Environment Commands (Sprint 03) ---
    /// User changed the selected environment.
    EnvironmentChanged { index: i32 },

    /// User clicked manage environments button.
    ManageEnvironments,

    /// User wants to create a new environment.
    CreateEnvironment { name: String },

    /// User wants to delete an environment.
    DeleteEnvironment { index: i32 },

    /// User selected an environment for editing.
    SelectEnvironmentForEditing { index: i32 },

    /// User wants to save environment changes.
    SaveEnvironment,

    /// User added a variable to the environment.
    AddEnvironmentVariable,

    /// User deleted a variable from the environment.
    DeleteEnvironmentVariable { index: i32 },

    /// User changed a variable value.
    EnvironmentVariableChanged {
        index: i32,
        name: String,
        value: String,
        enabled: bool,
        is_secret: bool,
    },

    /// User changed the URL (for variable preview).
    UrlChanged { url: String },

    // --- Settings Commands (Sprint 04) ---
    /// User toggled the theme (light/dark).
    ToggleTheme,

    /// User changed theme mode (0=Light, 1=Dark, 2=System).
    SetThemeMode { index: i32 },

    /// User changed font scale (0=Small, 1=Medium, 2=Large).
    SetFontScale { index: i32 },

    /// User wants to open settings dialog.
    OpenSettings,

    /// User wants to close settings dialog.
    CloseSettings,

    // --- History Commands (Sprint 04) ---
    /// User clicked a history item to reload it.
    LoadHistoryItem { id: String },

    /// User wants to clear history.
    ClearHistory,

    /// User toggled history visibility.
    ToggleHistoryVisibility,

    // --- Sprint 05: Query Parameters Commands ---
    /// Add a new query parameter.
    AddQueryParam,

    /// Delete a query parameter.
    DeleteQueryParam { index: i32 },

    /// Query parameter changed.
    QueryParamChanged {
        index: i32,
        key: String,
        value: String,
        description: String,
        enabled: bool,
    },

    // --- Sprint 05: Request Headers Commands ---
    /// Add a new request header.
    AddRequestHeader,

    /// Delete a request header.
    DeleteRequestHeader { index: i32 },

    /// Request header changed.
    RequestHeaderChanged {
        index: i32,
        key: String,
        value: String,
        description: String,
        enabled: bool,
    },

    // --- Sprint 05: Authentication Commands ---
    /// Auth type changed.
    AuthTypeChanged { auth_type: i32 },

    /// Bearer token changed.
    BearerTokenChanged { token: String },

    /// Basic auth credentials changed.
    BasicCredentialsChanged { username: String, password: String },

    /// API Key changed.
    ApiKeyChanged {
        key_name: String,
        key_value: String,
        location: i32,
    },

    // --- Sprint 05: Collection Management Commands ---
    /// Save the current request to a collection.
    SaveCurrentRequest,

    /// Rename an item (request or collection).
    RenameItem { id: String, new_name: String },

    /// Request to delete an item (shows confirmation).
    DeleteItemRequested { id: String, item_type: String },

    /// Confirm deletion of pending item.
    ConfirmDelete,

    /// Cancel pending deletion.
    CancelDelete,

    // --- Sprint 06: Tab Commands ---
    /// User clicked on a tab.
    TabClicked { id: String },

    /// User clicked close on a tab.
    TabCloseClicked { id: String },

    /// User clicked new tab button.
    NewTabClicked,

    // --- Sprint 06: Quick Search Commands ---
    /// Open quick search dialog.
    OpenQuickSearch,

    /// Close quick search dialog.
    CloseQuickSearch,

    /// Search query changed.
    SearchQueryChanged { query: String },

    /// User clicked on a search result.
    SearchResultClicked { id: String, path: String },

    // --- Sprint 06: Import/Export Commands ---
    /// Import a Postman collection (legacy - opens dialog).
    ImportCollection,

    /// Export the current collection.
    ExportCollection,

    // --- Sprint 04: Import Dialog Commands ---
    /// Browse for file to import.
    ImportBrowseFile,

    /// Start the import process with the selected file.
    ImportStart { file_path: String },

    /// Cancel the import process.
    ImportCancel,

    /// Export current request as cURL command.
    ExportAsCurl,

    // --- Sprint 06: JSON Format Commands ---
    /// Format the response body.
    FormatResponseBody,

    /// Format the request body.
    FormatRequestBody { body: String },

    /// Copy the formatted response body.
    CopyFormattedResponse,

    /// Refresh the collection tree.
    RefreshTree,

    /// Refresh the environments list.
    RefreshEnvironments,

    /// Import a Postman environment.
    ImportEnvironment,
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

/// Environment variable data for UI.
#[derive(Debug, Clone)]
pub struct VariableData {
    pub name: String,
    pub value: String,
    pub enabled: bool,
    pub is_secret: bool,
}

/// Environment info for the list.
#[derive(Debug, Clone)]
pub struct EnvironmentData {
    pub id: String,
    pub name: String,
    pub variable_count: i32,
}

/// History item for UI display.
#[derive(Debug, Clone)]
pub struct HistoryItemData {
    pub id: String,
    pub method: String,
    pub url: String,
    pub status_code: i32,
    pub time_ago: String,
    pub duration: String,
}

/// Query parameter data for UI (Sprint 05).
#[derive(Debug, Clone)]
pub struct QueryParamData {
    pub key: String,
    pub value: String,
    pub description: String,
    pub enabled: bool,
}

/// Request header data for UI (Sprint 05).
#[derive(Debug, Clone)]
pub struct HeaderData {
    pub key: String,
    pub value: String,
    pub description: String,
    pub enabled: bool,
}

/// Response header data for UI (Sprint 05).
#[derive(Debug, Clone)]
pub struct ResponseHeaderData {
    pub name: String,
    pub value: String,
}

/// Authentication data for UI (Sprint 05).
#[derive(Debug, Clone)]
pub struct AuthData {
    pub auth_type: i32,
    pub bearer_token: String,
    pub basic_username: String,
    pub basic_password: String,
    pub api_key_name: String,
    pub api_key_value: String,
    pub api_key_location: i32,
}

impl Default for AuthData {
    fn default() -> Self {
        Self {
            auth_type: 0,
            bearer_token: String::new(),
            basic_username: String::new(),
            basic_password: String::new(),
            api_key_name: String::new(),
            api_key_value: String::new(),
            api_key_location: 0,
        }
    }
}

/// Tab data for UI (Sprint 06).
#[derive(Debug, Clone)]
pub struct TabData {
    pub id: String,
    pub name: String,
    pub method: String,
    pub has_unsaved_changes: bool,
}

/// Tab state containing full request data for restoration.
#[derive(Debug, Clone)]
pub struct TabState {
    pub id: String,
    pub name: String,
    pub method: i32,
    pub url: String,
    pub body: String,
    pub headers: Vec<HeaderData>,
    pub query_params: Vec<QueryParamData>,
    pub auth: AuthData,
    pub has_unsaved_changes: bool,
    pub file_path: Option<String>,
    // Response state
    pub response_state: i32, // 0=Idle, 1=Loading, 2=Success, 3=Error
    pub response_body: String,
    pub status_code: i32,
    pub status_text: String,
    pub duration: String,
    pub size: String,
    pub response_headers: Vec<ResponseHeaderData>,
    pub error_title: String,
    pub error_message: String,
}

impl TabState {
    /// Creates a new empty tab state.
    pub fn new_empty() -> Self {
        Self {
            id: uuid::Uuid::now_v7().to_string(),
            name: "New Request".to_string(),
            method: 0,
            url: String::new(),
            body: String::new(),
            headers: Vec::new(),
            query_params: Vec::new(),
            auth: AuthData::default(),
            has_unsaved_changes: false,
            file_path: None,
            // Response defaults
            response_state: 0,
            response_body: String::new(),
            status_code: 0,
            status_text: String::new(),
            duration: String::new(),
            size: String::new(),
            response_headers: Vec::new(),
            error_title: String::new(),
            error_message: String::new(),
        }
    }

    /// Converts to TabData for UI display.
    pub fn to_tab_data(&self) -> TabData {
        let method_str = match self.method {
            0 => "GET",
            1 => "POST",
            2 => "PUT",
            3 => "PATCH",
            4 => "DELETE",
            5 => "HEAD",
            6 => "OPTIONS",
            _ => "GET",
        };
        TabData {
            id: self.id.clone(),
            name: self.name.clone(),
            method: method_str.to_string(),
            has_unsaved_changes: self.has_unsaved_changes,
        }
    }
}

/// Search result data for UI (Sprint 06).
#[derive(Debug, Clone)]
pub struct SearchResultData {
    pub id: String,
    pub name: String,
    pub method: String,
    pub url: String,
    pub collection_name: String,
    pub path: String,
}

/// Import warning data for UI (Sprint 04).
#[derive(Debug, Clone)]
pub struct ImportWarningData {
    pub path: String,
    pub message: String,
    pub severity: String,
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

    // --- Environment Updates (Sprint 03) ---
    /// Update the list of environment names.
    EnvironmentNames(Vec<String>),

    /// Update the current environment index.
    CurrentEnvironmentIndex(i32),

    /// Update the resolved URL preview.
    ResolvedUrl {
        resolved: String,
        has_unresolved: bool,
        unresolved_names: Vec<String>,
    },

    /// Update the environment list for the manager.
    EnvironmentList(Vec<EnvironmentData>),

    /// Update the selected environment for editing.
    SelectedEnvironment {
        index: i32,
        name: String,
        variables: Vec<VariableData>,
    },

    /// Toggle the environment manager dialog.
    ShowEnvironmentManager(bool),

    // --- Settings Updates (Sprint 04) ---
    /// Update theme mode (true = dark mode).
    ThemeMode(bool),

    /// Update font scale factor.
    FontScale(f32),

    /// Toggle the settings dialog.
    ShowSettings(bool),

    /// Settings loaded from disk.
    SettingsLoaded {
        theme_index: i32,
        font_scale_index: i32,
        dark_mode: bool,
        font_scale_factor: f32,
    },

    // --- History Updates (Sprint 04) ---
    /// Update the history items list.
    HistoryItems(Vec<HistoryItemData>),

    /// Toggle history panel visibility.
    HistoryVisible(bool),

    // --- Sprint 05: URL Update (sync from params) ---
    /// Update the URL in the UI (when params change).
    UpdateUrl(String),

    // --- Sprint 05: Query Parameters Updates ---
    /// Update query parameters list.
    QueryParams(Vec<QueryParamData>),

    // --- Sprint 05: Request Headers Updates ---
    /// Update request headers list.
    RequestHeaders(Vec<HeaderData>),

    // --- Sprint 05: Response Headers Updates ---
    /// Update response headers list.
    ResponseHeaders(Vec<ResponseHeaderData>),

    // --- Sprint 05: Authentication Updates ---
    /// Update authentication data.
    AuthData(AuthData),

    // --- Sprint 05: Collection Management Updates ---
    /// Show confirmation dialog for deletion.
    ShowConfirmDialog {
        title: String,
        message: String,
        item_id: String,
        item_type: String,
    },

    /// Hide confirmation dialog.
    HideConfirmDialog,

    /// Load request with full data (includes headers, params, auth).
    LoadFullRequest {
        url: String,
        method: i32,
        body: String,
        headers: Vec<HeaderData>,
        query_params: Vec<QueryParamData>,
        auth: AuthData,
    },

    // --- Sprint 06: Tab Updates ---
    /// Update the list of open tabs.
    TabsUpdated(Vec<TabData>),

    /// Update the active tab ID.
    ActiveTabChanged(String),

    // --- Sprint 06: Quick Search Updates ---
    /// Update search results.
    SearchResults(Vec<SearchResultData>),

    /// Show/hide quick search dialog.
    ShowQuickSearch(bool),

    // --- Sprint 06: Import/Export Updates ---
    /// Collection import completed successfully (legacy).
    ImportComplete { collection_name: String },

    /// Collection export completed successfully.
    ExportComplete { path: String },

    // --- Sprint 04: Import Dialog Updates ---
    /// Import file selected (from browse).
    ImportFileSelected { file_path: String },

    /// Import validation started.
    ImportValidating,

    /// Import preview ready.
    ImportPreview {
        format: String,
        collection_name: Option<String>,
        environment_name: Option<String>,
        request_count: usize,
        folder_count: usize,
        variable_count: usize,
        warnings: Vec<ImportWarningData>,
    },

    /// Import progress update.
    ImportProgress(f32),

    /// Import completed successfully with new dialog.
    ImportDialogComplete {
        name: String,
        requests_imported: usize,
        folders_imported: usize,
        variables_imported: usize,
    },

    /// Import error.
    ImportError { message: String },

    /// cURL export completed - copy to clipboard.
    CurlExport(String),

    // --- Sprint 06: JSON Format Updates ---
    /// Update response body with formatted JSON.
    FormattedResponseBody(String),

    /// Update request body with formatted JSON.
    FormattedRequestBody(String),

    /// Restore response state when switching tabs.
    RestoreResponseState {
        state: i32, // 0=Idle, 1=Loading, 2=Success, 3=Error
        body: String,
        status_code: i32,
        status_text: String,
        duration: String,
        size: String,
        headers: Vec<ResponseHeaderData>,
        error_title: String,
        error_message: String,
    },
}
