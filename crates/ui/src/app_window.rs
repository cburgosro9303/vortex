//! Application window management
//!
//! This module provides the main application window with all business logic bindings.

use std::collections::HashMap;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tokio::sync::mpsc;
use vortex_application::{
    CancellationToken, CreateWorkspace, CreateWorkspaceInput, EnvironmentRepository,
    ExecuteRequest, ExecuteResultExt, VariableResolver, ports::WorkspaceRepository,
};
use vortex_domain::{
    FontScale, HistoryAuth, HistoryEntry, HistoryHeader, HistoryParam, RequestHistory, ThemeMode,
    UserSettings,
};
use vortex_domain::{
    RequestState,
    environment::{Environment, ResolutionContext, Variable, VariableMap},
    persistence::{
        ApiKeyLocation, PersistenceAuth, PersistenceHttpMethod, PersistenceRequestBody,
        SavedRequest,
    },
    request::{HttpMethod, RequestBody, RequestSpec},
};
use vortex_infrastructure::{
    FileEnvironmentRepository, FileSystemWorkspaceRepository, HistoryRepository, PostmanImporter,
    ReqwestHttpClient, SettingsRepository, TokioFileSystem, from_json, to_json_stable,
};

use crate::EnvironmentInfo;
use crate::HeaderRow;
use crate::HistoryItem;
use crate::MainWindow;
use crate::QueryParam;
use crate::ResponseHeader;
use crate::TreeItem;
use crate::VariableRow;
use crate::VortexPalette;
use crate::VortexTypography;
// Sprint 06: Tab and Search types
use crate::RequestTab;
use crate::SearchResult;
// Sprint 04: Import Dialog types
use crate::ImportPreviewData;
use crate::ImportState;
use crate::ImportWarningItem;
use crate::bridge::{
    AuthData, EnvironmentData, HeaderData, HistoryItemData, ImportWarningData, QueryParamData,
    SearchResultData, TabData, TabState, TreeItemData, UiCommand, UiUpdate, VariableData,
};

/// Application window wrapper with business logic bindings.
pub struct AppWindow {
    window: MainWindow,
}

impl AppWindow {
    /// Creates a new application window.
    ///
    /// # Errors
    ///
    /// Returns an error if the window cannot be created.
    pub fn new() -> Result<Self, slint::PlatformError> {
        let window = MainWindow::new()?;
        let ui_weak = window.as_weak();

        // Create channels for UI <-> async communication
        let (cmd_tx, cmd_rx) = mpsc::unbounded_channel::<UiCommand>();
        let (update_tx, mut update_rx) = mpsc::unbounded_channel::<UiUpdate>();

        // Clone command senders for each callback
        let cmd_tx_send = cmd_tx.clone();
        let cmd_tx_cancel = cmd_tx.clone();
        let cmd_tx_create_ws = cmd_tx.clone();
        let cmd_tx_open_ws = cmd_tx.clone();
        let cmd_tx_close_ws = cmd_tx.clone();
        let cmd_tx_new_req = cmd_tx.clone();
        let cmd_tx_save = cmd_tx.clone();
        let cmd_tx_item_sel = cmd_tx.clone();
        let cmd_tx_item_dbl = cmd_tx.clone();
        let cmd_tx_toggle = cmd_tx.clone();

        // Environment command senders
        let cmd_tx_env_changed = cmd_tx.clone();
        let cmd_tx_manage_env = cmd_tx.clone();
        let cmd_tx_create_env = cmd_tx.clone();
        let cmd_tx_delete_env = cmd_tx.clone();
        let cmd_tx_select_env = cmd_tx.clone();
        let cmd_tx_save_env = cmd_tx.clone();
        let cmd_tx_add_var = cmd_tx.clone();
        let cmd_tx_del_var = cmd_tx.clone();
        let cmd_tx_var_changed = cmd_tx.clone();
        let cmd_tx_url_changed = cmd_tx.clone();

        // Settings command senders
        let cmd_tx_toggle_theme = cmd_tx.clone();
        let cmd_tx_open_settings = cmd_tx.clone();
        let cmd_tx_theme_mode = cmd_tx.clone();
        let cmd_tx_font_scale = cmd_tx.clone();

        // History command senders
        let cmd_tx_history_click = cmd_tx.clone();
        let cmd_tx_clear_history = cmd_tx.clone();

        // Sprint 05: Query params command senders
        let cmd_tx_add_qp = cmd_tx.clone();
        let cmd_tx_del_qp = cmd_tx.clone();
        let cmd_tx_qp_changed = cmd_tx.clone();

        // Sprint 05: Headers command senders
        let cmd_tx_add_header = cmd_tx.clone();
        let cmd_tx_del_header = cmd_tx.clone();
        let cmd_tx_header_changed = cmd_tx.clone();

        // Sprint 05: Auth command senders
        let cmd_tx_auth_type = cmd_tx.clone();
        let cmd_tx_bearer = cmd_tx.clone();
        let cmd_tx_basic = cmd_tx.clone();
        let cmd_tx_apikey = cmd_tx.clone();

        // Sprint 05: Collection management command senders
        let cmd_tx_save_req = cmd_tx.clone();
        let cmd_tx_rename = cmd_tx.clone();
        let cmd_tx_delete_req = cmd_tx.clone();
        let cmd_tx_confirm_del = cmd_tx.clone();
        let cmd_tx_cancel_del = cmd_tx.clone();

        // Sprint 06: Tab command senders
        let cmd_tx_tab_clicked = cmd_tx.clone();
        let cmd_tx_tab_close = cmd_tx.clone();
        let cmd_tx_new_tab = cmd_tx.clone();

        // Sprint 06: Quick search command senders
        let cmd_tx_open_search = cmd_tx.clone();
        let cmd_tx_close_search = cmd_tx.clone();
        let cmd_tx_search_query = cmd_tx.clone();
        let cmd_tx_search_result = cmd_tx.clone();

        // Sprint 06: Import/Export command senders
        let cmd_tx_import = cmd_tx.clone();
        let cmd_tx_export = cmd_tx.clone();
        let cmd_tx_export_curl = cmd_tx.clone();
        let cmd_tx_import_env = cmd_tx.clone();

        // Sprint 04: Import Dialog command senders
        let cmd_tx_import_browse = cmd_tx.clone();
        let cmd_tx_import_start = cmd_tx.clone();
        let cmd_tx_import_cancel = cmd_tx.clone();

        // Sprint 06: JSON format command senders
        let cmd_tx_format = cmd_tx.clone();
        let cmd_tx_format_req = cmd_tx.clone();
        let cmd_tx_copy_formatted = cmd_tx.clone();

        // Set up UI callbacks
        window.on_send_request(move || {
            let _ = cmd_tx_send.send(UiCommand::SendRequest);
        });

        window.on_cancel_request(move || {
            let _ = cmd_tx_cancel.send(UiCommand::CancelRequest);
        });

        window.on_copy_response_body(move || {
            // TODO: Implement clipboard copy
            eprintln!("Copy to clipboard not yet implemented");
        });

        // Workspace callbacks
        window.on_create_workspace(move || {
            let tx = cmd_tx_create_ws.clone();
            std::thread::spawn(move || {
                // Use rfd to pick a folder
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Create New Workspace")
                    .pick_folder()
                {
                    let name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("My Workspace")
                        .to_string();
                    let _ = tx.send(UiCommand::CreateWorkspace { path, name });
                }
            });
        });

        window.on_open_workspace(move || {
            let tx = cmd_tx_open_ws.clone();
            std::thread::spawn(move || {
                // Use rfd to pick a folder
                if let Some(path) = rfd::FileDialog::new()
                    .set_title("Open Workspace")
                    .pick_folder()
                {
                    let _ = tx.send(UiCommand::OpenWorkspace { path });
                }
            });
        });

        window.on_close_workspace(move || {
            let _ = cmd_tx_close_ws.send(UiCommand::CloseWorkspace);
        });

        window.on_new_request_clicked(move || {
            let _ = cmd_tx_new_req.send(UiCommand::CreateRequest {
                collection_path: PathBuf::new(),
                name: "New Request".to_string(),
            });
        });

        window.on_save_collection(move || {
            let _ = cmd_tx_save.send(UiCommand::SaveCollection);
        });

        window.on_item_selected(move |item: TreeItem| {
            let _ = cmd_tx_item_sel.send(UiCommand::ItemSelected {
                id: item.id.to_string(),
                path: PathBuf::from(item.path.to_string()),
            });
        });

        window.on_item_double_clicked(move |item: TreeItem| {
            let _ = cmd_tx_item_dbl.send(UiCommand::ItemDoubleClicked {
                id: item.id.to_string(),
                path: PathBuf::from(item.path.to_string()),
            });
        });

        window.on_toggle_folder(move |item: TreeItem| {
            let _ = cmd_tx_toggle.send(UiCommand::ToggleFolder {
                id: item.id.to_string(),
            });
        });

        // Environment callbacks (Sprint 03)
        window.on_environment_changed(move |index| {
            let _ = cmd_tx_env_changed.send(UiCommand::EnvironmentChanged { index });
        });

        window.on_manage_environments_clicked(move || {
            let _ = cmd_tx_manage_env.send(UiCommand::ManageEnvironments);
        });

        window.on_create_environment(move |name| {
            let _ = cmd_tx_create_env.send(UiCommand::CreateEnvironment {
                name: name.to_string(),
            });
        });

        window.on_delete_environment(move |index| {
            let _ = cmd_tx_delete_env.send(UiCommand::DeleteEnvironment { index });
        });

        window.on_select_env_for_editing(move |index| {
            let _ = cmd_tx_select_env.send(UiCommand::SelectEnvironmentForEditing { index });
        });

        window.on_save_environment(move || {
            let _ = cmd_tx_save_env.send(UiCommand::SaveEnvironment);
        });

        window.on_add_env_variable(move || {
            let _ = cmd_tx_add_var.send(UiCommand::AddEnvironmentVariable);
        });

        window.on_delete_env_variable(move |index| {
            let _ = cmd_tx_del_var.send(UiCommand::DeleteEnvironmentVariable { index });
        });

        window.on_env_variable_changed(move |index, var: VariableRow| {
            let _ = cmd_tx_var_changed.send(UiCommand::EnvironmentVariableChanged {
                index,
                name: var.name.to_string(),
                value: var.value.to_string(),
                enabled: var.enabled,
                is_secret: var.is_secret,
            });
        });

        window.on_url_changed(move |url| {
            let _ = cmd_tx_url_changed.send(UiCommand::UrlChanged {
                url: url.to_string(),
            });
        });

        // Settings callbacks (Sprint 04)
        window.on_toggle_theme(move || {
            let _ = cmd_tx_toggle_theme.send(UiCommand::ToggleTheme);
        });

        window.on_open_settings(move || {
            let _ = cmd_tx_open_settings.send(UiCommand::OpenSettings);
        });

        window.on_theme_mode_changed(move |index| {
            let _ = cmd_tx_theme_mode.send(UiCommand::SetThemeMode { index });
        });

        window.on_font_scale_changed(move |index| {
            let _ = cmd_tx_font_scale.send(UiCommand::SetFontScale { index });
        });

        // History callbacks (Sprint 04)
        window.on_history_item_clicked(move |item: HistoryItem| {
            let _ = cmd_tx_history_click.send(UiCommand::LoadHistoryItem {
                id: item.id.to_string(),
            });
        });

        window.on_clear_history(move || {
            let _ = cmd_tx_clear_history.send(UiCommand::ClearHistory);
        });

        // Sprint 05: Query params callbacks
        window.on_add_query_param(move || {
            let _ = cmd_tx_add_qp.send(UiCommand::AddQueryParam);
        });

        window.on_delete_query_param(move |index| {
            let _ = cmd_tx_del_qp.send(UiCommand::DeleteQueryParam { index });
        });

        window.on_query_param_changed(move |index, param: crate::QueryParam| {
            let _ = cmd_tx_qp_changed.send(UiCommand::QueryParamChanged {
                index,
                key: param.key.to_string(),
                value: param.value.to_string(),
                description: param.description.to_string(),
                enabled: param.enabled,
            });
        });

        // Sprint 05: Headers callbacks
        window.on_add_request_header(move || {
            let _ = cmd_tx_add_header.send(UiCommand::AddRequestHeader);
        });

        window.on_delete_request_header(move |index| {
            let _ = cmd_tx_del_header.send(UiCommand::DeleteRequestHeader { index });
        });

        window.on_request_header_changed(move |index, header: crate::HeaderRow| {
            let _ = cmd_tx_header_changed.send(UiCommand::RequestHeaderChanged {
                index,
                key: header.key.to_string(),
                value: header.value.to_string(),
                description: header.description.to_string(),
                enabled: header.enabled,
            });
        });

        // Sprint 05: Auth callbacks
        window.on_auth_type_changed(move |auth_type| {
            let _ = cmd_tx_auth_type.send(UiCommand::AuthTypeChanged { auth_type });
        });

        window.on_auth_bearer_token_changed(move |token| {
            let _ = cmd_tx_bearer.send(UiCommand::BearerTokenChanged {
                token: token.to_string(),
            });
        });

        window.on_auth_basic_credentials_changed(move |username, password| {
            let _ = cmd_tx_basic.send(UiCommand::BasicCredentialsChanged {
                username: username.to_string(),
                password: password.to_string(),
            });
        });

        window.on_auth_api_key_changed(move |key_name, key_value, location| {
            let _ = cmd_tx_apikey.send(UiCommand::ApiKeyChanged {
                key_name: key_name.to_string(),
                key_value: key_value.to_string(),
                location,
            });
        });

        // Sprint 05: Collection management callbacks
        window.on_save_current_request(move || {
            let _ = cmd_tx_save_req.send(UiCommand::SaveCurrentRequest);
        });

        window.on_rename_item(move |id, new_name| {
            let _ = cmd_tx_rename.send(UiCommand::RenameItem {
                id: id.to_string(),
                new_name: new_name.to_string(),
            });
        });

        window.on_delete_item_requested(move |id, item_type| {
            let _ = cmd_tx_delete_req.send(UiCommand::DeleteItemRequested {
                id: id.to_string(),
                item_type: item_type.to_string(),
            });
        });

        window.on_confirm_delete(move || {
            let _ = cmd_tx_confirm_del.send(UiCommand::ConfirmDelete);
        });

        window.on_cancel_delete(move || {
            let _ = cmd_tx_cancel_del.send(UiCommand::CancelDelete);
        });

        // Sprint 06: Tab callbacks
        window.on_tab_clicked(move |id| {
            let _ = cmd_tx_tab_clicked.send(UiCommand::TabClicked { id: id.to_string() });
        });

        window.on_tab_close_clicked(move |id| {
            let _ = cmd_tx_tab_close.send(UiCommand::TabCloseClicked { id: id.to_string() });
        });

        window.on_new_tab_clicked(move || {
            let _ = cmd_tx_new_tab.send(UiCommand::NewTabClicked);
        });

        // Sprint 06: Quick search callbacks
        window.on_open_quick_search(move || {
            let _ = cmd_tx_open_search.send(UiCommand::OpenQuickSearch);
        });

        window.on_close_quick_search(move || {
            let _ = cmd_tx_close_search.send(UiCommand::CloseQuickSearch);
        });

        window.on_search_query_changed(move |query| {
            let _ = cmd_tx_search_query.send(UiCommand::SearchQueryChanged {
                query: query.to_string(),
            });
        });

        window.on_search_result_clicked(move |result: SearchResult| {
            let _ = cmd_tx_search_result.send(UiCommand::SearchResultClicked {
                id: result.id.to_string(),
                path: result.path.to_string(),
            });
        });

        // Sprint 06: Import/Export callbacks
        window.on_import_collection(move || {
            let _ = cmd_tx_import.send(UiCommand::ImportCollection);
        });

        window.on_import_environment(move || {
            let _ = cmd_tx_import_env.send(UiCommand::ImportEnvironment);
        });

        window.on_export_collection(move || {
            let _ = cmd_tx_export.send(UiCommand::ExportCollection);
        });

        window.on_export_as_curl(move || {
            let _ = cmd_tx_export_curl.send(UiCommand::ExportAsCurl);
        });

        // Sprint 04: Import Dialog callbacks
        window.on_import_browse_file(move || {
            let _ = cmd_tx_import_browse.send(UiCommand::ImportBrowseFile);
        });

        let ui_weak_import_start = ui_weak.clone();
        window.on_import_start(move || {
            if let Some(ui) = ui_weak_import_start.upgrade() {
                let file_path = ui.get_import_selected_file().to_string();
                let _ = cmd_tx_import_start.send(UiCommand::ImportStart { file_path });
            }
        });

        window.on_import_cancel(move || {
            let _ = cmd_tx_import_cancel.send(UiCommand::ImportCancel);
        });

        // Sprint 06: JSON format callbacks
        window.on_format_response_body(move || {
            let _ = cmd_tx_format.send(UiCommand::FormatResponseBody);
        });

        window.on_copy_formatted_response(move || {
            let _ = cmd_tx_copy_formatted.send(UiCommand::CopyFormattedResponse);
        });

        let ui_weak_format_req = ui_weak.clone();
        window.on_format_request_body(move || {
            if let Some(ui) = ui_weak_format_req.upgrade() {
                let body = ui.get_request_body().to_string();
                let _ = cmd_tx_format_req.send(UiCommand::FormatRequestBody { body });
            }
        });

        // Spawn the async runtime in a separate thread
        let ui_weak_async = ui_weak.clone();
        let cmd_tx_async = cmd_tx;
        std::thread::spawn(move || {
            run_async_runtime(ui_weak_async, cmd_rx, update_tx, cmd_tx_async);
        });

        // Process UI updates on the main thread using a timer
        let ui_weak_update = ui_weak;
        let timer = slint::Timer::default();
        timer.start(
            slint::TimerMode::Repeated,
            std::time::Duration::from_millis(16), // ~60fps
            move || {
                while let Ok(update) = update_rx.try_recv() {
                    if let Some(ui) = ui_weak_update.upgrade() {
                        apply_update(&ui, update);
                    }
                }
            },
        );

        // Keep the timer alive by storing it
        // Note: We leak the timer intentionally to keep it running for the app lifetime
        std::mem::forget(timer);

        Ok(Self { window })
    }

