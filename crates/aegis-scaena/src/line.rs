// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Line-delimited JSON-RPC 2.0 for the decoded-frame descriptor (D77).
//!
//! One message is one line: UTF-8 JSON ending in exactly one newline, at most
//! [`MAX_LINE_BYTES`] bytes with that newline. The request is a JSON-RPC 2.0
//! Request object (<https://www.jsonrpc.org/specification>): `jsonrpc`
//! exactly `"2.0"`, `method` [`METHOD`], `params` the descriptor and a numeric
//! `id`. The receiver answers with a Response carrying the same `id`.
//!
//! The decoder refuses, each with its own [`LineError`]: a line over the byte
//! bound, a message that is not exactly one line, JSON of another shape
//! (unknown, duplicate or mistyped members included), a `jsonrpc` member that
//! is absent or not `"2.0"`, another method, a missing or non-numeric `id`, a
//! missing `params`, and a line that names a file-descriptor number (`fd` or
//! `fds` at any level): the descriptor travels as `SCM_RIGHTS` beside the
//! line, never in it. The envelope, the descriptor and each plane are read
//! from JSON objects only; the array form serde would also accept is refused.
//!
//! Encoding and decoding allocate nothing: the encoder writes into a
//! caller-owned [`LineBuffer`], and the decoder reads strings into fixed
//! inline slots. What this does not claim: `serde_json` unescapes an escaped
//! JSON string into its own scratch buffer before any field here sees it, a
//! transient copy bounded by [`MAX_LINE_BYTES`], and a refusal it reports
//! boxes its message; neither is on the path of an admitted frame.

use core::fmt;
use core::marker::PhantomData;

use serde::de::value::MapAccessDeserializer;
use serde::de::{self, Deserializer, IgnoredAny, MapAccess, SeqAccess, Visitor};
use serde::{Deserialize, Serialize};

use crate::descriptor::{CorrelationId, DecodedFrame, FourCc, MAX_PLANES, Plane, Planes, Schema};
use crate::error::{DescriptorError, LineError};

/// The longest message admitted, in bytes, its newline included.
pub const MAX_LINE_BYTES: usize = 1_024;

/// The protocol version every line names.
pub const JSONRPC_VERSION: &str = "2.0";

/// The one method this socket serves: the graph relation P08 to P17.
pub const METHOD: &str = "STREAM_DECODED_FRAME";

/// The result a Response carries for an attached frame.
pub const ATTACHED: &str = "attached";

/// A buffer one line is written into or received into.
#[derive(Clone)]
pub struct LineBuffer {
    bytes: [u8; MAX_LINE_BYTES],
    len: usize,
}

impl LineBuffer {
    /// An empty buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: [0; MAX_LINE_BYTES],
            len: 0,
        }
    }

    /// The line currently held.
    #[must_use]
    pub fn line(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or_default()
    }

    /// The whole storage, for a receive to fill.
    pub(crate) fn storage(&mut self) -> &mut [u8; MAX_LINE_BYTES] {
        &mut self.bytes
    }

    /// Records that a receive filled `len` bytes.
    pub(crate) fn set_len(&mut self, len: usize) {
        self.len = len.min(MAX_LINE_BYTES);
    }
}

impl Default for LineBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for LineBuffer {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LineBuffer")
            .field("len", &self.len)
            .finish_non_exhaustive()
    }
}

/// A decoded-frame request.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Request {
    /// The request identifier the Response repeats.
    pub id: u64,
    /// The descriptor.
    pub frame: DecodedFrame,
}

/// The JSON-RPC 2.0 error codes a refusal is answered with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum Refusal {
    /// -32700: the line is not JSON.
    Parse,
    /// -32600: the line is not an admitted Request.
    InvalidRequest,
    /// -32601: another method.
    MethodNotFound,
    /// -32602: the descriptor was refused.
    InvalidParams,
    /// -32000: the line is over the byte bound.
    LineTooLong,
    /// -32001: the frame was refused at attach.
    AttachRefused,
}

impl Refusal {
    /// The code on the wire.
    #[must_use]
    pub const fn code(self) -> i64 {
        match self {
            Self::Parse => -32_700,
            Self::InvalidRequest => -32_600,
            Self::MethodNotFound => -32_601,
            Self::InvalidParams => -32_602,
            Self::LineTooLong => -32_000,
            Self::AttachRefused => -32_001,
        }
    }

