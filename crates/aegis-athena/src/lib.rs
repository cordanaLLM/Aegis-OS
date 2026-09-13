// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P16 `aegis-athena`: the candidate lifecycle, the Pareto gate and
//! the checkpoint ledger.
//!
//! The imported P16 scaffold (export-025 `6b23723ddb76`) states a seven-step
//! candidate lifecycle in its header and then never transitions between the
//! steps: `evaluate_candidate` picks Publish or Invalidate straight from a
//! four-way predicate, guards its inputs with `assert!`, and appends an
//! unhashed JSON line to a file it calls a BLAKE3 hash-chained ledger.
//! Milestone M05 supplies the three things that were missing -- the transition
//! relation, the refusals, and a chain that can actually be re-walked -- and
//! leaves every effect out.
//!
//! # The five things a reviewer should look at
//!
//! * [`Lifecycle`] is the seven-stage machine (REQ-P16-01). Only
//!   [`Stage::Invalidate`] is terminal: a published candidate stays
//!   withdrawable, which is REQ-P16-05's reversible maturity gate rather than
//!   an oversight.
//! * [`ParetoVerdict`] is the promotion gate (REQ-P16-02). It keeps the four
//!   comparisons rather than collapsing them, so which bound a candidate
//!   breached is a value and not an inference. Three bounds are ceilings
//!   compared strictly and the fourth is a floor compared non-strictly, which
//!   is what puts the two recorded boundary values -- a latency of 1.5 and a
//!   retention of 0.99 -- on opposite sides.
//! * [`CheckpointLedger`] is the ledger, ported under decision D02: SHA-256
//!   behind [`aegis_justitia::LedgerHasher`], the trait M02 already created,
//!   with its own domain tag so a P06 audit record cannot be replayed as a P16
//!   checkpoint. [`register::LEDGER_ALGORITHM`] records the three algorithms
//!   the sources name and which one was chosen.
//! * [`contracts`] types the one edge P16 produces on and consumes M14's
//!   [`aegis_justitia::SignedAuditRecord`] rather than redefining it.
//! * [`CHECK_LATENCY_BUDGET_MS`] is REQ-P16-10's 500 ms budget, enforced
//!   rather than reported: a check that overruns it is refused.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: the machine dispatches flat into one small handler per
//!   group of stages and no handler calls another;
//! * every loop carries a scalar upper bound: [`MAX_CHECKPOINT_RECORDS`],
//!   [`PROMOTION_GATE_COUNT`], [`MAX_RECORDED_CALLS`] and
//!   [`MAX_CONTRACT_PAYLOAD_BYTES`];
//! * every I/O-shaped call takes an explicit deadline: [`SysupdatePort::call`]
//!   cannot be implemented without receiving a [`CallDeadline`];
//! * the ledger reserves its record list to the bound at construction, so an
//!   append never reallocates, and every other value on a decision path is
//!   `Copy`. `tests/allocation_bounds.rs` is the falsifier;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or unchecked arithmetic,
//!   and no `unsafe` (forbidden at the workspace root).
//!
//! # What this crate does not do
//!
//! It changes nothing. There is no file, no `systemd-sysupdate` invocation, no
//! D-Bus connection, no microVM, no `AF_VSOCK` socket, no partition write and no
//! reboot anywhere in it. The ledger is in memory; the promotion trigger is a
//! payload; [`StubSysupdate`] records calls and makes none.
//! `tests/stubbed_effects.rs` sweeps this crate's own sources for a recorded
//! list of identifiers any of those effects would have to name and fails if one
//! appears -- a regression gate over an enumeration, not a proof over every
//! such identifier.
//!
//! A pass of this crate's tests is evidence about the lifecycle, the gate, the
//! chain and the payload shapes. It closes no image, boot, hardware or release
//! gate, and it is not evidence that any candidate was ever executed.
//!
//! # Example
//!
//! ```
//! use aegis_athena::{
//!     CandidateMetrics, LatencyMs, MemoryMb, NullModelRetention, SciCarbonRate, Sha256AthenaEngine,
//!     Stage,
//! };
//! use aegis_justitia::UnixSeconds;
//! use aegis_tellus::CandidateId;
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut engine = Sha256AthenaEngine::new();
//! let candidate = CandidateId::parse("candidate-001-scx-cake-v2")?;
//! let passing = CandidateMetrics::new(
//!     LatencyMs::new(1.2)?,
//!     MemoryMb::new(32.0)?,
//!     SciCarbonRate::new(0.45)?,
//!     NullModelRetention::new(0.995)?,
//! );
//!
//! let (stage, _) = engine.evaluate(candidate, passing, 120, UnixSeconds::new(1_770_000_000))?;
//! assert_eq!(stage, Stage::Publish);
//! assert_eq!(engine.ledger().len(), 1);
//! engine.verify()?;
//!
//! // Exactly the latency bound is not superior, and the refusal is recorded.
//! let at_bound = CandidateMetrics::new(
//!     LatencyMs::AT_BOUND,
//!     MemoryMb::new(32.0)?,
//!     SciCarbonRate::new(0.45)?,
//!     NullModelRetention::AT_FLOOR,
//! );
//! let (stage, _) = engine.evaluate(candidate, at_bound, 120, UnixSeconds::new(1_770_000_001))?;
//! assert_eq!(stage, Stage::Invalidate);
//! assert_eq!(engine.ledger().len(), 2);
//! engine.verify()?;
//!
//! assert!(CandidateId::parse("").is_err());
//! # Ok(())
//! # }
//! ```

