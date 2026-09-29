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

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

use serde_json::Value;

use common::Fallible;

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

/// How long `cargo metadata` may run.
const METADATA_DEADLINE: Duration = Duration::from_secs(120);

/// How often the deadline loop polls the child.
const POLL: Duration = Duration::from_millis(25);

/// The most polls: the deadline divided by the poll interval, 120 s / 25 ms.
const MAX_POLLS: u32 = 4_800;

/// The workspace root, two directories above this crate's manifest.
fn workspace_root() -> Option<PathBuf> {
    let manifest = Path::new(env!("CARGO_MANIFEST_DIR"));
    Some(manifest.parent()?.parent()?.to_path_buf())
}

/// Reads a workspace file, or the empty string.
fn read(name: &str) -> String {
    workspace_root()
        .and_then(|root| std::fs::read_to_string(root.join(name)).ok())
        .unwrap_or_default()
}

/// Waits for `child` under `METADATA_DEADLINE`, killing it past the deadline.
fn wait(child: &mut std::process::Child) -> Result<std::process::ExitStatus, String> {
    let started = Instant::now();
    for _ in 0..=MAX_POLLS {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < METADATA_DEADLINE => std::thread::sleep(POLL),
            Ok(None) => break,
            Err(error) => return Err(format!("waiting for cargo metadata failed: {error}")),
        }
    }
    let killed = child.kill().and_then(|()| child.wait());
    Err(format!(
        "cargo metadata missed its {METADATA_DEADLINE:?} deadline; killed: {killed:?}"
    ))
}

/// Runs `cargo metadata` as D78 records it, output to a file so no pipe can
/// fill and stall the child, and returns the parsed document.
fn cargo_metadata() -> Result<Value, String> {
    let root = workspace_root().ok_or("no workspace root")?;
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR"));
    let (out, err) = (
        dir.join("forum-shell-metadata.json"),
        dir.join("forum-shell-metadata.err"),
    );
    let cargo = std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into());
    let files = (std::fs::File::create(&out), std::fs::File::create(&err));
    let (Ok(stdout), Ok(stderr)) = files else {
        return Err("cannot create the metadata output files".to_owned());
    };
    let mut child = Command::new(cargo)
        .args([
            "metadata",
            "--format-version",
            "1",
            "--locked",
            "--offline",
            "--all-features",
        ])
        .current_dir(root)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|error| format!("cargo metadata could not be spawned: {error}"))?;
    let status = wait(&mut child)?;
    if !status.success() {
        let text = std::fs::read_to_string(&err).unwrap_or_default();
        return Err(format!(
            "cargo metadata exited {status}; if offline, run `cargo fetch --locked`: {text}"
        ));
    }
    let text = std::fs::read_to_string(&out).map_err(|error| error.to_string())?;
    serde_json::from_str(&text)
        .map_err(|error| format!("cargo metadata output is not JSON: {error}"))
}

/// The package names in the closure of the one workspace member named
/// `package`, the member included.
fn closure(metadata: &Value, package: &str) -> Result<BTreeSet<String>, String> {
    let packages = metadata
        .get("packages")
        .and_then(Value::as_array)
        .ok_or("no packages")?;
    let names: BTreeMap<&str, &str> = packages
        .iter()
        .filter_map(|entry| Some((entry.get("id")?.as_str()?, entry.get("name")?.as_str()?)))
        .collect();
    let members = metadata
        .get("workspace_members")
        .and_then(Value::as_array)
        .ok_or("no members")?;
    let starts: Vec<&str> = members
        .iter()
        .filter_map(Value::as_str)
        .filter(|id| names.get(id) == Some(&package))
        .collect();
    let [start] = starts.as_slice() else {
        return Err(format!(
            "{} workspace members are named {package}",
            starts.len()
        ));
    };
    let nodes = metadata
        .get("resolve")
        .and_then(|resolve| resolve.get("nodes"))
        .and_then(Value::as_array)
        .ok_or("the resolve is null")?;
    let edges: BTreeMap<&str, Vec<&str>> = nodes
        .iter()
        .filter_map(|node| {
            let id = node.get("id")?.as_str()?;
            let deps = node.get("deps")?.as_array()?;
            Some((
                id,
                deps.iter()
                    .filter_map(|dep| dep.get("pkg")?.as_str())
                    .collect(),
            ))
        })
        .collect();
    let mut visited: BTreeSet<&str> = BTreeSet::from([*start]);
    let mut stack = vec![*start];
    for _ in 0..=nodes.len() {
        let Some(id) = stack.pop() else { break };
        for dep in edges.get(id).map(Vec::as_slice).unwrap_or_default() {
            if visited.insert(dep) {
                stack.push(dep);
            }
        }
    }
    Ok(visited
        .iter()
        .filter_map(|id| names.get(id))
        .map(|name| (*name).to_owned())
        .collect())
}

