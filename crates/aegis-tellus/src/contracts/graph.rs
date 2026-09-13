// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The subsystem-graph edges P13 sits on.
//!
//! The graph of record (export-062 `1ce919ed54bb`) names three edges touching
//! P13 Tellus, and this milestone types two of them:
//!
//! | Edge | Direction | Typed here |
//! | :-- | :-- | :-- |
//! | `EVALUATE_CANDIDATE_CARBON_SCI` | P16 Athena to P13 Tellus | yes, both directions of the exchange |
//! | `SPATIOTEMPORAL_TASK_SHIFT` | P13 Tellus to P07 Lictor | yes |
//! | `EMIT_CARBON_TELEMETRY` | P13 Tellus to P05 Forum | no: the payload is milestone M16 |
//!
//! The third variant exists without a schema on purpose. The edge is in the
//! graph of record, so leaving it out of [`EdgeId`] would make the vocabulary
//! disagree with the graph; and giving it a schema here would duplicate work
//! the roadmap assigns to M16. A payload naming it is refused by the wrong-edge
//! check on whichever schema it claims to be.
//!
//! No transport exists in this crate. The transports the graph names -- D-Bus
//! for both outbound edges -- are recorded by [`EdgeId::recorded_transport`]
//! and implemented nowhere.

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
    /// P13 Tellus to P05 Forum: emit carbon telemetry. Payload is M16 work.
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
