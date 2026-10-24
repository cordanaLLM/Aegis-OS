// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: The bounded, allocation-free correlation identifier.
//!
//! One file, included by path into every crate whose edge contracts carry a
//! correlation identifier (`#[path = "../../correlation_id.rs"]`), so the type
//! has one definition rather than one per crate. Included by `aegis-hestia`
//! and `aegis-vulcan`. It is not a crate: a member enters every dependency
//! closure and carries a permanent name.

#![allow(dead_code)]

use core::fmt;

#[path = "bounded_id.rs"]
mod bounded_id;

use bounded_id::InlineId;

/// Scalar upper bound, in bytes, on a correlation identifier.
pub const MAX_CORRELATION_LEN: usize = 64;

/// Reasons a correlation identifier is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IdError {
    /// The identifier was empty.
    #[error("a correlation identifier is empty")]
    Empty,
    /// The identifier was longer than [`MAX_CORRELATION_LEN`].
    #[error("a correlation identifier of {actual} bytes exceeds the bound of {max}")]
    TooLong {
        /// The bound it passed.
        max: usize,
        /// The length observed.
        actual: usize,
    },
    /// The identifier carries a byte outside the permitted character set.
    #[error("a correlation identifier carries a byte outside the permitted character set")]
    Charset,
}

/// A validated correlation identifier of at most [`MAX_CORRELATION_LEN`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CorrelationId {
    inner: InlineId<MAX_CORRELATION_LEN>,
}

impl CorrelationId {
    /// Parses `raw` into a correlation identifier.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] when `raw` is empty, longer than
    /// [`MAX_CORRELATION_LEN`] bytes, or carries a byte outside the permitted
    /// character set. The input is never truncated or sanitised.
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        InlineId::parse(
            raw,
            || IdError::Empty,
            |actual| IdError::TooLong {
                max: MAX_CORRELATION_LEN,
                actual,
            },
            || IdError::Charset,
        )
        .map(|inner| Self { inner })
    }

    /// Returns the validated bytes, without the trailing padding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.inner.as_bytes()
    }

    /// Returns the length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `false`; an identifier is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
