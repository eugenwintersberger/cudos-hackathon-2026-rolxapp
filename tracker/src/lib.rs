//! Rolx tracker UI, shared by the desktop and board builds.
//!
//! The embedding binary picks the Slint backend by enabling the `desktop`
//! or `linuxkms` feature of this crate (or via `SLINT_BACKEND` at runtime).

slint::include_modules!();

pub use slint::PlatformError;

/// Create the main window and run the event loop until it is closed.
pub fn run() -> Result<(), PlatformError> {
    let window = AppWindow::new()?;
    window.run()
}
