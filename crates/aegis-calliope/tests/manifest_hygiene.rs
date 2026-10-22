// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P08 side.
//!
//! Three of this milestone's claims are about files rather than about code:
//! `aegis-calliope` is a workspace member with a lock entry, the member list is
//! written out rather than globbed, and **the crate takes no `PipeWire`
//! binding, no plugin host, no accelerator crate and no async runtime**, which
//! is the exit criterion's "no `PipeWire`". Each is asserted here rather than
//! left as prose.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::{dependency_table, read};

/// This crate's package name, the start of its closure (D78).
const PACKAGE: &str = "aegis-calliope";

/// The crates this crate's closure must not resolve: the list the lock-wide
/// sweep carried before D78, kept verbatim.
const FORBIDDEN: [&str; 8] = [
    "pipewire",
    "libspa",
    "tokio",
    "zenoh",
    "aya",
    "smithay",
    "wayland-server",
    "drm",
];

// --- Positive -------------------------------------------------------------

/// Positive: the workspace names this crate as a member.
#[test]
fn the_workspace_names_this_crate() {
    manifest_hygiene::assert_workspace_names_crate(PACKAGE);
}

/// Positive: the lock file carries an entry for it.
#[test]
fn the_lock_file_carries_an_entry() {
    manifest_hygiene::assert_lock_carries_entry(PACKAGE);
}

/// Positive: the crate declares the workspace's own three dependencies and no
/// path dependency at all.
///
/// P08 sits at the bottom of this milestone's crate graph: P07 and P04 both
/// depend on it, so it depends on neither, and a path dependency here would
/// close a cycle Cargo refuses outright.
#[test]
fn the_crate_declares_only_the_workspace_dependencies() {
    let manifest = read("crates/aegis-calliope/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    for expected in ["serde", "serde_json", "thiserror"] {
        assert!(manifest.contains(expected));
    }
    assert!(
        !manifest.contains("path = \"../"),
        "P08 is the root of this milestone's crate graph and depends on no sibling"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: the member list is written out, never globbed.
#[test]
fn the_member_list_is_written_out_and_not_globbed() {
    manifest_hygiene::assert_member_list_is_written_out_and_not_globbed();
}

/// Negative: the exit criterion's dependency half, asserted against the
/// manifest.
///
/// No `PipeWire` binding, no plugin host or bridge, no accelerator or DRM
/// crate, no async runtime and no foreign-function build script. Each would be
/// how the effects this crate says it does not have would arrive.
#[test]
fn the_crate_manifest_declares_no_multimedia_dependency() {
    let manifest = read("crates/aegis-calliope/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    let declared = dependency_table(&manifest);
    assert!(
        !declared.is_empty(),
        "the manifest must declare dependencies"
    );
    for deferred in [
        "pipewire",
        "libspa",
        "gstreamer",
        "alsa",
        "jack",
        "cpal",
        "drm",
        "gbm",
        "ash",
        "wgpu",
        "vst3",
        "lv2",
        "clack",
        "tokio",
        "async-std",
        "libc",
        "nix",
        "rustix",
        "bindgen",
        "pkg-config",
        "cc",
    ] {
        assert!(
            !declared.contains(deferred),
            "{deferred} is deferred past milestone M07"
        );
    }
    assert!(
        !manifest.contains("[build-dependencies]"),
        "a build script is how a foreign library would arrive"
    );
}

/// Negative: milestone M07 ships libraries, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    manifest_hygiene::assert_crate_declares_no_binary_target("crates/aegis-calliope/Cargo.toml");
}

/// Negative: this crate's own dependency closure resolves no multimedia or
/// async crate.
///
/// The exit criterion says the M02 toolchain is reused. The dependency half of
/// that was a lock-wide sweep until M27; decision D78 re-scoped it to this
/// crate's own closure, read from `cargo metadata`, so P17's Wayland and VA-API
/// crates in the shared lock do not trip it while this crate still resolves
/// none of them. The forbidden list is the one the lock-wide sweep carried.
#[test]
fn the_lock_resolves_no_multimedia_or_async_crate() -> Result<(), String> {
    let names = dependency_closure::assert_closure_forbids(PACKAGE, &FORBIDDEN)?;
    assert!(names.contains(PACKAGE), "the closure starts at {PACKAGE}");
    Ok(())
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
    manifest_hygiene::assert_inherits_workspace_lints("crates/aegis-calliope/Cargo.toml");
}
