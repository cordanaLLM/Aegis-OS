// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The seven-stage candidate lifecycle vocabulary (REQ-P16-01).
//!
//! The scaffold's header states the lifecycle as a numbered list -- Propose,
//! Challenge, Decompose, Prove, Check, Publish, Invalidate -- and its
//! `CandidateStage` enum carries the same seven names. Nothing in the scaffold
//! transitions between them: `evaluate_candidate` picks Publish or Invalidate
//! directly from the Pareto predicate and the other five names are never
//! constructed. [`crate::machine::Lifecycle`] is the transition relation that
//! was missing.
//!
//! # Which stages are terminal
//!
//! Only [`Stage::Invalidate`]. [`Stage::Publish`] is not, and that is
//! REQ-P16-05 rather than an oversight: the requirement asks for reversible
//! maturity gates that prevent "False Maturity", where a change looks stable
//! and then fails under compositional stress. A published candidate must
//! therefore still be withdrawable, so `Publish` accepts
//! [`Event::Invalidate`] and nothing else.

use core::fmt;

/// One stage of the seven-step candidate lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Stage {
    /// A candidate has been proposed and nothing has been done to it.
    Propose,
    /// The candidate has been challenged against the incumbent.
    Challenge,
    /// The change has been decomposed into structural parts.
    Decompose,
    /// The parts have been proved individually.
    Prove,
    /// The whole has been checked, within the recorded latency budget.
    Check,
    /// The candidate cleared the Pareto gate and is published.
    Publish,
    /// The candidate is withdrawn. Terminal.
    Invalidate,
}

impl Stage {
    /// Every stage, in the order REQ-P16-01 numbers them.
    pub const ALL: [Self; 7] = [
        Self::Propose,
        Self::Challenge,
        Self::Decompose,
        Self::Prove,
        Self::Check,
        Self::Publish,
        Self::Invalidate,
    ];

    /// Returns the stage name used in records and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Propose => "propose",
            Self::Challenge => "challenge",
            Self::Decompose => "decompose",
            Self::Prove => "prove",
            Self::Check => "check",
            Self::Publish => "publish",
            Self::Invalidate => "invalidate",
        }
    }

    /// Returns the stable tag mixed into the canonical ledger pre-image.
    ///
    /// The tags are the step numbers REQ-P16-01 gives, one-based, so a record
    /// written today and one written after a stage is added still disagree
    /// only where they should.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::Propose => 1,
            Self::Challenge => 2,
            Self::Decompose => 3,
            Self::Prove => 4,
            Self::Check => 5,
            Self::Publish => 6,
            Self::Invalidate => 7,
        }
    }

    /// Returns `true` when the stage accepts no further event.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Invalidate)
    }
}

impl fmt::Display for Stage {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// What may be offered to the lifecycle, without its payload.
///
/// The kind is what a refusal names, so a diagnostic can say which event was
/// rejected without echoing the metrics it carried.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum EventKind {
    /// Challenge the candidate against the incumbent.
    Challenge,
    /// Decompose the change into structural parts.
    Decompose,
    /// Prove the parts.
    Prove,
    /// Check the whole against the Pareto gate.
    Check,
    /// Withdraw the candidate.
    Invalidate,
}

impl EventKind {
    /// Every event kind, in lifecycle order.
    pub const ALL: [Self; 5] = [
        Self::Challenge,
        Self::Decompose,
        Self::Prove,
        Self::Check,
        Self::Invalidate,
    ];

    /// Returns the event name used in diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Challenge => "challenge",
            Self::Decompose => "decompose",
            Self::Prove => "prove",
            Self::Check => "check",
            Self::Invalidate => "invalidate",
        }
    }
}

impl fmt::Display for EventKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Why a candidate was withdrawn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum InvalidationReason {
    /// The candidate did not clear the Pareto gate.
    ParetoBreach,
    /// A regression appeared after publication (REQ-P16-05).
    PostPublicationRegression,
    /// An operator withdrew the candidate.
    Withdrawn,
}

impl InvalidationReason {
    /// Every reason.
    pub const ALL: [Self; 3] = [
        Self::ParetoBreach,
        Self::PostPublicationRegression,
        Self::Withdrawn,
    ];

    /// Returns the reason name used in records and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::ParetoBreach => "pareto-breach",
            Self::PostPublicationRegression => "post-publication-regression",
            Self::Withdrawn => "withdrawn",
        }
    }

    /// Returns the stable tag mixed into the canonical ledger pre-image.
    #[must_use]
    pub const fn tag(self) -> u8 {
        match self {
            Self::ParetoBreach => 1,
            Self::PostPublicationRegression => 2,
            Self::Withdrawn => 3,
        }
    }
}

impl fmt::Display for InvalidationReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// One event offered to the lifecycle.
#[derive(Debug, Clone, Copy, PartialEq)]
#[non_exhaustive]
pub enum Event {
    /// Challenge the candidate against the incumbent.
    Challenge,
    /// Decompose the change into structural parts.
    Decompose,
    /// Prove the parts.
    Prove,
    /// Check the whole, reporting how long the check took.
    Check {
        /// The metrics the Pareto gate compares.
        metrics: crate::metrics::CandidateMetrics,
        /// How long the check took, in milliseconds (REQ-P16-10).
        elapsed_ms: u32,
    },
    /// Withdraw the candidate.
    Invalidate {
        /// Why the candidate is being withdrawn.
        reason: InvalidationReason,
    },
}

impl Event {
    /// Returns the kind of this event.
    #[must_use]
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::Challenge => EventKind::Challenge,
            Self::Decompose => EventKind::Decompose,
            Self::Prove => EventKind::Prove,
            Self::Check { .. } => EventKind::Check,
            Self::Invalidate { .. } => EventKind::Invalidate,
        }
    }
}
