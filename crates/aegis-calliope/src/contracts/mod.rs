// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two versioned payloads P08 Calliope stands at either end of (M07).
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P07 Lictor to P08 Calliope, `ENFORCE_REALTIME_RTPRIO` | [`realtime_grant::RealtimeGrant`] | [`realtime_grant`] |
//! | P08 Calliope to P04 Mercurius, `SHARE_DMA_BUF_STREAM` | [`dma_buf_stream::DmaBufStreamDescriptor`] | [`dma_buf_stream`] |
//!
//! The inbound schema lives with its consumer, which is the shape milestone
//! M14 set and milestone M06 followed. The outbound one lives with its
//! producer for a structural reason rather than a stylistic one: P04's crate
//! consumes it, and P04's crate already depends on this one, so putting the
//! descriptor in `aegis-compositor` would close a cycle in the crate graph.
//! `aegis-compositor` therefore decodes it through this crate's own decoder,
//! and `crates/aegis-compositor/src/chain.rs` is where that happens.
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! `PipeWire` connection, no `SPA` buffer, no DMA-BUF file descriptor, no socket
//! and no `cgroup` write. A schema states the shape and the bounds of a
//! payload; carrying one between two processes is later work.
//!
//! # Versioning
//!
//! Each schema carries an explicit `schema` tag as its first field, typed as an
//! enum with one variant per admitted version, so a payload naming any other
//! version is refused by the decoder rather than parsed leniently. It also
//! carries its [`graph::EdgeId`], typed the same way, and a payload naming the
//! other edge is refused with [`ContractError::WrongEdge`] -- this crate
//! carries one inbound and one outbound edge, so the guard is not vacuous and
//! is tested from both sides.
//!
//! # Bounds and allocation
//!
//! A payload longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it is
//! parsed at all, and every field of a decoded payload lands in a fixed inline
//! slot: both schema types are `Copy`, so a decoded value owns no heap.
//!
//! Decoding is **not** unconditionally allocation-free, and this module does
//! not claim it is. No contract path allocates for an escape-free payload: the
//! encoder writes into a caller-supplied [`PayloadBuffer`] and the decoder
//! reads each field straight out of its input. A payload that spells the same
//! value with a JSON escape is different -- `serde_json` unescapes any such
//! string into a heap scratch buffer before the field is validated -- and a
//! refusal allocates for the decoder's own error value. Both are transient,
//! neither is retained, and both are bounded by
//! [`MAX_CONTRACT_PAYLOAD_BYTES`]. `tests/allocation_bounds.rs` is the
//! executable statement of this.

pub mod dma_buf_stream;
pub mod encoding;
pub mod graph;
pub mod realtime_grant;

use core::fmt;

use crate::id::CorrelationId;

/// Scalar upper bound, in bytes, on one encoded contract payload.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 2048;

/// The contracts this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The real-time grant P07 Lictor sends to P08 Calliope.
    RealtimeGrant,
    /// The DMA-BUF stream descriptor P08 Calliope sends to P04 Mercurius.
    DmaBufStream,
}

impl SchemaId {
    /// Both contracts, in the order this module declares them.
    pub const ALL: [Self; 2] = [Self::RealtimeGrant, Self::DmaBufStream];

    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::RealtimeGrant => "aegis.p07-p08.realtime-grant.v1",
            Self::DmaBufStream => "aegis.p08-p04.dma-buf-stream.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::RealtimeGrant => graph::EdgeId::EnforceRealtimeRtprio,
            Self::DmaBufStream => graph::EdgeId::ShareDmaBufStream,
        }
    }
}

impl fmt::Display for SchemaId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.tag())
    }
}

/// What a refusal is about: the contract, and the payload when it could be read.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Correlation {
    schema: SchemaId,
    id: Option<CorrelationId>,
}

impl Correlation {
    /// Builds a correlation from a schema and the identifier, if one was read.
    #[must_use]
    pub const fn new(schema: SchemaId, id: Option<CorrelationId>) -> Self {
        Self { schema, id }
    }

    /// Returns the contract the payload claimed to satisfy.
    #[must_use]
    pub const fn schema(&self) -> SchemaId {
        self.schema
    }

    /// Returns the correlation identifier, when the decoder could read one.
    #[must_use]
    pub const fn id(&self) -> Option<CorrelationId> {
        self.id
    }
}

impl fmt::Display for Correlation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.id {
            Some(id) => write!(f, "{} [{}]", self.schema, id),
            None => write!(f, "{} [uncorrelated]", self.schema),
        }
    }
}

