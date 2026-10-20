// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The one versioned payload P14 Hephaestus produces (milestone M08).
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P14 Hephaestus to P15 Hestia, `RENDER_GEOMETRY_MICROFRONTEND` | [`viewport::GeometryViewport`] | [`viewport`] |
//!
//! P14's inbound edge is **not** declared here. `VERIFY_CODE_CAD` is
//! [`CadVerificationRequest`](aegis_minerva::CadVerificationRequest), typed at
//! M06 in the producer's crate, and [`verification`](crate::verification) is
//! where it is consumed rather than redefined.
//!
//! The viewport descriptor is typed in the producer's crate, which is where
//! every schema in this workspace lives except one: `aegis-vulcan` types both
//! descriptors P03 hands out -- including the one P15 receives, whose crate
//! already existed -- `aegis-hestia` types the registration P15 hands to P04,
//! and `aegis-minerva` types the request P09 submits here. The exception is the
//! P09-to-P10 capsule request, which `aegis-vesta` owns because its bounds and
//! its monitor identity are P10's rather than P09's. Nothing about a geometry
//! viewport is P15's in that way -- the element bound on it is
//! [`MAX_MESH_ELEMENTS`](crate::MAX_MESH_ELEMENTS), which is P14's -- so this
//! one stays with its producer.
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! socket, no browser, no database and no rendering of any kind. A descriptor
//! states the shape and the bounds of a payload; carrying one between two
//! processes is later work.
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

pub mod encoding;
pub mod graph;
pub mod viewport;

use aegis_justitia::Identity;

/// Scalar upper bound, in bytes, on one encoded contract payload.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 1024;

/// The consumer contract this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The geometry viewport descriptor P14 hands to P15.
    GeometryViewport,
}

impl SchemaId {
    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::GeometryViewport => "aegis.p14-p15.geometry-viewport.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::GeometryViewport => graph::EdgeId::RenderGeometryMicrofrontend,
        }
    }
}

#[macro_use]
#[path = "../../../contract_scaffold.rs"]
mod contract_scaffold;

/// What a refusal is about: the contract, and the payload when it could be read.
pub type Correlation = contract_scaffold::Correlation<SchemaId, Identity>;

/// A fixed buffer holding one encoded payload.
pub type PayloadBuffer = contract_scaffold::PayloadBuffer<MAX_CONTRACT_PAYLOAD_BYTES>;

define_contract_error! {
    SchemaId, Identity;
    /// Reasons a contract payload is refused.
    ///
    /// Every variant carries its [`Correlation`], so no refusal is anonymous.
    pub enum ContractError {
        /// A descriptor named an engine that cannot read the protocol it claims.
        ///
        /// Not a comparison of two fields, despite the name.
        /// [`StepSchema`](crate::geometry::StepSchema) has one variant, so every
        /// descriptor claims the same protocol and this refusal turns on the engine
        /// alone: the polyhedral engine has no representable viewport at this
        /// milestone, and a descriptor naming it is refused whatever else it says.
        #[error("{correlation}: the named engine does not read the STEP protocol this claims")]
        EngineProtocolMismatch {
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
