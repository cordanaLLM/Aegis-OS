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

/// Returns `true` for the bytes permitted inside a correlation identifier.
const fn permitted(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'@' | b'/')
}

/// A validated correlation identifier of at most [`MAX_CORRELATION_LEN`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CorrelationId {
    bytes: [u8; MAX_CORRELATION_LEN],
    len: u8,
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
        let source = raw.as_bytes();
        if source.is_empty() {
            return Err(IdError::Empty);
        }
        if source.len() > MAX_CORRELATION_LEN {
            return Err(IdError::TooLong {
                max: MAX_CORRELATION_LEN,
                actual: source.len(),
            });
        }
        let len = u8::try_from(source.len()).map_err(|_| IdError::TooLong {
            max: MAX_CORRELATION_LEN,
            actual: source.len(),
        })?;
        let mut bytes = [0u8; MAX_CORRELATION_LEN];
        for (slot, byte) in bytes.iter_mut().zip(source.iter()) {
            if !permitted(*byte) {
                return Err(IdError::Charset);
            }
            *slot = *byte;
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

    /// Returns `false`; an identifier is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Display for CorrelationId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = core::str::from_utf8(self.as_bytes()).map_err(|_| fmt::Error)?;
        f.write_str(text)
    }
}

/// Scalar upper bound, in bytes, on a surface identifier.
pub const MAX_SURFACE_ID_LEN: usize = 64;

/// A validated surface identifier of at most [`MAX_SURFACE_ID_LEN`] bytes.
///
/// It names the picture-in-picture surface a registration is about, so a
/// result can be tied back to the request that asked for it. It is not a
/// Wayland object identifier: no Wayland connection exists here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId {
    bytes: [u8; MAX_SURFACE_ID_LEN],
    len: u8,
}

impl SurfaceId {
    /// Parses `raw` into a surface identifier.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] under the same rules as
    /// [`CorrelationId::parse`]: empty, over-long, or outside the permitted
    /// character set. The input is never truncated or sanitised.
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        let source = raw.as_bytes();
        if source.is_empty() {
            return Err(IdError::Empty);
        }
        if source.len() > MAX_SURFACE_ID_LEN {
            return Err(IdError::TooLong {
                max: MAX_SURFACE_ID_LEN,
                actual: source.len(),
            });
        }
        let len = u8::try_from(source.len()).map_err(|_| IdError::TooLong {
            max: MAX_SURFACE_ID_LEN,
            actual: source.len(),
        })?;
        let mut bytes = [0u8; MAX_SURFACE_ID_LEN];
        for (slot, byte) in bytes.iter_mut().zip(source.iter()) {
            if !permitted(*byte) {
                return Err(IdError::Charset);
            }
            *slot = *byte;
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

    /// Returns `false`; a surface identifier is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Display for SurfaceId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = core::str::from_utf8(self.as_bytes()).map_err(|_| fmt::Error)?;
        f.write_str(text)
    }
}
