//! Rolx tracker UI, shared by the desktop and board builds.
//!
//! The embedding binary picks the Slint backend by enabling the `desktop`
//! or `linuxkms` feature of this crate (or via `SLINT_BACKEND` at runtime).

pub mod config;

use std::error::Error;

use config::Config;

slint::include_modules!();

/// Load the configuration, create the main window and run the event loop
/// until it is closed.
pub fn run() -> Result<(), Box<dyn Error>> {
    let config = Config::load()?;

    let window = AppWindow::new()?;
    window.set_admin_configured(config.has_admin_password());
    window.on_verify_admin_password(move |password| config.verify_admin_password(&password));
    window.run()?;
    Ok(())
}
