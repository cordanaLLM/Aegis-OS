// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Milestone M21, epic E21-1: measured energy behind the M05 seam.
//!
//! Positive: the SCI rate is computed from measured energy. The two readings
//! below are the ones `make verify-workstation` took on the reference profile
//! in run `r20260929T200422-2476` (2026-09-29, AMD Ryzen 9 9950X3D, kernel
//! 7.2.8-1-cachyos) through the one scoped `sudo -n cat`, and the expected
//! figures are hand-computed from them, not read back from the code.
//! Negative: an unreadable, empty or malformed counter fails closed with an
//! error and never a default value, and the `dram` and `psys` zones are
//! refused (D60). Boundary: a wrap at the recorded `max_energy_range_uj` of
//! 65532610987 uJ yields a correct positive delta, and the edges of every
//! bound -- the range, the text length, the unambiguous interval -- are held
//! on both sides.

mod common;

use aegis_tellus::{
    CounterPair, CounterRead, EnergyDelta, EnergyRange, EnergyReading, MAX_COUNTER_TEXT_BYTES,
    MAX_INTERVAL_NANOS, MAX_PLAUSIBLE_ZONE_WATTS, MICROJOULES_PER_JOULE, MeasuredWattage,
    NANOS_PER_SECOND, Provenance, REFERENCE_MAX_ENERGY_RANGE_UJ, RaplZone, SampleDeadline,
    SampleInterval, SciEngine, TellusError, WattageSource, unambiguous_interval_seconds,
};

use core::num::NonZeroU32;

use common::{EPSILON, Fallible, deadline};

/// Five milliseconds, the deadline `common::deadline` offers.
const FIVE_MILLIS: NonZeroU32 = match NonZeroU32::new(5) {
    Some(value) => value,
    None => NonZeroU32::MIN,
};

/// The deadline every source here declares as its service time.
const SERVICE: SampleDeadline = SampleDeadline::from_millis(FIVE_MILLIS);

/// The earlier reading of run r20260929T200422-2476, in microjoules.
const RUN_BEFORE_UJ: u64 = 45_514_845_753;

/// The later reading of the same run.
const RUN_AFTER_UJ: u64 = 46_107_552_971;

/// The interval between the two reads' midpoints, in nanoseconds.
const RUN_INTERVAL_NANOS: u64 = 5_015_051_212;

/// `46107552971 - 45514845753`.
const RUN_DELTA_UJ: u64 = 592_707_218;

/// `592.707218 J / 5.015051212 s`.
const RUN_WATTS: f64 = 118.185_676_066_830_95;

/// `592.707218 J / 3.6e6 J/kWh`.
const RUN_KWH: f64 = 1.646_408_938_888_889e-4;

/// `(RUN_KWH * 220 + 0.05) / 1`, the scaffold's intensity and embodied carbon.
const RUN_SCI: f64 = 0.086_220_996_655_555_56;

/// Builds the recorded run's pair.
fn run_pair() -> Result<CounterPair, TellusError> {
    let range = EnergyRange::REFERENCE;
    Ok(CounterPair {
        before: EnergyReading::parse("45514845753\n", range)?,
        after: EnergyReading::parse("46107552971\n", range)?,
        interval: SampleInterval::from_nanos(RUN_INTERVAL_NANOS)?,
    })
}

/// Builds a pair of raw values `nanos` apart against the reference range.
fn pair(before: u64, after: u64, nanos: u64) -> Result<CounterPair, TellusError> {
    let range = EnergyRange::REFERENCE;
    Ok(CounterPair {
        before: EnergyReading::new(before, range)?,
        after: EnergyReading::new(after, range)?,
        interval: SampleInterval::from_nanos(nanos)?,
    })
}

/// Builds a package-0 source over `pair`.
fn package(pair: CounterPair) -> Result<MeasuredWattage, TellusError> {
    MeasuredWattage::from_counters(RaplZone::Package0, pair, EnergyRange::REFERENCE, SERVICE)
}

/// The relative difference of two positive figures.
fn relative(actual: f64, expected: f64) -> f64 {
    (actual - expected).abs() / expected
}

// --- Positive -------------------------------------------------------------

