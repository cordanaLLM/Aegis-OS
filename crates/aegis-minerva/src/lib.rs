// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P09 `aegis-minerva`: the agent execution chain's middle link.
//!
//! The imported P09 scaffold (export-034 `213a95fc0d02`) is a daemon that
//! routes a prompt to one of two hard-coded experts, prints what it decided and
//! calls a "solver" that looks for an exclamation mark. Milestone M06 extracts
//! the part that can be checked without a `GPU`, a bus connection, a solver or
//! any inference -- the tables, the bounds and the payloads -- and turns each
//! `&'static str` refusal into a typed one, which is what HISS-07 asks for.
//!
//! # The four things a reviewer should look at
//!
//! * [`AlpsRouter`] holds the expert bound and the power envelope: 32 experts,
//!   the 33rd refused, and routing returns `None` when no active expert serves
//!   the domain or none fits the declared budget.
//! * [`AgentHerEngine`] holds the replay bound and the relabelling rule: 128
//!   steps, the 129th refused, and only a strictly negative reward is
//!   relabelled -- [`Reward`] is fixed-point so that boundary is exact.
//! * [`chain`] builds the two payloads other crates own --
//!   [`ActionProposal`](aegis_justitia::ActionProposal) from M14 and
//!   [`CapsuleRequest`](aegis_vesta::CapsuleRequest) from this milestone --
//!   rather than redefining either, and reads their edge, direction and
//!   runtime constants instead of restating them.
//! * [`decision`] records D27, which stays **open**: both readings of the
//!   P09/P14 verification edge remain representable and the submission carries
//!   the graph of record's.
//!
//! # What is deliberately not claimed
//!
//! [`ConstraintScreen`] is not a solver and cannot say that anything is valid.
//! Its two outcomes are "rejected" and "not rejected", and the second is the
//! absence of a rejection. The 20 W envelope and the 1.5 ms routing cap are
//! recorded constants, not measurements: this crate has no clock and no power
//! meter, and [`register`] says so for each of them.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: every function here is a flat check, a bounded sweep or a
//!   field access, and no function in the crate calls itself directly or
//!   through another;
//! * every loop carries a scalar upper bound: [`MAX_SLM_EXPERTS`],
//!   [`MAX_TRAJECTORY_STEPS`], [`MAX_SCRIPT_BYTES`], [`MAX_LABEL_LEN`] and
//!   [`MAX_CONTRACT_PAYLOAD_BYTES`];
//! * every value on a decision path is `Copy`, so no such path allocates;
//!   `tests/allocation_bounds.rs` is the falsifier, and the one place that is
//!   deliberately not claimed -- a JSON string carrying an escape, which
//!   `serde_json` unescapes into a heap scratch buffer before any field of ours
//!   sees it -- is tested rather than denied;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or unchecked arithmetic,
//!   and no `unsafe` (forbidden at the workspace root).
//!
//! # What this crate does not do
//!
//! It infers nothing and sends nothing. There is no model, no tokeniser, no
//! `GPU` or accelerator context, no weight streaming, no `Z3` or other solver,
//! no `D-Bus` connection, no socket, no `eBPF` program and no filesystem access
//! anywhere in it. `tests/stubbed_effects.rs` sweeps the crate's own sources
//! for a recorded list of identifiers that would be needed to do any of it and
//! fails if one appears -- a regression gate over an enumeration, not a proof
//! over every such identifier.
//!
//! A pass of this crate's tests is evidence about the tables, the bounds and
//! the payloads. It closes no hardware, inference or transport gate.
//!
//! # Example
//!
//! ```
//! use aegis_minerva::{
//!     AlpsRouter, ExpertDomain, ExpertId, Label, PowerEnvelope, SlmExpert,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut router = AlpsRouter::new();
//! router.register(SlmExpert {
//!     id: ExpertId::new(1),
//!     name: Label::parse("aegis-slm-code-v1")?,
//!     domain: ExpertDomain::CodeSynthesis,
//!     parameter_count_millions: 1_500,
//!     vram_footprint_mib: 1_200,
//!     avg_latency_us: 450,
//!     energy_per_token_ujoule: 120,
//!     draw_milliwatts: 18_500,
//!     active: true,
//! })?;
//!
//! let budget = PowerEnvelope::new(18_500)?;
//! let routed = router.route(ExpertDomain::CodeSynthesis, budget);
//! assert_eq!(routed.map(|expert| expert.id), Some(ExpertId::new(1)));
//! assert!(router.route(ExpertDomain::SymbolicMath, budget).is_none());
//! assert!(PowerEnvelope::new(20_001).is_err());
//! # Ok(())
//! # }
//! ```

pub mod chain;
pub mod contracts;
pub mod decision;
pub mod error;
pub mod expert;
pub mod id;
pub mod register;
pub mod screen;
pub mod trajectory;

pub use crate::chain::{CapsuleDispatch, ProposalDraft};
pub use crate::contracts::cad_verification::{CadVerificationRequest, CadVerificationVersion};
pub use crate::contracts::graph::{CadVerificationDirection, EdgeId};
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::decision::{
    Citation, D27_CAD_VERIFICATION_DIRECTION, DecisionRecord, DecisionState,
};
pub use crate::error::MinervaError;
pub use crate::expert::{
    AlpsRouter, ENERGY_WEIGHT_TENTHS, ExpertDomain, ExpertId, LATENCY_WEIGHT_TENTHS,
    MAX_SLM_EXPERTS, MIN_ENVELOPE_MILLIWATTS, POWER_CAP_MILLIWATTS, PowerEnvelope,
    ROUTER_LATENCY_CAP_US, SlmExpert, within_routing_budget,
};
pub use crate::id::{IdError, Label, MAX_LABEL_LEN};
pub use crate::register::{ClaimSource, ClaimStatus, P09_RECORDED_CLAIMS, RecordedClaim};
pub use crate::screen::{
    BYTES_PER_CONSTRAINT, ConstraintScreen, ConstraintScript, MAX_SCRIPT_BYTES, REJECTED_BYTE,
    ScreenOutcome,
};
pub use crate::trajectory::{
    AgentHerEngine, MAX_TRAJECTORY_STEPS, Reward, STATE_HASH_LEN, StateHash, TrajectoryStep,
};
