// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! One inbound session: lines in, the shell state updated, Responses out.
//!
//! [`serve`] reads line after line from a [`DeadlineStream`], decodes each
//! through the consumers and applies it to the [`ShellState`]. A refused line
//! never ends the session: it is answered, when the specification says it
//! must be, and the next line is read. Answers follow JSON-RPC 2.0: an
//! invalid Request is always answered, with its id when it could be read; a
//! call (a Request with an id) is answered with a result or an error; a
//! Notification is never answered, and a refused one is reported in the
//! [`SessionReport`] with its typed error instead.
//!
//! The session ends when the peer closes the stream, when a read misses its
//! deadline, or after `max_lines` lines, whichever comes first (HISS-02).
//! Every read and every write arms its deadline first. Nothing here opens a
//! socket: the caller hands in a stream, which the tests make with
//! `socketpair(2)`.

use core::time::Duration;

use serde_json::json;
use thiserror::Error;

use crate::consumers::{self, ConsumeError, Inbound};
use crate::jsonrpc::{
    DeadlineStream, EnvelopeError, Frame, FrameError, LINE_TOO_LONG, LineReader, MAX_LINE_BYTES,
    RequestId, STATE_REFUSED, classify, error_line, parse_line, result_line,
};
use crate::state::{Applied, ShellState, StateError};

/// The deadline every read and every write carries by default.
pub const IO_DEADLINE: Duration = Duration::from_millis(250);

/// The most lines one session reads by default.
pub const MAX_LINES_PER_SESSION: usize = 1_024;

/// A session's bounds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// The deadline each read carries.
    pub read_deadline: Duration,
    /// The deadline each write carries.
    pub write_deadline: Duration,
    /// The most lines read before the session ends.
    pub max_lines: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            read_deadline: IO_DEADLINE,
            write_deadline: IO_DEADLINE,
            max_lines: MAX_LINES_PER_SESSION,
        }
    }
}

/// Why a line was refused.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Refusal {
    /// The line exceeded [`MAX_LINE_BYTES`].
    #[error("the line ran to {length} bytes, past the {MAX_LINE_BYTES}-byte bound")]
    TooLong {
        /// Its length.
        length: usize,
    },
    /// The peer ended the stream in the middle of a line.
    #[error("the stream ended without a newline after the last line")]
    Unterminated,
    /// The line is not a valid Request.
    #[error(transparent)]
    Envelope(EnvelopeError),
    /// The method or the payload was refused by a consumer.
    #[error(transparent)]
    Consume(ConsumeError),
    /// The shell state refused the decoded message.
    #[error(transparent)]
    State(StateError),
}

impl Refusal {
    /// The JSON-RPC error code an answer carries.
    #[must_use]
    pub const fn code(&self) -> i64 {
        match self {
            Self::TooLong { .. } => LINE_TOO_LONG,
            Self::Unterminated => EnvelopeError::Parse.code(),
            Self::Envelope(error) => error.code(),
            Self::Consume(error) => error.code(),
            Self::State(_) => STATE_REFUSED,
        }
    }
}

/// What became of one line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineOutcome {
    /// The message was decoded and applied.
    Applied(Applied),
    /// The line was refused; `answered` says whether a Response went back.
    Refused {
        /// Why.
        refusal: Refusal,
        /// Whether an error Response was written.
        answered: bool,
    },
}

/// How a session ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionEnd {
    /// The peer closed the stream.
    EndOfStream,
    /// A read missed its deadline.
    Deadline,
    /// `max_lines` lines were read.
    LineLimit,
}

/// Why a session failed rather than ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum SessionError {
    /// A read failed other than by missing its deadline.
    #[error(transparent)]
    Read(FrameError),
    /// A write failed or missed its deadline.
    #[error("writing a Response failed: {0}")]
    Write(FrameError),
}

/// Everything one session did, line by line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionReport {
    /// One outcome per line, in arrival order.
    pub outcomes: Vec<LineOutcome>,
    /// Responses written.
    pub responses: usize,
    /// How the session ended.
    pub end: SessionEnd,
}

