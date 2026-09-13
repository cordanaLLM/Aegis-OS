// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P15 side.
//!
//! Three of this milestone's claims are about files rather than about code:
//! `aegis-hestia` is a workspace member (REQ-WS-01 recorded its absence as a
//! defect, and D09 committed P15 to having a Rust crate at all), the member
//! list is written out rather than globbed, and the crate takes no store,
//! compositor or user-interface dependency, because it reaches none of them.
//! Each is asserted here rather than left as prose.

use std::path::{Path, PathBuf};

/// Returns the workspace root, two directories above this crate's manifest.
fn workspace_root() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    Some(manifest.parent()?.parent()?.to_path_buf())
}

/// Reads a workspace file, returning an empty string when it is missing.
fn read(name: &str) -> String {
    workspace_root()
        .and_then(|root| std::fs::read_to_string(root.join(name)).ok())
        .unwrap_or_default()
}

// --- Positive -------------------------------------------------------------

/// Positive: the workspace names this crate as a member (REQ-WS-01, D09).
#[test]
fn the_workspace_names_this_crate_as_a_member() {
    let manifest = read("Cargo.toml");
    assert!(!manifest.is_empty(), "the workspace manifest must exist");
    assert!(
        manifest.contains("\"crates/aegis-hestia\""),
        "REQ-WS-01 and D09: a workspace-wide build must cover aegis-hestia"
    );
    assert!(
        manifest.contains("\"crates/aegis-vulcan\""),
        "REQ-WS-01: a workspace-wide build must cover aegis-vulcan"
    );
}

/// Positive: the lock file carries an entry for this crate.
#[test]
fn the_lock_file_carries_an_entry_for_this_crate() {
    let lock = read("Cargo.lock");
    assert!(!lock.is_empty(), "Cargo.lock must be committed");
    assert!(lock.contains("name = \"aegis-hestia\""));
    assert!(lock.contains("name = \"aegis-vulcan\""));
}

// --- Negative -------------------------------------------------------------

/// Negative: the member list is written out, never globbed.
///
/// A glob would activate every reserved directory the moment it acquired a
/// manifest, which is exactly the bar `crates/README.md` sets against.
#[test]
fn the_member_list_is_written_out_and_not_globbed() {
    let manifest = read("Cargo.toml");
    for glob in ["crates/*", "crates/**", "\"*\""] {
        assert!(
            !manifest.contains(glob),
            "the member list must be written out, not globbed with {glob}"
        );
    }
}

/// Negative: this crate declares no store, compositor or interface dependency.
///
/// A real P15 would need an embedded store, a Wayland client and a bundled
/// user interface. This crate reaches none of them, so it declares none of
/// them, and D09 puts the interface half in a separate package entirely.
#[test]
fn the_crate_manifest_declares_no_mapping_dependency() {
    let manifest = read("crates/aegis-hestia/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    for deferred in [
        "postgres", "sqlx", "wayland", "smithay", "wry", "tauri", "tokio", "libc", "rusqlite",
        "wasmtime",
    ] {
        assert!(
            !manifest.contains(deferred),
            "{deferred} is deferred past milestone M17"
        );
    }
    for expected in ["serde", "serde_json", "thiserror"] {
        assert!(
            manifest.contains(expected),
            "the manifest must declare {expected}"
        );
    }
}

/// Negative: milestone M17 ships a library, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    let manifest = read("crates/aegis-hestia/Cargo.toml");
    assert!(!manifest.contains("[[bin]]"));
    assert!(manifest.contains("[lib]"));
    assert!(manifest.contains("publish = false"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the workspace lists exactly the five activated crates.
///
/// Not four and not six: a sixth member would be an activation this milestone
/// did not review, and a fourth would mean one of M17's two crates fell out.
#[test]
fn the_workspace_lists_exactly_five_members() {
    let manifest = read("Cargo.toml");
    let members: Vec<&str> = manifest
        .lines()
        .filter(|line| line.trim_start().starts_with("\"crates/"))
        .collect();
    assert_eq!(members.len(), 5, "members found: {members:?}");
    for name in [
        "aegis-fabrica-defs",
        "aegis-hestia",
        "aegis-janus-lifecycle",
        "aegis-justitia",
        "aegis-hestia",
    ] {
        assert!(
            members.iter().any(|line| line.contains(name)),
            "the member list must name {name}"
        );
    }
}

/// Boundary: the crate inherits the workspace lints rather than restating them.
///
/// Restating them would let one crate drift from the invariants the workspace
/// root declares, `unsafe_code = "forbid"` among them.
#[test]
fn the_crate_inherits_the_workspace_lints() {
    let manifest = read("crates/aegis-hestia/Cargo.toml");
    assert!(manifest.contains("[lints]"));
    assert!(manifest.contains("workspace = true"));
    assert!(!manifest.contains("[lints.rust]"));
    assert!(!manifest.contains("[lints.clippy]"));
    let root = read("Cargo.toml");
    assert!(root.contains("unsafe_code = \"forbid\""));
}