/// Positive (E21-1): the SCI rate is computed from measured energy, through
/// the seam and the unchanged engine, and matches the hand computation.
#[test]
fn sci_is_computed_from_measured_energy() -> Fallible {
    let source = package(run_pair()?)?;
    let sample = source.sample(RaplZone::Package0, deadline()?)?;
    assert_eq!(sample.provenance, Provenance::Measured);
    assert!(sample.provenance.is_measured());
    assert!(relative(sample.watts.get(), RUN_WATTS) < EPSILON);
    let energy = sample.energy(source.interval().seconds()?)?;
    assert!(relative(energy.get(), RUN_KWH) < 1.0e-9);
    let sci = SciEngine::new().sci_rate(energy, 1.0);
    assert!(relative(sci.rate(), RUN_SCI) < 1.0e-9);
    assert!(!sci.fell_back());
    assert!(sci.as_rate().is_ok());
    Ok(())
}

/// Positive: the delta and the energy read straight off it agree with the
/// energy the seam derives from the draw.
#[test]
fn the_delta_and_the_seam_agree() -> Fallible {
    let source = package(run_pair()?)?;
    let delta = source.delta();
    assert_eq!(delta.microjoules(), RUN_DELTA_UJ);
    assert!(!delta.wrapped());
    assert!((delta.joules() - 592.707_218).abs() < EPSILON);
    assert!(relative(delta.energy()?.get(), RUN_KWH) < EPSILON);
    assert_eq!(source.interval().nanos(), RUN_INTERVAL_NANOS);
    assert_eq!(source.measured().zone, RaplZone::Package0);
    assert_eq!(format!("{delta}"), "592707218 uJ");
    Ok(())
}

/// Positive: a counter read's text parses, with or without its newline.
#[test]
fn a_counter_read_parses() -> Fallible {
    let range = EnergyRange::REFERENCE;
    assert_eq!(
        EnergyReading::parse("45514845753\n", range)?.microjoules(),
        RUN_BEFORE_UJ
    );
    assert_eq!(
        EnergyReading::parse("46107552971", range)?.microjoules(),
        RUN_AFTER_UJ
    );
    let read = EnergyReading::from_read(CounterRead::Text("0\n"), range)?;
    assert_eq!(read.microjoules(), 0);
    assert_eq!(
        EnergyRange::parse("65532610987\n")?.microjoules(),
        REFERENCE_MAX_ENERGY_RANGE_UJ
    );
    Ok(())
}

