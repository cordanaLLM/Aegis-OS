// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The graph parameters, and the arithmetic that follows from them
//! (REQ-P08-08).
//!
//! The scaffold sets a quantum of 64 samples at 48000 Hz, prints "RTL Latency:
//! ~1.33ms" and names a 5 ms round-trip-latency target. Two of those three are
//! arithmetic on declared constants and one is a target; none is a
//! measurement, and this crate cannot produce one.
//!
//! # Why every figure here is wrapped
//!
//! [`Declared<T>`] exists so that a target cannot be read back as an
//! observation. Its [`Display`](core::fmt::Display) renders `5 (declared,
//! unmeasured)`, and `tests/declared_literals.rs` fails to compile if one of
//! the three constants becomes a bare integer. A round-trip latency is a
//! property of a running graph on a scheduled thread, and **nothing in this
//! module is a measurement of one**.
//!
//! Milestone M23 did measure a kernel, and the figures live in
//! [`crate::measured`] as [`Measured<T>`](crate::measured::Measured) values
//! carrying the kernel that produced them. The three constants here stay
//! declared: what M23 measured is a wakeup latency on a kernel, not the
//! round-trip latency export-017 asks for, and the 5 ms figure is still a
//! target rather than a reading.
//!
//! [`quantum_latency_micros`] is the one figure here that is neither a target
//! nor a reading: it is exact arithmetic on two declared constants, and it is
//! returned unwrapped because what it states is a buffer size in time, not a
//! latency anyone observed.

use core::fmt;

use crate::error::CalliopeError;

/// A value recorded from a source, which nothing in this repository measured.
///
/// The wrapper is the point: a bare `u32` of 5 is indistinguishable from a
/// measured 5, and `tests/declared_literals.rs` is what stops one becoming the
/// other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Declared<T> {
    value: T,
}

impl<T> Declared<T> {
    /// Records `value` as declared rather than observed.
    #[must_use]
    pub const fn new(value: T) -> Self {
        Self { value }
    }

    /// Returns the declared value.
    #[must_use]
    pub const fn get(&self) -> &T {
        &self.value
    }
}

impl<T: Copy> Declared<T> {
    /// Returns a copy of the declared value.
    #[must_use]
    pub const fn copied(self) -> T {
        self.value
    }
}

impl<T: fmt::Display> fmt::Display for Declared<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} (declared, unmeasured)", self.value)
    }
}

/// The round-trip-latency target the P08 report states, in milliseconds.
///
/// Recorded from export-017 `afbc0af8056d` (REQ-P08-08). A target is not a
/// measurement, and no measurement exists.
pub const TARGET_RTL_LATENCY_MS: Declared<u32> = Declared::new(5);

/// The sample rate the report standardises on, in hertz.
///
/// Recorded from export-017 `afbc0af8056d`: `default.clock.rate` 48000,
/// chosen there to eliminate resampling energy.
pub const DEFAULT_SAMPLE_RATE_HZ: Declared<u32> = Declared::new(48_000);

/// The graph quantum the report forces, in samples.
///
/// Recorded from export-017 `afbc0af8056d`: `clock.force-quantum` 64.
pub const DEFAULT_QUANTUM_SAMPLES: Declared<u32> = Declared::new(64);

/// The smallest quantum this build admits, in samples.
pub const MIN_QUANTUM_SAMPLES: u32 = 16;

/// The largest quantum this build admits, in samples.
pub const MAX_QUANTUM_SAMPLES: u32 = 8192;

/// The sample rates this build admits, in hertz.
pub const ADMITTED_SAMPLE_RATES_HZ: [u32; 3] = [44_100, 48_000, 96_000];

/// A validated graph quantum, in samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Quantum(u32);

impl Quantum {
    /// Validates `samples` against `MIN_QUANTUM_SAMPLES..=MAX_QUANTUM_SAMPLES`.
    ///
    /// # Errors
    ///
    /// Returns [`CalliopeError::QuantumOutOfRange`] outside that range.
    pub const fn new(samples: u32) -> Result<Self, CalliopeError> {
        if samples < MIN_QUANTUM_SAMPLES || samples > MAX_QUANTUM_SAMPLES {
            return Err(CalliopeError::QuantumOutOfRange {
                samples,
                min: MIN_QUANTUM_SAMPLES,
                max: MAX_QUANTUM_SAMPLES,
            });
        }
        Ok(Self(samples))
    }

    /// Returns the validated quantum.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A validated sample rate, in hertz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SampleRate(u32);

impl SampleRate {
    /// Validates `hz` against [`ADMITTED_SAMPLE_RATES_HZ`].
    ///
    /// # Errors
    ///
    /// Returns [`CalliopeError::SampleRateUnsupported`] for any other rate, so
    /// a graph cannot be described at a rate nothing here reasons about.
    pub fn new(hz: u32) -> Result<Self, CalliopeError> {
        if ADMITTED_SAMPLE_RATES_HZ.contains(&hz) {
            return Ok(Self(hz));
        }
        Err(CalliopeError::SampleRateUnsupported { hz })
    }

    /// Returns the validated rate.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// Returns how long one quantum lasts at `rate`, in microseconds, truncated.
///
/// This is arithmetic on two validated values and nothing else: it is not a
/// latency anyone observed, and it is not the round-trip latency the report
/// targets, which also includes the device and the graph. At the recorded
/// defaults it is 1333 microseconds, which is where the scaffold's printed
/// "~1.33ms" comes from.
#[must_use]
pub fn quantum_latency_micros(quantum: Quantum, rate: SampleRate) -> u32 {
    let numerator = u64::from(quantum.get()).saturating_mul(1_000_000);
    let micros = numerator.checked_div(u64::from(rate.get())).unwrap_or(0);
    u32::try_from(micros).unwrap_or(u32::MAX)
}
