//! What Setup asks of Windows itself: which version and processor this is,
//! whether it owns its console window, and the user PATH in the registry.

use std::io;
use std::path::Path;

use windows_sys::Win32::Foundation::LPARAM;
use windows_sys::Win32::System::Console::GetConsoleProcessList;
use windows_sys::Win32::UI::WindowsAndMessaging::{
    SendMessageTimeoutW, HWND_BROADCAST, SMTO_ABORTIFHUNG, WM_SETTINGCHANGE,
};
use winreg::enums::{RegType, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, KEY_WRITE};
use winreg::{RegKey, RegValue};

use crate::path_list;

/// Whether Setup is the only program attached to its console. That is the case
/// when Windows opened the window for it because it was double-clicked, rather
/// than it being run from a terminal that stays open afterwards.
pub fn owns_console() -> bool {
    let mut processes = [0u32; 2];
    // SAFETY: the pointer and the length describe the same live array.
    let attached = unsafe { GetConsoleProcessList(processes.as_mut_ptr(), processes.len() as u32) };
    attached == 1
}

pub enum Arch {
    X64,
    /// Runs x64 programs through the emulation built into Windows 11.
    Arm64,
    Unsupported(String),
}

/// The processor Windows itself runs on. The registry is asked rather than
/// this process's environment, which reports AMD64 to an x64 program running
/// under emulation on ARM64.
pub fn native_arch() -> Arch {
    let reported = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SYSTEM\CurrentControlSet\Control\Session Manager\Environment")
        .and_then(|key| key.get_value::<String, _>("PROCESSOR_ARCHITECTURE"))
        .ok()
        .or_else(|| std::env::var("PROCESSOR_ARCHITECTURE").ok())
        .unwrap_or_default();
    match reported.to_ascii_uppercase().as_str() {
        "AMD64" => Arch::X64,
        "ARM64" => Arch::Arm64,
        _ => Arch::Unsupported(reported),
    }
}

/// For example `Windows 11 24H2 (build 26100)`.
pub fn windows_version() -> String {
    let Ok(key) = RegKey::predef(HKEY_LOCAL_MACHINE)
        .open_subkey(r"SOFTWARE\Microsoft\Windows NT\CurrentVersion")
    else {
        return "Windows".to_string();
    };
    let build: Option<String> = key.get_value("CurrentBuild").ok();
    let release: Option<String> = key
        .get_value("DisplayVersion")
        .or_else(|_| key.get_value("ReleaseId"))
        .ok();
    // Windows 11 still names itself Windows 10 in ProductName; the build
    // number is what tells them apart.
    let name = match build.as_deref().and_then(|b| b.parse::<u32>().ok()) {
        Some(22000..) => "Windows 11",
        Some(_) => "Windows 10",
        None => "Windows",
    };
    match (release, build) {
        (Some(release), Some(build)) => format!("{name} {release} (build {build})"),
        (None, Some(build)) => format!("{name} (build {build})"),
        _ => name.to_string(),
    }
}

const ENVIRONMENT: &str = "Environment";
const PATH: &str = "Path";

/// Whether the user PATH saved in the registry already has `dir`.
pub fn user_path_has(dir: &Path) -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(ENVIRONMENT)
        .ok()
        .and_then(|key| read_path(&key).ok())
        .is_some_and(|(path, _)| path_list::contains(&path, &dir.to_string_lossy(), variable))
}

/// Puts `dir` first in the user PATH, then tells Windows the environment
/// changed, so terminals opened from the Start menu or taskbar from now on
/// pick it up. Returns false when `dir` was already there.
pub fn add_to_user_path(dir: &Path) -> Result<bool, String> {
    let key = RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey_with_flags(ENVIRONMENT, KEY_READ | KEY_WRITE)
        .map_err(|error| format!("could not open HKEY_CURRENT_USER\\Environment: {error}"))?;
    let added = add_to_path_in(&key, &dir.to_string_lossy())?;
    if added {
        announce_environment_change();
    }
    Ok(added)
}

fn add_to_path_in(key: &RegKey, dir: &str) -> Result<bool, String> {
    let (path, stored_as) = read_path(key)?;
    if path_list::contains(&path, dir, variable) {
        return Ok(false);
    }
    let updated = RegValue {
        bytes: registry_text(&path_list::prepend(&path, dir)).into(),
        vtype: stored_as,
    };
    key.set_raw_value(PATH, &updated)
        .map_err(|error| format!("could not save the user PATH: {error}"))?;
    Ok(true)
}