    /// The message on the wire.
    #[must_use]
    pub const fn message(self) -> &'static str {
        match self {
            Self::Parse => "parse error",
            Self::InvalidRequest => "invalid request",
            Self::MethodNotFound => "method not found",
            Self::InvalidParams => "invalid params",
            Self::LineTooLong => "line too long",
            Self::AttachRefused => "attach refused",
        }
    }

    /// The refusal a line error is answered with.
    #[must_use]
    pub const fn for_line(error: LineError) -> Self {
        match error {
            LineError::TooLong { .. } => Self::LineTooLong,
            LineError::Json {
                category: crate::error::JsonCategory::Syntax | crate::error::JsonCategory::Eof,
                ..
            } => Self::Parse,
            LineError::Method => Self::MethodNotFound,
            LineError::Descriptor(_) | LineError::Params => Self::InvalidParams,
            _ => Self::InvalidRequest,
        }
    }

    fn from_code(code: i64) -> Option<Self> {
        [
            Self::Parse,
            Self::InvalidRequest,
            Self::MethodNotFound,
            Self::InvalidParams,
            Self::LineTooLong,
            Self::AttachRefused,
        ]
        .into_iter()
        .find(|refusal| refusal.code() == code)
    }
}

/// A Response: the `id` it answers, or `None` when the request's could not
/// be read, and the outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Response {
    /// The request's `id`.
    pub id: Option<u64>,
    /// `Ok` for an attached frame, the refusal otherwise.
    pub outcome: Result<(), Refusal>,
}

// --- Encoding ---------------------------------------------------------------

#[derive(Serialize)]
struct RequestOut<'a> {
    jsonrpc: &'static str,
    method: &'static str,
    params: &'a DecodedFrame,
    id: u64,
}

#[derive(Serialize)]
struct ErrorOut {
    code: i64,
    message: &'static str,
}

#[derive(Serialize)]
struct ResponseOut {
    jsonrpc: &'static str,
    id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ErrorOut>,
}

/// Writes `value` and a newline into `buffer`, bounded by its capacity.
fn write_line<'b, T: Serialize>(
    value: &T,
    buffer: &'b mut LineBuffer,
) -> Result<&'b [u8], LineError> {
    buffer.len = 0;
    let capacity = MAX_LINE_BYTES.saturating_sub(1);
    let body = buffer.bytes.get_mut(..capacity).ok_or(LineError::Framing)?;
    let mut cursor = std::io::Cursor::new(body);
    serde_json::to_writer(&mut cursor, value).map_err(|_| LineError::TooLong {
        length: MAX_LINE_BYTES.saturating_add(1),
    })?;
    let end = usize::try_from(cursor.position()).map_err(|_| LineError::Framing)?;
    let slot = buffer.bytes.get_mut(end).ok_or(LineError::Framing)?;
    *slot = b'\n';
    buffer.len = end.saturating_add(1);
    Ok(buffer.line())
}

/// Encodes `request` as one line into `buffer`.
///
/// # Errors
///
/// Returns [`LineError::TooLong`] when the line would exceed
/// [`MAX_LINE_BYTES`].
pub fn encode_request<'b>(
    request: &Request,
    buffer: &'b mut LineBuffer,
) -> Result<&'b [u8], LineError> {
    let out = RequestOut {
        jsonrpc: JSONRPC_VERSION,
        method: METHOD,
        params: &request.frame,
        id: request.id,
    };
    write_line(&out, buffer)
}

/// Encodes `response` as one line into `buffer`.
///
/// # Errors
///
/// Returns [`LineError::TooLong`] when the line would exceed
/// [`MAX_LINE_BYTES`], which a Response cannot reach.
pub fn encode_response<'b>(
    response: &Response,
    buffer: &'b mut LineBuffer,
) -> Result<&'b [u8], LineError> {
    let out = ResponseOut {
        jsonrpc: JSONRPC_VERSION,
        id: response.id,
        result: response.outcome.ok().map(|()| ATTACHED),
        error: response.outcome.err().map(|refusal| ErrorOut {
            code: refusal.code(),
            message: refusal.message(),
        }),
    };
    write_line(&out, buffer)
}

