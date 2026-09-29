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
//!
//! # What this crate does not claim
//!
//! It measures no boot time and no memory footprint: those are M22's under
//! decision D71, each naming the monitor that produced it. It prices no
//! `AF_VSOCK` traffic (REQ-P10-03's pricing is not implemented) and passes no
//! accelerator through (REQ-P10-02, Venus, is deferred to M12 by E21-3). It
//! uses no jailer and runs every microVM as the invoking user. Every pass is
//! development evidence on the reference profile and closes no hardware,
//! isolation or release gate.

pub mod config;
pub mod error;
pub mod evaluate;
pub mod guest;
pub mod line;
pub mod request;
pub mod run;
pub mod vm;

/// The guest memory every microVM of the reference run is given, in MiB.
///
/// The smallest power of two the pinned kernel boots in with this crate's
/// initramfs: on 2026-09-29 a probe booted 60 MiB and 64 MiB guests and saw
/// 48 MiB and 56 MiB guests panic out of memory before init. It is a
/// configuration value, not a footprint measurement (D71 leaves those to M22).
pub const REFERENCE_GUEST_MEMORY_MIB: u32 = 64;

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
    AT_LATENCY_BOUND, HOST_CASES, SUPERIOR, Verdict, admission, over_limit_memory, request,
    run_sandbox, sixty_fifth,
};
pub use crate::vm::{
    BOOT_BUDGET, EXCHANGE_BUDGET, HANDSHAKE_BUDGET, Launch, MAX_ANSWER_BYTES,
    MAX_DIRECTORY_ENTRIES, MAX_POLLS, POLL, RunningVm, STOP_BUDGET, Stopped, reference_memory,
};
