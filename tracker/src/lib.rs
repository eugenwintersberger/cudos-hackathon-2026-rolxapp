//! Rolx tracker UI, shared by the desktop and board builds.
//!
//! The embedding binary picks the Slint backend by enabling the `desktop`
//! or `linuxkms` feature of this crate (or via `SLINT_BACKEND` at runtime).

pub mod config;
pub mod connectivity;
pub mod oidc;
pub mod rolx;
pub mod wlan;

use std::{
    cell::RefCell,
    error::Error,
    rc::Rc,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::Duration,
};

use config::{Config, LoginConfig};
use oidc::{DeviceAuthorization, DeviceFlowClient};
use rolx::{Approval, RolxClient};

slint::include_modules!();

/// Render `text` as QR code, ready to be shown as image.
fn qr_code(text: &str) -> Option<slint::SharedPixelBuffer<slint::Rgba8Pixel>> {
    use qrcode::{Color, QrCode};

    const SCALE: usize = 6;
    const QUIET_ZONE: usize = 4;

    let code = QrCode::new(text).ok()?;
    let modules = code.width();
    let side = (modules + 2 * QUIET_ZONE) * SCALE;
    let mut buffer = slint::SharedPixelBuffer::<slint::Rgba8Pixel>::new(side as u32, side as u32);
    let white = slint::Rgba8Pixel::new(255, 255, 255, 255);
    let black = slint::Rgba8Pixel::new(0, 0, 0, 255);
    for (i, pixel) in buffer.make_mut_slice().iter_mut().enumerate() {
        let (x, y) = (i % side / SCALE, i / side / SCALE);
        let dark = x >= QUIET_ZONE
            && y >= QUIET_ZONE
            && x < QUIET_ZONE + modules
            && y < QUIET_ZONE + modules
            && code[(x - QUIET_ZONE, y - QUIET_ZONE)] == Color::Dark;
        *pixel = if dark { black } else { white };
    }
    Some(buffer)
}

/// Load the configuration, create the main window and run the event loop
/// until it is closed.
pub fn run() -> Result<(), Box<dyn Error>> {
    let config = Config::load()?;

    let window = AppWindow::new()?;
    window.set_admin_configured(config.has_admin_password());
    window.set_login_configured(config.login().is_some());
    setup_login(&window, config.login());
    watch_connection(&window, config.server_url.clone());
    if cfg!(feature = "linuxkms") {
        setup_wlan(&window, &config);
    }
    window.on_verify_admin_password(move |password| config.verify_admin_password(&password));
    window.run()?;
    Ok(())
}

/// Keep the `online` property of the window in sync with the reachability of
/// the server. Without a configured server the app counts as offline.
fn watch_connection(window: &AppWindow, server_url: Option<String>) {
    const INTERVAL: Duration = Duration::from_secs(10);
    const TIMEOUT: Duration = Duration::from_secs(3);

    let Some(server_url) = server_url else { return };
    let weak = window.as_weak();
    thread::spawn(move || {
        loop {
            let online = connectivity::is_reachable(&server_url, TIMEOUT);
            // fails once the event loop has ended
            if weak.upgrade_in_event_loop(move |w| w.set_online(online)).is_err() {
                return;
            }
            thread::sleep(INTERVAL);
        }
    });
}

/// Connect the WLAN dialog of the window to the wpa_supplicant configuration.
fn setup_wlan(window: &AppWindow, config: &Config) {
    let path = std::path::PathBuf::from(config.wlan_config_path());
    let restart = config.wlan_restart_command();

    window.set_wlan_available(true);
    window.on_apply_wlan({
        let weak = window.as_weak();
        move |ssid, passphrase| {
            let Some(window) = weak.upgrade() else { return };
            window.set_wlan_state("working".into());

            let weak = weak.clone();
            let (path, restart) = (path.clone(), restart.clone());
            let (ssid, passphrase) = (ssid.to_string(), passphrase.to_string());
            thread::spawn(move || {
                let result = wlan::apply(&path, &restart, &ssid, &passphrase);
                let _ = weak.upgrade_in_event_loop(move |w| match result {
                    Ok(()) => w.set_wlan_state("done".into()),
                    Err(e) => {
                        w.set_wlan_message(e.to_string().into());
                        w.set_wlan_state("failed".into());
                    }
                });
            });
        }
    });
}

