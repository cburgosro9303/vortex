//! Application window management
//!
//! This module provides the main application window with all business logic bindings.

use std::path::PathBuf;
use std::rc::Rc;
use std::sync::Arc;

use slint::{ComponentHandle, ModelRc, SharedString, VecModel};
use tokio::sync::mpsc;
use vortex_application::{
    CancellationToken, CreateWorkspace, CreateWorkspaceInput, ExecuteRequest, ExecuteResultExt,
    ports::WorkspaceRepository,
};
use vortex_domain::{
    RequestState,
    request::{HttpMethod, RequestBody, RequestSpec},
};
use vortex_infrastructure::{
    FileSystemWorkspaceRepository, ReqwestHttpClient, TokioFileSystem, from_json,
};

use crate::MainWindow;
use crate::TreeItem;
use crate::bridge::{TreeItemData, UiCommand, UiUpdate};

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
}

impl AppState {
    fn new() -> Self {
        Self {
            workspace_path: None,
            expanded_folders: std::collections::HashSet::new(),
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

        // Application state
        let mut state = AppState::new();
        let mut current_cancel: Option<CancellationToken> = None;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                UiCommand::SendRequest => {
                    handle_send_request(
                        &ui_weak,
                        &execute_request,
                        &update_tx,
                        &mut current_cancel,
                    )
                    .await;
                }

                UiCommand::CancelRequest => {
                    if let Some(cancel) = current_cancel.take() {
                        cancel.cancel();
                    }
                }

                UiCommand::CreateWorkspace { path, name } => {
                    let create_ws = CreateWorkspace::new(FileSystemWorkspaceRepository::new(TokioFileSystem));
                    match create_ws
                        .execute(CreateWorkspaceInput {
                            path: path.clone(),
                            name,
                        })
                        .await
                    {
                        Ok(_manifest) => {
                            state.workspace_path = Some(path.clone());
                            let _ = update_tx
                                .send(UiUpdate::WorkspacePath(path.display().to_string()));

                            // Load initial tree (empty for new workspace)
                            let _ = update_tx.send(UiUpdate::CollectionItems(vec![]));
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
                    let _ = update_tx.send(UiUpdate::WorkspacePath(String::new()));
                    let _ = update_tx.send(UiUpdate::CollectionItems(vec![]));
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
                            if let Ok(request) = from_json::<vortex_domain::persistence::SavedRequest>(&content) {
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

                                let body = request.body.as_ref()
                                    .map(|b| match b {
                                        PersistenceRequestBody::Json { content } => content.to_string(),
                                        PersistenceRequestBody::Text { content } => content.clone(),
                                        PersistenceRequestBody::Graphql { query, .. } => query.clone(),
                                        _ => String::new(),
                                    })
                                    .unwrap_or_default();

                                let _ = update_tx.send(UiUpdate::LoadRequest {
                                    url: request.url,
                                    method: method_index,
                                    body,
                                });
                            }
                        }
                    }
                }

                UiCommand::CreateRequest { collection_path: _, name: _ } => {
                    // TODO: Implement create request
                    eprintln!("Create request not yet fully implemented");
                }

                UiCommand::SaveCollection => {
                    let _ = update_tx.send(UiUpdate::SavingState(true));
                    // TODO: Implement save
                    tokio::time::sleep(std::time::Duration::from_millis(500)).await;
                    let _ = update_tx.send(UiUpdate::SavingState(false));
                }
            }
        }
    });
}

/// Handles the SendRequest command.
async fn handle_send_request(
    ui_weak: &slint::Weak<MainWindow>,
    execute_request: &ExecuteRequest<ReqwestHttpClient>,
    update_tx: &mpsc::UnboundedSender<UiUpdate>,
    current_cancel: &mut Option<CancellationToken>,
) {
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

        let request_body = if method.has_body() && !body.is_empty() {
            RequestBody::json(body)
        } else {
            RequestBody::none()
        };

        let mut request = RequestSpec::new("UI Request");
        request.method = method;
        request.url = url;
        request.body = request_body;

        // Update UI to loading state
        let _ = update_tx.send(UiUpdate::State(RequestState::loading()));

        // Create cancellation token
        let (cancel_token, cancel_receiver) = CancellationToken::new();
        *current_cancel = Some(cancel_token);

        // Execute request with cancellation support
        let result = execute_request
            .execute_with_cancellation(&request, cancel_receiver)
            .await;

        // Convert result to RequestState
        let state = result.to_request_state();

        // Update UI with result
        let _ = update_tx.send(UiUpdate::State(state));

        // Clear cancellation token
        *current_cancel = None;
    }
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
    }
}
