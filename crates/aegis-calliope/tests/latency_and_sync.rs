// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The graph parameters and the local synchronisation rule (REQ-P08-07,
//! REQ-P08-08).
//!
//! Positive: the recorded defaults produce the buffer size in time the
//! scaffold prints. Negative: a quantum and a rate outside the admitted ranges
//! are refused. Boundary: the quantum range is exact at both ends, and the
//! drift window refuses its 65th sample.
//!
//! **Nothing here is a latency measurement.** Every constant is a
//! [`Declared`] value recorded from a source, and
//! [`quantum_latency_micros`] is arithmetic on two of them.

mod common;

use aegis_calliope::{
    ADMITTED_SAMPLE_RATES_HZ, CalliopeError, DEFAULT_QUANTUM_SAMPLES, DEFAULT_SAMPLE_RATE_HZ,
    DRIFT_DIVISOR, DRIFT_WEIGHT_NEW, DRIFT_WEIGHT_OLD, Declared, DriftEstimator, MAX_DRIFT_SAMPLES,
    MAX_QUANTUM_SAMPLES, MIN_QUANTUM_SAMPLES, PhaseOffsetMicros, Quantum, SampleRate, SyncSource,
    SyncStrategy, TARGET_RTL_LATENCY_MS, quantum_latency_micros,
};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: the recorded defaults produce the figure the scaffold prints.
#[test]
fn the_recorded_defaults_produce_the_scaffold_figure() -> Fallible {
    let quantum = Quantum::new(DEFAULT_QUANTUM_SAMPLES.copied())?;
    let rate = SampleRate::new(DEFAULT_SAMPLE_RATE_HZ.copied())?;
    assert_eq!(quantum.get(), 64);
    assert_eq!(rate.get(), 48_000);
    assert_eq!(quantum_latency_micros(quantum, rate), 1_333);
    Ok(())
}

/// Positive: a declared value says it is declared.
#[test]
fn a_declared_value_says_so() {
    assert_eq!(TARGET_RTL_LATENCY_MS.copied(), 5);
    assert_eq!(*TARGET_RTL_LATENCY_MS.get(), 5);
    assert_eq!(
        TARGET_RTL_LATENCY_MS.to_string(),
        "5 (declared, unmeasured)"
    );
    assert_eq!(Declared::new(48_000u32).copied(), 48_000);
}

/// Positive: the estimator seeds on its first sample and converges after it.
#[test]
fn the_estimator_seeds_then_converges() -> Fallible {
    let mut estimator = DriftEstimator::new();
    assert!(estimator.is_empty());
    assert_eq!(estimator.average_micros(), 0);
    assert_eq!(estimator.observe(PhaseOffsetMicros::new(800))?, 800);
    assert_eq!(estimator.samples(), 1);
    // (800 * 7 + 0) / 8 = 700
    assert_eq!(estimator.observe(PhaseOffsetMicros::new(0))?, 700);
    assert_eq!(estimator.samples(), 2);
    assert_eq!(
        DRIFT_WEIGHT_OLD.saturating_add(DRIFT_WEIGHT_NEW),
        DRIFT_DIVISOR
    );
    Ok(())
}

/// Positive: the admitted strategy instantiates the reading REQ-P08-07 wants.
#[test]
fn the_admitted_strategy_is_the_local_one() {
    assert_eq!(SyncStrategy::ADMITTED, SyncStrategy::LocalPhaseAndDrift);
    assert_eq!(SyncStrategy::ADMITTED.tag(), "local-phase-and-drift");
    assert_eq!(
        SyncStrategy::ADMITTED.source(),
        SyncSource::LocalPhaseAndDrift
    );
    assert!(!SyncSource::LocalPhaseAndDrift.needs_privileged_clock());
    assert!(SyncSource::GlobalMasterClock.needs_privileged_clock());
    assert_eq!(SyncSource::ALL.len(), 2);
    assert_eq!(SyncSource::GlobalMasterClock.name(), "global-master-clock");
    assert_eq!(
        SyncSource::LocalPhaseAndDrift.name(),
        "local-phase-and-drift"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: a rate nothing here reasons about is refused.
#[test]
fn an_unadmitted_sample_rate_is_refused() {
    assert_eq!(
        SampleRate::new(44_101),
        Err(CalliopeError::SampleRateUnsupported { hz: 44_101 })
    );
    assert_eq!(
        SampleRate::new(0),
        Err(CalliopeError::SampleRateUnsupported { hz: 0 })
    );
    for hz in ADMITTED_SAMPLE_RATES_HZ {
        assert!(SampleRate::new(hz).is_ok());
    }
    assert_eq!(ADMITTED_SAMPLE_RATES_HZ.len(), 3);
}

/// Negative: the drift window refuses rather than counting forever.
#[test]
fn the_drift_window_refuses_past_its_bound() -> Fallible {
    let mut estimator = DriftEstimator::new();
    for _ in 0..MAX_DRIFT_SAMPLES {
        estimator.observe(PhaseOffsetMicros::new(-40))?;
    }
    assert_eq!(estimator.samples(), MAX_DRIFT_SAMPLES);
    assert_eq!(
        estimator.observe(PhaseOffsetMicros::new(-40)),
        Err(CalliopeError::DriftWindowFull {
            max: MAX_DRIFT_SAMPLES
        })
    );
    assert_eq!(estimator.samples(), MAX_DRIFT_SAMPLES);
    assert_eq!(PhaseOffsetMicros::new(-40).get(), -40);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the quantum range is exact at both ends.
#[test]
fn the_quantum_range_is_exact_at_both_ends() {
    assert!(Quantum::new(MIN_QUANTUM_SAMPLES).is_ok());
    assert!(Quantum::new(MAX_QUANTUM_SAMPLES).is_ok());
    assert_eq!(
        Quantum::new(MIN_QUANTUM_SAMPLES.saturating_sub(1)),
        Err(CalliopeError::QuantumOutOfRange {
            samples: 15,
            min: MIN_QUANTUM_SAMPLES,
            max: MAX_QUANTUM_SAMPLES,
        })
    );
    assert_eq!(
        Quantum::new(MAX_QUANTUM_SAMPLES.saturating_add(1)),
        Err(CalliopeError::QuantumOutOfRange {
            samples: 8193,
            min: MIN_QUANTUM_SAMPLES,
            max: MAX_QUANTUM_SAMPLES,
        })
    );
}

/// Boundary: the buffer-size arithmetic holds at both ends of the range.
#[test]
fn the_quantum_arithmetic_holds_at_both_ends() -> Fallible {
    let slowest = SampleRate::new(44_100)?;
    let fastest = SampleRate::new(96_000)?;
    assert_eq!(
        quantum_latency_micros(Quantum::new(MIN_QUANTUM_SAMPLES)?, fastest),
        166
    );
    assert_eq!(
        quantum_latency_micros(Quantum::new(MAX_QUANTUM_SAMPLES)?, slowest),
        185_759
    );
    Ok(())
}