// --- Decoding ---------------------------------------------------------------

/// A string read into a fixed slot; one longer than `N` is marked, not cut.
#[derive(Clone, Copy)]
struct Token<const N: usize> {
    bytes: [u8; N],
    len: usize,
    overflow: bool,
}

impl<const N: usize> Token<N> {
    fn as_bytes(&self) -> &[u8] {
        if self.overflow {
            return &[];
        }
        self.bytes.get(..self.len).unwrap_or_default()
    }

    fn is(&self, text: &str) -> bool {
        !self.overflow && self.as_bytes() == text.as_bytes()
    }
}

struct TokenVisitor<const N: usize>;

impl<const N: usize> Visitor<'_> for TokenVisitor<N> {
    type Value = Token<N>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a string")
    }

    fn visit_str<E: de::Error>(self, value: &str) -> Result<Token<N>, E> {
        let mut token = Token {
            bytes: [0; N],
            len: 0,
            overflow: value.len() > N,
        };
        if let Some(slot) = token.bytes.get_mut(..value.len()) {
            slot.copy_from_slice(value.as_bytes());
            token.len = value.len();
        }
        Ok(token)
    }
}

impl<'de, const N: usize> Deserialize<'de> for Token<N> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_str(TokenVisitor::<N>)
    }
}

/// A JSON object, and only an object.
///
/// serde's derived decoder reads a struct from a JSON array of its values in
/// declaration order as readily as from an object, and `deny_unknown_fields`
/// has no key to refuse there (the defect `crates/aegis-fabrica-defs` found on
/// 2026-09-29). Every envelope member here is read through this wrapper, which
/// asks the deserializer for a map and refuses every other JSON value.
struct Object<T>(T);

struct ObjectVisitor<T>(PhantomData<T>);

impl<'de, T: Deserialize<'de>> Visitor<'de> for ObjectVisitor<T> {
    type Value = Object<T>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object; the array form is refused")
    }

    fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Object<T>, A::Error> {
        T::deserialize(MapAccessDeserializer::new(map)).map(Object)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for Object<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_map(ObjectVisitor(PhantomData))
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PlaneIn {
    offset: u32,
    pitch: u32,
    #[serde(default)]
    fd: Option<IgnoredAny>,
    #[serde(default)]
    fds: Option<IgnoredAny>,
}

/// Up to [`MAX_PLANES`] planes, and how many the line listed in all.
#[derive(Clone, Copy)]
struct PlanesIn {
    slots: [Plane; MAX_PLANES],
    count: usize,
    fd_named: bool,
}

struct PlanesVisitor;

impl<'de> Visitor<'de> for PlanesVisitor {
    type Value = PlanesIn;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a list of planes")
    }

    fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<PlanesIn, A::Error> {
        let mut planes = PlanesIn {
            slots: [Plane::default(); MAX_PLANES],
            count: 0,
            fd_named: false,
        };
        // A line of MAX_LINE_BYTES cannot list more elements than bytes.
        for _ in 0..=MAX_LINE_BYTES {
            let Some(Object(plane)) = seq.next_element::<Object<PlaneIn>>()? else {
                return Ok(planes);
            };
            planes.fd_named |= plane.fd.is_some() || plane.fds.is_some();
            if let Some(slot) = planes.slots.get_mut(planes.count) {
                *slot = Plane {
                    offset: plane.offset,
                    pitch: plane.pitch,
                };
            }
            planes.count = planes.count.saturating_add(1);
        }
        Err(de::Error::custom("the plane list exceeds the line bound"))
    }
}

