// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P16 side.
//!
//! REQ-P16-07 states that `aegis-athena` is a declared member of the workspace,
//! and REQ-P16-09 that the ledger algorithm must be pinned before the port.
//! Both are claims about files, so both are asserted here rather than left as
//! prose. So is the negative half of D02: no BLAKE3 and no MD5 crate is
//! resolved, which is what makes "MD5 excluded structurally" a check rather
//! than a convention.
//!
//! The member *count* is deliberately not asserted. It changes every time a
//! milestone activates a crate, and hard-coding it in several crates is what
//! made two of them fail when this milestone added two members.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

use std::path::{Path, PathBuf};

/// This crate's package name, the start of its closure (D78).
const PACKAGE: &str = "aegis-athena";

/// The hash crates this crate's closure must not resolve: the list the
/// lock-wide sweep carried before D78, kept verbatim.
const FORBIDDEN: [&str; 4] = ["blake3", "md-5", "md5", "blake2"];

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

/// Positive: the workspace names this crate as a member (REQ-P16-07).
#[test]
fn the_workspace_names_this_crate_as_a_member() {
    let manifest = read("Cargo.toml");
    assert!(!manifest.is_empty(), "the workspace manifest must exist");
    assert!(
        manifest.contains("\"crates/aegis-athena\""),
        "REQ-P16-07: aegis-athena is a declared workspace member"
    );
}

/// Positive: the lock file carries an entry for this crate and its two
/// workspace dependencies.
#[test]
fn the_lock_file_carries_this_crate_and_its_dependencies() {
    let lock = read("Cargo.lock");
    assert!(!lock.is_empty(), "Cargo.lock must be committed");
    for name in ["aegis-athena", "aegis-tellus", "aegis-justitia"] {
        assert!(
            lock.contains(&format!("name = \"{name}\"")),
            "the lock must carry {name}"
        );
    }
}

/// Positive: the crate depends on the two workspace crates it consumes from.
///
/// `aegis-justitia` supplies the D02 hashing trait and the M14 audit record;
/// `aegis-tellus` supplies the shared identifiers and the SCI rate the gate
/// compares. Both are path dependencies inside this repository, so neither
/// resolves anything new.
#[test]
fn the_crate_depends_on_the_two_workspace_crates() {
    let manifest = read("crates/aegis-athena/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    assert!(manifest.contains("aegis-justitia = { path = \"../aegis-justitia\" }"));
    assert!(manifest.contains("aegis-tellus = { path = \"../aegis-tellus\" }"));
    for expected in ["serde", "serde_json", "thiserror"] {
        assert!(
            manifest.contains(expected),
            "the manifest must declare {expected}"
        );
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: no hash crate but the one D02 admits is resolved.
///
/// This is the structural half of D02. BLAKE3 is named by REQ-P16-01 and
/// REQ-P16-03 and MD5 by the reference daemon; neither has a crate, a lock
/// entry or a feature here, so a record naming either cannot be produced by
/// this crate at all. Since M27 the sweep reads this crate's own dependency
/// closure from `cargo metadata` (D78) rather than the whole lock; the
/// forbidden list is unchanged, and D02's algorithm must still be locked.
#[test]
fn no_hash_crate_but_the_one_d02_admits_is_resolved() -> Result<(), String> {
    let names = dependency_closure::own_closure(PACKAGE)?;
    let found = dependency_closure::hits(&names, &FORBIDDEN);
    assert!(
        found.is_empty(),
        "the closure of {PACKAGE} resolves {found:?}"
    );
    assert!(
        names.contains("sha2"),
        "the algorithm D02 chose reaches this crate through aegis-justitia"
    );
    let lock = read("Cargo.lock");
    assert!(
        lock.contains("name = \"sha2\""),
        "the algorithm D02 chose must be locked"
    );

    let manifest = read("crates/aegis-athena/Cargo.toml");
    assert!(
        !manifest.contains("sha2"),
        "the hash arrives through aegis-justitia's D02 trait, not a second dependency"
    );
    Ok(())
}

/// Negative: this crate declares no update, hypervisor or bus dependency.
///
/// A real P16 would need a `systemd-sysupdate` client, a D-Bus connection and
/// a microVM. This crate reaches none of them, so it declares none of them,
/// and the roadmap keeps each behind a later milestone.
#[test]
fn the_crate_manifest_declares_no_deferred_dependency() {
    let manifest = read("crates/aegis-athena/Cargo.toml");
    for deferred in [
        "zbus",
        "dbus",
        "tokio",
        "libc",
        "firecracker",
        "vsock",
        "kvm-ioctls",
        "nix",
    ] {
        assert!(
            !manifest.contains(deferred),
            "{deferred} is deferred past milestone M05"
        );
    }
}

/// Negative: milestone M05 ships a library, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    let manifest = read("crates/aegis-athena/Cargo.toml");
    assert!(!manifest.contains("[[bin]]"));
    assert!(manifest.contains("[lib]"));
    assert!(manifest.contains("publish = false"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the crate inherits the workspace lints rather than restating them.
#[test]
fn the_crate_inherits_the_workspace_lints() {
    let manifest = read("crates/aegis-athena/Cargo.toml");
    assert!(manifest.contains("[lints]"));
    assert!(manifest.contains("workspace = true"));
    assert!(!manifest.contains("[lints.rust]"));
    assert!(!manifest.contains("[lints.clippy]"));
    let root = read("Cargo.toml");
    assert!(root.contains("unsafe_code = \"forbid\""));
}

/// Boundary: the lock entry names exactly the five dependencies the manifest
/// declares, so a sixth cannot arrive unnoticed.
#[test]
fn the_lock_entry_names_exactly_five_dependencies() {
    let lock = read("Cargo.lock");
    let entry = lock
        .split("[[package]]")
        .find(|block| block.contains("name = \"aegis-athena\""))
        .unwrap_or_default();
    assert!(!entry.is_empty(), "the lock must carry this crate");
    let deps = dependency_names(entry);
    assert_eq!(
        deps,
        vec![
            "aegis-justitia",
            "aegis-tellus",
            "serde",
            "serde_json",
            "thiserror",
        ],
        "lock dependencies found: {deps:?}"
    );
    assert!(
        lock.contains("name = \"thiserror\"\nversion = \"1.")
            && lock.contains("name = \"thiserror\"\nversion = \"2."),
        "the case this strips a suffix for: thiserror 1 and 2 both locked"
    );
}

/// The package names a lock entry's `dependencies` array lists.
///
/// Cargo writes a dependency line as `"name",` while one version of the name
/// is locked and as `"name 2.0.20",` once two are (M27: cros-libva locks
/// thiserror 1 beside the workspace's thiserror 2), so the version it appends
/// is stripped before the names are compared.
fn dependency_names(entry: &str) -> Vec<&str> {
    entry
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('"')?.strip_suffix("\","))
        .map(|dependency| dependency.split(' ').next().unwrap_or(dependency))
        .collect()
}

/// Boundary: the toolchain pin is untouched by this milestone.
#[test]
fn the_toolchain_pin_is_the_one_milestone_m02_admitted() {
    let pin = read("rust-toolchain.toml");
    assert!(!pin.is_empty(), "the toolchain pin must be committed");
    assert!(pin.contains("channel = \"1.98.1\""));
    let admission = read("docs/roadmap/toolchain-admission.md");
    assert!(
        admission.contains("sha2"),
        "the admission record must name the hash crate D02 chose"
    );
}
