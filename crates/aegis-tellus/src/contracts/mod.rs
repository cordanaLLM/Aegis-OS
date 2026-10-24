// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The versioned payloads P13 exchanges (milestones M05 and M16).
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P16 Athena to P13 Tellus, `EVALUATE_CANDIDATE_CARBON_SCI` | [`sci_query::CandidateSciQuery`] | [`sci_query`] |
//! | P13 Tellus to P16 Athena, the answer to the same edge | [`sci_query::CandidateSciResponse`] | [`sci_query`] |
//! | P13 Tellus to P07 Lictor, `SPATIOTEMPORAL_TASK_SHIFT` | [`task_shift::TaskShiftDirective`] | [`task_shift`] |
//! | P13 Tellus to P05 Forum, `EMIT_CARBON_TELEMETRY` (M16) | [`telemetry::CarbonTelemetry`] | [`telemetry`] |
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
//! inline slot: every schema type is `Copy`, so a decoded value owns no heap.
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
pub mod telemetry;

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
    /// The carbon telemetry update P13 Tellus sends to P05 Forum (M16).
    CarbonTelemetry,
}

impl SchemaId {
    /// Every schema this module defines.
    pub const ALL: [Self; 4] = [
        Self::CandidateSciQuery,
        Self::CandidateSciResponse,
        Self::TaskShiftDirective,
        Self::CarbonTelemetry,
    ];

    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::CandidateSciQuery => "aegis.p16-p13.sci-query.v1",
            Self::CandidateSciResponse => "aegis.p13-p16.sci-response.v1",
            Self::TaskShiftDirective => "aegis.p13-p07.task-shift.v1",
            Self::CarbonTelemetry => "aegis.p13-p05.carbon-telemetry.v1",
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
            Self::CarbonTelemetry => graph::EdgeId::EmitCarbonTelemetry,
        }
    }
}

#[macro_use]
#[path = "../../../contract_scaffold.rs"]
mod contract_scaffold;

#[macro_use]
#[path = "../../../contract_codec_scaffold.rs"]
mod contract_codec_scaffold;

/// What a refusal is about: the contract, and the payload when it could be read.
pub type Correlation = contract_scaffold::Correlation<SchemaId, CorrelationId>;

/// A fixed buffer holding one encoded payload.
pub type PayloadBuffer = contract_scaffold::PayloadBuffer<MAX_CONTRACT_PAYLOAD_BYTES>;

pub(crate) use contract_scaffold::decode_validated;
use contract_scaffold::ensure_edge;

define_contract_error! {
    SchemaId, CorrelationId;
    /// Reasons a P13 payload is refused.
    ///
    /// Every variant carries its [`Correlation`], so no refusal is anonymous.
    pub enum ContractError {
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
}

scaffold_contract!(
    SchemaId,
    CorrelationId,
    graph::EdgeId,
    MAX_CONTRACT_PAYLOAD_BYTES,
    ContractError
);

/// Validates that an edge matches the expected edge for a P13 contract.
///
/// # Errors
///
/// Returns [`ContractError::WrongEdge`] if `found != expected`.
pub(crate) fn ensure_own_edge(
    found: graph::EdgeId,
    expected: graph::EdgeId,
    correlation: Correlation,
) -> Result<(), ContractError> {
    ensure_edge(found, expected, correlation, |c, found| {
        ContractError::WrongEdge {
            correlation: c,
            found,
        }
    })
}
