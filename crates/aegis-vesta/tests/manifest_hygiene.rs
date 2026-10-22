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

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::read;

// --- Positive -------------------------------------------------------------

/// Positive: the workspace names both M06 crates as members.
#[test]
fn the_workspace_names_both_milestone_crates() {
    manifest_hygiene::assert_workspace_names_crate("aegis-vesta");
    manifest_hygiene::assert_workspace_names_crate("aegis-minerva");
}

/// Positive: the lock file carries an entry for both.
#[test]
fn the_lock_file_carries_an_entry_for_both() {
    manifest_hygiene::assert_lock_carries_entry("aegis-vesta");
    manifest_hygiene::assert_lock_carries_entry("aegis-minerva");
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
    manifest_hygiene::assert_member_list_is_written_out_and_not_globbed();
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
    manifest_hygiene::assert_crate_declares_no_binary_target("crates/aegis-vesta/Cargo.toml");
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
    manifest_hygiene::assert_inherits_workspace_lints("crates/aegis-vesta/Cargo.toml");
}
