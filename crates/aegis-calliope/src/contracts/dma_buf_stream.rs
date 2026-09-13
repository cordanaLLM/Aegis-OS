// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `SHARE_DMA_BUF_STREAM`: the descriptor P08 Calliope hands P04 Mercurius
//! (REQ-P08-04, REQ-P08-01).
//!
//! The graph of record draws `P08_Calliope -> P04_Mercurius` over
//! `PipeWire pipewiresrc / SPA_DATA_DmaBuf`, carrying zero-copy video streams
//! for overlay picture-in-picture windows (export-062 `1ce919ed54bb`), and
//! export-002 draws the same edge, so this is one of the five relations both
//! documents agree on (dispute DSP-20).
//!
//! # Why the producer owns this schema
//!
//! The convention M14 set is that an inbound schema lives with its consumer.
//! Here the consumer is P04, whose crate already depends on this one for the
//! plugin and buffer bounds, so putting the descriptor there would close a
//! cycle in the crate graph. `aegis-compositor` decodes it through this
//! crate's decoder instead; `crates/aegis-compositor/src/chain.rs` is the
//! consumption site and `crates/aegis-compositor/tests/chain_contracts.rs`
//! exercises it.
//!
//! # What a descriptor is, and is not
//!
//! It is the shape of a frame: dimensions, the pixel format, the stride that
//! follows from the two, and an opaque table-local handle. It is **not** a
//! buffer: no DMA-BUF is exported, no file descriptor is opened or passed, and
//! nothing is mapped. The scaffold's `fd: RawFd` field is absent for that
//! reason -- see [`BufferHandle`].

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::dmabuf::{BufferHandle, BufferId, FrameGeometry, PixelFormat, stride_bytes};
use crate::id::{CorrelationId, Label};
use crate::sync::SyncStrategy;

/// The contract versions this build admits for the descriptor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum DmaBufStreamVersion {
    /// Version 1.
    #[serde(rename = "aegis.p08-p04.dma-buf-stream.v1")]
    V1,
}

/// The DMA-BUF stream descriptor P08 Calliope sends to P04 Mercurius.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct DmaBufStreamDescriptor {
    /// The contract version.
    pub schema: DmaBufStreamVersion,
    /// The graph edge this payload travels on.
    pub edge: EdgeId,
    /// The identifier that correlates this crossing.
    pub correlation_id: CorrelationId,
    /// The stream name.
    pub stream: Label,
    /// The descriptor identifier in the producer's table.
    pub buffer_id: BufferId,
    /// The opaque table-local handle; not a file descriptor.
    pub handle: BufferHandle,
    /// The row width in pixels.
    pub width: u32,
    /// The frame height in rows.
    pub height: u32,
    /// The pixel format the stride was computed for.
    pub format: PixelFormat,
    /// The row stride in bytes.
    pub stride_bytes: u32,
    /// How the two streams are kept in step.
    pub sync: SyncStrategy,
}

impl DmaBufStreamDescriptor {
    /// The contract this payload satisfies.
    pub const SCHEMA: SchemaId = SchemaId::DmaBufStream;

    /// The graph edge this payload travels on.
    pub const EDGE: EdgeId = EdgeId::ShareDmaBufStream;

    /// The synchronisation strategy every admitted descriptor names.
    pub const SYNC: SyncStrategy = SyncStrategy::ADMITTED;

    /// Returns what a refusal of this payload would be about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Returns the frame dimensions.
    #[must_use]
    pub const fn geometry(&self) -> FrameGeometry {
        FrameGeometry::new(self.width, self.height)
    }

    /// Decodes one descriptor from `text`.
    ///
    /// A descriptor whose stride does not follow from its width and its format
    /// is [`ContractError::Malformed`]: the three fields travel together, and a
    /// reader that trusted a stride the format contradicts would walk off the
    /// end of a row. This is the scaffold's `stride: width * 4` under an
    /// `NV12` comment, refused rather than carried.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::PayloadTooLong`] above the byte bound,
    /// [`ContractError::UnknownVersion`] for another contract version,
    /// [`ContractError::Malformed`] for anything else the decoder refuses, and
    /// [`ContractError::WrongEdge`] when the payload names the other edge this
    /// crate carries.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let descriptor: Self = decode_text(Self::SCHEMA, text)?;
        if descriptor.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation: descriptor.correlation(),
            });
        }
        let expected = stride_bytes(descriptor.format, descriptor.width).map_err(|_| {
            ContractError::Malformed {
                correlation: descriptor.correlation(),
            }
        })?;
        if expected != descriptor.stride_bytes {
            return Err(ContractError::Malformed {
                correlation: descriptor.correlation(),
            });
        }
        Ok(descriptor)
    }

    /// Encodes this descriptor into `buffer`.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::PayloadTooLong`] when the rendered payload
    /// does not fit the buffer's scalar bound.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        encode_into(self, self.correlation(), buffer)
    }
}
