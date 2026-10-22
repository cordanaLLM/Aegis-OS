// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P04 side.
//!
//! Three of this milestone's claims are about files rather than about code:
//! `aegis-compositor` is a workspace member with a lock entry, the member list
//! is written out rather than globbed, and **the crate takes no compositor
//! library of any kind, no Wayland or input binding, no accelerator crate and
//! no mesh transport**. That is decision D08's dependency half -- ADR-0001
//! selects pure Rust and admits no C compositor toolchain -- and the exit
//! criteria's "no wlroots, GPU, `PipeWire`, cgroups or BPF attach" and "the
//! export-006 Zenoh pin is not inherited". Each is asserted here rather than
//! left as prose.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::{dependency_table, read};

/// This crate's package name, the start of its closure (D78).
const PACKAGE: &str = "aegis-compositor";

/// The crates this crate's closure must not resolve, the list the lock-wide
/// sweep carried before D78, kept verbatim.
const FORBIDDEN: [&str; 8] = [
    "zenoh",
    "zenoh-transport",
    "smithay",
    "wayland-server",
    "wayland-backend",
    "drm",
    "gbm",
    "tokio",
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

/// Positive: the crate declares the two consumed-contract path dependencies
/// and the workspace's own three, and nothing else.
///
/// P04 builds P07's focus-switch report and consumes P08's stream descriptor,
/// so it depends on both of those crates rather than redeclaring either
/// message.
#[test]
fn the_crate_declares_the_consumed_contract_dependencies() {
    let manifest = read("crates/aegis-compositor/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    for expected in [
        "aegis-calliope = { path = \"../aegis-calliope\" }",
        "aegis-lictor = { path = \"../aegis-lictor\" }",
        "serde",
        "serde_json",
        "thiserror",
    ] {
        assert!(
            manifest.contains(expected),
            "the manifest must declare {expected}"
        );
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the member list is written out, never globbed.
#[test]
fn the_member_list_is_written_out_and_not_globbed() {
    manifest_hygiene::assert_member_list_is_written_out_and_not_globbed();
}

/// Negative: decision D08's dependency half and the exit criterion's, asserted
/// against the manifest.
///
/// No compositor library, C or Rust; no Wayland, input or keymap binding; no
/// accelerator or DRM crate; no mesh transport; no async runtime; and no
/// foreign-function build script, which is how a C library would arrive
/// despite ADR-0001. The `zenoh` requirement the imported manifest proposes is
/// the one this is most directly about: it is proposal data, so the name must
/// not appear here at all.
#[test]
fn the_crate_manifest_declares_no_compositor_or_transport_dependency() {
    let manifest = read("crates/aegis-compositor/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    let declared = dependency_table(&manifest);
    assert!(
        !declared.is_empty(),
        "the manifest must declare dependencies"
    );
    for deferred in [
        "smithay",
        "wlroots",
        "libweston",
        "wayland-server",
        "wayland-client",
        "wayland-protocols",
        "input",
        "xkbcommon",
        "drm",
        "gbm",
        "ash",
        "wgpu",
        "zenoh",
        "iceoryx",
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
    manifest_hygiene::assert_crate_declares_no_binary_target("crates/aegis-compositor/Cargo.toml");
}

/// Negative: this crate's own dependency closure resolves no multimedia or
/// async crate.
///
/// The exit criterion says the M02 toolchain is reused. The dependency half of
/// that was a lock-wide sweep until M27; decision D78 re-scoped it to this
/// crate's own closure, read from `cargo metadata`, because P17's
/// smithay-client-toolkit puts wayland-backend into the shared lock while this
/// crate still depends on none of it. The forbidden list is the one the
/// lock-wide sweep carried, unchanged.
#[test]
fn the_lock_resolves_no_multimedia_or_async_crate() -> Result<(), String> {
    let names = dependency_closure::assert_closure_forbids(PACKAGE, &FORBIDDEN)?;
    assert!(names.contains(PACKAGE), "the closure starts at {PACKAGE}");
    Ok(())
}

/// Negative (E27-3): tokio planted as a dev-dependency of this crate, the way
/// `cargo metadata` reports a manifest edit, fails the sweep: the closure
/// follows every edge whatever its `dep_kinds`.
#[test]
fn a_tokio_dev_dependency_planted_in_this_closure_fails_the_sweep() -> Result<(), String> {
    let mut metadata = dependency_closure::metadata()?;
    dependency_closure::plant(&mut metadata, PACKAGE, "tokio", Some("dev"))?;
    let names = dependency_closure::closure(&metadata, PACKAGE)?;
    assert_eq!(dependency_closure::hits(&names, &FORBIDDEN), vec!["tokio"]);
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
    manifest_hygiene::assert_inherits_workspace_lints("crates/aegis-compositor/Cargo.toml");
}

/// Boundary (E27-3): wayland-backend is in `Cargo.lock` -- P17's
/// smithay-client-toolkit resolves it -- but outside this crate's closure,
/// and the sweep passes.
#[test]
fn wayland_backend_in_the_lock_but_outside_this_closure_passes() -> Result<(), String> {
    let lock = read("Cargo.lock");
    assert!(lock.contains("name = \"wayland-backend\""));
    let names = dependency_closure::own_closure(PACKAGE)?;
    assert!(!names.contains("wayland-backend"), "{names:?}");
    assert_eq!(
        dependency_closure::hits(&names, &FORBIDDEN),
        [] as [&str; 0]
    );
    Ok(())
}

/// Boundary (E27-3): a name that only shares a prefix with a forbidden one,
/// tokio-macros or wayland-backend-sys, is not a hit.
#[test]
fn a_name_that_only_shares_a_prefix_is_not_a_hit() -> Result<(), String> {
    let metadata = dependency_closure::synthetic(
        PACKAGE,
        &["tokio-macros", "wayland-backend-sys", "drm-fourcc"],
    );
    let names = dependency_closure::closure(&metadata, PACKAGE)?;
    assert_eq!(names.len(), 4, "{names:?}");
    assert_eq!(
        dependency_closure::hits(&names, &FORBIDDEN),
        [] as [&str; 0]
    );
    Ok(())
}
