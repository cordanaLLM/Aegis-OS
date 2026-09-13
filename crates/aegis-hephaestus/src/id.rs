// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded, allocation-free `STEP` source path.
//!
//! The scaffold takes a `&Path` and asks the filesystem whether it exists.
//! This crate reads no filesystem, so what it can hold is the **name** of a
//! source: a validated, bounded path string that a loader would later open.
//! Refusing a malformed name is the half that is checkable here; whether a
//! well-formed name resolves to a file on some machine is not, and
//! [`StepPath`] does not pretend otherwise.
//!
//! There is deliberately no correlation-identifier type here. P14 threads its
//! payload with [`Identity`](aegis_justitia::Identity), the identifier the M14
//! contracts already use, because a second identifier type with its own bound
//! would be a second thing to keep in agreement with its neighbours.

use core::fmt;

/// Scalar upper bound, in bytes, on a `STEP` source path.
///
/// Chosen here, not recorded: no P14 requirement states a path bound. It is
/// what keeps a source name an inline, `Copy` value instead of a
/// heap-allocated one.
pub const MAX_STEP_PATH_LEN: usize = 128;

/// Reasons a `STEP` source path is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum IdError {
    /// The path was empty.
    #[error("a STEP source path is empty")]
    Empty,
    /// The path was longer than [`MAX_STEP_PATH_LEN`].
    #[error("a STEP source path of {actual} bytes exceeds the bound of {max}")]
    TooLong {
        /// The bound it passed.
        max: usize,
        /// The length observed.
        actual: usize,
    },
    /// The path carries a byte outside the permitted character set.
    #[error("a STEP source path carries a byte outside the permitted character set")]
    Charset,
}

/// Returns `true` for the bytes permitted inside a `STEP` source path.
const fn permitted(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'@' | b'/')
}

/// A validated `STEP` source path of at most [`MAX_STEP_PATH_LEN`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StepPath {
    bytes: [u8; MAX_STEP_PATH_LEN],
    len: u8,
}

impl StepPath {
    /// Parses `raw` into a `STEP` source path.
    ///
    /// # Errors
    ///
    /// Returns [`IdError`] when `raw` is empty, longer than
    /// [`MAX_STEP_PATH_LEN`] bytes, or carries a byte outside the permitted
    /// character set. The input is never truncated or sanitised, and nothing
    /// here touches a filesystem: a parsed path is a well-formed name and not
    /// an existing file.
    pub fn parse(raw: &str) -> Result<Self, IdError> {
        let source = raw.as_bytes();
        if source.is_empty() {
            return Err(IdError::Empty);
        }
        let len = u8::try_from(source.len())
            .ok()
            .filter(|_| source.len() <= MAX_STEP_PATH_LEN)
            .ok_or(IdError::TooLong {
                max: MAX_STEP_PATH_LEN,
                actual: source.len(),
            })?;
        let mut bytes = [0u8; MAX_STEP_PATH_LEN];
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

    /// Returns `false`; a parsed path is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Display for StepPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = core::str::from_utf8(self.as_bytes()).map_err(|_| fmt::Error)?;
        f.write_str(text)
    }
}
