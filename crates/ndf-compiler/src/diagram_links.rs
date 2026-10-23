//! Links in what a diagram shows of a clause.
//!
//! A clause's text names other clauses (`[[PIPE-ISSUE-001]]`) and files
//! (`` `gen/membus/memif.py` ``, `` `picorv32.v:1202-1210` ``,
//! `` `membus/memif.py::MemIf.hold_nop` ``), and the property panel that shows
//! it follows Markdown links. So the body goes out with those turned into
//! links, and a reader goes from the clause to what it names without opening
//! the document to find it:
//!
//! - a clause, to its node in the diagram (`#<node id>`), with its file and
//!   line as the link's title, which the panel opens instead when the diagram
//!   does not draw that clause;
//! - a file the project holds, to its absolute path, at the line or symbol
//!   named;
//! - a file the project does not hold but that looks like one (a known
//!   extension), left relative, for the editor to look for in its workspace.
//!
//! Fenced code blocks are left alone: a table in one is the clause's own
//! normative text, not a place for links.

use std::cell::OnceCell;
use std::collections::HashMap;
use std::ffi::{OsStr, OsString};
use std::path::{Component, Path, PathBuf};

use walkdir::WalkDir;

/// Extensions a code span is taken to be a file by even when the project does
/// not hold it: the sources and documents a specification names.
const FILE_EXTENSIONS: &[&str] = &[
    "v", "sv", "svh", "vh", "vhd", "vhdl", "py", "pyi", "rs", "c", "h", "cc", "cpp", "cxx", "hpp",
    "hh", "java", "scala", "kt", "ts", "tsx", "js", "jsx", "go", "ml", "mlir", "ll", "td", "cal",
    "json", "yaml", "yml", "toml", "md", "rst", "txt", "tcl", "xdc", "sdc", "f", "mk", "cmake",
    "s", "asm", "sh", "asl", "pdf", "html",
];

/// Directories never searched for a file a clause names.
const SKIPPED_DIRECTORIES: &[&str] = &[
    "node_modules",
    "target",
    "build",
    "dist",
    "out",
    "__pycache__",
];

/// The most files indexed; a project past it is searched in part.
const MAX_INDEXED_FILES: usize = 50_000;

/// A file a clause's text names, as written: the path, and the place in it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileRef<'t> {
    pub path: &'t str,
    pub line: Option<u32>,
    pub end_line: Option<u32>,
    pub symbol: Option<&'t str>,
}

/// A code span's text read as a file reference: `path.ext`, then optionally
/// `:12`, `:12-20`, `::Name.member` or `#name`. None for anything else.
pub fn parse_file_ref(text: &str) -> Option<FileRef<'_>> {
    let text = text.trim();
    if text.is_empty() || text.contains(char::is_whitespace) || text.contains("://") {
        return None;
    }
    // The path runs up to the first character a path does not hold (the `:`,
    // `::` or `#` of a place); its extension follows the last dot of its last
    // segment, so `HL.LWUP.asl` and `x.test.ts` are files.
    let extension_end = text
        .find(|c: char| !(c.is_ascii_alphanumeric() || "_./-+".contains(c)))
        .unwrap_or(text.len());
    let path = &text[..extension_end];
    let segment_start = path.rfind('/').map_or(0, |slash| slash + 1);
    let dot = segment_start + path[segment_start..].rfind('.')?;
    if path[segment_start..dot].trim_start_matches('.').is_empty() {
        // `.py`, `.gitignore`: no name before the extension.
        return None;
    }
    let extension = &path[dot + 1..];
    if extension.is_empty()
        || extension.len() > 10
        || extension.chars().all(|c| c.is_ascii_digit())
        || !extension
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '_')
    {
        return None;
    }
    let rest = &text[extension_end..];
    let mut reference = FileRef {
        path,
        line: None,
        end_line: None,
        symbol: None,
    };
    if rest.is_empty() {
        return Some(reference);
    }
    if let Some(symbol) = rest.strip_prefix("::").or_else(|| rest.strip_prefix('#')) {
        let valid = !symbol.is_empty()
            && symbol
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || "_.".contains(c));
        reference.symbol = valid.then_some(symbol);
        return valid.then_some(reference);
    }
    let lines = rest.strip_prefix(':')?;
    let (start, end) = match lines.split_once('-') {
        Some((start, end)) => (start, Some(end)),
        None => (lines, None),
    };
    reference.line = Some(start.parse().ok()?);
    reference.end_line = match end {
        Some(end) => Some(end.parse().ok()?),
        None => None,
    };
    Some(reference)
}

