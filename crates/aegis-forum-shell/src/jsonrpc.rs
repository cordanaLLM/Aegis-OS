// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Line-delimited JSON-RPC 2.0 framing (decision D77).
//!
//! One message is one line: UTF-8 JSON, terminated by a newline, at most
//! [`MAX_LINE_BYTES`] bytes before it. [`LineReader`] cuts a stream into such
//! lines under a byte bound and a per-read deadline (HISS-02), and
//! [`parse_line`] checks a line against the JSON-RPC 2.0 Request object
//! (<https://www.jsonrpc.org/specification>, read 2026-09-29): `jsonrpc`
//! exactly `"2.0"`, `method` a string, `params` a structured value if
//! present, `id` a string, a number or null if present, and a Request with
//! no `id` a Notification.
//!
//! A line that is not a valid Request is answered with an error Response
//! whose `id` is the Request's when it could be read and `null` otherwise,
//! as the specification requires, and the stream continues. A line longer
//! than the bound is answered with [`LINE_TOO_LONG`], a code from the range
//! the specification reserves for implementation-defined server errors, and
//! the reader resynchronises at the next newline. A batch (a JSON array) is
//! answered with [`INVALID_REQUEST`]: D77 carries one message per line.

use std::io;

use serde_json::{Map, Value, json};
use thiserror::Error;

/// The longest line admitted, in bytes, not counting its newline.
///
/// Twice the producers' own payload bound (`MAX_CONTRACT_PAYLOAD_BYTES`,
/// 4096 bytes in both `aegis-justitia` and `aegis-tellus`), which leaves room
/// for the envelope around the largest payload either admits.
pub const MAX_LINE_BYTES: usize = 8_192;

/// How many bytes one read asks for.
pub const READ_CHUNK: usize = 1_024;

/// How many bytes of one over-long line the reader discards while looking
/// for its end, before it gives the stream up.
pub const MAX_DISCARD_BYTES: usize = 1 << 20;

/// JSON-RPC 2.0: invalid JSON was received.
pub const PARSE_ERROR: i64 = -32_700;

/// JSON-RPC 2.0: the JSON sent is not a valid Request object.
pub const INVALID_REQUEST: i64 = -32_600;

/// JSON-RPC 2.0: the method does not exist or is not available.
pub const METHOD_NOT_FOUND: i64 = -32_601;

/// JSON-RPC 2.0: invalid method parameters.
pub const INVALID_PARAMS: i64 = -32_602;

/// Implementation-defined server error: the line exceeded [`MAX_LINE_BYTES`].
pub const LINE_TOO_LONG: i64 = -32_000;

/// Implementation-defined server error: the shell state refused the message.
pub const STATE_REFUSED: i64 = -32_001;

/// A source of bytes whose every read and write carries a deadline.
pub trait DeadlineStream: io::Read + io::Write {
    /// Sets the deadline the next read must complete within.
    ///
    /// # Errors
    ///
    /// Returns the platform's refusal to set it.
    fn arm_read_deadline(&mut self, deadline: core::time::Duration) -> io::Result<()>;

    /// Sets the deadline the next write must complete within.
    ///
    /// # Errors
    ///
    /// Returns the platform's refusal to set it.
    fn arm_write_deadline(&mut self, deadline: core::time::Duration) -> io::Result<()>;
}

/// Why the reader stopped reading the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum FrameError {
    /// A read did not complete within its deadline.
    #[error("a read missed its deadline")]
    Deadline,
    /// A read failed.
    #[error("a read failed: {0:?}")]
    Io(io::ErrorKind),
    /// An over-long line did not end within [`MAX_DISCARD_BYTES`].
    #[error("an over-long line did not end within {MAX_DISCARD_BYTES} bytes")]
    Flood,
}

/// Maps a read or write error onto the deadline or the error's kind.
#[must_use]
pub fn classify(error: &io::Error) -> FrameError {
    match error.kind() {
        io::ErrorKind::WouldBlock | io::ErrorKind::TimedOut => FrameError::Deadline,
        kind => FrameError::Io(kind),
    }
}

