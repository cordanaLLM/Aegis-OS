// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The promotion trigger on edge `TRIGGER_SYSUPDATE_ROLLBACK`.
//!
//! # Where the fields come from
//!
//! | Field | Source |
//! | :-- | :-- |
//! | `edge`, `action` | export-062 `1ce919ed54bb`, edge `TRIGGER_SYSUPDATE_ROLLBACK`, P16 to P02 |
//! | `slot` | export-063 `b284bb77c19f` (REQ-P01-04): sysupdate targets a dual-slot A/B root transfer |
//! | `candidate`, `stage`, `cleared` | export-025 `6b23723ddb76`, the candidate lifecycle and its Pareto gate |
//! | `correlation-id` | `docs/integration/stack.md` |
//!
//! # The action must follow from the stage
//!
//! A trigger carries both what it asks for and the stage that justifies it, and
//! [`PromotionTrigger::validate`] refuses a pair that does not follow: a deploy
//! from an invalidated candidate, or a rollback from a published one, is
//! [`ContractError::ActionDoesNotFollow`]. P02 would act on either, and the
//! action it would take is not reversible by a later message.
//!
//! # What this module does not do
//!
//! Nothing here runs `systemd-sysupdate`, opens D-Bus, writes a partition,
//! changes a boot order or reboots. The slot is a name, not a device. See
//! [`crate::ports::StubSysupdate`] for the recording stand-in that makes the
//! absence checkable.

use aegis_tellus::{CandidateId, CorrelationId};

use crate::contracts::graph::EdgeId;
use crate::contracts::{
    ContractError, Correlation, PayloadBuffer, SchemaId, decode_text, encode_into,
};
use crate::machine::Lifecycle;
use crate::stage::Stage;

/// Which of the two dual-slot roots a trigger names.
///
/// REQ-P01-04 records that sysupdate targets a dual-slot A/B root transfer, so
/// the slot vocabulary has exactly two values and no "current" or "other":
/// a message that did not name a slot would be ambiguous by the time it
/// arrived.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Slot {
    /// The A root slot.
    A,
    /// The B root slot.
    B,
}

impl Slot {
    /// Both slots.
    pub const ALL: [Self; 2] = [Self::A, Self::B];

    /// Returns the slot name used in payloads and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
        }
    }

    /// Returns the other slot.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }
}

/// What the trigger asks P02 to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum PromotionAction {
    /// Deploy the candidate into the named slot.
    Deploy,
    /// Roll the named slot back.
    Rollback,
}

impl PromotionAction {
    /// Both actions.
    pub const ALL: [Self; 2] = [Self::Deploy, Self::Rollback];

    /// Returns the action name used in payloads and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Deploy => "deploy",
            Self::Rollback => "rollback",
        }
    }

    /// Returns the action a lifecycle stage justifies, if any.
    #[must_use]
    pub const fn for_stage(stage: Stage) -> Option<Self> {
        match stage {
            Stage::Publish => Some(Self::Deploy),
            Stage::Invalidate => Some(Self::Rollback),
            _ => None,
        }
    }
}

/// The contract versions of the promotion trigger this build admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum PromotionTriggerVersion {
    /// Version 1, tagged `aegis.p16-p02.promotion-trigger.v1`.
    #[serde(rename = "aegis.p16-p02.promotion-trigger.v1")]
    V1,
}

/// The stage a trigger reports, as it travels on the wire.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum TriggerStage {
    /// The candidate cleared the Pareto gate.
    Publish,
    /// The candidate was withdrawn.
    Invalidate,
}

impl TriggerStage {
    /// Both stages a trigger may report.
    pub const ALL: [Self; 2] = [Self::Publish, Self::Invalidate];

    /// Returns the stage name used in payloads and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Publish => "publish",
            Self::Invalidate => "invalidate",
        }
    }

    /// Returns the lifecycle stage this reports.
    #[must_use]
    pub const fn as_stage(self) -> Stage {
        match self {
            Self::Publish => Stage::Publish,
            Self::Invalidate => Stage::Invalidate,
        }
    }

    /// Returns the wire stage for a lifecycle stage, when one exists.
    ///
    /// The five stages before Publish have no wire form, because none of them
    /// justifies an update action: a trigger is sent when the lifecycle has
    /// decided, not while it is deciding.
    #[must_use]
    pub const fn from_stage(stage: Stage) -> Option<Self> {
        match stage {
            Stage::Publish => Some(Self::Publish),
            Stage::Invalidate => Some(Self::Invalidate),
            _ => None,
        }
    }
}

/// One instruction from P16 Athena to P02 Janus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct PromotionTrigger {
    /// The contract version this payload claims.
    pub schema: PromotionTriggerVersion,
    /// The subsystem-graph edge the payload travels on.
    pub edge: EdgeId,
    /// The identifier threading this trigger to the evaluation behind it.
    pub correlation_id: CorrelationId,
    /// The candidate the trigger is about.
    pub candidate: CandidateId,
    /// The stage the candidate reached.
    pub stage: TriggerStage,
    /// What P02 is asked to do.
    pub action: PromotionAction,
    /// The root slot the action targets.
    pub slot: Slot,
    /// How many of the four Pareto objectives the candidate cleared.
    pub cleared: u8,
}

impl PromotionTrigger {
    /// The contract this type instantiates.
    pub const SCHEMA: SchemaId = SchemaId::PromotionTrigger;

    /// The edge the trigger travels on, kept from the graph of record.
    pub const EDGE: EdgeId = EdgeId::TriggerSysupdateRollback;

    /// Builds the trigger a decided lifecycle justifies.
    ///
    /// Returns `None` for a lifecycle that has not decided: the five stages
    /// before Publish justify no update action.
    #[must_use]
    pub fn from_lifecycle(
        lifecycle: &Lifecycle,
        correlation_id: CorrelationId,
        candidate: CandidateId,
        slot: Slot,
    ) -> Option<Self> {
        let stage = TriggerStage::from_stage(lifecycle.stage())?;
        let action = PromotionAction::for_stage(lifecycle.stage())?;
        let cleared = lifecycle
            .verdict()
            .map_or(0, |verdict| verdict.cleared_count());
        Some(Self {
            schema: PromotionTriggerVersion::V1,
            edge: Self::EDGE,
            correlation_id,
            candidate,
            stage,
            action,
            slot,
            cleared: u8::try_from(cleared).unwrap_or(u8::MAX),
        })
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
    /// Returns [`ContractError::WrongEdge`] for a payload naming another edge
    /// and [`ContractError::ActionDoesNotFollow`] when the action is not the
    /// one the reported stage justifies.
    pub fn validate(&self) -> Result<(), ContractError> {
        let correlation = self.correlation();
        if self.edge != Self::EDGE {
            return Err(ContractError::WrongEdge {
                correlation,
                found: self.edge.name(),
            });
        }
        if PromotionAction::for_stage(self.stage.as_stage()) != Some(self.action) {
            return Err(ContractError::ActionDoesNotFollow {
                correlation,
                action: self.action.name(),
                stage: self.stage.name(),
            });
        }
        Ok(())
    }

    /// Encodes a validated trigger into `buffer`.
    ///
    /// # Errors
    ///
    /// Propagates [`Self::validate`], and returns
    /// [`ContractError::PayloadTooLong`] when the payload does not fit.
    pub fn encode_into<'b>(&self, buffer: &'b mut PayloadBuffer) -> Result<&'b str, ContractError> {
        self.validate()?;
        encode_into(self, self.correlation(), buffer)
    }

    /// Decodes and validates one trigger payload.
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
