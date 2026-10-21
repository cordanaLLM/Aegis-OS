// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Shared fixture payload tampering helper for integration tests.
//!
//! One file, included by path into the integration test common modules of
//! `aegis-calliope`, `aegis-compositor`, `aegis-hephaestus`, `aegis-lictor`,
//! `aegis-ludus`, `aegis-minerva` and `aegis-vesta`
//! (`#[path = "../../../fixture_tamper.rs"] mod fixture_tamper;`), extending D78 and D116
//! precedents to test helpers. It is not a crate and has no manifest: a test helper
//! made a workspace member would enter every dependency closure.
//!
//! Replaces fixture substrings to exercise deserialization and contract
//! verification errors without maintaining separate mutated fixture files.

#![allow(dead_code)]

/// Returns `text` with the first occurrence of `from` replaced by `to`.
///
/// # Errors
///
/// Returns a failure when `from` does not occur, so a tamper that stops
/// applying fails the suite instead of testing the untampered payload.
pub fn tamper(text: &str, from: &str, to: &str) -> Result<String, Box<dyn std::error::Error>> {
    if !text.contains(from) {
        return Err(format!("the fixture payload does not contain {from:?}").into());
    }
    Ok(text.replacen(from, to, 1))
}
