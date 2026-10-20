// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The regression gate over the effects this crate does not have.
//!
//! The crate documentation says it changes nothing: no file, no
//! `systemd-sysupdate` invocation, no D-Bus connection, no microVM, no
//! `AF_VSOCK` socket, no partition write and no reboot. The imported P16
//! scaffold does two of those -- it opens
//! `/var/log/aegis/checkpoint_ledger.jsonl` in append mode and reads the wall
//! clock -- so the claim is a real difference from the source, not a
//! restatement of it, and it is worth exactly as much as a check that can
//! falsify it.
//!
//! # Why there are two lists rather than one
//!
//! [`WATCHED_CODE`] is the set of identifiers that are a finding anywhere
//! outside a comment. [`WATCHED_PATHS`] is different: `sysupdate` and the
//! scaffold's ledger path are exactly what this crate *records* -- the graph
//! vocabulary names `systemd-sysupdate over local D-Bus` as the transport
//! REQ-P16-06 assigns the edge, in a string literal. A sweep that treated a
//! recorded transport as a performed call would fail on the vocabulary whose
//! whole purpose is to record it, so these are a finding only outside a string
//! literal. The effect that would use one is still caught, because performing
//! it needs an identifier from the first list.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over twenty named identifiers. It is **not** a
//! proof that no effect of any kind is reachable: an identifier that is not on
//! either list is not checked, and a dependency could in principle perform an
//! effect this sweep cannot see. What makes the second half narrow in practice
//! is the dependency set, which `manifest_hygiene.rs` pins -- and both
//! dependencies are workspace crates that carry sweeps of their own.
//!
//! # The second check
//!
//! Alongside the two enumerations, this file holds one check that is not an
//! enumeration at all: no line of code under `src/` names [`STANDARD_LIBRARY_PATH`].
//! A reach the lists never thought of -- a `UdpSocket`, a `std::env` read --
//! still has to spell a `std::` path, so this catches the shape of reach the
//! lists cannot. It is checked by `no_line_under_src_names_the_standard_library`.
//!
//! It is not a proof over a category either. The standard macro prelude writes
//! to a file descriptor without naming `std::`, so `println!`, `eprintln!`,
//! `print!`, `eprint!` and `dbg!` walk past this check and the lists alike.
//! `print_stdout`, `print_stderr` and `dbg_macro` are denied in
//! `[workspace.lints.clippy]`, and clippy is what refuses those.

mod common;

use common::{Fallible, outside_string_literals};

/// Identifiers that are a finding anywhere outside a comment.
const WATCHED_CODE: [&str; 17] = [
    "std::fs",
    "OpenOptions",
    "File::",
    "read_to_string",
    "write_all",
    "create_dir",
    "systemctl",
    "zbus",
    "dbus",
    "firecracker",
    "vsock",
    "libc",
    "Command",
    "TcpStream",
    "UnixStream",
    "SystemTime",
    "Instant",
];

/// Names that are a finding only outside a string literal.
///
/// Each is recorded as data by this crate -- the update mechanism the graph
/// names as the P16-to-P02 transport, the path the imported scaffold appended
/// to, the hypervisor interface REQ-P16-04 names -- so a literal is a citation
/// and anything else is a reach.
const WATCHED_PATHS: [&str; 3] = ["sysupdate", "checkpoint_ledger.jsonl", "kvm"];

// --- Positive -------------------------------------------------------------

/// Positive: athena sources; see [`common::assert_sweep_reads_sources`].
#[test]
fn the_sweep_reads_the_whole_crate() -> Fallible {
    common::assert_sweep_reads_sources(
        &common::source_root(),
        12,
        &["lib.rs", "ledger.rs", "engine.rs", "ports.rs"],
    )
}

// --- Negative -------------------------------------------------------------

/// Negative: athena code; see [`common::assert_no_watched_code`].
#[test]
fn no_watched_code_identifier_appears_in_library_code() -> Fallible {
    common::assert_no_watched_code(&common::source_root(), &WATCHED_CODE)
}

/// Negative: athena paths; see [`common::assert_no_watched_paths`].
#[test]
fn no_watched_path_appears_outside_a_string_literal() -> Fallible {
    common::assert_no_watched_paths(&common::source_root(), &WATCHED_PATHS, "names")
}

