// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The crate's membership, its dependencies and its own closure (D78), and
//! the effects its sources do not have.
//!
//! The crate reads nothing and starts nothing: the gate reads the counter and
//! hands over text. Its closure therefore resolves no socket, process,
//! eBPF or bus crate, and its sources name no file, process or sysfs call.

#[path = "../../dependency_closure.rs"]
mod dependency_closure;

#[path = "../../manifest_hygiene.rs"]
mod manifest_hygiene;

use manifest_hygiene::read;

/// This crate's package name, the start of its closure.
const PACKAGE: &str = "aegis-tellus-rapl";

/// Crates this closure must not resolve: a reader would need one of them.
const FORBIDDEN: [&str; 8] = [
    "aya", "zbus", "tokio", "nix", "procfs", "socket2", "rustix", "libc",
];

/// Identifiers that would perform an effect this crate does not have.
const WATCHED: [&str; 9] = [
    "std::fs",
    "OpenOptions",
    "File::",
    "read_to_string",
    "read_dir",
    "Command",
    "UnixStream",
    "TcpStream",
    "sudo",
];

// --- Positive -------------------------------------------------------------

/// Positive: the crate is a written-out member with a lock entry.
#[test]
fn the_crate_is_a_locked_member() {
    assert!(read("Cargo.toml").contains("\"crates/aegis-tellus-rapl\""));
    assert!(read("Cargo.lock").contains("name = \"aegis-tellus-rapl\""));
    let manifest = read("crates/aegis-tellus-rapl/Cargo.toml");
    assert!(manifest.contains("aegis-tellus = { path = \"../aegis-tellus\" }"));
    assert!(manifest.contains("[lints]\nworkspace = true"));
    assert!(manifest.contains("publish = false"));
}

/// Positive: the sweep reads the library and the binary.
#[test]
fn the_sweep_reads_every_source() {
    manifest_hygiene::assert_sweep_reads_sources(
        &manifest_hygiene::sources(),
        &["lib.rs", "cases.rs", "input.rs", "aegis-tellus-rapl.rs"],
    );
}

// --- Negative -------------------------------------------------------------

/// Negative (D78): the closure resolves no reader, socket or bus crate.
#[test]
fn the_closure_resolves_no_reader() -> Result<(), String> {
    let names = dependency_closure::assert_closure_forbids(PACKAGE, &FORBIDDEN)?;
    assert!(names.contains("aegis-tellus"));
    Ok(())
}

/// Negative: no source names a file, process, socket or sudo call.
#[test]
fn no_source_performs_a_read() {
    for (path, text) in manifest_hygiene::sources() {
        let found = manifest_hygiene::token_hits(&text, &WATCHED);
        assert!(found.is_empty(), "{} names {found:?}", path.display());
    }
}

/// Negative: the sweep would notice a planted read.
#[test]
fn the_sweep_detects_a_planted_read() {
    let planted = "let text = std::fs::read_to_string(path)?;";
    assert_eq!(
        manifest_hygiene::token_hits(planted, &WATCHED),
        vec!["std::fs", "read_to_string"]
    );
    assert_eq!(
        manifest_hygiene::token_hits("// std::fs is named in a comment", &WATCHED),
        [] as [&str; 0]
    );
    assert_eq!(
        manifest_hygiene::token_hits(" * std::fs in a block comment", &WATCHED),
        [] as [&str; 0]
    );
    assert_eq!(
        manifest_hygiene::token_hits("*slot = std::fs::metadata(path);", &WATCHED),
        vec!["std::fs"]
    );
}

/// Negative: the sweep would notice a missing source file.
#[test]
#[should_panic(expected = "missing nonexistent.rs")]
fn the_sweep_detects_a_missing_source() {
    manifest_hygiene::assert_sweep_reads_sources(&manifest_hygiene::sources(), &["nonexistent.rs"]);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: exactly one binary target, the one the gate starts.
#[test]
fn there_is_exactly_one_binary() {
    let manifest = read("crates/aegis-tellus-rapl/Cargo.toml");
    assert_eq!(manifest.matches("[[bin]]").count(), 1);
    assert!(manifest.contains("name = \"aegis-tellus-rapl\""));
    assert_eq!(FORBIDDEN.len(), 8);
    assert_eq!(WATCHED.len(), 9);
}