/// The forbidden names found in `names`, matched exactly.
fn forbidden_in(names: &BTreeSet<String>) -> Vec<&'static str> {
    FORBIDDEN
        .into_iter()
        .filter(|name| names.contains(*name))
        .collect()
}

/// Sets `key` on a JSON object; anything else is left as it is.
fn set(value: &mut Value, key: &str, new: Value) {
    if let Some(object) = value.as_object_mut() {
        object.insert(key.to_owned(), new);
    }
}

/// A small synthetic metadata document: this crate depending on `deps`.
fn synthetic(deps: &[&str]) -> Value {
    let mut packages = vec![serde_json::json!({ "id": "shell", "name": PACKAGE })];
    packages.extend(
        deps.iter()
            .map(|name| serde_json::json!({ "id": name, "name": name })),
    );
    let mut nodes = vec![serde_json::json!({
        "id": "shell",
        "deps": deps.iter().map(|name| serde_json::json!({ "pkg": name })).collect::<Vec<_>>(),
    })];
    nodes.extend(
        deps.iter()
            .map(|name| serde_json::json!({ "id": name, "deps": [] })),
    );
    serde_json::json!({
        "packages": packages,
        "workspace_members": ["shell"],
        "resolve": { "nodes": nodes },
    })
}

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
/// by reference here, and locked at that version.
#[test]
fn accesskit_is_declared_once_and_locked_at_the_admitted_version() {
    let root = read("Cargo.toml");
    assert_eq!(root.matches("accesskit").count(), 1);
    assert!(root.contains("accesskit = { version = \"0.25.1\", default-features = false }"));
    let manifest = read("crates/aegis-forum-shell/Cargo.toml");
    assert!(manifest.contains("accesskit.workspace = true"));
    let lock = read("Cargo.lock");
    assert!(lock.contains("name = \"accesskit\"\nversion = \"0.25.1\""));
    assert_eq!(lock.matches("name = \"accesskit\"").count(), 1);
}

/// Positive: the real closure, read through `cargo metadata`, holds the
/// admitted data model and the two producer crates, and nothing forbidden.
#[test]
fn the_closure_holds_no_toolkit_window_bus_adapter_or_runtime() -> Fallible {
    let names = closure(&cargo_metadata()?, PACKAGE)?;
    for expected in [PACKAGE, "accesskit", "aegis-justitia", "aegis-tellus"] {
        assert!(
            names.contains(expected),
            "{expected} is not in the closure: {names:?}"
        );
    }
    assert_eq!(forbidden_in(&names), Vec::<&str>::new(), "{names:?}");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a closure that reaches any forbidden crate, directly or
/// transitively, is caught, and each is named.
#[test]
fn a_forbidden_crate_in_the_closure_is_caught() -> Fallible {
    let names = closure(&synthetic(&FORBIDDEN), PACKAGE)?;
    assert_eq!(forbidden_in(&names), FORBIDDEN.to_vec());
    let mut deep = synthetic(&["accesskit"]);
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
    assert_eq!(forbidden_in(&closure(&deep, PACKAGE)?), vec!["tokio"]);
    Ok(())
}

/// Negative: a metadata document with no start node, two, or a null resolve
/// fails closed.
#[test]
fn a_malformed_metadata_document_fails_closed() {
    let mut none = synthetic(&[]);
    set(&mut none, "workspace_members", serde_json::json!([]));
    assert!(closure(&none, PACKAGE).is_err());
    let mut two = synthetic(&[]);
    set(
        &mut two,
        "workspace_members",
        serde_json::json!(["shell", "shell"]),
    );
    assert!(closure(&two, PACKAGE).is_err());
    let mut null = synthetic(&[]);
    set(&mut null, "resolve", Value::Null);
    assert!(closure(&null, PACKAGE).is_err());
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
        &synthetic(&["accesskit", "tokio-util", "wayland-client-sys"]),
        PACKAGE,
    )?;
    assert_eq!(forbidden_in(&names), Vec::<&str>::new());
    let mut outside = synthetic(&["accesskit"]);
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
        forbidden_in(&closure(&outside, PACKAGE)?),
        Vec::<&str>::new()
    );
    Ok(())
}