/// Whether a path looks like a file even unresolved: a known extension.
pub fn has_file_extension(path: &str) -> bool {
    path.rsplit_once('.').is_some_and(|(_, extension)| {
        FILE_EXTENSIONS.contains(&extension.to_ascii_lowercase().as_str())
    })
}

/// The files of a project, indexed once (by file name, the last part of
/// every reference), for the references its clauses make.
pub struct FileIndex {
    root: PathBuf,
    files: OnceCell<HashMap<OsString, Vec<PathBuf>>>,
}

impl FileIndex {
    pub fn new(root: &Path) -> Self {
        Self {
            root: root.to_path_buf(),
            files: OnceCell::new(),
        }
    }

    fn files(&self) -> &HashMap<OsString, Vec<PathBuf>> {
        self.files.get_or_init(|| {
            let mut by_name: HashMap<OsString, Vec<PathBuf>> = HashMap::new();
            for path in WalkDir::new(&self.root)
                .into_iter()
                .filter_entry(|entry| {
                    let name = entry.file_name().to_string_lossy();
                    entry.depth() == 0
                        || !(name.starts_with('.')
                            || (entry.file_type().is_dir()
                                && SKIPPED_DIRECTORIES.contains(&name.as_ref())))
                })
                .filter_map(Result::ok)
                .filter(|entry| entry.file_type().is_file())
                .take(MAX_INDEXED_FILES)
                .map(|entry| entry.into_path())
            {
                if let Some(name) = path.file_name() {
                    by_name.entry(name.to_owned()).or_default().push(path);
                }
            }
            by_name
        })
    }

    /// The file `reference` names, read from a document in `base_dir`: as
    /// written there or in a folder above it up to the project root, else the
    /// one file of the project whose path ends with it (the closest to
    /// `base_dir` when several do).
    pub fn resolve(&self, base_dir: &Path, reference: &str) -> Option<PathBuf> {
        let reference_path = Path::new(reference);
        if reference_path.is_absolute() {
            return reference_path
                .is_file()
                .then(|| reference_path.to_path_buf());
        }
        let mut dir = Some(base_dir);
        while let Some(current) = dir {
            let candidate = current.join(reference_path);
            if candidate.is_file() {
                return Some(normalize(&candidate));
            }
            if current == self.root || !current.starts_with(&self.root) {
                break;
            }
            dir = current.parent();
        }
        let trailing: Vec<&str> = reference
            .split('/')
            .filter(|part| !part.is_empty() && *part != "." && *part != "..")
            .collect();
        if trailing.is_empty() {
            return None;
        }
        let name = OsStr::new(trailing.last().expect("a non-empty trailing path"));
        self.files()
            .get(name)
            .into_iter()
            .flatten()
            .filter(|file| {
                let parts: Vec<_> = file.components().rev().take(trailing.len()).collect();
                parts.len() == trailing.len()
                    && parts
                        .iter()
                        .rev()
                        .zip(&trailing)
                        .all(|(part, wanted)| part.as_os_str() == *wanted)
            })
            .max_by_key(|file| {
                (
                    shared_components(file, base_dir),
                    std::cmp::Reverse(file.as_os_str().len()),
                )
            })
            .cloned()
    }
}

fn shared_components(a: &Path, b: &Path) -> usize {
    a.components()
        .zip(b.components())
        .take_while(|(x, y)| x == y)
        .count()
}

fn normalize(path: &Path) -> PathBuf {
    path.canonicalize().unwrap_or_else(|_| {
        let mut out = PathBuf::new();
        for component in path.components() {
            match component {
                Component::ParentDir => {
                    out.pop();
                }
                Component::CurDir => {}
                other => out.push(other),
            }
        }
        out
    })
}

