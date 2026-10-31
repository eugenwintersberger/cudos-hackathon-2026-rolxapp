use chrono::NaiveDate;
use device_flow::{AppConfig, SessionStore};

/// Formats a date the way the Rolx API expects it (`YYYY-mm-dd`).
pub fn format_date(date: NaiveDate) -> String {
    date.format("%Y-%m-%d").to_string()
}

/// Builds an API client from the stored session, extending the session first if it is
/// expired or about to expire. Never prints or logs the token.
pub async fn authorized_client(
    config: &AppConfig,
    store: &impl SessionStore,
) -> anyhow::Result<api::Client> {
    let mut session = device_flow::whoami(store)?;
    if session.is_near_expiry() {
        session = device_flow::extend_session(config, store).await?;
    }
    Ok(api::Client::create(
        &config.server_url,
        session.bearer_token.as_str(),
    )?)
}

/// Fetches all activities that did not end before `today`.
pub async fn fetch_activities(
    client: &api::Client,
    today: NaiveDate,
) -> anyhow::Result<Vec<api::types::RolXServerProjectsWebApiResourceActivity>> {
    match client.get_activity(Some(&format_date(today))).await {
        Ok(response) => Ok(response.into_inner()),
        Err(e) if e.status().map(|s| s.as_u16()) == Some(401) => {
            anyhow::bail!("server rejected the session (HTTP 401): run `rolxapp login`")
        }
        Err(e) => anyhow::bail!("fetching activities failed: {e}"),
    }
}

#[cfg(test)]
#[path = "activities.tests.rs"]
mod tests;
