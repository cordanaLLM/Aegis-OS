// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P10 `aegis-vesta`: the bounded sandbox controllers and decision D06.
//!
//! The imported P10 scaffold (export-037 `ce490c88081f`) is a daemon that
//! prints what it would do: it creates sandbox rows with a hard-coded boot
//! time, registers capsules for a Go `WebAssembly` runtime it never loads, and
//! polls a ring that is two counters. Milestone M06 extracts the part that can
//! be checked without `KVM`, a monitor binary, a `WebAssembly` engine or
//! `io_uring` -- the tables, the bounds and the payloads -- and turns each
//! `&'static str` refusal into a typed one, which is what HISS-07 asks for.
//!
//! # The four things a reviewer should look at
//!
//! * [`MicroVmController`] holds REQ-P10-01's sandbox bound: 64 rows, the 65th
//!   refused, and terminating an identifier the table does not hold returns
//!   `false` rather than an error nobody reads.
//! * [`CapsuleRegistry`] holds the capsule bound the same way, and
//!   [`CapsuleSlot`] puts that bound on the P09 payload field as well as on the
//!   table, so a request naming slot 129 never decodes.
//! * [`decision`] records D06 -- a Rust-native `WebAssembly` runtime, no Go
//!   dependency and no protocol adapter -- with its sources cited by export
//!   identifier and digest prefix, and [`AdmittedRuntime`] makes the rejected
//!   runtime unrepresentable on the wire.
//! * [`VmmIdentity`] is on both payloads because decision D58 requires every
//!   measurement to record which monitor produced it, and [`VMM_PROBES`]
//!   records what the two probes returned on the reference profile.
//!
//! # The boot-time literal is not a measurement
//!
//! [`SCAFFOLD_BOOT_TIME_MS`], [`BOOT_TIME_TARGET_MS`] and
//! [`FOOTPRINT_TARGET_MIB`] are [`Unmeasured`] values. Nothing in this
//! repository has booted a microVM; the figures are recorded because the
//! sources state them, and REQ-P10-05 itself records that they are unmeasured.
//! `tests/unmeasured_literals.rs` is what keeps them that way.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: every function here is a flat check, a bounded sweep or a
//!   field access, and no function in the crate calls itself directly or
//!   through another;
//! * every loop carries a scalar upper bound: [`MAX_MICROVMS`],
//!   [`MAX_WASM_CAPSULES`], [`MAX_CAPABILITY_ENTRIES`] and
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
//! It starts nothing and isolates nothing. There is no `KVM` ioctl, no
//! Firecracker or `QEMU` process, no jailer, no monitor API socket, no guest
//! kernel, no `WebAssembly` engine, no `AF_VSOCK` connection, no `io_uring`
//! ring and no filesystem access anywhere in it. A sandbox is a row in a fixed
//! array and a capsule is another. `tests/stubbed_effects.rs` sweeps the
//! crate's own sources for a recorded list of identifiers that would be needed
//! to do any of it and fails if one appears -- a regression gate over an
//! enumeration, not a proof over every such identifier.
//!
//! A pass of this crate's tests is evidence about the tables, the bounds and
//! the payloads. It closes no hardware, boot or isolation gate: REQ-P10-02 and
//! REQ-P10-03 stay deferred hardware-backed requirements, recorded in
//! [`register`].
//!
//! # Example
//!
//! ```
//! use aegis_vesta::{
//!     CapabilitySet, Capability, CapsuleMemoryLimit, CapsuleRegistry, GuestMemoryMib, Label,
//!     MicroVmController, MicroVmRequest, VmmIdentity,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut sandboxes = MicroVmController::new();
//! let request = MicroVmRequest::new(
//!     Label::parse("aegis-agent-microvm-01")?,
//!     GuestMemoryMib::new(4)?,
//!     false,
//!     VmmIdentity::Firecracker,
//! )?;
//! let vm = sandboxes.spawn(request)?;
//! assert_eq!(sandboxes.count(), 1);
//! assert!(sandboxes.terminate(vm));
//! assert!(!sandboxes.terminate(aegis_vesta::VmId::new(999)));
//!
//! let mut capsules = CapsuleRegistry::new();
//! let slot = capsules.register(
//!     Label::parse("aegis-capsule-01")?,
//!     CapsuleMemoryLimit::new(16 * 1024 * 1024)?,
//!     CapabilitySet::new().with(Capability::FilesystemRead),
//! )?;
//! assert_eq!(slot.get(), 1);
//! # Ok(())
//! # }
//! ```

pub mod capsule;
pub mod contracts;
pub mod decision;
pub mod error;
pub mod id;
pub mod microvm;
pub mod register;
pub mod ring;
pub mod unmeasured;
pub mod vmm;

pub use crate::capsule::{
    Capability, CapabilitySet, CapsuleMemoryLimit, CapsuleRegistry, CapsuleSlot,
    MAX_CAPSULE_MEMORY_BYTES, MAX_WASM_CAPSULES, MIN_CAPSULE_MEMORY_BYTES, WasmCapsule,
};
pub use crate::contracts::candidate_evaluation::{
    CandidateEvaluation, CandidateEvaluationVersion, EvaluationVerdict,
};
pub use crate::contracts::capsule_request::{CapsuleRequest, CapsuleRequestVersion};
pub use crate::contracts::encoding::MAX_CAPABILITY_ENTRIES;
pub use crate::contracts::graph::EdgeId;
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::decision::{
    AdmittedRuntime, Citation, D06_WASM_RUNTIME, DecisionRecord, DecisionState, WasmRuntimeChoice,
};
pub use crate::error::VestaError;
pub use crate::id::{CorrelationId, IdError, Label, MAX_CORRELATION_LEN, MAX_LABEL_LEN};
pub use crate::microvm::{
    BOOT_TIME_TARGET_MS, FIRST_GUEST_CID, FIRST_VM_ID, FOOTPRINT_TARGET_MIB, GuestMemoryMib,
    MAX_GUEST_MEMORY_MIB, MAX_MICROVMS, MIN_GUEST_MEMORY_MIB, MicroVmController, MicroVmInstance,
    MicroVmRequest, SCAFFOLD_BOOT_TIME_MS, VmId, VmState, VsockCid,
};
pub use crate::register::{ClaimSource, ClaimStatus, P10_RECORDED_CLAIMS, RecordedClaim};
pub use crate::ring::{IOURING_QUEUE_DEPTH, RingCore};
pub use crate::unmeasured::Unmeasured;
pub use crate::vmm::{VMM_PROBES, VmmIdentity, VmmProbe};
