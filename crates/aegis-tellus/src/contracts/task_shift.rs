// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The task-shift directive on edge `SPATIOTEMPORAL_TASK_SHIFT`.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge`, `slice` | export-062 `1ce919ed54bb`: P13 Tellus defers background builds and batch inference on high grid carbon, per cgroup v2 slice |
//! | `intensity`, `threshold`, `defer` | export-036 `25813d240733`, `should_defer_spatiotemporal_tasks` |
//! | `correlation-id` | `docs/integration/stack.md` |
//!
//! # The directive carries the reason, not just the verdict
//!
//! A payload that said only "defer" would leave P07 Lictor unable to tell a
//! deferral from a misconfiguration. It carries the intensity it was decided
//! at and the threshold it was compared against, so the decision is
//! reconstructible from the payload alone -- and so a directive whose verdict
//! does not follow from its own two numbers is refused rather than obeyed.

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::id::{CorrelationId, SliceName};
use crate::sci::{DEFER_THRESHOLD_G_PER_KWH, GridIntensity, SciEngine};

/// The contract versions of the task-shift directive this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum TaskShiftVersion {
    /// Version 1, tagged `aegis.p13-p07.task-shift.v1`.
    #[serde(rename = "aegis.p13-p07.task-shift.v1")]
    V1,
}

/// One instruction to shift, or not to shift, a slice's background work.
#[derive(Debug, Clone, Copy, PartialEq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TaskShiftDirective {
    /// The contract version this payload claims.
    pub schema: TaskShiftVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this directive to what asked for it.
    pub correlation_id: CorrelationId,
    /// The cgroup v2 slice the directive is about.
    pub slice: SliceName,
    /// The grid carbon intensity the decision was taken at.
    pub intensity: GridIntensity,
    /// Whether the slice's background work is to be deferred.
    pub defer: bool,
}

impl TaskShiftDirective {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::TaskShiftDirective;

    /// The edge the directive travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::SpatiotemporalTaskShift;

    /// The threshold the directive's verdict is checked against.
    pub const THRESHOLD: f64 = DEFER_THRESHOLD_G_PER_KWH;

    /// Builds the directive an engine would issue for `slice`.
    #[must_use]
    pub fn from_engine(
        engine: &SciEngine,
        correlation_id: CorrelationId,
        slice: SliceName,
    ) -> Self {
        Self {
            schema: TaskShiftVersion::V1,
            edge: Self::EDGE,
            correlation_id,
            slice,
            intensity: engine.intensity(),
            defer: engine.should_defer(),
        }
    }

    /// Returns what a refusal of this payload is about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Checks the invariants the field types cannot express.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::WrongEdge`] for a payload naming another edge,
    /// and [`ContractError::Malformed`] when the verdict does not follow from
    /// the intensity the payload itself carries.
    pub fn validate(&self) -> Result<(), ContractError> {
        let correlation = self.correlation();
        if self.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation,
                found: self.edge.name(),
            });
        }
        if self.defer != (self.intensity.get() > Self::THRESHOLD) {
            return Err(ContractError::Malformed { correlation });
        }
        Ok(())
    }

    /// Encodes a validated directive into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one directive payload.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::UnknownVersion`] for a payload naming another
    /// contract version, [`ContractError::PayloadTooLong`] past the byte
    /// bound, [`ContractError::Malformed`] for anything the schema refuses,
    /// and otherwise whatever [`Self::validate`] refuses.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let decoded: Self = decode_text(Self::SCHEMA, text)?;
        decoded.validate()?;
        Ok(decoded)
    }
}
