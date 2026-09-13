// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The versioned payloads P13 exchanges (milestone M05).
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P16 Athena to P13 Tellus, `EVALUATE_CANDIDATE_CARBON_SCI` | [`sci_query::CandidateSciQuery`] | [`sci_query`] |
//! | P13 Tellus to P16 Athena, the answer to the same edge | [`sci_query::CandidateSciResponse`] | [`sci_query`] |
//! | P13 Tellus to P07 Lictor, `SPATIOTEMPORAL_TASK_SHIFT` | [`task_shift::TaskShiftDirective`] | [`task_shift`] |
//!
//! # Why the P16-to-P13 pair lives with P13
//!
//! The producer of a payload normally owns its schema, which would put the
//! query in `aegis-athena`. It is here instead, with both halves together, for
//! two reasons that are worth stating rather than leaving to be rediscovered:
//! the response carries an SCI rate, which only this crate can compute or
//! bound, and splitting a request/response pair across two crates would make
//! `aegis-athena` depend on `aegis-tellus` for the answer while
//! `aegis-tellus` depended on `aegis-athena` for the question. The dependency
//! runs one way: `aegis-athena` depends on this crate and constructs the query.
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! D-Bus connection, no socket, no eBPF program and no sysfs read anywhere in
//! this module or in the crate. A schema states the shape and the bounds of a
//! payload; carrying one between two processes is later work.
//!
//! # Versioning
//!
//! Every schema carries an explicit `schema` tag as its first field, typed as
//! an enum with one variant per admitted version. A payload naming any other
//! version is refused by the decoder rather than parsed leniently, and the
//! refusal is [`ContractError::UnknownVersion`]. Adding a version is adding a
//! variant, which makes every match on it a compile error until it is handled.
//!
//! # Bounds and allocation
//!
//! A payload longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it
//! is parsed at all, and every field of a decoded payload lands in a fixed
//! inline slot: all three schema types are `Copy`, so a decoded value owns no
//! heap.
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

pub mod encoding;
pub mod graph;
pub mod sci_query;
pub mod task_shift;

use core::fmt;

use crate::id::CorrelationId;

/// Scalar upper bound, in bytes, on one encoded P13 payload.
///
/// The largest schema is the query, whose worst case is
/// [`sci_query::MAX_QUERY_CANDIDATES`] identifiers of
/// [`crate::id::MAX_IDENTIFIER_LEN`] bytes each plus their JSON framing. The
/// bound leaves roughly three times that headroom and is the same for every
/// schema, so one buffer serves all three.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 4096;

/// The consumer contracts this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The candidate SCI query P16 Athena sends to P13 Tellus.
    CandidateSciQuery,
    /// The answer P13 Tellus sends back.
    CandidateSciResponse,
    /// The task-shift directive P13 Tellus sends to P07 Lictor.
    TaskShiftDirective,
}

impl SchemaId {
    /// Every schema this module defines.
    pub const ALL: [Self; 3] = [
        Self::CandidateSciQuery,
        Self::CandidateSciResponse,
        Self::TaskShiftDirective,
    ];

    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::CandidateSciQuery => "aegis.p16-p13.sci-query.v1",
            Self::CandidateSciResponse => "aegis.p13-p16.sci-response.v1",
            Self::TaskShiftDirective => "aegis.p13-p07.task-shift.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::CandidateSciQuery | Self::CandidateSciResponse => {
                graph::EdgeId::EvaluateCandidateCarbonSci
            }
            Self::TaskShiftDirective => graph::EdgeId::SpatiotemporalTaskShift,
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

/// Reasons a P13 payload is refused.
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
    #[error("{correlation}: the payload names the edge {found}, which this schema does not carry")]
    WrongEdge {
        /// What the refusal is about.
        correlation: Correlation,
        /// The edge the payload named.
        found: &'static str,
    },
    /// A query carries no candidate at all.
    ///
    /// An empty list is refused rather than answered, because an empty answer
    /// and "every candidate passed" are the same bytes to a consumer that does
    /// not check, and P16 promotes on that answer.
    #[error("{correlation}: the query carries no candidate; an empty list is not a question")]
    EmptyCandidateList {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// A response does not answer the query it claims to answer.
    #[error("{correlation}: the response carries {answered} rates for {asked} candidates")]
    UnansweredQuery {
        /// What the refusal is about.
        correlation: Correlation,
        /// How many candidates were asked about.
        asked: usize,
        /// How many rates came back.
        answered: usize,
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
            | Self::WrongEdge { correlation, .. }
            | Self::EmptyCandidateList { correlation }
            | Self::UnansweredQuery { correlation, .. } => *correlation,
        }
    }
}

/// A fixed buffer holding one encoded payload.
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
