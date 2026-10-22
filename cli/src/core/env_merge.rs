//! Three-way merge of `.env` files, key by key.
//!
//! The server only ever holds ciphertext, so when two devices edit the same
//! file the merge has to happen here. Each side's edits are measured against
//! the version both started from: a key only one side touched takes that
//! side's value, and a key both changed differently is a conflict for the user
//! to settle.
//!
//! The result keeps the local file's layout - its order, comments and line
//! endings - and slots in what came from the server. Comment-only edits made
//! on the server side are not carried over.

use std::collections::{BTreeSet, HashMap, HashSet};

/// Which copy wins a conflicting key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Side {
    Local,
    Server,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Added,
    Changed,
    Removed,
}

impl ChangeKind {
    pub fn describe(self) -> &'static str {
        match self {
            Self::Added => "added",
            Self::Changed => "changed",
            Self::Removed => "removed",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub key: String,
    pub kind: ChangeKind,
}

/// A key both sides changed, differently. `None` means that side removed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conflict {
    pub key: String,
    pub local: Option<String>,
    pub server: Option<String>,
}

/// What a key looked like in the version both sides started from.
enum Base<'a> {
    Known(Option<&'a str>),
    Unknown,
}
use Base::{Known, Unknown};

/// What merging would do, before any conflict is settled.
pub struct MergePlan {
    local: Document,
    server: Document,
    /// The outcome for every key that needs no decision; `None` drops it.
    settled: HashMap<String, Option<String>>,
    /// Server edits that will land in the local file.
    pub from_server: Vec<Change>,
    /// Local edits the server copy does not have yet.
    pub from_local: Vec<Change>,
    pub conflicts: Vec<Conflict>,
}

/// Plans a merge of `local` and `server`, both edited from `base`. Without a
/// base nothing can be called unchanged, so a key the two copies disagree on
/// is a conflict, and a key only one copy has is kept.
pub fn plan(base: Option<&str>, local: &str, server: &str) -> MergePlan {
    let base = base.map(Document::parse);
    let local = Document::parse(local);
    let server = Document::parse(server);

    let base_values = base.as_ref().map(Document::values);
    let local_values = local.values();
    let server_values = server.values();

    // Keys in the order a reader meets them: the local file first.
    let mut keys = Vec::new();
    let mut seen = HashSet::new();
    for document in [Some(&local), Some(&server), base.as_ref()]
        .into_iter()
        .flatten()
    {
        for key in document.keys() {
            if seen.insert(key) {
                keys.push(key.to_string());
            }
        }
    }

    let mut settled = HashMap::new();
    let mut from_server = Vec::new();
    let mut from_local = Vec::new();
    let mut conflicts = Vec::new();
    for key in keys {
        let mine = local_values.get(key.as_str()).copied();
        let theirs = server_values.get(key.as_str()).copied();
        if mine == theirs {
            settled.insert(key, mine.map(str::to_string));
            continue;
        }
        let original = match &base_values {
            Some(values) => Known(values.get(key.as_str()).copied()),
            None => Unknown,
        };
        match original {
            Known(original) if mine == original => {
                from_server.push(change(&key, original, theirs));
                settled.insert(key, theirs.map(str::to_string));
            }
            Known(original) if theirs == original => {
                from_local.push(change(&key, original, mine));
                settled.insert(key, mine.map(str::to_string));
            }
            Unknown if mine.is_none() => {
                from_server.push(change(&key, None, theirs));
                settled.insert(key, theirs.map(str::to_string));
            }
            Unknown if theirs.is_none() => {
                from_local.push(change(&key, None, mine));
                settled.insert(key, mine.map(str::to_string));
            }
            _ => conflicts.push(Conflict {
                key,
                local: mine.map(|text| value_of(text).to_string()),
                server: theirs.map(|text| value_of(text).to_string()),
            }),
        }
    }

    MergePlan {
        local,
        server,
        settled,
        from_server,
        from_local,
        conflicts,
    }
}

/// Whether two copies hold the same lines, whatever their line endings.
pub fn equivalent(first: &str, second: &str) -> bool {
    first.lines().eq(second.lines())
}

