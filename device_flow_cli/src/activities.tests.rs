use super::*;
use api::ClientInfo;

#[test]
fn format_date_is_zero_padded_iso() {
    let date = NaiveDate::from_ymd_opt(2026, 3, 7).unwrap();
    assert_eq!(format_date(date), "2026-03-07");
}

#[test]
fn client_rejects_invalid_token_without_leaking_it() {
    let err = api::Client::create("https://example.invalid/", "bad\ntoken")
        .expect_err("invalid token must be rejected");
    assert!(!err.to_string().contains("bad"));
}

#[test]
fn client_trims_trailing_slash() {
    let client = api::Client::create("https://example.invalid/", "token").unwrap();
    assert_eq!(client.baseurl(), "https://example.invalid");
}