    /// Runs the application event loop.
    ///
    /// This method blocks until the window is closed.
    ///
    /// # Errors
    ///
    /// Returns an error if the event loop fails.
    pub fn run(&self) -> Result<(), slint::PlatformError> {
        self.window.run()
    }

    /// Returns a reference to the underlying Slint window.
    #[must_use]
    pub const fn window(&self) -> &MainWindow {
        &self.window
    }
}

impl Default for AppWindow {
    fn default() -> Self {
        Self::new().expect("Failed to create application window")
    }
}

/// Application state managed by the async runtime.
struct AppState {
    workspace_path: Option<PathBuf>,
    expanded_folders: std::collections::HashSet<String>,
    // Environment state (Sprint 03)
    environments: Vec<Environment>,
    current_environment_index: Option<usize>,
    editing_environment: Option<Environment>,
    editing_environment_index: Option<usize>,
    editing_variable_keys: Vec<String>, // Tracks variable order for UI sync
    current_url: String,
    // Settings state (Sprint 04)
    dark_mode: bool,
    theme_mode: ThemeMode,
    font_scale: FontScale,
    // History state (Sprint 04)
    history: RequestHistory,
    history_visible: bool,
    // Sprint 05: Query params state
    query_params: Vec<QueryParamData>,
    base_url: String, // URL without query params
    // Sprint 05: Request headers state
    request_headers: Vec<HeaderData>,
    // Sprint 05: Authentication state
    auth_data: AuthData,
    // Sprint 05: Collection management state
    pending_delete_id: Option<String>,
    pending_delete_type: Option<String>,
    // Sprint 06: Tab state
    tabs: Vec<TabState>,
    active_tab_id: Option<String>,
    // Sprint 06: Search state
    all_requests: Vec<SearchResultData>, // Cached for search
    // Sprint 06: Response body for formatting
    response_body: String,
    // Flag to prevent circular URL update when params change
    updating_url_from_params: bool,
    // Sprint 04: Import state
    import_file_path: Option<String>,
    import_preview_done: bool,
}

impl AppState {
    fn from_settings(settings: UserSettings, history: RequestHistory) -> Self {
        Self {
            workspace_path: None,
            expanded_folders: std::collections::HashSet::new(),
            environments: Vec::new(),
            current_environment_index: None,
            editing_environment: None,
            editing_environment_index: None,
            editing_variable_keys: Vec::new(),
            current_url: String::new(),
            dark_mode: settings.theme.is_dark(),
            theme_mode: settings.theme,
            font_scale: settings.font_scale,
            history,
            history_visible: settings.history_visible,
            // Sprint 05
            query_params: Vec::new(),
            base_url: String::new(),
            request_headers: Vec::new(),
            auth_data: AuthData::default(),
            pending_delete_id: None,
            pending_delete_type: None,
            // Sprint 06
            tabs: Vec::new(),
            active_tab_id: None,
            all_requests: Vec::new(),
            response_body: String::new(),
            updating_url_from_params: false,
            import_file_path: None,
            import_preview_done: false,
        }
    }

    /// Gets tabs as UI data.
    fn tabs_to_ui(&self) -> Vec<TabData> {
        self.tabs.iter().map(TabState::to_tab_data).collect()
    }

    /// Saves current UI state to the active tab.
    fn save_current_tab_state(&mut self, url: &str, method: i32, body: &str) {
        if let Some(ref active_id) = self.active_tab_id
            && let Some(tab) = self.tabs.iter_mut().find(|t| &t.id == active_id) {
                tab.url = url.to_string();
                tab.method = method;
                tab.body = body.to_string();
                tab.headers = self.request_headers.clone();
                tab.query_params = self.query_params.clone();
                tab.auth = self.auth_data.clone();
            }
    }

    /// Restores tab state to UI.
    fn get_tab_state(&self, tab_id: &str) -> Option<&TabState> {
        self.tabs.iter().find(|t| t.id == tab_id)
    }

    fn to_settings(&self) -> UserSettings {
        UserSettings {
            theme: self.theme_mode,
            font_scale: self.font_scale,
            history_visible: self.history_visible,
            ..UserSettings::default()
        }
    }

    fn history_to_ui_items(&self) -> Vec<HistoryItemData> {
        self.history
            .entries()
            .iter()
            .map(|entry| HistoryItemData {
                id: entry.id.clone(),
                method: entry.method.to_string(),
                url: entry.url.clone(),
                #[allow(clippy::cast_possible_wrap)]
                status_code: entry.status_code.map_or(0, i32::from),
                time_ago: entry.time_ago(),
                duration: entry.duration_display(),
            })
            .collect()
    }

    fn current_environment(&self) -> Option<&Environment> {
        self.current_environment_index
            .and_then(|idx| self.environments.get(idx))
    }

    fn build_resolution_context(&self) -> ResolutionContext {
        let environment_vars = self
            .current_environment()
            .map(|env| env.variables.clone())
            .unwrap_or_default();

        let environment_name = self
            .current_environment()
            .map(|env| env.name.clone())
            .unwrap_or_default();

        ResolutionContext {
            globals: VariableMap::new(),
            collection: VariableMap::new(),
            environment: environment_vars,
            environment_name,
            secrets: HashMap::new(),
        }
    }
}

