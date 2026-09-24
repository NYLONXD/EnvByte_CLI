//! Reading version numbers and checksums out of what GitHub and envbyte print.

/// The version GitHub's `releases/latest` page redirects to:
/// `https://github.com/<repo>/releases/tag/v0.4.1` gives `0.4.1`.
pub fn version_from_tag_url(location: &str) -> Option<String> {
    let tag = location.split_once("/releases/tag/")?.1;
    let tag = tag.split(['?', '#']).next()?.trim_end_matches('/');
    clean_version(tag)
}

/// A version as someone might type it, `v0.4.1` or `0.4.1`, as `0.4.1`.
/// `None` for anything that is not a version number, since it ends up in a
/// download URL.
pub fn clean_version(input: &str) -> Option<String> {
    let input = input.trim();
    let version = input.strip_prefix('v').unwrap_or(input);
    let valid = version.starts_with(|c: char| c.is_ascii_digit())
        && version
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '+'));
    valid.then(|| version.to_string())
}

/// The version in `envbyte --version` output, which reads `envbyte 0.4.1`.
pub fn version_from_cli_output(output: &str) -> Option<String> {
    clean_version(output.split_whitespace().nth(1)?)
}

/// The digest in a `.sha256` file as `sha256sum` writes it:
/// `<64 hex digits>  <file name>`.
pub fn checksum_from_file(text: &str) -> Option<String> {
    let digest = text.split_whitespace().next()?;
    (digest.len() == 64 && digest.chars().all(|c| c.is_ascii_hexdigit()))
        .then(|| digest.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_version_from_the_latest_release_redirect() {
        let location = "https://github.com/NYLONXD/EnvByte_CLI/releases/tag/v0.4.1";
        assert_eq!(version_from_tag_url(location).as_deref(), Some("0.4.1"));
        assert_eq!(
            version_from_tag_url("https://github.com/o/r/releases/tag/v1.2.0-rc.1/?x=1").as_deref(),
            Some("1.2.0-rc.1")
        );
    }

    #[test]
    fn a_redirect_without_a_tag_means_nothing_is_released() {
        // What GitHub answers for a repository with no releases yet.
        assert_eq!(
            version_from_tag_url("https://github.com/NYLONXD/EnvByte_CLI/releases"),
            None
        );
        assert_eq!(
            version_from_tag_url("https://github.com/o/r/releases/tag/nightly"),
            None
        );
    }

    #[test]
    fn cleans_typed_versions_and_refuses_anything_else() {
        assert_eq!(clean_version(" v0.4.0 ").as_deref(), Some("0.4.0"));
        assert_eq!(clean_version("0.4.0").as_deref(), Some("0.4.0"));
        assert_eq!(clean_version(""), None);
        assert_eq!(clean_version("v"), None);
        assert_eq!(clean_version("latest"), None);
        assert_eq!(clean_version("0.4.0/../../evil"), None);
    }

    #[test]
    fn reads_the_version_envbyte_reports() {
        assert_eq!(
            version_from_cli_output("envbyte 0.4.1\r\n").as_deref(),
            Some("0.4.1")
        );
        assert_eq!(version_from_cli_output("envbyte"), None);
        assert_eq!(version_from_cli_output(""), None);
    }

    #[test]
    fn reads_sha256sum_output() {
        let digest = "A".repeat(64);
        let file = format!("{digest}  envbyte-x86_64-pc-windows-msvc.zip\n");
        assert_eq!(checksum_from_file(&file), Some("a".repeat(64)));
        assert_eq!(checksum_from_file(&"a".repeat(63)), None);
        assert_eq!(checksum_from_file(&"g".repeat(64)), None);
        assert_eq!(checksum_from_file(""), None);
    }
}
