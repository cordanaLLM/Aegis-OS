// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The versioned registration P15 hands to its consumer (milestone M17).
//!
//! P15 Hestia produces for one neighbour, and that edge now has a typed,
//! versioned schema with a stable field encoding:
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P15 Hestia to P04 Mercurius, `REGISTER_PIP_OVERLAY` | [`overlay_registration::OverlayRegistration`] | [`overlay_registration`] |
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! Wayland connection, no `wlr-layer-shell` binding, no socket, no
//! `SCM_RIGHTS` passing and no `DMA-BUF` import anywhere in this module or in
//! the crate. A registration states the shape and the bounds of a payload;
//! carrying one between two processes is later work. Milestone M27 exercises
//! the layer-shell client half against the host session's compositor, and no
//! milestone yet plans a P04 that serves `zwlr_layer_shell_v1` (D79).
//!
//! # Versioning
//!
//! The schema carries an explicit `schema` tag as its first field, typed as an
//! enum with one variant per admitted version, so a payload naming any other
//! version is refused by the decoder rather than parsed leniently. It also
//! carries its [`graph::EdgeId`], typed the same way.
//!
//! There is deliberately no wrong-edge refusal here, because there is nothing
//! it could refuse: P15 produces on exactly one edge, [`graph::EdgeId`] has
//! exactly one variant, and a payload naming another edge is rejected by the
//! decoder as [`ContractError::Malformed`] before any check of ours runs. A
//! second outbound edge would mean adding both the variant and the guard.
//!
//! # Bounds and allocation
//!
//! A payload longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it
//! is parsed at all, and every field of a decoded registration lands in a
//! fixed inline slot: the schema type is `Copy`, so a decoded value owns no
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
pub mod overlay_registration;

use crate::id::CorrelationId;

/// Scalar upper bound, in bytes, on one encoded registration payload.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 2048;

/// The consumer contract this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The overlay registration P15 hands to P04 Mercurius.
    OverlayRegistration,
}

impl SchemaId {
    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::OverlayRegistration => "aegis.p15-p04.overlay-registration.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::OverlayRegistration => graph::EdgeId::RegisterPipOverlay,
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

/// A fixed buffer holding one encoded registration payload.
pub type PayloadBuffer = contract_scaffold::PayloadBuffer<MAX_CONTRACT_PAYLOAD_BYTES>;

define_contract_error! {
    SchemaId, CorrelationId;
    /// Reasons a registration payload is refused.
    ///
    /// Every variant carries its [`Correlation`], so no refusal is anonymous.
    pub enum ContractError {
        /// A pinned picture-in-picture surface was registered on another layer.
        #[error("{correlation}: a pinned picture-in-picture surface belongs on the overlay layer")]
        WrongLayer {
            /// What the refusal is about.
            correlation: Correlation,
        },
    }
}

scaffold_contract!(
    SchemaId,
    CorrelationId,
    MAX_CONTRACT_PAYLOAD_BYTES,
    ContractError
);
