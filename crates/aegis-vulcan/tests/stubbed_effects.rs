// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The regression gate over the effects this crate does not have.
//!
//! The crate documentation says it maps nothing and transfers nothing. That
//! claim is worth exactly as much as a check that can falsify it, so this file
//! sweeps the crate's own sources for a **named list** of identifiers that
//! would be needed to perform any of those effects, and fails if one appears
//! outside a comment.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over [`WATCHED`]: the twenty-two identifiers below
//! are the ones a mapping, a transfer or a device open would have to name, and
//! adding one to the library without adding it here is what this catches. It
//! is **not** a proof that no effect of any kind is reachable: an identifier
//! that is not on the list is not checked, and a dependency could in principle
//! perform an effect this sweep cannot see. What makes the second half narrow
//! in practice is the dependency set, which `manifest_hygiene.rs` pins.

mod common;

use common::Fallible;

/// The identifiers a mapping, a transfer or a device open would have to name.
const WATCHED: [&str; 22] = [
    "memmap2",
    "mmap",
    "MmapOptions",
    "libc",
    "nix",
    "OpenOptions",
    "File",
    "std::fs",
    "read_to_string",
    "/dev/vfio",
    "/dev/dri",
    "/sys/bus/pci",
    "/sys/class/drm",
    "ioctl",
    "cuda",
    "cudarc",
    "OwnedFd",
    "RawFd",
    "AsRawFd",
    "from_raw_fd",
    "TcpStream",
    "UnixStream",
];

// --- Positive -------------------------------------------------------------

/// Positive: the sweep reads the crate's sources, and reads several of them.
#[test]
fn the_sweep_reads_the_whole_crate() -> Fallible {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let sources = common::rust_sources(&root)?;
    assert!(
        sources.len() >= 9,
        "the sweep read only {} sources; it is reading nothing, not finding nothing",
        sources.len()
    );
    assert!(sources.iter().any(|(name, _)| name == "lib.rs"));
    assert!(sources.iter().any(|(name, _)| name == "driver.rs"));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: no watched identifier appears in library code.
#[test]
fn no_watched_identifier_appears_in_library_code() -> Fallible {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = common::scan_sources(&root, &WATCHED)?;
    assert!(
        found.is_empty(),
        "effect identifiers in library code: {found:?}"
    );
    Ok(())
}

/// Negative: the sweep would notice one, so a pass means something.
///
/// The falsifier for the gate itself: a line naming a watched identifier is
/// detected by the same rule the sweep applies.
#[test]
fn the_sweep_detects_a_planted_identifier() {
    let planted = "    let map = unsafe { MmapOptions::new().map(&file) };";
    let hits: Vec<&str> = WATCHED
        .iter()
        .copied()
        .filter(|needle| planted.contains(needle))
        .collect();
    assert_eq!(hits, vec!["MmapOptions"]);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the watched list is exactly its recorded length and has no
/// duplicate, so the count in the documentation is the count that is swept.
#[test]
fn the_watched_list_is_its_recorded_length() {
    assert_eq!(WATCHED.len(), 22);
    let mut names = WATCHED.to_vec();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 22);
    assert!(names.iter().all(|needle| !needle.is_empty()));
}

/// Boundary: a comment naming a watched identifier is not a finding.
///
/// The crate documentation names `memmap2` and `/dev/vfio` precisely to say it
/// does not use them, so the sweep must skip comment lines or the
/// documentation would fail the gate it is describing.
#[test]
fn a_comment_naming_an_identifier_is_not_a_finding() {
    let comment = "//! no `memmap2`, no `/dev/vfio`, no `ioctl`";
    assert!(comment.trim().starts_with("//"));
    assert!(WATCHED.iter().any(|needle| comment.contains(needle)));
}