/// One unit the reader cut from the stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Frame<'a> {
    /// A complete line, without its newline.
    Line(&'a [u8]),
    /// A line longer than [`MAX_LINE_BYTES`], discarded up to its newline.
    TooLong {
        /// How many bytes it ran to before it ended or was given up.
        length: usize,
    },
    /// Bytes the peer sent and ended the stream after, with no newline.
    Unterminated(&'a [u8]),
    /// The peer ended the stream on a line boundary.
    End,
}

/// Cuts a byte stream into bounded lines.
#[derive(Debug, Clone, Default)]
pub struct LineReader {
    pending: Vec<u8>,
    consumed: usize,
}

impl LineReader {
    /// A reader with its line buffer allocated once, up front.
    #[must_use]
    pub fn new() -> Self {
        Self {
            pending: Vec::with_capacity(MAX_LINE_BYTES.saturating_add(READ_CHUNK)),
            consumed: 0,
        }
    }

    /// Reads until one frame is complete; every read is preceded by arming
    /// `deadline` on the stream.
    ///
    /// # Errors
    ///
    /// Returns [`FrameError::Deadline`] when a read times out,
    /// [`FrameError::Io`] when one fails, and [`FrameError::Flood`] when an
    /// over-long line does not end within [`MAX_DISCARD_BYTES`].
    pub fn next_frame<S: DeadlineStream>(
        &mut self,
        stream: &mut S,
        deadline: core::time::Duration,
    ) -> Result<Frame<'_>, FrameError> {
        self.pending.drain(..self.consumed.min(self.pending.len()));
        self.consumed = 0;
        let mut discarded = 0_usize;
        let mut chunk = [0_u8; READ_CHUNK];
        for _ in 0..=MAX_DISCARD_BYTES {
            if let Some(end) = self.pending.iter().position(|byte| *byte == b'\n') {
                return Ok(self.cut(end, discarded));
            }
            discarded = self.discard_if_over(discarded)?;
            stream
                .arm_read_deadline(deadline)
                .map_err(|error| classify(&error))?;
            let read = stream.read(&mut chunk).map_err(|error| classify(&error))?;
            if read == 0 {
                return Ok(self.at_end(discarded));
            }
            self.pending
                .extend_from_slice(chunk.get(..read).unwrap_or_default());
        }
        Err(FrameError::Flood)
    }

    /// Cuts the line ending at `end`, or reports the over-long line whose
    /// tail it is.
    fn cut(&mut self, end: usize, discarded: usize) -> Frame<'_> {
        self.consumed = end.saturating_add(1);
        let length = discarded.saturating_add(end);
        if discarded > 0 || end > MAX_LINE_BYTES {
            return Frame::TooLong { length };
        }
        Frame::Line(self.pending.get(..end).unwrap_or_default())
    }

    /// Drops the buffered bytes of a line already past the bound, counting
    /// them, and returns the new count.
    fn discard_if_over(&mut self, discarded: usize) -> Result<usize, FrameError> {
        if discarded == 0 && self.pending.len() <= MAX_LINE_BYTES {
            return Ok(0);
        }
        let total = discarded.saturating_add(self.pending.len());
        self.pending.clear();
        if total > MAX_DISCARD_BYTES {
            return Err(FrameError::Flood);
        }
        Ok(total)
    }

    /// The frame for a stream the peer ended.
    fn at_end(&mut self, discarded: usize) -> Frame<'_> {
        self.consumed = self.pending.len();
        if discarded > 0 {
            return Frame::TooLong {
                length: discarded.saturating_add(self.pending.len()),
            };
        }
        if self.pending.is_empty() {
            return Frame::End;
        }
        Frame::Unterminated(&self.pending)
    }
}

/// A Request's `id`: present on a call, absent on a Notification.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RequestId {
    /// `"id": null`.
    Null,
    /// A numeric id.
    Number(serde_json::Number),
    /// A string id.
    String(String),
}

impl RequestId {
    /// The id as a JSON value, for the Response.
    #[must_use]
    pub fn to_value(&self) -> Value {
        match self {
            Self::Null => Value::Null,
            Self::Number(number) => Value::Number(number.clone()),
            Self::String(text) => Value::String(text.clone()),
        }
    }
}

