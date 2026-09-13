// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edges P07 Lictor stands on.
//!
//! Every payload names its edge, so a message cannot be replayed onto a
//! different crossing by changing nothing but the schema tag. The identifiers
//! and the transports are the graph of record's own (export-062
//! `1ce919ed54bb`); nothing here edits them.
//!
//! Two of the four are graph-only edges -- they appear in the subsystem graph
//! and in neither architecture document -- which is dispute DSP-19. That is
//! recorded rather than resolved: a single-source edge is typed here with the
//! one source it has, and the register says so.

/// The graph edges this crate's payloads may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// `P04_Mercurius -> P07_Lictor`, `FOCUS_SWITCH_NOTIFY`.
    #[serde(rename = "P04_Mercurius->P07_Lictor:FOCUS_SWITCH_NOTIFY")]
    FocusSwitchNotify,
    /// `P07_Lictor -> P04_Mercurius`, `PRIORITIZE_COMPOSITOR_THREAD`.
    ///
    /// Representable but carried by no schema here: the payload the graph
    /// records for it is a dispatch-queue assignment, which is a scheduler
    /// action rather than a message, and milestone M19 is where an object that
    /// could perform one is loaded.
    #[serde(rename = "P07_Lictor->P04_Mercurius:PRIORITIZE_COMPOSITOR_THREAD")]
    PrioritizeCompositorThread,
    /// `P07_Lictor -> P08_Calliope`, `ENFORCE_REALTIME_RTPRIO`.
    ///
    /// Typed by its consumer, in `aegis-calliope`; this crate builds it.
    #[serde(rename = "P07_Lictor->P08_Calliope:ENFORCE_REALTIME_RTPRIO")]
    EnforceRealtimeRtprio,
    /// `P13_Tellus -> P07_Lictor`, `SPATIOTEMPORAL_TASK_SHIFT`.
    ///
    /// Typed by its producer at milestone M05, in `aegis-tellus`; this crate
    /// consumes it through that crate's own decoder.
    #[serde(rename = "P13_Tellus->P07_Lictor:SPATIOTEMPORAL_TASK_SHIFT")]
    SpatiotemporalTaskShift,
}

impl EdgeId {
    /// The four edges, in the order this module declares them.
    pub const ALL: [Self; 4] = [
        Self::FocusSwitchNotify,
        Self::PrioritizeCompositorThread,
        Self::EnforceRealtimeRtprio,
        Self::SpatiotemporalTaskShift,
    ];

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::FocusSwitchNotify => "P04_Mercurius->P07_Lictor:FOCUS_SWITCH_NOTIFY",
            Self::PrioritizeCompositorThread => {
                "P07_Lictor->P04_Mercurius:PRIORITIZE_COMPOSITOR_THREAD"
            }
            Self::EnforceRealtimeRtprio => "P07_Lictor->P08_Calliope:ENFORCE_REALTIME_RTPRIO",
            Self::SpatiotemporalTaskShift => "P13_Tellus->P07_Lictor:SPATIOTEMPORAL_TASK_SHIFT",
        }
    }

    /// Returns the transport the graph of record records for this edge.
    ///
    /// Recorded, not implemented: nothing in this crate opens any of them.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::FocusSwitchNotify => "Eclipse Zenoh Zero-Copy Shared Memory",
            Self::PrioritizeCompositorThread => "Linux sched_ext struct_ops / scx_cake",
            Self::EnforceRealtimeRtprio => "Linux cgroups v2 / RLIMIT_RTPRIO",
            Self::SpatiotemporalTaskShift => "D-Bus / kepler_power.bpf",
        }
    }

    /// Returns `true` when dispute DSP-19 lists this edge as graph-only.
    ///
    /// DSP-19 enumerates eight edges that the subsystem graph draws and that
    /// appear in neither architecture document; two of them are P07's own
    /// outbound edges. This is membership of that recorded list and nothing
    /// wider: `false` means the edge is not on the DSP-19 list, **not** that
    /// two documents attest it. No check in this crate reads either
    /// architecture document.
    #[must_use]
    pub const fn listed_graph_only_by_dsp_19(self) -> bool {
        matches!(
            self,
            Self::PrioritizeCompositorThread | Self::EnforceRealtimeRtprio
        )
    }

    /// Returns `true` when a schema in this crate declares the edge.
    #[must_use]
    pub const fn declared_here(self) -> bool {
        matches!(self, Self::FocusSwitchNotify)
    }
}
