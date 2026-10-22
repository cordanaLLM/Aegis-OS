// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: shared manifest hygiene helpers for integration tests.
//!
//! One file, included by path into the manifest hygiene tests of workspace crates
//! (`#[path = "../../manifest_hygiene.rs"] mod manifest_hygiene;`), extending D78 and D116
//! precedents to manifest sweeps. It is not a crate and has no manifest: a test helper
//! that were a workspace member would enter every dependency closure.
//!
//! The helpers traverse crate sources up to scalar bounds, inspect manifest files,
//! filter comment lines, check for forbidden tokens, and verify that expected source
//! files are present.

#![allow(dead_code)]

use std::path::{Path, PathBuf};

/// Scalar bound on directory traversals.
pub const DIR_BOUND: usize = 64;

/// Scalar bound on directory entries per directory.
pub const ENTRY_BOUND: usize = 64;

/// Scalar bound on lines scanned in any one source file.
pub const LINE_BOUND: usize = 4096;

/// Scalar bound on tokens scanned.
pub const TOKEN_BOUND: usize = 64;

/// Returns the crate directory from `CARGO_MANIFEST_DIR`.
#[must_use]
pub fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Reads a file relative to the workspace root.
#[must_use]
pub fn read(name: &str) -> String {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    let Some(root) = manifest.parent().and_then(Path::parent) else {
        return String::new();
    };
    std::fs::read_to_string(root.join(name)).unwrap_or_default()
}

/// Returns every `.rs` file under `src/`, bounded, with its text.
#[must_use]
pub fn sources() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut pending = vec![crate_dir().join("src")];
    for _ in 0..DIR_BOUND {
        let Some(directory) = pending.pop() else {
            break;
        };
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten().take(ENTRY_BOUND) {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                found.push((path, text));
            }
        }
    }
    found
}

/// Returns every `.rs` file under `src/`, relative to `src/`, with its text.
#[must_use]
pub fn relative_sources() -> Vec<(String, String)> {
    let src = crate_dir().join("src");
    let all = sources();
    let mut out = Vec::new();
    for (path, body) in all.into_iter().take(DIR_BOUND.saturating_mul(ENTRY_BOUND)) {
        let relative = path
            .strip_prefix(&src)
            .unwrap_or(&path)
            .display()
            .to_string();
        out.push((relative, body));
    }
    out
}

/// Returns `true` if `line` trimmed opens a line or block comment, or continues one.
///
/// A continuation is a lone `*`, `* text` or `*/`; a dereference such as
/// `*slot = value` stays code.
#[must_use]
pub fn is_comment_line(line: &str) -> bool {
    let trimmed = line.trim_start();
    trimmed.starts_with("//")
        || trimmed.starts_with("/*")
        || trimmed.starts_with("*/")
        || trimmed.starts_with("* ")
        || trimmed == "*"
}

/// Returns `body` with comment lines removed.
#[must_use]
pub fn code_only(body: &str) -> String {
    let mut out = String::new();
    for line in body.lines().take(LINE_BOUND) {
        if !is_comment_line(line) {
            out.push_str(line);
            out.push('\n');
        }
    }
    out
}

/// Returns the watched identifiers `text` names outside comments.
#[must_use]
pub fn token_hits<'a>(text: &str, watched: &[&'a str]) -> Vec<&'a str> {
    let code: Vec<&str> = text
        .lines()
        .take(LINE_BOUND)
        .map(str::trim)
        .filter(|line| !is_comment_line(line))
        .collect();
    let mut hits = Vec::new();
    for needle in watched.iter().take(TOKEN_BOUND) {
        if code.iter().any(|line| line.contains(*needle)) {
            hits.push(*needle);
        }
    }
    hits
}

/// Asserts that every expected file name is present in `sources`.
#[track_caller]
pub fn assert_sweep_reads_sources(sources: &[(PathBuf, String)], expected: &[&str]) {
    let names: Vec<String> = sources
        .iter()
        .take(LINE_BOUND)
        .filter_map(|(path, _)| path.file_name()?.to_str().map(str::to_owned))
        .collect();
    for exp in expected.iter().take(TOKEN_BOUND) {
        assert!(
            names.iter().any(|name| name == *exp),
            "missing {exp} in {names:?}"
        );
    }
}
