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

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::read;

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
    manifest_hygiene::assert_lock_carries_entry("aegis-hestia");
    manifest_hygiene::assert_lock_carries_entry("aegis-vulcan");
}

// --- Negative -------------------------------------------------------------

/// Negative: the member list is written out, never globbed.
///
/// A glob would activate every reserved directory the moment it acquired a
/// manifest, which is exactly the bar `crates/README.md` sets against.
#[test]
fn the_member_list_is_written_out_and_not_globbed() {
    manifest_hygiene::assert_member_list_is_written_out_and_not_globbed();
}

/// Negative: this crate declares no store, compositor or interface dependency.
///
/// A real P15 would need an embedded store, a Wayland client and a bundled
/// user interface. This crate reaches none of them, so it declares none of
/// them, and D09 puts the interface half in a separate package entirely.
#[test]
fn the_crate_manifest_declares_no_mapping_dependency() {
    manifest_hygiene::assert_deferred_and_expected_dependencies(
        "crates/aegis-hestia/Cargo.toml",
        &[
            "postgres", "sqlx", "wayland", "smithay", "wry", "tauri", "tokio", "libc", "rusqlite",
            "wasmtime",
        ],
        "M17",
        &["serde", "serde_json", "thiserror"],
    );
}

/// Negative: milestone M17 ships a library, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    manifest_hygiene::assert_crate_declares_no_binary_target("crates/aegis-hestia/Cargo.toml");
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
///
/// Restating them would let one crate drift from the invariants the workspace
/// root declares, `unsafe_code = "forbid"` among them.
#[test]
fn the_crate_inherits_the_workspace_lints() {
    manifest_hygiene::assert_inherits_workspace_lints("crates/aegis-hestia/Cargo.toml");
}
