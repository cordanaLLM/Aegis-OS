// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The regression gate over the effects this crate does not have.
//!
//! The crate documentation says it loads nothing, writes nothing and measures
//! nothing. That claim is worth exactly as much as a check that can falsify
//! it, so this file sweeps the crate's own sources for a **named list** of
//! identifiers that would be needed to perform any of those effects, and fails
//! if one appears outside a comment.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over [`WATCHED`]: the twenty-seven identifiers
//! below are the ones an eBPF loader, a control-group write, an affinity call,
//! a bus connection, an async runtime, a socket of any family or a clock read
//! would have to name, and adding one to the library without adding it here is
//! what this catches. It is **not** a proof that no effect of any kind is
//! reachable: an identifier that is not on the list is not checked, and a
//! dependency could in principle perform an effect this sweep cannot see. What
//! makes the second half narrow in practice is the dependency set, which
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
/// The identifiers a loader, a write, an affinity call, a bus, a socket or a
/// clock would have to name.
const WATCHED: [&str; 27] = [
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
    "aya::",
    "libbpf",
    "bpf_",
    "sched_setaffinity",
    "sched_setscheduler",
    "cgroup_write",
    "dbus",
    "zbus",
    "tokio",
    "libc",
    "RawFd",
    "std::net",
    "UnixListener",
    "UnixStream",
    "TcpListener",
    "TcpStream",
];

/// Three identifiers the sweep deliberately does **not** watch.
///
/// `scx_cake`, `sched_ext` and `kepler_power.bpf` are recorded names: the
/// first two identify the scheduler this crate models the arithmetic of, and
/// the third is the transport string the graph of record gives the P13 edge. A
/// sweep that watched them would fail on the documentation and the data it is
/// supposed to protect; what it watches instead is the machinery that would
/// load or attach one.
const NOT_WATCHED: [&str; 3] = ["scx_cake", "sched_ext", "kepler_power.bpf"];

// --- Positive -------------------------------------------------------------

/// Positive: the sweep reads the crate's sources, and reads several of them.
#[test]
fn the_sweep_reads_the_whole_crate() -> Fallible {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let sources = common::rust_sources(&root)?;
    assert!(
        sources.len() >= 12,
        "the sweep read only {} sources; it is reading nothing, not finding nothing",
        sources.len()
    );
    for expected in ["lib.rs", "tier.rs", "broker.rs", "probe.rs", "locality.rs"] {
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
    let planted = "    let mut loader = aya::Ebpf::load(bytes)?;";
    let hits = common::scan_text("planted.rs", planted, &WATCHED);
    assert!(hits.contains(&"planted.rs:1: aya::".to_owned()));
    let second = "    let started = Instant::now();";
    assert_eq!(
        common::scan_text("planted.rs", second, &WATCHED),
        vec!["planted.rs:1: Instant::".to_owned()]
    );
    let third = "    let connection = zbus::Connection::system().await?;";
    assert_eq!(
        common::scan_text("planted.rs", third, &WATCHED),
        vec!["planted.rs:1: zbus".to_owned()]
    );
    let socket = "    let _fd = std::net::TcpListener::bind(\"127.0.0.1:0\");";
    let socket_hits = common::scan_text("planted.rs", socket, &WATCHED);
    assert!(socket_hits.contains(&"planted.rs:1: std::net".to_owned()));
    assert!(socket_hits.contains(&"planted.rs:1: TcpListener".to_owned()));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the watched list is exactly its recorded length and has no
/// duplicate, so the count in the documentation is the count that is swept.
#[test]
fn the_watched_list_is_its_recorded_length() {
    assert_eq!(WATCHED.len(), 27);
    let mut names = WATCHED.to_vec();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 27);
    assert!(names.iter().all(|needle| !needle.is_empty()));
}

/// Boundary: a comment naming a watched identifier is not a finding.
#[test]
fn a_comment_naming_an_identifier_is_not_a_finding() {
    let comment = "//! no aya::, no libbpf, no tokio, no libc and no Instant:: read";
    assert!(comment.trim().starts_with("//"));
    assert!(WATCHED.iter().any(|needle| comment.contains(needle)));
    assert!(common::scan_text("lib.rs", comment, &WATCHED).is_empty());
}

/// Boundary: the recorded scheduler and transport names are deliberately
/// outside the list, and the library does carry them -- so watching them would
/// fail the gate on the very data the graph of record requires.
#[test]
fn the_recorded_names_are_deliberately_not_watched() -> Fallible {
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
        "the recorded names must appear in library code as EdgeId data"
    );
    Ok(())
}
