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

/// Returns the workspace root, two directories above this crate's manifest.
#[must_use]
pub fn workspace_root() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    Some(manifest.parent()?.parent()?.to_path_buf())
}

/// Returns every directory under `crates/` that carries a `Cargo.toml` manifest, sorted.
#[must_use]
pub fn crate_directories_with_a_manifest() -> Vec<String> {
    let Some(root) = workspace_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root.join("crates")) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .take(ENTRY_BOUND)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort_unstable();
    found
}

/// Returns the manifest's `[dependencies]` and `[dev-dependencies]` tables.
#[must_use]
pub fn dependency_table(manifest: &str) -> String {
    let mut inside = false;
    let mut out = String::new();
    for line in manifest.lines().take(LINE_BOUND) {
        let text = line.trim();
        if text.starts_with('[') {
            inside = text == "[dependencies]" || text == "[dev-dependencies]";
            continue;
        }
        if inside {
            out.push_str(text);
            out.push('\n');
        }
    }
    out
}

/// Returns the names the `[dependencies]` table declares, sorted and deduped.
#[must_use]
pub fn declared_dependencies(manifest: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut inside = false;
    for line in manifest.lines().take(LINE_BOUND) {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[dependencies]";
        } else if inside
            && !line.is_empty()
            && !line.starts_with('#')
            && let Some((name, _)) = line.split_once(['=', '.'])
        {
            names.push(name.trim().to_owned());
        }
    }
    names.sort_unstable();
    names.dedup();
    names
}

/// Asserts that the workspace manifest names `crate_name` as a member.
#[track_caller]
pub fn assert_workspace_names_crate(crate_name: &str) {
    let manifest = read("Cargo.toml");
    assert!(!manifest.is_empty(), "the workspace manifest must exist");
    assert!(manifest.contains(&format!("\"crates/{crate_name}\"")));
}

/// Asserts that `Cargo.lock` exists and carries an entry for `package`.
#[track_caller]
pub fn assert_lock_carries_entry(package: &str) {
    let lock = read("Cargo.lock");
    assert!(!lock.is_empty(), "Cargo.lock must be committed");
    let entry = format!("name = \"{package}\"");
    assert!(lock.contains(&entry));
}

/// Asserts that the workspace member list is written out rather than globbed.
#[track_caller]
pub fn assert_member_list_is_written_out_and_not_globbed() {
    let manifest = read("Cargo.toml");
    for glob in ["crates/*", "crates/**", "\"*\""] {
        assert!(
            !manifest.contains(glob),
            "the member list must be written out, not globbed with {glob}"
        );
    }
}

/// Asserts that the workspace manifest lists every crate directory with a manifest.
#[track_caller]
pub fn assert_workspace_lists_every_crate_with_a_manifest() {
    let manifest = read("Cargo.toml");
    let members: Vec<String> = manifest
        .lines()
        .take(LINE_BOUND)
        .map(str::trim)
        .filter(|line| line.starts_with("\"crates/"))
        .map(|line| line.trim_matches(|c| c == '"' || c == ',').to_owned())
        .collect();
    let directories = crate_directories_with_a_manifest();
    assert!(!directories.is_empty(), "crates/ must hold manifests");
    assert_eq!(
        members.len(),
        directories.len(),
        "members {members:?} against directories {directories:?}"
    );
    for name in &directories {
        assert!(
            members
                .iter()
                .any(|member| member == &format!("crates/{name}")),
            "the member list must name {name}"
        );
    }
}

/// Asserts that a crate manifest inherits the workspace lints.
#[track_caller]
pub fn assert_inherits_workspace_lints(manifest_path: &str) {
    let manifest = read(manifest_path);
    assert!(manifest.contains("[lints]"));
    assert!(manifest.contains("workspace = true"));
    assert!(!manifest.contains("[lints.rust]"));
    assert!(!manifest.contains("[lints.clippy]"));
    let root = read("Cargo.toml");
    assert!(root.contains("unsafe_code = \"forbid\""));
}

/// Asserts that a crate manifest inherits workspace lints and the pinned toolchain.
#[track_caller]
pub fn assert_inherits_workspace_lints_and_toolchain(manifest_path: &str) {
    assert_inherits_workspace_lints(manifest_path);
    let manifest = read(manifest_path);
    assert!(manifest.contains("rust-version.workspace = true"));
    let root = read("Cargo.toml");
    assert!(
        root.contains("rust-version = \"1.99.1\""),
        "D111: the workspace rust-version is the rust-toolchain.toml pin"
    );
    let pinned = read("rust-toolchain.toml");
    assert!(
        pinned.contains("channel = \"1.99.1\""),
        "M08 admits no new toolchain: the pin stays the one M02 admitted"
    );
}

/// Asserts that a crate manifest declares a library target and no binary target.
#[track_caller]
pub fn assert_crate_declares_no_binary_target(manifest_path: &str) {
    let manifest = read(manifest_path);
    assert!(!manifest.contains("[[bin]]"));
    assert!(manifest.contains("[lib]"));
    assert!(manifest.contains("publish = false"));
}

/// Asserts that a crate manifest contains every expected dependency or declaration snippet.
#[track_caller]
pub fn assert_manifest_contains(manifest: &str, expected: &[&str]) {
    for needle in expected.iter().take(TOKEN_BOUND) {
        assert!(manifest.contains(*needle), "manifest missing {needle}");
    }
}

/// Asserts that a crate manifest declares none of the `excluded` dependencies.
#[track_caller]
pub fn assert_no_excluded_dependencies(manifest_path: &str, excluded: &[&str], reason: &str) {
    let manifest = read(manifest_path);
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    let declarations: Vec<&str> = manifest
        .lines()
        .take(LINE_BOUND)
        .map(str::trim)
        .filter(|line| line.contains(" = ") || line.ends_with(".workspace = true"))
        .collect();
    for name in excluded.iter().take(TOKEN_BOUND) {
        assert!(
            !declarations
                .iter()
                .any(|line| line.starts_with(&format!("{name} "))
                    || line.starts_with(&format!("{name}."))),
            "{name} {reason}"
        );
    }
}

/// Asserts that a crate manifest declares expected dependencies and none of the deferred dependencies.
#[track_caller]
pub fn assert_deferred_and_expected_dependencies(
    manifest_path: &str,
    deferred: &[&str],
    milestone: &str,
    expected: &[&str],
) {
    let manifest = read(manifest_path);
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    for item in deferred.iter().take(TOKEN_BOUND) {
        assert!(
            !manifest.contains(*item),
            "{item} is deferred past milestone {milestone}"
        );
    }
    assert_manifest_contains(&manifest, expected);
}