/// Reasons a contract payload is refused.
///
/// Every variant carries its [`Correlation`], so no refusal is anonymous.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ContractError {
    /// The payload names a contract version this build does not admit.
    #[error("{correlation}: the payload names a contract version this build does not admit")]
    UnknownVersion {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// The payload is not a well-formed instance of its contract.
    #[error("{correlation}: the payload is not a well-formed instance of this contract")]
    Malformed {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// The payload does not fit the contract's scalar byte bound.
    #[error("{correlation}: the payload does not fit the {max}-byte contract bound")]
    PayloadTooLong {
        /// What the refusal is about.
        correlation: Correlation,
        /// The scalar bound in bytes.
        max: usize,
    },
    /// The payload travels on an edge this schema does not carry.
    #[error("{correlation}: this schema does not travel on the edge the payload names")]
    WrongEdge {
        /// What the refusal is about.
        correlation: Correlation,
    },
}

impl ContractError {
    /// Returns what the refusal is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        match self {
            Self::UnknownVersion { correlation }
            | Self::Malformed { correlation }
            | Self::PayloadTooLong { correlation, .. }
            | Self::WrongEdge { correlation } => *correlation,
        }
    }
}

/// A fixed buffer holding one encoded contract payload.
///
/// Encoding writes into a caller-supplied buffer rather than returning an owned
/// string, so a payload that encodes successfully costs no heap and the
/// [`MAX_CONTRACT_PAYLOAD_BYTES`] bound is enforced by the buffer itself rather
/// than by a length check after the fact.
#[derive(Debug, Clone, Copy)]
pub struct PayloadBuffer {
    bytes: [u8; MAX_CONTRACT_PAYLOAD_BYTES],
    len: usize,
}

impl Default for PayloadBuffer {
    fn default() -> Self {
        Self::new()
    }
}

impl PayloadBuffer {
    /// Builds an empty buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            bytes: [0u8; MAX_CONTRACT_PAYLOAD_BYTES],
            len: 0,
        }
    }

    /// Returns the bytes written so far.
    #[must_use]
    pub fn as_bytes(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    /// Returns the encoded payload as text, when it is valid UTF-8.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        core::str::from_utf8(self.as_bytes()).ok()
    }

    /// Returns the number of bytes written.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when nothing has been encoded into the buffer.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// The lenient view of a payload's `schema` field, and of nothing else.
#[derive(serde::Deserialize)]
struct PeekSchema {
    #[serde(default)]
    schema: Option<CorrelationId>,
}

/// The lenient view of a payload's correlation identifier, and of nothing else.
///
/// Deliberately a second type rather than a second field on [`PeekSchema`]: a
/// `schema` value outside the identifier charset would otherwise discard the
/// correlation identifier with it, and one byte in the version tag would make
/// every refusal anonymous.
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PeekCorrelation {
    #[serde(default)]
    correlation_id: Option<CorrelationId>,
}

/// Reads the correlation identifier a payload carried, if it carried one.
fn peek_correlation(text: &str) -> Option<CorrelationId> {
    serde_json::from_str::<PeekCorrelation>(text)
        .ok()
        .and_then(|row| row.correlation_id)
}

/// Reads the contract version a payload claimed, if it claimed a readable one.
fn peek_schema(text: &str) -> Option<CorrelationId> {
    serde_json::from_str::<PeekSchema>(text)
        .ok()
        .and_then(|row| row.schema)
}

/// Classifies a decode failure into a correlated refusal.
fn classify(schema: SchemaId, text: &str) -> ContractError {
    let correlation = Correlation::new(schema, peek_correlation(text));
    match peek_schema(text) {
        Some(tag) if tag.as_bytes() != schema.tag().as_bytes() => {
            ContractError::UnknownVersion { correlation }
        }
        _ => ContractError::Malformed { correlation },
    }
}

/// Decodes one payload of `schema`, refusing anything over the byte bound.
pub(crate) fn decode_text<T>(schema: SchemaId, text: &str) -> Result<T, ContractError>
where
    T: for<'de> serde::Deserialize<'de>,
{
    if text.len() > MAX_CONTRACT_PAYLOAD_BYTES {
        return Err(ContractError::PayloadTooLong {
            correlation: Correlation::new(schema, None),
            max: MAX_CONTRACT_PAYLOAD_BYTES,
        });
    }
    match serde_json::from_str::<T>(text) {
        Ok(value) => Ok(value),
        Err(_) => Err(classify(schema, text)),
    }
}

/// Encodes `value` into `buffer` and returns the rendered payload.
pub(crate) fn encode_into<'b, T>(
    value: &T,
    correlation: Correlation,
    buffer: &'b mut PayloadBuffer,
) -> Result<&'b str, ContractError>
where
    T: serde::Serialize,
{
    buffer.len = 0;
    let mut tail: &mut [u8] = &mut buffer.bytes;
    let written = serde_json::to_writer(&mut tail, value);
    let remaining = tail.len();
    if written.is_err() {
        return Err(ContractError::PayloadTooLong {
            correlation,
            max: MAX_CONTRACT_PAYLOAD_BYTES,
        });
    }
    buffer.len = MAX_CONTRACT_PAYLOAD_BYTES.saturating_sub(remaining);
    match buffer.as_str() {
        Some(text) => Ok(text),
        None => Err(ContractError::Malformed { correlation }),
    }
}
