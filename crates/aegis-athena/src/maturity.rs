// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The structural promotion gates (REQ-P15-04).
//!
//! REQ-P15-04 records that a Hestia feature reaches Established maturity only
//! after passing six of nine machine-checkable structural promotion gates under
//! Project Athena. The requirement states the two numbers and the word
//! "structural"; it does not name the nine gates, and no imported source does.
//!
//! So [`PromotionGate`] enumerates nine slots with the names the requirement's
//! own vocabulary supports, and this module is explicit that the *naming* is
//! this milestone's reading while the *counting* is the requirement's. What is
//! checked here is the counting rule: six of nine passes, five does not, and a
//! gate recorded twice is refused rather than counted twice.

use core::fmt;

use crate::error::AthenaError;

/// How many of the nine gates a feature must pass to reach Established.
pub const MIN_PASSING_GATES: usize = 6;

/// How many structural promotion gates there are.
pub const PROMOTION_GATE_COUNT: usize = 9;

/// One structural promotion gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum PromotionGate {
    /// The feature has a typed public interface.
    TypedInterface,
    /// Positive, negative and boundary tests exist for that interface.
    ThreeDimensionalTests,
    /// Every loop the feature adds carries a scalar upper bound.
    BoundedLoops,
    /// Every error path returns rather than aborting.
    NoAbortPath,
    /// Every external call carries an explicit deadline.
    DeadlinedIo,
    /// The feature's manifest and lock entry are committed.
    CommittedManifest,
    /// The feature's public surface is swept for untested names.
    SweptSurface,
    /// Every claim the feature records names its source.
    CitedClaims,
    /// The feature's decision path allocates nothing.
    NoDecisionPathHeap,
}

impl PromotionGate {
    /// Every gate, in the order this module numbers them.
    pub const ALL: [Self; PROMOTION_GATE_COUNT] = [
        Self::TypedInterface,
        Self::ThreeDimensionalTests,
        Self::BoundedLoops,
        Self::NoAbortPath,
        Self::DeadlinedIo,
        Self::CommittedManifest,
        Self::SweptSurface,
        Self::CitedClaims,
        Self::NoDecisionPathHeap,
    ];

    /// Returns the gate name used in records and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::TypedInterface => "typed-interface",
            Self::ThreeDimensionalTests => "three-dimensional-tests",
            Self::BoundedLoops => "bounded-loops",
            Self::NoAbortPath => "no-abort-path",
            Self::DeadlinedIo => "deadlined-io",
            Self::CommittedManifest => "committed-manifest",
            Self::SweptSurface => "swept-surface",
            Self::CitedClaims => "cited-claims",
            Self::NoDecisionPathHeap => "no-decision-path-heap",
        }
    }

    /// Returns the gate's zero-based slot.
    #[must_use]
    pub const fn slot(self) -> usize {
        match self {
            Self::TypedInterface => 0,
            Self::ThreeDimensionalTests => 1,
            Self::BoundedLoops => 2,
            Self::NoAbortPath => 3,
            Self::DeadlinedIo => 4,
            Self::CommittedManifest => 5,
            Self::SweptSurface => 6,
            Self::CitedClaims => 7,
            Self::NoDecisionPathHeap => 8,
        }
    }
}

impl fmt::Display for PromotionGate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The maturity a feature has reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Maturity {
    /// Fewer than [`MIN_PASSING_GATES`] gates passed.
    Provisional,
    /// At least [`MIN_PASSING_GATES`] gates passed.
    Established,
}

impl Maturity {
    /// Every maturity level, weakest first.
    pub const ALL: [Self; 2] = [Self::Provisional, Self::Established];

    /// Returns the level name used in records and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Provisional => "provisional",
            Self::Established => "established",
        }
    }
}

impl fmt::Display for Maturity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The recorded result of each of the nine gates.
///
/// A gate is recorded at most once; a second result for the same gate is
/// refused rather than overwritten, because "this gate passed" and "this gate
/// passed, then failed" are different findings and only the second is safe to
/// discard deliberately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PromotionGates {
    results: [Option<bool>; PROMOTION_GATE_COUNT],
}

impl PromotionGates {
    /// Builds a set with no gate recorded.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            results: [None; PROMOTION_GATE_COUNT],
        }
    }

    /// Records one gate's result.
    ///
    /// # Errors
    ///
    /// Returns [`AthenaError::MaturityGates`] when the gate already has a
    /// recorded result.
    pub fn record(&mut self, gate: PromotionGate, passed: bool) -> Result<(), AthenaError> {
        let slot = self
            .results
            .get_mut(gate.slot())
            .ok_or(AthenaError::MaturityGates {
                reason: "the gate slot is outside the recorded gate set",
            })?;
        if slot.is_some() {
            return Err(AthenaError::MaturityGates {
                reason: "the gate already carries a recorded result",
            });
        }
        *slot = Some(passed);
        Ok(())
    }

    /// Returns the recorded result for `gate`, if there is one.
    #[must_use]
    pub fn result(&self, gate: PromotionGate) -> Option<bool> {
        self.results.get(gate.slot()).copied().flatten()
    }

    /// Returns how many gates have been recorded as passing.
    #[must_use]
    pub fn passing(&self) -> usize {
        self.results
            .iter()
            .flatten()
            .filter(|passed| **passed)
            .count()
    }

    /// Returns how many gates have a recorded result at all.
    #[must_use]
    pub fn recorded(&self) -> usize {
        self.results.iter().flatten().count()
    }

    /// Returns the maturity the recorded results support.
    ///
    /// An unrecorded gate is not a pass: the count is over gates recorded as
    /// passing, so a feature with six unrecorded gates is Provisional.
    #[must_use]
    pub fn maturity(&self) -> Maturity {
        if self.passing() >= MIN_PASSING_GATES {
            return Maturity::Established;
        }
        Maturity::Provisional
    }
}
