// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The guest side: the evaluation service `aegis-vesta-guest` runs as the
//! microVM's init.
//!
//! It listens on `AF_VSOCK` port [`EVALUATION_PORT`] for any local context,
//! answers at most [`MAX_GUEST_CONNECTIONS`] connections with one evaluation
//! each, and returns when that bound is reached or no connection arrives
//! within [`ACCEPT_BUDGET`]; the binary then restarts the guest, which
//! Firecracker answers by exiting. Every socket carries a read and a write
//! timeout (HISS-02), and `accept` honours the listener's.
//!
//! A request that fails is answered with one `ERROR <reason>` line rather
//! than silence, so the host reports the guest's reason.

use std::io::Write;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use aegis_justitia::UnixSeconds;
use aegis_vesta::PayloadBuffer;
use socket2::{Domain, SockAddr, Socket, Type};

use crate::error::SandboxError;
use crate::evaluate::answer;
use crate::line::{Unframed, read_line, timed_out};
use crate::request::{EVALUATION_PORT, EvaluationRequest, MAX_REQUEST_BYTES};

/// Scalar upper bound on the connections one guest answers.
pub const MAX_GUEST_CONNECTIONS: u32 = 16;

/// How long the guest waits for the next connection before it ends.
pub const ACCEPT_BUDGET: Duration = Duration::from_secs(120);

/// How long one request and its answer may take inside the guest.
pub const GUEST_EXCHANGE_BUDGET: Duration = Duration::from_secs(5);

/// `VMADDR_CID_ANY`: listen whatever this guest's context identifier is.
pub const VMADDR_CID_ANY: u32 = u32::MAX;

/// The listen backlog.
pub const BACKLOG: i32 = 4;

/// Binds the evaluation port and answers until the bound or the budget.
///
/// Returns how many connections were answered.
///
/// # Errors
///
/// Returns [`SandboxError::Io`] when the socket cannot be created, bound or
/// listened on, or `accept` fails for a reason other than its timeout.
pub fn serve(log: &mut dyn Write) -> Result<u32, SandboxError> {
    let listener = Socket::new(Domain::VSOCK, Type::STREAM, None)
        .map_err(|error| SandboxError::io("creating the AF_VSOCK socket", error))?;
    listener
        .bind(&SockAddr::vsock(VMADDR_CID_ANY, EVALUATION_PORT))
        .and_then(|()| listener.listen(BACKLOG))
        .and_then(|()| listener.set_read_timeout(Some(ACCEPT_BUDGET)))
        .map_err(|error| SandboxError::io("listening on the AF_VSOCK port", error))?;
    let _ = writeln!(
        log,
        "aegis-vesta-guest: listening on vsock port {EVALUATION_PORT}"
    );
    let mut served = 0_u32;
    for _ in 0..MAX_GUEST_CONNECTIONS {
        match listener.accept() {
            Ok((connection, _peer)) => {
                if let Err(error) = handle(&connection) {
                    let _ = writeln!(log, "aegis-vesta-guest: {error}");
                }
                served = served.saturating_add(1);
            }
            Err(error) if timed_out(&error) => break,
            Err(error) => return Err(SandboxError::io("accepting on the AF_VSOCK port", error)),
        }
    }
    Ok(served)
}

/// Answers one connection.
fn handle(connection: &Socket) -> Result<(), SandboxError> {
    connection
        .set_read_timeout(Some(GUEST_EXCHANGE_BUDGET))
        .and_then(|()| connection.set_write_timeout(Some(GUEST_EXCHANGE_BUDGET)))
        .map_err(|error| SandboxError::io("setting a socket timeout", error))?;
    let local = connection
        .local_addr()
        .map_err(|error| SandboxError::io("reading the local vsock address", error))?;
    let (cid, _port) = local
        .as_vsock_address()
        .ok_or_else(|| SandboxError::Protocol("the local address is not AF_VSOCK".to_owned()))?;
    let mut reader = connection;
    let reply = match read_line(
        &mut reader,
        MAX_REQUEST_BYTES,
        GUEST_EXCHANGE_BUDGET,
        "the request",
    )? {
        Ok(line) => respond(&line, cid).unwrap_or_else(|error| format!("ERROR {error}\n")),
        Err(Unframed::Closed(bytes)) => {
            return Err(SandboxError::Protocol(format!(
                "the host closed the stream after {bytes} bytes of its request"
            )));
        }
    };
    let mut writer = connection;
    writer
        .write_all(reply.as_bytes())
        .map_err(|error| SandboxError::io("sending the answer", error))
}

/// Decodes, evaluates and encodes one request.
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
    Ok(format!("{text}\n"))
}
