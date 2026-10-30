use super::*;

fn response(complete: Option<&str>, interval: Option<u64>) -> auth::DeviceCodeResponse {
    let mut json = serde_json::json!({
        "device_code": "SECRET-DEVICE-CODE",
        "user_code": "ABCD-EFGH",
        "verification_url": "https://example.com/device",
        "expires_in": 300,
    });
    if let Some(c) = complete {
        json["verification_uri_complete"] = c.into();
    }
    if let Some(i) = interval {
        json["interval"] = i.into();
    }
    serde_json::from_value(json).unwrap()
}

#[test]
fn prompt_carries_user_facing_data() {
    let started = Started::from(response(
        Some("https://example.com/device?c=ABCD-EFGH"),
        Some(7),
    ));
    assert_eq!(
        started.prompt,
        DeviceCodePrompt {
            verification_uri: "https://example.com/device".into(),
            verification_uri_complete: Some("https://example.com/device?c=ABCD-EFGH".into()),
            user_code: "ABCD-EFGH".into(),
            expires_in: Duration::from_secs(300),
        }
    );
    assert_eq!(started.interval, Duration::from_secs(7));
}

#[test]
fn prompt_without_complete_uri_and_default_interval() {
    let started = Started::from(response(None, None));
    assert_eq!(started.prompt.verification_uri_complete, None);
    assert_eq!(started.interval, Duration::from_secs(5));
}

#[test]
fn prompt_never_exposes_device_code() {
    let started = Started::from(response(None, None));
    assert!(!format!("{:?}", started.prompt).contains("SECRET-DEVICE-CODE"));
}
