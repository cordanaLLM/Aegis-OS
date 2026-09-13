// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Local phase-and-drift synchronisation, without a privileged master clock
//! (REQ-P08-07).
//!
//! The P08 report requires audio/video synchronisation to come from **local**
//! phase and drift estimation rather than from a privileged global master
//! clock, on the ground that a global clock costs synchronisation energy
//! (export-017 `afbc0af8056d`). [`SyncStrategy`] has one variant, so a payload
//! naming a global master clock does not decode; the rejected reading stays
//! representable in [`SyncSource`], which is the register rather than the
//! wire.
//!
//! # The estimator
//!
//! [`DriftEstimator`] is the same shape as the scheduler's burst average: an
//! integer exponentially weighted moving average with a power-of-two divisor,
//! so there is no floating point and no rounding drift of its own. The first
//! sample seeds the average; every later sample is
//! `(old * 7 + new) / 8`. The window is bounded at [`MAX_DRIFT_SAMPLES`], so
//! the estimator refuses rather than counting forever.
//!
//! # What this module does not do
//!
//! **No clock is read.** There is no `Instant`, no `SystemTime`, no
//! `clock_gettime` and no device. A phase offset is a number a caller hands in;
//! where a real one would come from is a running graph, which is milestone
//! M23. Nothing here produces a latency or determinism figure.

use crate::error::CalliopeError;

/// Scalar upper bound on the samples one estimator accepts.
pub const MAX_DRIFT_SAMPLES: usize = 64;

/// The weight the running average keeps.
pub const DRIFT_WEIGHT_OLD: i64 = 7;

/// The weight one new sample carries.
pub const DRIFT_WEIGHT_NEW: i64 = 1;

/// The divisor, `DRIFT_WEIGHT_OLD + DRIFT_WEIGHT_NEW`, a power of two.
pub const DRIFT_DIVISOR: i64 = 8;

/// Where a synchronisation reference may come from.
///
/// Both readings stay representable so the register states a choice rather
/// than asserting the only option it can spell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SyncSource {
    /// Local phase and drift estimation, per REQ-P08-07.
    LocalPhaseAndDrift,
    /// A privileged global master clock, which the report rejects.
    GlobalMasterClock,
}

impl SyncSource {
    /// Both readings, in the order the report puts them.
    pub const ALL: [Self; 2] = [Self::LocalPhaseAndDrift, Self::GlobalMasterClock];

    /// Returns the stable name this reading is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::LocalPhaseAndDrift => "local-phase-and-drift",
            Self::GlobalMasterClock => "global-master-clock",
        }
    }

    /// Returns `true` when the reading needs a privileged system-wide clock.
    #[must_use]
    pub const fn needs_privileged_clock(self) -> bool {
        matches!(self, Self::GlobalMasterClock)
    }
}

/// The synchronisation strategy a payload may name.
///
/// One variant, on purpose: the same device decision D06 used for the capsule
/// runtime. A descriptor naming a global master clock fails to decode rather
/// than being merely discouraged.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum SyncStrategy {
    /// Local phase and drift estimation.
    #[serde(rename = "local-phase-and-drift")]
    LocalPhaseAndDrift,
}

impl SyncStrategy {
    /// The strategy REQ-P08-07 requires, and the only one this build admits.
    pub const ADMITTED: Self = Self::LocalPhaseAndDrift;

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::LocalPhaseAndDrift => "local-phase-and-drift",
        }
    }

    /// Returns the register reading this strategy instantiates.
    #[must_use]
    pub const fn source(self) -> SyncSource {
        match self {
            Self::LocalPhaseAndDrift => SyncSource::LocalPhaseAndDrift,
        }
    }
}

/// A phase offset between two local streams, in microseconds.
///
/// Signed: a stream may run early as well as late, and folding the sign away
/// would make an estimator that cannot tell one from the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PhaseOffsetMicros(i32);

impl PhaseOffsetMicros {
    /// Names an offset.
    #[must_use]
    pub const fn new(micros: i32) -> Self {
        Self(micros)
    }

    /// Returns the offset in microseconds.
    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}

/// A bounded integer estimator of the running phase offset.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DriftEstimator {
    average_micros: i64,
    samples: usize,
}

impl Default for DriftEstimator {
    fn default() -> Self {
        Self::new()
    }
}

impl DriftEstimator {
    /// Builds an estimator that has seen nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            average_micros: 0,
            samples: 0,
        }
    }

    /// Folds one offset into the running average.
    ///
    /// The first sample seeds the average outright, as the scheduler's own
    /// burst average does, so a single reading is not pulled seven eighths of
    /// the way towards zero.
    ///
    /// # Errors
    ///
    /// Returns [`CalliopeError::DriftWindowFull`] at [`MAX_DRIFT_SAMPLES`].
    pub fn observe(&mut self, offset: PhaseOffsetMicros) -> Result<i64, CalliopeError> {
        if self.samples >= MAX_DRIFT_SAMPLES {
            return Err(CalliopeError::DriftWindowFull {
                max: MAX_DRIFT_SAMPLES,
            });
        }
        let sample = i64::from(offset.get());
        self.average_micros = if self.samples == 0 {
            sample
        } else {
            let kept = self.average_micros.saturating_mul(DRIFT_WEIGHT_OLD);
            let added = sample.saturating_mul(DRIFT_WEIGHT_NEW);
            kept.saturating_add(added)
                .checked_div(DRIFT_DIVISOR)
                .unwrap_or(0)
        };
        self.samples = self.samples.saturating_add(1);
        Ok(self.average_micros)
    }

    /// Returns the running average in microseconds.
    #[must_use]
    pub const fn average_micros(&self) -> i64 {
        self.average_micros
    }

    /// Returns how many samples the estimator has folded in.
    #[must_use]
    pub const fn samples(&self) -> usize {
        self.samples
    }

    /// Returns `true` when the estimator has seen nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.samples == 0
    }
}
