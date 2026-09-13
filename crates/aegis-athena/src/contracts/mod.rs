// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The versioned payload P16 produces, and the M14 record it consumes.
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P16 Athena to P02 Janus, `TRIGGER_SYSUPDATE_ROLLBACK` | [`promotion::PromotionTrigger`] | [`promotion`] |
//! | P06 Justitia to P16 Athena, `AUDIT_RECONSTRUCTIVE_CANDIDATE` | `aegis_justitia::SignedAuditRecord` | [`audit_intake`] |
//!
//! # The audit record is consumed, not redefined
//!
//! Milestone M14 typed the signed audit record in `aegis-justitia`, with its
//! own versioning, its own genesis rule and its own refusals. [`audit_intake`]
//! decodes through that type and adds the one rule that belongs to this side
//! of the edge -- which recorded decisions admit a promotion -- so there is
//! one schema for the record rather than a producer's and a consumer's reading
//! of it that can drift apart.
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! D-Bus connection, no socket, no `systemd-sysupdate` invocation and no
//! partition write anywhere in this module or in the crate. See
//! [`crate::ports`] for the seam a real client would attach to.
//!
//! # Versioning, bounds and allocation
//!
//! The schema carries an explicit `schema` tag as its first field, typed as an
//! enum with one variant per admitted version; a payload naming another
//! version is [`ContractError::UnknownVersion`] rather than leniently parsed.
//! A payload longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it
//! is parsed at all, and every field of a decoded payload lands in a fixed
//! inline slot, so a decoded value owns no heap.
//!
//! Decoding is **not** unconditionally allocation-free, and this module does
//! not claim it is: `serde_json` unescapes an escaped JSON string into a heap
//! scratch buffer before any field is validated, and a refusal allocates for
//! the decoder's own error value. Both are transient, neither is retained, and
//! both are bounded by [`MAX_CONTRACT_PAYLOAD_BYTES`].
//! `tests/allocation_bounds.rs` is the executable statement of this.
//!
//! # Where the field encodings come from
//!
//! This crate declares no encoding module of its own. Both identifier fields
//! are `aegis_tellus::CorrelationId` and `aegis_tellus::CandidateId`, whose
//! validating `serde` implementations live with them, and every other field is
//! an enum or a small integer. One identifier type, one encoding, one set of
//! bounds -- shared with the crate on the other side of the
//! `EVALUATE_CANDIDATE_CARBON_SCI` edge rather than restated here.

pub mod audit_intake;
pub mod graph;
pub mod promotion;

use core::fmt;

use aegis_tellus::CorrelationId;

/// Scalar upper bound, in bytes, on one encoded P16 payload.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 4096;

/// The consumer contract this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The promotion trigger P16 Athena sends to P02 Janus.
    PromotionTrigger,
    /// The signed audit record P06 Justitia hands to P16 Athena.
    ///
    /// The schema is `aegis-justitia`'s; this variant exists so a refusal on
    /// the consuming side names the contract it was reading.
    SignedAuditRecord,
}

impl SchemaId {
    /// Every schema a P16 refusal can name.
    pub const ALL: [Self; 2] = [Self::PromotionTrigger, Self::SignedAuditRecord];

    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::PromotionTrigger => "aegis.p16-p02.promotion-trigger.v1",
            Self::SignedAuditRecord => "aegis.p06-p16.audit-record.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::PromotionTrigger => graph::EdgeId::TriggerSysupdateRollback,
            Self::SignedAuditRecord => graph::EdgeId::AuditReconstructiveCandidate,
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

/// Reasons a P16 payload is refused.
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
    /// The trigger's action does not follow from the stage it reports.
    ///
    /// A deploy from an invalidated candidate, or a rollback from a published
    /// one, is refused rather than sent: P02 would act on it either way.
    #[error("{correlation}: a {action} does not follow from the stage {stage}")]
    ActionDoesNotFollow {
        /// What the refusal is about.
        correlation: Correlation,
        /// The action the payload asked for.
        action: &'static str,
        /// The stage the payload reported.
        stage: &'static str,
    },
    /// The upstream M14 audit record did not decode or did not validate.
    ///
    /// The refusal is the producer's, carried across unchanged: this crate
    /// does not restate `aegis-justitia`'s rules, so it cannot disagree with
    /// them.
    #[error("{correlation}: the M14 audit record was refused by its own schema")]
    UpstreamAuditRecord {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// The audit record records a decision that does not admit a promotion.
    #[error("{correlation}: the audit record records {status}, which does not admit a promotion")]
    PromotionNotAdmitted {
        /// What the refusal is about.
        correlation: Correlation,
        /// The decision the record carries.
        status: &'static str,
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
            | Self::ActionDoesNotFollow { correlation, .. }
            | Self::UpstreamAuditRecord { correlation }
            | Self::PromotionNotAdmitted { correlation, .. } => *correlation,
        }
    }
}

/// A fixed buffer holding one encoded payload.
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
#[derive(serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
struct PeekCorrelation {
    #[serde(default)]
    correlation_id: Option<CorrelationId>,
}

/// Reads the correlation identifier a payload carried, if it carried one.
pub(crate) fn peek_correlation(text: &str) -> Option<CorrelationId> {
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