/// Positive: the measured source declares one zone and its service time.
#[test]
fn the_measured_source_declares_one_zone() -> Fallible {
    let source = package(run_pair()?)?;
    let zones = source.zones();
    assert_eq!(zones.len(), 1);
    assert!(zones.contains(RaplZone::Package0));
    assert!(!zones.contains(RaplZone::Core));
    assert_eq!(source.service_time(), deadline()?);
    let core = MeasuredWattage::from_counters(
        RaplZone::Core,
        run_pair()?,
        EnergyRange::REFERENCE,
        deadline()?,
    )?;
    assert!(core.sample(RaplZone::Core, deadline()?).is_ok());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative (E21-1): an unreadable counter fails closed with an error, and
/// nothing in the API turns a failed read into a reading of zero.
#[test]
fn an_unreadable_counter_fails_closed() {
    let range = EnergyRange::REFERENCE;
    let refused = EnergyReading::from_read(CounterRead::Unreadable, range);
    assert!(
        matches!(refused, Err(TellusError::Counter { .. })),
        "{refused:?}"
    );
    let message = refused.map(|_| ()).err().map(|error| error.to_string());
    assert!(
        message
            .as_deref()
            .is_some_and(|text| text.contains("no default value")),
        "{message:?}"
    );
}

/// Negative: text a failed or garbled read leaves behind is refused.
#[test]
fn a_malformed_read_is_refused() {
    let range = EnergyRange::REFERENCE;
    for text in [
        "", "\n", "12a\n", "-1\n", " 12\n", "12\n\n", "1.5\n", "0x10\n", "12 34\n",
    ] {
        let refused = EnergyReading::parse(text, range);
        assert!(
            matches!(refused, Err(TellusError::Counter { .. })),
            "{text:?} gave {refused:?}"
        );
    }
    assert!(EnergyReading::parse("99999999999999999999\n", range).is_err());
    assert!(EnergyRange::parse("0\n").is_err());
    assert!(EnergyRange::new(0).is_err());
}

/// Negative (D60): a dram or a psys source is refused outright, and a
/// package-0 source refuses to answer for either.
#[test]
fn dram_and_psys_are_refused() -> Fallible {
    for zone in [RaplZone::Dram, RaplZone::Psys] {
        let built =
            MeasuredWattage::from_counters(zone, run_pair()?, EnergyRange::REFERENCE, deadline()?);
        assert!(
            matches!(built, Err(TellusError::ZoneAbsent { carried: 2, .. })),
            "{zone}: {built:?}"
        );
        let source = package(run_pair()?)?;
        let sampled = source.sample(zone, deadline()?);
        assert!(
            matches!(sampled, Err(TellusError::ZoneAbsent { carried: 1, .. })),
            "{zone}: {sampled:?}"
        );
    }
    Ok(())
}

/// Negative: a counter reset read as a wrap implies a draw no package reaches
/// and is refused rather than reported.
#[test]
fn a_counter_reset_is_refused() -> Fallible {
    let reset = package(pair(RUN_AFTER_UJ, 1_000, 1_000_000_000)?);
    assert!(
        matches!(reset, Err(TellusError::Wattage { .. })),
        "{reset:?}"
    );
    Ok(())
}

/// Negative: a deadline shorter than the declared service time would block.
#[test]
fn a_short_deadline_is_refused() -> Fallible {
    let slow = SampleDeadline::try_from_millis(50).ok_or("positive")?;
    let source = MeasuredWattage::from_counters(
        RaplZone::Package0,
        run_pair()?,
        EnergyRange::REFERENCE,
        slow,
    )?;
    let refused = source.sample(RaplZone::Package0, deadline()?);
    assert!(
        matches!(
            refused,
            Err(TellusError::WouldBlock {
                needed: 50,
                offered: 5
            })
        ),
        "{refused:?}"
    );
    Ok(())
}

/// Negative: a zero interval, a zero range and a reading above the range are
/// not values at all.
#[test]
fn degenerate_inputs_are_not_values() {
    assert!(matches!(
        SampleInterval::from_nanos(0),
        Err(TellusError::Interval { .. })
    ));
    let small = EnergyRange::new(10);
    assert!(small.is_ok());
    let above = small.and_then(|range| EnergyReading::new(11, range));
    assert!(matches!(above, Err(TellusError::Counter { .. })));
}

// --- Boundary -------------------------------------------------------------

/// Boundary (E21-1): a wrap at the recorded range between two samples yields
/// the correct positive delta, not a negative or an absurd one.
#[test]
fn a_wrap_at_the_recorded_range_yields_a_correct_positive_delta() -> Fallible {
    let range = EnergyRange::REFERENCE;
    let before = EnergyReading::new(REFERENCE_MAX_ENERGY_RANGE_UJ - 400_000_000, range)?;
    let after = EnergyReading::new(192_707_218, range)?;
    let delta = EnergyDelta::between(before, after, range)?;
    assert!(delta.wrapped());
    assert_eq!(delta.microjoules(), RUN_DELTA_UJ);
    let wrapped = package(CounterPair {
        before,
        after,
        interval: SampleInterval::from_nanos(RUN_INTERVAL_NANOS)?,
    })?;
    let sample = wrapped.sample(RaplZone::Package0, deadline()?)?;
    assert!(relative(sample.watts.get(), RUN_WATTS) < EPSILON);
    assert_eq!(format!("{}", wrapped.delta()), "592707218 uJ, wrapped once");
    Ok(())
}

/// Boundary (E21-1), on the counter itself: the wrap run
/// `r20260929T201527-d65a` read 64527611843 then 121324200 ten seconds apart;
/// the wrapped delta is `65532610987 - 64527611843 + 121324200`, a draw in
/// line with the unwrapped pairs around it, and SCI is computed across it.
#[test]
fn the_observed_wrap_yields_a_correct_positive_delta() -> Fallible {
    let wrapped = package(pair(64_527_611_843, 121_324_200, 10_006_796_487)?)?;
    assert!(wrapped.delta().wrapped());
    assert_eq!(wrapped.delta().microjoules(), 1_126_323_344);
    let sample = wrapped.sample(RaplZone::Package0, deadline()?)?;
    assert!(relative(sample.watts.get(), 112.555_835_972_403_93) < EPSILON);
    let energy = sample.energy(wrapped.interval().seconds()?)?;
    let sci = SciEngine::new().sci_rate(energy, 1.0);
    assert!(relative(sci.rate(), 0.118_830_871_022_222_21) < 1.0e-9);
    Ok(())
}

/// Boundary: the edges of the wrap itself.
///
/// Equal readings are a zero delta without a wrap. From the range itself to
/// zero is a wrap of zero microjoules by the formula: the true increment is
/// one counter unit (15.258 uJ on the reference profile), which the module
/// documentation states as the formula's bound rather than hiding. From zero
/// to the range, and from one below the range to zero, are the largest
/// unwrapped and the smallest wrapped deltas.
#[test]
fn the_edges_of_the_wrap_are_exact() -> Fallible {
    let range = EnergyRange::REFERENCE;
    let at = |value| EnergyReading::new(value, range);
    let limit = REFERENCE_MAX_ENERGY_RANGE_UJ;
    let same = EnergyDelta::between(at(7)?, at(7)?, range)?;
    assert_eq!((same.microjoules(), same.wrapped()), (0, false));
    let from_top = EnergyDelta::between(at(limit)?, at(0)?, range)?;
    assert_eq!((from_top.microjoules(), from_top.wrapped()), (0, true));
    let full = EnergyDelta::between(at(0)?, at(limit)?, range)?;
    assert_eq!((full.microjoules(), full.wrapped()), (limit, false));
    let one = EnergyDelta::between(at(limit - 1)?, at(0)?, range)?;
    assert_eq!((one.microjoules(), one.wrapped()), (1, true));
    Ok(())
}

/// Boundary: a reading at exactly the range is admitted and one above is not;
/// a delta taken against a smaller range than its readings is refused.
#[test]
fn the_range_is_closed_at_its_edge() -> Fallible {
    let range = EnergyRange::REFERENCE;
    assert!(EnergyReading::new(REFERENCE_MAX_ENERGY_RANGE_UJ, range).is_ok());
    assert!(EnergyReading::new(REFERENCE_MAX_ENERGY_RANGE_UJ + 1, range).is_err());
    let small = EnergyRange::new(100)?;
    let high = EnergyReading::new(1_000, range)?;
    let low = EnergyReading::new(10, range)?;
    assert!(matches!(
        EnergyDelta::between(high, low, small),
        Err(TellusError::Counter { .. })
    ));
    assert_eq!(range.microjoules(), REFERENCE_MAX_ENERGY_RANGE_UJ);
    assert!((range.joules() - 65_532.610_987).abs() < 1.0e-9);
    Ok(())
}

/// Boundary: the longest well-formed read is admitted and one byte more is not.
#[test]
fn the_text_bound_is_closed_at_its_edge() -> Fallible {
    let widest = EnergyRange::new(u64::MAX)?;
    let largest = format!("{}\n", u64::MAX);
    assert_eq!(largest.len(), MAX_COUNTER_TEXT_BYTES);
    assert_eq!(
        EnergyReading::parse(&largest, widest)?.microjoules(),
        u64::MAX
    );
    assert!(EnergyReading::parse(&format!("0{largest}"), widest).is_err());
    Ok(())
}

/// Boundary: an interval a draw of [`MAX_PLAUSIBLE_ZONE_WATTS`] could wrap the
/// counter within is refused, and one just short of it is admitted.
#[test]
fn the_unambiguous_interval_is_closed_at_its_edge() -> Fallible {
    let limit = unambiguous_interval_seconds(EnergyRange::REFERENCE);
    assert!((limit - 65.532_610_987).abs() < 1.0e-9);
    assert!((MAX_PLAUSIBLE_ZONE_WATTS - 1_000.0).abs() < EPSILON);
    let admitted = package(pair(0, 1_000_000, 65_000_000_000)?);
    assert!(admitted.is_ok(), "{admitted:?}");
    let refused = package(pair(0, 1_000_000, 65_600_000_000)?);
    assert!(
        matches!(refused, Err(TellusError::Interval { .. })),
        "{refused:?}"
    );
    assert!(SampleInterval::from_nanos(MAX_INTERVAL_NANOS).is_ok());
    assert!(SampleInterval::from_nanos(MAX_INTERVAL_NANOS + 1).is_err());
    assert!((MICROJOULES_PER_JOULE - 1.0e6).abs() < EPSILON);
    assert!((NANOS_PER_SECOND - 1.0e9).abs() < EPSILON);
    Ok(())
}

/// Boundary: a draw exactly at the plausibility bound is admitted.
#[test]
fn a_draw_at_the_bound_is_admitted() -> Fallible {
    let at_bound = package(pair(0, 1_000_000_000, 1_000_000_000)?)?;
    let sample = at_bound.sample(RaplZone::Package0, deadline()?)?;
    assert!((sample.watts.get() - MAX_PLAUSIBLE_ZONE_WATTS).abs() < EPSILON);
    let over = package(pair(0, 1_000_000_001, 1_000_000_000)?);
    assert!(matches!(over, Err(TellusError::Wattage { .. })), "{over:?}");
    Ok(())
}
