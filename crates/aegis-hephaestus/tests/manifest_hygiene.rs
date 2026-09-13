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

/// The member list is checked against this rather than against a literal
/// count. A count has to be edited every time a milestone activates a crate,
/// and hard-coding it in several crates at once is what made three of them
/// fail when milestone M05 added two members. What actually needs holding is
/// the invariant behind the count: every crate directory that has a manifest
/// is a workspace member, and nothing else is.
fn crate_directories_with_a_manifest() -> Vec<String> {
    let Some(root) = workspace_root() else {
        return Vec::new();
    };
    let Ok(entries) = std::fs::read_dir(root.join("crates")) else {
        return Vec::new();
    };
    let mut found: Vec<String> = entries
        .flatten()
        .take(64)
        .filter(|entry| entry.path().join("Cargo.toml").is_file())
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .collect();
    found.sort_unstable();
    found
}

/// Returns the names the `[dependencies]` table declares, sorted and deduped.
///
/// The excluded-name check below is a check over the spellings it lists, and a
/// renamed dependency -- `kernel = { package = "opencascade", ... }` -- is not
/// one of them. Pinning the set **closed** is what makes the dependency half of
/// that claim hold for a name nobody thought to list. Only the
/// `[dependencies]` table is read: the walk stops at the next section header,
/// so a `[build-dependencies]` or `[lints]` table is never folded in.
fn declared_dependencies(manifest: &str) -> Vec<String> {
    let mut names: Vec<String> = Vec::new();
    let mut inside = false;
    for line in manifest.lines().take(256) {
        let line = line.trim();
        if line.starts_with('[') {
            inside = line == "[dependencies]";
        } else if inside && !line.is_empty() && !line.starts_with('#') {
            if let Some((name, _)) = line.split_once(['=', '.']) {
                names.push(name.trim().to_owned());
            }
        }
    }
    names.sort_unstable();
    names.dedup();
    names
}

// --- Positive -------------------------------------------------------------

/// Positive: the workspace names this crate as a member.
#[test]
fn the_workspace_names_this_crate_as_a_member() {
    let manifest = read("Cargo.toml");
    assert!(!manifest.is_empty(), "the workspace manifest must exist");
    assert!(manifest.contains("\"crates/aegis-hephaestus\""));
}

/// Positive: the lock file carries an entry for this crate.
#[test]
fn the_lock_file_carries_an_entry_for_this_crate() {
    let lock = read("Cargo.lock");
    assert!(!lock.is_empty(), "Cargo.lock must be committed");
    assert!(lock.contains("name = \"aegis-hephaestus\""));
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
    let manifest = read("Cargo.toml");
    for glob in ["crates/*", "crates/**", "\"*\""] {
        assert!(
            !manifest.contains(glob),
            "the member list must be written out, not globbed with {glob}"
        );
    }
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
/// crates already in the tree, and three inherited from the workspace table --
/// so a sixth declaration cannot arrive without this assertion being edited,
/// and "the CAD and solver versions are unpinned" stays a fact about the set
/// rather than about the spellings someone listed.
#[test]
fn the_crate_declares_exactly_its_five_dependencies() {
    let manifest = read("crates/aegis-hephaestus/Cargo.toml");
    assert!(!manifest.is_empty(), "the crate manifest must exist");
    assert_eq!(
        declared_dependencies(&manifest),
        vec![
            "aegis-justitia",
            "aegis-minerva",
            "serde",
            "serde_json",
            "thiserror"
        ]
    );
}

/// Negative: milestone M08 ships a library, so there is no binary target.
#[test]
fn the_crate_declares_no_binary_target() {
    let manifest = read("crates/aegis-hephaestus/Cargo.toml");
    assert!(!manifest.contains("[[bin]]"));
    assert!(manifest.contains("[lib]"));
    assert!(manifest.contains("publish = false"));
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
    let manifest = read("Cargo.toml");
    let members: Vec<String> = manifest
        .lines()
        .map(str::trim)
        .filter(|line| line.starts_with("\"crates/"))
        .map(|line| line.trim_matches(|c| c == '"' || c == ',').to_owned())
        .collect();
    let directories = crate_directories_with_a_manifest();
    assert!(!directories.is_empty(), "crates/ must hold manifests");
    assert_eq!(
        members.len(),
        directories.len(),
        "members {members:?} against directories {directories:?}"
    );
    for name in &directories {
        assert!(
            members
                .iter()
                .any(|member| member == &format!("crates/{name}")),
            "the member list must name {name}"
        );
    }
}

/// Boundary: the crate inherits the workspace lints and the M02 toolchain
/// rather than restating either.
#[test]
fn the_crate_inherits_the_workspace_lints_and_toolchain() {
    let manifest = read("crates/aegis-hephaestus/Cargo.toml");
    assert!(manifest.contains("[lints]"));
    assert!(manifest.contains("workspace = true"));
    assert!(!manifest.contains("[lints.rust]"));
    assert!(!manifest.contains("[lints.clippy]"));
    assert!(manifest.contains("rust-version.workspace = true"));
    let root = read("Cargo.toml");
    assert!(root.contains("unsafe_code = \"forbid\""));
    assert!(
        root.contains("rust-version = \"1.85\""),
        "the M02 toolchain"
    );
    let pinned = read("rust-toolchain.toml");
    assert!(
        pinned.contains("channel = \"1.98.1\""),
        "M08 admits no new toolchain: the pin stays the one M02 admitted"
    );
}

/// Boundary: the collector reads the declaration name, which is the half a
/// renamed dependency changes, and stops at the next table.
///
/// The planted line is the shape the excluded-name check cannot see: the
/// package is one this milestone defers and the declaration name is not. The
/// collector reports `kernel`, which is what makes the closed set above a gate
/// rather than a restatement of the five names.
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
    assert!(declared_dependencies("").is_empty());
}
