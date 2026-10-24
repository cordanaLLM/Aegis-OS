// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Criterion 1 of M16 and D78: the crate's manifest, its lock entry, its
//! admitted dependency, and its own dependency closure.
//!
//! The closure is read the way D78 records it: from
//! `cargo metadata --format-version 1 --locked --offline --all-features`
//! without `--filter-platform`, starting at the one workspace member named
//! `aegis-forum-shell`, following every `resolve.nodes[].deps[].pkg` edge
//! whatever its kinds, marking a node visited when it is pushed so the walk
//! is bounded by the node count, and matching package names exactly. It fails
//! closed on a spawn error, a non-zero exit, a missed deadline (HISS-02),
//! unparsable output, a null resolve, and zero or several start nodes; an
//! offline failure names `cargo fetch --locked` as the cure.
//!
//! The closure must not hold a GUI toolkit, a windowing or Wayland crate, a
//! D-Bus crate, an AT-SPI adapter or an async runtime (M16 criterion 1).

mod common;

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

use std::collections::BTreeSet;

use serde_json::Value;

use common::Fallible;
use dependency_closure::{
    assert_malformed_metadata_fails_closed, closure, closure_versions, hits, metadata,
    read_workspace_file as read, set, synthetic, workspace_root,
};

/// This crate's package name, which is the component name (P05).
const PACKAGE: &str = "aegis-forum-shell";

/// The crates M16's criterion 1 refuses in the closure: gpui, winit,
/// wayland-client, zbus, atspi, `accesskit_unix` and tokio.
const FORBIDDEN: [&str; 7] = [
    "gpui",
    "winit",
    "wayland-client",
    "zbus",
    "atspi",
    "accesskit_unix",
    "tokio",
];

// --- Positive -------------------------------------------------------------

/// Positive: a written-out member with a lock entry, inheriting the
/// workspace lints, so `unsafe_code = "forbid"` applies.
#[test]
fn the_crate_is_a_written_out_member_with_a_lock_entry_and_workspace_lints() {
    let root = read("Cargo.toml");
    assert!(root.contains("\"crates/aegis-forum-shell\""));
    assert!(!root.contains("crates/*"));
    assert!(root.contains("unsafe_code = \"forbid\""));
    let lock = read("Cargo.lock");
    assert!(lock.contains("name = \"aegis-forum-shell\""));
    let manifest = read("crates/aegis-forum-shell/Cargo.toml");
    assert!(
        manifest.contains("name = \"aegis-forum-shell\""),
        "the package name is the component name"
    );
    assert!(manifest.contains("[lints]\nworkspace = true"));
    assert!(!manifest.contains("[lints.rust]") && !manifest.contains("[lints.clippy]"));
    assert!(manifest.contains("publish = false"));
    assert!(!manifest.contains("[[bin]]") && !manifest.contains("[build-dependencies]"));
}

/// Positive: accesskit 0.25.1 is declared once, in the workspace table, used
/// by reference here, locked at that version, and the one accesskit in this
/// crate's closure. The lock also holds the accesskit gpui resolves (M28),
/// which only `aegis-forum-shell-gpui`'s closure reaches.
#[test]
fn accesskit_is_declared_once_and_locked_at_the_admitted_version() -> Fallible {
    let root = read("Cargo.toml");
    assert_eq!(root.matches("accesskit").count(), 1);
    assert!(root.contains("accesskit = { version = \"0.25.1\", default-features = false }"));
    let manifest = read("crates/aegis-forum-shell/Cargo.toml");
    assert!(manifest.contains("accesskit.workspace = true"));
    let lock = read("Cargo.lock");
    assert!(lock.contains("name = \"accesskit\"\nversion = \"0.25.1\""));
    let versions = closure_versions(&metadata()?, PACKAGE, "accesskit")?;
    assert_eq!(versions, BTreeSet::from(["0.25.1".to_owned()]));
    Ok(())
}

