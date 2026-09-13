// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The dm-verity root hash, and what "verified" means in this model.
//!
//! REQ-P02-01 requires that the read-only EROFS root is bit-for-bit identical
//! to the signed release before it is treated as trusted. In this crate that
//! reduces to one comparison: the [`RootHash`] declared for the candidate must
//! equal the [`RootHash`] measured from what actually landed in the slot.
//!
//! # What this is not
//!
//! Nothing here computes a root hash, opens a device-mapper target, reads a
//! partition or checks a signature over the hash. The measured value arrives
//! from a stubbed port as data. The crate therefore models *when* a verity
//! match is required and *what a mismatch does*; proving that a real image
//! hashes to a real value is M24 and M11 work over a real artefact.

use core::fmt;

/// The width of a dm-verity SHA-256 root hash, in bytes.
pub const ROOT_HASH_BYTES: usize = 32;

/// The width of a root hash written as lower-case hexadecimal.
pub const ROOT_HASH_HEX_LEN: usize = 64;

/// Reasons a root hash is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum VerityError {
    /// The text was not exactly [`ROOT_HASH_HEX_LEN`] characters.
    #[error("root hash text of {actual} characters is not the {expected} required")]
    Width {
        /// The width a root hash is written at.
        expected: usize,
        /// The width observed.
        actual: usize,
    },
    /// The text was not lower-case hexadecimal.
    #[error("root hash text is not lower-case hexadecimal")]
    Encoding,
}

/// A dm-verity root hash, stored inline.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RootHash([u8; ROOT_HASH_BYTES]);

impl RootHash {
    /// The all-zero hash.
    ///
    /// It is a representable value rather than a sentinel for "unmeasured":
    /// the machine tracks an absent measurement as `None`, so a slot cannot be
    /// blessed because its measurement happened to be zero.
    pub const ZERO: Self = Self([0u8; ROOT_HASH_BYTES]);

    /// Wraps raw hash bytes.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; ROOT_HASH_BYTES]) -> Self {
        Self(bytes)
    }

    /// Returns the hash bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; ROOT_HASH_BYTES] {
        &self.0
    }

    /// Parses a lower-case hexadecimal root hash.
    ///
    /// # Errors
    ///
    /// Returns [`VerityError::Width`] when the text is not
    /// [`ROOT_HASH_HEX_LEN`] characters and [`VerityError::Encoding`] when it
    /// is not lower-case hexadecimal. Upper-case input is refused rather than
    /// folded, so a trace cannot render two spellings of one hash.
    pub fn parse_hex(text: &str) -> Result<Self, VerityError> {
        if text.len() != ROOT_HASH_HEX_LEN {
            return Err(VerityError::Width {
                expected: ROOT_HASH_HEX_LEN,
                actual: text.len(),
            });
        }
        let mut bytes = [0u8; ROOT_HASH_BYTES];
        let decoded = base16ct::lower::decode(text.as_bytes(), &mut bytes)
            .map_err(|_| VerityError::Encoding)?;
        if decoded.len() != ROOT_HASH_BYTES {
            return Err(VerityError::Width {
                expected: ROOT_HASH_BYTES,
                actual: decoded.len(),
            });
        }
        Ok(Self(bytes))
    }

    /// Renders the hash as lower-case hexadecimal into a caller-supplied buffer.
    ///
    /// The buffer is caller-supplied so the lifecycle path never allocates.
    ///
    /// # Errors
    ///
    /// Returns [`VerityError::Encoding`] when the buffer is the wrong width.
    pub fn encode_hex<'b>(
        &self,
        buffer: &'b mut [u8; ROOT_HASH_HEX_LEN],
    ) -> Result<&'b str, VerityError> {
        base16ct::lower::encode_str(&self.0, buffer).map_err(|_| VerityError::Encoding)
    }
}

impl fmt::Display for RootHash {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut buffer = [0u8; ROOT_HASH_HEX_LEN];
        let text = self.encode_hex(&mut buffer).map_err(|_| fmt::Error)?;
        formatter.write_str(text)
    }
}
