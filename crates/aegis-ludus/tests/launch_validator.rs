// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E08-1: the bounded launch-argument validator.
//!
//! Positive: 64 arguments are accepted. Negative: 65 are rejected. Boundary:
//! the empty argument list is handled explicitly **and its authentication
//! outcome is recorded** -- which is the half the scaffold got wrong, because
//! its `token_found || !args.is_empty()` answers the empty line and the
//! tokenless line with the same kind of reasoning and one of the two answers is
//! wrong.

mod common;

use aegis_ludus::{
    AUTH_TOKEN_PREFIX, AuthenticationOutcome, LaunchArgument, LaunchCommandLine, LudusError,
    MAX_ARGUMENT_BYTES, MAX_LAUNCH_ARGS,
};

use common::{Fallible, sample_line, tokened_line, untokened_line};

// --- Positive -------------------------------------------------------------

/// Positive: the scaffold's own sample command line parses and authenticates.
#[test]
fn the_scaffold_sample_command_line_parses() -> Fallible {
    let line = sample_line()?;
    assert_eq!(line.len(), 3);
    assert!(!line.is_empty());
    assert_eq!(line.arguments().len(), 3);
    assert_eq!(
        line.arguments().first().map(LaunchArgument::as_bytes),
        Some(b"./game_binary".as_slice())
    );
    assert_eq!(line.authenticate(), AuthenticationOutcome::TokenPresent);
    assert!(line.authenticate().is_authenticated());
    Ok(())
}

/// Positive: exactly 64 arguments are accepted, with and without the token.
#[test]
fn sixty_four_arguments_are_accepted() -> Fallible {
    assert_eq!(MAX_LAUNCH_ARGS, 64);
    let plain = LaunchCommandLine::parse(&untokened_line(MAX_LAUNCH_ARGS))?;
    assert_eq!(plain.len(), MAX_LAUNCH_ARGS);
    assert_eq!(plain.authenticate(), AuthenticationOutcome::NoLaunchToken);

    let tokened = LaunchCommandLine::parse(&tokened_line(MAX_LAUNCH_ARGS))?;
    assert_eq!(tokened.len(), MAX_LAUNCH_ARGS);
    assert_eq!(tokened.authenticate(), AuthenticationOutcome::TokenPresent);
    Ok(())
}

