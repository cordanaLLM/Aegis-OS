// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Manifest hygiene, workspace membership, and dependency closure verification
//! for `aegis-vulcan-gpu` (M25 slice 1).

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

use std::path::{Path, PathBuf};

/// This crate's package name, the start of its closure.
const PACKAGE: &str = "aegis-vulcan-gpu";

/// Crates this closure must not resolve.
const FORBIDDEN: [&str; 8] = [
    "aya", "zbus", "tokio", "nix", "procfs", "socket2", "rustix", "libc",
];

/// Identifiers that would perform an effect this slice does not have.
/// Slice 1 reads sysfs and stats /dev, but executes no child processes,
/// opens no sockets, uses no sudo, and contains no unsafe code.
const WATCHED: [&str; 5] = ["Command", "UnixStream", "TcpStream", "sudo", "unsafe"];

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
        .filter(|line| !line.starts_with("//") && !line.starts_with("/*") && !line.starts_with('*'))
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
    assert!(read("Cargo.toml").contains("\"crates/aegis-vulcan-gpu\""));
    assert!(read("Cargo.lock").contains("name = \"aegis-vulcan-gpu\""));
    let manifest = read("crates/aegis-vulcan-gpu/Cargo.toml");
    assert!(manifest.contains("publish = false"));
    assert!(manifest.contains("[[bin]]"));
    assert!(manifest.contains("name = \"aegis-vulcan-gpu\""));
}

/// Positive: the sweep reads the library and all module sources.
#[test]
fn the_sweep_reads_every_source() {
    let names: Vec<String> = sources()
        .iter()
        .filter_map(|(path, _)| path.file_name()?.to_str().map(str::to_owned))
        .collect();
    for expected in [
        "lib.rs",
        "error.rs",
        "render_nodes.rs",
        "drm_sched.rs",
        "rebar.rs",
        "heap.rs",
        "reader.rs",
        "vfio.rs",
        "gpus.rs",
        "record.rs",
        "aegis-vulcan-gpu.rs",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "missing {expected} in {names:?}"
        );
    }
}

/// Boundary: the crate inherits the workspace lints rather than restating them.
///
/// Restating them would let one crate drift from the invariants the workspace
/// root declares, `unsafe_code = "forbid"` among them.
#[test]
fn the_crate_inherits_the_workspace_lints() {
    let manifest = read("crates/aegis-vulcan-gpu/Cargo.toml");
    assert!(manifest.contains("[lints]"));
    assert!(manifest.contains("workspace = true"));
    assert!(!manifest.contains("[lints.rust]"));
    assert!(!manifest.contains("[lints.clippy]"));
    let root = read("Cargo.toml");
    assert!(root.contains("unsafe_code = \"forbid\""));
}

// --- Negative -------------------------------------------------------------

/// Negative (D78): the closure resolves no reader, socket or bus crate.
#[test]
fn the_closure_resolves_no_forbidden_crate() -> Result<(), String> {
    let names = dependency_closure::own_closure(PACKAGE)?;
    let found = dependency_closure::hits(&names, &FORBIDDEN);
    assert!(
        found.is_empty(),
        "the closure of {PACKAGE} resolves forbidden crates {found:?}"
    );
    assert!(names.contains("serde"));
    assert!(names.contains("serde_json"));
    assert!(names.contains("thiserror"));
    Ok(())
}

/// Negative: no source in slice 1 names Command, sockets, sudo, or unsafe code.
#[test]
fn no_source_performs_banned_action_or_unsafe() {
    for (path, text) in sources() {
        let found = hits(&text);
        assert!(found.is_empty(), "{} names {found:?}", path.display());
    }
}

/// Negative: the sweep would notice a planted watched token.
#[test]
fn the_sweep_detects_a_planted_token() {
    let planted = "let stream = std::net::TcpStream::connect(addr);";
    assert_eq!(hits(planted), vec!["TcpStream"]);
    assert_eq!(hits("// TcpStream is named in a comment"), [] as [&str; 0]);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: exactly one binary target, the record CLI.
#[test]
fn there_is_exactly_one_binary() {
    let manifest = read("crates/aegis-vulcan-gpu/Cargo.toml");
    assert_eq!(manifest.matches("[[bin]]").count(), 1);
    assert!(manifest.contains("name = \"aegis-vulcan-gpu\""));
    assert_eq!(FORBIDDEN.len(), 8);
    assert_eq!(WATCHED.len(), 5);
}
