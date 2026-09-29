// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The cases a readings document is judged on (M21, E21-1).
//!
//! Positive: the recorded run passes every case and its SCI line carries the
//! hand-computed figures. Negative: a readable unprivileged read, a failed
//! privileged read and a range other than the recorded one each fail their
//! case, and a watch with no wrap fails the wrap case. Boundary: a watch that
//! did wrap passes it, and a wrapped draw outside the band is refused as
//! absurd.

mod common;

use aegis_tellus_rapl::{
    ACCURACY, Evaluation, OBSERVED_WRAP_CASE, PAIR_CASES, Readings, SEAM_TOLERANCE, SERVICE_MILLIS,
    WRAP_BAND, evaluate,
};
use serde_json::{Value, json};

use common::{Fallible, RECORDED_RUN, reading, recorded, set, watch};

/// Judges a JSON value.
fn judge(value: &Value) -> Result<Evaluation, Box<dyn std::error::Error>> {
    Ok(evaluate(&Readings::parse(&value.to_string())?)?)
}

/// Returns `true` when `run` printed `verdict` for `case`.
fn printed(run: &Evaluation, verdict: &str, case: &str) -> bool {
    run.lines()
        .iter()
        .any(|line| line == &format!("{verdict} {case}"))
}

// --- Positive -------------------------------------------------------------

/// Positive (E21-1): the recorded run passes every case, and the SCI is the
/// one computed from its measured energy.
#[test]
fn the_recorded_run_passes_every_case() -> Fallible {
    let run = evaluate(&Readings::parse(RECORDED_RUN)?)?;
    assert_eq!(run.failed(), 0, "{:#?}", run.lines());
    assert_eq!(run.passed(), u32::try_from(PAIR_CASES.len())?);
    for case in PAIR_CASES {
        assert!(printed(&run, "PASS", case), "{case}: {:#?}", run.lines());
    }
    let text = run.lines().join("\n");
    assert!(text.contains("delta 592707218 uJ"));
    assert!(text.contains("SCI 0.0862209966555"));
    assert!(text.contains("provenance measured"));
    assert!(text.contains(&format!("info accuracy: {ACCURACY}")));
    assert!(!printed(&run, "PASS", OBSERVED_WRAP_CASE));
    Ok(())
}

/// Positive: the recorded constants are the ones the run is judged with.
#[test]
fn the_recorded_constants_hold() {
    assert_eq!(SERVICE_MILLIS, 1);
    assert!((SEAM_TOLERANCE - 1.0e-9).abs() < f64::EPSILON);
    assert!((WRAP_BAND - 2.0).abs() < f64::EPSILON);
    assert!(ACCURACY.contains("model-based estimate"));
    assert!(ACCURACY.contains("same-zone deltas"));
    assert_eq!(PAIR_CASES.len(), 6);
}

// --- Negative -------------------------------------------------------------

/// Negative: an unprivileged read that returned text fails the case: the
/// counter would not be root-only.
#[test]
fn a_readable_unprivileged_counter_fails() -> Fallible {
    let mut document = recorded()?;
    set(&mut document, "unprivileged", reading("45514845753\n", 1));
    let run = judge(&document)?;
    assert!(printed(&run, "FAIL", "rapl/unprivileged-read-fails-closed"));
    assert!(
        run.lines()
            .iter()
            .any(|line| line.contains("energy_uj is not root-only here")),
        "{:#?}",
        run.lines()
    );
    assert_eq!(run.failed(), 1);
    Ok(())
}

/// Negative (E21-1): a privileged read that failed is an error in every case
/// that needs it, never a zero reading that passes.
#[test]
fn a_failed_privileged_read_fails_closed() -> Fallible {
    let mut document = recorded()?;
    set(
        &mut document,
        "readings",
        json!([
            {"error": "sudo: a password is required", "monotonic-ns": 1},
            reading("46107552971\n", 2),
        ]),
    );
    let run = judge(&document)?;
    for case in [
        "rapl/measured-sci",
        "rapl/dram-zone-refused",
        "rapl/psys-zone-refused",
        "rapl/wrap-at-live-range",
    ] {
        assert!(printed(&run, "FAIL", case), "{case}: {:#?}", run.lines());
    }
    assert!(run.lines().join("\n").contains("unreadable counter"));
    Ok(())
}

