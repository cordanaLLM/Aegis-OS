// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Versioned consumer contracts for the three P06 edges (milestone M14).
//!
//! P06 Justitia speaks to three neighbours. Each conversation now has a typed,
//! versioned schema with a stable field encoding:
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P09 Minerva to P06 Justitia, `ACTION_GATE_INTERCEPT` | [`proposal::ActionProposal`] | [`proposal`] |
//! | P06 Justitia to P05 Forum, `DISPATCH_DECISION_REQUEST` | [`decision_request::DecisionRequest`] | [`decision_request`] |
//! | P06 Justitia to P16 Athena, `AUDIT_RECONSTRUCTIVE_CANDIDATE` | [`audit::SignedAuditRecord`] | [`audit`] |
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! D-Bus connection, no socket, no eBPF program and no TPM2 signing anywhere in
//! this module or in the crate. A schema states the shape and the bounds of a
//! payload; carrying one between two processes is later work (M19 and M10 for
//! the eBPF half, M20 for TPM2 sealing, M16 for the Forum consumer). The
//! signature fields are field encodings, not a signing implementation: nothing
//! here produces or verifies a signature.
//!
//! # Versioning
//!
//! Every schema carries an explicit `schema` tag as its first field, typed as
//! an enum with one variant per admitted version. A payload naming any other
//! version is refused by the decoder rather than parsed leniently, and the
//! refusal is [`ContractError::UnknownVersion`]. Adding a version is adding a
//! variant, which makes every match on it a compile error until it is handled.
//!
//! # Correlated refusals
//!
//! Every refusal carries a [`Correlation`]: the schema the payload claimed and,
//! when the decoder could still read it, the correlation identifier the payload
//! carried. A rejected payload can therefore be tied to the conversation it
//! belonged to without echoing the payload itself into a log. The correlation
//! identifier is the intent identifier that threads all three schemas together
//! (export-031 `5ff683328148` names it `ActionIntent.id` on the proposal and
//! `AuditLogEntry.action_id` on the record it produces).
//!
//! The two fields are read independently of each other and of the rest of the
//! payload, so a `schema` value that is out of the identifier charset, longer
//! than [`crate::MAX_IDENTITY_LEN`] or not a string at all cannot discard a
//! correlation identifier that is right there and readable. The only refusal
//! that is genuinely anonymous is one whose input carries nothing to read.
//!
//! # Bounds
//!
//! A payload longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it is
//! parsed at all, and every field of a decoded payload lands in a fixed inline
//! buffer: all three schema types are `Copy`, so a decoded value owns no heap.
//!
//! Decoding is not unconditionally allocation-free, and this module does not
//! claim it is. No contract path allocates for an escape-free payload -- the
//! encoder writes into a caller-supplied [`PayloadBuffer`] and the decoder
//! borrows each field straight out of its input. A payload that spells the same
//! value with a JSON escape is different: `\/` is an admissible spelling of the
//! `/` in a target path, and `serde_json` unescapes any such string into a heap
//! scratch buffer before the field is validated. A refusal allocates too, for
//! the decoder's own error value. Both are transient, neither is retained, and
//! both are bounded by [`MAX_CONTRACT_PAYLOAD_BYTES`], because a payload longer
//! than that never reaches the parser. `tests/allocation_bounds.rs` is the
//! executable statement of all of this.

pub mod audit;
pub mod decision_request;
pub mod graph;
pub mod proposal;
pub mod unit;

use amussis_decision::identity::Identity;
use amussis_decision::risk::RequiredApproval;

pub use amussis_decision::encoding::{MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer};

/// The three consumer contracts this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SchemaId {
    /// The action proposal P09 Minerva sends to P06 Justitia.
    ActionProposal,
    /// The decision request P06 Justitia sends to P05 Forum.
    DecisionRequest,
    /// The signed audit record P06 Justitia hands to P16 Athena.
    SignedAuditRecord,
}

impl SchemaId {
    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::ActionProposal => "aegis.p09-p06.action-proposal.v1",
            Self::DecisionRequest => "aegis.p06-p05.decision-request.v1",
            Self::SignedAuditRecord => "aegis.p06-p16.audit-record.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::ActionProposal => graph::EdgeId::ActionGateIntercept,
            Self::DecisionRequest => graph::EdgeId::DispatchDecisionRequest,
            Self::SignedAuditRecord => graph::EdgeId::AuditReconstructiveCandidate,
        }
    }
}