/// The 1-based line `symbol` is defined on in `file`, else first mentioned on:
/// the last segment of a dotted name (`MemIf.hold_nop` → `hold_nop`) after a
/// definition keyword, then as a whole word.
pub fn symbol_line(file: &Path, symbol: &str) -> Option<u32> {
    const KEYWORDS: &[&str] = &[
        "def",
        "class",
        "fn",
        "function",
        "func",
        "struct",
        "enum",
        "trait",
        "interface",
        "type",
        "module",
        "entity",
        "architecture",
        "package",
        "component",
        "const",
        "let",
        "var",
        "signal",
        "reg",
        "wire",
        "logic",
    ];
    let name = symbol.rsplit(['.', ':']).find(|part| !part.is_empty())?;
    let metadata = std::fs::metadata(file).ok()?;
    if metadata.len() > 8 * 1024 * 1024 {
        return None;
    }
    let text = std::fs::read_to_string(file).ok()?;
    let is_word = |c: char| c.is_ascii_alphanumeric() || c == '_';
    let mentions = |line: &str| {
        line.match_indices(name).any(|(at, _)| {
            let before = line[..at].chars().next_back();
            let after = line[at + name.len()..].chars().next();
            !before.is_some_and(is_word) && !after.is_some_and(is_word)
        })
    };
    let defines = |line: &str| {
        let words: Vec<&str> = line
            .split(|c: char| !is_word(c))
            .filter(|w| !w.is_empty())
            .collect();
        words
            .windows(2)
            .any(|pair| KEYWORDS.contains(&pair[0]) && pair[1] == name)
    };
    let lines: Vec<&str> = text.lines().collect();
    lines
        .iter()
        .position(|line| defines(line))
        .or_else(|| lines.iter().position(|line| mentions(line)))
        .map(|index| index as u32 + 1)
}

/// An absolute path as a Markdown link target: percent-encoded like a `file:`
/// URI's path, but without the scheme, which Markdown renderers refuse.
pub fn link_path(absolute: &Path) -> String {
    let text = absolute.to_string_lossy();
    let mut out = String::with_capacity(text.len());
    if !text.starts_with('/') {
        out.push('/');
    }
    for byte in text.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' | b'/' => {
                out.push(byte as char)
            }
            b'\\' => out.push('/'),
            other => out.push_str(&format!("%{other:02X}")),
        }
    }
    out
}

/// A link's target and, for a clause, the file it falls back to.
pub struct LinkTarget {
    pub href: String,
    pub title: Option<String>,
}

/// Markdown link text, its brackets and backslashes escaped.
fn escape_text(text: &str) -> String {
    text.replace('\\', "\\\\")
        .replace('[', "\\[")
        .replace(']', "\\]")
}

fn link(text: &str, target: &LinkTarget) -> String {
    match &target.title {
        Some(title) => format!(
            "[{}]({} \"{}\")",
            escape_text(text),
            target.href,
            title.replace('"', "%22")
        ),
        None => format!("[{}]({})", escape_text(text), target.href),
    }
}

/// `body` with its clause mentions and file references made links: `clause`
/// resolves a `[[...]]` mention's text, `file` a code span's.
pub fn link_markdown(
    body: &str,
    clause: impl Fn(&str) -> Option<LinkTarget>,
    file: impl Fn(&str) -> Option<LinkTarget>,
) -> String {
    let mut out = String::with_capacity(body.len() + 64);
    let mut fence: Option<String> = None;
    for (index, line) in body.split('\n').enumerate() {
        if index > 0 {
            out.push('\n');
        }
        let trimmed = line.trim_start();
        let marker: String = trimmed
            .chars()
            .take_while(|c| *c == '`' || *c == '~')
            .collect();
        if marker.len() >= 3 && marker.chars().all(|c| c == marker.chars().next().unwrap()) {
            match &fence {
                Some(open) if marker.starts_with(open.as_str()) => fence = None,
                None => fence = Some(marker),
                _ => {}
            }
            out.push_str(line);
            continue;
        }
        if fence.is_some() {
            out.push_str(line);
            continue;
        }
        out.push_str(&link_line(line, &clause, &file));
    }
    out
}

