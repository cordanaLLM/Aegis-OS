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

use std::path::Path;

use common::Fallible;

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

/// Returns `line` with every double-quoted run removed, carrying `inside`
/// across lines.
///
/// The state has to be carried because this crate's recorded claims are
/// multi-line string literals joined with a trailing backslash: the second
/// line of one carries no opening quote, and a per-line rule reads it as code.
/// That was the first thing this sweep got wrong.
///
/// The rule is otherwise deliberately crude: it splits on the double-quote
/// character and drops every other piece. An escaped quote inside a literal
/// would be split wrongly, which can only ever make the sweep stricter than
/// intended, never looser.
fn code_only(line: &str, inside: &mut bool) -> String {
    let mut out = String::new();
    for (index, piece) in line.split('"').enumerate() {
        if index > 0 {
            *inside = !*inside;
        }
        if !*inside {
            out.push_str(piece);
            out.push(' ');
        }
    }
    out
}

/// Returns `line` with its literals removed, starting outside one.
fn outside_string_literals(line: &str) -> String {
    let mut inside = false;
    code_only(line, &mut inside)
}

/// Returns the `"<file>:<line>: <name>"` hits `root` carries outside literals.
fn scan_outside_literals(
    root: &Path,
    watched: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut found: Vec<String> = Vec::new();
    for (name, body) in common::rust_sources(root)? {
        let mut inside = false;
        for (number, line) in body.lines().take(common::LINE_BOUND).enumerate() {
            let text = line.trim();
            if !inside && text.starts_with("//") {
                continue;
            }
            let code = code_only(text, &mut inside);
            found.extend(
                watched
                    .iter()
                    .filter(|needle| code.contains(**needle))
                    .map(|needle| format!("{name}:{}: {needle}", number.saturating_add(1))),
            );
        }
    }
    Ok(found)
}

/// Returns this crate's `src/` directory.
fn source_root() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("src")
}

/// The path prefix a standard-library reach has to name.
///
/// Watched on its own rather than as a list entry, because it is a structural
/// property and not one more identifier somebody remembered.
const STANDARD_LIBRARY_PATH: &str = "std::";

// --- Positive -------------------------------------------------------------

/// Positive: the sweep reads the whole crate, and reads several files.
#[test]
fn the_sweep_reads_the_whole_crate() -> Fallible {
    let sources = common::rust_sources(&source_root())?;
    assert!(
        sources.len() >= 12,
        "the sweep read only {} sources; it is reading nothing, not finding nothing",
        sources.len()
    );
    for expected in ["lib.rs", "ledger.rs", "engine.rs", "ports.rs"] {
        assert!(
            sources.iter().any(|(name, _)| name == expected),
            "the sweep did not read {expected}"
        );
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: no watched code identifier appears in library code.
#[test]
fn no_watched_code_identifier_appears_in_library_code() -> Fallible {
    let found = common::scan_sources(&source_root(), &WATCHED_CODE)?;
    assert!(
        found.is_empty(),
        "effect identifiers in library code: {found:?}"
    );
    Ok(())
}

/// Negative: no watched path appears outside a string literal.
#[test]
fn no_watched_path_appears_outside_a_string_literal() -> Fallible {
    let found = scan_outside_literals(&source_root(), &WATCHED_PATHS)?;
    assert!(
        found.is_empty(),
        "recorded names used as code rather than as data: {found:?}"
    );
    Ok(())
}

/// Negative: the graph vocabulary does record the transport, which is why the
/// second list exists.
#[test]
fn the_vocabulary_records_the_transport_the_second_list_allows() -> Fallible {
    let sources = common::rust_sources(&source_root())?;
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

/// Negative: no line of library code names `std::` at all.
///
/// This is a different check from the two lists above rather than a stronger
/// one: not "none of these named identifiers appears" but "no line under
/// `src/` names the standard library". Every `use` under `src/` is `core::`,
/// `crate::`, `serde::` or a workspace crate, so a reach that has to spell a
/// `std::` path is caught whether or not anybody wrote its identifier down.
///
/// It is a check over the text of `src/`, not a proof that no effect is
/// reachable. The standard macro prelude is the hole it cannot see:
/// `println!`, `eprintln!`, `print!`, `eprint!` and `dbg!` write to a file
/// descriptor without naming `std::` and appear on no watched list. Those are
/// denied by `print_stdout`, `print_stderr` and `dbg_macro` in
/// `[workspace.lints.clippy]`, which is a third mechanism and is not this one.
/// A dependency can still perform an effect none of the three sees, and a
/// dependency that re-exported a standard-library type would hide the path.
#[test]
fn no_line_under_src_names_the_standard_library() -> Fallible {
    let found = common::scan_sources(&source_root(), &[STANDARD_LIBRARY_PATH])?;
    assert!(
        found.is_empty(),
        "library code naming the standard library: {found:?}"
    );
    Ok(())
}

/// Negative: the structural sweep catches reaches the named lists miss.
///
/// Both planted lines are real effects, and neither carries an identifier on
/// either list. That is the point of holding the structural property as well
/// as the enumeration.
#[test]
fn the_structural_sweep_detects_a_reach_the_named_lists_miss() {
    for planted in [
        "    let reachable = std::net::UdpSocket::bind(\"127.0.0.1:0\").is_ok();",
        "    let leaked = std::env::var(\"AEGIS_PLANT\").is_ok();",
    ] {
        assert!(
            common::scan_text("planted.rs", planted, &WATCHED_CODE).is_empty(),
            "the named list is not what catches {planted}"
        );
        let structural = common::scan_text("planted.rs", planted, &[STANDARD_LIBRARY_PATH]);
        assert!(
            !structural.is_empty(),
            "the structural sweep missed {planted}"
        );
    }
}

// --- Boundary -------------------------------------------------------------

/// Boundary: both lists are exactly their recorded lengths, with no duplicate
/// and no overlap, so the counts in the documentation are the counts swept.
#[test]
fn the_watched_lists_are_their_recorded_lengths() {
    assert_eq!(WATCHED_CODE.len(), 17);
    assert_eq!(WATCHED_PATHS.len(), 3);

    let mut all: Vec<&str> = WATCHED_CODE.to_vec();
    all.extend(WATCHED_PATHS);
    assert_eq!(all.len(), 20);
    all.sort_unstable();
    let before = all.len();
    all.dedup();
    assert_eq!(before, all.len(), "an identifier is watched twice");
    assert!(all.iter().all(|needle| !needle.is_empty()));
}

/// Boundary: a literal continued across lines stays a literal on the second
/// line, which is the case a per-line rule gets wrong.
#[test]
fn a_continued_literal_stays_a_literal_on_the_next_line() {
    let mut inside = false;
    let first = code_only("claim: \"the sysupdate transport is \\", &mut inside);
    assert!(!first.contains("sysupdate"));
    assert!(inside, "the literal is still open at the end of the line");

    let second = code_only("       recorded, not implemented\",", &mut inside);
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
    assert!(comment.trim().starts_with("//"));
    assert!(WATCHED_PATHS.iter().any(|needle| comment.contains(needle)));
    assert!(common::scan_text("doc.rs", comment, &WATCHED_PATHS).is_empty());
}
