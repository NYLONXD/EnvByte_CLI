//! Envbyte Setup: the Windows installer behind the website's Download button.
//!
//! It is a console program, so double-clicking envbyte-setup.exe opens a
//! terminal window. It works out what this PC has, says what it will install
//! and where, and asks before changing anything. Then it downloads the latest
//! release's Windows archive, checks it against the SHA-256 published with the
//! release, installs envbyte.exe into %USERPROFILE%\.envbyte\bin and adds that
//! folder to the user PATH. That is what web/public/install.ps1 does, with the
//! same settings, for people who would rather download a program than paste a
//! command.
//!
//! Layering: `setup` runs the steps, `ui` prints and asks, `download` talks to
//! GitHub, `system` talks to Windows. `release` and `path_list` are plain text
//! handling, built on every platform so Linux CI runs their tests.

#[cfg(windows)]
mod download;
#[cfg(any(windows, test))]
mod path_list;
#[cfg(any(windows, test))]
mod release;
#[cfg(windows)]
mod setup;
#[cfg(windows)]
mod system;
#[cfg(windows)]
mod ui;

/// Where releases are published.
#[cfg(windows)]
const REPO: &str = "NYLONXD/EnvByte_CLI";
const SITE: &str = "https://envbyte.trackedge.in";

#[cfg(windows)]
fn main() {
    std::process::exit(setup::run());
}

#[cfg(not(windows))]
fn main() {
    eprintln!("envbyte-setup installs Envbyte on Windows. On macOS and Linux, run:");
    eprintln!("  curl -fsSL {SITE}/install.sh | sh");
    std::process::exit(1);
}
