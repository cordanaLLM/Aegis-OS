// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The regression gate over the effects this crate does not have.
//!
//! The crate documentation says it measures nothing: no RAPL read, no sysfs
//! access, no eBPF program, no cgroup read, no D-Bus connection, no thread and
//! no sleep. That claim is worth exactly as much as a check that can falsify
//! it, so this file sweeps the crate's own sources for **two named lists** of
//! identifiers that would be needed to perform any of those effects, and fails
//! if one appears where it would mean the effect exists.
//!
//! # Why there are two lists rather than one
//!
//! [`WATCHED_CODE`] is the set of identifiers that are a finding anywhere
//! outside a comment: an opened file, a spawned process, a socket, a loaded
//! eBPF program, a bus connection, a sleep or a wall-clock read. Nothing in
//! this crate may name any of them.
//!
//! [`WATCHED_PATHS`] is different. `/sys/class/powercap`, `energy_uj` and
//! `kepler_power` are exactly what [`aegis_tellus::register`] and the graph
//! vocabulary *record*, in string literals, in order to state what the M21
//! reader will read and what the graph names as a transport. A sweep that
//! treated a recorded path as a performed read would fail on the register
//! whose whole purpose is to record it, so these are a finding only outside a
//! string literal. The effect that would use one is still caught, because
//! performing it needs an identifier from the first list.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over twenty-four named identifiers. It is **not** a
//! proof that no effect of any kind is reachable: an identifier that is not on
//! either list is not checked, and a dependency could in principle perform an
//! effect this sweep cannot see. What makes the second half narrow in practice
//! is the dependency set, which `manifest_hygiene.rs` pins.
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
const WATCHED_CODE: [&str; 19] = [
    "std::fs",
    "OpenOptions",
    "File::",
    "read_to_string",
    "read_dir",
    "msr",
    "perf_event",
    "aya::",
    "libbpf",
    "bpf_map",
    "zbus",
    "dbus",
    "libc",
    "Command",
    "TcpStream",
    "UnixStream",
    "thread::sleep",
    "SystemTime",
    "Instant",
];

/// Names that are a finding only outside a string literal.
///
/// Each of these is recorded as data by this crate -- the powercap zone M21's
/// gate reads, the attribute it reads it from, the probe the
/// graph names as a transport -- so a literal is a citation and anything else
/// is a reach.
const WATCHED_PATHS: [&str; 5] = ["/sys/", "/proc/", "powercap", "energy_uj", "kepler_power"];

// --- Positive -------------------------------------------------------------

/// Positive: tellus sources; see [`common::assert_sweep_reads_sources`].
#[test]
fn the_sweep_reads_the_whole_crate() -> Fallible {
    common::assert_sweep_reads_sources(
        &common::source_root(),
        11,
        &["lib.rs", "sci.rs", "power.rs", "register.rs"],
    )
}