/// The user PATH, and the type it is stored as. That is normally
/// REG_EXPAND_SZ, which is what lets entries use `%VARIABLES%`; saving it back
/// as plain REG_SZ would break every such entry, so the type is kept.
fn read_path(key: &RegKey) -> Result<(String, RegType), String> {
    match key.get_raw_value(PATH) {
        Ok(value) => match value.vtype {
            RegType::REG_SZ | RegType::REG_EXPAND_SZ => {
                let text = from_registry_text(&value.bytes).ok_or(
                    "the user PATH in the registry is not valid text, so Setup left it alone",
                )?;
                Ok((text, value.vtype))
            }
            other => Err(format!(
                "the user PATH is stored as {other:?} rather than text, so Setup left it alone"
            )),
        },
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            Ok((String::new(), RegType::REG_EXPAND_SZ))
        }
        Err(error) => Err(format!("could not read the user PATH: {error}")),
    }
}

fn variable(name: &str) -> Option<String> {
    std::env::var(name).ok()
}

/// Registry strings are UTF-16, usually ending in a NUL.
fn from_registry_text(bytes: &[u8]) -> Option<String> {
    if !bytes.len().is_multiple_of(2) {
        return None;
    }
    let units: Vec<u16> = bytes
        .chunks_exact(2)
        .map(|pair| u16::from_le_bytes([pair[0], pair[1]]))
        .collect();
    let end = units
        .iter()
        .rposition(|&unit| unit != 0)
        .map_or(0, |i| i + 1);
    String::from_utf16(&units[..end]).ok()
}

fn registry_text(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain([0])
        .flat_map(u16::to_le_bytes)
        .collect()
}

/// Explorer re-reads the environment when it gets this, and everything it
/// starts afterwards inherits the new PATH.
fn announce_environment_change() {
    let area: Vec<u16> = "Environment\0".encode_utf16().collect();
    let mut result = 0;
    // SAFETY: `area` is NUL-terminated and outlives the call, which returns
    // within the timeout even if some window is not responding.
    unsafe {
        SendMessageTimeoutW(
            HWND_BROADCAST,
            WM_SETTINGCHANGE,
            0,
            area.as_ptr() as LPARAM,
            SMTO_ABORTIFHUNG,
            5000,
            &mut result,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A throwaway key under HKEY_CURRENT_USER\Software, deleted when dropped,
    /// standing in for HKEY_CURRENT_USER\Environment.
    struct ScratchKey {
        name: String,
        key: RegKey,
    }

    impl ScratchKey {
        fn new(test: &str) -> Self {
            let name = format!(r"Software\EnvbyteSetupTest-{test}-{}", std::process::id());
            let (key, _) = RegKey::predef(HKEY_CURRENT_USER)
                .create_subkey(&name)
                .unwrap();
            Self { name, key }
        }
    }

    impl Drop for ScratchKey {
        fn drop(&mut self) {
            let _ = RegKey::predef(HKEY_CURRENT_USER).delete_subkey_all(&self.name);
        }
    }

    #[test]
    fn keeps_variables_and_the_expandable_type() {
        let scratch = ScratchKey::new("expand");
        let before = r"%USERPROFILE%\.cargo\bin;C:\tools";
        scratch
            .key
            .set_raw_value(
                PATH,
                &RegValue {
                    bytes: registry_text(before).into(),
                    vtype: RegType::REG_EXPAND_SZ,
                },
            )
            .unwrap();

        assert!(add_to_path_in(&scratch.key, r"C:\Envbyte\bin").unwrap());
        let after = scratch.key.get_raw_value(PATH).unwrap();
        assert_eq!(after.vtype, RegType::REG_EXPAND_SZ);
        assert_eq!(
            from_registry_text(&after.bytes).unwrap(),
            format!(r"C:\Envbyte\bin;{before}")
        );

        // A second run finds it and changes nothing.
        assert!(!add_to_path_in(&scratch.key, r"c:\envbyte\bin\").unwrap());
        assert_eq!(scratch.key.get_raw_value(PATH).unwrap().bytes, after.bytes);
    }

    #[test]
    fn creates_the_path_when_there_is_none() {
        let scratch = ScratchKey::new("create");
        assert!(add_to_path_in(&scratch.key, r"C:\Envbyte\bin").unwrap());
        let after = scratch.key.get_raw_value(PATH).unwrap();
        assert_eq!(after.vtype, RegType::REG_EXPAND_SZ);
        assert_eq!(from_registry_text(&after.bytes).unwrap(), r"C:\Envbyte\bin");
    }

    #[test]
    fn leaves_a_path_that_is_not_text_alone() {
        let scratch = ScratchKey::new("binary");
        scratch.key.set_value(PATH, &7u32).unwrap();
        assert!(add_to_path_in(&scratch.key, r"C:\Envbyte\bin").is_err());
        assert_eq!(scratch.key.get_value::<u32, _>(PATH).unwrap(), 7);
    }
}
