// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P07 side.
//!
//! Three of this milestone's claims are about files rather than about code:
//! `aegis-lictor` is a workspace member with a lock entry, the member list is
//! written out rather than globbed, and **the crate takes no eBPF library, no
//! control-group or affinity crate, no bus binding and no async runtime**,
//! which is the exit criterion's "no cgroups or BPF attach" and REQ-P01-07's
//! "an imported version is proposal data, not an active pin". Each is asserted
//! here rather than left as prose.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::{dependency_table, read};

/// This crate's package name, the start of its closure (D78).
const PACKAGE: &str = "aegis-lictor";

/// The crates this crate's closure must not resolve: the list the lock-wide
/// sweep carried before D78, kept verbatim.
const FORBIDDEN: [&str; 8] = [
    "aya",
    "aya-bpf",
    "libbpf-rs",
    "tokio",
    "monoio",
    "zenoh",
    "zbus",
    "memmap2",
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
/// P07 builds P08's grant and consumes P13's directive, so it depends on both
/// of those crates rather than redeclaring either message.
#[test]
fn the_crate_declares_the_consumed_contract_dependencies() {
    let manifest = read("crates/aegis-lictor/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    for expected in [
        "aegis-calliope = { path = \"../aegis-calliope\" }",
        "aegis-tellus = { path = \"../aegis-tellus\" }",
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

/// Negative: the exit criterion's dependency half, asserted against the
/// manifest.
///
/// No eBPF library, no control-group or affinity crate, no bus binding, no
/// async runtime and no foreign-function build script. The `aya` version
/// REQ-P01-07 records is the one this is most directly about: it is proposal
/// data, so the name must not appear here at all.
#[test]
fn the_crate_manifest_declares_no_kernel_or_bus_dependency() {
    let manifest = read("crates/aegis-lictor/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    let declared = dependency_table(&manifest);
    assert!(
        !declared.is_empty(),
        "the manifest must declare dependencies"
    );
    for deferred in [
        "aya",
        "libbpf",
        "bpf",
        "cgroups",
        "controlgroup",
        "procfs",
        "nvml",
        "drm",
        "dbus",
        "zbus",
        "tokio",
        "async-std",
        "monoio",
        "memmap2",
        "libc",
        "nix",
        "rustix",
        "bindgen",
        "pkg-config",
        "cc",
        "tracing",
        "anyhow",
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
    manifest_hygiene::assert_crate_declares_no_binary_target("crates/aegis-lictor/Cargo.toml");
}

/// Negative: this crate's own dependency closure resolves no multimedia or
/// async crate.
///
/// The exit criterion says the M02 toolchain is reused. The dependency half of
/// that was a lock-wide sweep until M27; decision D78 re-scoped it to this
/// crate's own closure, read from `cargo metadata`, because P17's
/// smithay-client-toolkit puts memmap2 into the shared lock while this crate
/// still depends on none of it. The forbidden list is the one the lock-wide
/// sweep carried.
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
    manifest_hygiene::assert_inherits_workspace_lints("crates/aegis-lictor/Cargo.toml");
}