fn change(key: &str, before: Option<&str>, after: Option<&str>) -> Change {
    let kind = match (before, after) {
        (None, _) => ChangeKind::Added,
        (_, None) => ChangeKind::Removed,
        _ => ChangeKind::Changed,
    };
    Change {
        key: key.to_string(),
        kind,
    }
}

impl MergePlan {
    /// Writes the merged file, settling each conflict with `choose`.
    pub fn render(&self, choose: impl Fn(&Conflict) -> Side) -> String {
        let mut outcome = self.settled.clone();
        let local_values = self.local.values();
        let server_values = self.server.values();
        for conflict in &self.conflicts {
            let values = match choose(conflict) {
                Side::Local => &local_values,
                Side::Server => &server_values,
            };
            let text = values
                .get(conflict.key.as_str())
                .map(|text| text.to_string());
            outcome.insert(conflict.key.clone(), text);
        }

        // Keys the local file lacks go after the key that precedes them in
        // the server copy, so a new entry lands beside its neighbours.
        let in_local: HashSet<&str> = self.local.keys().collect();
        let mut anchor: Option<&str> = None;
        let mut placed = HashSet::new();
        let mut additions: Vec<(Option<&str>, &str)> = Vec::new();
        for (key, _) in self.server.entries() {
            if in_local.contains(key) {
                if matches!(outcome.get(key), Some(Some(_))) {
                    anchor = Some(key);
                }
            } else if let Some(Some(text)) = outcome.get(key) {
                if placed.insert(key) {
                    additions.push((anchor, text.as_str()));
                }
            }
        }

        let last_line_of: HashMap<&str, usize> = self
            .local
            .entries_indexed()
            .map(|(index, key)| (key, index))
            .collect();
        let mut lines: Vec<&str> = Vec::new();
        let mut leading_placed = false;
        for (index, item) in self.local.items.iter().enumerate() {
            match item {
                Item::Other(text) => lines.push(text),
                Item::Entry { key, text } => {
                    if !leading_placed {
                        lines.extend(added_after(&additions, None));
                        leading_placed = true;
                    }
                    match outcome.get(key.as_str()) {
                        Some(Some(chosen)) if chosen == local_values[key.as_str()] => {
                            lines.push(text);
                        }
                        Some(Some(chosen)) => lines.push(chosen),
                        _ => {}
                    }
                    if last_line_of[key.as_str()] == index {
                        lines.extend(added_after(&additions, Some(key.as_str())));
                    }
                }
            }
        }
        if !leading_placed {
            lines.extend(added_after(&additions, None));
        }

        let template = if self.local.items.is_empty() {
            &self.server
        } else {
            &self.local
        };
        // Joined on bare newlines first: a multi-line value carries its own.
        let mut merged = lines.join("\n");
        if template.trailing_newline && !merged.is_empty() {
            merged.push('\n');
        }
        if template.crlf {
            merged = merged.replace('\n', "\r\n");
        }
        merged
    }
}

fn added_after<'a>(
    additions: &'a [(Option<&'a str>, &'a str)],
    anchor: Option<&'a str>,
) -> impl Iterator<Item = &'a str> + 'a {
    additions
        .iter()
        .filter(move |(after, _)| *after == anchor)
        .map(|(_, text)| *text)
}

/// One logical line: an assignment, which spans several physical lines when
/// its quoted value does, or anything else - comments, blanks, junk.
#[derive(Debug, Clone, PartialEq)]
enum Item {
    Entry { key: String, text: String },
    Other(String),
}

struct Document {
    items: Vec<Item>,
    crlf: bool,
    trailing_newline: bool,
}

impl Document {
    fn parse(source: &str) -> Self {
        let mut items = Vec::new();
        let mut lines = source.lines();
        while let Some(line) = lines.next() {
            let Some((key, value)) = assignment(line) else {
                items.push(Item::Other(line.to_string()));
                continue;
            };
            let mut text = line.to_string();
            if let Some(quote) = opening_quote(value) {
                if !closes(&value[1..], quote) {
                    for next in lines.by_ref() {
                        text.push('\n');
                        text.push_str(next);
                        if closes(next, quote) {
                            break;
                        }
                    }
                }
            }
            items.push(Item::Entry {
                key: key.to_string(),
                text,
            });
        }
        Self {
            items,
            crlf: source.contains("\r\n"),
            trailing_newline: source.ends_with('\n'),
        }
    }

