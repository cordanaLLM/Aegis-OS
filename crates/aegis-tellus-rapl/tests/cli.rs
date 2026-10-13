// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The built `aegis-tellus-rapl` binary: its arguments, its output and its
//! exit codes (0 pass, 1 a case failed, 3 refused, 64 usage).

mod common;

#[path = "../../binary_transcript.rs"]
mod binary_transcript;

use binary_transcript::Transcript;
use serde_json::json;

use common::{Fallible, RECORDED_RUN, reading, recorded, set};

/// The binary Cargo built for this test run.
const BINARY: &str = env!("CARGO_BIN_EXE_aegis-tellus-rapl");

/// The usage line the binary prints on a refused command line.
const USAGE: &str = "usage: aegis-tellus-rapl run <readings-json>";

/// Runs the binary with `args`.
fn rapl(args: &[&str]) -> Result<Transcript, String> {
    binary_transcript::run(BINARY, args, &[])
}

/// Asserts that `args` is refused as a usage error before any document is read.
fn refused_as_usage(args: &[&str]) -> Fallible {
    let run = rapl(args)?;
    assert_eq!(run.code, Some(64), "{run:#?}");
    assert_eq!(run.stdout, format!("{USAGE}\n"), "{run:#?}");
    Ok(())
}

// --- Positive -------------------------------------------------------------

/// Positive: the recorded run's document passes every case and exits 0.
#[test]
fn the_recorded_run_passes_through_the_binary() -> Fallible {
    let run = rapl(&["run", RECORDED_RUN])?;
    assert_eq!(run.code, Some(0), "{run:#?}");
    assert_eq!(run.last_line(), "RESULT pass", "{run:#?}");
    assert!(
        run.stdout.contains("rapl/unprivileged-read-fails-closed"),
        "{run:#?}"
    );
    assert!(run.stderr.is_empty(), "{run:#?}");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a readable unprivileged counter fails one case and exits 1.
#[test]
fn a_failed_case_exits_one() -> Fallible {
    let mut document = recorded()?;
    set(&mut document, "unprivileged", reading("45514845753\n", 1));
    let run = rapl(&["run", &document.to_string()])?;
    assert_eq!(run.code, Some(1), "{run:#?}");
    assert_eq!(run.last_line(), "RESULT fail: 1 case(s) failed", "{run:#?}");
    Ok(())
}

/// Negative: a document naming no powercap zone is refused with exit 3 and
/// no case line.
#[test]
fn a_refused_document_exits_three() -> Fallible {
    let mut document = recorded()?;
    set(&mut document, "zone", json!("uncore\n"));
    let run = rapl(&["run", &document.to_string()])?;
    assert_eq!(run.code, Some(3), "{run:#?}");
    assert_eq!(run.stdout.lines().count(), 1, "{run:#?}");
    assert!(run.last_line().starts_with("RESULT error: "), "{run:#?}");
    Ok(())
}

/// Negative: text that is not JSON is refused with exit 3.
#[test]
fn text_that_is_not_json_exits_three() -> Fallible {
    let run = rapl(&["run", "{"])?;
    assert_eq!(run.code, Some(3), "{run:#?}");
    assert!(run.last_line().starts_with("RESULT error: "), "{run:#?}");
    Ok(())
}

/// Negative: an unknown mode is a usage error.
#[test]
fn an_unknown_mode_is_a_usage_error() -> Fallible {
    refused_as_usage(&["judge", RECORDED_RUN])
}

/// Negative: no arguments at all is a usage error.
#[test]
fn no_arguments_is_a_usage_error() -> Fallible {
    refused_as_usage(&[])
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the mode without its document, one argument short.
#[test]
fn a_missing_document_is_a_usage_error() -> Fallible {
    refused_as_usage(&["run"])
}

/// Boundary: one argument past the two the binary takes.
#[test]
fn an_extra_argument_is_a_usage_error() -> Fallible {
    refused_as_usage(&["run", RECORDED_RUN, "extra"])
}

/// Boundary: the mode is matched exactly, not by case.
#[test]
fn the_mode_is_case_sensitive() -> Fallible {
    refused_as_usage(&["RUN", RECORDED_RUN])
}

/// Boundary: an empty document is refused as a document, not as usage.
#[test]
fn an_empty_document_exits_three() -> Fallible {
    let run = rapl(&["run", ""])?;
    assert_eq!(run.code, Some(3), "{run:#?}");
    Ok(())
}
