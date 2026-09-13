// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded label, at its edges.
//!
//! Positive: a label inside its bound parses and reads back byte for byte.
//! Negative: an empty label and one outside the charset are refused rather
//! than truncated, and the refusal reaches [`MinervaError`] as itself.
//! Boundary: the last admissible byte is accepted and the first inadmissible
//! one is not.

mod common;

use aegis_minerva::{IdError, Label, MAX_LABEL_LEN, MinervaError};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: a label reads back byte for byte and renders as itself.
#[test]
fn a_label_reads_back_byte_for_byte() -> Fallible {
    let label = Label::parse("aegis-slm-math-v1")?;
    assert_eq!(label.as_bytes(), b"aegis-slm-math-v1");
    assert_eq!(label.len(), 17);
    assert!(!label.is_empty());
    assert_eq!(label.to_string(), "aegis-slm-math-v1");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an empty label and one outside the charset are refused.
#[test]
fn a_malformed_label_is_refused() {
    assert_eq!(Label::parse(""), Err(IdError::Empty));
    for bad in ["with space", "semi;colon", "quote\"mark", "tab\there"] {
        assert_eq!(Label::parse(bad), Err(IdError::Charset));
    }
}

/// Negative: a label refusal reaches the crate error as itself, so a caller
/// matching on [`MinervaError`] still sees which rule was broken.
#[test]
fn a_label_refusal_is_carried_into_the_crate_error() {
    let refused = MinervaError::from(IdError::Charset);
    assert_eq!(refused, MinervaError::Label(IdError::Charset));
    assert!(refused.to_string().contains("label was refused"));
    assert_ne!(
        refused,
        MinervaError::from(IdError::TooLong {
            max: MAX_LABEL_LEN,
            actual: MAX_LABEL_LEN.saturating_add(1),
        })
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the last admissible byte is accepted and the first inadmissible
/// one is refused, with the refusal naming both lengths.
#[test]
fn the_label_bound_is_closed_at_the_last_byte() -> Fallible {
    let at_bound = "a".repeat(MAX_LABEL_LEN);
    assert_eq!(Label::parse(&at_bound)?.len(), MAX_LABEL_LEN);
    let over_bound = "a".repeat(MAX_LABEL_LEN.saturating_add(1));
    assert_eq!(
        Label::parse(&over_bound),
        Err(IdError::TooLong {
            max: MAX_LABEL_LEN,
            actual: MAX_LABEL_LEN.saturating_add(1),
        })
    );
    assert_eq!(MAX_LABEL_LEN, 32, "the scaffold's own array width");
    Ok(())
}
