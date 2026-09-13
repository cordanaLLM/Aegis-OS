// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The regression gate over the effects this crate does not have.
//!
//! The crate documentation says it composites nothing and connects to nothing.
//! That claim is worth exactly as much as a check that can falsify it, so this
//! file sweeps the crate's own sources for a **named list** of identifiers that
//! would be needed to perform any of those effects, and fails if one appears
//! outside a comment.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over [`WATCHED`]: the twenty-eight identifiers
//! below are the ones a compositor backend, a Wayland connection, a `DRM`
//! device, a mesh session, a socket of any family or a clock read would have
//! to name, and adding one to the library without adding it here is what this
//! catches. It is **not** a proof that no effect of any kind is reachable: an
//! identifier that is not on the list is not checked, and a dependency could
//! in principle perform an effect this sweep cannot see. What makes the second
//! half narrow in practice is the dependency set, which
//! `manifest_hygiene.rs` pins.

mod common;

use common::Fallible;

/// A note on the socket entries.
///
/// The M07 verification planted `std::net::TcpListener::bind` in library code
/// and watched it pass all three of this milestone's sweeps: the three lists
/// disagreed about sockets, and none of them watched `std::net` or a listener
/// on it. The five socket identifiers are now the same five on all three
/// lists, and `the_sweep_detects_a_planted_identifier` plants that exact line.
/// This is still a check over an enumeration: a socket opened through some
/// sixth spelling is not checked.
/// The identifiers a backend, a connection, a device, a session, a socket or a
/// clock would have to name.
const WATCHED: [&str; 28] = [
    "std::fs",
    "std::process",
    "Command::",
    "OpenOptions",
    "File::",
    "read_to_string",
    "std::thread",
    "thread::sleep",
    "Instant::",
    "SystemTime",
    "Duration::",
    "smithay::",
    "wlroots::",
    "wlroots_sys",
    "wayland_server",
    "wayland_client",
    "libinput::",
    "xkbcommon::",
    "drm::",
    "gbm::",
    "zenoh::",
    "libc",
    "RawFd",
    "std::net",
    "UnixListener",
    "UnixStream",
    "TcpListener",
    "TcpStream",
];

/// Three names the sweep deliberately does **not** watch.
///
/// `wlroots`, `smithay` and `zenoh` appear in this crate's library code as
/// recorded **data**: the first two are the register entries naming what
/// decision D08 rejected, and the third is the crate
/// [`ZENOH_CHECK`](aegis_compositor::ZENOH_CHECK) read from current upstream.
/// A sweep that watched the bare names would fail on the register it is
/// supposed to protect. What it watches instead is the spelling a *call* would
/// use: `smithay::`, `wlroots::`, `zenoh::` and the binding crates' module
/// paths. A bare name in a register string is a record; a module path is a
/// dependency.
const NOT_WATCHED: [&str; 3] = ["wlroots", "smithay", "zenoh"];

// --- Positive -------------------------------------------------------------

/// Positive: the sweep reads the crate's sources, and reads several of them.
#[test]
fn the_sweep_reads_the_whole_crate() -> Fallible {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let sources = common::rust_sources(&root)?;
    assert!(
        sources.len() >= 8,
        "the sweep read only {} sources; it is reading nothing, not finding nothing",
        sources.len()
    );
    for expected in [
        "lib.rs",
        "surface.rs",
        "mesh.rs",
        "pacing.rs",
        "decision.rs",
    ] {
        assert!(
            sources.iter().any(|(name, _)| name == expected),
            "the sweep did not read {expected}"
        );
    }
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
#[test]
fn the_sweep_detects_a_planted_identifier() {
    let planted = "    let listener = UnixListener::bind(&self.socket_path)?;";
    assert_eq!(
        common::scan_text("planted.rs", planted, &WATCHED),
        vec!["planted.rs:1: UnixListener".to_owned()]
    );
    let second = "    let session = zenoh::open(config).await?;";
    assert_eq!(
        common::scan_text("planted.rs", second, &WATCHED),
        vec!["planted.rs:1: zenoh::".to_owned()]
    );
    let third = "    let target = Duration::from_micros(100);";
    assert_eq!(
        common::scan_text("planted.rs", third, &WATCHED),
        vec!["planted.rs:1: Duration::".to_owned()]
    );
    let socket = "    let _fd = std::net::TcpListener::bind(\"127.0.0.1:0\");";
    let hits = common::scan_text("planted.rs", socket, &WATCHED);
    assert!(hits.contains(&"planted.rs:1: std::net".to_owned()));
    assert!(hits.contains(&"planted.rs:1: TcpListener".to_owned()));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the watched list is exactly its recorded length and has no
/// duplicate, so the count in the documentation is the count that is swept.
#[test]
fn the_watched_list_is_its_recorded_length() {
    assert_eq!(WATCHED.len(), 28);
    let mut names = WATCHED.to_vec();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 28);
    assert!(names.iter().all(|needle| !needle.is_empty()));
}

/// Boundary: a comment naming a watched identifier is not a finding.
#[test]
fn a_comment_naming_an_identifier_is_not_a_finding() {
    let comment = "//! no smithay::, no wlroots::, no zenoh:: session and no Duration:: sleep";
    assert!(comment.trim().starts_with("//"));
    assert!(WATCHED.iter().any(|needle| comment.contains(needle)));
    assert!(common::scan_text("lib.rs", comment, &WATCHED).is_empty());
}

/// Boundary: the recorded prose spellings are deliberately outside the list,
/// and at least one of them does appear in library code -- so watching them
/// would fail the gate on the register it is supposed to protect.
#[test]
fn the_recorded_prose_spellings_are_deliberately_not_watched() -> Fallible {
    for tag in NOT_WATCHED {
        assert!(
            !WATCHED.contains(&tag),
            "{tag} is recorded data this crate must carry, not an effect"
        );
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = common::scan_sources(&root, &NOT_WATCHED)?;
    assert!(
        !found.is_empty(),
        "at least one recorded prose spelling must appear in library code"
    );
    Ok(())
}
