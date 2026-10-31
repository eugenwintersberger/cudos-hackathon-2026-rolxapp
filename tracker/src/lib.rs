//! Rolx tracker UI, shared by the desktop and board builds.
//!
//! The embedding binary picks the Slint backend by enabling the `desktop`
//! or `linuxkms` feature of this crate (or via `SLINT_BACKEND` at runtime).

pub mod config;
pub mod connectivity;
pub mod login;
pub mod logging;
pub mod rolx;
pub mod wlan;
pub mod work;

use std::{
    cell::RefCell,
    error::Error,
    path::{Path, PathBuf},
    rc::Rc,
    sync::Arc,
    thread,
    time::Duration,
};

use chrono::Local;
use config::Config;
use device_flow::AppConfig;
use login::{Cancel, MemorySessionStore};
use tracing::{error, info, warn};

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
    let _log = logging::init(Path::new(config.log_dir()));
    info!(
        version = env!("CARGO_PKG_VERSION"),
        server_configured = config.server_url.is_some(),
        login_configured = config.login().is_some(),
        admin_password_configured = config.has_admin_password(),
        "starting"
    );

    let window = AppWindow::new()?;
    window.set_admin_configured(config.has_admin_password());
    window.set_login_configured(config.login().is_some());
    // The session granted by the Rolx server once the user has signed in, and
    // the client settings to extend it; shared by sign-in and tracking.
    let app_config = config
        .login()
        .map(|login| Arc::new(login::app_config(&login)));
    let store = Arc::new(MemorySessionStore::default());
    setup_login(&window, app_config.clone(), store.clone());
    setup_tracking(&window, app_config, store, config.work_session_path());
    watch_connection(&window, config.server_url.clone());
    if cfg!(feature = "linuxkms") {
        setup_wlan(&window, &config);
    }
    window.on_verify_admin_password(move |password| {
        let ok = config.verify_admin_password(&password);
        if ok {
            info!("administrator password accepted");
        } else {
            warn!("administrator password rejected");
        }
        ok
    });
    let result = window.run();
    info!("stopped");
    result?;
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
        let mut last = None;
        loop {
            let online = connectivity::is_reachable(&server_url, TIMEOUT);
            if last != Some(online) {
                info!(online, "connection to the server changed");
                last = Some(online);
            }
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
            info!(ssid = %ssid, "updating the WLAN configuration");

            let weak = weak.clone();
            let (path, restart) = (path.clone(), restart.clone());
            let (ssid, passphrase) = (ssid.to_string(), passphrase.to_string());
            thread::spawn(move || {
                let result = wlan::apply(&path, &restart, &ssid, &passphrase);
                match &result {
                    Ok(()) => info!("WLAN configuration updated, network restarted"),
                    Err(e) => error!("WLAN configuration failed: {e}"),
                }
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

/// Connect the sign-in callbacks of the window to the device flow.
fn setup_login(
    window: &AppWindow,
    app_config: Option<Arc<AppConfig>>,
    store: Arc<MemorySessionStore>,
) {
    let Some(app_config) = app_config else { return };
    // The attempt in progress, if any.
    let current: Rc<RefCell<Option<Arc<Cancel>>>> = Rc::default();

    window.on_start_login({
        let weak = window.as_weak();
        let current = current.clone();
        move |username| {
            let cancel = Arc::new(Cancel::default());
            if let Some(previous) = current.borrow_mut().replace(cancel.clone()) {
                previous.cancel();
            }
            let Some(window) = weak.upgrade() else { return };
            window.set_auth_state("starting".into());
            info!(username = %username, "starting the device flow login");

            let weak = weak.clone();
            let app_config = app_config.clone();
            let store = store.clone();
            let username = username.trim().to_owned();
            thread::spawn(move || {
                let show_code = |prompt: &device_flow::DeviceCodePrompt| {
                    let url = prompt.verification_uri.clone();
                    let code = prompt.user_code.clone();
                    let qr = qr_code(&login::complete_url(prompt));
                    let _ = weak.upgrade_in_event_loop(move |w| {
                        w.set_verification_url(url.into());
                        w.set_user_code(code.into());
                        if let Some(qr) = qr {
                            w.set_qr_code(slint::Image::from_rgba8(qr));
                        }
                        w.set_auth_state("waiting".into());
                    });
                };
                let result = login::sign_in(&app_config, &store, &username, &cancel, show_code);
                if cancel.is_cancelled() {
                    info!("login cancelled");
                    return;
                }
                match &result {
                    Ok(_) => info!("signed in"),
                    Err(e) => warn!("sign-in failed: {e}"),
                }
                let _ = weak.upgrade_in_event_loop(move |w| match result {
                    Ok(_) => w.set_auth_state("signed-in".into()),
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
            cancel.cancel();
        }
    });
}

/// Connect the work tracking: the stored start timestamp, the activities that
/// are fetched when the work is stopped, and the record entry sent to Rolx.
fn setup_tracking(
    window: &AppWindow,
    app_config: Option<Arc<AppConfig>>,
    store: Arc<MemorySessionStore>,
    work_session_path: Option<PathBuf>,
) {
    let work = Arc::new(work::Store::new(work_session_path));
    // The activities and the day record of the last "Stop work". The fetching
    // thread hands them over through the event loop.
    let activities: Shared<Vec<rolx::Activity>> = Shared::default();
    let day: Shared<Option<rolx::Day>> = Shared::default();

    restore_session(window, &work);

    window.on_start_work({
        let weak = window.as_weak();
        let work = work.clone();
        move || {
            let session = work::WorkSession::start();
            if let Err(e) = work.save(&session) {
                // Keep going: the session is still usable in memory.
                warn!("cannot store the work session: {e}");
            }
            info!(started_at = %session.started_at, "work started");
            if let Some(window) = weak.upgrade() {
                window.set_work_message("".into());
                show_running(&window, &session);
            }
        }
    });

    window.on_stop_work({
        let weak = window.as_weak();
        let app_config = app_config.clone();
        let store = store.clone();
        let work = work.clone();
        let activities = activities.clone();
        let day = day.clone();
        move || {
            let Some(app_config) = app_config.clone() else { return };
            let session = match work.load() {
                Ok(Some(session)) => session,
                Ok(None) => {
                    warn!("stopping work without a session");
                    return;
                }
                Err(e) => {
                    warn!("cannot read the work session: {e}");
                    return;
                }
            };
            if let Some(window) = weak.upgrade() {
                window.set_work_message("".into());
                window.set_work_state("loading".into());
            }
            info!("work stopped, fetching the activities and the record of the day");

            let weak = weak.clone();
            let store = store.clone();
            let activities = activities.clone();
            let day = day.clone();
            thread::spawn(move || {
                let date = session.date();
                let result = rolx::Rolx::connect(&app_config, &store).and_then(|rolx| {
                    let activities = rolx.activities(date)?;
                    let day = rolx.day(date)?;
                    Ok((activities, day))
                });
                match result {
                    Ok((fetched, fetched_day)) => {
                        let recorded = fetched_day.recorded_seconds();
                        info!(
                            date = %rolx::date_text(fetched_day.date),
                            new_day = !fetched_day.has_record(),
                            activities = fetched.len(),
                            recorded_seconds = recorded,
                            "fetched the activities and the record of the day"
                        );
                        let labels: Vec<slint::language::StandardListViewItem> = fetched
                            .iter()
                            .map(|activity| {
                                let text = if activity.favourite {
                                    format!("{}  ★", activity.label)
                                } else {
                                    activity.label.clone()
                                };
                                slint::language::StandardListViewItem::from(text.as_str())
                            })
                            .collect();
                        let _ = weak.upgrade_in_event_loop(move |window| {
                            *activities.lock().unwrap() = fetched;
                            *day.lock().unwrap() = Some(fetched_day);
                            window.set_activities(slint::ModelRc::new(
                                slint::VecModel::from(labels),
                            ));
                            window.set_selected_activity_index(-1);
                            window.set_work_today_seconds(recorded as i32);
                            window.set_work_state("selecting".into());
                        });
                    }
                    Err(e) => {
                        warn!("cannot fetch the activities or the record of the day: {e}");
                        let message = e.to_string();
                        let _ = weak.upgrade_in_event_loop(move |window| {
                            window.set_work_message(message.into());
                            window.set_work_state("error".into());
                        });
                    }
                }
            });
        }
    });

    window.on_save_work({
        let weak = window.as_weak();
        let app_config = app_config.clone();
        let store = store.clone();
        let work = work.clone();
        let activities = activities.clone();
        let day = day.clone();
        move || {
            let Some(app_config) = app_config.clone() else { return };
            let Some(window) = weak.upgrade() else { return };
            let index = window.get_selected_activity_index();
            let Some(activity) = activities
                .lock()
                .unwrap()
                .get(usize::try_from(index).unwrap_or(usize::MAX))
                .cloned()
            else {
                return;
            };
            let Some(day) = day.lock().unwrap().clone() else { return };
            let session = match work.load() {
                Ok(Some(session)) => session,
                Ok(None) => {
                    warn!("saving work without a session");
                    return;
                }
                Err(e) => {
                    warn!("cannot read the work session: {e}");
                    return;
                }
            };
            let duration = session.duration_seconds(Local::now());
            let begin = session.begin_seconds();
            window.set_work_message("".into());
            window.set_work_state("saving".into());
            info!(activity = %activity.label, duration_seconds = duration, "recording the work");

            let weak = weak.clone();
            let store = store.clone();
            let work = work.clone();
            thread::spawn(move || {
                let result = rolx::Rolx::connect(&app_config, &store)
                    .and_then(|rolx| rolx.add_entry(&day, &activity, duration, begin));
                match &result {
                    Ok(()) => info!(duration_seconds = duration, "work recorded"),
                    Err(e) => warn!("recording the work failed: {e}"),
                }
                let recorded = day.recorded_seconds() + duration;
                match result {
                    Ok(()) => {
                        // The session is at Rolx now; the timestamp is done.
                        if let Err(e) = work.clear() {
                            warn!("cannot remove the work session: {e}");
                        }
                        let message = format!(
                            "Recorded {} on {}.",
                            work::format_duration(duration),
                            activity.label
                        );
                        let _ = weak.upgrade_in_event_loop(move |window| {
                            window.set_selected_activity_index(-1);
                            window.set_work_today_seconds(recorded as i32);
                            window.set_work_message(message.into());
                            window.set_work_state("idle".into());
                        });
                    }
                    Err(e) => {
                        let message = e.to_string();
                        let _ = weak.upgrade_in_event_loop(move |window| {
                            window.set_work_message(message.into());
                            window.set_work_state("error".into());
                        });
                    }
                }
            });
        }
    });

    // "Keep working" after a stop and "Back" after an error: the session, if
    // any, is still running and its timestamp is kept.
    window.on_cancel_work({
        let weak = window.as_weak();
        let work = work.clone();
        let day = day.clone();
        move || {
            day.lock().unwrap().take();
            let session = work.load().ok().flatten();
            if let Some(window) = weak.upgrade() {
                window.set_selected_activity_index(-1);
                window.set_work_message("".into());
                match session {
                    Some(session) => show_running(&window, &session),
                    None => window.set_work_state("idle".into()),
                }
            }
        }
    });
}

/// The activities and the day record, shared between the fetching thread and
/// the UI thread.
type Shared<T> = Arc<std::sync::Mutex<T>>;

/// Continue a work session that was started in an earlier run and is still
/// running; a session from another day is dropped.
fn restore_session(window: &AppWindow, work: &work::Store) {
    match work.load() {
        Ok(Some(session)) if session.is_today() => {
            info!(started_at = %session.started_at, "continuing the work session");
            show_running(window, &session);
        }
        Ok(Some(session)) => {
            warn!(
                started_at = %session.started_at,
                "dropping a work session that did not start today"
            );
            if let Err(e) = work.clear() {
                warn!("cannot remove the work session: {e}");
            }
        }
        Ok(None) => {}
        Err(e) => warn!("cannot read the work session: {e}"),
    }
}

/// Show `session` as running in the window.
fn show_running(window: &AppWindow, session: &work::WorkSession) {
    window.set_work_started_at(session.started_at.format("%H:%M").to_string().into());
    window.set_work_elapsed_seconds(session.elapsed_seconds() as i32);
    window.set_work_state("running".into());
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