impl core::fmt::Display for SchemaId {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
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

impl core::fmt::Display for Correlation {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match &self.id {
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
    /// An action proposal arrived without an oversight signature.
    #[error("{correlation}: the action proposal carries no oversight signature")]
    Unsigned {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// The approval window is outside the declared bounds.
    #[error("{correlation}: an approval window of {seconds}s is outside {min}s..={max}s")]
    WindowOutOfRange {
        /// What the refusal is about.
        correlation: Correlation,
        /// The window the payload declared, in seconds.
        seconds: u64,
        /// The shortest admissible window, in seconds.
        min: u32,
        /// The longest admissible window, in seconds.
        max: u32,
    },
    /// A genesis previous-link appeared on a record that is not the first.
    #[error(
        "{correlation}: a genesis previous link is admissible only at sequence 1, not {sequence}"
    )]
    GenesisLinkOutOfPlace {
        /// What the refusal is about.
        correlation: Correlation,
        /// The sequence number the payload declared.
        sequence: u64,
    },
    /// The first record in a chain did not carry the genesis previous-link.
    #[error("{correlation}: the record at sequence 1 must carry the genesis previous link")]
    MissingGenesisLink {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// A refusal was recorded without the reason that produced it.
    #[error("{correlation}: a refused action must record the reason it was refused")]
    RefusalWithoutReason {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// A decision that refused nothing still carried a refusal reason.
    #[error("{correlation}: a decision that refused nothing must record no reason")]
    ReasonWithoutRefusal {
        /// What the refusal is about.
        correlation: Correlation,
    },
    /// The declared approval is not the one the tier and class demand.
    #[error("{correlation}: declared approval {declared:?} is not the demanded {demanded:?}")]
    ApprovalMismatch {
        /// What the refusal is about.
        correlation: Correlation,
        /// The approval the payload declared.
        declared: RequiredApproval,
        /// The approval the tier and oversight class demand.
        demanded: RequiredApproval,
    },
    /// A decision request was raised where no human decision is demanded.
    #[error("{correlation}: this tier and oversight class demand no human decision")]
    NoApprovalRequired {
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
            | Self::Unsigned { correlation }
            | Self::WindowOutOfRange { correlation, .. }
            | Self::GenesisLinkOutOfPlace { correlation, .. }
            | Self::MissingGenesisLink { correlation }
            | Self::RefusalWithoutReason { correlation }
            | Self::ReasonWithoutRefusal { correlation }
            | Self::ApprovalMismatch { correlation, .. }
            | Self::NoApprovalRequired { correlation } => *correlation,
        }
    }
}

fn map_encoding_error(
    schema: SchemaId,
    err: amussis_decision::encoding::ContractError,
) -> ContractError {
    let correlation = Correlation::new(schema, err.correlation().id());
    match err {
        amussis_decision::encoding::ContractError::UnknownVersion { .. } => {
            ContractError::UnknownVersion { correlation }
        }
        amussis_decision::encoding::ContractError::PayloadTooLong { max, .. } => {
            ContractError::PayloadTooLong { correlation, max }
        }
        amussis_decision::encoding::ContractError::Malformed { .. } => {
            ContractError::Malformed { correlation }
        }
        _ => ContractError::Malformed { correlation },
    }
}

pub(crate) fn decode_text<T: for<'de> serde::Deserialize<'de>>(
    schema: SchemaId,
    text: &str,
) -> Result<T, ContractError> {
    amussis_decision::encoding::decode_text(
        amussis_decision::encoding::SchemaTag::new(schema.tag()),
        text,
    )
    .map_err(|err| map_encoding_error(schema, err))
}

pub(crate) fn encode_into<'b, T: serde::Serialize>(
    value: &T,
    correlation: Correlation,
    buffer: &'b mut PayloadBuffer,
) -> Result<&'b str, ContractError> {
    let upstream_correlation = amussis_decision::encoding::Correlation::new(
        amussis_decision::encoding::SchemaTag::new(correlation.schema().tag()),
        correlation.id(),
    );
    amussis_decision::encoding::encode_into(value, upstream_correlation, buffer)
        .map_err(|err| map_encoding_error(correlation.schema(), err))
}

pub(crate) fn decode_validated<T, E>(
    decoded: Result<T, E>,
    validate: impl FnOnce(&T) -> Result<(), E>,
) -> Result<T, E> {
    let value = decoded?;
    validate(&value)?;
    Ok(value)
}

/// Verifies that `edge` matches `expected` for a contract payload.
///
/// # Errors
///
/// Returns [`ContractError::Malformed`] when `edge` does not match `expected`.
pub(crate) fn check_edge(
    edge: graph::EdgeId,
    expected: graph::EdgeId,
    correlation: Correlation,
) -> Result<(), ContractError> {
    if edge != expected {
        return Err(ContractError::Malformed { correlation });
    }
    Ok(())
}
