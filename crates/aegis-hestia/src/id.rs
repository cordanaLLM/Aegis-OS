// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded, allocation-free identifiers.
//!
//! `docs/integration/stack.md` asks every crossing to carry a correlation
//! identifier, and the overlay registration also names the surface it is
//! about. Both are validated on construction and stored inline, so no decision
//! path allocates for one and no identifier can be silently truncated or
//! sanitised: a malformed input is refused.

use core::fmt;

#[path = "../../correlation_id.rs"]
mod correlation_id;

pub use correlation_id::{CorrelationId, IdError, MAX_CORRELATION_LEN};

/// Scalar upper bound, in bytes, on a surface identifier.
///
/// A surface identifier is parsed as a correlation identifier, so it shares
/// that bound.
pub const MAX_SURFACE_ID_LEN: usize = MAX_CORRELATION_LEN;

/// A validated surface identifier of at most [`MAX_SURFACE_ID_LEN`] bytes.
///
/// It names the picture-in-picture surface a registration is about, so a
/// result can be tied back to the request that asked for it. It is not a
/// Wayland object identifier: no Wayland connection exists here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(CorrelationId);

impl SurfaceId {
    /// Parses `raw` into a surface identifier.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] under the same rules as
    /// [`CorrelationId::parse`]: empty, over-long, or outside the permitted
    /// character set. The input is never truncated or sanitised.
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        CorrelationId::parse(raw).map(Self)
    }

    /// Returns the validated bytes, without the trailing padding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.0.as_bytes()
    }

    /// Returns the length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns `false`; a surface identifier is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Display for SurfaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, f)
    }
}