/// Negative: the graph vocabulary does record the transport, which is why the
/// second list exists.
#[test]
fn the_vocabulary_records_the_transport_the_second_list_allows() -> Fallible {
    let sources = common::rust_sources(&common::source_root())?;
    let graph = sources
        .iter()
        .find(|(name, _)| name == "graph.rs")
        .map(|(_, body)| body.clone())
        .unwrap_or_default();
    assert!(graph.contains("systemd-sysupdate over local D-Bus"));
    Ok(())
}

/// Negative: the path sweep would notice a recorded name used as code.
#[test]
fn the_path_sweep_detects_a_recorded_name_used_as_code() {
    let literal = "Self::TriggerSysupdateRollback => \"systemd-sysupdate over local D-Bus\",";
    assert!(
        !outside_string_literals(literal).contains("sysupdate"),
        "a recorded transport in a literal is data"
    );
    let code = "let status = sysupdate_client().deploy(slot)?;";
    assert!(outside_string_literals(code).contains("sysupdate"));
}

/// Negative: the sweep would notice the scaffold's own ledger write.
///
/// This is the exact line the port replaced, so a pass of the sweep above
/// means the scaffold's effect did not travel with its logic.
#[test]
fn the_sweep_detects_the_scaffold_ledger_write() {
    let planted = "    let mut file = OpenOptions::new().append(true).open(\"/var/log/aegis/checkpoint_ledger.jsonl\")?;";
    let hits = common::scan_text("planted.rs", planted, &WATCHED_CODE);
    assert!(
        hits.iter().any(|hit| hit.ends_with("OpenOptions")),
        "the sweep missed the scaffold's own file open in {hits:?}"
    );
}

/// Negative: the sweep would notice a wall-clock read.
///
/// The scaffold calls `SystemTime::now()` inside its ledger write. Here every
/// timestamp arrives as a parameter, which is what lets a chain be rebuilt
/// byte for byte.
#[test]
fn the_sweep_detects_a_wall_clock_read() {
    let planted = "    let stamp = SystemTime::now().duration_since(UNIX_EPOCH)?.as_secs();";
    let hits = common::scan_text("planted.rs", planted, &WATCHED_CODE);
    assert!(hits.iter().any(|hit| hit.ends_with("SystemTime")));
}

/// Negative: athena no-std; see [`common::assert_no_standard_library`].
#[test]
fn no_line_under_src_names_the_standard_library() -> Fallible {
    common::assert_no_standard_library(&common::source_root())
}

/// Negative: athena structural; see [`common::assert_structural_sweep_detects_reach`].
#[test]
fn the_structural_sweep_detects_a_reach_the_named_lists_miss() {
    common::assert_structural_sweep_detects_reach(&WATCHED_CODE);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: athena list bounds; see [`common::assert_watched_lists_distinct`].
#[test]
fn the_watched_lists_are_their_recorded_lengths() {
    common::assert_watched_lists_distinct(&WATCHED_CODE, &WATCHED_PATHS, 17, 3);
}

/// Boundary: a literal continued across lines stays a literal on the second
/// line, which is the case a per-line rule gets wrong.
#[test]
fn a_continued_literal_stays_a_literal_on_the_next_line() {
    let mut inside = false;
    let first = common::code_only("claim: \"the sysupdate transport is \\", &mut inside);
    assert!(!first.contains("sysupdate"));
    assert!(inside, "the literal is still open at the end of the line");

    let second = common::code_only("       recorded, not implemented\",", &mut inside);
    assert!(!second.contains("recorded"));
    assert!(!inside, "the literal closes on the second line");
}

/// Boundary: a comment naming a watched identifier is not a finding.
///
/// The crate documentation names the scaffold's ledger path precisely to say
/// it does not open it, so the sweep must skip comment lines or the
/// documentation would fail the gate it is describing.
#[test]
fn a_comment_naming_an_identifier_is_not_a_finding() {
    let comment = "//! nothing here opens checkpoint_ledger.jsonl or runs sysupdate";
    common::assert_comment_naming_identifier_is_not_finding(comment, &WATCHED_PATHS);
}
