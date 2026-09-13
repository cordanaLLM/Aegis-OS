// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `FOCUS_SWITCH_NOTIFY`: the report P04 Mercurius hands P07 Lictor
//! (REQ-P04-07, REQ-P07-03).
//!
//! The graph of record draws `P04_Mercurius -> P07_Lictor` over shared memory,
//! carrying the active window's process identifier and a focus-switch event
//! (export-062 `1ce919ed54bb`). The shell protocol adds what the broker
//! actually needs in order to act: `report_focus_switch` takes an application
//! identifier **and a control-group slice**, so the compositor can drive
//! resource re-allocation on focus change (export-035 `4c2146ffed32`,
//! REQ-P04-07).
//!
//! This is the consumer's type for that message, so P04's crate builds it from
//! here rather than declaring a second shape for the same edge.
//!
//! # Why the slice is a stricter type here than at the producer
//!
//! [`SliceName`] requires the `.slice` suffix. The compositor could hand over
//! any string; a broker that took one would be asked to re-allocate a control
//! group that cannot exist. Validating on the consumer's side is the point of
//! the consumer owning the schema.

use crate::broker::Pid;
use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::id::{CorrelationId, Label, SliceName};

/// The contract versions this build admits for the report.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum FocusSwitchVersion {
    /// Version 1.
    #[serde(rename = "aegis.p04-p07.focus-switch.v1")]
    V1,
}

/// The focus-switch report P04 Mercurius sends to P07 Lictor.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub struct FocusSwitchReport {
    /// The contract version.
    pub schema: FocusSwitchVersion,
    /// The graph edge this payload travels on.
    pub edge: EdgeId,
    /// The identifier that correlates this crossing.
    pub correlation_id: CorrelationId,
    /// The application identifier the shell protocol carries.
    pub app_id: Label,
    /// The control-group slice the broker is to re-allocate.
    pub cgroup_slice: SliceName,
    /// The process that gained focus.
    pub pid: Pid,
    /// The compositor surface that gained focus.
    pub surface_id: u32,
}

impl FocusSwitchReport {
    /// The contract this payload satisfies.
    pub const SCHEMA: SchemaId = SchemaId::FocusSwitchReport;

    /// The graph edge this payload travels on.
    pub const EDGE: EdgeId = EdgeId::FocusSwitchNotify;

    /// Returns what a refusal of this payload would be about.
    #[must_use]
    pub const fn correlation(&self) -> Correlation {
        Correlation::new(Self::SCHEMA, Some(self.correlation_id))
    }

    /// Decodes one report from `text`.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::PayloadTooLong`] above the byte bound,
    /// [`ContractError::UnknownVersion`] for another contract version,
    /// [`ContractError::Malformed`] for anything else the decoder refuses --
    /// an unknown field, a process identifier of zero, a slice name without
    /// the `.slice` suffix -- and [`ContractError::WrongEdge`] when the
    /// payload names another of this crate's edges.
    pub fn decode(text: &str) -> Result<Self, ContractError> {
        let report: Self = decode_text(Self::SCHEMA, text)?;
        if report.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation: report.correlation(),
            });
        }
        Ok(report)
    }

    /// Encodes this report into `buffer`.
    ///
    /// # Errors
    ///
    /// Returns [`ContractError::PayloadTooLong`] when the rendered payload
    /// does not fit the buffer's scalar bound.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        encode_into(self, self.correlation(), buffer)
    }
}