/// Runs the async runtime for handling HTTP requests and workspace operations.
fn run_async_runtime(
    ui_weak: slint::Weak<MainWindow>,
    mut cmd_rx: mpsc::UnboundedReceiver<UiCommand>,
    update_tx: mpsc::UnboundedSender<UiUpdate>,
    cmd_tx: mpsc::UnboundedSender<UiCommand>,
) {
    let rt = tokio::runtime::Builder::new_multi_thread()
        .worker_threads(2)
        .enable_all()
        .build()
        .expect("Failed to create Tokio runtime");

    rt.block_on(async move {
        // Initialize infrastructure
        let http_client = ReqwestHttpClient::new().expect("Failed to create HTTP client");
        let execute_request = ExecuteRequest::new(Arc::new(http_client));
        let fs = TokioFileSystem;
        let workspace_repo = FileSystemWorkspaceRepository::new(fs);
        let settings_repo = SettingsRepository::new();

        // Load user settings and history
        let settings = settings_repo.load().await.unwrap_or_default();
        let history_repo = HistoryRepository::new();
        let history = history_repo.load().await.unwrap_or_else(|_| RequestHistory::new(settings.history_limit));

        // Application state (initialized from settings)
        let mut state = AppState::from_settings(settings.clone(), history);
        let mut current_cancel: Option<CancellationToken> = None;

        // Send initial settings to UI
        let _ = update_tx.send(UiUpdate::SettingsLoaded {
            theme_index: settings.theme.to_index(),
            font_scale_index: settings.font_scale.to_index(),
            dark_mode: settings.theme.is_dark(),
            font_scale_factor: settings.font_scale.factor(),
        });

        // Send initial history to UI
        let _ = update_tx.send(UiUpdate::HistoryItems(state.history_to_ui_items()));
        let _ = update_tx.send(UiUpdate::HistoryVisible(state.history_visible));

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                UiCommand::SendRequest => {
                    if let Some(result) = handle_send_request(
                        &ui_weak,
                        &execute_request,
                        &update_tx,
                        &mut current_cancel,
                        &state,
                    )
                    .await
                    {
                        // Save response to current tab
                        if let Some(ref active_id) = state.active_tab_id
                            && let Some(tab) = state.tabs.iter_mut().find(|t| &t.id == active_id) {
                                tab.response_state = result.response_state;
                                tab.response_body = result.response_body.clone();
                                tab.status_code = result.status_code.map_or(0, i32::from);
                                tab.status_text = result.status_text.clone();
                                tab.duration = result.duration_display.clone();
                                tab.size = result.size_display.clone();
                                tab.response_headers = result.response_headers.clone();
                                tab.error_title = result.error_title.clone();
                                tab.error_message = result.error_message.clone();
                            }

                        // Also save to state for formatting
                        state.response_body = result.response_body.clone();

                        // Add to history
                        let body_for_history = if result.request_body.is_empty() {
                            None
                        } else {
                            Some(result.request_body.clone())
                        };

                        // Convert headers for history
                        let headers_for_history: Vec<HistoryHeader> = state.request_headers
                            .iter()
                            .map(|h| HistoryHeader {
                                key: h.key.clone(),
                                value: h.value.clone(),
                                enabled: h.enabled,
                            })
                            .collect();

                        // Convert params for history
                        let params_for_history: Vec<HistoryParam> = state.query_params
                            .iter()
                            .map(|p| HistoryParam {
                                key: p.key.clone(),
                                value: p.value.clone(),
                                enabled: p.enabled,
                            })
                            .collect();

                        // Convert auth for history
                        let auth_for_history = if state.auth_data.auth_type > 0 {
                            Some(HistoryAuth {
                                auth_type: state.auth_data.auth_type,
                                bearer_token: state.auth_data.bearer_token.clone(),
                                basic_username: state.auth_data.basic_username.clone(),
                                basic_password: state.auth_data.basic_password.clone(),
                                api_key_name: state.auth_data.api_key_name.clone(),
                                api_key_value: state.auth_data.api_key_value.clone(),
                                api_key_location: state.auth_data.api_key_location,
                            })
                        } else {
                            None
                        };

                        let entry = if let (Some(status), Some(duration)) = (result.status_code, result.duration_ms) {
                            HistoryEntry::new(
                                result.method,
                                result.url,
                                status,
                                duration,
                                None,
                                body_for_history,
                                headers_for_history,
                                params_for_history,
                                auth_for_history,
                            )
                        } else {
                            HistoryEntry::failed(
                                result.method,
                                result.url,
                                None,
                                body_for_history,
                                headers_for_history,
                                params_for_history,
                                auth_for_history,
                            )
                        };

                        state.history.add(entry);

                        // Update UI with new history
                        let _ = update_tx.send(UiUpdate::HistoryItems(state.history_to_ui_items()));

                        // Save history to disk
                        let history_repo = HistoryRepository::new();
                        if let Err(e) = history_repo.save(&state.history).await {
                            eprintln!("Failed to save history: {e}");
                        }
                    }
                }

                UiCommand::CancelRequest => {
                    if let Some(cancel) = current_cancel.take() {
                        cancel.cancel();
                    }
                }

                UiCommand::CreateWorkspace { path, name } => {
                    let create_ws =
                        CreateWorkspace::new(FileSystemWorkspaceRepository::new(TokioFileSystem));
                    match create_ws
                        .execute(CreateWorkspaceInput {
                            path: path.clone(),
                            name,
                        })
                        .await
                    {
                        Ok(_manifest) => {
                            state.workspace_path = Some(path.clone());
                            state.environments.clear();
                            state.current_environment_index = None;

                            let _ = update_tx
                                .send(UiUpdate::WorkspacePath(path.display().to_string()));

                            // Load initial tree (empty for new workspace)
                            let _ = update_tx.send(UiUpdate::CollectionItems(vec![]));

                            // Load environments
                            load_environments(&path, &mut state, &update_tx).await;
                        }
                        Err(e) => {
                            let _ = update_tx.send(UiUpdate::Error {
                                title: "Failed to create workspace".to_string(),
                                message: e.to_string(),
                            });
                        }
                    }
                }

                UiCommand::OpenWorkspace { path } => {
                    match workspace_repo.load(&path).await {
                        Ok(_manifest) => {
                            state.workspace_path = Some(path.clone());
                            let _ = update_tx
                                .send(UiUpdate::WorkspacePath(path.display().to_string()));

                            // Load collections from workspace
                            let items =
                                load_workspace_tree(&path, &state.expanded_folders).await;
                            let _ = update_tx.send(UiUpdate::CollectionItems(items));

                            // Sprint 06: Cache all requests for quick search
                            state.all_requests = load_all_requests_for_search(&path).await;

                            // Load environments
                            load_environments(&path, &mut state, &update_tx).await;
                        }
                        Err(e) => {
                            let _ = update_tx.send(UiUpdate::Error {
                                title: "Failed to open workspace".to_string(),
                                message: e.to_string(),
                            });
                        }
                    }
                }

                UiCommand::CloseWorkspace => {
                    state.workspace_path = None;
                    state.expanded_folders.clear();
                    state.environments.clear();
                    state.current_environment_index = None;
                    state.editing_environment = None;
                    state.editing_environment_index = None;

                    let _ = update_tx.send(UiUpdate::WorkspacePath(String::new()));
                    let _ = update_tx.send(UiUpdate::CollectionItems(vec![]));
                    let _ = update_tx.send(UiUpdate::EnvironmentNames(vec![]));
                    let _ = update_tx.send(UiUpdate::CurrentEnvironmentIndex(0));
                }

                UiCommand::ToggleFolder { id } => {
                    if state.expanded_folders.contains(&id) {
                        state.expanded_folders.remove(&id);
                    } else {
                        state.expanded_folders.insert(id);
                    }

                    // Refresh tree
                    if let Some(ref ws_path) = state.workspace_path {
                        let items =
                            load_workspace_tree(ws_path, &state.expanded_folders).await;
                        let _ = update_tx.send(UiUpdate::CollectionItems(items));
                    }
                }

                UiCommand::ItemSelected { id: _, path: _ } => {
                    // Just selection, no action needed
                }

                UiCommand::ItemDoubleClicked { id: _, path } => {
                    // Load request into editor - Sprint 06: Opens in a tab
                    if path.extension().is_some_and(|e| e == "json") {
                        // Check if this request is already open in a tab
                        let path_str = path.display().to_string();
                        let existing_tab = state.tabs.iter().find(|t| t.file_path.as_ref() == Some(&path_str));

                        if let Some(tab) = existing_tab {
                            // Tab already exists, switch to it
                            let tab_id = tab.id.clone();
                            state.active_tab_id = Some(tab_id.clone());

                            let tab_data = state.get_tab_state(&tab_id).cloned();
                            if let Some(tab) = tab_data {
                                state.current_url = tab.url.clone();
                                state.base_url = tab.url.split('?').next().unwrap_or("").to_string();
                                state.query_params = tab.query_params.clone();
                                state.request_headers = tab.headers.clone();
                                state.auth_data = tab.auth.clone();

                                let _ = update_tx.send(UiUpdate::LoadFullRequest {
                                    url: tab.url.clone(),
                                    method: tab.method,
                                    body: tab.body.clone(),
                                    headers: tab.headers.clone(),
                                    query_params: tab.query_params.clone(),
                                    auth: tab.auth.clone(),
                                });
                            }

                            let _ = update_tx.send(UiUpdate::ActiveTabChanged(tab_id));
                        } else if let Ok(content) = tokio::fs::read_to_string(&path).await {
                            // Create a new tab for this request
                            // Try to parse as SavedRequest first, fall back to raw JSON for imported files
                            let parsed_request = from_json::<vortex_domain::persistence::SavedRequest>(&content);

                            // If parsing fails, try to extract data directly from JSON (for old imports)
                            let (request_name, method_str, url, body, headers_map, query_params_map) = if let Ok(req) = &parsed_request {
                                use vortex_domain::persistence::PersistenceRequestBody;
                                let body_str = req.body.as_ref().map(|b| match b {
                                    PersistenceRequestBody::Json { content } => content.to_string(),
                                    PersistenceRequestBody::Text { content } => content.clone(),
                                    PersistenceRequestBody::Graphql { query, .. } => query.clone(),
                                    _ => String::new(),
                                }).unwrap_or_default();

                                (
                                    req.name.clone(),
                                    format!("{:?}", req.method).to_uppercase(),
                                    req.url.clone(),
                                    body_str,
                                    req.headers.clone(),
                                    req.query_params.clone(),
                                )
                            } else if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                // Fallback for old import format
                                let name = json.get("name").and_then(|n| n.as_str()).unwrap_or("Unknown").to_string();
                                let method = json.get("method").and_then(|m| m.as_str()).unwrap_or("GET").to_string();
                                let url = json.get("url").and_then(|u| u.as_str()).unwrap_or("").to_string();
                                let body = json.get("body").and_then(|b| b.as_str()).unwrap_or("").to_string();

                                let headers_map: std::collections::BTreeMap<String, String> = json.get("headers")
                                    .and_then(|h| h.as_object())
                                    .map(|obj| obj.iter().filter_map(|(k, v)| {
                                        Some((k.clone(), v.as_str()?.to_string()))
                                    }).collect())
                                    .unwrap_or_default();

                                let query_params_map: std::collections::BTreeMap<String, String> = json.get("query_params")
                                    .and_then(|q| q.as_object())
                                    .map(|obj| obj.iter().filter_map(|(k, v)| {
                                        Some((k.clone(), v.as_str()?.to_string()))
                                    }).collect())
                                    .unwrap_or_default();

                                (name, method, url, body, headers_map, query_params_map)
                            } else {
                                continue;
                            };

                            {
                                let method_index = match method_str.to_uppercase().as_str() {
                                    "GET" => 0,
                                    "POST" => 1,
                                    "PUT" => 2,
                                    "PATCH" => 3,
                                    "DELETE" => 4,
                                    "HEAD" => 5,
                                    "OPTIONS" => 6,
                                    _ => 0,
                                };

                                let headers: Vec<HeaderData> = headers_map.iter()
                                    .map(|(k, v)| HeaderData { key: k.clone(), value: v.clone(), description: String::new(), enabled: true })
                                    .collect();

                                let query_params: Vec<QueryParamData> = query_params_map.iter()
                                    .map(|(k, v)| QueryParamData { key: k.clone(), value: v.clone(), description: String::new(), enabled: true })
                                    .collect();

                                let new_tab = TabState {
                                    id: uuid::Uuid::now_v7().to_string(),
                                    name: request_name.clone(),
                                    method: method_index,
                                    url: url.clone(),
                                    body: body.clone(),
                                    headers: headers.clone(),
                                    query_params: query_params.clone(),
                                    auth: AuthData::default(),
                                    has_unsaved_changes: false,
                                    file_path: Some(path_str),
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
                                };

                                let new_id = new_tab.id.clone();
                                state.tabs.push(new_tab);
                                state.active_tab_id = Some(new_id.clone());

                                state.current_url = url.clone();
                                state.base_url = url.split('?').next().unwrap_or("").to_string();
                                state.query_params = query_params.clone();
                                state.request_headers = headers.clone();

                                let _ = update_tx.send(UiUpdate::LoadFullRequest {
                                    url,
                                    method: method_index,
                                    body,
                                    headers,
                                    query_params,
                                    auth: AuthData::default(),
                                });

                                let _ = update_tx.send(UiUpdate::TabsUpdated(state.tabs_to_ui()));
                                let _ = update_tx.send(UiUpdate::ActiveTabChanged(new_id));

                                resolve_and_update_url(&state, &update_tx);
                            }
                        }
                    }
                }

                UiCommand::CreateRequest {
                    collection_path: _,
                    name,
                } => {
                    // Create a new request in the first collection (or selected collection)
                    if let Some(ref ws_path) = state.workspace_path {
                        let collections_dir = ws_path.join("collections");

                        // Find the first collection directory
                        if let Ok(mut entries) = tokio::fs::read_dir(&collections_dir).await
                            && let Ok(Some(first_entry)) = entries.next_entry().await {
                                let collection_path = first_entry.path();
                                if collection_path.is_dir() {
                                    // Create the requests directory if it doesn't exist
                                    let requests_dir = collection_path.join("request");
                                    let _ = tokio::fs::create_dir_all(&requests_dir).await;

                                    // Generate a unique ID and filename
                                    let request_id = uuid::Uuid::now_v7().to_string();
                                    let safe_name = name.to_lowercase().replace(' ', "-");
                                    let file_name = format!("{safe_name}.json");
                                    let file_path = requests_dir.join(&file_name);

                                    // Create the request
                                    let new_request = SavedRequest::new(
                                        request_id,
                                        &name,
                                        PersistenceHttpMethod::Get,
                                        "https://api.example.com",
                                    );

                                    // Serialize and save
                                    if let Ok(json) = serde_json::to_string_pretty(&new_request) {
                                        if let Err(e) = tokio::fs::write(&file_path, json).await {
                                            eprintln!("Failed to write request file: {e}");
                                        } else {
                                            // Expand the collection to show the new request
                                            state.expanded_folders.insert(collection_path.display().to_string());

                                            // Refresh the tree
                                            let items = load_workspace_tree(ws_path, &state.expanded_folders).await;
                                            let _ = update_tx.send(UiUpdate::CollectionItems(items));
                                        }
                                    }
                                }
                            }
                    }
                }

                UiCommand::SaveCollection => {
                    let _ = update_tx.send(UiUpdate::SavingState(true));

                    let mut saved_count = 0;
                    let mut error_count = 0;

                    // Save all tabs with unsaved changes
                    for tab in &mut state.tabs {
                        if tab.has_unsaved_changes
                            && let Some(ref file_path) = tab.file_path {
                                let mut saved_request = build_saved_request_from_tab(tab);

                                // Try to preserve the original ID
                                if let Ok(existing_content) = std::fs::read_to_string(file_path)
                                    && let Ok(existing_request) = serde_json::from_str::<SavedRequest>(&existing_content) {
                                        saved_request.id = existing_request.id;
                                    }

                                if let Ok(json) = to_json_stable(&saved_request) {
                                    if tokio::fs::write(file_path, json).await.is_ok() {
                                        tab.has_unsaved_changes = false;
                                        saved_count += 1;
                                    } else {
                                        error_count += 1;
                                    }
                                } else {
                                    error_count += 1;
                                }
                            }
                    }

                    let _ = update_tx.send(UiUpdate::SavingState(false));
                    let _ = update_tx.send(UiUpdate::TabsUpdated(state.tabs_to_ui()));

                    if error_count > 0 {
                        let _ = update_tx.send(UiUpdate::Error {
                            title: "Some requests failed to save".to_string(),
                            message: format!("Saved {saved_count} requests, {error_count} failed"),
                        });
                    }
                }

                // Environment commands (Sprint 03)
                UiCommand::EnvironmentChanged { index } => {
                    if index >= 0 {
                        state.current_environment_index = Some(index as usize);
                    } else {
                        state.current_environment_index = None;
                    }
                    let _ = update_tx.send(UiUpdate::CurrentEnvironmentIndex(index));

                    // Update resolved URL preview
                    resolve_and_update_url(&state, &update_tx);
                }

                UiCommand::ManageEnvironments => {
                    let env_list: Vec<EnvironmentData> = state
                        .environments
                        .iter()
                        .map(|env| EnvironmentData {
                            id: env.id.to_string(),
                            name: env.name.clone(),
                            variable_count: env.variables.len() as i32,
                        })
                        .collect();

                    let _ = update_tx.send(UiUpdate::EnvironmentList(env_list));
                    let _ = update_tx.send(UiUpdate::ShowEnvironmentManager(true));
                }

                UiCommand::CreateEnvironment { name } => {
                    if let Some(ref ws_path) = state.workspace_path {
                        let new_env = Environment::new(&name);
                        let env_repo = FileEnvironmentRepository::new(TokioFileSystem);

                        match env_repo.save(ws_path, &new_env).await {
                            Ok(()) => {
                                state.environments.push(new_env);

                                // Update environment names
                                let names: Vec<String> =
                                    state.environments.iter().map(|e| e.name.clone()).collect();
                                let _ = update_tx.send(UiUpdate::EnvironmentNames(names));

                                // Update environment list in manager
                                let env_list: Vec<EnvironmentData> = state
                                    .environments
                                    .iter()
                                    .map(|env| EnvironmentData {
                                        id: env.id.to_string(),
                                        name: env.name.clone(),
                                        variable_count: env.variables.len() as i32,
                                    })
                                    .collect();
                                let _ = update_tx.send(UiUpdate::EnvironmentList(env_list));
                            }
                            Err(e) => {
                                let _ = update_tx.send(UiUpdate::Error {
                                    title: "Failed to create environment".to_string(),
                                    message: e.to_string(),
                                });
                            }
                        }
                    }
                }

                UiCommand::DeleteEnvironment { index } => {
                    if let Some(ref ws_path) = state.workspace_path
                        && let Some(env) = state.environments.get(index as usize) {
                            let env_repo = FileEnvironmentRepository::new(TokioFileSystem);

                            match env_repo.delete(ws_path, &env.name).await {
                                Ok(()) => {
                                    state.environments.remove(index as usize);

                                    // Reset current environment if it was deleted
                                    if state.current_environment_index == Some(index as usize) {
                                        state.current_environment_index = None;
                                        let _ = update_tx.send(UiUpdate::CurrentEnvironmentIndex(-1));
                                    } else if let Some(current_idx) = state.current_environment_index
                                        && current_idx > index as usize {
                                            state.current_environment_index = Some(current_idx - 1);
                                            let _ = update_tx.send(UiUpdate::CurrentEnvironmentIndex(
                                                (current_idx - 1) as i32,
                                            ));
                                        }

                                    // Update environment names
                                    let names: Vec<String> =
                                        state.environments.iter().map(|e| e.name.clone()).collect();
                                    let _ = update_tx.send(UiUpdate::EnvironmentNames(names));

                                    // Update environment list in manager
                                    let env_list: Vec<EnvironmentData> = state
                                        .environments
                                        .iter()
                                        .map(|env| EnvironmentData {
                                            id: env.id.to_string(),
                                            name: env.name.clone(),
                                            variable_count: env.variables.len() as i32,
                                        })
                                        .collect();
                                    let _ = update_tx.send(UiUpdate::EnvironmentList(env_list));

                                    // Clear editing state if deleted
                                    if state.editing_environment_index == Some(index as usize) {
                                        state.editing_environment = None;
                                        state.editing_environment_index = None;
                                        let _ = update_tx.send(UiUpdate::SelectedEnvironment {
                                            index: -1,
                                            name: String::new(),
                                            variables: vec![],
                                        });
                                    }

                                    // Update resolved URL preview
                                    resolve_and_update_url(&state, &update_tx);
                                }
                                Err(e) => {
                                    let _ = update_tx.send(UiUpdate::Error {
                                        title: "Failed to delete environment".to_string(),
                                        message: e.to_string(),
                                    });
                                }
                            }
                        }
                }

                UiCommand::SelectEnvironmentForEditing { index } => {
                    if let Some(env) = state.environments.get(index as usize) {
                        state.editing_environment = Some(env.clone());
                        state.editing_environment_index = Some(index as usize);

                        // Store variable keys in order they're sent to UI
                        let mut keys: Vec<String> = env.variables.keys().cloned().collect();
                        keys.sort(); // Sort alphabetically for consistent ordering
                        state.editing_variable_keys = keys.clone();

                        let variables: Vec<VariableData> = keys
                            .iter()
                            .filter_map(|key| {
                                env.variables.get(key).map(|var| VariableData {
                                    name: key.clone(),
                                    value: var.value.clone(),
                                    enabled: var.enabled,
                                    is_secret: var.secret,
                                })
                            })
                            .collect();

                        let _ = update_tx.send(UiUpdate::SelectedEnvironment {
                            index,
                            name: env.name.clone(),
                            variables,
                        });
                    }
                }

                UiCommand::SaveEnvironment => {
                    if let Some(ref ws_path) = state.workspace_path
                        && let Some(ref editing_env) = state.editing_environment {
                            let env_repo = FileEnvironmentRepository::new(TokioFileSystem);

                            match env_repo.save(ws_path, editing_env).await {
                                Ok(()) => {
                                    // Update the environment in state
                                    if let Some(idx) = state.editing_environment_index
                                        && idx < state.environments.len() {
                                            state.environments[idx] = editing_env.clone();
                                        }

                                    // Update environment list in manager
                                    let env_list: Vec<EnvironmentData> = state
                                        .environments
                                        .iter()
                                        .map(|env| EnvironmentData {
                                            id: env.id.to_string(),
                                            name: env.name.clone(),
                                            variable_count: env.variables.len() as i32,
                                        })
                                        .collect();
                                    let _ = update_tx.send(UiUpdate::EnvironmentList(env_list));

                                    // Update resolved URL preview
                                    resolve_and_update_url(&state, &update_tx);
                                }
                                Err(e) => {
                                    let _ = update_tx.send(UiUpdate::Error {
                                        title: "Failed to save environment".to_string(),
                                        message: e.to_string(),
                                    });
                                }
                            }
                        }
                }

                UiCommand::AddEnvironmentVariable => {
                    if let Some(ref mut editing_env) = state.editing_environment {
                        let var_name = format!("variable_{}", editing_env.variables.len() + 1);
                        editing_env.variables.insert(
                            var_name.clone(),
                            Variable {
                                value: String::new(),
                                enabled: true,
                                secret: false,
                            },
                        );

                        // Add to tracked keys
                        state.editing_variable_keys.push(var_name);

                        // Send variables in tracked order
                        let variables: Vec<VariableData> = state.editing_variable_keys
                            .iter()
                            .filter_map(|key| {
                                editing_env.variables.get(key).map(|var| VariableData {
                                    name: key.clone(),
                                    value: var.value.clone(),
                                    enabled: var.enabled,
                                    is_secret: var.secret,
                                })
                            })
                            .collect();

                        let _ = update_tx.send(UiUpdate::SelectedEnvironment {
                            index: state.editing_environment_index.map_or(-1, |i| i as i32),
                            name: editing_env.name.clone(),
                            variables,
                        });
                    }
                }

                UiCommand::DeleteEnvironmentVariable { index } => {
                    if let Some(ref mut editing_env) = state.editing_environment {
                        // Use tracked key order for deletion
                        if let Some(key) = state.editing_variable_keys.get(index as usize).cloned() {
                            editing_env.variables.remove(&key);
                            state.editing_variable_keys.remove(index as usize);

                            // Send variables in tracked order
                            let variables: Vec<VariableData> = state.editing_variable_keys
                                .iter()
                                .filter_map(|k| {
                                    editing_env.variables.get(k).map(|var| VariableData {
                                        name: k.clone(),
                                        value: var.value.clone(),
                                        enabled: var.enabled,
                                        is_secret: var.secret,
                                    })
                                })
                                .collect();

                            let _ = update_tx.send(UiUpdate::SelectedEnvironment {
                                index: state.editing_environment_index.map_or(-1, |i| i as i32),
                                name: editing_env.name.clone(),
                                variables,
                            });
                        }
                    }
                }

                UiCommand::EnvironmentVariableChanged {
                    index,
                    name,
                    value,
                    enabled,
                    is_secret,
                } => {
                    if let Some(ref mut editing_env) = state.editing_environment {
                        // Get the old key at this index using our tracked order
                        if let Some(old_key) = state.editing_variable_keys.get(index as usize).cloned() {
                            // Remove old entry if name changed
                            if old_key != name {
                                editing_env.variables.remove(&old_key);
                                // Update the tracked key
                                if let Some(key) = state.editing_variable_keys.get_mut(index as usize) {
                                    *key = name.clone();
                                }
                            }

                            // Insert/update the variable
                            editing_env.variables.insert(
                                name,
                                Variable {
                                    value,
                                    enabled,
                                    secret: is_secret,
                                },
                            );
                        }
                    }
                }

                UiCommand::UrlChanged { url } => {
                    // Skip re-parsing if we're updating from params (to avoid circular update)
                    if state.updating_url_from_params {
                        state.updating_url_from_params = false;
                        state.current_url = url.clone();
                        resolve_and_update_url(&state, &update_tx);
                        continue;
                    }

                    state.current_url = url.clone();

                    // Sprint 05: Sync query params from URL (only when user edits URL directly)
                    if let Some(query_start) = url.find('?') {
                        state.base_url = url[..query_start].to_string();
                        let query_string = &url[query_start + 1..];

                        // Parse query params
                        state.query_params = query_string
                            .split('&')
                            .filter(|s| !s.is_empty())
                            .map(|param| {
                                let mut parts = param.splitn(2, '=');
                                let key = parts.next().unwrap_or("").to_string();
                                let value = parts.next().unwrap_or("").to_string();
                                QueryParamData {
                                    key,
                                    value,
                                    description: String::new(),
                                    enabled: true,
                                }
                            })
                            .collect();

                        let _ = update_tx.send(UiUpdate::QueryParams(state.query_params.clone()));
                    } else {
                        state.base_url = url;
                        // Only clear if there were params before
                        if !state.query_params.is_empty() {
                            state.query_params.clear();
                            let _ = update_tx.send(UiUpdate::QueryParams(vec![]));
                        }
                    }

                    resolve_and_update_url(&state, &update_tx);
                }

                // Settings commands (Sprint 04)
                UiCommand::ToggleTheme => {
                    state.dark_mode = !state.dark_mode;
                    let _ = update_tx.send(UiUpdate::ThemeMode(state.dark_mode));

                    // Save settings
                    let settings = state.to_settings();
                    let settings_repo = SettingsRepository::new();
                    if let Err(e) = settings_repo.save(&settings).await {
                        eprintln!("Failed to save settings: {e}");
                    }
                }

                UiCommand::SetThemeMode { index } => {
                    let theme = ThemeMode::from_index(index);
                    state.dark_mode = theme.is_dark();
                    state.theme_mode = theme;
                    let _ = update_tx.send(UiUpdate::ThemeMode(state.dark_mode));

                    // Save settings
                    let settings = state.to_settings();
                    let settings_repo = SettingsRepository::new();
                    if let Err(e) = settings_repo.save(&settings).await {
                        eprintln!("Failed to save settings: {e}");
                    }
                }

                UiCommand::SetFontScale { index } => {
                    let font_scale = FontScale::from_index(index);
                    state.font_scale = font_scale;
                    let _ = update_tx.send(UiUpdate::FontScale(font_scale.factor()));

                    // Save settings
                    let settings = state.to_settings();
                    let settings_repo = SettingsRepository::new();
                    if let Err(e) = settings_repo.save(&settings).await {
                        eprintln!("Failed to save settings: {e}");
                    }
                }

                UiCommand::OpenSettings => {
                    let _ = update_tx.send(UiUpdate::ShowSettings(true));
                }

                UiCommand::CloseSettings => {
                    let _ = update_tx.send(UiUpdate::ShowSettings(false));
                }

                // History commands (Sprint 04)
                UiCommand::LoadHistoryItem { id } => {
                    if let Some(entry) = state.history.get(&id).cloned() {
                        // Load the request into the editor
                        let method_index = match entry.method {
                            vortex_domain::request::HttpMethod::Get => 0,
                            vortex_domain::request::HttpMethod::Post => 1,
                            vortex_domain::request::HttpMethod::Put => 2,
                            vortex_domain::request::HttpMethod::Patch => 3,
                            vortex_domain::request::HttpMethod::Delete => 4,
                            vortex_domain::request::HttpMethod::Head => 5,
                            vortex_domain::request::HttpMethod::Options => 6,
                        };

                        // Convert history headers to HeaderData
                        let headers: Vec<HeaderData> = entry.headers
                            .iter()
                            .map(|h| HeaderData {
                                key: h.key.clone(),
                                value: h.value.clone(),
                                description: String::new(),
                                enabled: h.enabled,
                            })
                            .collect();

                        // Convert history params to QueryParamData
                        let query_params: Vec<QueryParamData> = entry.params
                            .iter()
                            .map(|p| QueryParamData {
                                key: p.key.clone(),
                                value: p.value.clone(),
                                description: String::new(),
                                enabled: p.enabled,
                            })
                            .collect();

                        // Convert history auth to AuthData
                        let auth = entry.auth.as_ref().map(|a| AuthData {
                            auth_type: a.auth_type,
                            bearer_token: a.bearer_token.clone(),
                            basic_username: a.basic_username.clone(),
                            basic_password: a.basic_password.clone(),
                            api_key_name: a.api_key_name.clone(),
                            api_key_value: a.api_key_value.clone(),
                            api_key_location: a.api_key_location,
                        }).unwrap_or_default();

                        // Update state
                        state.current_url = entry.url.clone();
                        state.request_headers = headers.clone();
                        state.query_params = query_params.clone();
                        state.auth_data = auth.clone();

                        // Send full request data to UI
                        let _ = update_tx.send(UiUpdate::LoadFullRequest {
                            url: entry.url.clone(),
                            method: method_index,
                            body: entry.body.clone().unwrap_or_default(),
                            headers,
                            query_params,
                            auth,
                        });

                        resolve_and_update_url(&state, &update_tx);
                    }
                }

                UiCommand::ClearHistory => {
                    state.history.clear();
                    let _ = update_tx.send(UiUpdate::HistoryItems(vec![]));

                    // Save cleared history
                    let history_repo = HistoryRepository::new();
                    if let Err(e) = history_repo.save(&state.history).await {
                        eprintln!("Failed to save history: {e}");
                    }
                }

                UiCommand::ToggleHistoryVisibility => {
                    state.history_visible = !state.history_visible;
                    let _ = update_tx.send(UiUpdate::HistoryVisible(state.history_visible));

                    // Save visibility preference
                    let settings = state.to_settings();
                    let settings_repo = SettingsRepository::new();
                    if let Err(e) = settings_repo.save(&settings).await {
                        eprintln!("Failed to save settings: {e}");
                    }
                }

                // Sprint 05: Query Parameters commands
                UiCommand::AddQueryParam => {
                    state.query_params.push(QueryParamData {
                        key: String::new(),
                        value: String::new(),
                        description: String::new(),
                        enabled: true,
                    });
                    let _ = update_tx.send(UiUpdate::QueryParams(state.query_params.clone()));
                    update_url_from_params(&mut state, &update_tx);
                }

                UiCommand::DeleteQueryParam { index } => {
                    if (index as usize) < state.query_params.len() {
                        state.query_params.remove(index as usize);
                        let _ = update_tx.send(UiUpdate::QueryParams(state.query_params.clone()));
                        update_url_from_params(&mut state, &update_tx);
                    }
                }

                UiCommand::QueryParamChanged { index, key, value, description, enabled } => {
                    if let Some(param) = state.query_params.get_mut(index as usize) {
                        param.key = key;
                        param.value = value;
                        param.description = description;
                        param.enabled = enabled;
                        // Don't send QueryParams back to avoid rebuilding UI and losing focus
                        // Only update the URL which doesn't affect the input focus
                        update_url_from_params(&mut state, &update_tx);
                    }
                }

                // Sprint 05: Request Headers commands
                UiCommand::AddRequestHeader => {
                    state.request_headers.push(HeaderData {
                        key: String::new(),
                        value: String::new(),
                        description: String::new(),
                        enabled: true,
                    });
                    let _ = update_tx.send(UiUpdate::RequestHeaders(state.request_headers.clone()));
                }

                UiCommand::DeleteRequestHeader { index } => {
                    if (index as usize) < state.request_headers.len() {
                        state.request_headers.remove(index as usize);
                        let _ = update_tx.send(UiUpdate::RequestHeaders(state.request_headers.clone()));
                    }
                }

                UiCommand::RequestHeaderChanged { index, key, value, description, enabled } => {
                    if let Some(header) = state.request_headers.get_mut(index as usize) {
                        header.key = key;
                        header.value = value;
                        header.description = description;
                        header.enabled = enabled;
                    }
                }

                // Sprint 05: Authentication commands
                UiCommand::AuthTypeChanged { auth_type } => {
                    state.auth_data.auth_type = auth_type;
                }

                UiCommand::BearerTokenChanged { token } => {
                    state.auth_data.bearer_token = token;
                }

                UiCommand::BasicCredentialsChanged { username, password } => {
                    state.auth_data.basic_username = username;
                    state.auth_data.basic_password = password;
                }

                UiCommand::ApiKeyChanged { key_name, key_value, location } => {
                    state.auth_data.api_key_name = key_name;
                    state.auth_data.api_key_value = key_value;
                    state.auth_data.api_key_location = location;
                }

                // Sprint 05: Collection Management commands
                UiCommand::SaveCurrentRequest => {
                    if let Some(ref active_id) = state.active_tab_id.clone()
                        && let Some(tab) = state.tabs.iter_mut().find(|t| &t.id == active_id) {
                            if let Some(ref file_path) = tab.file_path.clone() {
                                // Build and save the request
                                let mut saved_request = build_saved_request_from_tab(tab);

                                // Try to preserve the original ID if we can read the existing file
                                if let Ok(existing_content) = std::fs::read_to_string(file_path)
                                    && let Ok(existing_request) = serde_json::from_str::<SavedRequest>(&existing_content) {
                                        saved_request.id = existing_request.id;
                                    }

                                match to_json_stable(&saved_request) {
                                    Ok(json) => {
                                        if let Err(e) = tokio::fs::write(&file_path, json).await {
                                            let _ = update_tx.send(UiUpdate::Error {
                                                title: "Failed to save request".to_string(),
                                                message: e.to_string(),
                                            });
                                        } else {
                                            // Mark tab as saved
                                            tab.has_unsaved_changes = false;
                                            let _ = update_tx.send(UiUpdate::TabsUpdated(state.tabs_to_ui()));
                                        }
                                    }
                                    Err(e) => {
                                        let _ = update_tx.send(UiUpdate::Error {
                                            title: "Failed to serialize request".to_string(),
                                            message: e.to_string(),
                                        });
                                    }
                                }
                            } else {
                                // No file path - need to create a new file
                                // For now, notify the user
                                let _ = update_tx.send(UiUpdate::Error {
                                    title: "Cannot save".to_string(),
                                    message: "This request has no file path. Use 'New Request' in a collection first.".to_string(),
                                });
                            }
                        }
                }

                UiCommand::RenameItem { id, new_name } => {
                    // Rename the item (file or folder)
                    let path = PathBuf::from(&id);
                    if path.exists() {
                        let parent = path.parent().unwrap_or(&path);
                        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
                        let new_path = if ext.is_empty() {
                            parent.join(&new_name)
                        } else {
                            parent.join(format!("{new_name}.{ext}"))
                        };

                        if let Err(e) = tokio::fs::rename(&path, &new_path).await {
                            let _ = update_tx.send(UiUpdate::Error {
                                title: "Failed to rename".to_string(),
                                message: e.to_string(),
                            });
                        } else {
                            // Refresh tree
                            if let Some(ref ws_path) = state.workspace_path {
                                let items = load_workspace_tree(ws_path, &state.expanded_folders).await;
                                let _ = update_tx.send(UiUpdate::CollectionItems(items));
                            }
                        }
                    }
                }

                UiCommand::DeleteItemRequested { id, item_type } => {
                    state.pending_delete_id = Some(id.clone());
                    state.pending_delete_type = Some(item_type.clone());

                    let name = PathBuf::from(&id)
                        .file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or("item")
                        .to_string();

                    let _ = update_tx.send(UiUpdate::ShowConfirmDialog {
                        title: format!("Delete {}", if item_type == "request" { "Request" } else { "Collection" }),
                        message: format!("Are you sure you want to delete '{name}'? This action cannot be undone."),
                        item_id: id,
                        item_type,
                    });
                }

                UiCommand::ConfirmDelete => {
                    if let Some(ref id) = state.pending_delete_id.take() {
                        let path = PathBuf::from(id);
                        if path.is_file() {
                            if let Err(e) = tokio::fs::remove_file(&path).await {
                                let _ = update_tx.send(UiUpdate::Error {
                                    title: "Failed to delete".to_string(),
                                    message: e.to_string(),
                                });
                            }
                        } else if path.is_dir()
                            && let Err(e) = tokio::fs::remove_dir_all(&path).await {
                                let _ = update_tx.send(UiUpdate::Error {
                                    title: "Failed to delete".to_string(),
                                    message: e.to_string(),
                                });
                            }

                        // Refresh tree
                        if let Some(ref ws_path) = state.workspace_path {
                            let items = load_workspace_tree(ws_path, &state.expanded_folders).await;
                            let _ = update_tx.send(UiUpdate::CollectionItems(items));
                        }
                    }
                    state.pending_delete_type = None;
                    let _ = update_tx.send(UiUpdate::HideConfirmDialog);
                }

                UiCommand::CancelDelete => {
                    state.pending_delete_id = None;
                    state.pending_delete_type = None;
                    let _ = update_tx.send(UiUpdate::HideConfirmDialog);
                }

                // --- Sprint 06: Tab Commands ---
                UiCommand::TabClicked { id } => {
                    // Save current tab state before switching
                    if state.active_tab_id.is_some() {
                        // Get current UI values via oneshot channel
                        let (data_tx, mut data_rx) = tokio::sync::oneshot::channel::<(String, i32, String)>();
                        let ui_weak_clone = ui_weak.clone();
                        let _ = slint::invoke_from_event_loop(move || {
                            if let Some(ui) = ui_weak_clone.upgrade() {
                                let url = ui.get_url().to_string();
                                let method = ui.get_method_index();
                                let body = ui.get_request_body().to_string();
                                let _ = data_tx.send((url, method, body));
                            }
                        });

                        if let Ok(Ok((url, method, body))) = tokio::time::timeout(
                            std::time::Duration::from_millis(50),
                            &mut data_rx,
                        ).await {
                            state.save_current_tab_state(&url, method, &body);
                        }
                    }

                    // Switch to new tab
                    state.active_tab_id = Some(id.clone());

                    // Restore tab state to UI - clone tab data first to avoid borrow issues
                    let tab_data = state.get_tab_state(&id).cloned();
                    if let Some(tab) = tab_data {
                        state.current_url = tab.url.clone();
                        state.base_url = tab.url.split('?').next().unwrap_or("").to_string();
                        state.query_params = tab.query_params.clone();
                        state.request_headers = tab.headers.clone();
                        state.auth_data = tab.auth.clone();

                        let _ = update_tx.send(UiUpdate::LoadFullRequest {
                            url: tab.url.clone(),
                            method: tab.method,
                            body: tab.body.clone(),
                            headers: tab.headers.clone(),
                            query_params: tab.query_params.clone(),
                            auth: tab.auth.clone(),
                        });

                        // Restore response state for this tab
                        let _ = update_tx.send(UiUpdate::RestoreResponseState {
                            state: tab.response_state,
                            body: tab.response_body.clone(),
                            status_code: tab.status_code,
                            status_text: tab.status_text.clone(),
                            duration: tab.duration.clone(),
                            size: tab.size.clone(),
                            headers: tab.response_headers.clone(),
                            error_title: tab.error_title.clone(),
                            error_message: tab.error_message.clone(),
                        });

                        resolve_and_update_url(&state, &update_tx);
                    }

                    let _ = update_tx.send(UiUpdate::ActiveTabChanged(id));
                }

                UiCommand::TabCloseClicked { id } => {
                    // Remove the tab
                    state.tabs.retain(|t| t.id != id);

                    // If we closed the active tab, switch to another
                    if state.active_tab_id.as_ref() == Some(&id) {
                        state.active_tab_id = state.tabs.first().map(|t| t.id.clone());

                        if let Some(ref new_active) = state.active_tab_id.clone() {
                            let tab_data = state.get_tab_state(new_active).cloned();
                            if let Some(tab) = tab_data {
                                state.current_url = tab.url.clone();
                                state.query_params = tab.query_params.clone();
                                state.request_headers = tab.headers.clone();
                                state.auth_data = tab.auth.clone();

                                let _ = update_tx.send(UiUpdate::LoadFullRequest {
                                    url: tab.url.clone(),
                                    method: tab.method,
                                    body: tab.body.clone(),
                                    headers: tab.headers.clone(),
                                    query_params: tab.query_params.clone(),
                                    auth: tab.auth.clone(),
                                });

                                // Restore response state
                                let _ = update_tx.send(UiUpdate::RestoreResponseState {
                                    state: tab.response_state,
                                    body: tab.response_body.clone(),
                                    status_code: tab.status_code,
                                    status_text: tab.status_text.clone(),
                                    duration: tab.duration.clone(),
                                    size: tab.size.clone(),
                                    headers: tab.response_headers.clone(),
                                    error_title: tab.error_title.clone(),
                                    error_message: tab.error_message.clone(),
                                });
                            }
                            let _ = update_tx.send(UiUpdate::ActiveTabChanged(new_active.clone()));
                        }
                    }

                    let _ = update_tx.send(UiUpdate::TabsUpdated(state.tabs_to_ui()));
                }

                UiCommand::NewTabClicked => {
                    let new_tab = TabState::new_empty();
                    let new_id = new_tab.id.clone();
                    state.tabs.push(new_tab);
                    state.active_tab_id = Some(new_id.clone());

                    // Clear UI for new tab
                    state.current_url.clear();
                    state.base_url.clear();
                    state.query_params.clear();
                    state.request_headers.clear();
                    state.auth_data = AuthData::default();

                    let _ = update_tx.send(UiUpdate::LoadFullRequest {
                        url: String::new(),
                        method: 0,
                        body: String::new(),
                        headers: vec![],
                        query_params: vec![],
                        auth: AuthData::default(),
                    });

                    let _ = update_tx.send(UiUpdate::TabsUpdated(state.tabs_to_ui()));
                    let _ = update_tx.send(UiUpdate::ActiveTabChanged(new_id));
                }

                // --- Sprint 06: Quick Search Commands ---
                UiCommand::OpenQuickSearch => {
                    let _ = update_tx.send(UiUpdate::ShowQuickSearch(true));
                    // Send current cached results
                    let _ = update_tx.send(UiUpdate::SearchResults(state.all_requests.clone()));
                }

                UiCommand::CloseQuickSearch => {
                    let _ = update_tx.send(UiUpdate::ShowQuickSearch(false));
                }

                UiCommand::SearchQueryChanged { query } => {
                    // Filter requests based on query
                    let query_lower = query.to_lowercase();
                    let results: Vec<SearchResultData> = if query.is_empty() {
                        state.all_requests.clone()
                    } else {
                        state.all_requests
                            .iter()
                            .filter(|r| {
                                r.name.to_lowercase().contains(&query_lower)
                                    || r.url.to_lowercase().contains(&query_lower)
                                    || r.method.to_lowercase().contains(&query_lower)
                            })
                            .cloned()
                            .collect()
                    };
                    let _ = update_tx.send(UiUpdate::SearchResults(results));
                }

                UiCommand::SearchResultClicked { id: _, path } => {
                    // Load the request into a new tab
                    let path = PathBuf::from(&path);
                    if path.extension().is_some_and(|e| e == "json")
                        && let Ok(content) = tokio::fs::read_to_string(&path).await
                            && let Ok(request) = from_json::<vortex_domain::persistence::SavedRequest>(&content) {
                                // Create new tab with this request
                                let method_index = match request.method {
                                    PersistenceHttpMethod::Get => 0,
                                    PersistenceHttpMethod::Post => 1,
                                    PersistenceHttpMethod::Put => 2,
                                    PersistenceHttpMethod::Patch => 3,
                                    PersistenceHttpMethod::Delete => 4,
                                    PersistenceHttpMethod::Head => 5,
                                    PersistenceHttpMethod::Options => 6,
                                    PersistenceHttpMethod::Trace => 0,
                                };

                                let body = request.body.as_ref().map(|b| match b {
                                    vortex_domain::persistence::PersistenceRequestBody::Json { content } => content.to_string(),
                                    vortex_domain::persistence::PersistenceRequestBody::Text { content } => content.clone(),
                                    _ => String::new(),
                                }).unwrap_or_default();

                                let headers: Vec<HeaderData> = request.headers.iter()
                                    .map(|(k, v)| HeaderData { key: k.clone(), value: v.clone(), description: String::new(), enabled: true })
                                    .collect();

                                let query_params: Vec<QueryParamData> = request.query_params.iter()
                                    .map(|(k, v)| QueryParamData { key: k.clone(), value: v.clone(), description: String::new(), enabled: true })
                                    .collect();

                                let new_tab = TabState {
                                    id: uuid::Uuid::now_v7().to_string(),
                                    name: request.name.clone(),
                                    method: method_index,
                                    url: request.url.clone(),
                                    body: body.clone(),
                                    headers: headers.clone(),
                                    query_params: query_params.clone(),
                                    auth: persistence_auth_to_ui(request.auth.as_ref()),
                                    has_unsaved_changes: false,
                                    file_path: Some(path.display().to_string()),
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
                                };

                                let new_id = new_tab.id.clone();
                                state.tabs.push(new_tab);
                                state.active_tab_id = Some(new_id.clone());

                                state.current_url = request.url.clone();
                                state.query_params = query_params.clone();
                                state.request_headers = headers.clone();
                                state.auth_data = persistence_auth_to_ui(request.auth.as_ref());

                                let _ = update_tx.send(UiUpdate::LoadFullRequest {
                                    url: request.url,
                                    method: method_index,
                                    body,
                                    headers,
                                    query_params,
                                    auth: persistence_auth_to_ui(request.auth.as_ref()),
                                });

                                let _ = update_tx.send(UiUpdate::TabsUpdated(state.tabs_to_ui()));
                                let _ = update_tx.send(UiUpdate::ActiveTabChanged(new_id));
                                let _ = update_tx.send(UiUpdate::ShowQuickSearch(false));

                                resolve_and_update_url(&state, &update_tx);
                            }
                }

                // --- Sprint 06: Import/Export Commands ---
                UiCommand::ImportCollection => {
                    if let Some(ref ws_path) = state.workspace_path.clone() {
                        // Open file dialog to select Postman collection
                        let ws = ws_path.clone();
                        let tx = update_tx.clone();
                        let cmd_tx_refresh = cmd_tx.clone();
                        std::thread::spawn(move || {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Import Postman Collection")
                                .add_filter("JSON files", &["json"])
                                .pick_file()
                            {
                                // Read and parse Postman collection in a blocking context
                                if let Ok(content) = std::fs::read_to_string(&path) {
                                    match import_postman_collection(&content, &ws) {
                                        Ok(name) => {
                                            let _ = tx.send(UiUpdate::ImportComplete { collection_name: name });
                                            // Trigger tree refresh
                                            let _ = cmd_tx_refresh.send(UiCommand::RefreshTree);
                                        }
                                        Err(e) => {
                                            let _ = tx.send(UiUpdate::Error {
                                                title: "Import Failed".to_string(),
                                                message: e,
                                            });
                                        }
                                    }
                                }
                            }
                        });
                    }
                }

                UiCommand::ExportCollection => {
                    if let Some(ref ws_path) = state.workspace_path.clone() {
                        let ws = ws_path.clone();
                        let tx = update_tx.clone();
                        std::thread::spawn(move || {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Export Collection")
                                .add_filter("JSON files", &["json"])
                                .set_file_name("vortex_collection.json")
                                .save_file()
                            {
                                match export_vortex_collection(&ws, &path) {
                                    Ok(()) => {
                                        let _ = tx.send(UiUpdate::ExportComplete {
                                            path: path.display().to_string(),
                                        });
                                    }
                                    Err(e) => {
                                        let _ = tx.send(UiUpdate::Error {
                                            title: "Export Failed".to_string(),
                                            message: e,
                                        });
                                    }
                                }
                            }
                        });
                    }
                }

                UiCommand::ExportAsCurl => {
                    // Generate cURL command from current request
                    let curl = generate_curl_command(&state);
                    let _ = update_tx.send(UiUpdate::CurlExport(curl));
                }

                // --- Sprint 06: JSON Format Commands ---
                UiCommand::FormatResponseBody => {
                    if !state.response_body.is_empty()
                        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&state.response_body)
                            && let Ok(formatted) = serde_json::to_string_pretty(&parsed) {
                                state.response_body = formatted.clone();
                                let _ = update_tx.send(UiUpdate::FormattedResponseBody(formatted));
                            }
                }

                UiCommand::FormatRequestBody { body } => {
                    if !body.is_empty()
                        && let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&body)
                            && let Ok(formatted) = serde_json::to_string_pretty(&parsed) {
                                let _ = update_tx.send(UiUpdate::FormattedRequestBody(formatted));
                            }
                }

                UiCommand::CopyFormattedResponse => {
                    // Format and then the UI should copy it
                    if !state.response_body.is_empty() {
                        let formatted = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(&state.response_body) {
                            serde_json::to_string_pretty(&parsed).unwrap_or_else(|_| state.response_body.clone())
                        } else {
                            state.response_body.clone()
                        };
                        let _ = update_tx.send(UiUpdate::CurlExport(formatted)); // Reuse for clipboard
                    }
                }

                UiCommand::RefreshTree => {
                    // Refresh the collection tree
                    if let Some(ref ws_path) = state.workspace_path {
                        let items = load_workspace_tree(ws_path, &state.expanded_folders).await;
                        let _ = update_tx.send(UiUpdate::CollectionItems(items));
                    }
                }

                UiCommand::RefreshEnvironments => {
                    // Refresh the environments list
                    if let Some(ws_path) = state.workspace_path.clone() {
                        load_environments(&ws_path, &mut state, &update_tx).await;
                    }
                }

                UiCommand::ImportEnvironment => {
                    if let Some(ref ws_path) = state.workspace_path.clone() {
                        let ws = ws_path.clone();
                        let tx = update_tx.clone();
                        let cmd_tx_refresh = cmd_tx.clone();
                        std::thread::spawn(move || {
                            if let Some(path) = rfd::FileDialog::new()
                                .set_title("Import Postman Environment")
                                .add_filter("JSON files", &["json"])
                                .pick_file()
                                && let Ok(content) = std::fs::read_to_string(&path) {
                                    match import_postman_environment(&content, &ws) {
                                        Ok(name) => {
                                            let _ = tx.send(UiUpdate::Error {
                                                title: "Import Successful".to_string(),
                                                message: format!("Environment '{name}' imported successfully."),
                                            });
                                            // Refresh environments
                                            let _ = cmd_tx_refresh.send(UiCommand::RefreshEnvironments);
                                        }
                                        Err(e) => {
                                            let _ = tx.send(UiUpdate::Error {
                                                title: "Import Failed".to_string(),
                                                message: e,
                                            });
                                        }
                                    }
                                }
                        });
                    }
                }

                // --- Sprint 04: Import Dialog Commands ---
                UiCommand::ImportBrowseFile => {
                    let tx = update_tx.clone();
                    std::thread::spawn(move || {
                        if let Some(path) = rfd::FileDialog::new()
                            .set_title("Select Postman File to Import")
                            .add_filter("JSON files", &["json"])
                            .pick_file()
                        {
                            let _ = tx.send(UiUpdate::ImportFileSelected {
                                file_path: path.display().to_string(),
                            });
                        }
                    });
                }

                UiCommand::ImportStart { file_path } => {
                    // Check if this is a confirmation of the current import
                    let is_confirmation = match &state.import_file_path {
                        Some(current_path) => current_path == &file_path && state.import_preview_done,
                        None => false,
                    };

                    if is_confirmation {
                        // PHASE 2: EXECUTE IMPORT
                        if let Some(ref ws_path) = state.workspace_path.clone() {
                            let ws = ws_path.clone();
                            let tx = update_tx.clone();
                            let cmd_tx_refresh = cmd_tx.clone();
                            let file = file_path.clone();

                            // Reset state
                            state.import_file_path = None;
                            state.import_preview_done = false;

                            std::thread::spawn(move || {
                                // Read file again (fresh read for import)
                                let content = match std::fs::read_to_string(&file) {
                                    Ok(c) => c,
                                    Err(e) => {
                                        let _ = tx.send(UiUpdate::ImportError {
                                            message: format!("Failed to read file: {e}"),
                                        });
                                        return;
                                    }
                                };

                                let importer = PostmanImporter::new();

                                // Send initial progress
                                let _ = tx.send(UiUpdate::ImportProgress(0.1));

                                match importer.import_collection(&content, &ws) {
                                    Ok(result) => {
                                        let _ = tx.send(UiUpdate::ImportProgress(1.0));
                                        let _ = tx.send(UiUpdate::ImportDialogComplete {
                                            name: result.name,
                                            requests_imported: result.requests_imported,
                                            folders_imported: result.folders_imported,
                                            variables_imported: result.variables_imported,
                                        });
                                        // Refresh tree and environments
                                        let _ = cmd_tx_refresh.send(UiCommand::RefreshTree);
                                        let _ = cmd_tx_refresh.send(UiCommand::RefreshEnvironments);
                                    }
                                    Err(e) => {
                                        let _ = tx.send(UiUpdate::ImportError {
                                            message: e.to_string(),
                                        });
                                    }
                                }
                            });
                        }
                    } else {
                        // PHASE 1: PREVIEW
                        // Update state to track we are previewing this file
                        state.import_file_path = Some(file_path.clone());
                        state.import_preview_done = true;

                        let tx = update_tx.clone();
                        let file = file_path.clone();

                        // Set UI state to Validating immediately
                        let _ = tx.send(UiUpdate::ImportValidating);

                        std::thread::spawn(move || {
                            // Read file
                            let content = match std::fs::read_to_string(&file) {
                                Ok(c) => c,
                                Err(e) => {
                                    let _ = tx.send(UiUpdate::ImportError {
                                        message: format!("Failed to read file: {e}"),
                                    });
                                    return;
                                }
                            };

                            // Create importer
                            let importer = PostmanImporter::new();

                            // Validate
                            let validation = importer.validate_file(&content);
                            if !validation.is_valid {
                                let _ = tx.send(UiUpdate::ImportError {
                                    message: validation.issues.join(", "),
                                });
                                return;
                            }

                            // Preview
                            match importer.preview(&content) {
                                Ok(preview) => {
                                    let warnings: Vec<ImportWarningData> = preview.warnings.iter()
                                        .map(|w| ImportWarningData {
                                            path: w.path.clone(),
                                            message: w.message.clone(),
                                            severity: w.severity.to_string(),
                                        })
                                        .collect();

                                    let _ = tx.send(UiUpdate::ImportPreview {
                                        format: preview.format,
                                        collection_name: preview.collection_name,
                                        environment_name: preview.environment_name,
                                        request_count: preview.request_count,
                                        folder_count: preview.folder_count,
                                        variable_count: preview.variable_count,
                                        warnings,
                                    });
                                }
                                Err(e) => {
                                    let _ = tx.send(UiUpdate::ImportError {
                                        message: e.to_string(),
                                    });
                                }
                            }
                        });
                    }
                }

                UiCommand::ImportCancel => {
                    // Nothing to do - UI handles closing the dialog
                }
            }
        }
    });
}

