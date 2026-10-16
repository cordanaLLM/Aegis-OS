// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The guest side: the evaluation service `aegis-vesta-guest` runs as the
//! microVM's init.
//!
//! The listener, its bounds and its framing are [`saeptum_microvm::serve`]
//! (cut unit CUT-1632-04): it listens on `AF_VSOCK` port [`EVALUATION_PORT`]
//! for any local context, answers at most [`MAX_GUEST_CONNECTIONS`]
//! connections and returns when that bound is reached or no connection
//! arrives within [`ACCEPT_BUDGET`]; the binary then restarts the guest, which
//! Firecracker answers by exiting. What stays here is [`respond`], the P16
//! judgement each request line gets.
//!
//! A request that fails is answered with one `ERROR <reason>` line rather
//! than silence, so the host reports the guest's reason.

use std::io::Write;
use std::time::{SystemTime, UNIX_EPOCH};

use aegis_justitia::UnixSeconds;
use aegis_vesta::PayloadBuffer;

use saeptum_microvm::guest::Service;
pub use saeptum_microvm::guest::{
    ACCEPT_BUDGET, BACKLOG, GUEST_EXCHANGE_BUDGET, MAX_GUEST_CONNECTIONS, VMADDR_CID_ANY,
};

use crate::error::SandboxError;
use crate::evaluate::answer;
use crate::request::{EVALUATION_PORT, EvaluationRequest, MAX_REQUEST_BYTES};

/// The port and request bound the evaluation service listens with.
const EVALUATION_SERVICE: Service = Service {
    port: EVALUATION_PORT,
    max_request_bytes: MAX_REQUEST_BYTES,
};

/// Binds the evaluation port and answers until the bound or the budget.
///
/// Returns how many connections were answered.
///
/// # Errors
///
/// Returns [`SandboxError::MicroVm`] when the socket cannot be created, bound
/// or listened on, or `accept` fails for a reason other than its timeout.
pub fn serve(log: &mut dyn Write) -> Result<u32, SandboxError> {
    let mut responder = respond;
    Ok(saeptum_microvm::serve(
        EVALUATION_SERVICE,
        log,
        &mut responder,
    )?)
}

/// Decodes, evaluates and encodes one request line.
///
/// Returns the answer without its newline; the service frames it.
///
/// # Errors
///
/// Returns [`SandboxError`] for a request the decoder or [`answer`] refuses,
/// a guest clock before the epoch, or an answer the contract will not encode.
pub fn respond(line: &str, cid: u32) -> Result<String, SandboxError> {
    let request = EvaluationRequest::decode_line(line)?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|_| SandboxError::Refused("the guest clock reads before the epoch".to_owned()))?;
    let evaluation = answer(&request, cid, UnixSeconds::new(now.as_secs()))?;
    let mut buffer = PayloadBuffer::new();
    let text = evaluation.encode_into(&mut buffer)?;
    Ok(text.to_owned())
}
