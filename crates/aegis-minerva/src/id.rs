// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded, allocation-free label.
//!
//! The scaffold (export-034 `213a95fc0d02`) carries an expert name and an
//! action name as `[u8; 32]` written from a padded string literal, which can
//! refuse nothing and cannot say how long the name actually is. [`Label`]
//! keeps the inline storage and adds the refusal, so a malformed name is
//! rejected rather than truncated or padded into place.
//!
//! There is deliberately no correlation-identifier type here. P09 threads its
//! payloads with [`Identity`](aegis_justitia::Identity), the identifier the M14
//! contracts already use, because a second identifier type with its own bound
//! would be a second thing to keep in agreement with P06.

use core::fmt;

#[path = "../../bounded_id.rs"]
mod bounded_id;

use bounded_id::InlineId;

/// Scalar upper bound, in bytes, on a label.
///
/// Thirty-two is the scaffold's own array width for `SlmExpert.name` and
/// `TrajectoryStep.action_name`, kept so a name that fits the imported shape
/// still fits this one.
pub const MAX_LABEL_LEN: usize = 32;

/// Reasons a label is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IdError {
    /// The label was empty.
    #[error("a label is empty")]
    Empty,
    /// The label was longer than [`MAX_LABEL_LEN`].
    #[error("a label of {actual} bytes exceeds the bound of {max}")]
    TooLong {
        /// The bound it passed.
        max: usize,
        /// The length observed.
        actual: usize,
    },
    /// The label carries a byte outside the permitted character set.
    #[error("a label carries a byte outside the permitted character set")]
    Charset,
}

/// A validated label of at most [`MAX_LABEL_LEN`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label {
    inner: InlineId<MAX_LABEL_LEN>,
}

impl Label {
    /// Parses `raw` into a label.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] when `raw` is empty, longer than [`MAX_LABEL_LEN`]
    /// bytes, or carries a byte outside the permitted character set. The input
    /// is never truncated or sanitised.
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        InlineId::parse(
            raw,
            || IdError::Empty,
            |actual| IdError::TooLong {
                max: MAX_LABEL_LEN,
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

    /// Returns `false`; a label is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
