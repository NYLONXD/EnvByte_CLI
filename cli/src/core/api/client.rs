//! HTTP plumbing: server address, credentials and error messages.

use serde::de::DeserializeOwned;

/// Everything a request needs: where to send it and who is sending it.
#[derive(Clone)]
pub struct Session {
    pub server: String,
    pub token: String,
    client: reqwest::Client,
}

impl Session {
    pub fn new(server: &str, token: String) -> Result<Self, String> {
        validate_server_url(server)?;
        Ok(Self {
            server: server.trim_end_matches('/').to_string(),
            token,
            client: http_client()?,
        })
    }

    pub fn url(&self, path: &str) -> String {
        format!("{}{path}", self.server)
    }

    pub fn get(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.get(self.url(path)).bearer_auth(&self.token)
    }

    pub fn post(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.post(self.url(path)).bearer_auth(&self.token)
    }

    pub fn put(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.put(self.url(path)).bearer_auth(&self.token)
    }

    pub fn patch(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.patch(self.url(path)).bearer_auth(&self.token)
    }

    pub fn delete(&self, path: &str) -> reqwest::RequestBuilder {
        self.client.delete(self.url(path)).bearer_auth(&self.token)
    }
}

/// Hosted Envbyte API, used when `ENVBYTE_SERVER` is not set.
pub const DEFAULT_SERVER: &str = "https://api.envbyte.trackedge.in";

/// Server address. Override with `ENVBYTE_SERVER`.
pub fn server_url() -> String {
    std::env::var("ENVBYTE_SERVER")
        .ok()
        .filter(|value| !value.trim().is_empty())
        .unwrap_or_else(|| DEFAULT_SERVER.to_string())
        .trim_end_matches('/')
        .to_string()
}

pub fn http_client() -> Result<reqwest::Client, String> {
    reqwest::Client::builder()
        .connect_timeout(std::time::Duration::from_secs(10))
        .timeout(std::time::Duration::from_secs(30))
        .user_agent(concat!("envbyte/", env!("CARGO_PKG_VERSION")))
        .build()
        .map_err(|e| format!("Could not initialize HTTP client: {e}"))
}

/// Refuses to send credentials in the clear to anything but a local server.
pub fn validate_server_url(value: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(value).map_err(|e| format!("Invalid Envbyte server URL: {e}"))?;
    match url.scheme() {
        "https" => Ok(()),
        "http" if matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "::1")) => Ok(()),
        "http" => {
            Err("Refusing to send credentials over plain HTTP to a remote server.".to_string())
        }
        _ => Err(
            "Envbyte server URL must use HTTPS (HTTP is allowed only for localhost).".to_string(),
        ),
    }
}

/// Sends a request and decodes a successful JSON body, turning any failure
/// into a message worth showing a user.
pub async fn send_json<T: DeserializeOwned>(
    request: reqwest::RequestBuilder,
    action: &str,
) -> Result<T, String> {
    let response = request
        .send()
        .await
        .map_err(|e| format!("{action} failed: {e}"))?;
    if !response.status().is_success() {
        return Err(describe_error(response, action).await);
    }
    response
        .json::<T>()
        .await
        .map_err(|e| format!("{action} returned an unexpected response: {e}"))
}

/// Sends a request and ignores the body, for endpoints that only signal
/// success.
pub async fn send(request: reqwest::RequestBuilder, action: &str) -> Result<(), String> {
    let response = request
        .send()
        .await
        .map_err(|e| format!("{action} failed: {e}"))?;
    if !response.status().is_success() {
        return Err(describe_error(response, action).await);
    }
    Ok(())
}

pub async fn describe_error(response: reqwest::Response, action: &str) -> String {
    let status = response.status();
    let body = response.text().await.unwrap_or_default();
    let message = serde_json::from_str::<serde_json::Value>(&body)
        .ok()
        .and_then(|value| {
            value["error"]
                .as_str()
                .or_else(|| value["message"].as_str())
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
        assert!(validate_server_url("https://api.envbyte.dev").is_ok());
        assert!(validate_server_url("http://localhost:3030").is_ok());
        assert!(validate_server_url("http://127.0.0.1:3030").is_ok());
        assert!(validate_server_url("http://api.envbyte.dev").is_err());
        assert!(validate_server_url("file:///tmp/socket").is_err());
    }

    #[test]
    fn builds_urls_without_double_slashes() {
        let session = Session::new("https://api.envbyte.dev/", "token".to_string()).unwrap();
        assert_eq!(session.url("/projects"), "https://api.envbyte.dev/projects");
    }
}
