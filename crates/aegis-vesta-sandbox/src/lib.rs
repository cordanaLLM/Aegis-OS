// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P10 `aegis-vesta-sandbox`: the sandbox path on real KVM
//! (milestone M21, epic E21-2).
//!
//! `aegis-vesta` (M06) holds the bounded microVM table and the candidate
//! evaluation P10 hands to P16, and starts nothing. This crate is the half
//! that does start something, and it starts nothing the table did not admit:
//!
//! * [`run::run_sandbox`] is the host run `make verify-workstation` starts.
//!   Every microVM is a row [`aegis_vesta::MicroVmController`] admitted first;
//!   [`vm::RunningVm`] boots it with the pinned Firecracker (D58, 1.17.0)
//!   from a per-VM configuration file ([`config`]) with `--no-api`, no drive,
//!   no network interface and one vsock device, whose guest context
//!   identifier is the one the controller derived. The 65th request is the
//!   controller's refusal, and no process is started for it.
//! * [`guest::serve`] is the service `aegis-vesta-guest` runs as the microVM's
//!   init: it listens on `AF_VSOCK`, judges each candidate with P16's Pareto
//!   gate ([`evaluate::answer`]) and answers with the
//!   `aegis.p10-p16.candidate-evaluation.v1` payload.
//! * [`mod@line`] frames both directions and Firecracker's hybrid-vsock handshake:
//!   the host end is `AF_UNIX`, the guest end `AF_VSOCK`, and Firecracker's
//!   own device model carries the bytes between them (its `docs/vsock.md`).
//! * [`transport`] (M22, E22-2) times that round trip into a figure that names
//!   its monitor, and reads the host's vhost state to hold that the transport
//!   bypasses `vhost_vsock`, as that document says.
//! * [`boot`] (M22, E22-1) reads each Firecracker process's resident set
//!   after its first answer into a figure that names the monitor the process
//!   was found to run, and parses the boot timer line a boot time figure will
//!   come from.
//!
//! The process, its configuration, the framing and the guest listener moved
//! to `saeptum-microvm` (cut unit CUT-1632-04), consumed by git revision from
//! the workspace manifest and re-exported under the same paths; their tests
//! moved with them. This crate keeps the evaluation payload, P16's judgement
//! and the seven host cases.
//!
//! # What this crate does not claim
//!
//! It measures no boot time yet: the guest's write to Firecracker's boot timer
//! needs a mapped store to a device address, which `unsafe_code = "forbid"`
//! rules out in this workspace, and that exception is an open decision. The
//! footprint is the monitor process's resident set, not the guest's view of
//! its memory, and is compared with no target. The round-trip figure is a wall-clock time, not a price: it prices no
//! `AF_VSOCK` traffic (REQ-P10-03's pricing is not implemented) and passes no
//! accelerator through (REQ-P10-02, Venus, is deferred to M12 by E21-3). It
//! uses no jailer and runs every microVM as the invoking user. Every pass is
//! development evidence on the reference profile and closes no hardware,
//! isolation or release gate.

pub mod boot;
pub mod error;
pub mod evaluate;
pub mod guest;
pub mod request;
pub mod run;
pub mod transport;
pub mod vm;

pub use saeptum_microvm::{config, line};

/// The guest memory every microVM of the reference run is given, in MiB.
///
/// The smallest power of two the pinned kernel boots in with this crate's
/// initramfs: on 2026-09-29 a probe booted 60 MiB and 64 MiB guests and saw
/// 48 MiB and 56 MiB guests panic out of memory before init. It is a
/// configuration value, not a footprint measurement (D71 leaves those to M22).
pub const REFERENCE_GUEST_MEMORY_MIB: u32 = 64;

pub use crate::boot::{
    BOOT_COMPLETE, BOOT_TIME_MARKER, BOOT_TIMER_MMIO, FootprintFigure, FootprintSample,
    MAX_STATUS_BYTES, identify, measure, parse_boot_time, parse_resident_kib,
};
pub use crate::config::{
    BOOT_ARGS, MAX_SOCKET_PATH_BYTES, VCPU_COUNT, VmConfig, check_socket_path, render,
};
pub use crate::error::SandboxError;
pub use crate::evaluate::{answer, check_answer, expected_verdict, vm_id_for_cid};
pub use crate::guest::{
    ACCEPT_BUDGET, BACKLOG, GUEST_EXCHANGE_BUDGET, MAX_GUEST_CONNECTIONS, VMADDR_CID_ANY, respond,
    serve,
};
pub use crate::line::{
    MAX_ACK_BYTES, Unframed, connect_command, deadline_error, parse_ack, read_line, timed_out,
};
pub use crate::request::{
    EVALUATION_PORT, EvaluationRequest, MAX_REQUEST_BYTES, REQUEST_SCHEMA_TAG, RequestVersion,
    WireMetrics,
};
pub use crate::run::{
    AT_LATENCY_BOUND, HOST_CASES, SUPERIOR, Verdict, admission, footprint, over_limit_memory,
    request, run_sandbox, sixty_fifth,
};
pub use crate::transport::{
    BOOT_CASE_EXCHANGES, HostVsock, ModuleBuild, ROUND_TRIPS, RoundTripFigure, VHOST_DEVICE,
    VHOST_MODULE, check_mechanism, micros, time_round_trips,
};
pub use crate::vm::{
    BOOT_BUDGET, EXCHANGE_BUDGET, HANDSHAKE_BUDGET, Launch, MAX_ANSWER_BYTES,
    MAX_DIRECTORY_ENTRIES, MAX_POLLS, POLL, RunningVm, STOP_BUDGET, Stopped, evaluate_in,
    reference_memory,
};
pub use saeptum_microvm::MicroVmError;
