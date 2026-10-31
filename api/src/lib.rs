use progenitor::generate_api;
use reqwest::header;

// Generate types and Client directly from the JSON spec:
generate_api!("rolx-openapi.with-operation-id.json");

impl Client{
    pub fn with_token(token: String) -> Result<Client, reqwest::Error> {
        let headers = {
            let mut headers = header::HeaderMap::new();
            let mut auth_value = header::HeaderValue::from_str(format!("Bearer {}", token).as_str()).unwrap();
            auth_value.set_sensitive(true);
            headers.insert(header::AUTHORIZATION, auth_value);
            headers
        };
        let http = reqwest::Client::builder()
            .default_headers(headers)
            .build()?;
        Ok(Client {
            baseurl: "https://rolx-stage.cudos.ch".to_string(),
            client: http,
        })
    }
}
