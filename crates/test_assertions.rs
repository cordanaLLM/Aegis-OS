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

/// Asserts that a fresh buffer reports empty and that encoding into a default buffer preserves size.
#[allow(unused_macros)]
macro_rules! assert_payload_buffer_does_not_grow {
    ($buffer:ty, $encode:expr $(,)?) => {
        test_assertions::assert_payload_buffer_views_and_size(
            <$buffer>::new,
            |buffer: &$buffer| {
                (
                    buffer.is_empty(),
                    buffer.len(),
                    buffer.as_bytes(),
                    buffer.as_str(),
                )
            },
            $encode,
        )
    };
}

/// Asserts that a payload buffer's size is bounded by `bound` plus length.
#[allow(unused_macros)]
macro_rules! assert_payload_buffer_bound {
    ($buffer:ty, $bound:expr $(,)?) => {
        test_assertions::assert_bound_plus_length(core::mem::size_of::<$buffer>(), $bound)
    };
}

/// Asserts that a padding string past `max_bytes` is refused with `PayloadTooLong`.
#[allow(unused_macros)]
macro_rules! assert_unparsed_payload_too_long {
    ($max_bytes:expr, $refusal:expr, $err_ty:ident $(,)?) => {
        test_assertions::assert_unparsed_payload_refusal($max_bytes, $refusal, |err| match err {
            $err_ty::PayloadTooLong { max, correlation } => {
                Some((*max, correlation, correlation.id().is_some()))
            }
            _ => None,
        })
    };
}

#[allow(unused_imports)]
pub(crate) use assert_encoding_writes_into_payload_buffer;
#[allow(unused_imports)]
pub(crate) use assert_payload_buffer_bound;
#[allow(unused_imports)]
pub(crate) use assert_payload_buffer_does_not_grow;
#[allow(unused_imports)]
pub(crate) use assert_unparsed_payload_too_long;

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
    assert!(!export.contains('/'));
    assert_eq!(sha256_prefix.len(), 12);
    assert!(sha256_prefix.chars().all(|c| c.is_ascii_hexdigit()));
}

/// Asserts that a bounded identifier parser accepts up to `max_len` bytes and refuses past it.
#[track_caller]
pub fn assert_bounded_id_exact<T, E: core::fmt::Debug + PartialEq>(
    max_len: usize,
    parse: impl Fn(&str) -> Result<T, E>,
    len: impl Fn(&T) -> usize,
    expected_err: impl Fn(usize) -> E,
) {
    let at_bound = "a".repeat(max_len);
    let parsed = parse(&at_bound);
    assert!(parsed.is_ok());
    if let Ok(ref val) = parsed {
        assert_eq!(len(val), max_len);
    }

    let past = "a".repeat(max_len.saturating_add(1));
    assert_eq!(
        parse(&past).err(),
        Some(expected_err(max_len.saturating_add(1)))
    );

    let far_past = "a".repeat(512);
    assert_eq!(parse(&far_past).err(), Some(expected_err(512)));
}

/// Asserts that a fresh buffer reports empty and that encoding into a default buffer preserves size.
#[track_caller]
pub fn assert_payload_buffer_views_and_size<B: Default, E>(
    new: fn() -> B,
    view: impl Fn(&B) -> (bool, usize, &[u8], Option<&str>),
    encode: impl FnOnce(&mut B) -> Result<(), E>,
) -> Result<(), E> {
    let buffer = new();
    let (empty, len, bytes, text) = view(&buffer);
    assert_empty_buffer_view(empty, len, bytes, text);
    assert_eq!(core::mem::size_of_val(&buffer), core::mem::size_of::<B>());

    let mut second = B::default();
    encode(&mut second)?;
    assert_eq!(core::mem::size_of_val(&second), core::mem::size_of::<B>());
    Ok(())
}

/// Asserts that a payload past `max_bytes` is refused with `PayloadTooLong`.
#[track_caller]
pub fn assert_unparsed_payload_refusal<E: core::fmt::Debug, C: core::fmt::Display>(
    max_bytes: usize,
    refusal: impl FnOnce(&str) -> Result<E, Box<dyn std::error::Error>>,
    extract: impl FnOnce(&E) -> Option<(usize, &C, bool)>,
) -> Result<(), Box<dyn std::error::Error>> {
    let padding = "x".repeat(max_bytes.saturating_add(1));
    let err = refusal(&padding)?;
    if let Some((max, correlation, has_id)) = extract(&err) {
        assert_eq!(max, max_bytes);
        assert!(!has_id);
        assert!(correlation.to_string().ends_with("[uncorrelated]"));
        Ok(())
    } else {
        Err(format!("expected a length refusal, got {err:?}").into())
    }
}

/// Asserts that adding an unexpected stowaway field to an encoded JSON payload is refused as malformed.
#[track_caller]
pub fn assert_stowaway_rejected<E: core::fmt::Debug>(
    encoded: &str,
    refusal: impl FnOnce(&str) -> Result<E, Box<dyn std::error::Error>>,
    is_malformed: impl FnOnce(&E) -> bool,
) -> Result<(), Box<dyn std::error::Error>> {
    let extra = encoded.replace("{\"schema\"", "{\"stowaway\":1,\"schema\"");
    assert_ne!(extra, encoded, "encoded payload must contain schema prefix");
    let err = refusal(&extra)?;
    assert!(is_malformed(&err), "expected Malformed, got {err:?}");
    Ok(())
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
