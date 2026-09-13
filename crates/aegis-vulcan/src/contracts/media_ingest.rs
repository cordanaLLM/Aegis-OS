// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The media-ingest descriptor P03 Vulcan hands to P15 Hestia.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge` | export-062 `1ce919ed54bb`, edge `ZERO_COPY_MEDIA_INGEST` (REQ-P15-07) |
//! | `bar` | export-038 `fe2cb01cfd13`, `VfioDeviceConfig.bar0_phys_addr` and `bar0_size`; the recorded transport is a `PCIe` BAR memory mapping |
//! | `blocks` | export-038 `fe2cb01cfd13`, `execute_direct_dma` |
//! | `correlation-id` | `docs/integration/stack.md`: every crossing carries one |
//! | `export` | the `DMA-BUF` export path M25 binds; see [`dmabuf`](super::dmabuf) |
//!
//! # What this module does not do
//!
//! No transport is implemented. Nothing here maps a BAR, opens a render node,
//! exports a `DMA-BUF` or decodes a frame. `aegis-hestia` does not depend on
//! this crate: the two P15 slices of M17 are independent, and wiring a real
//! producer to a real consumer is later work.
//!
//! The descriptor carries the export path and deliberately no exported file
//! descriptor. A `DMA-BUF` fd travels out of band, as an `SCM_RIGHTS`
//! ancillary message, so it is a value of the transport and not of the
//! payload. That is why this schema has no `dma_buf` field where the sibling
//! P15 contract `aegis_hestia::OverlayRegistration` has one: there the number
//! is positional documentation of a field the imported scaffold already
//! carried, and here there is no such recorded field to preserve. The
//! asymmetry is a decision, not an oversight.

use crate::bar::BarWindow;
use crate::contracts::dmabuf::DmaBufExport;
use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::dma::BlockCount;
use crate::id::CorrelationId;

/// The contract versions of the media-ingest descriptor this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum MediaIngestVersion {
    /// Version 1, tagged `aegis.p03-p15.media-ingest.v1`.
    #[serde(rename = "aegis.p03-p15.media-ingest.v1")]
    V1,
}

/// One zero-copy media run P03 would hand to P15 over a mapped BAR window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct MediaIngestDescriptor {
    /// The contract version this payload claims.
    pub schema: MediaIngestVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this descriptor to its result.
    pub correlation_id: CorrelationId,
    /// The mapped BAR window the run is carried over.
    pub bar: BarWindow,
    /// How many logical blocks the run moves.
    pub blocks: BlockCount,
    /// The `DMA-BUF` export path a real transfer would use.
    pub export: DmaBufExport,
}

impl MediaIngestDescriptor {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::MediaIngest;

    /// The edge the descriptor travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::ZeroCopyMediaIngest;

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Returns the bytes one run of this descriptor moves.
    #[must_use]
    pub const fn transfer_bytes(&self) -> u64 {
        self.blocks.transfer_bytes()
    }

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::WrongEdge`] when the payload names the other
    /// P03 edge, [`ContractError::Malformed`] when the BAR window leaves the
    /// address space, and [`ContractError::ExportPathUnusable`] when the
    /// declared export path could publish no render node.
    pub fn validate(&self) -> Result<(), ContractError> {
        let correlation = self.correlation();
        if self.edge != Self::EDGE {
            return Err(ContractError::WrongEdge { correlation });
        }
        if self.bar.validate().is_err() {
            return Err(ContractError::Malformed { correlation });
        }
        self.export
            .validate()
            .map_err(|error| ContractError::ExportPathUnusable {
                correlation,
                reason: error.reason(),
            })
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

    /// Decodes and validates one media-ingest payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::Malformed`] for anything else the
    /// schema refuses -- an unknown field, a missing field, a misaligned BAR
    /// address, or a block count outside `1..=8192` -- and
    /// [`ContractError::PayloadTooLong`] past the byte bound.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