/// Resolves variables in the current URL and sends the update.
fn resolve_and_update_url(state: &AppState, update_tx: &mpsc::UnboundedSender<UiUpdate>) {
    if state.current_url.is_empty() {
        let _ = update_tx.send(UiUpdate::ResolvedUrl {
            resolved: String::new(),
            has_unresolved: false,
            unresolved_names: vec![],
        });
        return;
    }

    let context = state.build_resolution_context();
    let mut resolver = VariableResolver::new(context);
    let result = resolver.resolve(&state.current_url);

    let _ = update_tx.send(UiUpdate::ResolvedUrl {
        resolved: result.resolved,
        has_unresolved: !result.unresolved.is_empty(),
        unresolved_names: result.unresolved,
    });
}

/// Updates the URL based on query parameters (Sprint 05).
fn update_url_from_params(state: &mut AppState, update_tx: &mpsc::UnboundedSender<UiUpdate>) {
    // Build query string from enabled params
    let query_parts: Vec<String> = state
        .query_params
        .iter()
        .filter(|p| p.enabled && !p.key.is_empty())
        .map(|p| {
            if p.value.is_empty() {
                p.key.clone()
            } else {
                format!("{}={}", p.key, p.value)
            }
        })
        .collect();

    // Update current_url with new query string
    let base = if state.base_url.is_empty() {
        // Extract base URL from current_url if not set
        state
            .current_url
            .split('?')
            .next()
            .unwrap_or("")
            .to_string()
    } else {
        state.base_url.clone()
    };

    // Store the base URL for future updates
    if state.base_url.is_empty() && !base.is_empty() {
        state.base_url = base.clone();
    }

    state.current_url = if query_parts.is_empty() {
        base
    } else {
        format!("{}?{}", base, query_parts.join("&"))
    };

    // Set flag to prevent circular update when URL changes
    state.updating_url_from_params = true;

    // Update the URL in the UI
    let _ = update_tx.send(UiUpdate::UpdateUrl(state.current_url.clone()));

    // Update the resolved URL preview
    resolve_and_update_url(state, update_tx);
}

