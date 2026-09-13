// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two subsystem-graph edges P03 produces on.
//!
//! The graph of record (export-062 `1ce919ed54bb`) names both edges and both
//! directions: `GPUDIRECT_WEIGHT_STREAMING` runs from P03 Vulcan to P09
//! Minerva (REQ-P03-07, REQ-P09-04), and `ZERO_COPY_MEDIA_INGEST` runs from
//! P03 Vulcan to P15 Hestia (REQ-P15-07). Neither direction is disputed, so
//! nothing here is annotated the way `aegis-justitia` annotates D03.
//!
//! The edge is a field of every descriptor and a constant of every descriptor
//! type. Because [`EdgeId`] has one variant per admitted edge and each schema
//! refuses the edge it does not travel on, a media-ingest payload claiming to
//! be a weight stream is refused by name rather than accepted and mis-routed.

/// An edge of the subsystem graph that P03 Vulcan produces on.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// P03 Vulcan to P09 Minerva: zero-copy `NVMe`-to-GPU weight loading.
    #[serde(rename = "GPUDIRECT_WEIGHT_STREAMING")]
    GpudirectWeightStreaming,
    /// P03 Vulcan to P15 Hestia: zero-copy media ingest over a `PCIe` BAR.
    #[serde(rename = "ZERO_COPY_MEDIA_INGEST")]
    ZeroCopyMediaIngest,
}

impl EdgeId {
    /// Both edges, in the order the graph of record lists them.
    pub const ALL: [Self; 2] = [Self::GpudirectWeightStreaming, Self::ZeroCopyMediaIngest];

    /// Returns the edge identifier exactly as the graph of record spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::GpudirectWeightStreaming => "GPUDIRECT_WEIGHT_STREAMING",
            Self::ZeroCopyMediaIngest => "ZERO_COPY_MEDIA_INGEST",
        }
    }

    /// Returns the component identifier at the far end of the edge.
    #[must_use]
    pub const fn consumer(self) -> &'static str {
        match self {
            Self::GpudirectWeightStreaming => "P09",
            Self::ZeroCopyMediaIngest => "P15",
        }
    }

    /// Returns the transport the graph of record names for the edge.
    ///
    /// Recorded, not implemented: no transport exists in this crate, and the
    /// roadmap keeps both stubbed until M12.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::GpudirectWeightStreaming => "PCIe P2PDMA / CUDA GPUDirect",
            Self::ZeroCopyMediaIngest => "PCIe BAR / Memory Mapping",
        }
    }
}