pub mod contracts;
pub mod engine;
pub mod error;
pub mod ledger;
pub mod machine;
pub mod maturity;
pub mod metrics;
pub mod pareto;
pub mod ports;
pub mod register;
pub mod stage;

/// The contract version mixed into every canonical checkpoint pre-image.
///
/// It is part of the digest, so a record written under one version and one
/// written under another never collide even if every other field matches.
pub const CONTRACT_VERSION: &str = "aegis.athena.v1";

/// The identifier types P16 shares with P13.
///
/// They are `aegis-tellus`'s, re-exported rather than redeclared: the
/// candidate identifier a promotion trigger carries is the same value the
/// `EVALUATE_CANDIDATE_CARBON_SCI` query asked about, so one type, one
/// charset and one bound serve both sides of that edge. The empty identifier
/// the imported P16 scaffold guards with `assert!` is refused by
/// [`CandidateId::parse`].
pub use aegis_tellus::{CandidateId, CorrelationId};

pub use crate::contracts::audit_intake::AuditIntake;
pub use crate::contracts::graph::EdgeId;
pub use crate::contracts::promotion::{
    PromotionAction, PromotionTrigger, PromotionTriggerVersion, Slot, TriggerStage,
};
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::engine::{AthenaEngine, Sha256AthenaEngine, gate};
pub use crate::error::AthenaError;
pub use crate::ledger::{
    CHECKPOINT_DOMAIN, CheckpointBody, CheckpointDraft, CheckpointLedger, CheckpointRecord,
    LedgerError, MAX_CHECKPOINT_RECORDS, Sha256CheckpointLedger, verify_chain,
    verify_chain_bounded,
};
pub use crate::machine::{CHECK_LATENCY_BUDGET_MS, Lifecycle};
pub use crate::maturity::{
    MIN_PASSING_GATES, Maturity, PROMOTION_GATE_COUNT, PromotionGate, PromotionGates,
};
pub use crate::metrics::{
    CARBON_RATE_BOUND, CandidateMetrics, LATENCY_BOUND_MS, LatencyMs, MAX_LATENCY_MS,
    MAX_MEMORY_MB, MAX_SCI_CARBON_RATE, MEMORY_BOUND_MB, MIN_NULL_MODEL_RETENTION, MemoryMb,
    NullModelRetention, SciCarbonRate,
};
pub use crate::pareto::{Objective, ParetoVerdict};
pub use crate::ports::{
    CallDeadline, CallKind, MAX_RECORDED_CALLS, PortError, STUB_SERVICE_MILLIS, StubSysupdate,
    SysupdateCall, SysupdatePort,
};
pub use crate::register::{
    Citation, ClaimStatus, DecisionRecord, DecisionState, LEDGER_ALGORITHM, LedgerAlgorithmReading,
    P16_RECORDED_CLAIMS, RecordedClaim, SANDBOX_DEFERRED_TO,
};
pub use crate::stage::{Event, EventKind, InvalidationReason, Stage};
