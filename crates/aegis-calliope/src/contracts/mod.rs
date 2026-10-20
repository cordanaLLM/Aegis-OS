// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two versioned payloads P08 Calliope stands at either end of (M07).
//!
//! | Edge | Schema | Module |
//! | :-- | :-- | :-- |
//! | P07 Lictor to P08 Calliope, `ENFORCE_REALTIME_RTPRIO` | [`realtime_grant::RealtimeGrant`] | [`realtime_grant`] |
//! | P08 Calliope to P04 Mercurius, `SHARE_DMA_BUF_STREAM` | [`dma_buf_stream::DmaBufStreamDescriptor`] | [`dma_buf_stream`] |
//!
//! The inbound schema lives with its consumer, which is the shape milestone
//! M14 set and milestone M06 followed. The outbound one lives with its
//! producer for a structural reason rather than a stylistic one: P04's crate
//! consumes it, and P04's crate already depends on this one, so putting the
//! descriptor in `aegis-compositor` would close a cycle in the crate graph.
//! `aegis-compositor` therefore decodes it through this crate's own decoder,
//! and `crates/aegis-compositor/src/chain.rs` is where that happens.
//!
//! # What this module is not
//!
//! **No transport is implemented here, and none is implied.** There is no
//! `PipeWire` connection, no `SPA` buffer, no DMA-BUF file descriptor, no socket
//! and no `cgroup` write. A schema states the shape and the bounds of a
//! payload; carrying one between two processes is later work.
//!
//! # Versioning
//!
//! Each schema carries an explicit `schema` tag as its first field, typed as an
//! enum with one variant per admitted version, so a payload naming any other
//! version is refused by the decoder rather than parsed leniently. It also
//! carries its [`graph::EdgeId`], typed the same way, and a payload naming the
//! other edge is refused with [`ContractError::WrongEdge`] -- this crate
//! carries one inbound and one outbound edge, so the guard is not vacuous and
//! is tested from both sides.
//!
//! # Bounds and allocation
//!
//! A payload longer than [`MAX_CONTRACT_PAYLOAD_BYTES`] is refused before it is
//! parsed at all, and every field of a decoded payload lands in a fixed inline
//! slot: both schema types are `Copy`, so a decoded value owns no heap.
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

pub mod dma_buf_stream;
pub mod encoding;
pub mod graph;
pub mod realtime_grant;

use crate::id::CorrelationId;

/// Scalar upper bound, in bytes, on one encoded contract payload.
pub const MAX_CONTRACT_PAYLOAD_BYTES: usize = 2048;

/// The contracts this module defines.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SchemaId {
    /// The real-time grant P07 Lictor sends to P08 Calliope.
    RealtimeGrant,
    /// The DMA-BUF stream descriptor P08 Calliope sends to P04 Mercurius.
    DmaBufStream,
}

impl SchemaId {
    /// Both contracts, in the order this module declares them.
    pub const ALL: [Self; 2] = [Self::RealtimeGrant, Self::DmaBufStream];

    /// Returns the stable version tag the payload's `schema` field carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::RealtimeGrant => "aegis.p07-p08.realtime-grant.v1",
            Self::DmaBufStream => "aegis.p08-p04.dma-buf-stream.v1",
        }
    }

    /// Returns the subsystem-graph edge this schema carries.
    #[must_use]
    pub const fn edge(self) -> graph::EdgeId {
        match self {
            Self::RealtimeGrant => graph::EdgeId::EnforceRealtimeRtprio,
            Self::DmaBufStream => graph::EdgeId::ShareDmaBufStream,
        }
    }
}

#[macro_use]
#[path = "../../../contract_scaffold.rs"]
mod contract_scaffold;

/// What a refusal is about: the contract, and the payload when it could be read.
pub type Correlation = contract_scaffold::Correlation<SchemaId, CorrelationId>;

/// A fixed buffer holding one encoded contract payload.
pub type PayloadBuffer = contract_scaffold::PayloadBuffer<MAX_CONTRACT_PAYLOAD_BYTES>;

define_contract_error! {
    SchemaId, CorrelationId;
    /// Reasons a contract payload is refused.
    ///
    /// Every variant carries its [`Correlation`], so no refusal is anonymous.
    pub enum ContractError {
        /// The payload travels on an edge this schema does not carry.
        #[error("{correlation}: this schema does not travel on the edge the payload names")]
        WrongEdge {
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
