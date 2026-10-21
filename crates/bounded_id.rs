// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: The bounded identifier parse shared by every inline identifier.
//!
//! One file, included by path wherever an identifier is parsed into a fixed
//! inline array (`#[path = "../../bounded_id.rs"]` from a crate, or
//! `#[path = "bounded_id.rs"]` from `correlation_id.rs`), so the empty,
//! length and character-set refusals have one definition. Included by
//! `correlation_id.rs`, `aegis-calliope` and `aegis-minerva`. It is not a
//! crate: a member enters every dependency closure and carries a permanent
//! name.

#![allow(dead_code)]

/// Returns `true` for the bytes permitted inside a bounded identifier.
pub(crate) const fn permitted(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-' | b'_' | b':' | b'@' | b'/')
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
    let source = raw.as_bytes();
    if source.is_empty() {
        return Err(on_empty());
    }
    if source.len() > N {
        return Err(on_too_long(source.len()));
    }
    if source.iter().copied().any(|b| !permitted(b)) {
        return Err(on_charset());
    }
    let len = u8::try_from(source.len()).map_err(|_| on_too_long(source.len()))?;
    let mut bytes = [0u8; N];
    bytes[..source.len()].copy_from_slice(source);
    Ok((bytes, len))
}
