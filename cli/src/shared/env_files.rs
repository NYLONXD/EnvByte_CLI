//! Finding, reading, validating and writing the `.env*` files in the current directory.

pub const MAX_ENV_FILE_SIZE: u64 = 5 * 1024 * 1024;

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
    crate::shared::storage::secure_atomic_write(std::path::Path::new(filename), content.as_bytes())
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
}
