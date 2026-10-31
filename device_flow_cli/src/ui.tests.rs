use super::*;

fn pixel(buf: &SharedPixelBuffer<Rgba8Pixel>, x: usize, y: usize) -> u8 {
    buf.as_slice()[y * buf.width() as usize + x].r
}

#[test]
fn qr_is_square_and_scaled() {
    let buf = qr_pixels("https://example.com/device?c=ABCD-EFGH").unwrap();
    assert_eq!(buf.width(), buf.height());
    assert!(buf.width() as usize <= TARGET_SIZE);
    assert!(buf.width() as usize >= TARGET_SIZE / 2);
}

#[test]
fn qr_has_white_quiet_zone() {
    let buf = qr_pixels("https://example.com/device").unwrap();
    let size = buf.width() as usize;
    for i in 0..size {
        assert_eq!(pixel(&buf, i, 0), 255);
        assert_eq!(pixel(&buf, 0, i), 255);
        assert_eq!(pixel(&buf, i, size - 1), 255);
        assert_eq!(pixel(&buf, size - 1, i), 255);
    }
}

#[test]
fn qr_top_left_finder_pattern_is_dark() {
    let buf = qr_pixels("https://example.com/device").unwrap();
    let total = QrCode::new(b"https://example.com/device").unwrap().width() + 2 * QUIET_ZONE;
    let scale = buf.width() as usize / total;
    let corner = QUIET_ZONE * scale;
    assert_eq!(pixel(&buf, corner, corner), 0);
}

#[test]
fn target_url_prefers_complete_uri() {
    let mut prompt = DeviceCodePrompt {
        verification_uri: "https://example.com/device".into(),
        verification_uri_complete: Some("https://example.com/device?c=X".into()),
        user_code: "X".into(),
        expires_in: std::time::Duration::from_secs(60),
    };
    assert_eq!(target_url(&prompt), "https://example.com/device?c=X");
    prompt.verification_uri_complete = None;
    assert_eq!(target_url(&prompt), "https://example.com/device");
}
