use super::*;
use std::time::Duration;

fn prompt(complete: Option<&str>) -> DeviceCodePrompt {
    DeviceCodePrompt {
        verification_uri: "https://example.com/device".into(),
        verification_uri_complete: complete.map(Into::into),
        user_code: "ABCD-EFGH".into(),
        expires_in: Duration::from_secs(300),
    }
}

#[test]
fn device_prompt_prefers_complete_uri() {
    let text = format_device_prompt(&prompt(Some("https://example.com/device?c=ABCD-EFGH")));
    assert!(text.contains("https://example.com/device?c=ABCD-EFGH"));
    assert!(text.contains("ABCD-EFGH"));
}

#[test]
fn device_prompt_falls_back_to_verification_uri() {
    let text = format_device_prompt(&prompt(None));
    assert!(text.contains("https://example.com/device"));
    assert!(text.contains("code: ABCD-EFGH"));
}

#[test]
fn manual_open_contains_url() {
    assert!(format_manual_open("https://x.test/auth").contains("https://x.test/auth"));
}
