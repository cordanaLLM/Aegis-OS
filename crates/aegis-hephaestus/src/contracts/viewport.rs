// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The geometry viewport descriptor P14 Hephaestus hands to P15 Hestia.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge`, `step-schema` | export-062 `1ce919ed54bb`, edge `RENDER_GEOMETRY_MICROFRONTEND` (REQ-P14-06) |
//! | `engine`, `tolerance-micrometres` | export-021 `6a3cda152e6c`, the dual-engine approach (REQ-P14-01) |
//! | `elements` | export-029 `427996186520`, `MAX_MESH_ELEMENTS` (REQ-P14-02) |
//! | `source` | the caller's `STEP` source name, validated and never opened |
//! | `correlation-id`, `prepared-at` | `docs/integration/stack.md` and the M14 field encodings |
//!
//! # The element bound is the same bound
//!
//! `elements` is a [`MeshElementCount`], so the viewport carries P14's own
//! meshing bound onto the wire rather than a second copy of the number: a
//! descriptor naming 500,000 elements decodes and one naming 500,001 does not,
//! for the same reason [`BrepEvaluator::mesh`](crate::BrepEvaluator::mesh)
//! refuses the second. A viewport that could describe a mesh the producer
//! cannot plan would be a bound in one place and not the other.
//!
//! # The geometry does not travel
//!
//! A source name and a count do. Nothing here serialises a solid, a face or a
//! triangle: the descriptor says which source was prepared, under which engine
//! and protocol, at which tolerance, and how many elements the result has. What
//! a consumer would fetch with that is later work, and no transport exists.
//!
//! # What this module does not do
//!
//! No transport is implemented, no `CAD` kernel is linked, no file is opened
//! and nothing is rendered. There is no proof, no verdict and no claim that the
//! described geometry is valid: `verification` is where a request is admitted,
//! and it has no variant meaning verified either.

use aegis_justitia::{Identity, UnixSeconds};

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::geometry::{CadEngine, MeshElementCount, StepSchema, Tolerance};
use crate::id::StepPath;

/// The contract versions of the viewport descriptor this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum GeometryViewportVersion {
    /// Version 1, tagged `aegis.p14-p15.geometry-viewport.v1`.
    #[serde(rename = "aegis.p14-p15.geometry-viewport.v1")]
    V1,
}

/// One prepared geometry viewport P14 offers to P15.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct GeometryViewport {
    /// The contract version this payload claims.
    pub schema: GeometryViewportVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this descriptor to its request.
    pub correlation_id: Identity,
    /// The `STEP` source the viewport was prepared from. Never opened here.
    pub source: StepPath,
    /// The interchange protocol the source claims.
    pub step_schema: StepSchema,
    /// Which recorded engine prepared it.
    pub engine: CadEngine,
    /// The tolerance it was prepared at, in micrometres.
    pub tolerance_micrometres: Tolerance,
    /// How many mesh elements the result has, inside P14's own bound.
    pub elements: MeshElementCount,
    /// When the viewport was prepared.
    pub prepared_at: UnixSeconds,
}

impl GeometryViewport {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::GeometryViewport;

    /// The edge the descriptor travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::RenderGeometryMicrofrontend;

    /// The interchange protocol every descriptor this build produces claims.
    pub const STEP_SCHEMA: StepSchema = StepSchema::ADMITTED;

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::EngineProtocolMismatch`] when the descriptor
    /// names the polyhedral engine: the sources give `STEP` interchange to the
    /// exact boundary-representation engine, and a descriptor claiming both
    /// would tell a consumer to read the source with something that does not
    /// read it.
    ///
    /// The check is on the engine and nothing else. [`StepSchema`] has one
    /// variant, so there is no second field for it to disagree with, and the
    /// polyhedral engine -- the second half of REQ-P14-01's recorded
    /// dual-engine approach -- has no representable viewport at this milestone
    /// rather than one that is refused in some combinations.
    ///
    /// The edge needs no check of its own: [`EdgeId`] admits exactly the edge
    /// this schema travels on, so a payload naming another is refused by the
    /// decoder before this runs. Neither does the element count: its type
    /// carries [`MAX_MESH_ELEMENTS`](crate::MAX_MESH_ELEMENTS).
    pub fn validate(&self) -> Result<(), ContractError> {
        if !self.engine.reads_step() {
            return Err(ContractError::EngineProtocolMismatch {
                correlation: self.correlation(),
            });
        }
        Ok(())
    }

    /// Encodes a validated descriptor into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one viewport descriptor payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::Malformed`] for anything else the
    /// schema refuses -- an unknown field, an absent one, an element count past
    /// the bound, a tolerance outside its range, a malformed source path --
    /// [`ContractError::PayloadTooLong`] past the byte bound, and
    /// [`ContractError::EngineProtocolMismatch`] for the polyhedral engine,
    /// which has no representable viewport here.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
