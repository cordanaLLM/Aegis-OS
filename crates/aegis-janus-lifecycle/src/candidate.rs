// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The candidate release: what an A/B update is *about*.
//!
//! A [`Candidate`] names three things and nothing else: which release version
//! is being installed, which slot it is being written into, and the dm-verity
//! root hash the signed release declares. All three are needed before the
//! machine leaves [`State::Idle`](crate::State::Idle), because every later
//! stage is judged against them.
//!
//! [`Version`] is a bounded, validated, inline type in the shape the activated
//! crates already use: validated on construction, stored in a fixed buffer,
//! never truncated or sanitised, and `Copy`, so no lifecycle path allocates.

use core::fmt;

use crate::slot::Slot;
use crate::verity::RootHash;

/// Scalar upper bound on a release version string, in bytes.
pub const MAX_VERSION_LEN: usize = 64;

/// Reasons a release version is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VersionError {
    /// The version was empty.
    #[error("release version is empty")]
    Empty,
    /// The version exceeded the scalar bound.
    #[error("release version of {actual} bytes exceeds the bound of {max}")]
    TooLong {
        /// Inclusive upper bound in bytes.
        max: usize,
        /// Observed length in bytes.
        actual: usize,
    },
    /// The version contained a byte outside the permitted character set.
    #[error("release version contains a byte outside the permitted character set")]
    Charset,
}

/// Returns `true` for the bytes permitted inside a release version.
///
/// The set is what `sysupdate.d(5)`'s `@v` wildcard can capture out of a file
/// name such as `aegis-os-root-1.2.3.raw.xz`, minus anything that would need
/// escaping when the version is rendered into a JSON trace line. A version
/// carrying a quote, a solidus or a control byte is refused rather than
/// escaped, which is what keeps a rendered trace byte-comparable.
#[must_use]
const fn permitted(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'+' | b'~')
}

/// A validated release version of at most [`MAX_VERSION_LEN`] bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Version {
    bytes: [u8; MAX_VERSION_LEN],
    len: u8,
}

impl Version {
    /// Parses `raw` into a release version.
    ///
    /// # Errors
    ///
    /// Returns [`VersionError`] when `raw` is empty, longer than
    /// [`MAX_VERSION_LEN`] bytes, or carries a byte outside the permitted
    /// character set. The input is never truncated or sanitised.
    pub fn parse(raw: &str) -> Result<Self, VersionError> {
        let source = raw.as_bytes();
        if source.is_empty() {
            return Err(VersionError::Empty);
        }
        if source.len() > MAX_VERSION_LEN {
            return Err(VersionError::TooLong {
                max: MAX_VERSION_LEN,
                actual: source.len(),
            });
        }
        let mut bytes = [0u8; MAX_VERSION_LEN];
        for (slot, byte) in bytes.iter_mut().zip(source.iter()) {
            if !permitted(*byte) {
                return Err(VersionError::Charset);
            }
            *slot = *byte;
        }
        let len = u8::try_from(source.len()).map_err(|_| VersionError::TooLong {
            max: MAX_VERSION_LEN,
            actual: source.len(),
        })?;
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

    /// Returns `false`; a version is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the version as text.
    ///
    /// # Errors
    ///
    /// Returns [`VersionError::Charset`] when the stored bytes are not UTF-8.
    /// They always are, because [`Self::parse`] admits ASCII only; the error
    /// exists so that no caller needs an `unwrap` to read one back.
    pub fn as_str(&self) -> Result<&str, VersionError> {
        core::str::from_utf8(self.as_bytes()).map_err(|_| VersionError::Charset)
    }
}

impl fmt::Display for Version {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = self.as_str().map_err(|_| fmt::Error)?;
        formatter.write_str(text)
    }
}

/// The release an A/B update is installing, and where.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Candidate {
    version: Version,
    target: Slot,
    expected_root_hash: RootHash,
}

impl Candidate {
    /// Declares a candidate release for `target`.
    ///
    /// `expected_root_hash` is the dm-verity root hash the signed release
    /// declares. It is the value every later verity comparison is made
    /// against, including the one a reopened slot must pass again (D13).
    #[must_use]
    pub const fn new(version: Version, target: Slot, expected_root_hash: RootHash) -> Self {
        Self {
            version,
            target,
            expected_root_hash,
        }
    }

    /// Returns the release version.
    #[must_use]
    pub const fn version(&self) -> Version {
        self.version
    }

    /// Returns the slot the release is written into.
    #[must_use]
    pub const fn target(&self) -> Slot {
        self.target
    }

    /// Returns the slot that stays bootable while the candidate is on trial.
    #[must_use]
    pub const fn fallback(&self) -> Slot {
        self.target.other()
    }

    /// Returns the root hash the signed release declares.
    #[must_use]
    pub const fn expected_root_hash(&self) -> RootHash {
        self.expected_root_hash
    }
}
