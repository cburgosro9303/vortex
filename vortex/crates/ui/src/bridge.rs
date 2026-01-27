//! UI Bridge Module
//!
//! Defines the communication protocol between the Slint UI thread
//! and the async Tokio runtime.

use vortex_domain::RequestState;

/// Commands sent from UI to the async runtime.
#[derive(Debug, Clone)]
pub enum UiCommand {
    /// User clicked Send button or pressed Enter.
    SendRequest,

    /// User clicked Cancel button.
    CancelRequest,
}

/// Updates sent from async runtime to the UI.
#[derive(Debug, Clone)]
pub enum UiUpdate {
    /// Update the request state (Idle/Loading/Success/Error).
    State(RequestState),

    /// Update the elapsed time display during loading.
    ElapsedTime(String),
}
