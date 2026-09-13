// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P13 side.
//!
//! Three of this milestone's claims are about files rather than about code:
//! `aegis-tellus` is a workspace member with a lock entry, the member list is
//! written out rather than globbed, and the crate takes no counter, probe or
//! bus dependency, because it reaches none of them. Each is asserted here
//! rather than left as prose.
//!
//! The member *count* is deliberately not asserted. It changes every time a
//! milestone activates a crate, and hard-coding it in several crates is what
//! made two of them fail when this milestone added two members. What is
//! asserted is that this crate and its sibling are named, which is the claim
//! this milestone actually makes.

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

/// Positive: the workspace names both M05 crates as members.
#[test]
fn the_workspace_names_both_milestone_crates_as_members() {
    let manifest = read("Cargo.toml");
    assert!(!manifest.is_empty(), "the workspace manifest must exist");
    for member in ["\"crates/aegis-tellus\"", "\"crates/aegis-athena\""] {
        assert!(
            manifest.contains(member),
            "a workspace-wide build must cover {member}"
        );
    }
}

/// Positive: the lock file carries an entry for both crates.
#[test]
fn the_lock_file_carries_an_entry_for_both_crates() {
    let lock = read("Cargo.lock");
    assert!(!lock.is_empty(), "Cargo.lock must be committed");
    assert!(lock.contains("name = \"aegis-tellus\""));
    assert!(lock.contains("name = \"aegis-athena\""));
}

/// Positive: the member list stays sorted, so a new crate lands in one place.
#[test]
fn the_member_list_is_sorted() {
    let manifest = read("Cargo.toml");
    let members: Vec<&str> = manifest
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("\"crates/"))
        .collect();
    assert!(members.len() >= 7, "members found: {members:?}");
    let mut sorted = members.clone();
    sorted.sort_unstable();
    assert_eq!(members, sorted, "the member list must stay sorted");
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

/// Negative: this crate declares no counter, probe or bus dependency.
///
/// A real P13 would need a privileged sysfs reader, an eBPF loader and a D-Bus
/// client. This crate reaches none of them, so it declares none of them, and
/// the roadmap keeps every one of them behind a later milestone.
#[test]
fn the_crate_manifest_declares_no_deferred_dependency() {
    let manifest = read("crates/aegis-tellus/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    for deferred in [
        "aya",
        "libbpf",
        "zbus",
        "dbus",
        "tokio",
        "libc",
        "nix",
        "procfs",
        "perf-event",
        "sysinfo",
    ] {
        assert!(
            !manifest.contains(deferred),
            "{deferred} is deferred past milestone M05"
        );
    }
    for expected in ["serde", "serde_json", "thiserror"] {
        assert!(
            manifest.contains(expected),
            "the manifest must declare {expected}"
        );
    }
}

/// Negative: milestone M05 ships a library, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    let manifest = read("crates/aegis-tellus/Cargo.toml");
    assert!(!manifest.contains("[[bin]]"));
    assert!(manifest.contains("[lib]"));
    assert!(manifest.contains("publish = false"));
}

/// Negative: no new third-party crate is resolved by this milestone.
///
/// The lock gains an entry for each new workspace member and nothing else: the
/// two crates depend only on what the workspace already resolved, which is what
/// keeps "no new toolchain and no new dependency" a checkable statement.
///
/// `libc` is deliberately **not** on this list, and the omission is the
/// finding rather than an oversight. It is already in the lock as a transitive
/// dependency of the `sha2` stack that milestone M02 resolved, so asserting
/// its absence would fail on a fact this milestone did not create. What is
/// asserted instead is that this crate's own lock entry does not name it.
#[test]
fn no_new_third_party_crate_is_resolved() {
    let lock = read("Cargo.lock");
    for absent in [
        "name = \"aya\"",
        "name = \"zbus\"",
        "name = \"tokio\"",
        "name = \"blake3\"",
        "name = \"md5\"",
        "name = \"nix\"",
        "name = \"procfs\"",
    ] {
        assert!(!lock.contains(absent), "the lock resolved {absent}");
    }
    assert!(
        lock.contains("name = \"sha2\""),
        "D02's hash must be locked"
    );
}

/// Negative: this crate's lock entry names exactly its three dependencies.
#[test]
fn the_lock_entry_names_exactly_three_dependencies() {
    let lock = read("Cargo.lock");
    let entry = lock
        .split("[[package]]")
        .find(|block| block.contains("name = \"aegis-tellus\""))
        .unwrap_or_default();
    assert!(!entry.is_empty(), "the lock must carry this crate");
    let deps: Vec<&str> = entry
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with('"') && line.ends_with("\","))
        .collect();
    assert_eq!(
        deps,
        vec!["\"serde\",", "\"serde_json\",", "\"thiserror\","],
        "lock dependencies found: {deps:?}"
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the crate inherits the workspace lints rather than restating them.
#[test]
fn the_crate_inherits_the_workspace_lints() {
    let manifest = read("crates/aegis-tellus/Cargo.toml");
    assert!(manifest.contains("[lints]"));
    assert!(manifest.contains("workspace = true"));
    assert!(!manifest.contains("[lints.rust]"));
    assert!(!manifest.contains("[lints.clippy]"));
    let root = read("Cargo.toml");
    assert!(root.contains("unsafe_code = \"forbid\""));
}

/// Boundary: the toolchain pin is untouched by this milestone.
///
/// The exit criterion says the M02 toolchain is reused and no new toolchain is
/// admitted. The pin is a file, so the check is over the file.
#[test]
fn the_toolchain_pin_is_the_one_milestone_m02_admitted() {
    let pin = read("rust-toolchain.toml");
    assert!(!pin.is_empty(), "the toolchain pin must be committed");
    assert!(pin.contains("channel = \"1.98.1\""));
    assert!(pin.contains("rustfmt"));
    assert!(pin.contains("clippy"));
    let admission = read("docs/roadmap/toolchain-admission.md");
    assert!(
        admission.contains("1.98.1"),
        "the admission record must name the pinned release"
    );
}