/// A valid JSON-RPC 2.0 Request object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Request {
    /// The id; `None` for a Notification.
    pub id: Option<RequestId>,
    /// The method name.
    pub method: String,
    /// The parameters; `None` when the member is absent.
    pub params: Option<Value>,
}

/// Why a line is not a valid Request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum EnvelopeError {
    /// The line is not UTF-8 JSON.
    #[error("the line is not valid JSON")]
    Parse,
    /// The line is a JSON array: a batch, which D77 does not carry.
    #[error("a batch is not admitted: D77 carries one message per line")]
    Batch,
    /// The line is JSON but not an object.
    #[error("the message is not a JSON object")]
    NotAnObject,
    /// `jsonrpc` is missing or not exactly "2.0".
    #[error("jsonrpc is not exactly \"2.0\"")]
    Version,
    /// `method` is missing or not a string.
    #[error("method is missing or not a string")]
    Method,
    /// `params` is present and neither an object nor an array.
    #[error("params is neither an object nor an array")]
    Params,
    /// `id` is present and neither a string, a number nor null.
    #[error("id is neither a string, a number nor null")]
    Id,
}

impl EnvelopeError {
    /// The JSON-RPC error code the Response carries.
    #[must_use]
    pub const fn code(self) -> i64 {
        match self {
            Self::Parse => PARSE_ERROR,
            _ => INVALID_REQUEST,
        }
    }
}

/// Reads the `id` member, if it is a valid one.
fn read_id(object: &Map<String, Value>) -> Result<Option<RequestId>, EnvelopeError> {
    match object.get("id") {
        None => Ok(None),
        Some(Value::Null) => Ok(Some(RequestId::Null)),
        Some(Value::Number(number)) => Ok(Some(RequestId::Number(number.clone()))),
        Some(Value::String(text)) => Ok(Some(RequestId::String(text.clone()))),
        Some(_) => Err(EnvelopeError::Id),
    }
}

/// Checks the members other than `id`.
fn read_members(object: &Map<String, Value>) -> Result<(String, Option<Value>), EnvelopeError> {
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Err(EnvelopeError::Version);
    }
    let method = object
        .get("method")
        .and_then(Value::as_str)
        .ok_or(EnvelopeError::Method)?;
    let params = object.get("params").cloned();
    if params
        .as_ref()
        .is_some_and(|value| !value.is_object() && !value.is_array())
    {
        return Err(EnvelopeError::Params);
    }
    Ok((method.to_owned(), params))
}

/// Checks one line against the Request object.
///
/// # Errors
///
/// Returns the [`EnvelopeError`] together with the id the error Response
/// must carry: the Request's own when it could be read, else `null`.
pub fn parse_line(line: &[u8]) -> Result<Request, (EnvelopeError, RequestId)> {
    let value: Value =
        serde_json::from_slice(line).map_err(|_| (EnvelopeError::Parse, RequestId::Null))?;
    let object = match value {
        Value::Object(object) => object,
        Value::Array(_) => return Err((EnvelopeError::Batch, RequestId::Null)),
        _ => return Err((EnvelopeError::NotAnObject, RequestId::Null)),
    };
    let id = read_id(&object).map_err(|error| (error, RequestId::Null))?;
    let (method, params) =
        read_members(&object).map_err(|error| (error, id.clone().unwrap_or(RequestId::Null)))?;
    Ok(Request { id, method, params })
}

/// Encodes an error Response as one line, newline included.
#[must_use]
pub fn error_line(id: &RequestId, code: i64, message: &str) -> Vec<u8> {
    let response = json!({
        "jsonrpc": "2.0",
        "error": { "code": code, "message": message },
        "id": id.to_value(),
    });
    let mut line = serde_json::to_vec(&response).unwrap_or_default();
    line.push(b'\n');
    line
}

/// Encodes a success Response as one line, newline included.
#[must_use]
pub fn result_line(id: &RequestId, result: &Value) -> Vec<u8> {
    let response = json!({ "jsonrpc": "2.0", "result": result, "id": id.to_value() });
    let mut line = serde_json::to_vec(&response).unwrap_or_default();
    line.push(b'\n');
    line
}