/// Positive: the register does record the paths, which is why the second list
/// exists.
///
/// If this stopped being true the second list would be dead weight, and the
/// reader of `stubbed_effects.rs` would be left wondering why it was split.
#[test]
fn the_register_records_the_paths_the_second_list_allows() -> Fallible {
    let sources = common::rust_sources(&common::source_root())?;
    let register = sources
        .iter()
        .find(|(name, _)| name == "register.rs")
        .map(|(_, body)| body.clone())
        .unwrap_or_default();
    assert!(register.contains("/sys/class/powercap"));
    assert!(register.contains("energy_uj"));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: tellus code; see [`common::assert_no_watched_code`].
#[test]
fn no_watched_code_identifier_appears_in_library_code() -> Fallible {
    common::assert_no_watched_code(&common::source_root(), &WATCHED_CODE)
}

/// Negative: tellus paths; see [`common::assert_no_watched_paths`].
#[test]
fn no_watched_path_appears_outside_a_string_literal() -> Fallible {
    common::assert_no_watched_paths(&common::source_root(), &WATCHED_PATHS, "paths")
}

/// Negative: the sweep would notice a planted read, so a pass means something.
#[test]
fn the_sweep_detects_a_planted_counter_read() {
    let planted = "    let raw = std::fs::read_to_string(\"/sys/class/powercap/x/energy_uj\")?;";
    let hits = common::scan_text("planted.rs", planted, &WATCHED_CODE);
    for needle in ["std::fs", "read_to_string"] {
        assert!(
            hits.iter().any(|hit| hit.ends_with(needle)),
            "the sweep missed {needle} in {hits:?}"
        );
    }
}

/// Negative: the path sweep would notice a planted constant, so its narrower
/// rule still catches the shape it is meant to catch.
#[test]
fn the_path_sweep_detects_a_planted_path_used_as_code() {
    let literal = "const ZONE: &str = \"/sys/class/powercap/intel-rapl:0\";";
    assert!(
        !outside_string_literals(literal).contains("/sys/"),
        "a recorded path in a literal is data"
    );

    let code = "let path = ZONE_ROOT.join(powercap_dir()).join(energy_uj());";
    let stripped = outside_string_literals(code);
    assert!(stripped.contains("powercap"));
    assert!(stripped.contains("energy_uj"));
}

/// Negative: tellus no-std; see [`common::assert_no_standard_library`].
#[test]
fn no_line_under_src_names_the_standard_library() -> Fallible {
    common::assert_no_standard_library(&common::source_root())
}

/// Negative: tellus structural; see [`common::assert_structural_sweep_detects_reach`].
#[test]
fn the_structural_sweep_detects_a_reach_the_named_lists_miss() {
    common::assert_structural_sweep_detects_reach(&WATCHED_CODE);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: tellus list bounds; see [`common::assert_watched_lists_distinct`].
#[test]
fn the_watched_lists_are_their_recorded_lengths() {
    common::assert_watched_lists_distinct(&WATCHED_CODE, &WATCHED_PATHS, 19, 5);
}

/// Boundary: a comment naming a watched identifier is not a finding.
///
/// The crate documentation names `/sys/class/powercap` and `energy_uj`
/// precisely to say it reads neither, so the sweep must skip comment lines or
/// the documentation would fail the gate it is describing.
#[test]
fn a_comment_naming_an_identifier_is_not_a_finding() {
    let comment = "//! nothing here opens /sys/class/powercap or calls std::fs";
    common::assert_comment_naming_identifier_is_not_finding(comment, &WATCHED_CODE);
}

/// Boundary: the literal-stripping rule keeps code on either side of a
/// literal, so a reach next to a citation is still a finding.
#[test]
fn stripping_a_literal_keeps_the_code_on_either_side() {
    let mixed = "let raw = libc::open(\"/sys/class/powercap\", 0);";
    let stripped = outside_string_literals(mixed);
    assert!(stripped.contains("libc"));
    assert!(!stripped.contains("/sys/"));
    assert_eq!(outside_string_literals("\"only a literal\""), "  ");
    assert_eq!(
        outside_string_literals("no literal here"),
        "no literal here "
    );
}

/// Boundary: a literal continued across lines stays a literal on the second
/// line, which is the case a per-line rule gets wrong.
#[test]
fn a_continued_literal_stays_a_literal_on_the_next_line() {
    let mut inside = false;
    let first = common::code_only("citation: \"ls -d /sys/class/powercap/*; \\", &mut inside);
    assert!(!first.contains("/sys/"));
    assert!(inside, "the literal is still open at the end of the line");

    let second = common::code_only(
        "           ls -l /sys/class/powercap/*/energy_uj\",",
        &mut inside,
    );
    assert!(
        !second.contains("energy_uj"),
        "the continuation line is still inside the literal"
    );
    assert!(!inside, "the literal closes on the second line");

    let after = common::code_only("let reach = energy_uj();", &mut inside);
    assert!(
        after.contains("energy_uj"),
        "code after the literal is code"
    );
}
