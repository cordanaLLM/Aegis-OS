// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded surface label, at its edges.
//!
//! The key expression has its own file, `mesh_tiers.rs`, because its rules are
//! part of the Tier-2 acceptance. This file covers the label the registry
//! stores.
//!
//! Positive: a well-formed label parses and renders back byte for byte.
//! Negative: an empty one, an over-long one and one carrying a byte outside
//! the permitted set are each refused, and the scaffold's own surface titles
//! are refused rather than repaired. Boundary: the bound is exact.

mod common;

use aegis_compositor::{CompositorError, IdError, Label, MAX_LABEL_LEN};

// --- Positive -------------------------------------------------------------

/// Positive: a label renders back exactly what it was given.
#[test]
fn a_label_renders_back_what_it_was_given() -> Result<(), IdError> {
    let label = Label::parse(common::SHELL)?;
    assert_eq!(label.to_string(), common::SHELL);
    assert_eq!(label.as_bytes(), common::SHELL.as_bytes());
    assert_eq!(label.len(), common::SHELL.len());
    assert!(!label.is_empty());
    Ok(())
}

/// Positive: the punctuation the recorded names use is admitted.
#[test]
fn the_permitted_set_admits_the_recorded_punctuation() {
    for raw in [
        "hestia-pip-media-player",
        "aegis_compositor",
        "aegis:surface@1/top",
        "a",
    ] {
        assert!(Label::parse(raw).is_ok(), "{raw} should parse");
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the scaffold's own surface titles are refused rather than
/// repaired.
///
/// `"Forum Desktop Shell"` and `"Hestia PiP Media Player"` are `String`s in
/// the scaffold and nothing can reject them there; here the space is
/// [`IdError::Charset`].
#[test]
fn the_scaffold_surface_titles_are_refused() {
    assert_eq!(Label::parse("Forum Desktop Shell"), Err(IdError::Charset));
    assert_eq!(
        Label::parse("Hestia PiP Media Player"),
        Err(IdError::Charset)
    );
    assert_eq!(Label::parse(""), Err(IdError::Empty));
}

/// Negative: a byte outside the permitted set is refused, never repaired.
#[test]
fn a_byte_outside_the_set_is_refused() {
    for raw in ["shell!", "caf\u{e9}", "a\nb", "a,b"] {
        assert_eq!(Label::parse(raw), Err(IdError::Charset), "{raw}");
    }
    assert!(IdError::Charset.to_string().contains("character set"));
    assert_eq!(IdError::Empty.to_string(), "an identifier is empty");
}

/// Negative: an identifier error becomes a crate error without losing which
/// refusal it was.
#[test]
fn an_identifier_error_converts_without_loss() {
    let converted: CompositorError = IdError::Empty.into();
    assert_eq!(converted, CompositorError::Identifier(IdError::Empty));
    assert!(converted.to_string().contains("an identifier is empty"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the label bound is exact.
#[test]
fn the_label_bound_is_exact() {
    let longest = "a".repeat(MAX_LABEL_LEN);
    assert_eq!(Label::parse(&longest).map(|l| l.len()), Ok(MAX_LABEL_LEN));
    assert_eq!(
        Label::parse(&"a".repeat(MAX_LABEL_LEN.saturating_add(1))),
        Err(IdError::TooLong {
            max: MAX_LABEL_LEN,
            actual: MAX_LABEL_LEN.saturating_add(1),
        })
    );
    assert_eq!(MAX_LABEL_LEN, 32);
}

/// Boundary: an over-long input reports the length it actually had rather than
/// a truncated one, so a caller can tell how far past the bound it went.
#[test]
fn an_over_long_input_reports_its_real_length() {
    let far_over = "a".repeat(MAX_LABEL_LEN.saturating_mul(4));
    assert_eq!(
        Label::parse(&far_over),
        Err(IdError::TooLong {
            max: MAX_LABEL_LEN,
            actual: MAX_LABEL_LEN.saturating_mul(4),
        })
    );
}
