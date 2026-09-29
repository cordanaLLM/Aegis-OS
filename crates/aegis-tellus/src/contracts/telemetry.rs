// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The carbon telemetry update on edge `EMIT_CARBON_TELEMETRY` (milestone M16).
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge` | export-062 `1ce919ed54bb`, edge `EMIT_CARBON_TELEMETRY`, P13 Tellus to P05 Forum |
//! | `power-watts`, `sci-rate`, `grid-intensity` | export-050 `8ffa666dcc5a`, signal `org.aegisos.Tellus1.TelemetryUpdated(watts, sci, grid)` |
//! | `provenance` | this crate's [`Provenance`]: whether the draw was measured or stands in for a measurement |
//! | `correlation-id` | `docs/integration/stack.md` |
//!
//! The communication matrices name the same three values: export-003
//! `13af15ffc316` gives power draw in watts and the ISO/IEC 21031 SCI rate at
//! a one-second interval, export-002 `7e0c95f4ea05` watts, SCI rate and grid
//! carbon under 2 ms. Which of those two figures is an emission interval and
//! which a delivery deadline is D31, still open, so the payload carries
//! neither.
//!
//! # Why the payload says where its draw came from
//!
//! Nothing in this crate measures a watt: every draw it produces is
//! [`Provenance::Simulated`] or [`Provenance::Modelled`] until the M21 RAPL
//! reader exists. A shell that rendered "14.2 W" from a simulated constant
//! would present a stand-in as a measurement, so the payload carries the
//! provenance and a consumer can label the figure honestly.
//!
//! # Zero is a reading, not a missing value
//!
//! A draw of `0.0` W is admitted, as [`Watts`] admits it: an idle or
//! power-gated package reports zero, and the shell must render it rather than
//! treat it as absent (E16-2's boundary). With zero energy the SCI rate is the
//! embodied share alone, which is still a valid rate.
//!
//! # Transport
//!
//! None is implemented here. The graph of record names a D-Bus signal on
//! `org.aegisos.Tellus1`; D77 moved the edge to one line-delimited JSON-RPC
//! 2.0 message on an `AF_UNIX` stream, whose `params` is this payload, and M16
//! exercises that framing on a mocked stream in the Forum shell. The shell
//! (`crates/aegis-forum-shell`) decodes this type directly through
//! [`CarbonTelemetry::decode`], so no schema file, generated binding or copy
//! in a second language exists (D101).

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::id::CorrelationId;
use crate::power::Provenance;
use crate::sci::{GridIntensity, SciRate, Watts};

/// The contract versions of the carbon telemetry update this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum CarbonTelemetryVersion {
    /// Version 1, tagged `aegis.p13-p05.carbon-telemetry.v1`.
    #[serde(rename = "aegis.p13-p05.carbon-telemetry.v1")]
    V1,
}

/// One carbon telemetry update, for the shell's status display.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct CarbonTelemetry {
    /// The contract version this payload claims.
    pub schema: CarbonTelemetryVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this update to what produced it.
    pub correlation_id: CorrelationId,
    /// The current draw, in watts; zero is a valid reading.
    pub power_watts: Watts,
    /// The current SCI rate, in gCO2eq per functional unit.
    pub sci_rate: SciRate,
    /// The grid carbon intensity the rate was computed at, in gCO2eq/kWh.
    pub grid_intensity: GridIntensity,
    /// Whether the draw was measured, modelled or simulated.
    pub provenance: Provenance,
}

impl CarbonTelemetry {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::CarbonTelemetry;

    /// The edge the update travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::EmitCarbonTelemetry;

    /// Builds a version-1 update on the graph's edge.
    #[must_use]
    pub const fn new(
        correlation_id: CorrelationId,
        power_watts: Watts,
        sci_rate: SciRate,
        grid_intensity: GridIntensity,
        provenance: Provenance,
    ) -> Self {
        Self {
            schema: CarbonTelemetryVersion::V1,
            edge: Self::EDGE,
            correlation_id,
            power_watts,
            sci_rate,
            grid_intensity,
            provenance,
        }
    }

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Checks the invariant the field types cannot express: the edge.
    ///
    /// Every quantity is already bounded by its own type, so a decoded update
    /// holds no negative, non-finite or over-bound value.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::WrongEdge`] for a payload naming another edge.
    pub fn validate(&self) -> Result<(), ContractError> {
        if self.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation: self.correlation(),
                found: self.edge.name(),
            });
        }
        Ok(())
    }

    /// Encodes a validated update into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one telemetry payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::PayloadTooLong`] past the byte
    /// bound, [`ContractError::Malformed`] for anything the schema refuses --
    /// a negative draw, an unknown provenance, an unknown field -- and
    /// otherwise whatever [`Self::validate`] refuses.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