/// The session granted by the Rolx server once the user has signed in.
type Session = Arc<Mutex<Option<Approval>>>;

/// Connect the sign-in callbacks of the window to the device flow.
fn setup_login(window: &AppWindow, login: Option<LoginConfig<'_>>) {
    let Some(login) = login else { return };
    let device_flow = Arc::new(DeviceFlowClient::new(login.client_id, login.client_secret));
    let rolx = Arc::new(RolxClient::new(login.server_url));
    let session = Session::default();
    // Cancel flag of the attempt in progress, if any.
    let current: Rc<RefCell<Option<Arc<AtomicBool>>>> = Rc::default();

    window.on_start_login({
        let weak = window.as_weak();
        let current = current.clone();
        move |username| {
            let cancel = Arc::new(AtomicBool::new(false));
            if let Some(previous) = current.borrow_mut().replace(cancel.clone()) {
                previous.store(true, Ordering::Relaxed);
            }
            let Some(window) = weak.upgrade() else { return };
            window.set_auth_state("starting".into());

            let weak = weak.clone();
            let device_flow = device_flow.clone();
            let rolx = rolx.clone();
            let session = session.clone();
            let username = username.trim().to_owned();
            thread::spawn(move || {
                let show_code = |authorization: &DeviceAuthorization| {
                    let url = authorization.verification_url.clone();
                    let code = authorization.user_code.clone();
                    let qr = qr_code(&authorization.complete_url());
                    let _ = weak.upgrade_in_event_loop(move |w| {
                        w.set_verification_url(url.into());
                        w.set_user_code(code.into());
                        if let Some(qr) = qr {
                            w.set_qr_code(slint::Image::from_rgba8(qr));
                        }
                        w.set_auth_state("waiting".into());
                    });
                };
                let result = sign_in(&device_flow, &rolx, &username, &cancel, show_code);
                if cancel.load(Ordering::Relaxed) {
                    return;
                }
                let _ = weak.upgrade_in_event_loop(move |w| match result {
                    Ok(approval) => {
                        *session.lock().unwrap() = Some(approval);
                        w.set_auth_state("signed-in".into());
                    }
                    Err(e) => {
                        w.set_auth_error(e.into());
                        w.set_auth_state("failed".into());
                    }
                });
            });
        }
    });

    window.on_cancel_login(move || {
        if let Some(cancel) = current.borrow_mut().take() {
            cancel.store(true, Ordering::Relaxed);
        }
    });
}

/// Run the device flow for `username` and exchange the result for a session.
fn sign_in(
    device_flow: &DeviceFlowClient,
    rolx: &RolxClient,
    username: &str,
    cancel: &AtomicBool,
    show_code: impl FnOnce(&DeviceAuthorization),
) -> Result<Approval, String> {
    let authorization = device_flow.start().map_err(|e| e.to_string())?;
    show_code(&authorization);
    let id_token = device_flow
        .wait_for_id_token(&authorization, cancel)
        .map_err(|e| e.to_string())?;

    // The device flow cannot be bound to an account in advance, so make sure
    // that the person who confirmed the login is the one who was asked for.
    let email = oidc::email_from_id_token(&id_token).map_err(|e| e.to_string())?;
    if !email.eq_ignore_ascii_case(username) {
        return Err(format!("signed in as {email}, but {username} was expected"));
    }
    rolx.sign_in(&id_token).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn qr_code_is_square_with_quiet_zone() {
        let qr = qr_code("https://www.google.com/device?user_code=ABC-DEF-GHIJ").unwrap();
        assert_eq!(qr.width(), qr.height());
        // top-left corner lies in the quiet zone and must be white
        assert_eq!(qr.as_slice()[0], slint::Rgba8Pixel::new(255, 255, 255, 255));
    }
}
