// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P06 `aegis-justitia`: the decision engine.
//!
//! The crate consumes `amussis-decision` for the risk-tier, maker-checker,
//! Annex III, timeout, killswitch and hash-linked audit ledger logic (ADR-0006).
//! Every rule is reachable through the public API so the integration tests
//! exercise the shipped logic, not a test-only copy.
//!
//! Design invariants enforced here (see `AGENTS.md`):
//!
//! * no recursion: [`ledger::verify_chain_bounded`] walks a chain iteratively;
//! * every loop carries a scalar upper bound;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or wrapping arithmetic;
//! * no dynamic execution and no `unsafe` (forbidden at the crate root).
//!
//! # HISS-03: what the decision path allocates
//!
//! [`JustitiaEngine::decide`] and [`JustitiaEngine::adjudicate`] perform **no
//! heap allocation after construction**. That is a structural property, not a
//! convention:
//!
//! * every value on the path ([`ActionIntent`], [`InterceptorOutcome`],
//!   [`AuditRecord`], [`identity::Identity`], [`Signature`]) is `Copy` and holds
//!   its bytes inline in a fixed-size array; the crate contains no `String`,
//!   `Box` or `Vec` field outside the three retention buffers named below;
//! * those three buffers -- the pending-request registry, the ledger's record
//!   list and the sink's record list -- are each `Vec::with_capacity(bound)` at
//!   construction and each refuses a push at `len >= bound`, so `Vec::push`
//!   never reaches its reallocation path;
//! * reclamation uses `Vec::retain`, which compacts in place and never
//!   allocates.
//!
//! `tests/allocation_bounds.rs` proves it rather than asserting it: it records
//! `reserved()` on all three buffers, drives the engine to each bound, and fails
//! if any reserved capacity changed.
//!
//! # Memory arithmetic behind the default bounds
//!
//! An [`AuditRecord`] is dominated by its [`MAX_SIGNATURE_BYTES`]-byte inline
//! signature. Milestone M02 retains every record **twice** -- once in
//! [`AuditLedger`] for the chain re-walk and once in the [`InMemorySink`] that
//! stands in for the durable sink M14 pins -- so the default retention is
//!
//! ```text
//! 2 x MAX_LEDGER_RECORDS x size_of::<AuditRecord>()
//! ```
//!
//! and the pending registry adds
//!
//! ```text
//! MAX_PENDING_REQUESTS x size_of::<(ApprovalTicket, RequestState)>()
//! ```
//!
//! No byte figure is written here, because a hand-written one drifts the moment
//! a field moves or the target changes its padding. `tests/allocation_bounds.rs`
//! computes both from `size_of` on the target that is actually building the
//! crate and fails if the retained total passes the
//! [`MAX_RETAINED_AUDIT_BYTES`] budget; the figures in that failure message are
//! the authority, not this paragraph. The point the bounds exist to make is the
//! order of magnitude: a hardened unit reserves single-digit mebibytes for P06,
//! not the gibibytes a `1 << 20` record bound would have demanded.
//!
//! # Total inventory of `unwrap_or` in the library
//!
//! HISS-07 bans `unwrap` and `expect`; `unwrap_or` is permitted because it names
//! the fallback, but every use still has to be justified rather than counted
//! loosely. Following ADR-0006, the six sites in the engine and ledger logic are
//! carried upstream by `amussis-decision`, leaving zero in `aegis-justitia`'s own
//! source code.
//!
//! # What milestone M14 added, and what it deliberately did not
//!
//! [`contracts`] holds the three versioned consumer schemas P06 speaks over --
//! the action proposal from P09 Minerva, the decision request to P05 Forum and
//! the signed audit record to P16 Athena -- together with the reviewed
//! hardened-unit contract. Every schema is a typed Rust struct with an explicit
//! contract version, a stable field encoding and a decoder that lands every
//! field in a fixed inline buffer. See [`contracts`] for what that does and
//! does not say about heap: an escape-free payload is decoded without one, an
//! admissible JSON escape costs the decoder a bounded scratch copy, and the
//! decoded value owns nothing either way.
//!
//! **No transport is implemented anywhere in this crate.** There is no D-Bus
//! connection, no socket, no eBPF program and no TPM2 signing: a signature
//! field is a field encoding, not a signer. eBPF compilation and verifier load
//! are M19 and M10, TPM2 sealing is M20, the Forum consumer is M16, and a
//! file-backed ledger sink is still absent.

#![forbid(unsafe_code)]
#![deny(missing_docs)]

pub mod contracts;
pub mod ledger;

/// Adapter module re-exporting the decision engine.
pub mod engine {
    pub use amussis_decision::engine::DecisionEngine as JustitiaEngine;
    pub use amussis_decision::engine::*;
}