/// Writes one Response under the write deadline.
fn answer<S: DeadlineStream>(
    stream: &mut S,
    limits: &Limits,
    line: &[u8],
) -> Result<(), SessionError> {
    stream
        .arm_write_deadline(limits.write_deadline)
        .map_err(|error| SessionError::Write(classify(&error)))?;
    stream
        .write_all(line)
        .and_then(|()| stream.flush())
        .map_err(|error| SessionError::Write(classify(&error)))
}

/// Decodes and applies one Request; returns the outcome and, for a call, the
/// Response.
fn handle_request(
    state: &mut ShellState,
    request: &crate::jsonrpc::Request,
) -> (LineOutcome, Option<Vec<u8>>) {
    let applied = consumers::decode(&request.method, request.params.as_ref())
        .map_err(Refusal::Consume)
        .and_then(|inbound| apply(state, inbound));
    match (applied, &request.id) {
        (Ok(applied), Some(id)) => {
            let line = result_line(id, &json!({ "applied": request.method }));
            (LineOutcome::Applied(applied), Some(line))
        }
        (Ok(applied), None) => (LineOutcome::Applied(applied), None),
        (Err(refusal), Some(id)) => {
            let line = error_line(id, refusal.code(), &refusal.to_string());
            (
                LineOutcome::Refused {
                    refusal,
                    answered: true,
                },
                Some(line),
            )
        }
        (Err(refusal), None) => (
            LineOutcome::Refused {
                refusal,
                answered: false,
            },
            None,
        ),
    }
}

/// Applies a decoded message to the state.
fn apply(state: &mut ShellState, inbound: Inbound) -> Result<Applied, Refusal> {
    state.apply(inbound).map_err(Refusal::State)
}

/// The outcome and answer for a refusal that is always answered, with `id`.
fn always_answered(refusal: Refusal, id: &RequestId) -> (LineOutcome, Option<Vec<u8>>) {
    let line = error_line(id, refusal.code(), &refusal.to_string());
    (
        LineOutcome::Refused {
            refusal,
            answered: true,
        },
        Some(line),
    )
}

/// What to do with one frame; `None` once the stream ended.
fn handle_frame(
    state: &mut ShellState,
    frame: Frame<'_>,
) -> Option<(LineOutcome, Option<Vec<u8>>)> {
    match frame {
        Frame::End => None,
        Frame::TooLong { length } => Some(always_answered(
            Refusal::TooLong { length },
            &RequestId::Null,
        )),
        Frame::Unterminated(_) => Some(always_answered(Refusal::Unterminated, &RequestId::Null)),
        Frame::Line(line) => Some(match parse_line(line) {
            Ok(request) => handle_request(state, &request),
            Err((error, id)) => always_answered(Refusal::Envelope(error), &id),
        }),
    }
}

/// Serves one session; see the module documentation.
///
/// # Errors
///
/// Returns [`SessionError::Read`] for a read that fails other than by its
/// deadline, and [`SessionError::Write`] for a Response that cannot be
/// written within its deadline.
pub fn serve<S: DeadlineStream>(
    stream: &mut S,
    state: &mut ShellState,
    limits: &Limits,
) -> Result<SessionReport, SessionError> {
    let mut reader = LineReader::new();
    let mut report = SessionReport {
        outcomes: Vec::new(),
        responses: 0,
        end: SessionEnd::LineLimit,
    };
    for _ in 0..limits.max_lines {
        let frame = match reader.next_frame(stream, limits.read_deadline) {
            Ok(frame) => frame,
            Err(FrameError::Deadline) => {
                report.end = SessionEnd::Deadline;
                return Ok(report);
            }
            Err(error) => return Err(SessionError::Read(error)),
        };
        let Some((outcome, response)) = handle_frame(state, frame) else {
            report.end = SessionEnd::EndOfStream;
            return Ok(report);
        };
        if let Some(line) = response {
            answer(stream, limits, &line)?;
            report.responses = report.responses.saturating_add(1);
        }
        report.outcomes.push(outcome);
    }
    Ok(report)
}
