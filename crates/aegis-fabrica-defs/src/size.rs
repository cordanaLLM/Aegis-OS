// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Sizes as systemd writes them: a decimal count with an optional binary suffix.
//!
//! `repart.d(5)` sizes are `K`, `M`, `G`, `T`, `P` and `E` over 1024, or a bare
//! byte count. Parsing is exact and checked: an overflowing product is a
//! refusal, never a wrapped value, and no arithmetic in this module can panic.

/// One kibibyte, in bytes.
pub const KIBIBYTE: u64 = 1024;

/// One mebibyte, in bytes.
pub const MEBIBYTE: u64 = 1_048_576;

/// One gibibyte, in bytes.
pub const GIBIBYTE: u64 = 1_073_741_824;

/// The smallest ESP the reviewed definitions admit (REQ-P01-10).
pub const ESP_MIN_BYTES: u64 = 536_870_912;

/// The largest ESP the reviewed definitions admit (REQ-P01-10).
pub const ESP_MAX_BYTES: u64 = GIBIBYTE;

/// Scalar bound on the digits a size may carry.
pub const MAX_SIZE_DIGITS: usize = 20;

/// The suffixes systemd understands, each with its multiplier.
const SUFFIXES: [(char, u64); 6] = [
    ('K', KIBIBYTE),
    ('M', MEBIBYTE),
    ('G', GIBIBYTE),
    ('T', 1_099_511_627_776),
    ('P', 1_125_899_906_842_624),
    ('E', 1_152_921_504_606_846_976),
];

/// Returns the multiplier `suffix` names, or `None` when it names none.
fn multiplier(suffix: char) -> Option<u64> {
    SUFFIXES
        .iter()
        .find(|(letter, _)| *letter == suffix)
        .map(|(_, factor)| *factor)
}

/// Splits `text` into its digits and its optional suffix multiplier.
fn split_suffix(text: &str) -> Option<(&str, u64)> {
    let last = text.chars().next_back()?;
    if last.is_ascii_digit() {
        return Some((text, 1));
    }
    let factor = multiplier(last)?;
    let digits = text.get(..text.len().saturating_sub(last.len_utf8()))?;
    Some((digits, factor))
}

/// Parses a systemd size, returning the exact byte count.
///
/// Returns `None` when `text` is not a decimal count with an optional `K`,
/// `M`, `G`, `T`, `P` or `E` suffix, or when the product overflows `u64`.
///
/// ```
/// use aegis_fabrica_defs::{parse_size, ESP_MIN_BYTES};
///
/// assert_eq!(parse_size("512M"), Some(ESP_MIN_BYTES));
/// assert_eq!(parse_size("1G"), Some(1_073_741_824));
/// assert_eq!(parse_size("512 M"), None);
/// ```
#[must_use]
pub fn parse_size(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    if trimmed.is_empty() || trimmed.len() > MAX_SIZE_DIGITS.saturating_add(1) {
        return None;
    }
    let (digits, factor) = split_suffix(trimmed)?;
    if digits.is_empty() || !digits.chars().all(|c| c.is_ascii_digit()) {
        return None;
    }
    let count: u64 = digits.parse().ok()?;
    count.checked_mul(factor)
}
