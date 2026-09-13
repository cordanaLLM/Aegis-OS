// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The Pareto promotion gate (REQ-P16-02).
//!
//! A candidate is promoted only when it is superior on all four objectives at
//! once. The gate is written as four independent comparisons whose results are
//! kept, not collapsed: [`ParetoVerdict`] says which bounds were exceeded, so
//! "exceeding any single bound reaches Invalidate" is checkable per bound
//! rather than as one boolean.
//!
//! # The comparison directions, and the two boundary values
//!
//! Three of the four bounds are ceilings compared strictly, and the fourth is
//! a floor compared non-strictly. That asymmetry is the imported scaffold's,
//! kept deliberately, and it is what puts the two recorded boundary values on
//! opposite sides:
//!
//! | Objective | Bound | Comparison | At the bound |
//! | :-- | :-- | :-- | :-- |
//! | latency | [`crate::metrics::LATENCY_BOUND_MS`] 1.5 ms | `<` | fails |
//! | memory | [`crate::metrics::MEMORY_BOUND_MB`] 64.0 MB | `<` | fails |
//! | carbon rate | [`crate::metrics::CARBON_RATE_BOUND`] 0.8 | `<` | fails |
//! | retention | [`crate::metrics::MIN_NULL_MODEL_RETENTION`] 0.99 | `>=` | passes |

use core::fmt;

use crate::metrics::{
    CARBON_RATE_BOUND, CandidateMetrics, LATENCY_BOUND_MS, MEMORY_BOUND_MB,
    MIN_NULL_MODEL_RETENTION,
};

/// One of the four objectives the gate compares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Objective {
    /// Latency, against a ceiling.
    Latency,
    /// Resident memory, against a ceiling.
    Memory,
    /// SCI carbon rate, against a ceiling.
    Carbon,
    /// Null-model retention, against a floor.
    Retention,
}

impl Objective {
    /// Every objective, in the order the scaffold's predicate evaluates them.
    pub const ALL: [Self; 4] = [Self::Latency, Self::Memory, Self::Carbon, Self::Retention];

    /// Returns the objective name used in records and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Latency => "latency",
            Self::Memory => "memory",
            Self::Carbon => "carbon",
            Self::Retention => "retention",
        }
    }

    /// Returns the numeric bound this objective is compared against.
    #[must_use]
    pub const fn bound(self) -> f64 {
        match self {
            Self::Latency => LATENCY_BOUND_MS,
            Self::Memory => MEMORY_BOUND_MB,
            Self::Carbon => CARBON_RATE_BOUND,
            Self::Retention => MIN_NULL_MODEL_RETENTION,
        }
    }

    /// Returns `true` when the bound is a floor rather than a ceiling.
    #[must_use]
    pub const fn is_floor(self) -> bool {
        matches!(self, Self::Retention)
    }

    /// Returns the stable tag mixed into the canonical ledger pre-image.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Latency => 1,
            Self::Memory => 2,
            Self::Carbon => 3,
            Self::Retention => 4,
        }
    }

    /// Returns the objective's zero-based slot in a verdict.
    #[must_use]
    pub const fn slot(self) -> usize {
        match self {
            Self::Latency => 0,
            Self::Memory => 1,
            Self::Carbon => 2,
            Self::Retention => 3,
        }
    }
}

impl fmt::Display for Objective {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Which objectives a candidate cleared, and which it did not.
///
/// The four results are an array indexed by [`Objective::slot`] rather than
/// four named fields, so adding an objective is adding a variant and widening
/// one array instead of touching every method here.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ParetoVerdict {
    cleared: [bool; 4],
}

impl ParetoVerdict {
    /// Evaluates all four comparisons against `metrics`.
    ///
    /// The order is [`Objective::ALL`], which is the order the imported
    /// scaffold's `&&` chain evaluates in. Nothing short-circuits: every
    /// comparison is made and kept, so a candidate that breaches three bounds
    /// is distinguishable from one that breaches the first.
    #[must_use]
    pub fn evaluate(metrics: &CandidateMetrics) -> Self {
        Self {
            cleared: [
                metrics.latency.get() < LATENCY_BOUND_MS,
                metrics.memory.get() < MEMORY_BOUND_MB,
                metrics.carbon.get() < CARBON_RATE_BOUND,
                metrics.retention.get() >= MIN_NULL_MODEL_RETENTION,
            ],
        }
    }

    /// Returns whether `objective` was cleared.
    #[must_use]
    pub fn cleared(&self, objective: Objective) -> bool {
        self.cleared.get(objective.slot()).copied().unwrap_or(false)
    }

    /// Returns `true` when every objective was cleared.
    #[must_use]
    pub fn is_superior(&self) -> bool {
        self.cleared.iter().all(|cleared| *cleared)
    }

    /// Returns the first objective that was not cleared, if any.
    ///
    /// "First" is the order of [`Objective::ALL`], which is the order the
    /// scaffold's `&&` chain would have short-circuited in.
    #[must_use]
    pub fn first_breach(&self) -> Option<Objective> {
        Objective::ALL
            .into_iter()
            .find(|objective| !self.cleared(*objective))
    }

    /// Returns how many objectives were cleared.
    #[must_use]
    pub fn cleared_count(&self) -> usize {
        Objective::ALL
            .into_iter()
            .filter(|objective| self.cleared(*objective))
            .count()
    }

    /// Returns the cleared flags as a bit field, lowest bit first.
    ///
    /// This is what the ledger pre-image commits to, so a re-walk detects an
    /// edited verdict as readily as an edited metric.
    #[must_use]
    pub fn bits(&self) -> u8 {
        let mut bits = 0u8;
        for (index, objective) in Objective::ALL.into_iter().enumerate() {
            if self.cleared(objective) {
                bits |= 1u8 << index;
            }
        }
        bits
    }
}
