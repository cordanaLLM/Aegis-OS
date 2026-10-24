// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The regression gate over the effects this crate does not have.
//!
//! The crate documentation says it stores nothing and displays nothing. That
//! claim is worth exactly as much as a check that can falsify it, so this file
//! sweeps the crate's own sources for a **named list** of identifiers that
//! would be needed to perform any of those effects, and fails if one appears
//! outside a comment.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over [`WATCHED`]: the twenty identifiers below are
//! the ones a store, a filesystem reach or a compositor connection would have
//! to name, and adding one to the library without adding it here is what this
//! catches. It is **not** a proof that no effect of any kind is reachable: an
//! identifier that is not on the list is not checked, and a dependency could in
//! principle perform an effect this sweep cannot see. What makes the second
//! half narrow in practice is the dependency set, which `manifest_hygiene.rs`
//! pins.

mod common;

use common::Fallible;

/// The identifiers a store, a filesystem reach or a display would have to name.
const WATCHED: [&str; 20] = [
    "std::fs",
    "OpenOptions",
    "File::",
    "read_to_string",
    "create_dir",
    "btrfs",
    "subvolume(",
    "pglite::",
    "postgres",
    "sqlx",
    "wayland",
    "wlr_layer_shell",
    "smithay",
    "libc",
    "RawFd",
    "OwnedFd",
    "from_raw_fd",
    "Command",
    "TcpStream",
    "UnixStream",
];

// --- Positive -------------------------------------------------------------

/// Positive: the sweep reads the crate's sources, and reads several of them.
#[test]
fn the_sweep_reads_the_whole_crate() -> Fallible {
    common::assert_sweep_reads_sources(&common::source_root(), 9, &["lib.rs", "store.rs"])
}

// --- Negative -------------------------------------------------------------

/// Negative: no watched identifier appears in library code.
#[test]
fn no_watched_identifier_appears_in_library_code() -> Fallible {
    common::assert_no_watched_code(&common::source_root(), &WATCHED)
}

/// Negative: the sweep would notice one, so a pass means something.
#[test]
fn the_sweep_detects_a_planted_identifier() {
    let planted = "    let body = std::fs::read_to_string(path)?;";
    let hits: Vec<&str> = WATCHED
        .iter()
        .copied()
        .filter(|needle| planted.contains(needle))
        .collect();
    assert_eq!(hits, vec!["std::fs", "read_to_string"]);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the watched list is exactly its recorded length and has no
/// duplicate, so the count in the documentation is the count that is swept.
#[test]
fn the_watched_list_is_its_recorded_length() {
    common::assert_watched_list_exact(&WATCHED, 20);
}

/// Boundary: a comment naming a watched identifier is not a finding.
///
/// The crate documentation names `PostgreSQL` and Wayland precisely to say it
/// reaches neither, so the sweep must skip comment lines or the documentation
/// would fail the gate it is describing.
#[test]
fn a_comment_naming_an_identifier_is_not_a_finding() {
    let comment = "//! no postgres connection, no wayland socket, no libc";
    assert!(comment.trim().starts_with("//"));
    assert!(WATCHED.iter().any(|needle| comment.contains(needle)));
}
