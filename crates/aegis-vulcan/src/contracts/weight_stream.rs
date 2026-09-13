// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The weight-streaming descriptor P03 Vulcan hands to P09 Minerva.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge` | export-062 `1ce919ed54bb`, edge `GPUDIRECT_WEIGHT_STREAMING` (REQ-P03-07, REQ-P09-04) |
//! | `group` | export-038 `fe2cb01cfd13`, `VfioDeviceConfig.group_id`; REQ-P03-06 isolation model |
//! | `nvme-lba`, `destination`, `blocks` | export-038 `fe2cb01cfd13`, `execute_direct_dma` |
//! | `correlation-id` | `docs/integration/stack.md`: every crossing carries one |
//! | `export` | the `DMA-BUF` export path M25 binds; see [`dmabuf`](super::dmabuf) |
//!
//! # What this module does not do
//!
//! No transport is implemented. Nothing here opens a render node, maps a BAR,
//! issues an `NVMe` command, exports a `DMA-BUF` or calls `CUDA`. The P09 crate that
//! would receive one of these does not exist yet, which is why the schema
//! lives with its producer.
//!
//! The descriptor carries the export path and deliberately no exported file
//! descriptor. A `DMA-BUF` fd travels out of band, as an `SCM_RIGHTS`
//! ancillary message, so it is a value of the transport and not of the
//! payload. That is why this schema has no `dma_buf` field where the sibling
//! P15 contract `aegis_hestia::OverlayRegistration` has one: there the number
//! is positional documentation of a field the imported scaffold already
//! carried, and here there is no such recorded field to preserve. The
//! asymmetry is a decision, not an oversight.

use crate::contracts::dmabuf::DmaBufExport;
use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::device::IommuGroup;
use crate::dma::{BlockCount, Lba, VramAddress};
use crate::id::CorrelationId;

/// The contract versions of the weight-stream descriptor this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum WeightStreamVersion {
    /// Version 1, tagged `aegis.p03-p09.weight-stream.v1`.
    #[serde(rename = "aegis.p03-p09.weight-stream.v1")]
    V1,
}

/// One run of model weights P03 would stream into a P09 device allocation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct WeightStreamDescriptor {
    /// The contract version this payload claims.
    pub schema: WeightStreamVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this descriptor to its result.
    pub correlation_id: CorrelationId,
    /// The `IOMMU` group the `NVMe` source is isolated in.
    pub group: IommuGroup,
    /// Where the weights start on the `NVMe` source.
    pub nvme_lba: Lba,
    /// Where the weights land on the consumer's device.
    pub destination: VramAddress,
    /// How many logical blocks the run moves.
    pub blocks: BlockCount,
    /// The `DMA-BUF` export path a real transfer would use.
    pub export: DmaBufExport,
}

impl WeightStreamDescriptor {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::WeightStream;

    /// The edge the descriptor travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::GpudirectWeightStreaming;

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
    /// P03 edge, and [`ContractError::ExportPathUnusable`] when the declared
    /// export path could publish no render node.
    pub fn validate(&self) -> Result<(), ContractError> {
        let correlation = self.correlation();
        if self.edge != Self::EDGE {
            return Err(ContractError::WrongEdge { correlation });
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

    /// Decodes and validates one weight-stream payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::Malformed`] for anything else the
    /// schema refuses -- an unknown field, a missing field, or a block count
    /// outside `1..=8192` -- and [`ContractError::PayloadTooLong`] past the
    /// byte bound.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
