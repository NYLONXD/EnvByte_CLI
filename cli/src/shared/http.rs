//! Talking to the Greenbyte API: server address, HTTP client and error messages.

/// Default Greenbyte server URL.
/// Override with the GREENBYTE_SERVER environment variable.
pub fn server_url() -> String {
    std::env::var("GREENBYTE_SERVER")
        .unwrap_or_else(|_| "http://localhost:3030".to_string())
        .trim_end_matches('/')
        .to_string()
}

pub fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(concat!("greenbyte/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("Could not initialize HTTP client: {e}"))
}

pub fn validate_server_url(value: &str) -> Result<(), String> {
    let url =
        reqwest::Url::parse(value).map_err(|e| format!("Invalid Greenbyte server URL: {e}"))?;
    match url.scheme() {
        "https" => Ok(()),
        "http" if matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1")) => Ok(()),
        "http" => {
            Err("Refusing to send credentials over plain HTTP to a remote server.".to_string())
        }
        _ => Err(
            "Greenbyte server URL must use HTTPS (HTTP is allowed only for localhost).".to_string(),
        ),
    }
}

pub async fn response_error(response: reqwest::Response, action: &str) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value["message"]
                .as_str()
                .or_else(|| value["error"].as_str())
                .map(String::from)
        })
        .unwrap_or_else(|| {
            let mut safe: String = body.chars().take(500).collect();
            if safe.is_empty() {
                safe = "the server returned no details".to_string();
            }
            safe
        });
    format!("{action} failed ({status}): {message}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_insecure_remote_server_urls() {
        assert!(validate_server_url("https://api.greenbyte.dev").is_ok());
        assert!(validate_server_url("http://localhost:3030").is_ok());
        assert!(validate_server_url("http://api.greenbyte.dev").is_err());
        assert!(validate_server_url("file:///tmp/socket").is_err());
    }
}
