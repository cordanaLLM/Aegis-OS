// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Where the local-first store lives, as a validated path.
//!
//! REQ-P15-08 places stateful persistence on a LUKS2-encrypted Btrfs `/var`
//! partition with an `@pglite` subvolume among others, and REQ-GRAPH-03
//! records that the dataflow document draws a P15-to-P02 edge for exactly that
//! subvolume which the graph of record does not carry. [`StoragePath`] is the
//! field those requirements touch: an absolute, bounded, charset-checked path,
//! validated on construction so a relative or traversing path never becomes
//! one.
//!
//! # What this module does not do
//!
//! It creates no directory, mounts no subvolume, opens no file and asks the
//! filesystem nothing. A [`StoragePath`] is a validated string; whether the
//! path exists, is on Btrfs, or is a subvolume at all is not knowable here and
//! is not claimed.

use core::fmt;

use crate::error::HestiaError;

/// Scalar upper bound, in bytes, on a storage path.
pub const MAX_STORAGE_PATH_LEN: usize = 128;

/// The Btrfs subvolume REQ-P15-08 records for the local-first store.
pub const PGLITE_SUBVOLUME: &str = "@pglite";

/// The path the imported scaffold opens the store at.
pub const SCAFFOLD_STORAGE_PATH: &str = "/var/pglite";

/// Returns `true` for the bytes permitted inside a storage path.
const fn permitted(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b'/' | b'@')
}

/// An absolute, bounded storage path.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StoragePath {
    bytes: [u8; MAX_STORAGE_PATH_LEN],
    len: u8,
}

impl StoragePath {
    /// Parses an absolute storage path.
    ///
    /// # Errors
    ///
    /// Returns [`HestiaError::StoragePath`] when the path is empty, longer
    /// than [`MAX_STORAGE_PATH_LEN`], not absolute, carries a byte outside the
    /// permitted character set, or contains a `..` component. The input is
    /// never truncated, normalised or sanitised.
    pub fn parse(raw: &str) -> Result<Self, HestiaError> {
        let source = raw.as_bytes();
        Self::check(raw, source)?;
        let len = u8::try_from(source.len()).map_err(|_| HestiaError::StoragePath {
            reason: "a storage path is longer than the recorded bound",
        })?;
        let mut bytes = [0u8; MAX_STORAGE_PATH_LEN];
        for (slot, byte) in bytes.iter_mut().zip(source.iter()) {
            *slot = *byte;
        }
        Ok(Self { bytes, len })
    }

    /// Refuses every shape a storage path may not take.
    fn check(raw: &str, source: &[u8]) -> Result<(), HestiaError> {
        if source.is_empty() {
            return Err(HestiaError::StoragePath {
                reason: "a storage path is empty",
            });
        }
        if source.len() > MAX_STORAGE_PATH_LEN {
            return Err(HestiaError::StoragePath {
                reason: "a storage path is longer than the recorded bound",
            });
        }
        if !raw.starts_with('/') {
            return Err(HestiaError::StoragePath {
                reason: "a storage path is not absolute",
            });
        }
        if raw.split('/').any(|part| part == "..") {
            return Err(HestiaError::StoragePath {
                reason: "a storage path carries a parent-directory component",
            });
        }
        if !source.iter().all(|byte| permitted(*byte)) {
            return Err(HestiaError::StoragePath {
                reason: "a storage path carries a byte outside the permitted character set",
            });
        }
        Ok(())
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

    /// Returns `false`; a storage path is never empty by construction.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

impl fmt::Display for StoragePath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = core::str::from_utf8(self.as_bytes()).map_err(|_| fmt::Error)?;
        f.write_str(text)
    }
}
