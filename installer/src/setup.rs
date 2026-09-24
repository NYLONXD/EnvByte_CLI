//! The install itself: look at the machine, say what will happen, ask, then do
//! it. Nothing on disk or in the registry changes before the user says yes.

use std::env;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::thread;
use std::time::Duration;

use indicatif::HumanBytes;

use crate::download::{self, Downloader, FetchError};
use crate::system::{self, Arch};
use crate::ui::{self, PathState};
use crate::{path_list, release, REPO, SITE};

/// The release archive holding the Windows build, the x64 one. ARM64 runs it
/// through emulation. It keeps envbyte.exe in a folder of the same name.
const ARCHIVE: &str = "envbyte-x86_64-pc-windows-msvc.zip";

const NETWORK_HINT: &str = "Check your internet connection and try again.
Behind a proxy? Set HTTPS_PROXY to its address, then run Setup again.";

/// What went wrong, why, and what the user can do about it.
pub struct Failure {
    pub message: String,
    pub cause: Option<String>,
    pub hint: Option<String>,
}

impl Failure {
    fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            cause: None,
            hint: None,
        }
    }

    fn cause(mut self, cause: impl ToString) -> Self {
        self.cause = Some(cause.to_string());
        self
    }

    fn hint(mut self, hint: impl Into<String>) -> Self {
        self.hint = Some(hint.into());
        self
    }
}

/// Runs Setup and returns the process exit code.
pub fn run() -> i32 {
    ui::init();
    let own_window = system::owns_console();

    let code = match parse_arguments(env::args().skip(1)) {
        Ok(Mode::Help) => {
            ui::help();
            0
        }
        Ok(Mode::Install { assume_yes }) => match install(assume_yes) {
            Ok(()) => 0,
            Err(failure) => {
                ui::fail(&failure);
                1
            }
        },
        Err(failure) => {
            ui::fail(&failure);
            2
        }
    };

    if own_window {
        ui::pause();
    }
    code
}

