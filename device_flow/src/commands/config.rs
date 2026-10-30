use std::path::PathBuf;

use crate::config::{self, AppConfig};
use crate::error::Result;

/// Lists the settings of the config file as dotted `(key, value)` pairs, ignoring overrides.
pub fn list() -> Result<Vec<(String, String)>> {
    Ok(flatten(&toml::Value::try_from(AppConfig::load()?)?))
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

/// Returns the config file path. The file need not exist.
pub fn path() -> Result<PathBuf> {
    config::config_path()
}

#[cfg(test)]
#[path = "config.tests.rs"]
mod tests;
