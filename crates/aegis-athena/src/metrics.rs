// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The four metrics the Pareto gate compares, as validated quantities.
//!
//! REQ-P16-02 gates promotion on latency, memory, carbon rate and null-model
//! retention. The imported scaffold carries all four as bare `f64` fields on a
//! `CandidateMetrics` struct and guards only one of them, with an `assert!`.
//!
//! Here each is a newtype with its own constructor, for two reasons. The first
//! is HISS-07: the scaffold's positive-latency assertion becomes
//! [`AthenaError::Latency`]. The second is that the gate's comparisons are
//! then over values that are known finite, so a NaN cannot silently fail every
//! comparison and send a candidate to Invalidate for a reason nobody can see.

use crate::error::AthenaError;

/// Scalar upper bound on an admissible latency, in milliseconds.
pub const MAX_LATENCY_MS: f64 = 1.0e6;

/// Scalar upper bound on an admissible memory figure, in megabytes.
pub const MAX_MEMORY_MB: f64 = 1.0e6;

/// Scalar upper bound on an admissible carbon rate.
pub const MAX_SCI_CARBON_RATE: f64 = 1.0e6;

/// The latency bound the Pareto gate compares against, in milliseconds.
///
/// 1.5, from the scaffold's `metrics.latency_ms < 1.5`. The comparison is
/// strict, so exactly 1.5 does not pass.
pub const LATENCY_BOUND_MS: f64 = 1.5;

/// The memory bound the Pareto gate compares against, in megabytes.
///
/// 64.0, from the scaffold's `metrics.memory_mb < 64.0`.
pub const MEMORY_BOUND_MB: f64 = 64.0;

/// The carbon-rate bound the Pareto gate compares against.
///
/// 0.8, from the scaffold's `metrics.sci_carbon_rate < 0.8`. It is in the same
/// unit as [`aegis_tellus::SciRate`]: gCO2eq per functional unit.
pub const CARBON_RATE_BOUND: f64 = 0.8;

/// The null-model retention floor the Pareto gate compares against.
///
/// 0.99, from the scaffold's `metrics.null_model_retention >= 0.99`. This is
/// the one bound of the four that is a floor rather than a ceiling, and the
/// one comparison of the four that is not strict, so exactly 0.99 passes. The
/// asymmetry is the scaffold's and is kept deliberately: it is what makes the
/// two recorded boundary values, 1.5 and 0.99, land on opposite sides.
pub const MIN_NULL_MODEL_RETENTION: f64 = 0.99;

/// A validated latency in milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct LatencyMs(f64);

impl LatencyMs {
    /// Exactly the gate's bound, 1.5 ms.
    pub const AT_BOUND: Self = Self(LATENCY_BOUND_MS);

    /// Builds a latency.
    ///
    /// # Errors
    ///
    /// Returns [`AthenaError::Latency`] for a non-finite, non-positive or
    /// over-bound value. Non-positive rather than negative: the scaffold's
    /// assertion is `latency_ms > 0.0`, and a run that took no time did not
    /// run.
    pub fn new(millis: f64) -> Result<Self, AthenaError> {
        if !millis.is_finite() {
            return Err(AthenaError::Latency {
                reason: "a value that is not finite",
            });
        }
        if millis <= 0.0 {
            return Err(AthenaError::Latency {
                reason: "a value that is not positive",
            });
        }
        if millis > MAX_LATENCY_MS {
            return Err(AthenaError::Latency {
                reason: "a value past the recorded bound",
            });
        }
        Ok(Self(millis))
    }

    /// Returns the latency in milliseconds.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A validated memory figure in megabytes.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct MemoryMb(f64);

impl MemoryMb {
    /// Exactly the gate's bound, 64.0 MB.
    pub const AT_BOUND: Self = Self(MEMORY_BOUND_MB);

