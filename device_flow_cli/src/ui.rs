//! Small window showing the device-flow URL, user code and a QR code.

use anyhow::Context;
use device_flow::{DeviceCodePrompt, FileSessionStore, PendingCodeLogin, Session};
use qrcode::{Color, QrCode};
use slint::{ComponentHandle, Image, Rgba8Pixel, SharedPixelBuffer};
use std::sync::{Arc, Mutex};

slint::include_modules!();

/// Empty modules around the code, required by the QR specification for reliable scanning.
const QUIET_ZONE: usize = 4;
/// Approximate edge length of the QR image in pixels.
const TARGET_SIZE: usize = 200;

/// Renders `data` as a black-on-white QR code with a quiet zone.
pub fn qr_pixels(data: &str) -> anyhow::Result<SharedPixelBuffer<Rgba8Pixel>> {
    let code = QrCode::new(data.as_bytes()).context("could not encode QR code")?;
    let modules = code.width();
    let total = modules + 2 * QUIET_ZONE;
    let scale = (TARGET_SIZE / total).max(1);
    let size = total * scale;

    let mut buffer = SharedPixelBuffer::<Rgba8Pixel>::new(size as u32, size as u32);
    let pixels = buffer.make_mut_slice();
    for (i, pixel) in pixels.iter_mut().enumerate() {
        let (x, y) = (i % size / scale, i / size / scale);
        let dark = x >= QUIET_ZONE
            && y >= QUIET_ZONE
            && x < QUIET_ZONE + modules
            && y < QUIET_ZONE + modules
            && code[(x - QUIET_ZONE, y - QUIET_ZONE)] == Color::Dark;
        let v = if dark { 0 } else { 255 };
        *pixel = Rgba8Pixel::new(v, v, v, 255);
    }
    Ok(buffer)
}

/// URL to open or scan: the one with the code already filled in, if available.
fn target_url(prompt: &DeviceCodePrompt) -> &str {
    prompt
        .verification_uri_complete
        .as_deref()
        .unwrap_or(&prompt.verification_uri)
}

/// Shows the prompt window and waits for the login to finish.
///
/// Blocks the calling thread while the window is open (Slint's event loop), so call it from
/// the main thread of a multi-threaded tokio runtime. Closing the window cancels the login.
pub async fn login_with_window(
    pending: PendingCodeLogin,
    store: FileSessionStore,
) -> anyhow::Result<Session> {
    let prompt = pending.prompt().clone();
    let url = target_url(&prompt);
    let qr = Image::from_rgba8(qr_pixels(url)?);

    let window = LoginWindow::new().context("could not create window")?;
    window.set_url(url.into());
    window.set_code(prompt.user_code.as_str().into());
    window.set_qr(qr);

    let outcome: Arc<Mutex<Option<device_flow::Result<Session>>>> = Arc::default();
    let task = tokio::spawn({
        let outcome = Arc::clone(&outcome);
        async move {
            let result = pending.finish(&store).await;
            *outcome.lock().unwrap() = Some(result);
            // Err only means the event loop is already gone (window closed).
            let _ = slint::invoke_from_event_loop(|| {
                let _ = slint::quit_event_loop();
            });
        }
    });

    window.run().context("window failed")?;

    // Dropping the pending login (by aborting its task) cancels it.
    task.abort();
    let result = outcome.lock().unwrap().take();
    match result {
        Some(result) => Ok(result?),
        None => anyhow::bail!("login cancelled: window closed"),
    }
}

#[cfg(test)]
#[path = "ui.tests.rs"]
mod tests;
