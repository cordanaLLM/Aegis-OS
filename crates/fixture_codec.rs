// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Shared fixture parse and encode helpers for integration tests.
//!
//! One file, included by path into the integration test common modules of
//! `aegis-calliope`, `aegis-lictor` and `aegis-vesta`
//! (`#[path = "../../../fixture_codec.rs"] mod fixture_codec;`), as
//! `fixture_tamper.rs` is. It is not a crate and has no manifest: a test helper
//! made a workspace member would enter every dependency closure.
//!
//! The including module supplies `CORRELATION`, `CorrelationId`, `Label` and
//! `PayloadBuffer` from its own crate, so each suite parses and encodes with
//! that crate's contract types.

#![allow(dead_code)]

use super::{CORRELATION, CorrelationId, Label, PayloadBuffer};

/// Returns the fixture correlation identifier.
///
/// # Errors
///
/// Propagates [`CorrelationId::parse`].
pub fn correlation() -> Result<CorrelationId, Box<dyn std::error::Error>> {
    Ok(CorrelationId::parse(CORRELATION)?)
}

/// Returns a label, so a fixture that stops parsing fails the suite.
///
/// # Errors
///
/// Propagates [`Label::parse`].
pub fn label(raw: &str) -> Result<Label, Box<dyn std::error::Error>> {
    Ok(Label::parse(raw)?)
}

/// Renders `value` to owned text through its contract `encode`.
///
/// # Errors
///
/// Propagates the contract refusal.
pub fn encoded<T, E>(
    value: &T,
    encode: for<'b> fn(&T, &'b mut PayloadBuffer) -> Result<&'b str, E>,
) -> Result<String, Box<dyn std::error::Error>>
where
    E: std::error::Error + 'static,
{
    let mut buffer = PayloadBuffer::new();
    Ok(encode(value, &mut buffer)?.to_owned())
}
