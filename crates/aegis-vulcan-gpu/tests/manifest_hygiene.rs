// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Manifest hygiene, workspace membership, and dependency closure verification
//! for `aegis-vulcan-gpu` (M25: the record half and the DMA-BUF half).

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// This crate's package name, the start of its closure.
const PACKAGE: &str = "aegis-vulcan-gpu";

/// Crates this closure must not resolve.
///
/// rustix left this list with the DMA-BUF half (ledger
/// `aegis-os-m25-unsafe-home`): it carries the ioctls and the mapping. The
/// closure also lists libc because it is read for every target platform and
/// rustix depends on libc off Linux.
const FORBIDDEN: [&str; 6] = ["aya", "zbus", "tokio", "nix", "procfs", "socket2"];

/// The one source file that may name `unsafe`: the DMA-BUF syscalls.
const UNSAFE_SOURCE: &str = "sys.rs";

/// Identifiers that would perform an effect this slice does not have.
/// The crate reads sysfs, stats /dev and opens `/dev/udmabuf` and render
/// nodes, but executes no child processes, opens no sockets and uses no sudo;
/// `unsafe` appears in [`UNSAFE_SOURCE`] only.
const WATCHED: [&str; 5] = ["Command", "UnixStream", "TcpStream", "sudo", "unsafe"];

/// Returns the crate directory.
fn crate_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_path_buf()
}

/// Reads a file relative to the workspace root.
fn read(name: &str) -> String {
    dependency_closure::workspace_root()
        .and_then(|root| std::fs::read_to_string(root.join(name)).ok())
        .unwrap_or_default()
}

/// Returns every `.rs` file under `src/`, bounded, with its text.
fn sources() -> Vec<(PathBuf, String)> {
    let mut found = Vec::new();
    let mut pending = vec![crate_dir().join("src")];
    for _ in 0..16 {
        let Some(directory) = pending.pop() else {
            break;
        };
        let Ok(entries) = std::fs::read_dir(&directory) else {
            continue;
        };
        for entry in entries.flatten().take(64) {
            let path = entry.path();
            if path.is_dir() {
                pending.push(path);
            } else if path.extension().is_some_and(|ext| ext == "rs") {
                let text = std::fs::read_to_string(&path).unwrap_or_default();
                found.push((path, text));
            }
        }
    }
    found
}

/// Returns the watched identifiers `text` names outside comments.
fn hits(text: &str) -> Vec<&'static str> {
    let code: Vec<&str> = text
        .lines()
        .map(str::trim)
        .filter(|line| !line.starts_with("//") && !line.starts_with("/*") && !line.starts_with('*'))
        .collect();
    WATCHED
        .into_iter()
        .filter(|needle| code.iter().any(|line| line.contains(needle)))
        .collect()
}

/// Parses key-value pairs of a simple TOML table.
fn extract_table(content: &str, header: &str) -> BTreeMap<String, String> {
    let mut in_table = false;
    let mut table = BTreeMap::new();
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with('#') || trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            if trimmed == header {
                in_table = true;
            } else if in_table {
                break;
            }
            continue;
        }
        if in_table && let Some((k, v)) = trimmed.split_once('=') {
            table.insert(k.trim().to_owned(), v.trim().to_owned());
        }
    }
    table
}

// --- Positive -------------------------------------------------------------

/// Positive: the crate is a written-out member with a lock entry.
#[test]
fn the_crate_is_a_locked_member() {
    assert!(read("Cargo.toml").contains("\"crates/aegis-vulcan-gpu\""));
    assert!(read("Cargo.lock").contains("name = \"aegis-vulcan-gpu\""));
    let manifest = read("crates/aegis-vulcan-gpu/Cargo.toml");
    assert!(manifest.contains("publish = false"));
    assert!(manifest.contains("[[bin]]"));
    assert!(manifest.contains("name = \"aegis-vulcan-gpu\""));
}

