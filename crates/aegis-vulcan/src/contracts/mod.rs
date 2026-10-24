// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two versioned descriptors P03 hands to its consumers (milestone M17).
//!
//! P03 Vulcan produces for two neighbours, and each edge now has a typed,
//! versioned schema with a stable field encoding:
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P03 Vulcan to P09 Minerva, `GPUDIRECT_WEIGHT_STREAMING` | [`weight_stream::WeightStreamDescriptor`] | [`weight_stream`] |
//! | P03 Vulcan to P15 Hestia, `ZERO_COPY_MEDIA_INGEST` | [`media_ingest::MediaIngestDescriptor`] | [`media_ingest`] |
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! `mmap`, no `VFIO` container, no `CUDA` call, no `PCIe` peer-to-peer transfer and
//! no `DMA-BUF` file descriptor anywhere in this module or in the crate. A
//! descriptor states the shape and the bounds of a payload; carrying one
//! between two processes is later work, and the recorded roadmap keeps the
//! transports stubbed until M12. Binding a descriptor to a real render node is
//! M25.
//!
//! # Versioning
//!
//! Every schema carries an explicit `schema` tag as its first field, typed as
//! an enum with one variant per admitted version, so a payload naming any
//! other version is refused by the decoder rather than parsed leniently. Each
//! schema also carries its [`graph::EdgeId`], typed the same way: the edge
//! enum admits only the edge that schema travels on, so a descriptor asserting
//! the other P03 edge does not decode.
//!
//! # Bounds and allocation
//!
//! A payload longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it
//! is parsed at all, and every field of a decoded descriptor lands in a fixed
//! inline slot: both schema types are `Copy`, so a decoded value owns no heap.
//!
//! Decoding is **not** unconditionally allocation-free, and this module does
//! not claim it is. No contract path allocates for an escape-free payload: the
//! encoder writes into a caller-supplied [`PayloadBuffer`] and the decoder
//! reads each field straight out of its input. A payload that spells the same
//! value with a JSON escape is different -- `serde_json` unescapes any such
//! string into a heap scratch buffer before the field is validated -- and a
//! refusal allocates for the decoder's own error value. Both are transient,
//! neither is retained, and both are bounded by
//! [`MAX_CONTRACT_PAYLOAD_BYTES`], because a longer payload never reaches the
//! parser. `tests/allocation_bounds.rs` is the executable statement of this.

pub mod dmabuf;
pub mod encoding;
pub mod graph;
pub mod media_ingest;
pub mod weight_stream;

use crate::id::CorrelationId;

/// Scalar upper bound, in bytes, on one encoded descriptor payload.
///
/// Both descriptors are small and fixed: a version tag, an edge tag, a
/// correlation identifier and a handful of integers. The bound leaves an order
/// of magnitude of headroom and is the same for both, so one buffer serves
/// each.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 2048;

/// The two consumer contracts this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The weight-streaming descriptor P03 hands to P09 Minerva.
    WeightStream,
    /// The media-ingest descriptor P03 hands to P15 Hestia.
    MediaIngest,
}

impl SchemaId {
    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::WeightStream => "aegis.p03-p09.weight-stream.v1",
            Self::MediaIngest => "aegis.p03-p15.media-ingest.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::WeightStream => graph::EdgeId::GpudirectWeightStreaming,
            Self::MediaIngest => graph::EdgeId::ZeroCopyMediaIngest,
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

/// A fixed buffer holding one encoded descriptor payload.
pub type PayloadBuffer = contract_scaffold::PayloadBuffer<MAX_CONTRACT_PAYLOAD_BYTES>;

pub(crate) use contract_scaffold::decode_validated;
pub(crate) use dmabuf::DmaBufExport;
pub(crate) use graph::EdgeId;

define_contract_error! {
    SchemaId, CorrelationId;
    /// Reasons a descriptor payload is refused.
    ///
    /// Every variant carries its [`Correlation`], so no refusal is anonymous.
    pub enum ContractError {
        /// The payload names an edge this schema does not travel on.
        #[error("{correlation}: the payload names an edge this schema does not travel on")]
        WrongEdge {
            /// What the refusal is about.
            correlation: Correlation,
        },
        /// The declared export path cannot export a `DMA-BUF` as described.
        #[error("{correlation}: {reason}")]
        ExportPathUnusable {
            /// What the refusal is about.
            correlation: Correlation,
            /// Why the export path was refused.
            reason: &'static str,
        },
    }
}

scaffold_contract!(
    SchemaId,
    CorrelationId,
    MAX_CONTRACT_PAYLOAD_BYTES,
    ContractError
);

/// Validates descriptor invariants: edge, export path, and payload validity.
///
/// # Errors
///
/// Returns [`ContractError::WrongEdge`] if `edge` does not match `expected_edge`,
/// [`ContractError::Malformed`] if `payload_valid` is false, and
/// [`ContractError::ExportPathUnusable`] if `export` is unusable.
pub(crate) fn validate_descriptor(
    edge: graph::EdgeId,
    expected_edge: graph::EdgeId,
    export: dmabuf::DmaBufExport,
    correlation: Correlation,
    payload_valid: bool,
) -> Result<(), ContractError> {
    if edge != expected_edge {
        return Err(ContractError::WrongEdge { correlation });
    }
    if !payload_valid {
        return Err(ContractError::Malformed { correlation });
    }
    export
        .validate()
        .map_err(|error| ContractError::ExportPathUnusable {
            correlation,
            reason: error.reason(),
        })
}
