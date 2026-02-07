//! Vortex UI - User interface layer
//!
//! This crate provides the Slint-based user interface for the Vortex API Client.

// Allow lints that trigger on Slint-generated code which we cannot control
#![allow(clippy::unwrap_used)]
#![allow(clippy::expect_used)]
#![allow(clippy::panic)]
#![allow(clippy::cast_possible_wrap)]
#![allow(clippy::cast_possible_truncation)]
#![allow(clippy::cast_sign_loss)]
#![allow(clippy::assigning_clones)]
#![allow(clippy::match_same_arms)]
#![allow(clippy::option_if_let_else)]
#![allow(clippy::too_many_lines)]
#![allow(clippy::struct_excessive_bools)]
#![allow(clippy::needless_pass_by_value)]
#![allow(clippy::ptr_arg)]
#![allow(clippy::similar_names)]
#![allow(clippy::items_after_statements)]
#![allow(clippy::uninlined_format_args)]

mod app_window;
pub mod bridge;
pub mod state;

pub use app_window::AppWindow;
pub use bridge::{SearchResultData, TabData, TabState, UiCommand, UiUpdate};
pub use state::{CollectionData, CollectionState, FolderData, TreeNode, TreeNodeType};

// Include the generated Slint code
slint::include_modules!();
