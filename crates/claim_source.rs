// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: Where a recorded claim comes from.
//!
//! One file, included by path into every crate whose register cites claim
//! sources (`#[path = "../../claim_source.rs"]`), so the type has one
//! definition rather than one per crate. It is not a crate: a member enters
//! every dependency closure and carries a permanent name.

#![allow(dead_code)]

/// A source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClaimSource {
    /// The export identifier of the source.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that export's sha256.
    pub sha256_prefix: &'static str,
}
