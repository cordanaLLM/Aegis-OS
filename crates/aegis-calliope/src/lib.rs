// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P08 `aegis-calliope`: the real-time multimedia slice, minus the
//! real time.
//!
//! The imported P08 scaffold (export-026 `c2f1e433cd32`) is a daemon that
//! prints that it is setting `RLIMIT_RTPRIO`, hands back a DMA-BUF descriptor
//! carrying the literal file descriptor `42`, loads "sandboxed" plugins by
//! setting a boolean, and sleeps in a loop while reporting a latency it never
//! measured. Milestone M07 extracts the part that can be checked without
//! `PipeWire`, an accelerator, a plugin bridge or a real-time kernel -- the
//! tables, the bounds, the lifecycle and the payloads -- and turns each string
//! refusal into a typed one, which is what HISS-07 asks for.
//!
//! # The four things a reviewer should look at
//!
//! * [`PluginHost`] holds the slot bound and the tolerance lifecycle: 32
//!   slots, the 33rd refused, and [`PluginStage::may_advance_to`] is where
//!   REQ-P08-03's "Quarantine before Activation" is enforced rather than
//!   described.
//! * [`DmaBufTable`] holds the descriptor bound and the stride rule: 64
//!   descriptors, the 65th refused, and [`stride_bytes`] is checked, so a
//!   width above `u32::MAX / 4` is [`CalliopeError::StrideOverflow`] rather
//!   than a wrapped product.
//! * [`rtprio`] carries the grant P07 hands P08 and, beside it, what the
//!   reference machine actually offers: `ulimit -r` read **99** and
//!   `ulimit -l` read **8192** kibibytes, so the priority requirement is
//!   already reachable and the memlock requirement is not.
//! * [`decision`] records D29, which stays **open**: both directions of the
//!   P08/P11 capture edge remain representable, because settling it means
//!   owning a timing budget and this milestone measures no time.
//!
//! # What is deliberately not claimed
//!
//! **No latency or determinism figure is produced by this crate.** The
//! reference kernel is `PREEMPT_DYNAMIC` and not `PREEMPT_RT`, so a number
//! measured here would describe a machine the requirement does not target;
//! milestone M23 is where the P07 and P08 latency fixtures run. Every recorded
//! target -- the 5 ms round trip, the 48000 Hz rate, the 64-sample quantum --
//! is a [`Declared`] value whose `Display` says so, and
//! `tests/declared_literals.rs` fails to compile if one becomes a bare
//! integer. [`quantum_latency_micros`] is arithmetic on two declared constants
//! and is not a latency anyone observed.
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
//! * every loop carries a scalar upper bound: [`MAX_PLUGIN_SLOTS`],
//!   [`MAX_DMA_BUFFERS`], [`MAX_DRIFT_SAMPLES`], [`MAX_LABEL_LEN`],
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
//! It plays nothing and captures nothing. There is no `PipeWire` connection, no
//! `SPA` buffer, no DMA-BUF export or import, no file descriptor, no plugin
//! bridge or subprocess, no `setrlimit` or `sched_setscheduler`, no `cgroup`
//! write, no accelerator and no clock read anywhere in it.
//! `tests/stubbed_effects.rs` sweeps the crate's own sources for a recorded
//! list of identifiers that would be needed to do any of it and fails if one
//! appears -- a regression gate over an enumeration, not a proof over every
//! such identifier.
//!
//! A pass of this crate's tests is evidence about the tables, the bounds, the
//! lifecycle and the payloads. It closes no hardware, latency or transport
//! gate.
//!
//! # Example
//!
//! ```
//! use aegis_calliope::{
//!     CalliopeError, DmaBufTable, FrameGeometry, GrantedRtPrio, Label, PixelFormat,
//!     PluginFormat, PluginHost, PluginStage,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut host = PluginHost::new();
//! let slot = host.admit(Label::parse("calf-studio-reverb")?, PluginFormat::Lv2)?;
//! assert_eq!(host.get(slot).map(|row| row.stage), Some(PluginStage::Recognition));
//!
//! // REQ-P08-03: Quarantine is not optional.
//! assert!(host.advance(slot, PluginStage::Activation).is_err());
//! host.advance(slot, PluginStage::Quarantine)?;
//! host.advance(slot, PluginStage::Activation)?;
//! assert_eq!(host.in_realtime_graph(), 1);
//!
//! let mut buffers = DmaBufTable::new();
//! let id = buffers.register(FrameGeometry::new(2560, 1440), PixelFormat::Argb8888)?;
//! assert_eq!(buffers.get(id).map(|frame| frame.stride_bytes), Some(10_240));
//!
//! // The source authorises 95 and nothing above it.
//! assert!(GrantedRtPrio::new(95).is_ok());
//! assert!(matches!(
//!     GrantedRtPrio::new(96),
//!     Err(CalliopeError::RtPrioOutOfRange { value: 96, .. })
//! ));
//! # Ok(())
//! # }
//! ```

pub mod contracts;
pub mod decision;
pub mod dmabuf;
pub mod error;
pub mod id;
pub mod latency;
pub mod plugin;
pub mod register;
pub mod rtprio;
pub mod sync;

pub use crate::contracts::dma_buf_stream::{DmaBufStreamDescriptor, DmaBufStreamVersion};
pub use crate::contracts::graph::EdgeId;
pub use crate::contracts::realtime_grant::{RealtimeGrant, RealtimeGrantVersion};
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::decision::{
    CaptureDirection, Citation, D29_REMOTE_PLAY_CAPTURE, DecisionRecord, DecisionState,
};
pub use crate::dmabuf::{
    BufferHandle, BufferId, DmaBufFrame, DmaBufTable, FourCc, FrameGeometry, MAX_DMA_BUFFERS,
    PixelFormat, SCAFFOLD_FOURCC, stride_bytes,
};
pub use crate::error::CalliopeError;
pub use crate::id::{CorrelationId, IdError, Label, MAX_CORRELATION_LEN, MAX_LABEL_LEN};
pub use crate::latency::{
    ADMITTED_SAMPLE_RATES_HZ, DEFAULT_QUANTUM_SAMPLES, DEFAULT_SAMPLE_RATE_HZ, Declared,
    MAX_QUANTUM_SAMPLES, MIN_QUANTUM_SAMPLES, Quantum, SampleRate, TARGET_RTL_LATENCY_MS,
    quantum_latency_micros,
};
pub use crate::plugin::{
    MAX_PLUGIN_SLOTS, PluginFormat, PluginHost, PluginSandboxSlot, PluginSlotId, PluginStage,
};
pub use crate::register::{ClaimSource, ClaimStatus, P08_RECORDED_CLAIMS, RecordedClaim};
pub use crate::rtprio::{
    GrantedRtPrio, MIN_RTPRIO, MemlockExpectation, PrivilegedAction, REFERENCE_PROFILE_LIMITS,
    REFERENCE_PROFILE_MEMLOCK_KIB, REFERENCE_PROFILE_RTPRIO_CEILING, REQUIRED_RTPRIO,
    ReferenceProfileLimits, SchedPolicy, UNIT_SCHED_PRIORITY,
};
pub use crate::sync::{
    DRIFT_DIVISOR, DRIFT_WEIGHT_NEW, DRIFT_WEIGHT_OLD, DriftEstimator, MAX_DRIFT_SAMPLES,
    PhaseOffsetMicros, SyncSource, SyncStrategy,
};