    /// Builds a memory figure.
    ///
    /// # Errors
    ///
    /// Returns [`AthenaError::Memory`] for a non-finite, negative or
    /// over-bound value.
    pub fn new(megabytes: f64) -> Result<Self, AthenaError> {
        if !megabytes.is_finite() {
            return Err(AthenaError::Memory {
                reason: "a value that is not finite",
            });
        }
        if megabytes < 0.0 {
            return Err(AthenaError::Memory {
                reason: "a negative value",
            });
        }
        if megabytes > MAX_MEMORY_MB {
            return Err(AthenaError::Memory {
                reason: "a value past the recorded bound",
            });
        }
        Ok(Self(megabytes))
    }

    /// Returns the figure in megabytes.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A validated SCI carbon rate, in gCO2eq per functional unit.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SciCarbonRate(f64);

impl SciCarbonRate {
    /// Exactly the gate's bound, 0.8.
    pub const AT_BOUND: Self = Self(CARBON_RATE_BOUND);

    /// Builds a carbon rate.
    ///
    /// # Errors
    ///
    /// Returns [`AthenaError::CarbonRate`] for a non-finite, negative or
    /// over-bound value.
    pub fn new(rate: f64) -> Result<Self, AthenaError> {
        if !rate.is_finite() {
            return Err(AthenaError::CarbonRate {
                reason: "a value that is not finite",
            });
        }
        if rate < 0.0 {
            return Err(AthenaError::CarbonRate {
                reason: "a negative value",
            });
        }
        if rate > MAX_SCI_CARBON_RATE {
            return Err(AthenaError::CarbonRate {
                reason: "a value past the recorded bound",
            });
        }
        Ok(Self(rate))
    }

    /// Builds a carbon rate from a P13 evaluation.
    ///
    /// This is the one place the two crates' numbers meet: the rate P13
    /// returns on the `EVALUATE_CANDIDATE_CARBON_SCI` edge is the rate the
    /// gate compares. It is fallible because [`aegis_tellus::SciRate`] admits
    /// a wider range than this type does.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::new`].
    pub fn from_sci(rate: aegis_tellus::SciRate) -> Result<Self, AthenaError> {
        Self::new(rate.get())
    }

    /// Returns the rate.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A validated null-model retention proportion.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct NullModelRetention(f64);

impl NullModelRetention {
    /// Exactly the gate's floor, 0.99.
    pub const AT_FLOOR: Self = Self(MIN_NULL_MODEL_RETENTION);

    /// Full retention, 1.0.
    pub const FULL: Self = Self(1.0);

    /// Builds a retention proportion.
    ///
    /// # Errors
    ///
    /// Returns [`AthenaError::Retention`] for a non-finite value or one
    /// outside `0.0..=1.0`. A retention is a proportion, so the range is the
    /// unit interval rather than a recorded bound.
    pub fn new(proportion: f64) -> Result<Self, AthenaError> {
        if !proportion.is_finite() {
            return Err(AthenaError::Retention {
                reason: "a value that is not finite",
            });
        }
        if !(0.0..=1.0).contains(&proportion) {
            return Err(AthenaError::Retention {
                reason: "a value outside the unit interval",
            });
        }
        Ok(Self(proportion))
    }

    /// Returns the proportion.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// The four metrics one candidate is judged on.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct CandidateMetrics {
    /// Measured latency, in milliseconds.
    pub latency: LatencyMs,
    /// Measured resident memory, in megabytes.
    pub memory: MemoryMb,
    /// The SCI carbon rate P13 returned for the candidate.
    pub carbon: SciCarbonRate,
    /// Retention against the null model.
    pub retention: NullModelRetention,
}

impl CandidateMetrics {
    /// Builds a metric set.
    #[must_use]
    pub const fn new(
        latency: LatencyMs,
        memory: MemoryMb,
        carbon: SciCarbonRate,
        retention: NullModelRetention,
    ) -> Self {
        Self {
            latency,
            memory,
            carbon,
            retention,
        }
    }
}