/// Loads environments from the workspace.
async fn load_environments(
    workspace_path: &PathBuf,
    state: &mut AppState,
    update_tx: &mpsc::UnboundedSender<UiUpdate>,
) {
    let env_repo = FileEnvironmentRepository::new(TokioFileSystem);

    match env_repo.list(workspace_path).await {
        Ok(env_names) => {
            state.environments.clear();

            for name in &env_names {
                if let Ok(env) = env_repo.load(workspace_path, name).await {
                    state.environments.push(env);
                }
            }

            let names: Vec<String> = state.environments.iter().map(|e| e.name.clone()).collect();
            let _ = update_tx.send(UiUpdate::EnvironmentNames(names));

            // Select first environment if available
            if !state.environments.is_empty() {
                state.current_environment_index = Some(0);
                let _ = update_tx.send(UiUpdate::CurrentEnvironmentIndex(0));
            }
        }
        Err(e) => {
            eprintln!("Failed to load environments: {e}");
            let _ = update_tx.send(UiUpdate::EnvironmentNames(vec![]));
        }
    }
}

/// Handles the `SendRequest` command.
/// Result of a request execution for history tracking and tab state saving.
struct RequestResult {
    method: HttpMethod,
    url: String,
    status_code: Option<u16>,
    duration_ms: Option<u64>,
    // Response data for tab state
    response_state: i32, // 2=Success, 3=Error
    response_body: String,
    status_text: String,
    duration_display: String,
    size_display: String,
    response_headers: Vec<crate::bridge::ResponseHeaderData>,
    error_title: String,
    error_message: String,
    // Request body for history
    request_body: String,
}

