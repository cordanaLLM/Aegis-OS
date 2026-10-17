// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The built `aegis-scaena-display` binary: its mode dispatch and the exit
//! codes it reaches without the reference profile (2 skipped, 64 usage).
//!
//! Every run here has an empty environment apart from the loader path, so no
//! `WAYLAND_DISPLAY` or `XDG_RUNTIME_DIR` names a compositor: both modes stop
//! at the connection and skip before any render node is opened. The loader
//! path carries over because CI links the binary against the pinned libva
//! build outside the system library path (`.github/workflows/ci.yml`). A pass on the reference
//! profile stays the hand-run `make verify-display`, and so does telling the
//! two modes apart: both stop at the same connection here, so a swapped
//! dispatch passes this file.

mod common;

#[path = "../../binary_transcript.rs"]
mod binary_transcript;

use binary_transcript::Transcript;

use common::Fallible;

/// The binary Cargo built for this test run.
const BINARY: &str = env!("CARGO_BIN_EXE_aegis-scaena-display");

/// The usage line the binary prints on a refused command line.
const USAGE: &str = "usage: aegis-scaena-display run|subtitles|driver-probe";

/// Where the dynamic loader finds libva when it is not a system library.
const LOADER_PATH: &str = "LD_LIBRARY_PATH";

/// The banner each mode prints before it connects.
const BANNER: &str = "Display slice (M27, D79)";

/// Runs the binary with `args` and an environment that holds only the
/// loader path of this test run, empty when it has none.
fn display(args: &[&str]) -> Result<Transcript, String> {
    let loader = std::env::var(LOADER_PATH).unwrap_or_default();
    binary_transcript::run(BINARY, args, &[(LOADER_PATH, &loader)])
}

/// Asserts that `args` is refused as a usage error before any connection.
fn refused_as_usage(args: &[&str]) -> Fallible {
    let run = display(args)?;
    assert_eq!(run.code, Some(64), "{run:#?}");
    assert_eq!(run.stdout, format!("{USAGE}\n"), "{run:#?}");
    Ok(())
}

/// Asserts that `mode` reached its connection and skipped with exit 2.
fn skipped_without_a_compositor(mode: &str) -> Fallible {
    let run = display(&[mode])?;
    assert_eq!(run.code, Some(2), "{run:#?}");
    assert!(run.stdout.starts_with(BANNER), "{run:#?}");
    assert!(
        run.last_line()
            .starts_with("RESULT skip: no Wayland compositor could be reached"),
        "{run:#?}"
    );
    assert!(!run.stdout.contains("decode node"), "{run:#?}");
    Ok(())
}

// --- Positive -------------------------------------------------------------

/// Positive: `run` dispatches to the display run, which skips cleanly on a
/// host with no compositor.
#[test]
fn run_dispatches_and_skips_without_a_compositor() -> Fallible {
    skipped_without_a_compositor("run")
}

/// Positive: `subtitles` dispatches to the subtitle timing run, which skips
/// cleanly on a host with no compositor.
#[test]
fn subtitles_dispatches_and_skips_without_a_compositor() -> Fallible {
    let run = display(&["subtitles"])?;
    assert_eq!(run.code, Some(2), "{run:#?}");
    assert!(
        run.stdout.starts_with("Display slice (M32, D79)"),
        "{run:#?}"
    );
    assert!(
        run.last_line()
            .starts_with("RESULT skip: no Wayland compositor could be reached"),
        "{run:#?}"
    );
    assert!(!run.stdout.contains("decode node"), "{run:#?}");
    Ok(())
}

/// Positive: `driver-probe` dispatches to negative (a), which skips the same
/// way.
#[test]
fn driver_probe_dispatches_and_skips_without_a_compositor() -> Fallible {
    skipped_without_a_compositor("driver-probe")
}

// --- Negative -------------------------------------------------------------

/// Negative: no mode is a usage error.
#[test]
fn no_mode_is_a_usage_error() -> Fallible {
    refused_as_usage(&[])
}

/// Negative: a mode the binary does not know is a usage error.
#[test]
fn an_unknown_mode_is_a_usage_error() -> Fallible {
    refused_as_usage(&["present"])
}

// --- Boundary -------------------------------------------------------------

/// Boundary: an empty mode is not a mode.
#[test]
fn an_empty_mode_is_a_usage_error() -> Fallible {
    refused_as_usage(&[""])
}

/// Boundary: the mode is matched exactly, not by case or prefix.
#[test]
fn the_mode_is_matched_exactly() -> Fallible {
    refused_as_usage(&["Run"])?;
    refused_as_usage(&["driver"])
}
