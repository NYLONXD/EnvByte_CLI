//! The user PATH as text: `;`-separated folders, where two entries can name the
//! same folder while differing in case, a trailing backslash, surrounding
//! quotes, or a `%VARIABLE%` standing in for part of it.

/// Whether `dir` is already one of the folders in `path`. `lookup` resolves
/// `%VARIABLES%` in the entries.
pub fn contains(path: &str, dir: &str, lookup: impl Fn(&str) -> Option<String>) -> bool {
    path.split(';')
        .any(|entry| same_folder(&expand(entry, &lookup), dir))
}

/// `path` with `dir` added in front. Everything already there is kept exactly
/// as written, `%VARIABLES%` included.
pub fn prepend(path: &str, dir: &str) -> String {
    let rest = path.trim_start_matches(';');
    if rest.is_empty() {
        dir.to_string()
    } else {
        format!("{dir};{rest}")
    }
}

/// Whether two folder names point at the same folder, the way Windows compares
/// them.
pub fn same_folder(a: &str, b: &str) -> bool {
    let a = normalize(a);
    !a.is_empty() && a == normalize(b)
}

fn normalize(entry: &str) -> String {
    entry
        .trim()
        .trim_matches('"')
        .trim_end_matches(['\\', '/'])
        .to_lowercase()
}

/// Replaces each `%NAME%` that `lookup` knows. Unknown ones stay as written,
/// which is what Windows does with them.
fn expand(entry: &str, lookup: &impl Fn(&str) -> Option<String>) -> String {
    let mut expanded = String::new();
    let mut rest = entry;
    while let Some(start) = rest.find('%') {
        let after = &rest[start + 1..];
        let Some(end) = after.find('%') else { break };
        let name = &after[..end];
        expanded.push_str(&rest[..start]);
        match lookup(name).filter(|_| !name.is_empty()) {
            Some(value) => expanded.push_str(&value),
            None => expanded.push_str(&rest[start..start + end + 2]),
        }
        rest = &after[end + 1..];
    }
    expanded.push_str(rest);
    expanded
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup(name: &str) -> Option<String> {
        (name.eq_ignore_ascii_case("USERPROFILE")).then(|| r"C:\Users\priya".to_string())
    }

    const DIR: &str = r"C:\Users\priya\.envbyte\bin";

    #[test]
    fn finds_the_folder_however_it_is_written() {
        for path in [
            r"C:\Windows;C:\Users\priya\.envbyte\bin",
            r"c:\users\PRIYA\.envbyte\bin\;C:\Windows",
            r#""C:\Users\priya\.envbyte\bin";C:\Windows"#,
            r"%USERPROFILE%\.envbyte\bin",
            r"%userprofile%\.envbyte\bin\",
        ] {
            assert!(contains(path, DIR, lookup), "{path}");
        }
    }

    #[test]
    fn does_not_match_other_folders() {
        for path in [
            "",
            ";;",
            r"C:\Users\priya\.envbyte",
            r"C:\Users\priya\.envbyte\bin2",
            r"%UNKNOWN%\.envbyte\bin",
        ] {
            assert!(!contains(path, DIR, lookup), "{path}");
        }
    }

    #[test]
    fn keeps_unknown_variables_as_written() {
        assert_eq!(expand(r"%NOPE%\x", &lookup), r"%NOPE%\x");
        assert_eq!(
            expand(r"%NOPE%;%USERPROFILE%\x", &lookup),
            r"%NOPE%;C:\Users\priya\x"
        );
        assert_eq!(expand("100%", &lookup), "100%");
        assert_eq!(expand("%%", &lookup), "%%");
    }

    #[test]
    fn prepends_without_touching_the_rest() {
        assert_eq!(prepend("", DIR), DIR);
        assert_eq!(prepend(";", DIR), DIR);
        assert_eq!(
            prepend(r"%USERPROFILE%\.cargo\bin;C:\tools;", DIR),
            format!(r"{DIR};%USERPROFILE%\.cargo\bin;C:\tools;")
        );
    }

    #[test]
    fn an_empty_entry_is_no_folder() {
        assert!(!same_folder("", ""));
        assert!(!same_folder("  ", DIR));
    }
}
