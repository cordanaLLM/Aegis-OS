// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P10 side.
//!
//! Three of this milestone's claims are about files rather than about code:
//! `aegis-vesta` is a workspace member with a lock entry, the member list is
//! written out rather than globbed, and **the crate takes no Go runtime, no
//! `WebAssembly` engine, no monitor binding and no `io_uring` dependency**,
//! which is decision D06's dependency half and the exit criterion's "no Wasm
//! engine is used". Each is asserted here rather than left as prose.

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

/// The member list is checked against this rather than against a literal
/// count. A count has to be edited every time a milestone activates a crate,
/// and hard-coding it in several crates at once is what made three of them
/// fail when milestone M05 added two members. What actually needs holding is
/// the invariant behind the count: every crate directory that has a manifest
/// is a workspace member, and nothing else is.
fn crate_directories_with_a_manifest() -> Vec<String> {
    let Some(root) = workspace_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root.join("crates")) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .take(64)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort_unstable();
    found
}

// --- Positive -------------------------------------------------------------

/// Positive: the workspace names both M06 crates as members.
#[test]
fn the_workspace_names_both_milestone_crates() {
    let manifest = read("Cargo.toml");
    assert!(!manifest.is_empty(), "the workspace manifest must exist");
    assert!(manifest.contains("\"crates/aegis-vesta\""));
    assert!(manifest.contains("\"crates/aegis-minerva\""));
}

/// Positive: the lock file carries an entry for both.
#[test]
fn the_lock_file_carries_an_entry_for_both() {
    let lock = read("Cargo.lock");
    assert!(!lock.is_empty(), "Cargo.lock must be committed");
    assert!(lock.contains("name = \"aegis-vesta\""));
    assert!(lock.contains("name = \"aegis-minerva\""));
}

/// Positive: the crate declares the M14 dependency it consumes, and nothing
/// else beyond the workspace's own three.
#[test]
fn the_crate_declares_the_consumed_contract_dependency() {
    let manifest = read("crates/aegis-vesta/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    assert!(
        manifest.contains("aegis-justitia = { path = \"../aegis-justitia\" }"),
        "D04's admission path is consumed from P06's crate, not redeclared"
    );
    for expected in ["serde", "serde_json", "thiserror"] {
        assert!(manifest.contains(expected));
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the member list is written out, never globbed.
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

/// Negative: decision D06's dependency half, asserted against the manifest.
///
/// A Go runtime cannot be a Cargo dependency at all, so what a violation would
/// actually look like is a binding crate, a foreign-function build script or a
/// Rust engine pinned here without a recorded admission. Each is refused.
#[test]
fn the_crate_manifest_declares_no_runtime_or_monitor_dependency() {
    let manifest = read("crates/aegis-vesta/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    for deferred in [
        "wasmtime",
        "wasmer",
        "wasmi",
        "extism",
        "wazero",
        "kvm-ioctls",
        "firecracker",
        "vsock",
        "io-uring",
        "tokio",
        "libc",
        "bindgen",
        "cc",
    ] {
        assert!(
            !manifest.contains(deferred),
            "{deferred} is deferred past milestone M06"
        );
    }
    assert!(
        !manifest.contains("[build-dependencies]"),
        "a build script is how a foreign runtime would arrive"
    );
}

/// Negative: milestone M06 ships a library, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    let manifest = read("crates/aegis-vesta/Cargo.toml");
    assert!(!manifest.contains("[[bin]]"));
    assert!(manifest.contains("[lib]"));
    assert!(manifest.contains("publish = false"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the workspace lists exactly the crate directories that have a
/// manifest, no more and no fewer.
///
/// Not a hard-coded count: the number changes every time a milestone activates
/// a crate. What must not change is that a directory with a manifest is a
/// member and nothing else is, so a new crate cannot be built by the workspace
/// without being named, and a named member cannot vanish from the tree.
#[test]
fn the_workspace_lists_every_crate_with_a_manifest() {
    let manifest = read("Cargo.toml");
    let members: Vec<String> = manifest
        .lines()
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

/// Boundary: the crate inherits the workspace lints rather than restating them.
#[test]
fn the_crate_inherits_the_workspace_lints() {
    let manifest = read("crates/aegis-vesta/Cargo.toml");
    assert!(manifest.contains("[lints]"));
    assert!(manifest.contains("workspace = true"));
    assert!(!manifest.contains("[lints.rust]"));
    assert!(!manifest.contains("[lints.clippy]"));
    let root = read("Cargo.toml");
    assert!(root.contains("unsafe_code = \"forbid\""));
}