async fn handle_send_request(
    ui_weak: &slint::Weak<MainWindow>,
    execute_request: &ExecuteRequest<ReqwestHttpClient>,
    update_tx: &mpsc::UnboundedSender<UiUpdate>,
    current_cancel: &mut Option<CancellationToken>,
    state: &AppState,
) -> Option<RequestResult> {
    // Get current request data from UI
    let (data_tx, mut data_rx) = tokio::sync::oneshot::channel::<(String, i32, String)>();

    let _ = ui_weak.upgrade_in_event_loop(move |ui| {
        let url = ui.get_url().to_string();
        let method_index = ui.get_method_index();
        let body = ui.get_request_body().to_string();
        let _ = data_tx.send((url, method_index, body));
    });

    // Wait a bit for the UI to respond
    let request_data = tokio::time::timeout(std::time::Duration::from_millis(100), &mut data_rx)
        .await
        .ok()
        .and_then(std::result::Result::ok);

    if let Some((url, method_index, body)) = request_data {
        // Resolve variables in URL and body
        let context = state.build_resolution_context();
        let mut resolver = VariableResolver::new(context);

        let resolved_url = resolver.resolve(&url).resolved;
        let resolved_body = resolver.resolve(&body).resolved;

        // Create request spec
        let method = match method_index {
            0 => HttpMethod::Get,
            1 => HttpMethod::Post,
            2 => HttpMethod::Put,
            3 => HttpMethod::Patch,
            4 => HttpMethod::Delete,
            5 => HttpMethod::Head,
            6 => HttpMethod::Options,
            _ => HttpMethod::Get,
        };

        // Save body for history before it's moved
        let request_body_for_history = resolved_body.clone();
        let request_body = if method.has_body() && !resolved_body.is_empty() {
            RequestBody::json(resolved_body)
        } else {
            RequestBody::none()
        };

        let mut request = RequestSpec::new("UI Request");
        request.method = method;
        request.url = resolved_url.clone();
        request.body = request_body;

        // Sprint 05: Add custom headers
        for header in &state.request_headers {
            if header.enabled && !header.key.is_empty() {
                let resolved_value = resolver.resolve(&header.value).resolved;
                request.headers.add(vortex_domain::request::Header::new(
                    header.key.clone(),
                    resolved_value,
                ));
            }
        }

        // Sprint 05: Add authentication headers
        match state.auth_data.auth_type {
            1 => {
                // Bearer token
                if !state.auth_data.bearer_token.is_empty() {
                    let resolved_token = resolver.resolve(&state.auth_data.bearer_token).resolved;
                    request.headers.add(vortex_domain::request::Header::new(
                        "Authorization",
                        format!("Bearer {resolved_token}"),
                    ));
                }
            }
            2 => {
                // Basic auth
                if !state.auth_data.basic_username.is_empty() {
                    let resolved_username =
                        resolver.resolve(&state.auth_data.basic_username).resolved;
                    let resolved_password =
                        resolver.resolve(&state.auth_data.basic_password).resolved;
                    let credentials = format!("{resolved_username}:{resolved_password}");
                    use base64::Engine;
                    let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
                    request.headers.add(vortex_domain::request::Header::new(
                        "Authorization",
                        format!("Basic {encoded}"),
                    ));
                }
            }
            3 => {
                // API Key
                if !state.auth_data.api_key_name.is_empty()
                    && !state.auth_data.api_key_value.is_empty()
                {
                    let resolved_value = resolver.resolve(&state.auth_data.api_key_value).resolved;
                    if state.auth_data.api_key_location == 0 {
                        // Header
                        request.headers.add(vortex_domain::request::Header::new(
                            state.auth_data.api_key_name.clone(),
                            resolved_value,
                        ));
                    } else {
                        // Query param - append to URL
                        let separator = if resolved_url.contains('?') { "&" } else { "?" };
                        request.url = format!(
                            "{}{}{}={}",
                            request.url, separator, state.auth_data.api_key_name, resolved_value
                        );
                    }
                }
            }
            _ => {}
        }

        // Update UI to loading state
        let _ = update_tx.send(UiUpdate::State(RequestState::loading()));

        // Create cancellation token
        let (cancel_token, cancel_receiver) = CancellationToken::new();
        *current_cancel = Some(cancel_token);

        // Execute request with cancellation support
        let result = execute_request
            .execute_with_cancellation(&request, cancel_receiver)
            .await;

        // Extract response data for history and tab state
        let (
            status_code,
            duration_ms,
            response_state,
            response_body,
            status_text,
            duration_display,
            size_display,
            response_headers,
            error_title,
            error_message,
        ) = match &result {
            Ok(response) => {
                let headers: Vec<crate::bridge::ResponseHeaderData> = response
                    .headers_map
                    .iter()
                    .map(|(name, value)| crate::bridge::ResponseHeaderData {
                        name: name.clone(),
                        value: value.clone(),
                    })
                    .collect();

                (
                    Some(response.status),
                    Some(response.duration.as_millis() as u64),
                    2, // Success
                    response.body_as_string_lossy(),
                    response.status_text.clone(),
                    response.duration_display(),
                    response.size_display(),
                    headers,
                    String::new(),
                    String::new(),
                )
            }
            Err(e) => {
                let (title, message) = ("Request Failed".to_string(), e.to_string());
                (
                    None,
                    None,
                    3, // Error
                    String::new(),
                    String::new(),
                    String::new(),
                    String::new(),
                    Vec::new(),
                    title,
                    message,
                )
            }
        };

        // Send response headers to UI
        let _ = update_tx.send(UiUpdate::ResponseHeaders(response_headers.clone()));

        // Convert result to RequestState
        let request_state = result.to_request_state();

        // Update UI with result
        let _ = update_tx.send(UiUpdate::State(request_state));

        // Clear cancellation token
        *current_cancel = None;

        // Return result for history tracking and tab state saving
        return Some(RequestResult {
            method,
            url: resolved_url,
            status_code,
            duration_ms,
            response_state,
            response_body,
            status_text,
            duration_display,
            size_display,
            response_headers,
            error_title,
            error_message,
            request_body: request_body_for_history,
        });
    }

    None
}

/// Loads the workspace tree from disk.
async fn load_workspace_tree(
    workspace_path: &PathBuf,
    expanded_folders: &std::collections::HashSet<String>,
) -> Vec<TreeItemData> {
    let mut items = Vec::new();
    let collections_dir = workspace_path.join("collections");

    if let Ok(mut entries) = tokio::fs::read_dir(&collections_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                // This is a collection
                let collection_id = path.display().to_string();
                let collection_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown")
                    .to_string();

                let is_expanded = expanded_folders.contains(&collection_id);

                items.push(TreeItemData {
                    id: collection_id.clone(),
                    name: collection_name,
                    item_type: "collection".to_string(),
                    method: String::new(),
                    depth: 0,
                    expanded: is_expanded,
                    path: path.display().to_string(),
                });

                // If expanded, load children
                if is_expanded {
                    load_folder_items(&path, 1, expanded_folders, &mut items).await;
                }
            }
        }
    }

    items
}

/// Recursively loads folder items.
fn load_folder_items<'a>(
    folder_path: &'a std::path::Path,
    depth: i32,
    expanded_folders: &'a std::collections::HashSet<String>,
    items: &'a mut Vec<TreeItemData>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        // Try "requests" first, then "request", then the folder itself for flexibility
        let requests_dir = folder_path.join("requests");
        let request_dir = folder_path.join("request");

        // Try to read from "requests" directory first, then "request", then folder itself
        let dir_to_read = if tokio::fs::metadata(&requests_dir).await.is_ok() {
            requests_dir
        } else if tokio::fs::metadata(&request_dir).await.is_ok() {
            request_dir
        } else {
            // For subfolders (from Postman import), read directly from the folder
            folder_path.to_path_buf()
        };

        if let Ok(mut entries) = tokio::fs::read_dir(&dir_to_read).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();

                if path.is_dir() {
                    // This is a subfolder
                    let folder_id = path.display().to_string();

                    // Try to read display name from folder.json, fallback to filesystem name
                    let folder_meta_path = path.join("folder.json");
                    let folder_name =
                        if let Ok(content) = tokio::fs::read_to_string(&folder_meta_path).await {
                            if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                                json.get("name")
                                    .and_then(|n| n.as_str())
                                    .unwrap_or_else(|| {
                                        path.file_name()
                                            .and_then(|n| n.to_str())
                                            .unwrap_or("Unknown")
                                    })
                                    .to_string()
                            } else {
                                path.file_name()
                                    .and_then(|n| n.to_str())
                                    .unwrap_or("Unknown")
                                    .to_string()
                            }
                        } else {
                            path.file_name()
                                .and_then(|n| n.to_str())
                                .unwrap_or("Unknown")
                                .to_string()
                        };

                    let is_expanded = expanded_folders.contains(&folder_id);

                    items.push(TreeItemData {
                        id: folder_id.clone(),
                        name: folder_name,
                        item_type: "folder".to_string(),
                        method: String::new(),
                        depth,
                        expanded: is_expanded,
                        path: path.display().to_string(),
                    });

                    if is_expanded {
                        // Load subfolder contents (check for requests subfolder or direct json files)
                        load_folder_items(&path, depth + 1, expanded_folders, items).await;
                    }
                } else if path.extension().is_some_and(|e| e == "json") {
                    // Skip collection.json and folder.json metadata files
                    if path
                        .file_name()
                        .is_some_and(|n| n == "collection.json" || n == "folder.json")
                    {
                        continue;
                    }

                    // This is a request file
                    let fallback_name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    // Try to read the name and method from the file
                    let (name, method) = if let Ok(content) = tokio::fs::read_to_string(&path).await
                    {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                            let name = json
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or(&fallback_name)
                                .to_string();
                            let method = json
                                .get("method")
                                .and_then(|m| m.as_str())
                                .unwrap_or("GET")
                                .to_string();
                            (name, method)
                        } else {
                            (fallback_name.clone(), "GET".to_string())
                        }
                    } else {
                        (fallback_name.clone(), "GET".to_string())
                    };

                    items.push(TreeItemData {
                        id: path.display().to_string(),
                        name,
                        item_type: "request".to_string(),
                        method,
                        depth,
                        expanded: false,
                        path: path.display().to_string(),
                    });
                }
            }
        }
    })
}

