//! Writing sensitive files safely.

use std::{fs, io::Write, path::Path};

/// Writes atomically and restricts the result to the current user on Unix.
///
/// The temporary file is created with `create_new`, so it cannot land on a
/// symlink an attacker planted, and the rename is what makes the new content
/// visible - readers never observe a partial file.
pub fn secure_atomic_write(path: &Path, contents: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    if !parent.exists() {
        fs::create_dir_all(parent)
            .map_err(|e| format!("Could not create {}: {e}", parent.display()))?;
    }

    let filename = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("envbyte");
    let temporary = parent.join(format!(".{filename}.{}.tmp", uuid::Uuid::new_v4()));
    let result = write_temporary(&temporary, contents)
        .and_then(|_| fs::rename(&temporary, path).map_err(|e| e.to_string()));
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn write_temporary(path: &Path, contents: &[u8]) -> Result<(), String> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(path)
        .map_err(|e| format!("Could not create {}: {e}", path.display()))?;
    file.write_all(contents)
        .map_err(|e| format!("Could not write {}: {e}", path.display()))?;
    file.sync_all()
        .map_err(|e| format!("Could not sync {}: {e}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replaces_content_and_restricts_permissions() {
        let directory = std::env::temp_dir().join(format!("envbyte-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        let path = directory.join("secret");
        secure_atomic_write(&path, b"first").unwrap();
        secure_atomic_write(&path, b"second").unwrap();
        assert_eq!(fs::read(&path).unwrap(), b"second");
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(directory).unwrap();
    }

    #[test]
    fn leaves_no_temporary_files_behind() {
        let directory = std::env::temp_dir().join(format!("envbyte-test-{}", uuid::Uuid::new_v4()));
        fs::create_dir(&directory).unwrap();
        secure_atomic_write(&directory.join("secret"), b"value").unwrap();
        let entries: Vec<_> = fs::read_dir(&directory)
            .unwrap()
            .filter_map(Result::ok)
            .map(|entry| entry.file_name().to_string_lossy().to_string())
            .collect();
        assert_eq!(entries, vec!["secret".to_string()]);
        fs::remove_dir_all(directory).unwrap();
    }
}
