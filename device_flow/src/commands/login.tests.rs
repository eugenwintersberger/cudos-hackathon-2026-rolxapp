use super::*;

fn config_without_clients() -> AppConfig {
    AppConfig {
        server_url: "https://rolx.example.com".into(),
        google_desktop_client: None,
        google_device_client: None,
    }
}

#[tokio::test]
async fn login_code_requires_device_client() {
    let err = login_code(&config_without_clients()).await.err().unwrap();
    assert!(matches!(
        err,
        Error::MissingClientConfig("google_device_client")
    ));
}

#[cfg(feature = "browser-login")]
struct NoStore;

#[cfg(feature = "browser-login")]
impl SessionStore for NoStore {
    fn load(&self) -> Result<Option<Session>> {
        Ok(None)
    }
    fn save(&self, _: &Session) -> Result<()> {
        Ok(())
    }
    fn delete(&self) -> Result<()> {
        Ok(())
    }
}

#[cfg(feature = "browser-login")]
#[tokio::test]
async fn login_browser_requires_desktop_client() {
    let err = login_browser(&config_without_clients(), &NoStore)
        .await
        .err()
        .unwrap();
    assert!(matches!(
        err,
        Error::MissingClientConfig("google_desktop_client")
    ));
}