/// Negative: a live range other than the recorded one fails its case.
#[test]
fn another_range_fails_the_recorded_range_case() -> Fallible {
    let mut document = recorded()?;
    set(
        &mut document,
        "max-energy-range-uj",
        json!("262143328850\n"),
    );
    let run = judge(&document)?;
    assert!(printed(&run, "FAIL", "rapl/recorded-range"));
    assert!(printed(&run, "PASS", "rapl/measured-sci"));
    Ok(())
}

/// Negative: a document naming no powercap zone is refused before any case.
#[test]
fn an_unknown_zone_is_refused_before_any_case() -> Fallible {
    let mut document = recorded()?;
    set(&mut document, "zone", json!("uncore\n"));
    let parsed = Readings::parse(&document.to_string())?;
    assert!(evaluate(&parsed).is_err());
    Ok(())
}

/// Negative: a watch that saw no wrap fails the wrap case.
#[test]
fn a_watch_without_a_wrap_fails() -> Fallible {
    let run = judge(&watch(&[1_000_000_000, 2_000_000_000, 3_000_000_000]))?;
    assert!(
        printed(&run, "FAIL", OBSERVED_WRAP_CASE),
        "{:#?}",
        run.lines()
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary (E21-1): a watch that saw the counter wrap passes the wrap case,
/// and the SCI is computed over the wrapped pair.
#[test]
fn a_watch_across_the_wrap_passes() -> Fallible {
    let values = [
        62_532_610_987,
        63_532_610_987,
        64_532_610_987,
        50_000_000,
        1_050_000_000,
    ];
    let run = judge(&watch(&values))?;
    assert_eq!(run.failed(), 0, "{:#?}", run.lines());
    assert!(printed(&run, "PASS", OBSERVED_WRAP_CASE));
    let text = run.lines().join("\n");
    assert!(
        text.contains("pair 2 wrapped: 1050000000 uJ, wrapped once"),
        "{text}"
    );
    assert!(text.contains("pair 2: energy_uj"), "{text}");
    Ok(())
}

/// Boundary (E21-1): the wrap run `r20260929T201527-d65a`, replayed from its
/// seven readings, passes every case, the observed wrap included.
#[test]
fn the_recorded_wrap_run_passes() -> Fallible {
    let values = [
        59_374_959_060,
        60_770_719_871,
        62_074_613_409,
        63_426_207_666,
        64_527_611_843,
        121_324_200,
        1_297_006_778,
    ];
    let stamps = [
        172_771_206_458_534_u64,
        172_781_223_081_348,
        172_791_230_522_245,
        172_801_237_752_947,
        172_811_244_783_378,
        172_821_251_579_865,
        172_831_258_044_999,
    ];
    let mut document = watch(&values);
    let readings: Vec<Value> = values
        .iter()
        .zip(stamps)
        .map(|(value, stamp)| reading(&format!("{value}\n"), stamp))
        .collect();
    set(&mut document, "readings", Value::Array(readings));
    let run = judge(&document)?;
    assert_eq!(run.failed(), 0, "{:#?}", run.lines());
    let text = run.lines().join("\n");
    assert!(
        text.contains("pair 4 wrapped: 1126323344 uJ, wrapped once"),
        "{text}"
    );
    assert!(text.contains("10006796487 ns apart"), "{text}");
    assert!(text.contains("SCI 0.1188308710"), "{text}");
    Ok(())
}

/// Boundary: a wrapped draw outside the band around the median is absurd.
#[test]
fn a_wrapped_draw_outside_the_band_fails() -> Fallible {
    let values = [
        62_532_610_987,
        63_532_610_987,
        64_532_610_987,
        3_000_000_000,
        4_000_000_000,
    ];
    let run = judge(&watch(&values))?;
    assert!(
        printed(&run, "FAIL", OBSERVED_WRAP_CASE),
        "{:#?}",
        run.lines()
    );
    assert!(run.lines().join("\n").contains("against a median of"));
    Ok(())
}
