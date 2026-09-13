// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The one versioned payload P09 Minerva produces for a consumer that has no
//! crate yet (M06).
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P09 Minerva to P14 Hephaestus, `VERIFY_CODE_CAD` | [`cad_verification::CadVerificationRequest`] | [`cad_verification`] |
//!
//! P09's other two edges are typed in their consumers' crates and consumed
//! here rather than redeclared:
//! [`ActionProposal`](aegis_justitia::ActionProposal) is P06's, typed at M14,
//! and [`CapsuleRequest`](aegis_vesta::CapsuleRequest) is P10's, typed by this
//! milestone in `aegis-vesta`. [`chain`](crate::chain) is where both are
//! built. A fourth copy of the same encoding scaffolding is the cost of this
//! module existing at all, and it exists because no P14 crate does.
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! socket, no bus connection, no solver and no proof. A schema states the shape
//! and the bounds of a payload; carrying one between two processes is later
//! work, and the P14 slice is milestone M08.
//!
//! # Versioning and bounds
//!
//! The schema carries an explicit `schema` tag as its first field, typed as an
//! enum with one variant per admitted version, so a payload naming any other
//! version is refused by the decoder rather than parsed leniently. A payload
//! longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it is parsed at
//! all, and every field of a decoded payload lands in a fixed inline slot: the
//! schema type is `Copy`, so a decoded value owns no heap.
//!
//! Decoding is **not** unconditionally allocation-free, and this module does
//! not claim it is: `serde_json` unescapes a JSON string into a heap scratch
//! buffer before the field is validated, and a refusal allocates for the
//! decoder's own error value. Both are transient, neither is retained, and both
//! are bounded by [`MAX_CONTRACT_PAYLOAD_BYTES`].
//! `tests/allocation_bounds.rs` is the executable statement of this.

pub mod cad_verification;
pub mod graph;

use core::fmt;

use aegis_justitia::Identity;

/// Scalar upper bound, in bytes, on one encoded contract payload.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 1024;

/// The consumer contract this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The code-CAD verification request P09 submits to P14.
    CadVerification,
}

impl SchemaId {
    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::CadVerification => "aegis.p09-p14.cad-verification.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::CadVerification => graph::EdgeId::VerifyCodeCad,
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
    id: Option<Identity>,
}

impl Correlation {
    /// Builds a correlation from a schema and the identifier, if one was read.
    #[must_use]
    pub const fn new(schema: SchemaId, id: Option<Identity>) -> Self {
        Self { schema, id }
    }

    /// Returns the contract the payload claimed to satisfy.
    #[must_use]
    pub const fn schema(&self) -> SchemaId {
        self.schema
    }

    /// Returns the correlation identifier, when the decoder could read one.
    #[must_use]
    pub const fn id(&self) -> Option<Identity> {
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
    /// The payload names the reading of the edge this schema does not carry.
    #[error("{correlation}: this schema carries the graph-of-record direction only")]
    WrongDirection {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// A request the local screen already rejected was submitted anyway.
    #[error("{correlation}: a script the screen rejected must not be submitted")]
    RejectedScriptSubmitted {
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
            | Self::WrongDirection { correlation }
            | Self::RejectedScriptSubmitted { correlation } => *correlation,
        }
    }
}

/// A fixed buffer holding one encoded contract payload.
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
    schema: Option<Identity>,
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
    correlation_id: Option<Identity>,
}

/// Reads the correlation identifier a payload carried, if it carried one.
fn peek_correlation(text: &str) -> Option<Identity> {
    serde_json::from_str::<PeekCorrelation>(text)
        .ok()
        .and_then(|row| row.correlation_id)
}

/// Reads the contract version a payload claimed, if it claimed a readable one.
fn peek_schema(text: &str) -> Option<Identity> {
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