/// Applies a UI update to the Slint window.
fn apply_update(ui: &MainWindow, update: UiUpdate) {
    match update {
        UiUpdate::State(state) => {
            match state {
                RequestState::Idle => {
                    ui.set_response_state(0);
                }
                RequestState::Loading { .. } => {
                    ui.set_response_state(1);
                    ui.set_elapsed_time("0ms".into());
                }
                RequestState::Success { response } => {
                    ui.set_response_state(2);
                    #[allow(clippy::cast_possible_wrap)]
                    ui.set_status_code(i32::from(response.status));
                    ui.set_status_text(response.status_text.clone().into());
                    ui.set_response_body(response.body_as_string_lossy().into());
                    ui.set_duration(response.duration_display().into());
                    ui.set_size(response.size_display().into());
                }
                RequestState::Error {
                    kind,
                    message,
                    details,
                } => {
                    ui.set_response_state(3);
                    ui.set_error_title(kind.title().into());
                    ui.set_error_message(details.unwrap_or(message).into());

                    // Convert suggestions to Slint model
                    let suggestions: Vec<SharedString> =
                        kind.suggestions().iter().map(|s| (*s).into()).collect();
                    let model: ModelRc<SharedString> = Rc::new(VecModel::from(suggestions)).into();
                    ui.set_error_suggestions(model);
                }
            }
        }

        UiUpdate::ElapsedTime(elapsed) => {
            ui.set_elapsed_time(elapsed.into());
        }

        UiUpdate::WorkspacePath(path) => {
            ui.set_workspace_path(path.into());
        }

        UiUpdate::CollectionItems(items) => {
            let slint_items: Vec<TreeItem> = items
                .into_iter()
                .map(|item| TreeItem {
                    id: item.id.into(),
                    name: item.name.into(),
                    item_type: item.item_type.into(),
                    method: item.method.into(),
                    depth: item.depth,
                    expanded: item.expanded,
                    path: item.path.into(),
                })
                .collect();

            let model: ModelRc<TreeItem> = Rc::new(VecModel::from(slint_items)).into();
            ui.set_collection_items(model);
        }

        UiUpdate::Error { title, message } => {
            eprintln!("Error: {title}: {message}");
            // TODO: Show error dialog in UI
        }

        UiUpdate::SavingState(is_saving) => {
            ui.set_is_saving(is_saving);
        }

        UiUpdate::LoadRequest { url, method, body } => {
            ui.set_url(url.into());
            ui.set_method_index(method);
            ui.set_request_body(body.into());
        }

        // Environment updates (Sprint 03)
        UiUpdate::EnvironmentNames(names) => {
            let model: ModelRc<SharedString> = Rc::new(VecModel::from(
                names
                    .into_iter()
                    .map(SharedString::from)
                    .collect::<Vec<_>>(),
            ))
            .into();
            ui.set_environment_names(model);
        }

        UiUpdate::CurrentEnvironmentIndex(index) => {
            ui.set_current_environment_index(index);
        }

        UiUpdate::ResolvedUrl {
            resolved,
            has_unresolved,
            unresolved_names,
        } => {
            ui.set_resolved_url(resolved.into());
            ui.set_has_unresolved_variables(has_unresolved);
            let unresolved_model: ModelRc<SharedString> = Rc::new(VecModel::from(
                unresolved_names
                    .into_iter()
                    .map(SharedString::from)
                    .collect::<Vec<_>>(),
            ))
            .into();
            ui.set_unresolved_variables(unresolved_model);
        }

        UiUpdate::EnvironmentList(envs) => {
            let slint_envs: Vec<EnvironmentInfo> = envs
                .into_iter()
                .map(|env| EnvironmentInfo {
                    id: env.id.into(),
                    name: env.name.into(),
                    variable_count: env.variable_count,
                })
                .collect();

            let model: ModelRc<EnvironmentInfo> = Rc::new(VecModel::from(slint_envs)).into();
            ui.set_environment_list(model);
        }

        UiUpdate::SelectedEnvironment {
            index,
            name,
            variables,
        } => {
            ui.set_env_manager_selected_index(index);
            ui.set_env_manager_selected_name(name.into());

            let slint_vars: Vec<VariableRow> = variables
                .into_iter()
                .map(|var| VariableRow {
                    name: var.name.into(),
                    value: var.value.into(),
                    enabled: var.enabled,
                    is_secret: var.is_secret,
                })
                .collect();

            let model: ModelRc<VariableRow> = Rc::new(VecModel::from(slint_vars)).into();
            ui.set_env_manager_variables(model);
        }

        UiUpdate::ShowEnvironmentManager(show) => {
            ui.set_show_environment_manager(show);
        }

        // Settings updates (Sprint 04)
        UiUpdate::ThemeMode(dark_mode) => {
            ui.global::<VortexPalette>().set_dark_mode(dark_mode);
        }

        UiUpdate::FontScale(scale) => {
            ui.global::<VortexTypography>().set_scale_factor(scale);
        }

        UiUpdate::ShowSettings(show) => {
            ui.set_show_settings(show);
        }

        UiUpdate::SettingsLoaded {
            theme_index,
            font_scale_index,
            dark_mode,
            font_scale_factor,
        } => {
            ui.set_theme_mode_index(theme_index);
            ui.set_font_scale_index(font_scale_index);
            ui.global::<VortexPalette>().set_dark_mode(dark_mode);
            ui.global::<VortexTypography>()
                .set_scale_factor(font_scale_factor);
        }

        // History updates (Sprint 04)
        UiUpdate::HistoryItems(items) => {
            let slint_items: Vec<HistoryItem> = items
                .into_iter()
                .map(|item| HistoryItem {
                    id: item.id.into(),
                    method: item.method.into(),
                    url: item.url.into(),
                    status_code: item.status_code,
                    time_ago: item.time_ago.into(),
                    duration: item.duration.into(),
                })
                .collect();

            let model: ModelRc<HistoryItem> = Rc::new(VecModel::from(slint_items)).into();
            ui.set_history_items(model);
        }

        UiUpdate::HistoryVisible(visible) => {
            ui.set_history_visible(visible);
        }

        // Sprint 05: URL update (from params sync)
        UiUpdate::UpdateUrl(url) => {
            ui.set_url(url.into());
        }

        // Sprint 05: Query params updates
        UiUpdate::QueryParams(params) => {
            let slint_params: Vec<QueryParam> = params
                .into_iter()
                .map(|p| QueryParam {
                    key: p.key.into(),
                    value: p.value.into(),
                    description: p.description.into(),
                    enabled: p.enabled,
                })
                .collect();

            let model: ModelRc<QueryParam> = Rc::new(VecModel::from(slint_params)).into();
            ui.set_query_params(model);
        }

        // Sprint 05: Request headers updates
        UiUpdate::RequestHeaders(headers) => {
            let slint_headers: Vec<HeaderRow> = headers
                .into_iter()
                .map(|h| HeaderRow {
                    key: h.key.into(),
                    value: h.value.into(),
                    description: h.description.into(),
                    enabled: h.enabled,
                })
                .collect();

            let model: ModelRc<HeaderRow> = Rc::new(VecModel::from(slint_headers)).into();
            ui.set_request_headers(model);
        }

        // Sprint 05: Response headers updates
        UiUpdate::ResponseHeaders(headers) => {
            let slint_headers: Vec<ResponseHeader> = headers
                .into_iter()
                .map(|h| ResponseHeader {
                    name: h.name.into(),
                    value: h.value.into(),
                })
                .collect();

            let model: ModelRc<ResponseHeader> = Rc::new(VecModel::from(slint_headers)).into();
            ui.set_response_headers(model);
        }

        // Sprint 05: Auth data updates
        UiUpdate::AuthData(auth) => {
            ui.set_auth_type(auth.auth_type);
            ui.set_auth_bearer_token(auth.bearer_token.into());
            ui.set_auth_basic_username(auth.basic_username.into());
            ui.set_auth_basic_password(auth.basic_password.into());
            ui.set_auth_api_key_name(auth.api_key_name.into());
            ui.set_auth_api_key_value(auth.api_key_value.into());
            ui.set_auth_api_key_location(auth.api_key_location);
        }

        // Sprint 05: Confirm dialog updates
        UiUpdate::ShowConfirmDialog {
            title,
            message,
            item_id,
            item_type,
        } => {
            ui.set_confirm_dialog_title(title.into());
            ui.set_confirm_dialog_message(message.into());
            ui.set_pending_delete_id(item_id.into());
            ui.set_pending_delete_type(item_type.into());
            ui.set_show_confirm_dialog(true);
        }

        UiUpdate::HideConfirmDialog => {
            ui.set_show_confirm_dialog(false);
        }

        // Sprint 05: Load full request (with headers, params, auth)
        UiUpdate::LoadFullRequest {
            url,
            method,
            body,
            headers,
            query_params,
            auth,
        } => {
            ui.set_url(url.into());
            ui.set_method_index(method);
            ui.set_request_body(body.into());

            // Update headers
            let slint_headers: Vec<HeaderRow> = headers
                .into_iter()
                .map(|h| HeaderRow {
                    key: h.key.into(),
                    value: h.value.into(),
                    description: h.description.into(),
                    enabled: h.enabled,
                })
                .collect();
            let headers_model: ModelRc<HeaderRow> = Rc::new(VecModel::from(slint_headers)).into();
            ui.set_request_headers(headers_model);

            // Update query params
            let slint_params: Vec<QueryParam> = query_params
                .into_iter()
                .map(|p| QueryParam {
                    key: p.key.into(),
                    value: p.value.into(),
                    description: p.description.into(),
                    enabled: p.enabled,
                })
                .collect();
            let params_model: ModelRc<QueryParam> = Rc::new(VecModel::from(slint_params)).into();
            ui.set_query_params(params_model);

            // Update auth
            ui.set_auth_type(auth.auth_type);
            ui.set_auth_bearer_token(auth.bearer_token.into());
            ui.set_auth_basic_username(auth.basic_username.into());
            ui.set_auth_basic_password(auth.basic_password.into());
            ui.set_auth_api_key_name(auth.api_key_name.into());
            ui.set_auth_api_key_value(auth.api_key_value.into());
            ui.set_auth_api_key_location(auth.api_key_location);
        }

        // Sprint 06: Tab updates
        UiUpdate::TabsUpdated(tabs) => {
            let slint_tabs: Vec<RequestTab> = tabs
                .into_iter()
                .map(|t| RequestTab {
                    id: t.id.into(),
                    name: t.name.into(),
                    method: t.method.into(),
                    has_unsaved_changes: t.has_unsaved_changes,
                })
                .collect();

            let model: ModelRc<RequestTab> = Rc::new(VecModel::from(slint_tabs)).into();
            ui.set_request_tabs(model);
        }

        UiUpdate::ActiveTabChanged(id) => {
            ui.set_active_tab_id(id.into());
        }

        // Sprint 06: Search updates
        UiUpdate::SearchResults(results) => {
            let slint_results: Vec<SearchResult> = results
                .into_iter()
                .map(|r| SearchResult {
                    id: r.id.into(),
                    name: r.name.into(),
                    method: r.method.into(),
                    url: r.url.into(),
                    collection_name: r.collection_name.into(),
                    path: r.path.into(),
                })
                .collect();

            let model: ModelRc<SearchResult> = Rc::new(VecModel::from(slint_results)).into();
            ui.set_search_results(model);
        }

        UiUpdate::ShowQuickSearch(visible) => {
            ui.set_show_quick_search(visible);
            if visible {
                ui.set_search_query("".into());
            }
        }

        // Sprint 06: Import/Export updates
        UiUpdate::ImportComplete { collection_name } => {
            // Show success - could also refresh tree
            eprintln!("Imported collection: {collection_name}");
        }

        // --- Sprint 04: Import Dialog Updates ---
        UiUpdate::ImportFileSelected { file_path } => {
            ui.set_import_selected_file(file_path.into());
            // Don't set state to Validating here - let user click Import button
            // The Import button triggers validation, then shows preview
        }

        UiUpdate::ImportValidating => {
            ui.set_import_state(ImportState::Validating);
        }

        UiUpdate::ImportPreview {
            format,
            collection_name,
            environment_name,
            request_count,
            folder_count,
            variable_count,
            warnings,
        } => {
            // Set preview data
            ui.set_import_preview(ImportPreviewData {
                format: format.into(),
                collection_name: collection_name.unwrap_or_default().into(),
                environment_name: environment_name.unwrap_or_default().into(),
                request_count: request_count as i32,
                folder_count: folder_count as i32,
                variable_count: variable_count as i32,
            });

            // Convert warnings to Slint model
            let slint_warnings: Vec<ImportWarningItem> = warnings
                .into_iter()
                .map(|w| ImportWarningItem {
                    path: w.path.into(),
                    message: w.message.into(),
                    severity: w.severity.into(),
                })
                .collect();
            let model: ModelRc<ImportWarningItem> = Rc::new(VecModel::from(slint_warnings)).into();
            ui.set_import_warnings(model);

            ui.set_import_state(ImportState::Previewing);
        }

        UiUpdate::ImportProgress(progress) => {
            ui.set_import_progress(progress);
        }

        UiUpdate::ImportDialogComplete {
            name,
            requests_imported,
            folders_imported,
            variables_imported,
        } => {
            eprintln!(
                "Import complete: {name} ({requests_imported} requests, {folders_imported} folders, {variables_imported} variables)"
            );
            ui.set_import_state(ImportState::Complete);
        }

        UiUpdate::ImportError { message } => {
            ui.set_import_error_message(message.into());
            ui.set_import_state(ImportState::Error);
        }

        UiUpdate::ExportComplete { path } => {
            eprintln!("Exported to: {path}");
        }

        UiUpdate::CurlExport(curl) => {
            // Copy to clipboard (platform-specific)
            #[cfg(target_os = "macos")]
            {
                use std::process::Command;
                let _ = Command::new("pbcopy")
                    .stdin(std::process::Stdio::piped())
                    .spawn()
                    .and_then(|mut child| {
                        use std::io::Write;
                        if let Some(ref mut stdin) = child.stdin {
                            let _ = stdin.write_all(curl.as_bytes());
                        }
                        child.wait()
                    });
            }
            eprintln!("cURL command copied to clipboard");
        }

        // Sprint 06: JSON formatting updates
        UiUpdate::FormattedResponseBody(body) => {
            ui.set_response_body(body.into());
        }

        UiUpdate::FormattedRequestBody(body) => {
            ui.set_request_body(body.into());
        }

        // Restore response state when switching tabs
        UiUpdate::RestoreResponseState {
            state,
            body,
            status_code,
            status_text,
            duration,
            size,
            headers,
            error_title,
            error_message,
        } => {
            ui.set_response_state(state);
            ui.set_response_body(body.into());
            ui.set_status_code(status_code);
            ui.set_status_text(status_text.into());
            ui.set_duration(duration.into());
            ui.set_size(size.into());
            ui.set_error_title(error_title.into());
            ui.set_error_message(error_message.into());

            // Convert headers to Slint model
            let slint_headers: Vec<ResponseHeader> = headers
                .into_iter()
                .map(|h| ResponseHeader {
                    name: h.name.into(),
                    value: h.value.into(),
                })
                .collect();
            let model: ModelRc<ResponseHeader> = Rc::new(VecModel::from(slint_headers)).into();
            ui.set_response_headers(model);
        }
    }
}

// --- Sprint 06: Import/Export Helper Functions ---

/// Import a Postman collection v2.1 format.
/// Also auto-detects and imports Postman environments.
fn import_postman_collection(content: &str, workspace_path: &PathBuf) -> Result<String, String> {
    // Parse Postman collection JSON
    let collection: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("Invalid JSON: {e}"))?;

    // Auto-detect: If this is an environment file (has "name" and "values" but no "info"),
    // redirect to environment import
    if collection.get("info").is_none() {
        if collection.get("name").is_some() && collection.get("values").is_some() {
            return import_postman_environment(content, workspace_path);
        }
        return Err("Invalid Postman file: Missing 'info' field for collection, or 'name'/'values' fields for environment".to_string());
    }

    // Get collection info
    let info = collection
        .get("info")
        .ok_or("Missing 'info' field in Postman collection")?;

    let collection_name = info
        .get("name")
        .and_then(|n| n.as_str())
        .unwrap_or("Imported Collection")
        .to_string();

    // Create collection directory
    let safe_name = collection_name.to_lowercase().replace(' ', "-");
    let collection_dir = workspace_path.join("collections").join(&safe_name);
    std::fs::create_dir_all(&collection_dir)
        .map_err(|e| format!("Failed to create directory: {e}"))?;

    // Create requests directory
    let requests_dir = collection_dir.join("request");
    std::fs::create_dir_all(&requests_dir)
        .map_err(|e| format!("Failed to create requests directory: {e}"))?;

    // Create collection.json
    let coll_meta = serde_json::json!({
        "id": uuid::Uuid::now_v7().to_string(),
        "name": collection_name,
        "schema_version": 1,
    });
    std::fs::write(
        collection_dir.join("collection.json"),
        serde_json::to_string_pretty(&coll_meta).unwrap_or_default(),
    )
    .map_err(|e| format!("Failed to write collection.json: {e}"))?;

    // Import items
    if let Some(items) = collection.get("item").and_then(|i| i.as_array()) {
        import_postman_items(items, &requests_dir)?;
    }

    Ok(collection_name)
}

