// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The crate's membership, its dependencies and its own closure (D78), and
//! the effects its sources do not have.
//!
//! The crate reads nothing and starts nothing: the gate reads the counter and
//! hands over text. Its closure therefore resolves no socket, process,
//! eBPF or bus crate, and its sources name no file, process or sysfs call.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

use std::path::{Path, PathBuf};

/// This crate's package name, the start of its closure.
const PACKAGE: &str = "aegis-tellus-rapl";

/// Crates this closure must not resolve: a reader would need one of them.
const FORBIDDEN: [&str; 8] = [
    "aya", "zbus", "tokio", "nix", "procfs", "socket2", "rustix", "libc",
];

/// Identifiers that would perform an effect this crate does not have.
const WATCHED: [&str; 9] = [
    "std::fs",
    "OpenOptions",
    "File::",
    "read_to_string",
    "read_dir",
    "Command",
    "UnixStream",
    "TcpStream",
    "sudo",
];

/// Returns the crate directory.
fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Reads a file relative to the workspace root.
fn read(name: &str) -> String {
    dependency_closure::workspace_root()
        .and_then(|root| std::fs::read_to_string(root.join(name)).ok())
        .unwrap_or_default()
}

/// Returns every `.rs` file under `src/`, bounded, with its text.
fn sources() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut pending = vec![crate_dir().join("src")];
    for _ in 0..16 {
        let Some(directory) = pending.pop() else {
            break;
        };
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten().take(64) {
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

/// Returns the watched identifiers `text` names outside comments.
fn hits(text: &str) -> Vec<&'static str> {
    let code: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//"))
        .collect();
    WATCHED
        .into_iter()
        .filter(|needle| code.iter().any(|line| line.contains(needle)))
        .collect()
}

// --- Positive -------------------------------------------------------------

/// Positive: the crate is a written-out member with a lock entry.
#[test]
fn the_crate_is_a_locked_member() {
    assert!(read("Cargo.toml").contains("\"crates/aegis-tellus-rapl\""));
    assert!(read("Cargo.lock").contains("name = \"aegis-tellus-rapl\""));
    let manifest = read("crates/aegis-tellus-rapl/Cargo.toml");
    assert!(manifest.contains("aegis-tellus = { path = \"../aegis-tellus\" }"));
    assert!(manifest.contains("[lints]\nworkspace = true"));
    assert!(manifest.contains("publish = false"));
}

/// Positive: the sweep reads the library and the binary.
#[test]
fn the_sweep_reads_every_source() {
    let names: Vec<String> = sources()
        .iter()
        .filter_map(|(path, _)| path.file_name()?.to_str().map(str::to_owned))
        .collect();
    for expected in ["lib.rs", "cases.rs", "input.rs", "aegis-tellus-rapl.rs"] {
        assert!(names.iter().any(|name| name == expected), "{names:?}");
    }
}

// --- Negative -------------------------------------------------------------

/// Negative (D78): the closure resolves no reader, socket or bus crate.
#[test]
fn the_closure_resolves_no_reader() -> Result<(), String> {
    let names = dependency_closure::own_closure(PACKAGE)?;
    let found = dependency_closure::hits(&names, &FORBIDDEN);
    assert!(
        found.is_empty(),
        "the closure of {PACKAGE} resolves {found:?}"
    );
    assert!(names.contains("aegis-tellus"));
    Ok(())
}

/// Negative: no source names a file, process, socket or sudo call.
#[test]
fn no_source_performs_a_read() {
    for (path, text) in sources() {
        let found = hits(&text);
        assert!(found.is_empty(), "{} names {found:?}", path.display());
    }
}

/// Negative: the sweep would notice a planted read.
#[test]
fn the_sweep_detects_a_planted_read() {
    let planted = "let text = std::fs::read_to_string(path)?;";
    assert_eq!(hits(planted), vec!["std::fs", "read_to_string"]);
    assert!(hits("// std::fs is named in a comment").is_empty());
}

// --- Boundary -------------------------------------------------------------

/// Boundary: exactly one binary target, the one the gate starts.
#[test]
fn there_is_exactly_one_binary() {
    let manifest = read("crates/aegis-tellus-rapl/Cargo.toml");
    assert_eq!(manifest.matches("[[bin]]").count(), 1);
    assert!(manifest.contains("name = \"aegis-tellus-rapl\""));
    assert_eq!(FORBIDDEN.len(), 8);
    assert_eq!(WATCHED.len(), 9);
}
