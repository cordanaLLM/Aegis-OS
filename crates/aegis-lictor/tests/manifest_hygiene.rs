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

/// Returns the manifest's `[dependencies]` table, and only that table.
///
/// The check below is about what the crate *depends on*, not about which words
/// appear in its manifest: the package description names the very libraries the
/// crate says it does not use, and a whole-file substring search would fail on
/// that sentence. Scoping the search to the dependency table is what makes the
/// check mean what it says.
fn dependency_table(manifest: &str) -> String {
    let mut inside = false;
    let mut out = String::new();
    for line in manifest.lines().take(512) {
        let text = line.trim();
        if text.starts_with('[') {
            inside = text == "[dependencies]" || text == "[dev-dependencies]";
            continue;
        }
        if inside {
            out.push_str(text);
            out.push('\n');
        }
    }
    out
}

// --- Positive -------------------------------------------------------------

/// Positive: the workspace names this crate as a member.
#[test]
fn the_workspace_names_this_crate() {
    let manifest = read("Cargo.toml");
    assert!(!manifest.is_empty(), "the workspace manifest must exist");
    assert!(manifest.contains("\"crates/aegis-lictor\""));
}

/// Positive: the lock file carries an entry for it.
#[test]
fn the_lock_file_carries_an_entry() {
    let lock = read("Cargo.lock");
    assert!(!lock.is_empty(), "Cargo.lock must be committed");
    assert!(lock.contains("name = \"aegis-lictor\""));
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
    let manifest = read("Cargo.toml");
    for glob in ["crates/*", "crates/**", "\"*\""] {
        assert!(
            !manifest.contains(glob),
            "the member list must be written out, not globbed with {glob}"
        );
    }
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
    let manifest = read("crates/aegis-lictor/Cargo.toml");
    assert!(!manifest.contains("[[bin]]"));
    assert!(manifest.contains("[lib]"));
    assert!(manifest.contains("publish = false"));
}

/// Negative: no new third-party crate is resolved into the lock file.
///
/// The exit criterion says the M02 toolchain is reused. The dependency half of
/// that is that this milestone resolves nothing new, so the lock's package set
/// must not gain a multimedia or async entry.
#[test]
fn the_lock_resolves_no_multimedia_or_async_crate() {
    let lock = read("Cargo.lock");
    assert!(!lock.is_empty(), "Cargo.lock must be committed");
    for absent in [
        "name = \"aya\"",
        "name = \"aya-bpf\"",
        "name = \"libbpf-rs\"",
        "name = \"tokio\"",
        "name = \"monoio\"",
        "name = \"zenoh\"",
        "name = \"zbus\"",
        "name = \"memmap2\"",
    ] {
        assert!(!lock.contains(absent), "the lock resolves {absent}");
    }
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

/// Boundary: the crate inherits the workspace lints rather than restating them.
#[test]
fn the_crate_inherits_the_workspace_lints() {
    let manifest = read("crates/aegis-lictor/Cargo.toml");
    assert!(manifest.contains("[lints]"));
    assert!(manifest.contains("workspace = true"));
    assert!(!manifest.contains("[lints.rust]"));
    assert!(!manifest.contains("[lints.clippy]"));
    let root = read("Cargo.toml");
    assert!(root.contains("unsafe_code = \"forbid\""));
}