fn link_line(
    line: &str,
    clause: &impl Fn(&str) -> Option<LinkTarget>,
    file: &impl Fn(&str) -> Option<LinkTarget>,
) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while !rest.is_empty() {
        if rest.starts_with('`') {
            let ticks = rest.chars().take_while(|c| *c == '`').count();
            let fence = &rest[..ticks];
            if let Some(close) = rest[ticks..].find(fence) {
                let span = &rest[..ticks + close + ticks];
                let content = &rest[ticks..ticks + close];
                // A code span that is already a link's text stays as it is.
                let in_link = out.ends_with('[') && rest[span.len()..].starts_with("](");
                match (in_link, file(content)) {
                    (false, Some(target)) => out.push_str(&link_code(span, &target)),
                    _ => out.push_str(span),
                }
                rest = &rest[span.len()..];
                continue;
            }
            out.push_str(fence);
            rest = &rest[ticks..];
            continue;
        }
        if let Some(after) = rest.strip_prefix("[[")
            && let Some(close) = after.find("]]")
        {
            let mention = &after[..close];
            match clause(mention) {
                Some(target) => out.push_str(&link(mention, &target)),
                None => out.push_str(&rest[..close + 4]),
            }
            rest = &after[close + 2..];
            continue;
        }
        let first = rest.chars().next().map_or(1, char::len_utf8);
        let next = rest[first..]
            .find(['`', '['])
            .map_or(rest.len(), |at| at + first);
        out.push_str(&rest[..next]);
        rest = &rest[next..];
    }
    out
}

/// A code span as a link's text, the span kept so it still reads as code.
fn link_code(span: &str, target: &LinkTarget) -> String {
    match &target.title {
        Some(title) => format!(
            "[{span}]({} \"{}\")",
            target.href,
            title.replace('"', "%22")
        ),
        None => format!("[{span}]({})", target.href),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_forms_a_specification_writes() {
        assert_eq!(
            parse_file_ref("picorv32.v:1202-1210"),
            Some(FileRef {
                path: "picorv32.v",
                line: Some(1202),
                end_line: Some(1210),
                symbol: None
            })
        );
        assert_eq!(
            parse_file_ref("membus/memif.py::MemIf.hold_nop"),
            Some(FileRef {
                path: "membus/memif.py",
                line: None,
                end_line: None,
                symbol: Some("MemIf.hold_nop")
            })
        );
        assert_eq!(
            parse_file_ref("picorv32.v#latched_store").and_then(|r| r.symbol),
            Some("latched_store")
        );
        assert_eq!(parse_file_ref("gen/a.py").map(|r| r.path), Some("gen/a.py"));
        // A name with dots of its own: the extension is the last one.
        assert_eq!(
            parse_file_ref("asl/scalar/agu/HL.LWUP.asl").map(|r| r.path),
            Some("asl/scalar/agu/HL.LWUP.asl")
        );
        assert_eq!(
            parse_file_ref("src/x.test.ts:4").map(|r| (r.path, r.line)),
            Some(("src/x.test.ts", Some(4)))
        );
    }

    #[test]
    fn leaves_what_is_not_a_file() {
        for text in [
            "cpu_state_fetch",
            "1.0",
            "a b.py",
            "https://x.org/a.py",
            ".py",
            ".gitignore",
            "x.py:abc",
            "",
        ] {
            assert_eq!(parse_file_ref(text), None, "{text}");
        }
        assert!(has_file_extension("picorv32.v"));
        assert!(!has_file_extension("MemIf.hold_nop"));
    }

    #[test]
    fn links_mentions_and_files_but_not_fenced_code() {
        let clause = |text: &str| {
            (text == "DEF-1").then(|| LinkTarget {
                href: "#node:scope:root:DEF-1".to_owned(),
                title: Some("/w/d.md#L3".to_owned()),
            })
        };
        let file = |text: &str| {
            (text == "a.py").then(|| LinkTarget {
                href: "/w/a.py".to_owned(),
                title: None,
            })
        };
        let body = "See [[DEF-1]] and `a.py` → not [[NOPE-2]] or `b`.\n```text\n[[DEF-1]] `a.py`\n```\n[`a.py`](x)";
        assert_eq!(
            link_markdown(body, clause, file),
            "See [DEF-1](#node:scope:root:DEF-1 \"/w/d.md#L3\") and [`a.py`](/w/a.py) → not [[NOPE-2]] or `b`.\n```text\n[[DEF-1]] `a.py`\n```\n[`a.py`](x)"
        );
    }

    #[test]
    fn encodes_a_path_as_a_link_target() {
        assert_eq!(link_path(Path::new("/w/a b/(x).md")), "/w/a%20b/%28x%29.md");
    }
}
