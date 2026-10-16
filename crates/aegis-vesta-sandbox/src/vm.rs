// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! One candidate evaluation sent to a running microVM, and the reference
//! guest memory.
//!
//! The process itself -- started from a row
//! [`aegis_vesta::MicroVmController`] admitted, reached through the hybrid
//! vsock, killed, reaped and its sockets removed -- is
//! [`saeptum_microvm::RunningVm`] (cut unit CUT-1632-04), re-exported here
//! under its old path. What stays is the evaluation payload on top of it.

use aegis_vesta::{CandidateEvaluation, GuestMemoryMib, MAX_CONTRACT_PAYLOAD_BYTES};

pub use saeptum_microvm::vm::{
    BOOT_BUDGET, EXCHANGE_BUDGET, HANDSHAKE_BUDGET, Launch, MAX_DIRECTORY_ENTRIES, MAX_POLLS, POLL,
    RunningVm, SOCKET_NAME, STOP_BUDGET, Stopped,
};

use crate::error::SandboxError;
use crate::evaluate::check_answer;
use crate::request::{EVALUATION_PORT, EvaluationRequest};

/// Scalar upper bound, in bytes, on one answer line: the contract's payload
/// bound and its newline.
pub const MAX_ANSWER_BYTES: usize = MAX_CONTRACT_PAYLOAD_BYTES + 1;

/// Sends one request to the guest of `vm` and returns its checked answer.
///
/// # Errors
///
/// Returns [`SandboxError::MicroVm`] when the guest does not accept within
/// [`BOOT_BUDGET`], the exchange passes [`EXCHANGE_BUDGET`] or the guest
/// answers `ERROR`; [`SandboxError::Contract`] when the payload fails its
/// contract; and [`SandboxError::Refused`] when [`check_answer`] refuses it.
pub fn evaluate_in(
    vm: &mut RunningVm,
    request: &EvaluationRequest,
) -> Result<CandidateEvaluation, SandboxError> {
    let line = request.encode_line()?;
    let body = vm.exchange(
        EVALUATION_PORT,
        line.trim_end_matches('\n'),
        MAX_ANSWER_BYTES,
    )?;
    let answer = CandidateEvaluation::decode(&body)?;
    check_answer(request, &answer)?;
    Ok(answer)
}

/// Returns the guest memory every microVM of the reference run is given.
///
/// # Errors
///
/// Propagates [`GuestMemoryMib::new`].
pub fn reference_memory() -> Result<GuestMemoryMib, SandboxError> {
    Ok(GuestMemoryMib::new(crate::REFERENCE_GUEST_MEMORY_MIB)?)
}
