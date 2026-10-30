/// Redacts the values of given `sensitive_keys` if direct key of `value`. Disabled for debug builds.
#[cfg(not(debug_assertions))]
pub fn redact_json(mut value: serde_json::Value, sensitive_keys: &[&str]) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        for &key in sensitive_keys {
            if obj.contains_key(key) {
                obj.insert(key.into(), serde_json::Value::String("***".into()));
            }
        }
    }
    value
}

/// Returns value as is for non-debug builds.
#[cfg(debug_assertions)]
pub fn redact_json(value: serde_json::Value, _sensitive_keys: &[&str]) -> serde_json::Value {
    value
}
