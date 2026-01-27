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
    CancellationToken, CreateWorkspace, CreateWorkspaceInput, EnvironmentRepository, ExecuteRequest,
    ExecuteResultExt, VariableResolver,
    ports::WorkspaceRepository,
};
use vortex_domain::{
    RequestState,
    environment::{Environment, ResolutionContext, Variable, VariableMap},
    persistence::{PersistenceHttpMethod, SavedRequest},
    request::{HttpMethod, RequestBody, RequestSpec},
};
use vortex_domain::{FontScale, HistoryEntry, RequestHistory, ThemeMode, UserSettings};
use vortex_infrastructure::{
    FileEnvironmentRepository, FileSystemWorkspaceRepository, HistoryRepository,
    ReqwestHttpClient, SettingsRepository, TokioFileSystem, from_json,
};

use crate::MainWindow;
use crate::TreeItem;
use crate::EnvironmentInfo;
use crate::HistoryItem;
use crate::VariableRow;
use crate::VortexPalette;
use crate::VortexTypography;
use crate::ResponseHeader;
use crate::QueryParam;
use crate::HeaderRow;
use crate::bridge::{
    AuthData, EnvironmentData, HeaderData, HistoryItemData, QueryParamData, TreeItemData,
    UiCommand, UiUpdate, VariableData,
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

        // Spawn the async runtime in a separate thread
        let ui_weak_async = ui_weak.clone();
        std::thread::spawn(move || {
            run_async_runtime(ui_weak_async, cmd_rx, update_tx);
        });

        // Process UI updates on the main thread using a timer
        let ui_weak_update = ui_weak.clone();
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
        }
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
                status_code: entry.status_code.map_or(0, |s| s as i32),
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
                        // Add to history
                        let entry = if let (Some(status), Some(duration)) = (result.status_code, result.duration_ms) {
                            HistoryEntry::new(result.method, result.url, status, duration, None)
                        } else {
                            HistoryEntry::failed(result.method, result.url, None)
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
                    // Load request into editor
                    if path.extension().map_or(false, |e| e == "json") {
                        if let Ok(content) = tokio::fs::read_to_string(&path).await {
                            if let Ok(request) =
                                from_json::<vortex_domain::persistence::SavedRequest>(&content)
                            {
                                use vortex_domain::persistence::PersistenceHttpMethod;
                                use vortex_domain::persistence::PersistenceRequestBody;

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

                                let body = request
                                    .body
                                    .as_ref()
                                    .map(|b| match b {
                                        PersistenceRequestBody::Json { content } => {
                                            content.to_string()
                                        }
                                        PersistenceRequestBody::Text { content } => content.clone(),
                                        PersistenceRequestBody::Graphql { query, .. } => {
                                            query.clone()
                                        }
                                        _ => String::new(),
                                    })
                                    .unwrap_or_default();

                                state.current_url = request.url.clone();

                                let _ = update_tx.send(UiUpdate::LoadRequest {
                                    url: request.url.clone(),
                                    method: method_index,
                                    body,
                                });

                                // Update resolved URL preview
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
                        if let Ok(mut entries) = tokio::fs::read_dir(&collections_dir).await {
                            if let Ok(Some(first_entry)) = entries.next_entry().await {
                                let collection_path = first_entry.path();
                                if collection_path.is_dir() {
                                    // Create the requests directory if it doesn't exist
                                    let requests_dir = collection_path.join("request");
                                    let _ = tokio::fs::create_dir_all(&requests_dir).await;

                                    // Generate a unique ID and filename
                                    let request_id = uuid::Uuid::new_v4().to_string();
                                    let safe_name = name.to_lowercase().replace(' ', "-");
                                    let file_name = format!("{}.json", safe_name);
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
                }

                UiCommand::SaveCollection => {
                    let _ = update_tx.send(UiUpdate::SavingState(true));
                    // TODO: Implement save
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    let _ = update_tx.send(UiUpdate::SavingState(false));
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
                    if let Some(ref ws_path) = state.workspace_path {
                        if let Some(env) = state.environments.get(index as usize) {
                            let env_repo = FileEnvironmentRepository::new(TokioFileSystem);

                            match env_repo.delete(ws_path, &env.name).await {
                                Ok(()) => {
                                    state.environments.remove(index as usize);

                                    // Reset current environment if it was deleted
                                    if state.current_environment_index == Some(index as usize) {
                                        state.current_environment_index = None;
                                        let _ = update_tx.send(UiUpdate::CurrentEnvironmentIndex(-1));
                                    } else if let Some(current_idx) = state.current_environment_index
                                    {
                                        if current_idx > index as usize {
                                            state.current_environment_index = Some(current_idx - 1);
                                            let _ = update_tx.send(UiUpdate::CurrentEnvironmentIndex(
                                                (current_idx - 1) as i32,
                                            ));
                                        }
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
                    if let Some(ref ws_path) = state.workspace_path {
                        if let Some(ref editing_env) = state.editing_environment {
                            let env_repo = FileEnvironmentRepository::new(TokioFileSystem);

                            match env_repo.save(ws_path, editing_env).await {
                                Ok(()) => {
                                    // Update the environment in state
                                    if let Some(idx) = state.editing_environment_index {
                                        if idx < state.environments.len() {
                                            state.environments[idx] = editing_env.clone();
                                        }
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
                    state.current_url = url.clone();

                    // Sprint 05: Sync query params from URL
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
                    if let Some(entry) = state.history.get(&id) {
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

                        let _ = update_tx.send(UiUpdate::LoadRequest {
                            url: entry.url.clone(),
                            method: method_index,
                            body: String::new(), // History doesn't store body
                        });

                        // Update current URL for variable resolution
                        state.current_url = entry.url.clone();
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

                UiCommand::QueryParamChanged { index, key, value, enabled } => {
                    if let Some(param) = state.query_params.get_mut(index as usize) {
                        param.key = key;
                        param.value = value;
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

                UiCommand::RequestHeaderChanged { index, key, value, enabled } => {
                    if let Some(header) = state.request_headers.get_mut(index as usize) {
                        header.key = key;
                        header.value = value;
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
                    // TODO: Implement save current request to collection
                    eprintln!("SaveCurrentRequest not yet implemented");
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
                            parent.join(format!("{}.{}", new_name, ext))
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
                        message: format!("Are you sure you want to delete '{}'? This action cannot be undone.", name),
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
                        } else if path.is_dir() {
                            if let Err(e) = tokio::fs::remove_dir_all(&path).await {
                                let _ = update_tx.send(UiUpdate::Error {
                                    title: "Failed to delete".to_string(),
                                    message: e.to_string(),
                                });
                            }
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
        state.current_url.split('?').next().unwrap_or("").to_string()
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

/// Handles the SendRequest command.
/// Result of a request execution for history tracking.
struct RequestResult {
    method: HttpMethod,
    url: String,
    status_code: Option<u16>,
    duration_ms: Option<u64>,
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
    let request_data =
        tokio::time::timeout(std::time::Duration::from_millis(100), &mut data_rx)
            .await
            .ok()
            .and_then(|r| r.ok());

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
                        format!("Bearer {}", resolved_token),
                    ));
                }
            }
            2 => {
                // Basic auth
                if !state.auth_data.basic_username.is_empty() {
                    let resolved_username = resolver.resolve(&state.auth_data.basic_username).resolved;
                    let resolved_password = resolver.resolve(&state.auth_data.basic_password).resolved;
                    let credentials = format!("{}:{}", resolved_username, resolved_password);
                    use base64::Engine;
                    let encoded = base64::engine::general_purpose::STANDARD.encode(credentials);
                    request.headers.add(vortex_domain::request::Header::new(
                        "Authorization",
                        format!("Basic {}", encoded),
                    ));
                }
            }
            3 => {
                // API Key
                if !state.auth_data.api_key_name.is_empty() && !state.auth_data.api_key_value.is_empty() {
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

        // Extract status and duration for history
        let (status_code, duration_ms) = match &result {
            Ok(response) => (Some(response.status), Some(response.duration.as_millis() as u64)),
            Err(_) => (None, None),
        };

        // Sprint 05: Extract response headers
        if let Ok(ref response) = result {
            let response_headers: Vec<crate::bridge::ResponseHeaderData> = response
                .headers_map
                .iter()
                .map(|(name, value)| crate::bridge::ResponseHeaderData {
                    name: name.clone(),
                    value: value.clone(),
                })
                .collect();
            let _ = update_tx.send(UiUpdate::ResponseHeaders(response_headers));
        }

        // Convert result to RequestState
        let request_state = result.to_request_state();

        // Update UI with result
        let _ = update_tx.send(UiUpdate::State(request_state));

        // Clear cancellation token
        *current_cancel = None;

        // Return result for history tracking
        return Some(RequestResult {
            method,
            url: resolved_url,
            status_code,
            duration_ms,
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
        // Try "requests" first, then "request" for flexibility
        let requests_dir = folder_path.join("requests");
        let request_dir = folder_path.join("request");

        // Try to read from "requests" directory first, then "request"
        let dir_to_read = if tokio::fs::metadata(&requests_dir).await.is_ok() {
            requests_dir
        } else {
            request_dir
        };

        if let Ok(mut entries) = tokio::fs::read_dir(&dir_to_read).await {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();

                if path.is_dir() {
                    // This is a subfolder
                    let folder_id = path.display().to_string();
                    let folder_name = path
                        .file_name()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();

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
                } else if path.extension().map_or(false, |e| e == "json") {
                    // This is a request file
                    let file_name = path
                        .file_stem()
                        .and_then(|n| n.to_str())
                        .unwrap_or("Unknown")
                        .to_string();

                    // Try to read the method from the file
                    let method = if let Ok(content) = tokio::fs::read_to_string(&path).await {
                        if let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) {
                            json.get("method")
                                .and_then(|m| m.as_str())
                                .unwrap_or("GET")
                                .to_string()
                        } else {
                            "GET".to_string()
                        }
                    } else {
                        "GET".to_string()
                    };

                    items.push(TreeItemData {
                        id: path.display().to_string(),
                        name: file_name,
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
                    ui.set_status_code(response.status as i32);
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
                    let model: ModelRc<SharedString> =
                        Rc::new(VecModel::from(suggestions)).into();
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
            let model: ModelRc<SharedString> =
                Rc::new(VecModel::from(names.into_iter().map(SharedString::from).collect::<Vec<_>>())).into();
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
            let unresolved_model: ModelRc<SharedString> =
                Rc::new(VecModel::from(unresolved_names.into_iter().map(SharedString::from).collect::<Vec<_>>())).into();
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
            ui.global::<VortexTypography>().set_scale_factor(font_scale_factor);
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
        UiUpdate::ShowConfirmDialog { title, message, item_id, item_type } => {
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
        UiUpdate::LoadFullRequest { url, method, body, headers, query_params, auth } => {
            ui.set_url(url.into());
            ui.set_method_index(method);
            ui.set_request_body(body.into());

            // Update headers
            let slint_headers: Vec<HeaderRow> = headers
                .into_iter()
                .map(|h| HeaderRow {
                    key: h.key.into(),
                    value: h.value.into(),
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
    }
}
