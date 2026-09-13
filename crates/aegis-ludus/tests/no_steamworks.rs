// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Exit criterion 3: P11 Ludus integrates no platform SDK (D12, ADR-0002).
//!
//! The negative test is this file plus the dependency half in
//! `manifest_hygiene.rs`. It sweeps the crate's own sources for [`WATCHED`] --
//! the twelve spellings a vendored SDK, its binding generator or its shared
//! library would have to name -- and fails if one appears outside a comment.
//! The crate then builds without any of them, which is what `cargo build
//! --locked` shows.
//!
//! # What this is, and what it is not
//!
//! It is a regression gate over those twelve spellings. It is **not** a proof
//! that no proprietary code could ever arrive: a spelling that is not on the
//! list is not checked, and a dependency could in principle vendor something
//! this sweep cannot see. What makes the second half narrow in practice is the
//! dependency set, which `manifest_hygiene.rs` pins **closed**: the
//! `[dependencies]` table is asserted to be exactly four declarations -- one
//! path dependency and three inherited from the workspace table -- so a direct
//! dependency added under any name fails that test, including the renamed
//! shape `sdk = { package = "steamworks", ... }` that no spelling on either
//! list would catch.
//!
//! It also does **not** check the image. ADR-0002 excludes the SDK from the
//! Aegis image as well, and this repository builds no image: that gate is
//! blocked, and `make readiness` is where its state is recorded. Claiming an
//! image property from a crate test would be exactly the kind of unqualified
//! readiness claim the repository contract forbids.
//!
//! # The register's own names are deliberately not watched
//!
//! [`NOT_WATCHED`] holds the three identifiers the decision record has to spell
//! in library code -- the disposition type, the decision constant and the
//! architecture decision record's own filename -- because a register that could
//! not name what it excludes would not be a register. The watch list holds
//! crate, symbol and library spellings, which is the form a real dependency
//! takes; a test below shows the register names do appear in library code, so
//! the exclusion is deliberate rather than accidental.

mod common;

use common::Fallible;

/// The spellings a vendored SDK, its generator or its library would name.
const WATCHED: [&str; 12] = [
    "steamworks::",
    "steamworks_sys",
    "steamworks-rs",
    "steam_api",
    "SteamAPI_",
    "ISteamUser",
    "libsteam_api",
    "bindgen",
    "extern \"C\"",
    "#[link(",
    "#[repr(C)]",
    "build.rs",
];

/// Three identifiers the sweep deliberately does **not** watch.
///
/// The disposition type, the decision constant and the architecture decision
/// record's own filename are the register of the exclusion, and all three
/// appear in library code as data. A sweep that watched them would fail on the
/// record it is supposed to protect; what it watches instead is the machinery
/// that would link the thing. This is why the list holds `steamworks::` rather
/// than the bare word: the bare word is in the record's filename.
const NOT_WATCHED: [&str; 3] = [
    "SteamworksDisposition",
    "D12_STEAMWORKS_EXCLUSION",
    "0002-exclude-steamworks-from-image.md",
];

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
    assert!(sources.iter().any(|(name, _)| name == "launch.rs"));
    assert!(sources.iter().any(|(name, _)| name == "decision.rs"));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: no watched spelling appears in library code, so the crate builds
/// without any platform SDK.
#[test]
fn no_platform_sdk_spelling_appears_in_library_code() -> Fallible {
    let found = common::scan_sources(&common::source_root(), &WATCHED)?;
    assert!(
        found.is_empty(),
        "platform SDK spellings in library code: {found:?}"
    );
    Ok(())
}

/// Negative: the sweep would notice one, so a pass means something.
#[test]
fn the_sweep_detects_a_planted_spelling() {
    let planted = "    let client = steamworks::Client::init_app(480)?;";
    assert_eq!(
        common::scan_text("planted.rs", planted, &WATCHED),
        vec!["planted.rs:1: steamworks::".to_owned()]
    );
    let generator = "    let bindings = bindgen::Builder::default();";
    assert_eq!(
        common::scan_text("planted.rs", generator, &WATCHED),
        vec!["planted.rs:1: bindgen".to_owned()]
    );
    let foreign = "    unsafe extern \"C\" { fn SteamAPI_Init() -> bool; }";
    let hits = common::scan_text("planted.rs", foreign, &WATCHED);
    assert!(hits.contains(&"planted.rs:1: SteamAPI_".to_owned()));
    assert!(hits.contains(&"planted.rs:1: extern \"C\"".to_owned()));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the watched list is exactly its recorded length and has no
/// duplicate, so the count in the documentation is the count that is swept.
#[test]
fn the_watched_list_is_its_recorded_length() {
    assert_eq!(WATCHED.len(), 12);
    let mut names = WATCHED.to_vec();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 12);
    assert!(names.iter().all(|needle| !needle.is_empty()));
}

/// Boundary: the register's own names are outside the list, and the library
/// does carry them -- so watching them would fail the gate on the very record
/// that states the exclusion.
#[test]
fn the_register_names_are_deliberately_not_watched() -> Fallible {
    for name in NOT_WATCHED {
        assert!(
            !WATCHED.contains(&name),
            "{name} is the record of the exclusion, not the thing excluded"
        );
    }
    let found = common::scan_sources(&common::source_root(), &NOT_WATCHED)?;
    assert!(
        !found.is_empty(),
        "the register names must appear in library code as the record of D12"
    );
    Ok(())
}

/// Boundary: a comment naming a watched spelling is not a finding.
#[test]
fn a_comment_naming_a_spelling_is_not_a_finding() {
    let comment = "//! no steamworks:: crate, no bindgen build.rs, no libsteam_api";
    assert!(comment.trim().starts_with("//"));
    assert!(WATCHED.iter().any(|needle| comment.contains(needle)));
    assert!(common::scan_text("lib.rs", comment, &WATCHED).is_empty());
}
