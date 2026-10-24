// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: A private source cited by a recorded decision.
//!
//! One file, included by path into every crate whose decision register cites
//! private sources (`#[path = "../../citation.rs"]`), so the type has one
//! definition rather than one per crate. Included by `aegis-calliope`,
//! `aegis-compositor`, `aegis-hestia`, `aegis-janus-lifecycle` and
//! `aegis-vesta`. It is not a crate: a member enters every dependency
//! closure and carries a permanent name.

#![allow(dead_code)]

/// A private source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The export identifier of the private source.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that export's sha256.
    pub sha256_prefix: &'static str,
}
