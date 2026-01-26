//! Vortex UI - User interface layer
//!
//! This crate provides the Slint-based user interface for the Vortex API Client.

// Allow lints that trigger on Slint-generated code which we cannot control
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]

mod app_window;

pub use app_window::AppWindow;

// Include the generated Slint code
slint::include_modules!();