    fn entries(&self) -> impl Iterator<Item = (&str, &str)> {
        self.items.iter().filter_map(|item| match item {
            Item::Entry { key, text } => Some((key.as_str(), text.as_str())),
            Item::Other(_) => None,
        })
    }

    fn entries_indexed(&self) -> impl Iterator<Item = (usize, &str)> {
        self.items
            .iter()
            .enumerate()
            .filter_map(|(index, item)| match item {
                Item::Entry { key, .. } => Some((index, key.as_str())),
                Item::Other(_) => None,
            })
    }

    fn keys(&self) -> impl Iterator<Item = &str> {
        let mut seen = BTreeSet::new();
        self.entries()
            .map(|(key, _)| key)
            .filter(move |key| seen.insert(*key))
    }

    /// Each key's full assignment text. A repeated key counts at its last
    /// occurrence, as dotenv loaders read it.
    fn values(&self) -> HashMap<&str, &str> {
        self.entries().collect()
    }
}

/// Splits `KEY=value` (optionally `export KEY=value`) into key and raw value.
fn assignment(line: &str) -> Option<(&str, &str)> {
    let rest = line.trim_start();
    let rest = rest
        .strip_prefix("export")
        .filter(|after| after.starts_with([' ', '\t']))
        .map_or(rest, str::trim_start);
    let (key, value) = rest.split_once('=')?;
    let key = key.trim_end();
    let mut chars = key.chars();
    let first = chars.next()?;
    let valid = (first.is_ascii_alphabetic() || first == '_')
        && chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '-'));
    valid.then_some((key, value.trim_start()))
}

fn opening_quote(value: &str) -> Option<char> {
    value
        .chars()
        .next()
        .filter(|c| matches!(c, '"' | '\'' | '`'))
}

/// Whether `text` contains the closing `quote`. Only double quotes honour
/// backslash escapes.
fn closes(text: &str, quote: char) -> bool {
    let mut escaped = false;
    for c in text.chars() {
        if escaped {
            escaped = false;
        } else if c == '\\' && quote == '"' {
            escaped = true;
        } else if c == quote {
            return true;
        }
    }
    false
}

