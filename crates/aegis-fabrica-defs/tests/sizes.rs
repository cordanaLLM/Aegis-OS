// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Triples for size parsing, including the 512M/1G/511M boundary of REQ-P01-10.

use aegis_fabrica_defs::{
    ESP_MAX_BYTES, ESP_MIN_BYTES, GIBIBYTE, KIBIBYTE, MAX_SIZE_DIGITS, MEBIBYTE, parse_size,
};

// --- Positive -------------------------------------------------------------

/// Positive: every suffix systemd understands, and a bare byte count.
#[test]
fn every_suffix_multiplies_by_its_power_of_1024() {
    assert_eq!(parse_size("0"), Some(0));
    assert_eq!(parse_size("4096"), Some(4096));
    assert_eq!(parse_size("1K"), Some(KIBIBYTE));
    assert_eq!(parse_size("1M"), Some(MEBIBYTE));
    assert_eq!(parse_size("1G"), Some(GIBIBYTE));
    assert_eq!(parse_size("1T"), Some(1_099_511_627_776));
    assert_eq!(parse_size("1P"), Some(1_125_899_906_842_624));
    assert_eq!(parse_size("1E"), Some(1_152_921_504_606_846_976));
}

/// Positive: the two sizes the reviewed ESP declares are exact.
#[test]
fn the_reviewed_esp_bounds_parse_exactly() {
    assert_eq!(parse_size("512M"), Some(ESP_MIN_BYTES));
    assert_eq!(parse_size("1G"), Some(ESP_MAX_BYTES));
    assert_eq!(ESP_MIN_BYTES, 536_870_912);
    assert_eq!(ESP_MAX_BYTES, 1_073_741_824);
    assert_eq!(parse_size("10G"), Some(10_737_418_240));
}

// --- Negative -------------------------------------------------------------

/// Negative: anything that is not a decimal count with one known suffix.
#[test]
fn a_malformed_size_is_not_a_size() {
    for text in [
        "", " ", "M", "-1", "1.5G", "512 M", "512MB", "512m", "0x200", "512Gi", "１G",
    ] {
        assert_eq!(parse_size(text), None, "{text:?} must not parse");
    }
}

/// Negative: an overflowing product is refused, never wrapped.
#[test]
fn an_overflowing_size_is_refused() {
    assert_eq!(parse_size("16E"), None);
    assert_eq!(parse_size("18446744073709551616"), None);
    assert_eq!(
        parse_size(&"9".repeat(MAX_SIZE_DIGITS.saturating_add(2))),
        None
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: 511M sits one mebibyte below the ESP minimum and is detected as such.
#[test]
fn the_esp_minimum_is_exact_at_511m_and_512m() -> Result<(), Box<dyn std::error::Error>> {
    let below = parse_size("511M").ok_or("511M must parse")?;
    let at = parse_size("512M").ok_or("512M must parse")?;
    assert!(below < ESP_MIN_BYTES);
    assert_eq!(at, ESP_MIN_BYTES);
    assert_eq!(ESP_MIN_BYTES.checked_sub(below), Some(MEBIBYTE));
    Ok(())
}

/// Boundary: the largest u64 value parses, and the next suffix step does not.
#[test]
fn the_widest_representable_size_parses() {
    assert_eq!(parse_size("18446744073709551615"), Some(u64::MAX));
    assert_eq!(parse_size("15E"), Some(17_293_822_569_102_704_640));
    assert_eq!(
        parse_size("18014398509481983K"),
        Some(18_446_744_073_709_550_592)
    );
    assert_eq!(parse_size("18014398509481984K"), None);
}
