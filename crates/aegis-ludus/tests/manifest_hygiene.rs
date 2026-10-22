// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P11 side.
//!
//! Three of this milestone's claims are about files rather than about code:
//! `aegis-ludus` is a workspace member with a lock entry, it reuses the M02
//! toolchain and admits no new one, and it declares **no platform SDK, no
//! binding generator and no build script**, which is the dependency half of
//! exit criterion 3 (D12, ADR-0002).

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::{declared_dependencies, read};

// --- Positive -------------------------------------------------------------

/// Positive: the workspace names this crate as a member.
#[test]
fn the_workspace_names_this_crate_as_a_member() {
    manifest_hygiene::assert_workspace_names_crate("aegis-ludus");
}

/// Positive: the lock file carries an entry for this crate.
#[test]
fn the_lock_file_carries_an_entry_for_this_crate() {
    manifest_hygiene::assert_lock_carries_entry("aegis-ludus");
}

/// Positive: the crate declares the identifier crate whose types it fills in.
///
/// The correlation identifier, the digest and the timestamp on the receipt are
/// `aegis-justitia`'s, consumed rather than redefined, so the shape M14 pinned
/// stays the one shape.
#[test]
fn the_crate_declares_the_shared_identifier_crate() {
    let manifest = read("crates/aegis-ludus/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    manifest_hygiene::assert_manifest_contains(
        &manifest,
        &[
            "aegis-justitia = { path = \"../aegis-justitia\" }",
            "serde",
            "serde_json",
            "thiserror",
        ],
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: the member list is written out, never globbed.
#[test]
fn the_member_list_is_written_out_and_not_globbed() {
    manifest_hygiene::assert_member_list_is_written_out_and_not_globbed();
}

/// Negative: no platform SDK, binding generator, socket, compositor or TPM2
/// dependency, and no build script.
///
/// This is the dependency half of exit criterion 3. The check is on a
/// declaration line rather than on the whole file, because the package
/// description names several of these words in order to say the crate does not
/// use them -- a substring sweep would fail on the sentence that makes the
/// claim.
#[test]
fn the_crate_manifest_declares_no_excluded_dependency() {
    manifest_hygiene::assert_no_excluded_dependencies(
        "crates/aegis-ludus/Cargo.toml",
        &[
            "steamworks",
            "steamworks-rs",
            "steamworks-sys",
            "bindgen",
            "cc",
            "discord-rich-presence",
            "discord-sdk",
            "tss-esapi",
            "tpm2-tss",
            "ctap-hid-fido2",
            "authenticator",
            "pipewire",
            "wayland-client",
            "drm",
            "tokio",
            "libc",
        ],
        "is excluded from milestone M08",
    );
    let manifest = read("crates/aegis-ludus/Cargo.toml");
    assert!(
        !manifest.contains("[build-dependencies]"),
        "a build script is how a binding generator would arrive"
    );
}

/// Negative: the declared dependency set is closed, so nothing arrives under a
/// name the excluded list does not carry.
///
/// The check above refuses sixteen spellings on a declaration line, and a
/// renamed dependency is none of them: `sdk = { package = "steamworks", path =
/// "../vendor" }` declares `sdk`, and every name check in this file passes.
/// What holds instead is the set itself -- one path dependency on a crate
/// already in the tree, and three inherited from the workspace table -- so a
/// fifth declaration cannot arrive without this assertion being edited.
#[test]
fn the_crate_declares_exactly_its_four_dependencies() {
    let manifest = read("crates/aegis-ludus/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    assert_eq!(
        declared_dependencies(&manifest),
        vec!["aegis-justitia", "serde", "serde_json", "thiserror"]
    );
}

/// Negative: milestone M08 ships a library, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    manifest_hygiene::assert_crate_declares_no_binary_target("crates/aegis-ludus/Cargo.toml");
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

/// Boundary: the crate inherits the workspace lints and the M02 toolchain
/// rather than restating either.
#[test]
fn the_crate_inherits_the_workspace_lints_and_toolchain() {
    manifest_hygiene::assert_inherits_workspace_lints_and_toolchain(
        "crates/aegis-ludus/Cargo.toml",
    );
}

/// Boundary: the collector reads the declaration name, which is the half a
/// renamed dependency changes, and stops at the next table.
///
/// The planted line is the shape the excluded-name check cannot see: the
/// package is the one D12 excludes and the declaration name is not. The
/// collector reports `sdk`, which is what makes the closed set above a gate
/// rather than a restatement of the four names.
#[test]
fn the_collector_reports_a_renamed_dependency() {
    let planted = concat!(
        "[dependencies]\n",
        "aegis-justitia = { path = \"../aegis-justitia\" }\n",
        "sdk = { package = \"steamworks\", version = \"0.11\" }\n",
        "# commented = \"1\"\n",
        "\n",
        "[lints]\n",
        "workspace = true\n",
    );
    assert_eq!(
        declared_dependencies(planted),
        vec!["aegis-justitia", "sdk"]
    );
    assert_eq!(declared_dependencies(""), [] as [std::string::String; 0]);
}
