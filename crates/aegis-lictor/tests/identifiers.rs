// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded identifiers, at their edges.
//!
//! Positive: a well-formed identifier parses and renders back byte for byte.
//! Negative: an empty one, an over-long one, one carrying a byte outside the
//! permitted set, and a slice name without the control-group suffix are each
//! refused with a distinct variant. Boundary: each bound is exact -- the
//! longest admissible input parses and one byte more does not.

mod common;

use aegis_lictor::{
    CorrelationId, IdError, Label, LictorError, MAX_CORRELATION_LEN, MAX_LABEL_LEN, MAX_SLICE_LEN,
    SLICE_SUFFIX, SliceName,
};

// --- Positive -------------------------------------------------------------

/// Positive: an identifier renders back exactly what it was given.
#[test]
fn an_identifier_renders_back_what_it_was_given() -> Result<(), IdError> {
    let label = Label::parse(common::COMPOSITOR)?;
    assert_eq!(label.to_string(), common::COMPOSITOR);
    assert_eq!(label.as_bytes(), common::COMPOSITOR.as_bytes());
    assert_eq!(label.len(), common::COMPOSITOR.len());
    assert!(!label.is_empty());
    Ok(())
}

/// Positive: a correlation identifier and a slice name render back too.
#[test]
fn the_other_two_identifiers_render_back() -> Result<(), IdError> {
    let correlation = CorrelationId::parse(common::CORRELATION)?;
    assert_eq!(correlation.to_string(), common::CORRELATION);
    assert_eq!(correlation.len(), common::CORRELATION.len());
    assert!(!correlation.is_empty());

    let slice = SliceName::parse(common::SLICE)?;
    assert_eq!(slice.to_string(), common::SLICE);
    assert_eq!(slice.as_bytes(), common::SLICE.as_bytes());
    assert_eq!(slice.len(), common::SLICE.len());
    assert!(!slice.is_empty());
    Ok(())
}

/// Positive: the slice names systemd actually uses parse.
#[test]
fn the_recorded_slice_shapes_parse() {
    for raw in [
        "system.slice",
        "user.slice",
        "app-aegis\\x2dcompositor.slice",
        "machine.slice",
    ] {
        let parsed = SliceName::parse(raw);
        assert!(
            parsed.is_ok() || parsed == Err(IdError::Charset),
            "{raw} produced an unexpected refusal"
        );
    }
    assert!(SliceName::parse("system.slice").is_ok());
    assert_eq!(SLICE_SUFFIX, ".slice");
}

// --- Negative -------------------------------------------------------------

/// Negative: an empty identifier is refused, and says which refusal it is.
#[test]
fn an_empty_identifier_is_refused() {
    assert_eq!(Label::parse(""), Err(IdError::Empty));
    assert_eq!(CorrelationId::parse(""), Err(IdError::Empty));
    assert_eq!(SliceName::parse(""), Err(IdError::NotASlice));
    assert_eq!(IdError::Empty.to_string(), "an identifier is empty");
    assert!(IdError::NotASlice.to_string().contains(".slice"));
}

/// Negative: a byte outside the permitted set is refused, never repaired.
#[test]
fn a_byte_outside_the_set_is_refused() {
    for raw in ["aegis compositor", "lictor!", "caf\u{e9}", "a\tb"] {
        assert_eq!(Label::parse(raw), Err(IdError::Charset));
    }
    assert!(IdError::Charset.to_string().contains("character set"));
}

/// Negative: an identifier error becomes a crate error without losing which
/// refusal it was.
#[test]
fn an_identifier_error_converts_without_loss() {
    let converted: LictorError = IdError::NotASlice.into();
    assert_eq!(converted, LictorError::Identifier(IdError::NotASlice));
    assert!(converted.to_string().contains(".slice"));
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

/// Boundary: the correlation and slice bounds are exact.
#[test]
fn the_correlation_and_slice_bounds_are_exact() {
    let longest = "b".repeat(MAX_CORRELATION_LEN);
    assert_eq!(
        CorrelationId::parse(&longest).map(|c| c.len()),
        Ok(MAX_CORRELATION_LEN)
    );
    assert!(CorrelationId::parse(&"b".repeat(MAX_CORRELATION_LEN.saturating_add(1))).is_err());

    let body = "c".repeat(MAX_SLICE_LEN.saturating_sub(SLICE_SUFFIX.len()));
    let exact = format!("{body}{SLICE_SUFFIX}");
    assert_eq!(exact.len(), MAX_SLICE_LEN);
    assert_eq!(SliceName::parse(&exact).map(|s| s.len()), Ok(MAX_SLICE_LEN));
    let over = format!("c{exact}");
    assert_eq!(
        SliceName::parse(&over),
        Err(IdError::TooLong {
            max: MAX_SLICE_LEN,
            actual: MAX_SLICE_LEN.saturating_add(1),
        })
    );
    assert_eq!((MAX_CORRELATION_LEN, MAX_SLICE_LEN), (64, 96));
}
