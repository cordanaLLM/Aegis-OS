// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two edges P07 stands on that another crate owns (REQ-P08-05,
//! REQ-P13-03).
//!
//! Milestone M14 set the rule this module follows: a crate that sends a
//! message another subsystem consumes **builds that subsystem's type** rather
//! than declaring a second shape for the same crossing, and a crate that
//! receives a message another subsystem produced **decodes it with the
//! producer's own decoder**. Both halves appear here.
//!
//! | Direction | Type | Owner |
//! | :-- | :-- | :-- |
//! | P07 to P08, `ENFORCE_REALTIME_RTPRIO` | [`RealtimeGrant`] | `aegis-calliope`, the consumer |
//! | P13 to P07, `SPATIOTEMPORAL_TASK_SHIFT` | [`TaskShiftDirective`] | `aegis-tellus`, the producer, typed at milestone M05 |
//!
//! The grant's priority bound, its scheduling policy and its edge are read
//! from P08's own constants; the directive's threshold, verdict rule and edge
//! are P13's, and this crate does not restate any of them.
//!
//! # What the consumer side adds
//!
//! P13's decoder already refuses a directive whose verdict does not follow
//! from its own intensity. What it cannot refuse is a slice name P07 cannot
//! act on: P13's [`SliceName`](aegis_tellus::SliceName) admits any bounded
//! identifier, and a control group must end in `.slice`. [`TaskShiftAction`]
//! is therefore reached through this crate's own [`SliceName`],
//! and a directive naming `background.service` is accepted by P13 and refused
//! here. That is the consumer validating more strictly than the producer, not
//! a disagreement between them.
//!
//! # What this module does not do
//!
//! **Nothing is sent, received, deferred or granted.** No transport is opened
//! in either direction; building a payload is not sending it, and decoding one
//! is not obeying it.

use aegis_calliope::{
    CorrelationId as GrantCorrelationId, GrantedRtPrio, Label as GrantLabel, RealtimeGrant,
    RealtimeGrantVersion,
};
use aegis_tellus::TaskShiftDirective;

use crate::error::LictorError;
use crate::id::{IdError, SliceName};

/// What P07 does with one decoded task-shift directive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum TaskShiftAction {
    /// Background work on the slice is to be deferred.
    Defer {
        /// The slice the directive named, re-validated as a control group.
        slice: SliceName,
    },
    /// Background work on the slice may run.
    Resume {
        /// The slice the directive named, re-validated as a control group.
        slice: SliceName,
    },
}

impl TaskShiftAction {
    /// Returns the slice the action is about.
    #[must_use]
    pub const fn slice(self) -> SliceName {
        match self {
            Self::Defer { slice } | Self::Resume { slice } => slice,
        }
    }

    /// Returns `true` when the action defers background work.
    #[must_use]
    pub const fn defers(self) -> bool {
        matches!(self, Self::Defer { .. })
    }

    /// Returns the stable name this action is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Defer { .. } => "defer",
            Self::Resume { .. } => "resume",
        }
    }
}

/// Reads one directive P13 Tellus produced, as P07 Lictor would act on it.
///
/// The directive itself is P13's type, already validated by P13's own rules.
/// What this adds is the one check P07 needs and P13 does not make: the slice
/// must name a control group.
///
/// # Errors
///
/// Returns [`LictorError::Identifier`] carrying [`IdError::NotASlice`] when
/// the directive names something that is not a control-group slice, and the
/// other [`IdError`] variants when the name is empty, over-long or outside the
/// permitted character set.
pub fn act_on_task_shift(directive: &TaskShiftDirective) -> Result<TaskShiftAction, LictorError> {
    let mut rendered = [0u8; crate::id::MAX_SLICE_LEN];
    let source = directive.slice.as_bytes();
    let room = rendered.get_mut(..source.len()).ok_or(IdError::TooLong {
        max: crate::id::MAX_SLICE_LEN,
        actual: source.len(),
    })?;
    room.copy_from_slice(source);
    let text = core::str::from_utf8(room).map_err(|_| IdError::Charset)?;
    let slice = SliceName::parse(text)?;
    if directive.defer {
        Ok(TaskShiftAction::Defer { slice })
    } else {
        Ok(TaskShiftAction::Resume { slice })
    }
}

/// Everything P07 needs in order to build P08's grant.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GrantDraft {
    /// The identifier that correlates this crossing.
    pub correlation_id: GrantCorrelationId,
    /// The thread group the grant is about.
    pub thread: GrantLabel,
    /// The real-time priority to reserve.
    pub rtprio: GrantedRtPrio,
}

impl GrantDraft {
    /// Builds P08's grant from this draft.
    ///
    /// The edge and the scheduling policy come from P08's own constants,
    /// [`RealtimeGrant::EDGE`] and [`RealtimeGrant::POLICY`], so a change on
    /// the consumer's side reaches this builder rather than being duplicated
    /// here.
    #[must_use]
    pub const fn into_grant(self) -> RealtimeGrant {
        RealtimeGrant {
            schema: RealtimeGrantVersion::V1,
            edge: RealtimeGrant::EDGE,
            correlation_id: self.correlation_id,
            thread: self.thread,
            policy: RealtimeGrant::POLICY,
            rtprio: self.rtprio,
        }
    }
}
