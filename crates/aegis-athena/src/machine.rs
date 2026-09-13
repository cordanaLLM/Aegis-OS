// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The seven-stage lifecycle as a transition relation.
//!
//! [`Lifecycle`] holds one candidate's current [`Stage`] and the verdict it
//! reached, if any. Dispatch is a flat match over the stage into one small
//! handler per group of stages; no handler calls another, so the call graph
//! stays the DAG HISS-01 requires.
//!
//! A refused transition leaves the machine exactly where it was and writes
//! nothing, which is what lets the ledger record what happened rather than
//! what was attempted.

use crate::error::AthenaError;
use crate::metrics::CandidateMetrics;
use crate::pareto::ParetoVerdict;
use crate::stage::{Event, EventKind, InvalidationReason, Stage};

/// The candidate-check latency budget, in milliseconds (REQ-P16-10).
///
/// 500, the figure the subsystem graph assigns Athena. The comparison is
/// non-strict, so a check taking exactly the budget is accepted and one
/// millisecond more is refused.
pub const CHECK_LATENCY_BUDGET_MS: u32 = 500;

/// One candidate's position in the lifecycle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Lifecycle {
    stage: Stage,
    verdict: Option<ParetoVerdict>,
    metrics: Option<CandidateMetrics>,
    reason: Option<InvalidationReason>,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl Lifecycle {
    /// Builds a lifecycle at [`Stage::Propose`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stage: Stage::Propose,
            verdict: None,
            metrics: None,
            reason: None,
        }
    }

    /// Returns the current stage.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        self.stage
    }

    /// Returns the Pareto verdict, once a check has produced one.
    #[must_use]
    pub const fn verdict(&self) -> Option<ParetoVerdict> {
        self.verdict
    }

    /// Returns the metrics the check compared, once one has run.
    #[must_use]
    pub const fn metrics(&self) -> Option<CandidateMetrics> {
        self.metrics
    }

    /// Returns why the candidate was withdrawn, once it has been.
    #[must_use]
    pub const fn reason(&self) -> Option<InvalidationReason> {
        self.reason
    }

    /// Offers one event to the lifecycle.
    ///
    /// # Errors
    ///
    /// Returns [`AthenaError::Terminal`] when the stage accepts nothing,
    /// [`AthenaError::Unexpected`] when it accepts events but not this one,
    /// and [`AthenaError::CheckBudgetExceeded`] when a check overran
    /// [`CHECK_LATENCY_BUDGET_MS`].
    pub fn step(&mut self, event: Event) -> Result<Stage, AthenaError> {
        if self.stage.is_terminal() {
            return Err(AthenaError::Terminal {
                stage: self.stage,
                event: event.kind(),
            });
        }
        if let Event::Invalidate { reason } = event {
            return Ok(self.withdraw(reason));
        }
        match self.stage {
            Stage::Propose | Stage::Challenge | Stage::Decompose => self.advance(event),
            Stage::Prove => self.check(event),
            Stage::Check | Stage::Publish | Stage::Invalidate => Err(AthenaError::Unexpected {
                stage: self.stage,
                event: event.kind(),
            }),
        }
    }

    /// Advances one of the three linear stages before the check.
    fn advance(&mut self, event: Event) -> Result<Stage, AthenaError> {
        let next = match (self.stage, event.kind()) {
            (Stage::Propose, EventKind::Challenge) => Stage::Challenge,
            (Stage::Challenge, EventKind::Decompose) => Stage::Decompose,
            (Stage::Decompose, EventKind::Prove) => Stage::Prove,
            _ => {
                return Err(AthenaError::Unexpected {
                    stage: self.stage,
                    event: event.kind(),
                });
            }
        };
        self.stage = next;
        Ok(next)
    }

    /// Runs the check, which decides between Publish and Invalidate.
    fn check(&mut self, event: Event) -> Result<Stage, AthenaError> {
        let Event::Check {
            metrics,
            elapsed_ms,
        } = event
        else {
            return Err(AthenaError::Unexpected {
                stage: self.stage,
                event: event.kind(),
            });
        };
        if elapsed_ms > CHECK_LATENCY_BUDGET_MS {
            return Err(AthenaError::CheckBudgetExceeded {
                budget_ms: CHECK_LATENCY_BUDGET_MS,
                elapsed_ms,
            });
        }
        let verdict = ParetoVerdict::evaluate(&metrics);
        self.metrics = Some(metrics);
        self.verdict = Some(verdict);
        if verdict.is_superior() {
            self.stage = Stage::Publish;
        } else {
            self.stage = Stage::Invalidate;
            self.reason = Some(InvalidationReason::ParetoBreach);
        }
        Ok(self.stage)
    }

    /// Withdraws the candidate from any non-terminal stage.
    ///
    /// This is REQ-P16-05's reversibility: a published candidate is still
    /// withdrawable, so a change that looked stable and then failed under
    /// compositional stress does not stay published because the stage that
    /// published it was treated as final.
    fn withdraw(&mut self, reason: InvalidationReason) -> Stage {
        self.stage = Stage::Invalidate;
        self.reason = Some(reason);
        self.stage
    }
}
