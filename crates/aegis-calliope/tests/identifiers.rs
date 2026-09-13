// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded identifiers, at their edges.
//!
//! Positive: a well-formed identifier parses and renders back byte for byte.
//! Negative: an empty one, an over-long one and one carrying a byte outside
//! the permitted set are each refused with a distinct variant, and none is
//! truncated or sanitised. Boundary: the bound is exact -- the longest
//! admissible input parses and one byte more does not.

mod common;

use aegis_calliope::{
    CalliopeError, CorrelationId, IdError, Label, MAX_CORRELATION_LEN, MAX_LABEL_LEN,
};

// --- Positive -------------------------------------------------------------

/// Positive: an identifier renders back exactly what it was given.
#[test]
fn an_identifier_renders_back_what_it_was_given() -> Result<(), IdError> {
    let label = Label::parse(common::PLUGIN)?;
    assert_eq!(label.to_string(), common::PLUGIN);
    assert_eq!(label.as_bytes(), common::PLUGIN.as_bytes());
    assert_eq!(label.len(), common::PLUGIN.len());
    assert!(!label.is_empty());

    let correlation = CorrelationId::parse(common::CORRELATION)?;
    assert_eq!(correlation.to_string(), common::CORRELATION);
    assert_eq!(correlation.as_bytes(), common::CORRELATION.as_bytes());
    assert_eq!(correlation.len(), common::CORRELATION.len());
    assert!(!correlation.is_empty());
    Ok(())
}

/// Positive: the permitted character set admits the punctuation the recorded
/// names actually use.
#[test]
fn the_permitted_set_admits_the_recorded_punctuation() {
    for raw in [
        "aegis.p08-p04.dma-buf-stream.v1",
        "pipewire_rt_audio",
        "aegis:calliope@node/0",
        "a",
    ] {
        assert!(Label::parse(raw).is_ok(), "{raw} should parse");
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: an empty identifier is refused, and says which refusal it is.
#[test]
fn an_empty_identifier_is_refused() {
    assert_eq!(Label::parse(""), Err(IdError::Empty));
    assert_eq!(CorrelationId::parse(""), Err(IdError::Empty));
    assert_eq!(IdError::Empty.to_string(), "an identifier is empty");
}

/// Negative: a byte outside the permitted set is refused, never repaired.
#[test]
fn a_byte_outside_the_set_is_refused() {
    for raw in ["FabFilter Pro-Q 3", "reverb!", "caf\u{e9}", "a\nb", "a,b"] {
        assert_eq!(
            Label::parse(raw),
            Err(IdError::Charset),
            "{raw} should be refused"
        );
    }
    assert!(IdError::Charset.to_string().contains("character set"));
}

/// Negative: an identifier error becomes a crate error without losing which
/// refusal it was.
#[test]
fn an_identifier_error_converts_without_loss() {
    let converted: CalliopeError = IdError::Empty.into();
    assert_eq!(converted, CalliopeError::Identifier(IdError::Empty));
    assert!(converted.to_string().contains("an identifier is empty"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the label bound is exact.
#[test]
fn the_label_bound_is_exact() {
    let longest = "a".repeat(MAX_LABEL_LEN);
    assert_eq!(Label::parse(&longest).map(|l| l.len()), Ok(MAX_LABEL_LEN));
    let over = "a".repeat(MAX_LABEL_LEN.saturating_add(1));
    assert_eq!(
        Label::parse(&over),
        Err(IdError::TooLong {
            max: MAX_LABEL_LEN,
            actual: MAX_LABEL_LEN.saturating_add(1),
        })
    );
    assert_eq!(MAX_LABEL_LEN, 64);
}

/// Boundary: the correlation bound is exact, and an over-long input reports
/// the length it actually had rather than a truncated one.
#[test]
fn the_correlation_bound_is_exact() {
    let longest = "b".repeat(MAX_CORRELATION_LEN);
    assert_eq!(
        CorrelationId::parse(&longest).map(|c| c.len()),
        Ok(MAX_CORRELATION_LEN)
    );
    let over = "b".repeat(MAX_CORRELATION_LEN.saturating_mul(3));
    assert_eq!(
        CorrelationId::parse(&over),
        Err(IdError::TooLong {
            max: MAX_CORRELATION_LEN,
            actual: MAX_CORRELATION_LEN.saturating_mul(3),
        })
    );
    assert_eq!(MAX_CORRELATION_LEN, 64);
}
