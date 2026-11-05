//! TTM RM Proscription 6 — no internal-level constructs in the logical
//! interface (the project's "no tuple-level IDs exposed" rule).
//!
//! The logical API is what the `relvar` facade exposes: relvar-core's
//! public items and the `PersistentEngine` it re-exports. This gate scans
//! every public signature, public field and re-export in those sources and
//! fails if one mentions a physical identifier: tuple ids, page ids,
//! slots, log sequence numbers, transaction ids or version stamps. Users can
//! observe values, never where or when they are stored.

use std::fs;
use std::path::{Path, PathBuf};

/// Identifiers of internal-level constructs. Matched as whole words.
const FORBIDDEN: &[&str] = &[
    "TupleId",
    "RowId",
    "rowid",
    "PageId",
    "page_id",
    "SlotId",
    "slot_id",
    "Lsn",
    "TransactionId",
    "txn_id",
    "VersionMetadata",
    "xmin",
    "xmax",
    "tuple_id",
];

fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("workspace root")
        .to_path_buf()
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            // Test modules are not part of the public API.
            if path.file_name().is_some_and(|n| n == "tests") {
                continue;
            }
            rust_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "rs") {
            out.push(path);
        }
    }
}

fn is_word_char(c: char) -> bool {
    c.is_alphanumeric() || c == '_'
}

fn mentions(text: &str, word: &str) -> bool {
    text.match_indices(word).any(|(i, _)| {
        let before = text[..i].chars().next_back();
        let after = text[i + word.len()..].chars().next();
        !before.is_some_and(is_word_char) && !after.is_some_and(is_word_char)
    })
}

/// Public declarations in `source`, as the text that is part of the API:
///
/// - `pub fn ...` up to the body (`{`) or `;`, across lines;
/// - every other `pub ` item or field (not `pub(crate)` etc.) up to the next
///   `{`, `}`, `;` or `,`, wherever it appears on a line;
/// - the whole body of a `pub enum` (variant payloads are public).
///
/// Comments and `#[cfg(test)]` modules are skipped.
fn public_declarations(source: &str) -> Vec<(usize, String)> {
    let lines: Vec<&str> = source.lines().collect();
    let mut found = Vec::new();
    let mut index = 0;
    while index < lines.len() {
        let line = lines[index];
        let trimmed = line.trim_start();
        if trimmed.starts_with("#[cfg(test)]") {
            // Inline test modules are not part of the public API: skip the
            // rest of the file (they conventionally come last).
            break;
        }
        if trimmed.starts_with("//") {
            index += 1;
            continue;
        }
        let code = trimmed.split("//").next().unwrap_or("");
        for (offset, _) in code.match_indices("pub ") {
            if offset > 0 && code[..offset].chars().next_back().is_some_and(is_word_char) {
                continue;
            }
            let rest = &code[offset..];
            if rest.starts_with("pub fn")
                || rest.starts_with("pub const fn")
                || rest.starts_with("pub unsafe fn")
            {
                let mut signature = String::from(rest);
                let mut next = index + 1;
                while !signature.contains('{') && !signature.contains(';') && next < lines.len() {
                    let more = lines[next].trim();
                    if !more.starts_with("//") {
                        signature.push(' ');
                        signature.push_str(more);
                    }
                    next += 1;
                }
                let end = signature.find(['{', ';']).unwrap_or(signature.len());
                found.push((index + 1, signature[..end].to_string()));
            } else if rest.starts_with("pub enum") {
                let mut body = String::new();
                let mut depth = 0i32;
                let mut opened = false;
                let mut next = index;
                while next < lines.len() {
                    let text = if next == index {
                        rest
                    } else {
                        lines[next].trim()
                    };
                    if !text.starts_with("//") {
                        body.push(' ');
                        body.push_str(text.split("//").next().unwrap_or(""));
                        depth += i32::try_from(text.matches('{').count()).unwrap_or(0);
                        depth -= i32::try_from(text.matches('}').count()).unwrap_or(0);
                        opened |= text.contains('{');
                    }
                    if opened && depth <= 0 || (!opened && text.contains(';')) {
                        break;
                    }
                    next += 1;
                }
                found.push((index + 1, body));
            } else {
                let end = rest.find(['{', '}', ';', ',']).unwrap_or(rest.len());
                found.push((index + 1, rest[..end].to_string()));
            }
        }
        index += 1;
    }
    found
}

fn violations_in(files: &[PathBuf]) -> Vec<String> {
    let mut violations = Vec::new();
    for file in files {
        let source = fs::read_to_string(file).expect("readable source");
        for (line, declaration) in public_declarations(&source) {
            for word in FORBIDDEN {
                if mentions(&declaration, word) {
                    violations.push(format!(
                        "{}:{line}: public API mentions `{word}`: {declaration}",
                        file.display()
                    ));
                }
            }
        }
    }
    violations
}

#[test]
fn logical_api_exposes_no_internal_identifiers() {
    let root = workspace_root();
    let mut files = Vec::new();
    rust_files(&root.join("relvar-core/src"), &mut files);
    rust_files(&root.join("relvar/src"), &mut files);
    // The facade re-exports PersistentEngine: its public surface counts.
    files.push(root.join("relvar-storage/src/persistent_engine/mod.rs"));
    assert!(
        files.len() > 20,
        "expected to scan the sources, found {files:?}"
    );

    let violations = violations_in(&files);
    assert!(
        violations.is_empty(),
        "RM Proscription 6 violated:\n{}",
        violations.join("\n")
    );
}

#[test]
fn gate_detects_a_leak() {
    // The scanner itself must catch the constructs it guards against.
    let leaky = "pub fn locate(&self) -> TupleId { todo!() }\n\
                 pub struct Version { pub xmin: u64 }\n\
                 pub(crate) fn fine(id: TupleId) {}\n\
                 /// pub fn docs(id: TupleId)\n\
                 pub fn ok(&self, tuple: &Tuple) -> bool;\n\
                 pub enum Event {\n    Moved(PageId),\n    Other,\n}\n";
    let declarations = public_declarations(leaky);
    let leaks: Vec<_> = declarations
        .iter()
        .filter(|(_, d)| FORBIDDEN.iter().any(|w| mentions(d, w)))
        .collect();
    assert_eq!(leaks.len(), 3, "{declarations:?}");
    assert!(!mentions("pub fn lsn_like(x: Lsnx)", "Lsn"));
}

/// The persistent engine's transaction handle is opaque: printing it does
/// not reveal its internal transaction id.
#[cfg(feature = "storage")]
#[test]
fn transaction_handle_is_opaque() {
    use relvar::{PersistentEngine, StorageEngine};
    let dir = tempfile::tempdir().expect("temp dir");
    let mut engine = PersistentEngine::open(dir.path()).expect("open");
    let handle = engine.begin_transaction().expect("begin");
    let rendered = format!("{handle:?}");
    assert!(
        !rendered.chars().any(|c| c.is_ascii_digit()),
        "transaction handle leaks an id: {rendered}"
    );
    engine.rollback_transaction(handle).expect("rollback");
}
