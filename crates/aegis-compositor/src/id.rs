// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded, allocation-free identifiers P04 carries.
//!
//! Two shapes live here: a [`Label`] for a surface title or an application
//! identifier, and a [`KeyExpr`] for the mesh key expression a publication or
//! a subscription names. Each is validated on construction and stored inline,
//! so no decision path allocates for one and none can be silently truncated or
//! sanitised: a malformed input is refused.
//!
//! The scaffold (export-027 `f19640d7a7da`) carries both as `String` and
//! validates neither, so `publish_zenoh_shm("", &[])` inserts a topic whose
//! name is the empty string and reports success.

use core::fmt;

/// Scalar upper bound, in bytes, on a surface title or application identifier.
///
/// Thirty-two. The bound is deliberately tight: the registry is a fixed array
/// of [`MAX_SURFACES`](crate::MAX_SURFACES) rows carried by value, so a wider
/// title costs memory that is always resident.
pub const MAX_LABEL_LEN: usize = 32;

/// Scalar upper bound, in bytes, on a mesh key expression.
pub const MAX_KEY_EXPR_LEN: usize = 96;

/// The separator between the segments of a key expression.
pub const KEY_EXPR_SEPARATOR: u8 = b'/';

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
    /// A key expression carries an empty segment.
    #[error("a key expression carries an empty segment")]
    EmptyKeySegment,
}

/// Returns `true` for the bytes permitted inside a label.
const fn permitted(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'@' | b'/')
}

/// Returns `true` for the bytes permitted inside a key expression.
///
/// The separator is excluded here and handled by the segment walk, and `*` is
/// admitted because a subscription names a pattern rather than one topic.
const fn permitted_in_key(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'*')
}

/// Advances the current segment length over one key-expression byte.
///
/// A separator closes the segment, so an empty one -- a leading, trailing or
/// doubled `/` -- is refused here rather than by a nested branch in the walk.
const fn key_step(byte: u8, segment_len: usize) -> Result<usize, IdError> {
    if byte == KEY_EXPR_SEPARATOR {
        if segment_len == 0 {
            return Err(IdError::EmptyKeySegment);
        }
        return Ok(0);
    }
    if permitted_in_key(byte) {
        return Ok(segment_len.saturating_add(1));
    }
    Err(IdError::Charset)
}

/// Copies `raw` into `slot`, refusing an empty, over-long or ill-formed input.
fn fill(raw: &str, slot: &mut [u8], max: usize) -> Result<u8, IdError> {
    let source = raw.as_bytes();
    if source.is_empty() {
        return Err(IdError::Empty);
    }
    let len = u8::try_from(source.len())
        .ok()
        .filter(|_| source.len() <= max)
        .ok_or(IdError::TooLong {
            max,
            actual: source.len(),
        })?;
    for (target, byte) in slot.iter_mut().zip(source.iter()) {
        if !permitted(*byte) {
            return Err(IdError::Charset);
        }
        *target = *byte;
    }
    Ok(len)
}

/// A validated surface title or application identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Label {
    bytes: [u8; MAX_LABEL_LEN],
    len: u8,
}

impl Label {
    /// Parses `raw` into a label.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] when `raw` is empty, longer than [`MAX_LABEL_LEN`]
    /// bytes, or carries a byte outside the permitted character set. The input
    /// is never truncated or sanitised, so the scaffold's own
    /// `"Forum Desktop Shell"` is [`IdError::Charset`] rather than a repaired
    /// title.
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        let mut bytes = [0u8; MAX_LABEL_LEN];
        let len = fill(raw, &mut bytes, MAX_LABEL_LEN)?;
        Ok(Self { bytes, len })
    }

    /// Returns the validated bytes, without the trailing padding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.len)).unwrap_or(&[])
    }

    /// Returns the length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `false`; a label is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Display for Label {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = core::str::from_utf8(self.as_bytes()).map_err(|_| fmt::Error)?;
        f.write_str(text)
    }
}

/// A validated mesh key expression, such as `aegis/compositor/focus`.
///
/// The shape is the one the P04 report's own topic hierarchy uses: one or more
/// non-empty segments joined by `/`, with no leading or trailing separator.
/// The empty key expression is [`IdError::Empty`], which is the case the
/// scaffold accepts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyExpr {
    bytes: [u8; MAX_KEY_EXPR_LEN],
    len: u8,
}

impl KeyExpr {
    /// Parses `raw` into a key expression.
    ///
    /// # Errors
    ///
    /// Returns [`IdError::Empty`] for the empty expression,
    /// [`IdError::TooLong`] past [`MAX_KEY_EXPR_LEN`],
    /// [`IdError::EmptyKeySegment`] for a leading, trailing or doubled
    /// separator, and [`IdError::Charset`] for any other byte outside the
    /// permitted set.
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        let source = raw.as_bytes();
        if source.is_empty() {
            return Err(IdError::Empty);
        }
        let len = u8::try_from(source.len())
            .ok()
            .filter(|_| source.len() <= MAX_KEY_EXPR_LEN)
            .ok_or(IdError::TooLong {
                max: MAX_KEY_EXPR_LEN,
                actual: source.len(),
            })?;
        let mut bytes = [0u8; MAX_KEY_EXPR_LEN];
        let mut segment_len = 0usize;
        for (target, byte) in bytes.iter_mut().zip(source.iter()) {
            segment_len = key_step(*byte, segment_len)?;
            *target = *byte;
        }
        if segment_len == 0 {
            return Err(IdError::EmptyKeySegment);
        }
        Ok(Self { bytes, len })
    }

    /// Returns the validated bytes, without the trailing padding.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.len)).unwrap_or(&[])
    }

    /// Returns the length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `false`; a key expression is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns how many segments the expression carries.
    #[must_use]
    pub fn segments(&self) -> usize {
        self.as_bytes().iter().fold(1usize, |seen, byte| {
            if *byte == KEY_EXPR_SEPARATOR {
                seen.saturating_add(1)
            } else {
                seen
            }
        })
    }
}

impl fmt::Display for KeyExpr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = core::str::from_utf8(self.as_bytes()).map_err(|_| fmt::Error)?;
        f.write_str(text)
    }
}
