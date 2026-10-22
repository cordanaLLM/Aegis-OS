// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P09 side.
//!
//! Three of this milestone's claims are about files rather than about code:
//! `aegis-minerva` is a workspace member with a lock entry, it declares the
//! two crates whose payload types it consumes rather than redefining them, and
//! it takes **no accelerator, solver, bus or runtime dependency**, which is the
//! exit criterion's "no GPU, D-Bus, Z3 FFI, KVM or Wasm engine is used".

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::read;

// --- Positive -------------------------------------------------------------

/// Positive: the workspace names this crate as a member.
#[test]
fn the_workspace_names_this_crate_as_a_member() {
    manifest_hygiene::assert_workspace_names_crate("aegis-minerva");
}

/// Positive: the lock file carries an entry for this crate.
#[test]
fn the_lock_file_carries_an_entry_for_this_crate() {
    manifest_hygiene::assert_lock_carries_entry("aegis-minerva");
}

/// Positive: the crate declares the two consumers whose types it fills in.
///
/// This is exit criterion 3 as a file fact: the M14 action proposal is
/// consumed from P06's crate and the capsule request from P10's, so neither
/// can be redefined here without the dependency becoming unused.
#[test]
fn the_crate_declares_both_consumed_contract_crates() {
    let manifest = read("crates/aegis-minerva/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    assert!(manifest.contains("aegis-justitia = { path = \"../aegis-justitia\" }"));
    assert!(manifest.contains("aegis-vesta = { path = \"../aegis-vesta\" }"));
    for expected in ["serde", "serde_json", "thiserror"] {
        assert!(manifest.contains(expected));
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the member list is written out, never globbed.
#[test]
fn the_member_list_is_written_out_and_not_globbed() {
    manifest_hygiene::assert_member_list_is_written_out_and_not_globbed();
}

/// Negative: no accelerator, solver, bus, runtime or clock dependency.
///
/// The check is on a declaration line rather than on the whole file, because
/// the package description names several of these words in order to say the
/// crate does not use them -- a substring sweep would fail on the sentence
/// that makes the claim.
#[test]
fn the_crate_manifest_declares_no_deferred_dependency() {
    manifest_hygiene::assert_no_excluded_dependencies(
        "crates/aegis-minerva/Cargo.toml",
        &[
            "cust",
            "cuda",
            "nvml",
            "z3",
            "zbus",
            "dbus",
            "tokio",
            "libc",
            "wasmtime",
            "kvm-ioctls",
            "candle",
            "ort",
            "tch",
            "chrono",
            "time",
        ],
        "is deferred past milestone M06",
    );
    let manifest = read("crates/aegis-minerva/Cargo.toml");
    assert!(
        !manifest.contains("[build-dependencies]"),
        "a build script is how a foreign solver would arrive"
    );
}

/// Negative: milestone M06 ships a library, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    manifest_hygiene::assert_crate_declares_no_binary_target("crates/aegis-minerva/Cargo.toml");
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
    manifest_hygiene::assert_workspace_lists_every_crate_with_a_manifest();
}

/// Boundary: the crate inherits the workspace lints rather than restating them.
#[test]
fn the_crate_inherits_the_workspace_lints() {
    manifest_hygiene::assert_inherits_workspace_lints("crates/aegis-minerva/Cargo.toml");
    let root = read("Cargo.toml");
    assert!(
        root.contains("rust-version = \"1.99.1\""),
        "D111: the workspace rust-version is the rust-toolchain.toml pin"
    );
}