/// Adapter module re-exporting the decision errors.
pub mod error {
    pub use amussis_decision::error::DecisionError as JustitiaError;
    pub use amussis_decision::error::*;
}

pub use amussis_decision::effects;
pub use amussis_decision::identity;
pub use amussis_decision::intent;
pub use amussis_decision::killswitch;
pub use amussis_decision::outcome;
pub use amussis_decision::oversight;
pub use amussis_decision::registry;
pub use amussis_decision::risk;
pub use amussis_decision::time;

pub use amussis_decision::CONTRACT_VERSION;
pub use amussis_decision::DEFAULT_APPROVAL_TTL_SECS;
pub use amussis_decision::DEFAULT_SINK_SERVICE_MILLIS;
pub use amussis_decision::DIGEST_LEN;
pub use amussis_decision::MAX_IDENTITY_LEN;
pub use amussis_decision::MAX_LEDGER_RECORDS;
pub use amussis_decision::MAX_PENDING_REQUESTS;
pub use amussis_decision::MAX_PREIMAGE_BYTES;
pub use amussis_decision::MAX_RETAINED_AUDIT_BYTES;
pub use amussis_decision::MAX_SIGNATURE_BYTES;
pub use amussis_decision::MAX_TARGET_LEN;

pub use crate::contracts::audit::{AuditRecordVersion, PLD_EFFECTIVE_AT, SignedAuditRecord};
pub use crate::contracts::decision_request::{
    DecisionRequest, DecisionRequestVersion, MAX_DECISION_WINDOW_SECS, MIN_DECISION_WINDOW_SECS,
};
pub use crate::contracts::graph::{
    D03_ACTION_GATE, D03_ACTION_GATE_DIRECTION, D03_ACTION_GATE_TRANSPORT, D04_SANDBOX_GATE,
    DecisionRecord, DecisionState, EdgeId, GateDirection, MAX_ADMISSION_HOPS, SandboxAdmissionPath,
    contract_edges,
};
pub use crate::contracts::proposal::{ActionProposal, ActionProposalVersion};
pub use crate::contracts::unit::{
    CONTRACT_MARKER, INSTALL_DIRECTORIES, MAX_UNIT_BYTES, MAX_UNIT_LINES, REQUIRED_DIRECTIVES,
    START_LIMIT_DIRECTIVES, UnitContractError, is_installed_path, review,
};
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::ledger::{Chain, ChainRecord, Timestamped};
pub use amussis_decision::encoding::fields::{
    DIGEST_HEX_CHARS, MAX_SIGNATURE_HEX_CHARS, OversightSignature, SignatureAlgorithm,
    SignatureBytes,
};
pub use amussis_decision::engine::DecisionEngine as JustitiaEngine;
pub use amussis_decision::engine::EngineConfig;
pub use amussis_decision::error::DecisionError as JustitiaError;
pub use amussis_decision::identity::{
    AgentId, CheckerId, EventId, Identity, IdentityError, IntentId, MakerId, RequestId,
    SignerKeyId, TargetResource,
};
pub use amussis_decision::intent::{ActionIntent, IntentError, IntentSpec};
pub use amussis_decision::killswitch::{HaltReason, Killswitch, KillswitchState, ReleaseError};
pub use amussis_decision::ledger::hash::{
    CanonicalBuffer, Digest32, HashAlgorithm, HashError, LedgerHasher, PreimageError,
};
pub use amussis_decision::ledger::sha256::Sha256Hasher;
pub use amussis_decision::ledger::signer::{RecordSigner, SignError, Signature, SignerBinding};
pub use amussis_decision::ledger::{
    AUDIT_DOMAIN, AuditLedger, AuditRecord, InMemorySink, IoDeadline, LedgerError, LedgerSink,
    RecordBody, RecordDraft, RecordPrincipals, RecordStatus, RecordSubject, Sequence, Sha256Ledger,
    SinkError, UnavailableSink, verify_chain, verify_chain_bounded, verify_continuation,
};
pub use amussis_decision::outcome::{BlockReason, InterceptorOutcome, WarningCode};
pub use amussis_decision::oversight::{
    Adjudication, ApprovalTicket, CheckerSet, OversightError, VerifiedCheckerSet, Vote,
};
pub use amussis_decision::registry::{PendingRegistry, RegistryError, RequestState};
pub use amussis_decision::risk::{
    ActionType, OversightClass, RequiredApproval, RiskLevel, RiskTier,
};
pub use amussis_decision::time::{
    Clock, ClockError, Deadline, DeadlineError, DeadlineStatus, FixedClock, MonotonicGuard, Ttl,
    UnixSeconds,
};
