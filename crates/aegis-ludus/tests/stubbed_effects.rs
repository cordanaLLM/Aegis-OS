// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The regression gate over the effects this crate does not have.
//!
//! The crate documentation says it links nothing and connects to nothing. That
//! claim is worth exactly as much as a check that can falsify it, so this file
//! sweeps the crate's own sources for a **named list** of identifiers that
//! would be needed to perform any of those effects, and fails if one appears
//! outside a comment.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over [`WATCHED`]: the twenty identifiers below are
//! the ones a socket, a compositor, a `TPM2` handle, a `FIDO2` device or a
//! process would have to name, and adding one to the library without adding it
//! here is what this catches. It is **not** a proof that no effect of any kind
//! is reachable: an identifier that is not on the list is not checked, and a
//! dependency could in principle perform an effect this sweep cannot see. What
//! makes the second half narrow in practice is the dependency set, which
//! `manifest_hygiene.rs` pins closed, name by name.
//!
//! The platform SDK has its own sweep in `no_steamworks.rs`, on its own list,
//! because that exclusion is an exit criterion in its own right.

mod common;

use common::Fallible;

/// The identifiers a socket, a device, a compositor or a process would name.
const WATCHED: [&str; 20] = [
    "std::fs",
    "std::process",
    "std::net",
    "Command::",
    "OpenOptions",
    "File::",
    "read_to_string",
    "UnixStream",
    "TcpStream",
    "/dev/tpm",
    "/dev/tpmrm",
    "tss_esapi",
    "ctap",
    "hidapi",
    "pipewire",
    "wayland",
    "dma_buf",
    "dbus",
    "zbus",
    "SystemTime",
];

/// Two paths the sweep deliberately does **not** watch.
///
/// The probe register records the commands that were run by hand on the
/// reference profile, and two of them name the `sysfs` path the platform
/// exposes its `TPM` at. Those strings are the dated reading; a sweep that
/// watched them would fail on the register it is supposed to protect. What it
/// watches instead is `/dev/tpm`, which is the device node a program would have
/// to open to do anything with the chip.
const NOT_WATCHED: [&str; 2] = ["/sys/class/tpm/", "lsusb"];

// --- Positive -------------------------------------------------------------

/// Positive: the sweep reads the crate's sources, and reads several of them.
#[test]
fn the_sweep_reads_the_whole_crate() -> Fallible {
    let sources = common::rust_sources(&common::source_root())?;
    assert!(
        sources.len() >= 10,
        "the sweep read only {} sources; it is reading nothing, not finding nothing",
        sources.len()
    );
    assert!(sources.iter().any(|(name, _)| name == "lib.rs"));
    assert!(sources.iter().any(|(name, _)| name == "probe.rs"));
    assert!(
        sources
            .iter()
            .any(|(name, _)| name == "transaction_receipt.rs")
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: no watched identifier appears in library code.
#[test]
fn no_watched_identifier_appears_in_library_code() -> Fallible {
    let found = common::scan_sources(&common::source_root(), &WATCHED)?;
    assert!(
        found.is_empty(),
        "effect identifiers in library code: {found:?}"
    );
    Ok(())
}

/// Negative: the sweep would notice one, so a pass means something.
#[test]
fn the_sweep_detects_a_planted_identifier() {
    let planted = "    let socket = UnixStream::connect(\"/tmp/discord-ipc-0\")?;";
    assert_eq!(
        common::scan_text("planted.rs", planted, &WATCHED),
        vec!["planted.rs:1: UnixStream".to_owned()]
    );
    let device = "    let handle = File::open(\"/dev/tpmrm0\")?;";
    let hits = common::scan_text("planted.rs", device, &WATCHED);
    assert!(hits.contains(&"planted.rs:1: File::".to_owned()));
    assert!(hits.contains(&"planted.rs:1: /dev/tpmrm".to_owned()));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the watched list is exactly its recorded length and has no
/// duplicate, so the count in the documentation is the count that is swept.
#[test]
fn the_watched_list_is_its_recorded_length() {
    assert_eq!(WATCHED.len(), 20);
    let mut names = WATCHED.to_vec();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 20);
    assert!(names.iter().all(|needle| !needle.is_empty()));
}

/// Boundary: a comment naming a watched identifier is not a finding.
#[test]
fn a_comment_naming_an_identifier_is_not_a_finding() {
    let comment = "//! no UnixStream socket, no pipewire stream, no /dev/tpm handle";
    assert!(comment.trim().starts_with("//"));
    assert!(WATCHED.iter().any(|needle| comment.contains(needle)));
    assert!(common::scan_text("lib.rs", comment, &WATCHED).is_empty());
}

/// Boundary: the two recorded probe commands are deliberately outside the
/// list, and the library does carry them -- so watching them would fail the
/// gate on the register that records what was probed.
#[test]
fn the_recorded_probe_commands_are_deliberately_not_watched() -> Fallible {
    for path in NOT_WATCHED {
        assert!(
            !WATCHED.contains(&path),
            "{path} is a recorded reading this crate must carry, not an effect"
        );
    }
    let found = common::scan_sources(&common::source_root(), &NOT_WATCHED)?;
    assert!(
        !found.is_empty(),
        "the recorded probe commands must appear in library code as the register"
    );
    assert!(WATCHED.contains(&"/dev/tpm"));
    Ok(())
}
