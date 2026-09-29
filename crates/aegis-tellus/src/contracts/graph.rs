// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edges P13 sits on.
//!
//! The graph of record (export-062 `1ce919ed54bb`) names three edges touching
//! P13 Tellus. Milestone M05 typed two of them, and M16 the third:
//!
//! | Edge | Direction | Typed |
//! | :-- | :-- | :-- |
//! | `EVALUATE_CANDIDATE_CARBON_SCI` | P16 Athena to P13 Tellus | M05, both directions of the exchange |
//! | `SPATIOTEMPORAL_TASK_SHIFT` | P13 Tellus to P07 Lictor | M05 |
//! | `EMIT_CARBON_TELEMETRY` | P13 Tellus to P05 Forum | M16, [`super::telemetry::CarbonTelemetry`] |
//!
//! [`EdgeId::typed_at_m05`] still answers for M05 alone, so it stays `false`
//! for the telemetry edge. A payload naming an edge its schema does not carry
//! is refused by the wrong-edge check on whichever schema it claims to be.
//!
//! No transport exists in this crate. The transports the graph names -- D-Bus
//! for both outbound edges -- are recorded by [`EdgeId::recorded_transport`]
//! and implemented nowhere; D77 has since moved both off D-Bus to
//! line-delimited JSON-RPC 2.0 over `AF_UNIX`, which is recorded, not built,
//! here.

/// An edge of the subsystem graph that touches P13 Tellus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum EdgeId {
    /// P16 Athena to P13 Tellus: evaluate a candidate's SCI rate.
    #[serde(rename = "EVALUATE_CANDIDATE_CARBON_SCI")]
    EvaluateCandidateCarbonSci,
    /// P13 Tellus to P07 Lictor: shift background work in time or space.
    #[serde(rename = "SPATIOTEMPORAL_TASK_SHIFT")]
    SpatiotemporalTaskShift,
    /// P13 Tellus to P05 Forum: emit carbon telemetry, typed at M16.
    #[serde(rename = "EMIT_CARBON_TELEMETRY")]
    EmitCarbonTelemetry,
}

impl EdgeId {
    /// Every edge, in the order the graph of record lists them.
    pub const ALL: [Self; 3] = [
        Self::EvaluateCandidateCarbonSci,
        Self::SpatiotemporalTaskShift,
        Self::EmitCarbonTelemetry,
    ];

    /// Returns the edge identifier exactly as the graph of record spells it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::EvaluateCandidateCarbonSci => "EVALUATE_CANDIDATE_CARBON_SCI",
            Self::SpatiotemporalTaskShift => "SPATIOTEMPORAL_TASK_SHIFT",
            Self::EmitCarbonTelemetry => "EMIT_CARBON_TELEMETRY",
        }
    }

    /// Returns the component identifier at the far end of the edge.
    #[must_use]
    pub const fn peer(self) -> &'static str {
        match self {
            Self::EvaluateCandidateCarbonSci => "P16",
            Self::SpatiotemporalTaskShift => "P07",
            Self::EmitCarbonTelemetry => "P05",
        }
    }

    /// Returns the transport the graph of record names for the edge.
    ///
    /// Recorded, not implemented: no transport exists in this crate.
    #[must_use]
    pub const fn recorded_transport(self) -> &'static str {
        match self {
            Self::EvaluateCandidateCarbonSci => "ISO/IEC 21031:2024 SCI Rate Calculation",
            Self::SpatiotemporalTaskShift => "D-Bus / kepler_power.bpf",
            Self::EmitCarbonTelemetry => "D-Bus Signal org.aegisos.Tellus1",
        }
    }

    /// Returns `true` when milestone M05 types a payload for the edge.
    #[must_use]
    pub const fn typed_at_m05(self) -> bool {
        !matches!(self, Self::EmitCarbonTelemetry)
    }
}
