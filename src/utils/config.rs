pub const MAX_ENV_FILE_SIZE: u64 = 5 * 1024 * 1024;

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

/// Returns the path to the .env file in the current directory.
pub fn env_file_path() -> std::path::PathBuf {
    std::path::Path::new(".env").to_path_buf()
}

/// Reads the raw contents of the local .env file.
pub fn read_env_file() -> Result<String, String> {
    let path = env_file_path();
    if !path.exists() {
        return Err("No .env file found in the current directory.".to_string());
    }
    ensure_file_size(&path)?;
    std::fs::read_to_string(&path).map_err(|e| format!("Could not read .env: {}", e))
}

/// Writes content to the local .env file.
/// Validates and writes an environment filename without allowing path traversal.
pub fn write_named_env_file(filename: &str, content: &str) -> Result<(), String> {
    validate_env_filename(filename)?;
    if content.len() as u64 > MAX_ENV_FILE_SIZE {
        return Err(format!(
            "Refusing to write an environment file larger than {} MiB.",
            MAX_ENV_FILE_SIZE / 1024 / 1024
        ));
    }
    crate::utils::local_store::secure_atomic_write(
        std::path::Path::new(filename),
        content.as_bytes(),
    )
    .map_err(|e| format!("Could not write {filename}: {e}"))
}

pub fn validate_env_filename(filename: &str) -> Result<(), String> {
    let path = std::path::Path::new(filename);
    if !filename.starts_with(".env")
        || filename == ".env."
        || path.is_absolute()
        || path.components().count() != 1
    {
        return Err(format!("Unsafe environment filename returned: {filename}"));
    }
    Ok(())
}

pub fn ensure_file_size(path: &std::path::Path) -> Result<(), String> {
    let size = std::fs::metadata(path)
        .map_err(|e| format!("Could not inspect {}: {e}", path.display()))?
        .len();
    if size > MAX_ENV_FILE_SIZE {
        return Err(format!(
            "{} is larger than the {} MiB safety limit.",
            path.display(),
            MAX_ENV_FILE_SIZE / 1024 / 1024
        ));
    }
    Ok(())
}

/// Scans the current directory for `.env*` files (e.g. `.env`, `.env.local`, `.env.production`).
/// Returns a sorted list of file paths.
pub fn scan_env_files() -> Vec<std::path::PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(".") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            let is_regular_file = entry
                .file_type()
                .map(|kind| kind.is_file())
                .unwrap_or(false);
            if name_str.starts_with(".env") && is_regular_file {
                results.push(entry.path());
            }
        }
    }
    results.sort();
    results
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_safe_env_filenames() {
        for name in [".env", ".env.local", ".env-production"] {
            assert!(validate_env_filename(name).is_ok());
        }
    }

    #[test]
    fn rejects_paths_and_unrelated_files() {
        for name in [
            "../.env",
            ".env/secret",
            "/tmp/.env",
            "secrets.txt",
            ".env.",
        ] {
            assert!(validate_env_filename(name).is_err(), "accepted {name}");
        }
    }

    #[test]
    fn rejects_insecure_remote_server_urls() {
        assert!(validate_server_url("https://api.greenbyte.dev").is_ok());
        assert!(validate_server_url("http://localhost:3030").is_ok());
        assert!(validate_server_url("http://api.greenbyte.dev").is_err());
        assert!(validate_server_url("file:///tmp/socket").is_err());
    }
}
