// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two versioned payloads P10 Vesta stands at either end of (M06).
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P09 Minerva to P10 Vesta, `EXECUTE_WASMED_CAPSULE` | [`capsule_request::CapsuleRequest`] | [`capsule_request`] |
//! | P10 Vesta to P16 Athena, `SANDBOX_CANDIDATE_EVALUATION` | [`candidate_evaluation::CandidateEvaluation`] | [`candidate_evaluation`] |
//!
//! The inbound schema lives with its consumer, which is the shape milestone
//! M14 set when it typed the action proposal P09 produces inside P06's crate.
//! The outbound one lives with its producer because the P16 crate does not
//! exist.
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! `AF_VSOCK` socket, no monitor API connection, no `WebAssembly` instance and
//! no signing. A schema states the shape and the bounds of a payload; carrying
//! one between two processes is later work, and the recorded roadmap keeps the
//! hardware-backed half at M21 and M22. M21's `aegis-vesta-sandbox` carries
//! the candidate evaluation over `AF_VSOCK` out of a Firecracker guest and
//! decodes it with this module; nothing here changed for it.
//!
//! # Versioning
//!
//! Each schema carries an explicit `schema` tag as its first field, typed as an
//! enum with one variant per admitted version, so a payload naming any other
//! version is refused by the decoder rather than parsed leniently. It also
//! carries its [`graph::EdgeId`], typed the same way, and a payload naming the
//! other subsystem's edge is refused with [`ContractError::WrongEdge`] -- this
//! crate has two outbound-and-inbound edges rather than one, so unlike the P15
//! registration the guard is not vacuous and is tested from both sides.
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

pub mod candidate_evaluation;
pub mod capsule_request;
pub mod encoding;
pub mod graph;

use crate::id::CorrelationId;

/// Scalar upper bound, in bytes, on one encoded contract payload.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 2048;

/// The consumer contracts this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The capsule request P09 Minerva sends to P10 Vesta.
    CapsuleRequest,
    /// The candidate evaluation P10 Vesta hands to P16 Athena.
    CandidateEvaluation,
}

impl SchemaId {
    /// Both contracts, in the order this module declares them.
    pub const ALL: [Self; 2] = [Self::CapsuleRequest, Self::CandidateEvaluation];

    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::CapsuleRequest => "aegis.p09-p10.capsule-request.v1",
            Self::CandidateEvaluation => "aegis.p10-p16.candidate-evaluation.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::CapsuleRequest => graph::EdgeId::ExecuteWasmedCapsule,
            Self::CandidateEvaluation => graph::EdgeId::SandboxCandidateEvaluation,
        }
    }
}

#[macro_use]
#[path = "../../../contract_scaffold.rs"]
mod contract_scaffold;

scaffold_edge_contract!(SchemaId, CorrelationId, MAX_CONTRACT_PAYLOAD_BYTES);
