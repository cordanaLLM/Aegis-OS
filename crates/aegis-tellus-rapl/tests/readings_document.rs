// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The readings document the gate hands over: parsed, bounded, refused.

mod common;

use aegis_tellus::{CounterRead, EnergyRange, RaplZone, TellusError};
use aegis_tellus_rapl::{
    InputError, MAX_DOCUMENT_BYTES, MAX_FIELD_BYTES, MAX_READINGS, MIN_READINGS, Mode,
    READINGS_SCHEMA, Read, Readings,
};
use serde_json::{Value, json};

use common::{Fallible, RECORDED_RUN, reading, recorded, set, set_in};

/// Parses a JSON value as a document.
fn parse(value: &Value) -> Result<Readings, InputError> {
    Readings::parse(&value.to_string())
}

/// Returns the recorded run with `key` replaced by `new`.
fn with(key: &str, new: Value) -> Result<Value, serde_json::Error> {
    let mut document = recorded()?;
    set(&mut document, key, new);
    Ok(document)
}

/// Returns the recorded run carrying `count` readings.
fn with_count(count: usize) -> Result<Value, Box<dyn std::error::Error>> {
    let stamps = 1..=u64::try_from(count)?;
    let readings: Vec<Value> = stamps.map(|stamp| reading("1\n", stamp)).collect();
    Ok(with("readings", Value::Array(readings))?)
}

// --- Positive -------------------------------------------------------------

/// Positive: the recorded run's document parses into its zone and range.
#[test]
fn the_recorded_document_parses() -> Fallible {
    let document = Readings::parse(RECORDED_RUN)?;
    assert_eq!(document.schema, READINGS_SCHEMA);
    assert_eq!(document.mode, Mode::Pair);
    assert_eq!(document.zone()?, RaplZone::Package0);
    assert_eq!(document.range()?, EnergyRange::REFERENCE);
    assert_eq!(document.max_energy_range_uj, "65532610987\n");
    assert_eq!(document.readings.len(), 2);
    Ok(())
}

/// Positive: the recorded run's one pair carries its readings and interval.
#[test]
fn the_recorded_pair_is_read_back() -> Fallible {
    let document = Readings::parse(RECORDED_RUN)?;
    assert_eq!(document.pair_count(), 1);
    let pair = document.pair(0, EnergyRange::REFERENCE)?;
    assert_eq!(pair.before.microjoules(), 45_514_845_753);
    assert_eq!(pair.after.microjoules(), 46_107_552_971);
    assert_eq!(pair.interval.nanos(), 5_015_051_212);
    Ok(())
}

/// Positive: a read names exactly one of its text and its error.
#[test]
fn a_read_names_its_outcome() -> Fallible {
    let document = Readings::parse(RECORDED_RUN)?;
    let first = document.readings.first().ok_or("a reading")?;
    assert!(matches!(
        first.outcome()?,
        CounterRead::Text("45514845753\n")
    ));
    assert_eq!(first.monotonic_ns, 172_106_800_168_887);
    assert!(first.error.is_none());
    assert_eq!(document.unprivileged.outcome()?, CounterRead::Unreadable);
    assert!(document.unprivileged.text.is_none());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a wrong schema, an unknown field and a malformed document are
/// refused before anything is judged.
#[test]
fn a_foreign_document_is_refused() -> Fallible {
    let wrong = with("schema", json!("aegis.m21.rapl-readings.v2"))?;
    assert!(matches!(parse(&wrong), Err(InputError::Refused(_))));
    let extra = with("watts", json!(118.0))?;
    assert!(matches!(parse(&extra), Err(InputError::Malformed(_))));
    assert!(matches!(
        Readings::parse("{"),
        Err(InputError::Malformed(_))
    ));
    let mode = with("mode", json!("guess"))?;
    assert!(matches!(parse(&mode), Err(InputError::Malformed(_))));
    Ok(())
}

/// Negative: timestamps that do not increase, and a read with both or
/// neither of text and error, are refused.
#[test]
fn inconsistent_reads_are_refused() -> Fallible {
    let backwards = with("readings", json!([reading("1\n", 10), reading("2\n", 10)]))?;
    assert!(matches!(parse(&backwards), Err(InputError::Refused(_))));
    let both = Read {
        text: Some("1\n".to_owned()),
        error: Some("EACCES".to_owned()),
        monotonic_ns: 1,
    };
    assert!(matches!(both.outcome(), Err(InputError::Refused(_))));
    let neither = Read {
        text: None,
        error: None,
        monotonic_ns: 1,
    };
    assert!(matches!(neither.outcome(), Err(InputError::Refused(_))));
    Ok(())
}

/// Negative: a zone name outside the powercap vocabulary, and a privileged
/// read that failed, are refused rather than defaulted.
#[test]
fn an_unknown_zone_and_a_failed_read_are_refused() -> Fallible {
    let zone = with("zone", json!("uncore\n"))?;
    assert!(matches!(parse(&zone)?.zone(), Err(InputError::Refused(_))));
    let failed = with(
        "readings",
        json!([
            {"error": "TimeoutExpired", "monotonic-ns": 1},
            reading("46107552971\n", 2),
        ]),
    )?;
    let document = parse(&failed)?;
    assert!(matches!(
        document.pair(0, EnergyRange::REFERENCE),
        Err(InputError::Tellus(TellusError::Counter { .. }))
    ));
    assert!(matches!(
        document.pair(1, EnergyRange::REFERENCE),
        Err(InputError::Refused(_))
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: two readings are the fewest admitted and one is refused; the
/// most admitted is the bound and one more is refused.
#[test]
fn the_reading_count_is_closed_at_both_ends() -> Fallible {
    assert!(parse(&with_count(MIN_READINGS)?).is_ok());
    let fewer = MIN_READINGS.checked_sub(1).ok_or("a bound above zero")?;
    assert!(matches!(
        parse(&with_count(fewer)?),
        Err(InputError::Refused(_))
    ));
    assert!(parse(&with_count(MAX_READINGS)?).is_ok());
    let more = MAX_READINGS
        .checked_add(1)
        .ok_or("a bound below usize::MAX")?;
    assert!(matches!(
        parse(&with_count(more)?),
        Err(InputError::Refused(_))
    ));
    Ok(())
}

/// Boundary: the field and document byte bounds are closed at their edges.
#[test]
fn the_byte_bounds_are_closed_at_their_edges() -> Fallible {
    let mut field = recorded()?;
    set_in(
        &mut field,
        "unprivileged",
        "error",
        json!("e".repeat(MAX_FIELD_BYTES)),
    );
    assert!(parse(&field).is_ok());
    let longer = MAX_FIELD_BYTES.checked_add(1).ok_or("a bound")?;
    set_in(
        &mut field,
        "unprivileged",
        "error",
        json!("e".repeat(longer)),
    );
    assert!(matches!(parse(&field), Err(InputError::Refused(_))));
    let padded = format!("{RECORDED_RUN}{}", " ".repeat(MAX_DOCUMENT_BYTES));
    assert!(matches!(
        Readings::parse(&padded),
        Err(InputError::TooLong(length)) if length > MAX_DOCUMENT_BYTES
    ));
    assert!(
        InputError::TooLong(padded.len())
            .to_string()
            .contains("65536")
    );
    Ok(())
}
