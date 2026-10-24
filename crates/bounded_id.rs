// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: The bounded identifier parse shared by every inline identifier.
//!
//! One file, included by path wherever an identifier is parsed into a fixed
//! inline array (`#[path = "../../bounded_id.rs"]` from a crate, or
//! `#[path = "bounded_id.rs"]` from `correlation_id.rs`), so the empty,
//! length and character-set refusals have one definition. Included by
//! `correlation_id.rs`, `aegis-calliope`, `aegis-compositor`, `aegis-lictor`
//! and `aegis-minerva`. It is not a crate: a member enters every dependency
//! closure and carries a permanent name.

#![allow(dead_code)]

/// Returns `true` for the bytes permitted inside a bounded identifier.
pub(crate) const fn permitted(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'@' | b'/')
}

/// Validates that `raw` is non-empty and fits within `max` bytes, returning its length as `u8`.
pub(crate) fn validate_bounded_len<E>(
    raw: &str,
    max: usize,
    on_empty: impl FnOnce() -> E,
    on_too_long: impl FnOnce(usize) -> E,
) -> Result<u8, E> {
    let source = raw.as_bytes();
    if source.is_empty() {
        return Err(on_empty());
    }
    if source.len() > max {
        return Err(on_too_long(source.len()));
    }
    u8::try_from(source.len()).map_err(|_| on_too_long(source.len()))
}

/// Parses `raw` into an inline array of `N` bytes and its length.
///
/// Refuses, in this order, empty input, input longer than `N` bytes and input
/// carrying a byte [`permitted`] rejects, each through the caller's own error.
pub(crate) fn parse_bounded_id<const N: usize, E>(
    raw: &str,
    on_empty: impl FnOnce() -> E,
    on_too_long: impl FnOnce(usize) -> E,
    on_charset: impl FnOnce() -> E,
) -> Result<([u8; N], u8), E> {
    let len = validate_bounded_len(raw, N, on_empty, on_too_long)?;
    let source = raw.as_bytes();
    if source.iter().copied().any(|b| !permitted(b)) {
        return Err(on_charset());
    }
    let mut bytes = [0u8; N];
    bytes[..source.len()].copy_from_slice(source);
    Ok((bytes, len))
}

/// A validated inline identifier of at most `N` bytes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct InlineId<const N: usize> {
    bytes: [u8; N],
    len: u8,
}

impl<const N: usize> InlineId<N> {
    /// Creates an identifier directly from validated bytes and length.
    pub(crate) const fn from_parts(bytes: [u8; N], len: u8) -> Self {
        Self { bytes, len }
    }

    /// Parses `raw` using the standard bounded identifier rules.
    pub(crate) fn parse<E>(
        raw: &str,
        on_empty: impl FnOnce() -> E,
        on_too_long: impl FnOnce(usize) -> E,
        on_charset: impl FnOnce() -> E,
    ) -> Result<Self, E> {
        let (bytes, len) = parse_bounded_id(raw, on_empty, on_too_long, on_charset)?;
        Ok(Self { bytes, len })
    }

    /// Returns the validated bytes, without trailing padding.
    pub(crate) fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..usize::from(self.len)).unwrap_or(&[])
    }

    /// Returns the length in bytes.
    pub(crate) const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `false`; an identifier is never empty by construction.
    pub(crate) const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Formats the validated bytes as UTF-8.
    pub(crate) fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let text = core::str::from_utf8(self.as_bytes()).map_err(|_| core::fmt::Error)?;
        f.write_str(text)
    }
}
