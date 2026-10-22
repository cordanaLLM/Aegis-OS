// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The regression gate over the effects this crate does not have.
//!
//! The crate documentation says it starts nothing and isolates nothing. That
//! claim is worth exactly as much as a check that can falsify it, so this file
//! sweeps the crate's own sources for a **named list** of identifiers that
//! would be needed to perform any of those effects, and fails if one appears
//! outside a comment.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over [`WATCHED`]: the twenty-two identifiers below
//! are the ones a monitor, a `WebAssembly` engine, a ring or a socket would
//! have to name, and adding one to the library without adding it here is what
//! this catches. It is **not** a proof that no effect of any kind is
//! reachable: an identifier that is not on the list is not checked, and a
//! dependency could in principle perform an effect this sweep cannot see. What
//! makes the second half narrow in practice is the dependency set, which
//! `manifest_hygiene.rs` pins.

mod common;

use common::Fallible;

/// The identifiers a monitor, an engine, a ring or a socket would have to name.
const WATCHED: [&str; 22] = [
    "std::fs",
    "std::process",
    "Command::",
    "OpenOptions",
    "File::",
    "read_to_string",
    "create_dir",
    "kvm_ioctls",
    "/dev/kvm",
    "vmm_api",
    "jailer",
    "wasmtime",
    "wasmer",
    "wazero",
    "extism",
    "io_uring",
    "iou::",
    "vsock::",
    "libc",
    "RawFd",
    "TcpStream",
    "UnixStream",
];

/// Two identifiers the sweep deliberately does **not** watch.
///
/// `firecracker` and `qemu-microvm` are the wire tags of
/// [`VmmIdentity`](aegis_vesta::VmmIdentity), and decision D58 requires a
/// payload to name its monitor, so both appear in library code as data. A
/// sweep that watched them would fail on the field it is supposed to protect;
/// what it watches instead is the machinery that would talk to one.
const NOT_WATCHED: [&str; 2] = ["firecracker", "qemu-microvm"];

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
    assert!(sources.iter().any(|(name, _)| name == "decision.rs"));
    assert!(sources.iter().any(|(name, _)| name == "capsule_request.rs"));
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
    let planted = "    let socket = vsock::VsockStream::connect(cid)?;";
    let hits = common::scan_text("planted.rs", planted, &WATCHED);
    assert_eq!(hits, vec!["planted.rs:1: vsock::".to_owned()]);
    let second = "    let engine = wasmtime::Engine::default();";
    assert_eq!(
        common::scan_text("planted.rs", second, &WATCHED),
        vec!["planted.rs:1: wasmtime".to_owned()]
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the watched list is exactly its recorded length and has no
/// duplicate, so the count in the documentation is the count that is swept.
#[test]
fn the_watched_list_is_its_recorded_length() {
    common::assert_watched_list_exact(&WATCHED, 22);
}

/// Boundary: a comment naming a watched identifier is not a finding.
///
/// The crate documentation names a `WebAssembly` engine and `io_uring`
/// precisely to say it reaches neither, so the sweep must skip comment lines
/// or the documentation would fail the gate it is describing.
#[test]
fn a_comment_naming_an_identifier_is_not_a_finding() {
    let comment = "//! no wasmtime engine, no io_uring ring, no vsock:: socket";
    common::assert_comment_naming_identifier_is_not_finding(comment, &WATCHED);
}

/// Boundary: the two monitor tags are deliberately outside the list, so
/// watching them can never fail the gate on the field decision D58 requires.
///
/// Since 2026-10-16 the wire form of `VmmIdentity` lives in `saeptum-capsule`
/// (cut unit CUT-1632-03), whose own sweep asserts the tags appear there; this
/// crate re-exports the type and must not grow a second copy of the tags.
#[test]
fn the_monitor_tags_are_deliberately_not_watched() -> Fallible {
    for tag in NOT_WATCHED {
        assert!(
            !WATCHED.contains(&tag),
            "{tag} is data the payloads carry, not an effect"
        );
    }
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = common::scan_sources(&root, &NOT_WATCHED)?;
    assert!(
        found.is_empty(),
        "monitor tags re-declared here instead of in saeptum-capsule: {found:?}"
    );
    Ok(())
}
