/// Default Greenbyte server URL.
/// Override with the GREENBYTE_SERVER environment variable.
pub fn server_url() -> String {
    std::env::var("GREENBYTE_SERVER")
        .unwrap_or_else(|_| "http://localhost:3030".to_string())
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
    std::fs::read_to_string(&path)
        .map_err(|e| format!("Could not read .env: {}", e))
}

/// Writes content to the local .env file.
pub fn write_env_file(content: &str) -> Result<(), String> {
    std::fs::write(env_file_path(), content)
        .map_err(|e| format!("Could not write .env: {}", e))
}

/// Scans the current directory for `.env*` files (e.g. `.env`, `.env.local`, `.env.production`).
/// Returns a sorted list of file paths.
pub fn scan_env_files() -> Vec<std::path::PathBuf> {
    let mut results = Vec::new();
    if let Ok(entries) = std::fs::read_dir(".") {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let name_str = name.to_string_lossy();
            if name_str.starts_with(".env") && entry.path().is_file() {
                results.push(entry.path());
            }
        }
    }
    results.sort();
    results
}