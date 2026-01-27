//! Application window management
//!
//! This module provides the main application window with all business logic bindings.

use std::rc::Rc;
use std::sync::Arc;

use slint::{ComponentHandle, ModelRc, VecModel};
use tokio::sync::mpsc;
use vortex_application::{CancellationToken, ExecuteRequest, ExecuteResultExt};
use vortex_domain::{
    RequestState,
    request::{HttpMethod, RequestBody, RequestSpec},
};
use vortex_infrastructure::ReqwestHttpClient;

use crate::MainWindow;
use crate::bridge::{UiCommand, UiUpdate};

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

        // Store the command sender in the UI callbacks
        let cmd_tx_send = cmd_tx.clone();
        let cmd_tx_cancel = cmd_tx.clone();

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

/// Runs the async runtime for handling HTTP requests.
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
        let http_client = Arc::new(ReqwestHttpClient::new().expect("Failed to create HTTP client"));
        let execute_request = ExecuteRequest::new(http_client);

        // Track current cancellation token
        let mut current_cancel: Option<CancellationToken> = None;

        while let Some(cmd) = cmd_rx.recv().await {
            match cmd {
                UiCommand::SendRequest => {
                    // Get current request data from UI
                    // We need to use channels because upgrade_in_event_loop returns ()
                    let (data_tx, mut data_rx) =
                        tokio::sync::oneshot::channel::<(String, i32, String)>();

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
                        current_cancel = Some(cancel_token);

                        // Execute request with cancellation support
                        let result = execute_request
                            .execute_with_cancellation(&request, cancel_receiver)
                            .await;

                        // Convert result to RequestState
                        let state = result.to_request_state();

                        // Update UI with result
                        let _ = update_tx.send(UiUpdate::State(state));

                        // Clear cancellation token
                        current_cancel = None;
                    }
                }

                UiCommand::CancelRequest => {
                    if let Some(cancel) = current_cancel.take() {
                        cancel.cancel();
                    }
                }
            }
        }
    });
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
                    let suggestions: Vec<slint::SharedString> =
                        kind.suggestions().iter().map(|s| (*s).into()).collect();
                    let model: ModelRc<slint::SharedString> =
                        Rc::new(VecModel::from(suggestions)).into();
                    ui.set_error_suggestions(model);
                }
            }
        }

        UiUpdate::ElapsedTime(elapsed) => {
            ui.set_elapsed_time(elapsed.into());
        }
    }
}
