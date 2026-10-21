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
