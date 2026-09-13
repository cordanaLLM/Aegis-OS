// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two versioned descriptors P03 hands to its consumers (milestone M17).
//!
//! P03 Vulcan produces for two neighbours, and each edge now has a typed,
//! versioned schema with a stable field encoding:
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P03 Vulcan to P09 Minerva, `GPUDIRECT_WEIGHT_STREAMING` | [`weight_stream::WeightStreamDescriptor`] | [`weight_stream`] |
//! | P03 Vulcan to P15 Hestia, `ZERO_COPY_MEDIA_INGEST` | [`media_ingest::MediaIngestDescriptor`] | [`media_ingest`] |
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! `mmap`, no `VFIO` container, no `CUDA` call, no `PCIe` peer-to-peer transfer and
//! no `DMA-BUF` file descriptor anywhere in this module or in the crate. A
//! descriptor states the shape and the bounds of a payload; carrying one
//! between two processes is later work, and the recorded roadmap keeps the
//! transports stubbed until M12. Binding a descriptor to a real render node is
//! M25.
//!
//! # Versioning
//!
//! Every schema carries an explicit `schema` tag as its first field, typed as
//! an enum with one variant per admitted version, so a payload naming any
//! other version is refused by the decoder rather than parsed leniently. Each
//! schema also carries its [`graph::EdgeId`], typed the same way: the edge
//! enum admits only the edge that schema travels on, so a descriptor asserting
//! the other P03 edge does not decode.
//!
//! # Bounds and allocation
//!
//! A payload longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it
//! is parsed at all, and every field of a decoded descriptor lands in a fixed
//! inline slot: both schema types are `Copy`, so a decoded value owns no heap.
//!
//! Decoding is **not** unconditionally allocation-free, and this module does
//! not claim it is. No contract path allocates for an escape-free payload: the
//! encoder writes into a caller-supplied [`PayloadBuffer`] and the decoder
//! reads each field straight out of its input. A payload that spells the same
//! value with a JSON escape is different -- `serde_json` unescapes any such
//! string into a heap scratch buffer before the field is validated -- and a
//! refusal allocates for the decoder's own error value. Both are transient,
//! neither is retained, and both are bounded by
//! [`MAX_CONTRACT_PAYLOAD_BYTES`], because a longer payload never reaches the
//! parser. `tests/allocation_bounds.rs` is the executable statement of this.

pub mod dmabuf;
pub mod encoding;
pub mod graph;
pub mod media_ingest;
pub mod weight_stream;

use core::fmt;

use crate::id::CorrelationId;

/// Scalar upper bound, in bytes, on one encoded descriptor payload.
///
/// Both descriptors are small and fixed: a version tag, an edge tag, a
/// correlation identifier and a handful of integers. The bound leaves an order
/// of magnitude of headroom and is the same for both, so one buffer serves
/// each.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 2048;

/// The two consumer contracts this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The weight-streaming descriptor P03 hands to P09 Minerva.
    WeightStream,
    /// The media-ingest descriptor P03 hands to P15 Hestia.
    MediaIngest,
}

impl SchemaId {
    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::WeightStream => "aegis.p03-p09.weight-stream.v1",
            Self::MediaIngest => "aegis.p03-p15.media-ingest.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::WeightStream => graph::EdgeId::GpudirectWeightStreaming,
            Self::MediaIngest => graph::EdgeId::ZeroCopyMediaIngest,
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

/// Reasons a descriptor payload is refused.
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
    /// The payload names an edge this schema does not travel on.
    #[error("{correlation}: the payload names an edge this schema does not travel on")]
    WrongEdge {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// The declared export path cannot export a `DMA-BUF` as described.
    #[error("{correlation}: {reason}")]
    ExportPathUnusable {
        /// What the refusal is about.
        correlation: Correlation,
        /// Why the export path was refused.
        reason: &'static str,
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
            | Self::WrongEdge { correlation }
            | Self::ExportPathUnusable { correlation, .. } => *correlation,
        }
    }
}

/// A fixed buffer holding one encoded descriptor payload.
///
/// Encoding writes into a caller-supplied buffer rather than returning an
/// owned string, so a payload that encodes successfully costs no heap and the
/// [`MAX_CONTRACT_PAYLOAD_BYTES`] bound is enforced by the buffer itself
/// rather than by a length check after the fact.
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