/// The value half of an assignment's text, for showing in a prompt. The
/// value runs from where the first line's value starts to the end.
fn value_of(text: &str) -> &str {
    let first = text.lines().next().unwrap_or_default();
    match assignment(first) {
        Some((_, value)) => &text[first.len() - value.len()..],
        None => text,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn merge(base: Option<&str>, local: &str, server: &str, side: Side) -> String {
        plan(base, local, server).render(|_| side)
    }

    #[test]
    fn takes_each_sides_edits_to_different_keys() {
        let base = "A=1\nB=2\nC=3\n";
        let local = "A=1\nB=local\nC=3\n";
        let server = "A=server\nB=2\nC=3\n";
        let plan = plan(Some(base), local, server);
        assert!(plan.conflicts.is_empty());
        assert_eq!(plan.from_server[0].key, "A");
        assert_eq!(plan.from_local[0].key, "B");
        assert_eq!(plan.render(|_| Side::Local), "A=server\nB=local\nC=3\n");
    }

    #[test]
    fn a_key_both_sides_changed_is_a_conflict() {
        let plan = plan(Some("A=1\n"), "A=mine\n", "A=theirs\n");
        assert_eq!(
            plan.conflicts,
            vec![Conflict {
                key: "A".to_string(),
                local: Some("mine".to_string()),
                server: Some("theirs".to_string()),
            }]
        );
        assert_eq!(plan.render(|_| Side::Local), "A=mine\n");
        assert_eq!(plan.render(|_| Side::Server), "A=theirs\n");
    }

    #[test]
    fn identical_edits_on_both_sides_agree() {
        let plan = plan(Some("A=1\n"), "A=2\n", "A=2\n");
        assert!(plan.conflicts.is_empty());
        assert!(plan.from_server.is_empty() && plan.from_local.is_empty());
    }

    #[test]
    fn removals_carry_over_and_conflict_with_edits() {
        let base = "A=1\nB=2\n";
        assert_eq!(
            merge(Some(base), "A=1\nB=2\n", "B=2\n", Side::Local),
            "B=2\n"
        );
        assert_eq!(
            merge(Some(base), "B=2\n", "A=1\nB=2\n", Side::Local),
            "B=2\n"
        );
        let removed_and_edited = plan(Some(base), "B=2\n", "A=edited\nB=2\n");
        assert_eq!(removed_and_edited.conflicts[0].local, None);
        assert_eq!(removed_and_edited.render(|_| Side::Local), "B=2\n");
    }

    #[test]
    fn server_additions_land_beside_their_neighbours() {
        let base = "# db\nDB_HOST=x\n\n# cache\nREDIS=y\n";
        let local = "# db\nDB_HOST=x\n\n# cache\nREDIS=y\nLOCAL_NEW=1\n";
        let server = "# db\nDB_HOST=x\nDB_PORT=5432\n\n# cache\nREDIS=y\n";
        assert_eq!(
            merge(Some(base), local, server, Side::Local),
            "# db\nDB_HOST=x\nDB_PORT=5432\n\n# cache\nREDIS=y\nLOCAL_NEW=1\n"
        );
    }

    #[test]
    fn an_addition_at_the_top_stays_at_the_top() {
        let merged = merge(
            Some("# header\nB=2\n"),
            "# header\nB=2\n",
            "A=1\nB=2\n",
            Side::Local,
        );
        assert_eq!(merged, "# header\nA=1\nB=2\n");
    }

    #[test]
    fn keeps_the_local_line_endings() {
        let merged = merge(
            Some("A=1\nB=2\n"),
            "A=1\r\nB=mine\r\n",
            "A=new\nB=2\n",
            Side::Local,
        );
        assert_eq!(merged, "A=new\r\nB=mine\r\n");
        let multiline = merge(None, "K=\"a\r\nb\"\r\n", "K=\"a\r\nb\"\r\n", Side::Local);
        assert_eq!(multiline, "K=\"a\r\nb\"\r\n");
        assert!(equivalent("A=1\r\nB=2\r\n", "A=1\nB=2\n"));
        assert!(!equivalent("A=1\n", "A=2\n"));
    }

    #[test]
    fn multiline_quoted_values_are_one_entry() {
        let base = "KEY=\"-----BEGIN\nold\n-----END\"\nA=1\n";
        let local = "KEY=\"-----BEGIN\nnew\n-----END\"\nA=1\n";
        let server = "KEY=\"-----BEGIN\nold\n-----END\"\nA=2\n";
        let plan = plan(Some(base), local, server);
        assert!(plan.conflicts.is_empty());
        assert_eq!(
            plan.render(|_| Side::Local),
            "KEY=\"-----BEGIN\nnew\n-----END\"\nA=2\n"
        );
    }

    #[test]
    fn understands_export_and_spacing() {
        assert_eq!(assignment("export API_KEY = abc"), Some(("API_KEY", "abc")));
        assert_eq!(assignment("  DB.URL=x"), Some(("DB.URL", "x")));
        assert_eq!(assignment("# A=1"), None);
        assert_eq!(assignment("exporter=1"), Some(("exporter", "1")));
        assert_eq!(value_of("export API_KEY = abc"), "abc");
        assert_eq!(value_of("export ex=1"), "1");
        assert_eq!(value_of("K=\"a\nb\""), "\"a\nb\"");
    }

    #[test]
    fn without_a_base_disagreements_are_conflicts_and_additions_are_kept() {
        let plan = plan(None, "A=1\nB=mine\n", "B=theirs\nC=3\n");
        assert_eq!(plan.conflicts.len(), 1);
        assert_eq!(plan.conflicts[0].key, "B");
        assert_eq!(plan.render(|_| Side::Server), "A=1\nB=theirs\nC=3\n");
    }

    #[test]
    fn a_repeated_key_is_rewritten_everywhere_it_appears() {
        let merged = merge(Some("A=1\nA=1\n"), "A=1\nA=1\n", "A=2\n", Side::Local);
        assert_eq!(merged, "A=2\nA=2\n");
    }
}
