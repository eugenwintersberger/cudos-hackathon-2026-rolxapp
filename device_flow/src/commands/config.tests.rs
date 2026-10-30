use super::*;

#[test]
fn flattens_nested_tables_to_dotted_keys() {
    let value: toml::Value = toml::from_str(
        r#"
server_url = "https://example.com"
[google_desktop_client]
id = "abc"
"#,
    )
    .unwrap();
    let pairs = flatten(&value);
    assert_eq!(
        pairs,
        vec![
            (
                "google_desktop_client.id".to_string(),
                "\"abc\"".to_string()
            ),
            (
                "server_url".to_string(),
                "\"https://example.com\"".to_string()
            ),
        ]
    );
}
