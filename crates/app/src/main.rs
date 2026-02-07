//! Vortex API Client - Main Entry Point
//!
//! This is the desktop application entry point that initializes
//! all components and starts the UI event loop.

use vortex_ui::AppWindow;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Initialize the application window
    let app = AppWindow::new()?;

    // Run the event loop (blocks until window closes)
    app.run()?;

    Ok(())
}
