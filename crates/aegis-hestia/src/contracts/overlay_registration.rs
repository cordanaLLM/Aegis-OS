// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The overlay registration P15 Hestia hands to P04 Mercurius.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge`, `layer` | export-062 `1ce919ed54bb`, edge `REGISTER_PIP_OVERLAY` over the `wlr-layer-shell` overlay surface protocol |
//! | `dma-buf`, `width`, `height` | export-030 `689d175667d6`, `WaylandPipMediaController::launch_pip_overlay` |
//! | `correlation-id`, `surface` | `docs/integration/stack.md`: every crossing carries a correlation identifier |
//!
//! # The layer is part of the contract
//!
//! The recorded edge is specifically an overlay registration, so a payload
//! naming any other `wlr-layer-shell` layer is refused with
//! [`ContractError::WrongLayer`] rather than accepted and pinned in the wrong
//! place. The other three layers stay representable in [`ShellLayer`] because
//! the protocol defines them; this contract simply does not carry them.
//!
//! # What this module does not do
//!
//! No transport is implemented. Nothing here connects to a compositor, binds a
//! protocol, creates a surface, or passes a file descriptor. The descriptor
//! number in a payload is positional: on a real system a `DMA-BUF` travels as
//! an `SCM_RIGHTS` ancillary message and is meaningful only inside the process
//! that holds it. The P04 crate that would receive one of these does not exist
//! yet, which is why the schema lives with its producer.

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::id::{CorrelationId, SurfaceId};
use crate::overlay::{DmaBufFd, PipSurface, PixelExtent, ShellLayer};

/// The contract versions of the overlay registration this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum OverlayRegistrationVersion {
    /// Version 1, tagged `aegis.p15-p04.overlay-registration.v1`.
    #[serde(rename = "aegis.p15-p04.overlay-registration.v1")]
    V1,
}

/// One pinned picture-in-picture surface P15 asks P04 to place.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct OverlayRegistration {
    /// The contract version this payload claims.
    pub schema: OverlayRegistrationVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this registration to its result.
    pub correlation_id: CorrelationId,
    /// The surface the registration is about.
    pub surface: SurfaceId,
    /// The layer the surface is pinned to.
    pub layer: ShellLayer,
    /// The imported buffer the surface displays.
    pub dma_buf: DmaBufFd,
    /// The surface width, in pixels.
    pub width: PixelExtent,
    /// The surface height, in pixels.
    pub height: PixelExtent,
}

impl OverlayRegistration {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::OverlayRegistration;

    /// The edge the registration travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::RegisterPipOverlay;

    /// The only layer this contract carries.
    pub const LAYER: ShellLayer = ShellLayer::Overlay;

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Returns the surface this registration describes.
    #[must_use]
    pub const fn as_surface(&self) -> PipSurface {
        PipSurface::new(self.dma_buf, self.layer, self.width, self.height)
    }

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::WrongLayer`] when the payload names a layer
    /// other than [`Self::LAYER`].
    ///
    /// The edge needs no check of its own: [`EdgeId`] admits exactly the edge
    /// this schema travels on, so a payload naming another is refused by the
    /// decoder before this runs.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.layer != Self::LAYER {
            return Err(ContractError::WrongLayer {
                correlation: self.correlation(),
            });
        }
        Ok(())
    }

    /// Encodes a validated registration into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one overlay registration payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::Malformed`] for anything else the
    /// schema refuses -- an unknown field, a missing field, a negative
    /// `DMA-BUF` descriptor, or an extent outside `1..=16384` -- and
    /// [`ContractError::PayloadTooLong`] past the byte bound.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
