use device_flow::AppConfig;

/// Lists the settings of the config as dotted `(key, value)` pairs.
pub fn list(config: &AppConfig) -> Result<Vec<(String, String)>, toml::ser::Error> {
    Ok(flatten(&toml::Value::try_from(config)?))
}

fn flatten(value: &toml::Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    if let toml::Value::Table(table) = value {
        flatten_table("", table, &mut out);
    }
    out
}

fn flatten_table(prefix: &str, table: &toml::Table, out: &mut Vec<(String, String)>) {
    for (key, value) in table {
        let path = if prefix.is_empty() {
            key.clone()
        } else {
            format!("{prefix}.{key}")
        };
        match value {
            toml::Value::Table(nested) => flatten_table(&path, nested, out),
            _ => out.push((path, value.to_string())),
        }
    }
}

#[cfg(test)]
#[path = "config_list.tests.rs"]
mod tests;
