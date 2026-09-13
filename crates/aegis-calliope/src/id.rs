// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded, allocation-free identifiers P08 carries.
//!
//! `docs/integration/stack.md` asks every crossing to carry a correlation
//! identifier, and both P08 payloads also name the plugin or the stream they
//! are about. Each is validated on construction and stored inline, so no
//! decision path allocates for one and none can be silently truncated or
//! sanitised: a malformed input is refused.
//!
//! The scaffold (export-026 `c2f1e433cd32`) carries every name as a `String`
//! and admits whatever it is handed, which is how `"FabFilter Pro-Q 3"` --
//! spaces and all -- becomes a plugin name that no later validation can
//! reject. [`Label`] keeps the inline storage and adds the refusal, so that
//! name is refused here rather than accepted and repaired downstream.

use core::fmt;

/// Scalar upper bound, in bytes, on a correlation identifier.
pub const MAX_CORRELATION_LEN: usize = 64;

/// Scalar upper bound, in bytes, on a plugin, thread or stream label.
pub const MAX_LABEL_LEN: usize = 64;

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
}

/// Returns `true` for the bytes permitted inside an identifier.
///
/// The space is deliberately absent: it is the byte that makes the scaffold's
/// own plugin names unrepresentable here, and admitting it would mean a label
/// that no downstream reader can split on whitespace.
const fn permitted(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'@' | b'/')
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
        let mut bytes = [0u8; MAX_CORRELATION_LEN];
        let len = fill(raw, &mut bytes, MAX_CORRELATION_LEN)?;
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

/// A validated plugin, thread or stream label of at most [`MAX_LABEL_LEN`]
/// bytes.
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
    /// Returns [`IdError`] under the same rules as [`CorrelationId::parse`]:
    /// empty, over-long, or outside the permitted character set. The input is
    /// never truncated or sanitised, so the scaffold's own `"FabFilter Pro-Q
    /// 3"` is [`IdError::Charset`] rather than a repaired name.
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
