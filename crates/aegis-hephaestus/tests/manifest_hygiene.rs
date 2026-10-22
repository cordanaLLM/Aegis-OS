// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Executable checks on the workspace manifests, from the P14 side.
//!
//! Three of this milestone's claims are about files rather than about code:
//! `aegis-hephaestus` is a workspace member with a lock entry, it reuses the
//! M02 toolchain and admits no new one, and it declares **no CAD kernel, no
//! mesher, no solver binding and no build script** -- which is why "the CAD and
//! solver versions are unpinned" is a fact about the dependency set and not
//! only a line in the register.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::{declared_dependencies, read};

// --- Positive -------------------------------------------------------------

/// Positive: the workspace names this crate as a member.
#[test]
fn the_workspace_names_this_crate_as_a_member() {
    manifest_hygiene::assert_workspace_names_crate("aegis-hephaestus");
}

/// Positive: the lock file carries an entry for this crate.
#[test]
fn the_lock_file_carries_an_entry_for_this_crate() {
    manifest_hygiene::assert_lock_carries_entry("aegis-hephaestus");
}

/// Positive: the crate declares the producer crate whose request type it
/// consumes, and the identifier crate whose field types it fills in.
///
/// This is exit criterion 4 as a file fact: the P09 verification request is
/// consumed from P09's crate, so it cannot be redefined here without the
/// dependency becoming unused.
#[test]
fn the_crate_declares_the_producer_and_identifier_crates() {
    let manifest = read("crates/aegis-hephaestus/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    assert!(manifest.contains("aegis-minerva = { path = \"../aegis-minerva\" }"));
    assert!(manifest.contains("aegis-justitia = { path = \"../aegis-justitia\" }"));
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

/// Negative: no CAD kernel, mesher, solver binding, process or clock
/// dependency, and no build script.
///
/// A solver binding is also where the outstanding licence decision for
/// GPL-licensed bindings would arrive, which is recorded as an activation
/// blocker for P14. The check is on a declaration line rather than on the whole
/// file, because the package description names several of these words in order
/// to say the crate does not use them -- a substring sweep would fail on the
/// sentence that makes the claim.
#[test]
fn the_crate_manifest_declares_no_deferred_dependency() {
    let manifest = read("crates/aegis-hephaestus/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    let declarations: Vec<&str> = manifest
        .lines()
        .map(str::trim)
        .filter(|line| line.contains(" = ") || line.ends_with(".workspace = true"))
        .collect();
    for excluded in [
        "truck",
        "truck-modeling",
        "opencascade",
        "occt-sys",
        "manifold3d",
        "csgrs",
        "gmsh",
        "gmsh-sys",
        "z3",
        "z3-sys",
        "bindgen",
        "cc",
        "cgroups-rs",
        "nix",
        "tokio",
        "libc",
        "chrono",
        "time",
    ] {
        assert!(
            !declarations
                .iter()
                .any(|line| line.starts_with(&format!("{excluded} "))
                    || line.starts_with(&format!("{excluded}."))),
            "{excluded} is deferred past milestone M08"
        );
    }
    assert!(
        !manifest.contains("[build-dependencies]"),
        "a build script is how a foreign kernel or solver would arrive"
    );
}

/// Negative: the declared dependency set is closed, so nothing arrives under a
/// name the excluded list does not carry.
///
/// The check above refuses eighteen spellings on a declaration line, and a
/// renamed dependency is none of them: `kernel = { package = "opencascade",
/// path = "../vendor" }` declares `kernel`, and every name check in this file
/// passes. What holds instead is the set itself -- two path dependencies on
/// crates already in the tree, and four inherited from the workspace table,
/// one of them `tornus-geometry` at a git revision since 2026-10-16 (cut unit
/// CUT-1632-01) -- so a seventh declaration cannot arrive without this
/// assertion being edited, and "the CAD and solver versions are unpinned"
/// stays a fact about the set rather than about the spellings someone listed.
#[test]
fn the_crate_declares_exactly_its_six_dependencies() {
    let manifest = read("crates/aegis-hephaestus/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    assert_eq!(
        declared_dependencies(&manifest),
        vec![
            "aegis-justitia",
            "aegis-minerva",
            "serde",
            "serde_json",
            "thiserror",
            "tornus-geometry"
        ]
    );
    let root = read("Cargo.toml");
    assert!(
        root.contains("tornus-geometry = { git = \"https://github.com/plurano/tornus\", rev = \"")
    );
}

/// Negative: milestone M08 ships a library, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    manifest_hygiene::assert_crate_declares_no_binary_target("crates/aegis-hephaestus/Cargo.toml");
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
        "crates/aegis-hephaestus/Cargo.toml",
    );
}

/// Boundary: the collector reads the declaration name, which is the half a
/// renamed dependency changes, and stops at the next table.
///
/// The planted line is the shape the excluded-name check cannot see: the
/// package is one this milestone defers and the declaration name is not. The
/// collector reports `kernel`, which is what makes the closed set above a gate
/// rather than a restatement of the six names.
#[test]
fn the_collector_reports_a_renamed_dependency() {
    let planted = concat!(
        "[dependencies]\n",
        "aegis-justitia = { path = \"../aegis-justitia\" }\n",
        "kernel = { package = \"opencascade\", version = \"0.4\" }\n",
        "# commented = \"1\"\n",
        "\n",
        "[lints]\n",
        "workspace = true\n",
    );
    assert_eq!(
        declared_dependencies(planted),
        vec!["aegis-justitia", "kernel"]
    );
    assert_eq!(declared_dependencies(""), [] as [std::string::String; 0]);
}
