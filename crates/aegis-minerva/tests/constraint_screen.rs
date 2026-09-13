// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E06-1: the constraint screen, which is not a solver.
//!
//! Positive: a script the screen does not reject is screened and counted.
//! Negative: a script carrying the recorded byte is rejected, and an empty or
//! ill-formed script never becomes a value. Boundary: a script at the byte
//! bound parses and one byte over does not.
//!
//! What none of this shows is that a screened script is *valid*. The screen
//! has no verdict to give, which is why [`ScreenOutcome`] has no such variant.

mod common;

use aegis_minerva::{
    BYTES_PER_CONSTRAINT, ConstraintScreen, ConstraintScript, MAX_SCRIPT_BYTES, MinervaError,
    REJECTED_BYTE, ScreenOutcome,
};

use common::{Fallible, SCRIPT, script};

// --- Positive -------------------------------------------------------------

/// Positive: a script parses, screens and counts its constraints.
#[test]
fn a_script_parses_screens_and_counts() -> Fallible {
    let parsed = script()?;
    assert_eq!(parsed.as_bytes(), SCRIPT.as_bytes());
    assert_eq!(parsed.len(), SCRIPT.len());
    assert!(!parsed.is_empty());

    let mut screen = ConstraintScreen::new();
    assert_eq!(screen.screened(), 0);
    assert_eq!(screen.screen(&parsed), ScreenOutcome::NotRejected);
    assert_eq!(screen.screened(), 1);
    assert_eq!(
        parsed.constraint_count(),
        u32::try_from(SCRIPT.len() / BYTES_PER_CONSTRAINT).unwrap_or(u32::MAX)
    );
    Ok(())
}

/// Positive: both outcomes have distinct recorded tags.
#[test]
fn both_outcomes_have_distinct_tags() {
    assert_eq!(ScreenOutcome::BOTH.len(), 2);
    assert_eq!(ScreenOutcome::RejectedByScreen.tag(), "rejected-by-screen");
    assert_eq!(ScreenOutcome::NotRejected.tag(), "not-rejected");
    assert_ne!(
        ScreenOutcome::RejectedByScreen.tag(),
        ScreenOutcome::NotRejected.tag()
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: a script carrying the recorded byte is rejected, and "not
/// rejected" is not an acceptance.
#[test]
fn a_script_carrying_the_recorded_byte_is_rejected() -> Fallible {
    let mut screen = ConstraintScreen::default();
    let negated = ConstraintScript::parse("x > 0 && !(y <= 100)")?;
    assert_eq!(screen.screen(&negated), ScreenOutcome::RejectedByScreen);
    assert!(ScreenOutcome::RejectedByScreen.is_rejected());
    assert!(
        !ScreenOutcome::NotRejected.is_rejected(),
        "the absence of a rejection is all the second outcome says"
    );
    assert_eq!(REJECTED_BYTE, b'!');
    assert_eq!(screen.screened(), 1, "a rejection is still a screening");
    Ok(())
}

/// Negative: an empty script and one outside printable ASCII are refused.
#[test]
fn a_malformed_script_never_becomes_a_value() {
    assert_eq!(
        ConstraintScript::parse(""),
        Err(MinervaError::ScriptOutOfRange {
            actual: 0,
            max: MAX_SCRIPT_BYTES
        })
    );
    assert_eq!(
        ConstraintScript::parse("x > 0\n&& y < 1"),
        Err(MinervaError::ScriptCharset)
    );
    assert_eq!(
        ConstraintScript::parse("x > \u{7f}"),
        Err(MinervaError::ScriptCharset)
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a script at the byte bound parses and one byte over does not.
#[test]
fn the_script_bound_is_closed_at_the_last_byte() -> Fallible {
    let at_bound = "x".repeat(MAX_SCRIPT_BYTES);
    assert_eq!(ConstraintScript::parse(&at_bound)?.len(), MAX_SCRIPT_BYTES);
    let over_bound = "x".repeat(MAX_SCRIPT_BYTES.saturating_add(1));
    assert_eq!(
        ConstraintScript::parse(&over_bound),
        Err(MinervaError::ScriptOutOfRange {
            actual: MAX_SCRIPT_BYTES.saturating_add(1),
            max: MAX_SCRIPT_BYTES,
        })
    );
    Ok(())
}

/// Boundary: the constraint count never drops below one, and grows by one per
/// recorded chunk.
#[test]
fn the_constraint_count_never_drops_below_one() -> Fallible {
    assert_eq!(ConstraintScript::parse("x")?.constraint_count(), 1);
    let one_chunk = "y".repeat(BYTES_PER_CONSTRAINT);
    assert_eq!(ConstraintScript::parse(&one_chunk)?.constraint_count(), 1);
    let two_chunks = "y".repeat(BYTES_PER_CONSTRAINT.saturating_mul(2));
    assert_eq!(ConstraintScript::parse(&two_chunks)?.constraint_count(), 2);
    let at_bound = "y".repeat(MAX_SCRIPT_BYTES);
    assert_eq!(
        ConstraintScript::parse(&at_bound)?.constraint_count(),
        u32::try_from(MAX_SCRIPT_BYTES / BYTES_PER_CONSTRAINT).unwrap_or(u32::MAX)
    );
    Ok(())
}
