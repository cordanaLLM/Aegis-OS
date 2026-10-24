// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded, allocation-free identifiers P07 carries.
//!
//! `docs/integration/stack.md` asks every crossing to carry a correlation
//! identifier, and the two inbound P07 payloads also name the process and the
//! control group they are about. Each is validated on construction and stored
//! inline, so no decision path allocates for one and none can be silently
//! truncated or sanitised: a malformed input is refused.
//!
//! [`SliceName`] is the one identifier with its own character set. A systemd
//! slice unit is a path of dash-separated components ending in `.slice`, and
//! admitting an arbitrary label there would let a payload name a control group
//! that cannot exist -- the scaffold carries the field as a bare `String` and
//! checks nothing.

use core::fmt;

#[path = "../../bounded_id.rs"]
mod bounded_id;

use bounded_id::InlineId;

/// Scalar upper bound, in bytes, on a correlation identifier.
pub const MAX_CORRELATION_LEN: usize = 64;

/// Scalar upper bound, in bytes, on a process or application label.
///
/// Thirty-two, which is twice the kernel's own `TASK_COMM_LEN` of sixteen and
/// holds every process name the scaffold registers. The bound is deliberately
/// tight: a table of [`MAX_TRACKED_PROCESSES`](crate::MAX_TRACKED_PROCESSES)
/// rows is a fixed array, and a wider label would trade memory that is always
/// resident for a name nothing reads past.
pub const MAX_LABEL_LEN: usize = 32;

/// Scalar upper bound, in bytes, on a control-group slice name.
pub const MAX_SLICE_LEN: usize = 96;

/// The suffix every systemd slice unit name ends in.
pub const SLICE_SUFFIX: &str = ".slice";

/// Reasons an identifier is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IdError {
    /// The identifier was empty.
    #[error("an identifier is empty")]
    Empty,
    /// The identifier was longer than its bound.
    #[error("an identifier of {actual} bytes exceeds the bound of {max}")]
    TooLong {
        /// The bound it passed.
        max: usize,
        /// The length observed.
        actual: usize,
    },
    /// The identifier carries a byte outside the permitted character set.
    #[error("an identifier carries a byte outside the permitted character set")]
    Charset,
    /// A slice name does not end in the slice suffix.
    #[error("a control-group slice name must end in {SLICE_SUFFIX}")]
    NotASlice,
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

/// A validated process or application label of at most [`MAX_LABEL_LEN`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label {
    inner: InlineId<MAX_LABEL_LEN>,
}

impl Label {
    /// Parses `raw` into a label.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] under the same rules as [`CorrelationId::parse`]:
    /// empty, over-long, or outside the permitted character set. The input is
    /// never truncated or sanitised.
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

/// A validated control-group slice name of at most [`MAX_SLICE_LEN`] bytes.
///
/// The focus-switch report REQ-P04-07 describes carries the target slice so
/// the broker can re-allocate on focus change. Naming something that is not a
/// slice would ask the broker to write to a control group that cannot exist,
/// so the suffix is part of the type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SliceName {
    inner: InlineId<MAX_SLICE_LEN>,
}

impl SliceName {
    /// Parses `raw` into a slice name.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] under [`Label::parse`]'s rules, plus
    /// [`IdError::NotASlice`] when the name does not end in
    /// [`SLICE_SUFFIX`].
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        if !raw.ends_with(SLICE_SUFFIX) || raw.len() <= SLICE_SUFFIX.len() {
            return Err(IdError::NotASlice);
        }
        InlineId::parse(
            raw,
            || IdError::Empty,
            |actual| IdError::TooLong {
                max: MAX_SLICE_LEN,
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

    /// Returns `false`; a slice name is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }
}

impl fmt::Display for SliceName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.inner.fmt(f)
    }
}