/// Positive: the real closure, read through `cargo metadata`, holds the
/// admitted data model and the two producer crates, and nothing forbidden.
#[test]
fn the_closure_holds_no_toolkit_window_bus_adapter_or_runtime() -> Fallible {
    let names = closure(&metadata()?, PACKAGE)?;
    for expected in [PACKAGE, "accesskit", "aegis-justitia", "aegis-tellus"] {
        assert!(
            names.contains(expected),
            "{expected} is not in the closure: {names:?}"
        );
    }
    assert_eq!(hits(&names, &FORBIDDEN), Vec::<&str>::new(), "{names:?}");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a closure that reaches any forbidden crate, directly or
/// transitively, is caught, and each is named.
#[test]
fn a_forbidden_crate_in_the_closure_is_caught() -> Fallible {
    let names = closure(&synthetic(PACKAGE, &FORBIDDEN), PACKAGE)?;
    assert_eq!(hits(&names, &FORBIDDEN), FORBIDDEN.to_vec());
    let mut deep = synthetic(PACKAGE, &["accesskit"]);
    if let Some(nodes) = deep
        .pointer_mut("/resolve/nodes")
        .and_then(Value::as_array_mut)
    {
        for node in nodes.iter_mut() {
            if node.get("id") == Some(&Value::from("accesskit")) {
                set(node, "deps", serde_json::json!([{ "pkg": "tokio" }]));
            }
        }
        nodes.push(serde_json::json!({ "id": "tokio", "deps": [] }));
    }
    if let Some(packages) = deep.get_mut("packages").and_then(Value::as_array_mut) {
        packages.push(serde_json::json!({ "id": "tokio", "name": "tokio" }));
    }
    assert_eq!(hits(&closure(&deep, PACKAGE)?, &FORBIDDEN), vec!["tokio"]);
    Ok(())
}

/// Negative: a metadata document with no start node, two, or a null resolve
/// fails closed.
#[test]
fn a_malformed_metadata_document_fails_closed() {
    assert_malformed_metadata_fails_closed(PACKAGE);
}

/// Negative: no Node, pnpm, Playwright or npm package serves the shell
/// (D101): there is no `ui/forum-shell` and no package manifest here.
#[test]
fn no_javascript_package_serves_the_shell() -> Fallible {
    let root = workspace_root().ok_or("no workspace root")?;
    assert!(!root.join("ui/forum-shell").exists());
    let crate_dir = root.join("crates/aegis-forum-shell");
    for name in [
        "package.json",
        "pnpm-lock.yaml",
        "node_modules",
        "playwright.config.js",
    ] {
        assert!(!crate_dir.join(name).exists(), "{name}");
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: names match exactly -- `accesskit` is admitted while
/// `accesskit_unix` is refused, and `tokio-util` is not `tokio`; a forbidden
/// crate outside the closure does not count.
#[test]
fn names_match_exactly_and_only_the_closure_counts() -> Fallible {
    let names = closure(
        &synthetic(PACKAGE, &["accesskit", "tokio-util", "wayland-client-sys"]),
        PACKAGE,
    )?;
    assert_eq!(hits(&names, &FORBIDDEN), Vec::<&str>::new());
    let mut outside = synthetic(PACKAGE, &["accesskit"]);
    if let Some(packages) = outside.get_mut("packages").and_then(Value::as_array_mut) {
        packages.push(serde_json::json!({ "id": "zbus", "name": "zbus" }));
    }
    if let Some(nodes) = outside
        .pointer_mut("/resolve/nodes")
        .and_then(Value::as_array_mut)
    {
        nodes.push(serde_json::json!({ "id": "zbus", "deps": [] }));
    }
    assert_eq!(
        hits(&closure(&outside, PACKAGE)?, &FORBIDDEN),
        Vec::<&str>::new()
    );
    Ok(())
}

/// Negative and boundary: a second accesskit reached through this crate's
/// closure is a second version there, and one in the document but outside the
/// closure is not.
#[test]
fn only_an_accesskit_inside_the_closure_counts_as_a_version() -> Fallible {
    let mut doc = synthetic(PACKAGE, &["accesskit", "consumer"]);
    if let Some(packages) = doc.get_mut("packages").and_then(Value::as_array_mut) {
        for entry in packages.iter_mut() {
            if entry.get("name").and_then(Value::as_str) == Some("accesskit") {
                set(entry, "version", Value::from("0.25.1"));
            }
        }
        packages.push(serde_json::json!({ "id": "old", "name": "accesskit", "version": "0.24.1" }));
    }
    let nodes = doc
        .pointer_mut("/resolve/nodes")
        .and_then(Value::as_array_mut)
        .ok_or("no nodes")?;
    nodes.push(serde_json::json!({ "id": "old", "deps": [] }));
    let outside = closure_versions(&doc, PACKAGE, "accesskit")?;
    assert_eq!(outside, BTreeSet::from(["0.25.1".to_owned()]));
    for node in nodes_mut(&mut doc) {
        if node.get("id").and_then(Value::as_str) == Some("consumer") {
            set(node, "deps", serde_json::json!([{ "pkg": "old" }]));
        }
    }
    let inside = closure_versions(&doc, PACKAGE, "accesskit")?;
    assert_eq!(
        inside,
        BTreeSet::from(["0.24.1".to_owned(), "0.25.1".to_owned()])
    );
    Ok(())
}

/// The resolve nodes of a metadata document, or none.
fn nodes_mut(metadata: &mut Value) -> impl Iterator<Item = &mut Value> {
    metadata
        .pointer_mut("/resolve/nodes")
        .and_then(Value::as_array_mut)
        .map(Vec::as_mut_slice)
        .unwrap_or_default()
        .iter_mut()
}