/// Positive: the sweep reads the library and all module sources.
#[test]
fn the_sweep_reads_every_source() {
    let names: Vec<String> = sources()
        .iter()
        .filter_map(|(path, _)| path.file_name()?.to_str().map(str::to_owned))
        .collect();
    for expected in [
        "lib.rs",
        "error.rs",
        "render_nodes.rs",
        "drm_sched.rs",
        "rebar.rs",
        "heap.rs",
        "reader.rs",
        "vfio.rs",
        "gpus.rs",
        "record.rs",
        "dmabuf.rs",
        "live.rs",
        "sys.rs",
        "aegis-vulcan-gpu.rs",
    ] {
        assert!(
            names.iter().any(|name| name == expected),
            "missing {expected} in {names:?}"
        );
    }
}

/// Positive: lints tables equal root workspace.lints except `unsafe_code` = "deny".
#[test]
fn the_lints_match_workspace_except_unsafe_code() {
    let root = read("Cargo.toml");
    let manifest = read("crates/aegis-vulcan-gpu/Cargo.toml");

    let mut root_rust = extract_table(&root, "[workspace.lints.rust]");
    let mut crate_rust = extract_table(&manifest, "[lints.rust]");

    assert_eq!(
        crate_rust.remove("unsafe_code"),
        Some("\"deny\"".to_owned()),
        "crate must deny unsafe_code"
    );
    assert_eq!(
        root_rust.remove("unsafe_code"),
        Some("\"forbid\"".to_owned()),
        "root workspace must forbid unsafe_code"
    );
    assert_eq!(
        crate_rust, root_rust,
        "remaining [lints.rust] entries must match workspace root"
    );

    let root_clippy = extract_table(&root, "[workspace.lints.clippy]");
    let crate_clippy = extract_table(&manifest, "[lints.clippy]");

    assert_eq!(
        crate_clippy, root_clippy,
        "[lints.clippy] must exactly match [workspace.lints.clippy]"
    );
    assert!(
        crate_clippy.len() >= 15,
        "expected clippy lint table to be populated"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative (D78): the closure resolves no reader, socket or bus crate.
#[test]
fn the_closure_resolves_no_forbidden_crate() -> Result<(), String> {
    let names = dependency_closure::own_closure(PACKAGE)?;
    let found = dependency_closure::hits(&names, &FORBIDDEN);
    assert!(
        found.is_empty(),
        "the closure of {PACKAGE} resolves forbidden crates {found:?}"
    );
    assert!(names.contains("serde"));
    assert!(names.contains("serde_json"));
    assert!(names.contains("thiserror"));
    assert!(names.contains("rustix"));
    assert!(names.contains("sha2"));
    Ok(())
}

/// Negative: no source names Command, sockets or sudo, and none but the
/// syscall module names `unsafe`.
#[test]
fn no_source_performs_banned_action_or_unsafe() {
    let mut unsafe_home_seen = false;
    for (path, text) in sources() {
        let mut found = hits(&text);
        if path.file_name().is_some_and(|name| name == UNSAFE_SOURCE) {
            unsafe_home_seen = found.contains(&"unsafe");
            found.retain(|needle| *needle != "unsafe");
        }
        assert!(found.is_empty(), "{} names {found:?}", path.display());
    }
    assert!(
        unsafe_home_seen,
        "the sweep did not read the unsafe syscall module"
    );
}

/// Negative: the sweep would notice a planted watched token.
#[test]
fn the_sweep_detects_a_planted_token() {
    let planted = "let stream = std::net::TcpStream::connect(addr);";
    assert_eq!(hits(planted), vec!["TcpStream"]);
    assert_eq!(hits("// TcpStream is named in a comment"), [] as [&str; 0]);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: exactly one binary target, the record CLI.
#[test]
fn there_is_exactly_one_binary() {
    let manifest = read("crates/aegis-vulcan-gpu/Cargo.toml");
    assert_eq!(manifest.matches("[[bin]]").count(), 1);
    assert!(manifest.contains("name = \"aegis-vulcan-gpu\""));
    assert_eq!(FORBIDDEN.len(), 6);
    assert_eq!(WATCHED.len(), 5);
}
