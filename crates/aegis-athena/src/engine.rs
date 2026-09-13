// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The evolution loop: lifecycle, gate and ledger driven together.
//!
//! The imported scaffold's `evaluate_candidate` does three things in one
//! function: it asserts its inputs, evaluates the Pareto predicate, and
//! appends a line to a file. [`AthenaEngine`] does the same three things with
//! each one moved to where it can be checked -- the inputs are refused by
//! their constructors, the predicate is [`ParetoVerdict`], and the append is a
//! hash-linked [`CheckpointLedger`] record.
//!
//! Every path that reaches a terminal or published stage writes exactly one
//! record. That is the property "an invalidation reaches Invalidate with a
//! ledger entry" rests on, and [`AthenaEngine::evaluate`] is where it is held.

use aegis_justitia::{Digest32, LedgerHasher, Sha256Hasher, UnixSeconds};
use aegis_tellus::CandidateId;

use crate::error::AthenaError;
use crate::ledger::{CheckpointDraft, CheckpointLedger};
use crate::machine::Lifecycle;
use crate::metrics::CandidateMetrics;
use crate::pareto::ParetoVerdict;
use crate::stage::{Event, InvalidationReason, Stage};

/// The evolution-loop engine over the D02 hashing trait.
#[derive(Debug, Clone, PartialEq)]
pub struct AthenaEngine<H: LedgerHasher> {
    ledger: CheckpointLedger<H>,
}

/// The SHA-256 engine, the only instantiation M05 ships (decision D02).
pub type Sha256AthenaEngine = AthenaEngine<Sha256Hasher>;

impl<H: LedgerHasher> Default for AthenaEngine<H> {
    fn default() -> Self {
        Self::new()
    }
}

impl<H: LedgerHasher> AthenaEngine<H> {
    /// Builds an engine with an empty ledger at the default bound.
    #[must_use]
    pub fn new() -> Self {
        Self {
            ledger: CheckpointLedger::new(),
        }
    }

    /// Builds an engine over an explicit ledger.
    #[must_use]
    pub const fn with_ledger(ledger: CheckpointLedger<H>) -> Self {
        Self { ledger }
    }

    /// Returns the checkpoint ledger.
    #[must_use]
    pub const fn ledger(&self) -> &CheckpointLedger<H> {
        &self.ledger
    }

    /// Drives one candidate from Propose to Publish or Invalidate.
    ///
    /// The three stages between Propose and the check carry no decision, so
    /// they are walked here rather than offered one at a time; a caller that
    /// wants to interleave its own work uses [`Self::step`] instead.
    ///
    /// # Errors
    ///
    /// Returns [`AthenaError`] from the lifecycle -- including
    /// [`AthenaError::CheckBudgetExceeded`] when the check overran REQ-P16-10's
    /// budget -- or from the ledger. A refused evaluation writes no record.
    pub fn evaluate(
        &mut self,
        candidate: CandidateId,
        metrics: CandidateMetrics,
        elapsed_ms: u32,
        at: UnixSeconds,
    ) -> Result<(Stage, Lifecycle), AthenaError> {
        let mut lifecycle = Lifecycle::new();
        lifecycle.step(Event::Challenge)?;
        lifecycle.step(Event::Decompose)?;
        lifecycle.step(Event::Prove)?;
        let stage = lifecycle.step(Event::Check {
            metrics,
            elapsed_ms,
        })?;
        self.record(candidate, &lifecycle, at)?;
        Ok((stage, lifecycle))
    }

    /// Offers one event to `lifecycle` and records the stage it reached.
    ///
    /// # Errors
    ///
    /// Returns [`AthenaError`] from the lifecycle or from the ledger. A
    /// refused step writes no record.
    pub fn step(
        &mut self,
        candidate: CandidateId,
        lifecycle: &mut Lifecycle,
        event: Event,
        at: UnixSeconds,
    ) -> Result<Stage, AthenaError> {
        let stage = lifecycle.step(event)?;
        self.record(candidate, lifecycle, at)?;
        Ok(stage)
    }

    /// Withdraws a candidate, whatever stage it is in (REQ-P16-05).
    ///
    /// # Errors
    ///
    /// Returns [`AthenaError::Terminal`] when the candidate is already
    /// withdrawn, and otherwise a ledger refusal.
    pub fn withdraw(
        &mut self,
        candidate: CandidateId,
        lifecycle: &mut Lifecycle,
        reason: InvalidationReason,
        at: UnixSeconds,
    ) -> Result<Stage, AthenaError> {
        self.step(candidate, lifecycle, Event::Invalidate { reason }, at)
    }

    /// Appends one record describing where `lifecycle` now is.
    fn record(
        &mut self,
        candidate: CandidateId,
        lifecycle: &Lifecycle,
        at: UnixSeconds,
    ) -> Result<Digest32, AthenaError> {
        let draft = CheckpointDraft {
            candidate,
            stage: lifecycle.stage(),
            metrics: lifecycle.metrics(),
            verdict: lifecycle.verdict(),
            reason: lifecycle.reason(),
            at,
        };
        Ok(self.ledger.append(draft)?)
    }

    /// Re-walks the ledger this engine wrote.
    ///
    /// # Errors
    ///
    /// Propagates [`CheckpointLedger::verify`].
    pub fn verify(&self) -> Result<Digest32, AthenaError> {
        Ok(self.ledger.verify()?)
    }
}

/// Returns the verdict a metric set produces, without a lifecycle.
///
/// This is the gate on its own, for a caller that wants the four comparisons
/// without driving a candidate: [`AthenaEngine::evaluate`] uses the same
/// predicate.
#[must_use]
pub fn gate(metrics: &CandidateMetrics) -> ParetoVerdict {
    ParetoVerdict::evaluate(metrics)
}
