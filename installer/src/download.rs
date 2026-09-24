//! Fetching a release from GitHub, and checking it before anything is installed.

use std::error::Error as _;
use std::fmt;
use std::io::{Cursor, Read};
use std::time::Duration;

use reqwest::blocking::{Client, Response};
use reqwest::redirect::Policy;
use reqwest::StatusCode;
use sha2::{Digest, Sha256};

use crate::{release, ui, REPO};

pub enum FetchError {
    /// GitHub has no such file: the version or the release does not exist.
    Missing,
    /// The network, a proxy or GitHub failed along the way.
    Failed(String),
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("not found"),
            Self::Failed(cause) => f.write_str(cause),
        }
    }
}

pub struct Downloader {
    client: Client,
    /// Stops at the first redirect, to read where it points.
    no_redirects: Client,
}

impl Downloader {
    pub fn new() -> Result<Self, String> {
        let build = |redirects: Policy| {
            Client::builder()
                .user_agent(concat!("envbyte-setup/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(20))
                // Room for the whole archive to arrive over a slow connection.
                .timeout(Duration::from_secs(15 * 60))
                .redirect(redirects)
                .build()
                .map_err(|error| describe(&error))
        };
        Ok(Self {
            client: build(Policy::default())?,
            no_redirects: build(Policy::none())?,
        })
    }

    /// The newest published version, or `None` when there is no release yet.
    /// It is read from where github.com/<repo>/releases/latest redirects to,
    /// which, unlike GitHub's API, has no hourly limit for anonymous callers.
    pub fn latest_version(&self) -> Result<Option<String>, String> {
        let url = format!("https://github.com/{REPO}/releases/latest");
        let response = self
            .no_redirects
            .get(&url)
            .send()
            .map_err(|error| describe(&error))?;
        if !response.status().is_redirection() {
            return Err(format!("GitHub answered {}", response.status()));
        }
        let location = response
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default();
        Ok(release::version_from_tag_url(location))
    }

    pub fn text(&self, url: &str) -> Result<String, FetchError> {
        self.get(url)?
            .text()
            .map_err(|error| FetchError::Failed(describe(&error)))
    }

    /// Downloads `url` into memory, drawing a progress bar as it goes.
    pub fn bytes(&self, url: &str) -> Result<Vec<u8>, FetchError> {
        let mut response = self.get(url)?;
        let total = response.content_length();
        let bar = ui::download_bar(total);
        let mut body = Vec::with_capacity(total.unwrap_or(0) as usize);
        let mut chunk = vec![0; 64 * 1024];
        loop {
            match response.read(&mut chunk) {
                Ok(0) => break,
                Ok(read) => {
                    body.extend_from_slice(&chunk[..read]);
                    bar.inc(read as u64);
                }
                Err(error) => {
                    bar.abandon();
                    return Err(FetchError::Failed(format!(
                        "the download broke off: {error}"
                    )));
                }
            }
        }
        bar.finish_and_clear();
        if total.is_some_and(|total| total != body.len() as u64) {
            return Err(FetchError::Failed("the download ended early".to_string()));
        }
        Ok(body)
    }

    fn get(&self, url: &str) -> Result<Response, FetchError> {
        let response = self
            .client
            .get(url)
            .send()
            .map_err(|error| FetchError::Failed(describe(&error)))?;
        match response.status() {
            StatusCode::NOT_FOUND => Err(FetchError::Missing),
            status if !status.is_success() => {
                Err(FetchError::Failed(format!("GitHub answered {status}")))
            }
            _ => Ok(response),
        }
    }
}

pub fn sha256_hex(bytes: &[u8]) -> String {
    hex::encode(Sha256::digest(bytes))
}

/// Takes envbyte.exe out of a release archive, which keeps it in a folder
/// named after the target.
pub fn extract_binary(archive: &[u8]) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipArchive::new(Cursor::new(archive))
        .map_err(|error| format!("the archive is not a readable zip file ({error})"))?;
    for index in 0..zip.len() {
        let mut entry = zip
            .by_index(index)
            .map_err(|error| format!("the archive is damaged ({error})"))?;
        let name = entry.name().replace('\\', "/");
        if name != "envbyte.exe" && !name.ends_with("/envbyte.exe") {
            continue;
        }
        let mut binary = Vec::with_capacity(entry.size() as usize);
        entry
            .read_to_end(&mut binary)
            .map_err(|error| format!("could not unpack envbyte.exe ({error})"))?;
        // Every Windows program starts with these two bytes.
        if !binary.starts_with(b"MZ") {
            return Err("the envbyte.exe in the archive is not a Windows program".to_string());
        }
        return Ok(binary);
    }
    Err("the archive has no envbyte.exe in it".to_string())
}

/// reqwest's own message is only the outermost layer ("error sending
/// request"); the reason, such as a DNS failure or a refused connection, is
/// further down the chain.
fn describe(error: &reqwest::Error) -> String {
    let mut message = error.to_string();
    let mut source = error.source();
    while let Some(cause) = source {
        let text = cause.to_string();
        if !message.contains(&text) {
            message = format!("{message}: {text}");
        }
        source = cause.source();
    }
    message
}
