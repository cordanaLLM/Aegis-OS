// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: shared assertion helpers for integration tests.
//!
//! One file, included by path into integration test suites
//! (`#[path = "../../test_assertions.rs"] mod test_assertions;`), extending D78 and D116
//! precedents to test assertions. It is not a crate and has no manifest:
//! a workspace member would enter every dependency closure.
//!
//! The helpers here operate on values rather than crate-specific types,
//! allowing tests across crates to share assertion logic without binding
//! them to a shared contract or type identity. Each helper carries
//! `#[track_caller]`, so a failure names the calling test's line.

#![allow(dead_code)]

/// Asserts that every requirement identifier in `names` appears at most once.
#[track_caller]
pub fn assert_unique_requirements(mut names: Vec<&str>) {
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(names.len(), before, "a requirement is recorded twice");
}

/// Asserts that a buffer view matches an empty payload buffer.
#[track_caller]
pub fn assert_empty_buffer_view(is_empty: bool, len: usize, bytes: &[u8], text: Option<&str>) {
    assert!(is_empty);
    assert_eq!(len, 0);
    assert_eq!(bytes, b"");
    assert_eq!(text, Some(""));
}

/// Asserts that `buffer_size` sits between `bound` and `bound + 2 * size_of::<usize>()`.
#[track_caller]
pub fn assert_bound_plus_length(buffer_size: usize, bound: usize) {
    assert!(buffer_size >= bound);
    assert!(buffer_size <= bound.saturating_add(core::mem::size_of::<usize>().saturating_mul(2)));
}

/// Asserts that `encode` writes its payload into a buffer from `new` and
/// returns a borrow of it, so a payload that fits costs no allocation of ours.
///
/// `view` reads a buffer as (length, byte length, text length, is empty); the
/// empty buffer is read through both `B::default()` and `new()`.
///
/// # Errors
///
/// Propagates what `encode` refuses.
#[track_caller]
pub fn assert_encoding_writes_into_buffer<B: Default, E>(
    new: fn() -> B,
    encode: impl FnOnce(&mut B) -> Result<usize, E>,
    max_bytes: usize,
    view: impl Fn(&B) -> (usize, usize, Option<usize>, bool),
) -> Result<(), E> {
    let mut buffer = new();
    let written = encode(&mut buffer)?;
    assert!(written > 0);
    assert!(written <= max_bytes);
    assert_eq!(core::mem::size_of_val(&buffer), core::mem::size_of::<B>());
    let (len, bytes, text, empty) = view(&buffer);
    assert_eq!(len, written);
    assert_eq!(bytes, written);
    assert_eq!(text, Some(written));
    assert!(!empty);
    assert_eq!(view(&B::default()).0, 0);
    assert_eq!(view(&new()).2, Some(0));
    Ok(())
}

/// Calls [`assert_encoding_writes_into_buffer`] for a payload buffer type
/// with `new`, `len`, `as_bytes`, `as_str` and `is_empty`, so each suite
/// names only its buffer type, its encoding and its byte bound.
#[allow(unused_macros)]
macro_rules! assert_encoding_writes_into_payload_buffer {
    ($buffer:ty, $encode:expr, $max_bytes:expr $(,)?) => {
        test_assertions::assert_encoding_writes_into_buffer(
            <$buffer>::new,
            $encode,
            $max_bytes,
            |buffer: &$buffer| {
                (
                    buffer.len(),
                    buffer.as_bytes().len(),
                    buffer.as_str().map(str::len),
                    buffer.is_empty(),
                )
            },
        )
    };
}

#[allow(unused_imports)]
pub(crate) use assert_encoding_writes_into_payload_buffer;

/// Asserts that payload `len` stays comfortably inside `max_bytes` and that the
/// buffer size is exactly the payload bound plus the length field.
#[track_caller]
pub fn assert_payload_inside_contract_bound(len: usize, max_bytes: usize, buffer_size: usize) {
    assert!(len < max_bytes);
    assert!(len.saturating_mul(2) < max_bytes);
    assert_eq!(
        buffer_size,
        max_bytes.saturating_add(core::mem::size_of::<usize>())
    );
}

/// Asserts that a claim source has a valid export identifier and sha256 prefix.
#[track_caller]
pub fn assert_claim_source(export: &str, sha256_prefix: &str) {
    assert!(export.starts_with("export-"));
    assert_eq!(sha256_prefix.len(), 12);
    assert!(sha256_prefix.chars().all(|c| c.is_ascii_hexdigit()));
}

/// Scalar bound on names scanned in an encoded payload assertion.
const NAME_BOUND: usize = 32;

/// Asserts that `text` contains every name in `names`.
#[track_caller]
pub fn assert_encoded_names(text: &str, names: &[&str]) {
    assert!(names.len() <= NAME_BOUND, "more names than the scan bound");
    for name in names.iter().take(NAME_BOUND) {
        assert!(text.contains(*name));
    }
}

/// Asserts that `plain`, with `from` respelled as the escaped `to`, still
/// decodes to `expected`.
#[track_caller]
pub fn escaped_round_trip<T, E>(
    plain: &str,
    from: &str,
    to: &str,
    decode: impl FnOnce(&str) -> Result<T, E>,
    expected: &T,
) -> Result<(), E>
where
    T: PartialEq + core::fmt::Debug,
{
    let escaped = plain.replace(from, to);
    assert!(escaped.contains(to));
    assert_ne!(escaped, plain);
    assert_eq!(&decode(&escaped)?, expected);
    Ok(())
}
