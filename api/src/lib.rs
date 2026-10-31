use progenitor::generate_api;
use reqwest::header;

// Generate types and Client directly from the JSON spec:
generate_api!("rolx-openapi.with-operation-id.json");

/// Error returned by [`Client::create`].
#[derive(Debug)]
pub enum CreateError {
    /// The token contains characters that are not allowed in an HTTP header.
    InvalidToken,
    /// The HTTP client could not be built.
    Http(reqwest::Error),
}

impl std::fmt::Display for CreateError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CreateError::InvalidToken => write!(f, "bearer token is not a valid HTTP header value"),
            CreateError::Http(e) => write!(f, "cannot build HTTP client: {e}"),
        }
    }
}

impl std::error::Error for CreateError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            CreateError::InvalidToken => None,
            CreateError::Http(e) => Some(e),
        }
    }
}

impl Client {
    /// Creates a client for the Rolx server at `baseurl` that authenticates every request
    /// with the given bearer token. The token is never included in error messages.
    pub fn create(baseurl: &str, token: &str) -> Result<Client, CreateError> {
        let mut auth_value = header::HeaderValue::from_str(&format!("Bearer {token}"))
            .map_err(|_| CreateError::InvalidToken)?;
        auth_value.set_sensitive(true);
        let mut headers = header::HeaderMap::new();
        headers.insert(header::AUTHORIZATION, auth_value);
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()
            .map_err(CreateError::Http)?;
        Ok(Client::new_with_client(baseurl.trim_end_matches('/'), http))
    }
}