/// Recursively import Postman items.
fn import_postman_items(items: &[serde_json::Value], target_dir: &PathBuf) -> Result<(), String> {
    eprintln!(
        "[IMPORT] Processing {} items in {}",
        items.len(),
        target_dir.display()
    );

    for item in items {
        let name = item
            .get("name")
            .and_then(|n| n.as_str())
            .unwrap_or("Unnamed");

        // Check if this is a folder or a request
        if item.get("item").is_some() {
            // This is a folder - create subdirectory and recurse
            let safe_name = name.to_lowercase().replace(' ', "-");
            let subfolder = target_dir.join(&safe_name);
            eprintln!("[IMPORT] Folder: {name} -> {}", subfolder.display());
            std::fs::create_dir_all(&subfolder)
                .map_err(|e| format!("Failed to create folder '{name}': {e}"))?;

            // Create folder.json with display name
            let folder_meta = serde_json::json!({
                "name": name,
                "schema_version": 1,
            });
            std::fs::write(
                subfolder.join("folder.json"),
                serde_json::to_string_pretty(&folder_meta).unwrap_or_default(),
            )
            .map_err(|e| format!("Failed to write folder.json for '{name}': {e}"))?;

            if let Some(sub_items) = item.get("item").and_then(|i| i.as_array()) {
                import_postman_items(sub_items, &subfolder)?;
            }
        } else if let Some(request) = item.get("request") {
            // This is a request
            let method = request
                .get("method")
                .and_then(|m| m.as_str())
                .unwrap_or("GET");

            let url = extract_postman_url(request.get("url"));

            // Extract headers
            let headers: std::collections::BTreeMap<String, String> = request
                .get("header")
                .and_then(|h| h.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|h| {
                            let key = h.get("key")?.as_str()?;
                            let value = h.get("value")?.as_str()?;
                            Some((key.to_string(), value.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();

            // Extract body from Postman format
            let body = request
                .get("body")
                .and_then(|body_obj| {
                    let mode = body_obj.get("mode")?.as_str()?;
                    match mode {
                        "raw" => body_obj
                            .get("raw")
                            .and_then(|r| r.as_str())
                            .map(std::string::ToString::to_string),
                        "urlencoded" => {
                            // Convert form-urlencoded to string representation
                            body_obj
                                .get("urlencoded")
                                .and_then(|arr| arr.as_array())
                                .map(|items| {
                                    items
                                        .iter()
                                        .filter_map(|item| {
                                            let key = item.get("key")?.as_str()?;
                                            let value = item.get("value")?.as_str().unwrap_or("");
                                            Some(format!("{key}={value}"))
                                        })
                                        .collect::<Vec<_>>()
                                        .join("&")
                                })
                        }
                        "formdata" => {
                            // Convert form-data to JSON representation for display
                            body_obj
                                .get("formdata")
                                .and_then(|arr| arr.as_array())
                                .map(|items| {
                                    let obj: serde_json::Map<String, serde_json::Value> = items
                                        .iter()
                                        .filter_map(|item| {
                                            let key = item.get("key")?.as_str()?.to_string();
                                            let value = item
                                                .get("value")?
                                                .as_str()
                                                .unwrap_or("")
                                                .to_string();
                                            Some((key, serde_json::Value::String(value)))
                                        })
                                        .collect();
                                    serde_json::to_string_pretty(&obj).unwrap_or_default()
                                })
                        }
                        _ => None,
                    }
                })
                .unwrap_or_default();

            // Extract query params from Postman URL as BTreeMap
            let query_params: std::collections::BTreeMap<String, String> = request
                .get("url")
                .and_then(|url_obj| url_obj.get("query"))
                .and_then(|q| q.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|param| {
                            let disabled = param
                                .get("disabled")
                                .and_then(serde_json::Value::as_bool)
                                .unwrap_or(false);
                            if disabled {
                                return None;
                            }
                            let key = param.get("key")?.as_str()?;
                            let value = param.get("value")?.as_str().unwrap_or("");
                            Some((key.to_string(), value.to_string()))
                        })
                        .collect()
                })
                .unwrap_or_default();

            eprintln!(
                "[IMPORT] Request: {} {} (body: {} chars)",
                method,
                name,
                body.len()
            );

            // Create Vortex request
            let mut vortex_request = serde_json::json!({
                "id": uuid::Uuid::now_v7().to_string(),
                "name": name,
                "method": method.to_uppercase(),
                "url": url,
                "headers": headers,
                "schema_version": 1,
            });

            // Add body if present using the correct PersistenceRequestBody format
            if !body.is_empty() {
                // Try to parse as JSON first, fall back to text
                if let Ok(json_content) = serde_json::from_str::<serde_json::Value>(&body) {
                    vortex_request["body"] = serde_json::json!({
                        "type": "json",
                        "content": json_content
                    });
                } else {
                    vortex_request["body"] = serde_json::json!({
                        "type": "text",
                        "content": body
                    });
                }
            }

            // Add query params if present
            if !query_params.is_empty() {
                vortex_request["query_params"] = serde_json::json!(query_params);
            }

            let safe_name = name.to_lowercase().replace([' ', '/'], "-");
            let file_path = target_dir.join(format!("{safe_name}.json"));
            eprintln!("[IMPORT] Writing file: {}", file_path.display());
            std::fs::write(
                &file_path,
                serde_json::to_string_pretty(&vortex_request).unwrap_or_default(),
            )
            .map_err(|e| format!("Failed to write request '{name}': {e}"))?;
        }
    }

    Ok(())
}

/// Extract URL from Postman URL object or string.
fn extract_postman_url(url_value: Option<&serde_json::Value>) -> String {
    match url_value {
        Some(serde_json::Value::String(s)) => s.clone(),
        Some(serde_json::Value::Object(obj)) => obj
            .get("raw")
            .and_then(|r| r.as_str())
            .unwrap_or("")
            .to_string(),
        _ => String::new(),
    }
}

/// Export workspace collection to Vortex JSON format.
fn export_vortex_collection(workspace_path: &PathBuf, output_path: &PathBuf) -> Result<(), String> {
    let collections_dir = workspace_path.join("collections");

    let mut export = serde_json::json!({
        "vortex_version": "0.1.0",
        "export_date": chrono::Utc::now().to_rfc3339(),
        "collections": [],
    });

    // Read all collections
    if let Ok(entries) = std::fs::read_dir(&collections_dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir()
                && let Ok(collection) = export_collection_dir(&path)
                    && let Some(collections) =
                        export.get_mut("collections").and_then(|c| c.as_array_mut())
                    {
                        collections.push(collection);
                    }
        }
    }

    std::fs::write(
        output_path,
        serde_json::to_string_pretty(&export).unwrap_or_default(),
    )
    .map_err(|e| format!("Failed to write export file: {e}"))?;

    Ok(())
}

/// Export a single collection directory.
fn export_collection_dir(collection_path: &PathBuf) -> Result<serde_json::Value, String> {
    let collection_json = collection_path.join("collection.json");
    let collection_meta: serde_json::Value = if collection_json.exists() {
        let content = std::fs::read_to_string(&collection_json)
            .map_err(|e| format!("Failed to read collection.json: {e}"))?;
        serde_json::from_str(&content).unwrap_or_default()
    } else {
        serde_json::json!({
            "name": collection_path.file_name().and_then(|n| n.to_str()).unwrap_or("Unknown"),
        })
    };

    let mut requests = Vec::new();
    let requests_dir = collection_path.join("request");

    if requests_dir.exists() {
        collect_requests(&requests_dir, &mut requests)?;
    }

    Ok(serde_json::json!({
        "info": collection_meta,
        "requests": requests,
    }))
}

/// Recursively collect requests from a directory.
fn collect_requests(dir: &PathBuf, requests: &mut Vec<serde_json::Value>) -> Result<(), String> {
    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|e| e == "json") {
                if let Ok(content) = std::fs::read_to_string(&path)
                    && let Ok(req) = serde_json::from_str::<serde_json::Value>(&content) {
                        requests.push(req);
                    }
            } else if path.is_dir() {
                collect_requests(&path, requests)?;
            }
        }
    }
    Ok(())
}

/// Generate cURL command from current request state.
fn generate_curl_command(state: &AppState) -> String {
    let mut parts = vec!["curl".to_string()];

    // Method
    let method = match state
        .tabs
        .iter()
        .find(|t| state.active_tab_id.as_ref() == Some(&t.id))
        .map_or(0, |t| t.method)
    {
        0 => "GET",
        1 => "POST",
        2 => "PUT",
        3 => "PATCH",
        4 => "DELETE",
        5 => "HEAD",
        6 => "OPTIONS",
        _ => "GET",
    };

    if method != "GET" {
        parts.push(format!("-X {method}"));
    }

    // URL
    parts.push(format!("'{}'", state.current_url));

    // Headers
    for header in &state.request_headers {
        if header.enabled && !header.key.is_empty() {
            parts.push(format!("-H '{}: {}'", header.key, header.value));
        }
    }

    // Auth
    match state.auth_data.auth_type {
        1 => {
            if !state.auth_data.bearer_token.is_empty() {
                parts.push(format!(
                    "-H 'Authorization: Bearer {}'",
                    state.auth_data.bearer_token
                ));
            }
        }
        2 => {
            if !state.auth_data.basic_username.is_empty() {
                parts.push(format!(
                    "-u '{}:{}'",
                    state.auth_data.basic_username, state.auth_data.basic_password
                ));
            }
        }
        3 => {
            if !state.auth_data.api_key_name.is_empty() && state.auth_data.api_key_location == 0 {
                parts.push(format!(
                    "-H '{}: {}'",
                    state.auth_data.api_key_name, state.auth_data.api_key_value
                ));
            }
        }
        _ => {}
    }

    // Body
    if let Some(tab) = state
        .tabs
        .iter()
        .find(|t| state.active_tab_id.as_ref() == Some(&t.id))
        && !tab.body.is_empty() && (method == "POST" || method == "PUT" || method == "PATCH") {
            parts.push(format!("-d '{}'", tab.body.replace('\'', "'\\''")));
        }

    parts.join(" \\\n  ")
}

/// Load all requests from workspace for quick search.
async fn load_all_requests_for_search(workspace_path: &PathBuf) -> Vec<SearchResultData> {
    let mut results = Vec::new();
    let collections_dir = workspace_path.join("collections");

    if let Ok(mut entries) = tokio::fs::read_dir(&collections_dir).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if path.is_dir() {
                let collection_name = path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .unwrap_or("Unknown")
                    .to_string();

                // Scan for requests in this collection
                collect_requests_for_search(&path, &collection_name, &mut results).await;
            }
        }
    }

    results
}

/// Recursively collect requests for search.
fn collect_requests_for_search<'a>(
    dir: &'a std::path::Path,
    collection_name: &'a str,
    results: &'a mut Vec<SearchResultData>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'a>> {
    Box::pin(async move {
        // Check requests subdirectory
        let requests_dir = dir.join("request");
        let dir_to_scan = if tokio::fs::metadata(&requests_dir).await.is_ok() {
            requests_dir
        } else {
            dir.to_path_buf()
        };

        if let Ok(mut entries) = tokio::fs::read_dir(&dir_to_scan).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();

                if path.is_dir() {
                    // Recurse into subdirectories
                    collect_requests_for_search(&path, collection_name, results).await;
                } else if path.extension().is_some_and(|e| e == "json") {
                    // This is a request file
                    if let Ok(content) = tokio::fs::read_to_string(&path).await
                        && let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                            let name = json
                                .get("name")
                                .and_then(|n| n.as_str())
                                .unwrap_or_else(|| {
                                    path.file_stem()
                                        .and_then(|s| s.to_str())
                                        .unwrap_or("Unknown")
                                })
                                .to_string();

                            let method = json
                                .get("method")
                                .and_then(|m| m.as_str())
                                .unwrap_or("GET")
                                .to_string();

                            let url = json
                                .get("url")
                                .and_then(|u| u.as_str())
                                .unwrap_or("")
                                .to_string();

                            results.push(SearchResultData {
                                id: path.display().to_string(),
                                name,
                                method,
                                url,
                                collection_name: collection_name.to_string(),
                                path: path.display().to_string(),
                            });
                        }
                }
            }
        }
    })
}

/// Import a Postman environment file.
fn import_postman_environment(content: &str, workspace_path: &PathBuf) -> Result<String, String> {
    let env: serde_json::Value =
        serde_json::from_str(content).map_err(|e| format!("Invalid JSON: {e}"))?;

    // Postman environment format has "name" and "values" at the root level
    // (unlike collections which have "info")
    let name = env
        .get("name")
        .and_then(|n| n.as_str())
        .ok_or("Missing 'name' field - this may not be a Postman environment file")?
        .to_string();

    let values = env
        .get("values")
        .and_then(|v| v.as_array())
        .ok_or("Missing 'values' field - this may not be a Postman environment file")?;

    // Create environments directory if it doesn't exist
    let environments_dir = workspace_path.join("environments");
    std::fs::create_dir_all(&environments_dir)
        .map_err(|e| format!("Failed to create environments directory: {e}"))?;

    // Convert Postman variables to Vortex format
    let variables: Vec<serde_json::Value> = values
        .iter()
        .filter_map(|v| {
            let key = v.get("key")?.as_str()?;
            let value = v.get("value")?.as_str().unwrap_or("");
            let enabled = v.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(true);
            let secret = v.get("type").and_then(|t| t.as_str()) == Some("secret");

            Some(serde_json::json!({
                "name": key,
                "value": value,
                "enabled": enabled,
                "is_secret": secret
            }))
        })
        .collect();

    // Create Vortex environment file
    let vortex_env = serde_json::json!({
        "id": uuid::Uuid::now_v7().to_string(),
        "name": name,
        "variables": variables,
        "schema_version": 1,
    });

    let safe_name = name.to_lowercase().replace(' ', "-");
    let file_path = environments_dir.join(format!("{safe_name}.json"));

    std::fs::write(
        &file_path,
        serde_json::to_string_pretty(&vortex_env).unwrap_or_default(),
    )
    .map_err(|e| format!("Failed to write environment file: {e}"))?;

    Ok(name)
}

/// Converts persistence auth configuration to UI-friendly `AuthData`.
fn persistence_auth_to_ui(auth: Option<&PersistenceAuth>) -> AuthData {
    match auth {
        None => AuthData::default(),
        Some(PersistenceAuth::Bearer { token }) => AuthData {
            auth_type: 1,
            bearer_token: token.clone(),
            ..AuthData::default()
        },
        Some(PersistenceAuth::Basic { username, password }) => AuthData {
            auth_type: 2,
            basic_username: username.clone(),
            basic_password: password.clone(),
            ..AuthData::default()
        },
        Some(PersistenceAuth::ApiKey {
            key,
            value,
            location,
        }) => AuthData {
            auth_type: 3,
            api_key_name: key.clone(),
            api_key_value: value.clone(),
            api_key_location: match location {
                ApiKeyLocation::Header => 0,
                ApiKeyLocation::Query => 1,
            },
            ..AuthData::default()
        },
        // OAuth2 types default to None for now (not supported in UI yet)
        Some(PersistenceAuth::Oauth2ClientCredentials { .. } |
PersistenceAuth::Oauth2AuthCode { .. }) => AuthData::default(),
    }
}

/// Converts UI `AuthData` back to persistence format for saving.
fn ui_auth_to_persistence(auth: &AuthData) -> Option<PersistenceAuth> {
    match auth.auth_type {
        0 => None, // No auth
        1 => Some(PersistenceAuth::bearer(&auth.bearer_token)),
        2 => Some(PersistenceAuth::basic(
            &auth.basic_username,
            &auth.basic_password,
        )),
        3 => {
            if auth.api_key_location == 0 {
                Some(PersistenceAuth::api_key_header(
                    &auth.api_key_name,
                    &auth.api_key_value,
                ))
            } else {
                Some(PersistenceAuth::api_key_query(
                    &auth.api_key_name,
                    &auth.api_key_value,
                ))
            }
        }
        _ => None,
    }
}

/// Builds a `SavedRequest` from `TabState` for persistence.
fn build_saved_request_from_tab(tab: &TabState) -> SavedRequest {
    let method = match tab.method {
        0 => PersistenceHttpMethod::Get,
        1 => PersistenceHttpMethod::Post,
        2 => PersistenceHttpMethod::Put,
        3 => PersistenceHttpMethod::Patch,
        4 => PersistenceHttpMethod::Delete,
        5 => PersistenceHttpMethod::Head,
        6 => PersistenceHttpMethod::Options,
        _ => PersistenceHttpMethod::Get,
    };

    let mut request = SavedRequest::new(
        uuid::Uuid::now_v7().to_string(),
        &tab.name,
        method,
        &tab.url,
    );

    // Add headers
    for header in &tab.headers {
        if header.enabled {
            request
                .headers
                .insert(header.key.clone(), header.value.clone());
        }
    }

    // Add query params
    for param in &tab.query_params {
        if param.enabled {
            request
                .query_params
                .insert(param.key.clone(), param.value.clone());
        }
    }

    // Add body if present
    if !tab.body.is_empty() {
        // Try to parse as JSON, otherwise use raw text
        if let Ok(json_value) = serde_json::from_str::<serde_json::Value>(&tab.body) {
            request.body = Some(PersistenceRequestBody::json(json_value));
        } else {
            request.body = Some(PersistenceRequestBody::text(tab.body.clone()));
        }
    }

    // Add auth if present
    request.auth = ui_auth_to_persistence(&tab.auth);

    request
}
