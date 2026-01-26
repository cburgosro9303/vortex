//! Application window management

use slint::ComponentHandle;

use crate::MainWindow;

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