impl<'de> Deserialize<'de> for PlanesIn {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        deserializer.deserialize_seq(PlanesVisitor)
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FrameIn {
    schema: Token<48>,
    correlation_id: Token<{ crate::descriptor::MAX_CORRELATION_BYTES }>,
    fourcc: Token<8>,
    modifier: u64,
    width: u32,
    height: u32,
    planes: PlanesIn,
    #[serde(default)]
    fd: Option<IgnoredAny>,
    #[serde(default)]
    fds: Option<IgnoredAny>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RequestIn {
    #[serde(default)]
    jsonrpc: Option<Token<8>>,
    #[serde(default)]
    method: Option<Token<32>>,
    #[serde(default)]
    params: Option<Object<FrameIn>>,
    #[serde(default)]
    id: Option<u64>,
    #[serde(default)]
    fd: Option<IgnoredAny>,
    #[serde(default)]
    fds: Option<IgnoredAny>,
}

impl FrameIn {
    fn names_an_fd(&self) -> bool {
        self.fd.is_some() || self.fds.is_some() || self.planes.fd_named
    }

    fn into_frame(self) -> Result<DecodedFrame, DescriptorError> {
        let schema = Schema::parse(self.schema.as_bytes())?;
        let correlation_id = CorrelationId::parse(self.correlation_id.as_bytes())?;
        let fourcc = FourCc::parse(self.fourcc.as_bytes())?;
        let listed =
            self.planes
                .slots
                .get(..self.planes.count)
                .ok_or(DescriptorError::PlaneCount {
                    count: self.planes.count,
                })?;
        let planes = Planes::new(listed)?;
        let frame = DecodedFrame::new(
            correlation_id,
            fourcc,
            self.modifier,
            (self.width, self.height),
            planes,
        )?;
        Ok(DecodedFrame { schema, ..frame })
    }
}

/// The body of a message: its bytes before exactly one trailing newline.
///
/// # Errors
///
/// Returns [`LineError::TooLong`] over [`MAX_LINE_BYTES`] and
/// [`LineError::Framing`] for a message that does not end in one newline or
/// holds another line break.
pub fn body(message: &[u8]) -> Result<&[u8], LineError> {
    if message.len() > MAX_LINE_BYTES {
        return Err(LineError::TooLong {
            length: message.len(),
        });
    }
    let text = message.strip_suffix(b"\n").ok_or(LineError::Framing)?;
    if text.is_empty() || text.iter().any(|byte| matches!(byte, b'\n' | b'\r')) {
        return Err(LineError::Framing);
    }
    Ok(text)
}

/// Decodes one request line.
///
/// # Errors
///
/// Returns the [`LineError`] naming why the line was refused; see the module
/// documentation for the order the checks run in.
pub fn decode_request(message: &[u8]) -> Result<Request, LineError> {
    let text = body(message)?;
    let Object(raw): Object<RequestIn> =
        serde_json::from_slice(text).map_err(|error| LineError::from(&error))?;
    if !raw.jsonrpc.is_some_and(|token| token.is(JSONRPC_VERSION)) {
        return Err(LineError::Version);
    }
    let fd_named = raw.fd.is_some()
        || raw.fds.is_some()
        || raw
            .params
            .as_ref()
            .is_some_and(|Object(frame)| frame.names_an_fd());
    if fd_named {
        return Err(LineError::FdNumber);
    }
    if !raw.method.is_some_and(|token| token.is(METHOD)) {
        return Err(LineError::Method);
    }
    let id = raw.id.ok_or(LineError::Id)?;
    let Object(params) = raw.params.ok_or(LineError::Params)?;
    Ok(Request {
        id,
        frame: params.into_frame()?,
    })
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ErrorIn {
    code: i64,
    #[serde(rename = "message")]
    _message: IgnoredAny,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ResponseIn {
    #[serde(default)]
    jsonrpc: Option<Token<8>>,
    id: Option<u64>,
    #[serde(default)]
    result: Option<Token<16>>,
    #[serde(default)]
    error: Option<Object<ErrorIn>>,
}

/// Decodes one Response line.
///
/// # Errors
///
/// Returns [`LineError::Version`] for another protocol version and
/// [`LineError::Response`] for a line with both, neither or an unknown
/// outcome.
pub fn decode_response(message: &[u8]) -> Result<Response, LineError> {
    let text = body(message)?;
    let Object(raw): Object<ResponseIn> =
        serde_json::from_slice(text).map_err(|error| LineError::from(&error))?;
    if !raw.jsonrpc.is_some_and(|token| token.is(JSONRPC_VERSION)) {
        return Err(LineError::Version);
    }
    let outcome = match (raw.result, raw.error) {
        (Some(result), None) if result.is(ATTACHED) => Ok(()),
        (None, Some(Object(error))) => {
            Err(Refusal::from_code(error.code).ok_or(LineError::Response)?)
        }
        _ => return Err(LineError::Response),
    };
    Ok(Response {
        id: raw.id,
        outcome,
    })
}
