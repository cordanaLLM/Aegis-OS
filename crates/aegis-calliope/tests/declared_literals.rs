// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The falsifier for "no timing figure is produced by this milestone".
//!
//! A claim that a number was never measured is worth what a check can do to
//! it. What this file does is make the three recorded timing constants
//! **unusable as integers**: each is a [`Declared<u32>`], and the positive
//! test below binds each to a `Declared<u32>` pattern. Rewriting any of them
//! as a bare `u32` stops this file compiling, which fails the gate.
//!
//! What it does not claim: that no arithmetic anywhere produces a number with
//! units of time. [`quantum_latency_micros`](aegis_calliope::quantum_latency_micros)
//! does, and returns a bare `u32`, because what it computes is exact
//! arithmetic on two declared constants rather than an observation. The
//! distinction is the point, and the negative test below pins it.
//!
//! Milestone M23 added the opposite wrapper,
//! [`Measured<T>`](aegis_calliope::Measured), for figures a fixture did
//! observe. The two must not meet, so the last test here sweeps the crate's own
//! sources for a conversion between them. That is a check over an enumeration
//! of spellings, not a proof that no conversion could ever be written; what
//! makes it hold in practice is that `Measured::new` demands a
//! `KernelIdentity` and a `MeasurementTool`, neither of which a `Declared<T>`
//! has anything to supply.

mod common;

use aegis_calliope::{
    DEFAULT_QUANTUM_SAMPLES, DEFAULT_SAMPLE_RATE_HZ, Declared, GUEST_WORST_WAKEUP_NS, Quantum,
    SampleRate, TARGET_RTL_LATENCY_MS, quantum_latency_micros,
};

use common::Fallible;

/// Accepts only a value that is already marked as declared rather than
/// observed.
const fn require_declared(value: Declared<u32>) -> u32 {
    value.copied()
}

// --- Positive -------------------------------------------------------------

/// Positive: every recorded timing constant is a declared value.
///
/// This is the falsifier. Replacing any of the three with a bare integer
/// stops this test compiling.
#[test]
fn every_recorded_timing_constant_is_declared() {
    assert_eq!(require_declared(TARGET_RTL_LATENCY_MS), 5);
    assert_eq!(require_declared(DEFAULT_SAMPLE_RATE_HZ), 48_000);
    assert_eq!(require_declared(DEFAULT_QUANTUM_SAMPLES), 64);
}

/// Positive: rendering a declared value says it is unmeasured, so a figure
/// copied out of a log carries its own provenance.
#[test]
fn rendering_a_declared_value_says_it_is_unmeasured() {
    for rendered in [
        TARGET_RTL_LATENCY_MS.to_string(),
        DEFAULT_SAMPLE_RATE_HZ.to_string(),
        DEFAULT_QUANTUM_SAMPLES.to_string(),
    ] {
        assert!(
            rendered.ends_with("(declared, unmeasured)"),
            "{rendered} does not carry its provenance"
        );
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the one arithmetic result that is a bare integer is not a
/// measurement either, and this file says which one it is.
///
/// A quantum's duration follows from the quantum and the rate by division. It
/// is exact, it is not observed, and it is not the round-trip latency the
/// report targets -- which is why the target is wrapped and this is not.
#[test]
fn the_arithmetic_result_is_not_the_targeted_latency() -> Fallible {
    let quantum = Quantum::new(DEFAULT_QUANTUM_SAMPLES.copied())?;
    let rate = SampleRate::new(DEFAULT_SAMPLE_RATE_HZ.copied())?;
    let micros = quantum_latency_micros(quantum, rate);
    let target_micros = TARGET_RTL_LATENCY_MS.copied().saturating_mul(1_000);
    assert_eq!(micros, 1_333);
    assert_eq!(target_micros, 5_000);
    assert_ne!(
        micros, target_micros,
        "the buffer duration is not the round-trip latency, and must not be read as one"
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a declared wrapper holds the value it was given and nothing else,
/// at the extremes of the type it wraps.
#[test]
fn the_wrapper_is_transparent_at_the_extremes() {
    assert_eq!(Declared::new(0u32).copied(), 0);
    assert_eq!(Declared::new(u32::MAX).copied(), u32::MAX);
    assert_eq!(*Declared::new(u32::MAX).get(), u32::MAX);
    assert_eq!(Declared::new(0u32).to_string(), "0 (declared, unmeasured)");
    assert_eq!(Declared::new(7u32), Declared::new(7u32));
    assert_ne!(Declared::new(7u32), Declared::new(8u32));
}

/// Boundary: the declared wrapper and the measured wrapper render differently
/// and are not convertible into one another anywhere in this crate.
///
/// The sweep is over six spellings of a conversion. A seventh would be
/// invisible to it, which is why the rendering assertions sit beside it: a
/// figure that lost its provenance on the way through a conversion would stop
/// carrying either marker.
#[test]
fn a_declared_value_and_a_measured_one_do_not_convert() -> Fallible {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let watched = [
        "impl From<Declared",
        "impl From<Measured",
        "> for Measured",
        "> for Declared",
        "fn into_measured",
        "fn into_declared",
    ];
    let found = common::scan_sources(&root, &watched)?;
    assert!(
        found.is_empty(),
        "a conversion between the two wrappers exists: {found:?}"
    );
    assert!(
        TARGET_RTL_LATENCY_MS
            .to_string()
            .ends_with("(declared, unmeasured)")
    );
    assert!(GUEST_WORST_WAKEUP_NS.to_string().contains("measured by"));
    assert!(!GUEST_WORST_WAKEUP_NS.to_string().contains("unmeasured"));
    Ok(())
}
