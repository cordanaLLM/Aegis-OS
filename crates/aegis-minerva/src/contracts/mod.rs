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

#[macro_use]
#[path = "../../../contract_scaffold.rs"]
mod contract_scaffold;

/// What a refusal is about: the contract, and the payload when it could be read.
pub type Correlation = contract_scaffold::Correlation<SchemaId, Identity>;

/// A fixed buffer holding one encoded contract payload.
pub type PayloadBuffer = contract_scaffold::PayloadBuffer<MAX_CONTRACT_PAYLOAD_BYTES>;

define_contract_error! {
    SchemaId, Identity;
    /// Reasons a contract payload is refused.
    ///
    /// Every variant carries its [`Correlation`], so no refusal is anonymous.
    pub enum ContractError {
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
}

scaffold_contract!(
    SchemaId,
    Identity,
    MAX_CONTRACT_PAYLOAD_BYTES,
    ContractError
);
