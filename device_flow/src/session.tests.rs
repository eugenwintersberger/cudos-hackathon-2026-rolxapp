use super::*;

fn sample() -> Session {
    Session {
        bearer_token: BearerToken::from_string("token".into()),
        expires: Utc::now() + Duration::hours(1),
        user_id: "1".into(),
        user_email: "a@example.com".into(),
        user_first_name: "A".into(),
        user_last_name: "B".into(),
    }
}

fn store(name: &str) -> FileSessionStore {
    let dir = std::env::temp_dir().join(format!("rolx-test-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    FileSessionStore::at(dir.join("nested").join("session.json"))
}

#[test]
fn load_without_session_is_none() {
    assert!(store("none").load().unwrap().is_none());
}

#[test]
fn save_then_load_round_trips() {
    let store = store("roundtrip");
    store.save(&sample()).unwrap();
    let loaded = store.load().unwrap().unwrap();
    assert_eq!(loaded.user_email, "a@example.com");
    assert_eq!(loaded.bearer_token.as_str(), "token");
}

#[test]
fn delete_removes_session_and_tolerates_missing() {
    let store = store("delete");
    store.save(&sample()).unwrap();
    store.delete().unwrap();
    assert!(store.load().unwrap().is_none());
    store.delete().unwrap();
}

#[test]
fn expiry_checks() {
    let mut s = sample();
    assert!(!s.is_expired());
    assert!(!s.is_near_expiry());
    s.expires = Utc::now() + Duration::minutes(5);
    assert!(s.is_near_expiry());
    assert!(!s.is_expired());
    s.expires = Utc::now() - Duration::minutes(1);
    assert!(s.is_expired());
}