enum Mode {
    Install { assume_yes: bool },
    Help,
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Mode, Failure> {
    let mut assume_yes = false;
    for argument in arguments {
        match argument.as_str() {
            "-y" | "--yes" | "/y" => assume_yes = true,
            "-h" | "--help" | "/?" => return Ok(Mode::Help),
            other => {
                return Err(Failure::new(format!("Unknown option '{other}'."))
                    .hint("Run envbyte-setup --help to see the options."))
            }
        }
    }
    Ok(Mode::Install { assume_yes })
}

#[derive(PartialEq)]
enum PathPlan {
    Add,
    AlreadyThere,
    /// ENVBYTE_NO_MODIFY_PATH is set.
    LeaveAlone,
}

fn install(assume_yes: bool) -> Result<(), Failure> {
    ui::title();

    // What this machine has.
    let arch = system::native_arch();
    let arch_label = match &arch {
        Arch::X64 => "x64",
        Arch::Arm64 => "ARM64",
        Arch::Unsupported(name) => {
            return Err(Failure::new("Envbyte needs 64-bit Windows.")
                .cause(format!("This PC reports its processor as '{name}'.")))
        }
    };
    let install_dir = install_dir()?;
    let destination = install_dir.join("envbyte.exe");
    let current = installed_version(&destination);
    let path_plan = if env::var("ENVBYTE_NO_MODIFY_PATH").is_ok_and(|value| value == "1") {
        PathPlan::LeaveAlone
    } else if system::user_path_has(&install_dir) {
        PathPlan::AlreadyThere
    } else {
        PathPlan::Add
    };

    // What would be installed.
    let downloader = Downloader::new()
        .map_err(|cause| Failure::new("Could not start the downloader.").cause(cause))?;
    let requested = env::var("ENVBYTE_VERSION")
        .ok()
        .filter(|value| !value.trim().is_empty());
    let spinner = ui::spinner("Looking up the release");
    let found = find_release(&downloader, requested.as_deref());
    spinner.finish_and_clear();
    let (version, expected) = found?;

    // The plan, before anything changes.
    let dir = install_dir.display().to_string();
    let status = match current.as_deref() {
        None => String::new(),
        Some(current) if current == version => ", already installed".to_string(),
        Some(current) => format!(", replacing {current}"),
    };
    let which = if requested.is_some() { "" } else { " (latest)" };
    ui::field(
        "System",
        &format!("{}, {arch_label}", system::windows_version()),
    );
    ui::field("Envbyte", &format!("{version}{which}{status}"));
    ui::field("Install to", &destination.display().to_string());
    ui::field(
        "PATH",
        &match path_plan {
            PathPlan::Add => format!("adds {dir} to your user PATH"),
            PathPlan::AlreadyThere => format!("{dir} is already on your PATH"),
            PathPlan::LeaveAlone => "left unchanged (ENVBYTE_NO_MODIFY_PATH is set)".to_string(),
        },
    );
    ui::field(
        "Download",
        &format!("github.com/{REPO}, checked against its published SHA-256"),
    );
    ui::blank();
    ui::note("No administrator rights needed. Nothing else on this PC is changed.");
    if matches!(arch, Arch::Arm64) {
        ui::note("On ARM64, Windows runs Envbyte's x64 build through its built-in emulation.");
    }
    for other in other_copies(&install_dir) {
        ui::warn(&format!(
            "Another envbyte is installed at {}. Whichever comes first on PATH is the one that runs.",
            other.display()
        ));
    }
    ui::blank();

    let question = match current.as_deref() {
        None => "Do you want to install Envbyte?".to_string(),
        Some(current) if current == version => "Do you want to reinstall Envbyte?".to_string(),
        Some(current) => format!("Do you want to replace Envbyte {current} with {version}?"),
    };
    if !ui::confirm(&question, assume_yes) {
        ui::blank();
        ui::note("Setup cancelled. Nothing was changed.");
        return Ok(());
    }
    let add_to_path = path_plan == PathPlan::Add
        && ui::confirm(
            "Add Envbyte to your PATH so you can run it from any terminal?",
            assume_yes,
        );
    ui::blank();

    // Download and check.
    let archive = downloader
        .bytes(&format!("{}/{ARCHIVE}", release_url(&version)))
        .map_err(|error| fetch_failure(error, ARCHIVE, &version, requested.is_some()))?;
    ui::done(&format!(
        "Downloaded {ARCHIVE} ({})",
        HumanBytes(archive.len() as u64)
    ));

    // A secrets tool must not install a binary it could not verify.
    let actual = download::sha256_hex(&archive);
    if actual != expected {
        return Err(Failure::new(
            "The download does not match its published checksum, so it was not installed.",
        )
        .cause(format!("expected {expected}, got {actual}"))
        .hint(format!(
            "Try again. If it keeps happening, report it at https://github.com/{REPO}/issues."
        )));
    }
    ui::done("Checksum verified (SHA-256)");

    let binary = download::extract_binary(&archive).map_err(|cause| {
        Failure::new(format!("Could not unpack {ARCHIVE}."))
            .cause(cause)
            .hint(format!(
                "Please report this at https://github.com/{REPO}/issues."
            ))
    })?;

    // Install.
    place(&binary, &install_dir).map_err(|cause| {
        Failure::new(format!("Could not install envbyte.exe into {dir}."))
            .cause(cause)
            .hint(
                "Close any program using that folder and try again, or set \
                 ENVBYTE_INSTALL_DIR to a folder you can write to.",
            )
    })?;
    let running = installed_version(&destination).ok_or_else(|| {
        Failure::new(format!(
            "Installed {}, but it does not start.",
            destination.display()
        ))
        .hint("Antivirus software may have blocked or removed it. Check its quarantine, then run Setup again.")
    })?;
    ui::done(&format!("Installed Envbyte {running} to {dir}"));

    let path_state = match path_plan {
        PathPlan::AlreadyThere => PathState::AlreadyThere,
        PathPlan::LeaveAlone => PathState::Missing,
        PathPlan::Add if !add_to_path => PathState::Missing,
        PathPlan::Add => match system::add_to_user_path(&install_dir) {
            Ok(_) => {
                ui::done(&format!("Added {dir} to your user PATH"));
                PathState::Added
            }
            Err(cause) => {
                ui::warn(&format!("Could not add {dir} to your PATH: {cause}"));
                PathState::Missing
            }
        },
    };

    ui::installed(path_state, &destination);
    Ok(())
}

/// The version to install and the SHA-256 published for its archive. The
/// checksum is tiny, and fetching it before asking anything confirms that the
/// release exists and has a Windows download.
fn find_release(
    downloader: &Downloader,
    requested: Option<&str>,
) -> Result<(String, String), Failure> {
    let version = match requested {
        Some(requested) => release::clean_version(requested).ok_or_else(|| {
            Failure::new(format!(
                "ENVBYTE_VERSION is set to '{requested}', which is not a version number."
            ))
            .hint("Set it to a version such as 0.4.1, or remove it to install the latest release.")
        })?,
        None => match downloader.latest_version() {
            Ok(Some(version)) => version,
            Ok(None) => {
                return Err(
                    Failure::new("No Envbyte release has been published yet.").hint(format!(
                        "Check https://github.com/{REPO}/releases, or see {SITE} for other ways to install."
                    )),
                )
            }
            Err(cause) => {
                return Err(Failure::new(
                    "Could not reach GitHub to find the latest Envbyte release.",
                )
                .cause(cause)
                .hint(NETWORK_HINT))
            }
        },
    };

    let checksum_name = format!("{ARCHIVE}.sha256");
    let checksum = downloader
        .text(&format!("{}/{checksum_name}", release_url(&version)))
        .map_err(|error| fetch_failure(error, &checksum_name, &version, requested.is_some()))?;
    let expected = release::checksum_from_file(&checksum).ok_or_else(|| {
        Failure::new(format!("{checksum_name} is not a SHA-256 checksum.")).hint(format!(
            "Please report this at https://github.com/{REPO}/issues."
        ))
    })?;
    Ok((version, expected))
}

fn release_url(version: &str) -> String {
    format!("https://github.com/{REPO}/releases/download/v{version}")
}

fn fetch_failure(error: FetchError, file: &str, version: &str, requested: bool) -> Failure {
    match error {
        FetchError::Missing => Failure::new(format!("Envbyte {version} has no Windows download."))
            .hint(if requested {
                "Check the version in ENVBYTE_VERSION, or remove it to install the latest release."
            } else {
                "The release may still be publishing. Try again in a few minutes."
            }),
        FetchError::Failed(cause) => Failure::new(format!("Could not download {file}."))
            .cause(cause)
            .hint(NETWORK_HINT),
    }
}

/// %USERPROFILE%\.envbyte\bin, the same folder install.ps1 uses, unless
/// ENVBYTE_INSTALL_DIR names another.
fn install_dir() -> Result<PathBuf, Failure> {
    if let Some(dir) = env::var_os("ENVBYTE_INSTALL_DIR").filter(|dir| !dir.is_empty()) {
        return std::path::absolute(&dir).map_err(|error| {
            Failure::new(format!(
                "ENVBYTE_INSTALL_DIR is set to '{}', which is not a usable folder.",
                dir.to_string_lossy()
            ))
            .cause(error)
        });
    }
    dirs::home_dir()
        .map(|home| home.join(".envbyte").join("bin"))
        .ok_or_else(|| {
            Failure::new("Could not find your user profile folder.")
                .hint("Set ENVBYTE_INSTALL_DIR to the folder to install into.")
        })
}

/// The version of the envbyte.exe at `path`, if there is one and it runs.
fn installed_version(path: &Path) -> Option<String> {
    if !path.is_file() {
        return None;
    }
    let output = Command::new(path).arg("--version").output().ok()?;
    if !output.status.success() {
        return None;
    }
    release::version_from_cli_output(&String::from_utf8_lossy(&output.stdout))
}

/// Other `envbyte` commands on PATH, from cargo or npm for example.
fn other_copies(install_dir: &Path) -> Vec<PathBuf> {
    let Some(path) = env::var_os("PATH") else {
        return Vec::new();
    };
    let own = install_dir.to_string_lossy();
    let mut found: Vec<PathBuf> = Vec::new();
    for dir in env::split_paths(&path) {
        if path_list::same_folder(&dir.to_string_lossy(), &own) {
            continue;
        }
        for name in ["envbyte.exe", "envbyte.cmd"] {
            let candidate = dir.join(name);
            if candidate.is_file() && !found.contains(&candidate) {
                found.push(candidate);
            }
        }
    }
    found
}

/// Puts the new envbyte.exe in place. Windows will not overwrite a program
/// that is running, but it will rename one, so the old copy moves aside first
/// and an upgrade works even while envbyte is open in another terminal. If the
/// swap fails, the old copy goes back.
fn place(binary: &[u8], dir: &Path) -> Result<(), String> {
    fs::create_dir_all(dir).map_err(|error| format!("could not create the folder: {error}"))?;
    let target = dir.join("envbyte.exe");
    let staged = dir.join("envbyte.exe.new");
    let aside = dir.join("envbyte.exe.old");

    fs::write(&staged, binary).map_err(|error| format!("could not write envbyte.exe: {error}"))?;
    // Left over from an upgrade while the previous copy was running.
    let _ = fs::remove_file(&aside);

    let replacing = target.exists();
    if replacing {
        if let Err(error) = retry(|| fs::rename(&target, &aside)) {
            let _ = fs::remove_file(&staged);
            return Err(format!("could not move the old envbyte.exe aside: {error}"));
        }
    }
    if let Err(error) = retry(|| fs::rename(&staged, &target)) {
        if replacing {
            let _ = fs::rename(&aside, &target);
        }
        let _ = fs::remove_file(&staged);
        return Err(format!(
            "could not put the new envbyte.exe in place: {error}"
        ));
    }
    // Fails while the old copy is still running; the next install clears it.
    let _ = fs::remove_file(&aside);
    Ok(())
}

/// Antivirus scanners lock a program for a moment right after it is written,
/// which makes renaming it fail until they let go.
fn retry(mut operation: impl FnMut() -> io::Result<()>) -> io::Result<()> {
    let mut attempts = 0;
    loop {
        match operation() {
            Err(error) if error.kind() != io::ErrorKind::NotFound && attempts < 10 => {
                attempts += 1;
                thread::sleep(Duration::from_millis(200));
            }
            result => return result,
        }
    }
}