/// Positive: an argument is validated by prefix, and only by prefix.
#[test]
fn the_token_is_recognised_by_its_recorded_prefix() -> Fallible {
    let carrier = LaunchArgument::parse("+connect_auth_token=anything-at-all")?;
    assert!(carrier.carries_launch_token());
    assert_eq!(carrier.len(), 35);

    let suffix = LaunchArgument::parse("x+connect_auth_token=anything")?;
    assert!(!suffix.carries_launch_token());
    assert_eq!(AUTH_TOKEN_PREFIX, "+connect_auth_token=");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: 65 arguments are rejected, and nothing is truncated into place.
#[test]
fn sixty_five_arguments_are_rejected() {
    let over = untokened_line(MAX_LAUNCH_ARGS + 1);
    assert_eq!(
        LaunchCommandLine::parse(&over),
        Err(LudusError::TooManyLaunchArguments {
            max: MAX_LAUNCH_ARGS,
            at_least: MAX_LAUNCH_ARGS + 1,
        })
    );
}

/// Negative: the refusal reports a lower bound, never an invented total.
///
/// The counter stops one past the bound, so a line of 500 arguments is refused
/// with the same `at_least` as a line of 65. A refusal that claimed 500 would
/// be claiming a count it never made.
#[test]
fn the_refusal_never_claims_a_count_it_did_not_make() {
    let far_over = untokened_line(500);
    assert_eq!(
        LaunchCommandLine::parse(&far_over),
        Err(LudusError::TooManyLaunchArguments {
            max: MAX_LAUNCH_ARGS,
            at_least: MAX_LAUNCH_ARGS + 1,
        })
    );
}

/// Negative: an over-long or non-printable argument is refused whole.
#[test]
fn a_malformed_argument_is_refused() {
    let long = "a".repeat(MAX_ARGUMENT_BYTES + 1);
    assert_eq!(
        LaunchArgument::parse(&long),
        Err(LudusError::LaunchArgumentOutOfRange {
            max: MAX_ARGUMENT_BYTES,
            actual: MAX_ARGUMENT_BYTES + 1,
        })
    );
    assert_eq!(
        LaunchArgument::parse(""),
        Err(LudusError::LaunchArgumentOutOfRange {
            max: MAX_ARGUMENT_BYTES,
            actual: 0,
        })
    );
    assert_eq!(
        LaunchArgument::parse("bell\u{7}"),
        Err(LudusError::LaunchArgumentCharset)
    );
    assert_eq!(
        LaunchArgument::parse("tab\there"),
        Err(LudusError::LaunchArgumentCharset)
    );
}

/// Negative: a line carrying a malformed argument produces no line at all.
#[test]
fn a_line_with_one_bad_argument_yields_no_line() {
    let long = "a".repeat(MAX_ARGUMENT_BYTES + 1);
    let line = format!("-ok {long} -also-ok");
    assert!(matches!(
        LaunchCommandLine::parse(&line),
        Err(LudusError::LaunchArgumentOutOfRange { .. })
    ));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the empty argument list is handled explicitly and its
/// authentication outcome is recorded.
///
/// Three spellings of "no arguments" -- the empty string, a string of spaces
/// and one of mixed ASCII whitespace -- all parse to the same line and all
/// report [`AuthenticationOutcome::EmptyCommandLine`], which is a value and not
/// the absence of one.
#[test]
fn the_empty_command_line_is_handled_explicitly() -> Fallible {
    for raw in ["", "   ", " \t \n "] {
        let line = LaunchCommandLine::parse(raw)?;
        assert_eq!(line.len(), 0);
        assert!(line.is_empty());
        assert!(line.arguments().is_empty());
        assert_eq!(line.authenticate(), AuthenticationOutcome::EmptyCommandLine);
        assert!(!line.authenticate().is_authenticated());
        assert_eq!(line.authenticate().tag(), "empty-command-line");
    }
    assert_eq!(LaunchCommandLine::empty().len(), 0);
    assert_eq!(
        LaunchCommandLine::default().authenticate(),
        AuthenticationOutcome::EmptyCommandLine
    );
    Ok(())
}

/// Boundary: this validator and the scaffold's rule differ on exactly one
/// outcome, and the difference is the defect the type removes.
///
/// The three outcomes are enumerated, so the claim is a check over all of them
/// and not over the two a test happened to pick.
#[test]
fn the_scaffold_verdict_disagrees_on_exactly_one_outcome() {
    let disagreements: Vec<&str> = AuthenticationOutcome::ALL
        .into_iter()
        .filter(|outcome| outcome.is_authenticated() != outcome.scaffold_verdict())
        .map(AuthenticationOutcome::tag)
        .collect();
    assert_eq!(disagreements, vec!["no-launch-token"]);
    assert_eq!(AuthenticationOutcome::ALL.len(), 3);

    assert!(AuthenticationOutcome::TokenPresent.is_authenticated());
    assert!(AuthenticationOutcome::TokenPresent.scaffold_verdict());
    assert!(!AuthenticationOutcome::NoLaunchToken.is_authenticated());
    assert!(AuthenticationOutcome::NoLaunchToken.scaffold_verdict());
    assert!(!AuthenticationOutcome::EmptyCommandLine.is_authenticated());
    assert!(!AuthenticationOutcome::EmptyCommandLine.scaffold_verdict());
}

/// Boundary: one argument of exactly the byte bound is accepted and the next
/// byte is refused.
#[test]
fn an_argument_at_the_byte_bound_is_accepted() -> Fallible {
    assert_eq!(MAX_ARGUMENT_BYTES, 64);
    let at_bound = "b".repeat(MAX_ARGUMENT_BYTES);
    let parsed = LaunchArgument::parse(&at_bound)?;
    assert_eq!(parsed.len(), MAX_ARGUMENT_BYTES);
    assert!(!parsed.is_empty());
    assert_eq!(parsed.as_bytes().len(), MAX_ARGUMENT_BYTES);

    let over = "b".repeat(MAX_ARGUMENT_BYTES + 1);
    assert!(LaunchArgument::parse(&over).is_err());
    Ok(())
}

/// Boundary: a single argument is a line, and the token may be the only one.
#[test]
fn one_argument_is_a_line() -> Fallible {
    let line = LaunchCommandLine::parse("+connect_auth_token=only")?;
    assert_eq!(line.len(), 1);
    assert_eq!(line.authenticate(), AuthenticationOutcome::TokenPresent);
    assert_eq!(
        LaunchCommandLine::parse("-only")?.authenticate(),
        AuthenticationOutcome::NoLaunchToken
    );
    Ok(())
}
