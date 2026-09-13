// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edges P08 Calliope stands on.
//!
//! Every payload names its edge, so a message cannot be replayed onto a
//! different crossing by changing nothing but the schema tag. The identifiers
//! and the transports are the graph of record's own (export-062
//! `1ce919ed54bb`); nothing here edits them.

/// The graph edges this crate's payloads may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// `P07_Lictor -> P08_Calliope`, `ENFORCE_REALTIME_RTPRIO`.
    #[serde(rename = "P07_Lictor->P08_Calliope:ENFORCE_REALTIME_RTPRIO")]
    EnforceRealtimeRtprio,
    /// `P08_Calliope -> P04_Mercurius`, `SHARE_DMA_BUF_STREAM`.
    #[serde(rename = "P08_Calliope->P04_Mercurius:SHARE_DMA_BUF_STREAM")]
    ShareDmaBufStream,
    /// `P08_Calliope -> P11_Ludus`, `REMOTE_PLAY_CAPTURE`.
    ///
    /// Representable but carried by no schema here: decision D29 leaves the
    /// direction of this edge open, and a payload would have to assert one.
    #[serde(rename = "P08_Calliope->P11_Ludus:REMOTE_PLAY_CAPTURE")]
    RemotePlayCapture,
}

impl EdgeId {
    /// The three edges, in the order this module declares them.
    pub const ALL: [Self; 3] = [
        Self::EnforceRealtimeRtprio,
        Self::ShareDmaBufStream,
        Self::RemotePlayCapture,
    ];

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::EnforceRealtimeRtprio => "P07_Lictor->P08_Calliope:ENFORCE_REALTIME_RTPRIO",
            Self::ShareDmaBufStream => "P08_Calliope->P04_Mercurius:SHARE_DMA_BUF_STREAM",
            Self::RemotePlayCapture => "P08_Calliope->P11_Ludus:REMOTE_PLAY_CAPTURE",
        }
    }

    /// Returns the transport the graph of record records for this edge.
    ///
    /// Recorded, not implemented: nothing in this crate opens any of them.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::EnforceRealtimeRtprio => "Linux cgroups v2 / RLIMIT_RTPRIO",
            Self::ShareDmaBufStream => "PipeWire pipewiresrc / SPA_DATA_DmaBuf",
            Self::RemotePlayCapture => "Gamescope / PipeWire DMA-BUF",
        }
    }

    /// Returns `true` when a schema in this crate carries the edge.
    #[must_use]
    pub const fn is_carried(self) -> bool {
        !matches!(self, Self::RemotePlayCapture)
    }
}
