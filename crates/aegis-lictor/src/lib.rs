// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P07 `aegis-lictor`: the resource broker's arithmetic, without the
//! kernel.
//!
//! The imported P07 scaffold (export-032 `30d67bc52290`) is an async daemon
//! that pushes process rows onto an unbounded vector behind a mutex it
//! unwraps, prints that it is elevating and throttling, and then exits after
//! announcing a D-Bus listener it never starts. Beside it sits the eBPF
//! program the daemon would load (export-056 `39af243568ad`), whose tier
//! arithmetic is the part that can be checked without a kernel at all.
//! Milestone M07 extracts both -- the classifier, the table, the bounds, the
//! locality set and the payloads -- and turns each missing refusal into a
//! typed one, which is what HISS-07 asks for.
//!
//! # The four things a reviewer should look at
//!
//! * [`Tier::classify`] is the scheduler's own comparison chain, strict
//!   comparisons included: an average of exactly [`BURST_CRITICAL_NS`] is
//!   **not** critical. The boundary tests pin all three edges rather than the
//!   friendlier reading.
//! * [`ResourceBroker`] holds the table bound and the arbitration:
//!   [`MAX_TRACKED_PROCESSES`] rows, the next one refused, and a focus switch
//!   to a process it does not track returns [`FocusOutcome::UnknownPid`] and
//!   throttles nothing -- the case the scaffold silently accepts.
//! * [`chain`] builds P08's own [`RealtimeGrant`](aegis_calliope::RealtimeGrant)
//!   and consumes P13's own
//!   [`TaskShiftDirective`](aegis_tellus::TaskShiftDirective) rather than
//!   redefining either, reading each owner's edge, bound and policy constants.
//! * [`register`] records what this milestone does not discharge, and
//!   [`P07_PROPOSAL_PINS`] records the two eBPF crate versions the imported
//!   manifest proposes and this repository does **not** inherit.
//!
//! # What is deliberately not claimed
//!
//! **This crate measures nothing.** A burst duration here is a number a caller
//! hands in, never a reading: nothing in it touches a clock, and
//! `tests/stubbed_effects.rs` is the sweep that keeps it so.
//!
//! What changed at milestone M23 is not that, but what the crate may be handed.
//! [`determinism`] evaluates a figure `make verify-latency` measured -- on a
//! kernel that figure names -- against the tier edges [`tier`] declares. The
//! evaluation refuses any figure from a kernel without `CONFIG_PREEMPT_RT`
//! before it looks at its value, which matters on the reference profile because
//! the `PREEMPT_DYNAMIC` host measured the **lower** worst case of the two.
//! REQ-P07-01's deterministic tier-0 response is therefore still not claimed:
//! the guest run exceeded the critical edge and is reported as exceeding it.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion -- **checked by review, not by a gate**: every function here
//!   is a flat check, a bounded sweep or a field access, and no function in
//!   the crate calls itself directly or through another. This is the one
//!   bullet in this list with no falsifier behind it. The M07 verification
//!   planted direct recursion and mutual recursion in this crate and both
//!   passed `cargo clippy -- -D warnings` and `praetorctl audit`, which
//!   reported no violation, while planted controls for the other invariants
//!   failed as they should. Read it as a reviewed statement about the code as
//!   written, not as a property this repository can currently enforce;
//! * every loop carries a scalar upper bound: [`MAX_TRACKED_PROCESSES`],
//!   [`CORE_MASK_WIDTH`], [`MAX_LABEL_LEN`], [`MAX_SLICE_LEN`],
//!   [`MAX_CORRELATION_LEN`] and [`MAX_CONTRACT_PAYLOAD_BYTES`];
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
//! It schedules nothing and brokers nothing. There is no eBPF program, no
//! `aya` or `libbpf` dependency, no `sched_ext` attachment, no `cgroup` write,
//! no `sched_setaffinity`, no `drm_sched` priority, no accelerator, no D-Bus
//! connection, no async runtime and no clock read anywhere in it.
//! `tests/stubbed_effects.rs` sweeps the crate's own sources for a recorded
//! list of identifiers that would be needed to do any of it and fails if one
//! appears -- a regression gate over an enumeration, not a proof over every
//! such identifier.
//!
//! A pass of this crate's tests is evidence about the classifier, the table,
//! the bounds and the payloads. It closes no kernel, eBPF, latency or
//! transport gate.
//!
//! # Example
//!
//! ```
//! use aegis_lictor::{
//!     BURST_CRITICAL_NS, EwmaBurst, FocusOutcome, Label, Pid, ProcessTier,
//!     ResourceBroker, Tier,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! // The source comparison is strict: exactly at the edge is the tier above.
//! assert_eq!(Tier::classify(EwmaBurst::seeded(BURST_CRITICAL_NS - 1)), Tier::Critical);
//! assert_eq!(Tier::classify(EwmaBurst::seeded(BURST_CRITICAL_NS)), Tier::Interactive);
//!
//! let mut broker = ResourceBroker::new();
//! let compositor = Pid::new(1001)?;
//! broker.register(
//!     compositor,
//!     Label::parse("aegis-compositor")?,
//!     ProcessTier::T0WaylandCompositor,
//!     256,
//! )?;
//! broker.register(
//!     Pid::new(2002)?,
//!     Label::parse("aegis-minerva-slm")?,
//!     ProcessTier::T2BackgroundAgent,
//!     4096,
//! )?;
//!
//! assert_eq!(
//!     broker.on_focus_change(compositor),
//!     FocusOutcome::Switched { pid: compositor, throttled: 1 }
//! );
//!
//! // A process the broker does not track throttles nothing.
//! let stranger = Pid::new(4242)?;
//! assert_eq!(broker.on_focus_change(stranger), FocusOutcome::UnknownPid { pid: stranger });
//! # Ok(())
//! # }
//! ```

pub mod broker;
pub mod chain;
pub mod contracts;
pub mod determinism;
pub mod error;
pub mod id;
pub mod locality;
pub mod probe;
pub mod register;
pub mod tier;

pub use crate::broker::{
    EBPF_TASK_MAP_ENTRIES, FocusOutcome, MAX_PID, MAX_TRACKED_PROCESSES, Pid, ProcessRecord,
    ProcessTier, ResourceBroker,
};
pub use crate::chain::{GrantDraft, TaskShiftAction, act_on_task_shift};
pub use crate::contracts::focus_switch::{FocusSwitchReport, FocusSwitchVersion};
pub use crate::contracts::graph::EdgeId;
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::determinism::{
    BOUNDED_TIER_COUNT, TierDeterminism, satisfied_edge_count, strictest_satisfied_tier,
    tier_determinism,
};
pub use crate::error::LictorError;
pub use crate::id::{
    CorrelationId, IdError, Label, MAX_CORRELATION_LEN, MAX_LABEL_LEN, MAX_SLICE_LEN, SLICE_SUFFIX,
    SliceName,
};
pub use crate::locality::{CORE_MASK_WIDTH, CoreMask};
pub use crate::probe::{FragilityProbe, ProbeStage};
pub use crate::register::{
    ClaimSource, ClaimStatus, P07_PROPOSAL_PINS, P07_RECORDED_CLAIMS, ProposalPin, RecordedClaim,
};
pub use crate::tier::{
    BURST_CRITICAL_NS, BURST_FRAME_NS, BURST_INTERACTIVE_NS, DispatchQueue, EWMA_DIVISOR,
    EWMA_WEIGHT_NEW, EWMA_WEIGHT_OLD, EwmaBurst, Tier,
};
