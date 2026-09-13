// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-5: the two edges P07 stands on that another crate owns.
//!
//! Positive: the real-time grant this crate builds is P08's own type and
//! round-trips through P08's own decoder, and a directive P13 produced is
//! consumed through P13's decoder and turned into an action. Negative: a draft
//! the consumer would refuse produces no payload, and a directive naming
//! something that is not a control-group slice is refused here even though P13
//! accepted it. Boundary: a grant at the source's priority is built and one
//! above it cannot be drafted at all.

mod common;

use aegis_calliope::{
    CorrelationId as GrantCorrelationId, GrantedRtPrio, Label as GrantLabel, PayloadBuffer,
    RealtimeGrant, RealtimeGrantVersion, SchedPolicy,
};
use aegis_lictor::{GrantDraft, IdError, LictorError, TaskShiftAction, act_on_task_shift};
use aegis_tellus::{
    CorrelationId as ShiftCorrelationId, GridIntensity, SliceName as ShiftSliceName,
    TaskShiftDirective, TaskShiftVersion,
};

use common::Fallible;

/// Builds a draft for the audio thread group at `rtprio`.
fn draft(rtprio: u8) -> Result<GrantDraft, Box<dyn std::error::Error>> {
    Ok(GrantDraft {
        correlation_id: GrantCorrelationId::parse(common::CORRELATION)?,
        thread: GrantLabel::parse("pipewire-rt-audio")?,
        rtprio: GrantedRtPrio::new(rtprio)?,
    })
}

/// Builds a directive as P13 would produce it, for `slice` at `intensity`.
fn directive(
    slice: &str,
    intensity: f64,
) -> Result<TaskShiftDirective, Box<dyn std::error::Error>> {
    Ok(TaskShiftDirective {
        schema: TaskShiftVersion::V1,
        edge: TaskShiftDirective::EDGE,
        correlation_id: ShiftCorrelationId::parse(common::CORRELATION)?,
        slice: ShiftSliceName::parse(slice)?,
        intensity: GridIntensity::new(intensity)?,
        defer: intensity > TaskShiftDirective::THRESHOLD,
    })
}

// --- Positive -------------------------------------------------------------

/// Positive: the grant this crate builds is P08's own type, and P08's own
/// decoder reads it back.
#[test]
fn the_grant_is_built_from_the_consumers_type() -> Fallible {
    let grant: RealtimeGrant = draft(95)?.into_grant();
    let mut buffer = PayloadBuffer::new();
    let text = grant.encode_into(&mut buffer)?.to_owned();
    let decoded = RealtimeGrant::decode(&text)?;
    assert_eq!(decoded, grant);
    assert_eq!(decoded.schema, RealtimeGrantVersion::V1);
    assert_eq!(decoded.edge, RealtimeGrant::EDGE);
    assert_eq!(decoded.policy, SchedPolicy::ADMITTED);
    assert!(decoded.is_source_priority());
    Ok(())
}

/// Positive: the edge and the policy come from P08's constants rather than
/// from a copy in this crate.
#[test]
fn the_grant_reads_the_consumers_constants() -> Fallible {
    let grant = draft(80)?.into_grant();
    assert_eq!(grant.edge, RealtimeGrant::EDGE);
    assert_eq!(grant.policy, RealtimeGrant::POLICY);
    assert_eq!(grant.rtprio.get(), 80);
    assert!(!grant.is_source_priority());
    Ok(())
}

/// Positive: a deferring directive becomes a deferring action, and a
/// non-deferring one does not.
#[test]
fn a_directive_becomes_the_action_it_states() -> Fallible {
    let deferring = act_on_task_shift(&directive(common::SLICE, 400.0)?)?;
    assert!(deferring.defers());
    assert_eq!(deferring.name(), "defer");
    assert_eq!(deferring.slice().to_string(), common::SLICE);
    assert_eq!(
        deferring,
        TaskShiftAction::Defer {
            slice: aegis_lictor::SliceName::parse(common::SLICE)?
        }
    );

    let resuming = act_on_task_shift(&directive(common::SLICE, 100.0)?)?;
    assert!(!resuming.defers());
    assert_eq!(resuming.name(), "resume");
    assert_eq!(
        resuming,
        TaskShiftAction::Resume {
            slice: aegis_lictor::SliceName::parse(common::SLICE)?
        }
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a directive P13 accepts is still refused here when it names
/// something that is not a control-group slice.
///
/// P13's own identifier admits any bounded name; a broker asked to re-allocate
/// `background-builds` has nothing to write to. The consumer validating more
/// strictly than the producer is the point of the consumer acting on it.
#[test]
fn a_directive_naming_a_non_slice_is_refused_here() -> Fallible {
    let produced = directive("background-builds", 400.0)?;
    // P13 itself is satisfied: the payload validates on the producer's rules.
    produced.validate()?;
    let refusal = act_on_task_shift(&produced);
    assert_eq!(refusal, Err(LictorError::Identifier(IdError::NotASlice)));
    Ok(())
}

/// Negative: a priority the consumer does not authorise cannot be drafted, so
/// no grant is built from one.
#[test]
fn a_priority_the_consumer_refuses_never_becomes_a_draft() {
    assert!(GrantedRtPrio::new(96).is_err());
    assert!(GrantedRtPrio::new(0).is_err());
    assert!(draft(96).is_err());
}

/// Negative: P13's own verdict rule is P13's, and this crate does not restate
/// it -- a directive whose verdict contradicts its intensity is refused by the
/// producer's validator before this crate sees it.
#[test]
fn the_producers_verdict_rule_stays_the_producers() -> Fallible {
    let mut contradictory = directive(common::SLICE, 100.0)?;
    contradictory.defer = true;
    assert!(contradictory.validate().is_err());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the draft is exact at the source priority.
#[test]
fn the_draft_is_exact_at_the_source_priority() -> Fallible {
    assert_eq!(draft(95)?.into_grant().rtprio, GrantedRtPrio::REQUIRED);
    assert!(draft(95).is_ok());
    assert!(draft(96).is_err());
    assert!(draft(1).is_ok());
    assert!(draft(0).is_err());
    Ok(())
}

/// Boundary: P13's defer threshold is exclusive, and this crate carries the
/// verdict across rather than recomputing it.
#[test]
fn the_threshold_is_carried_across_rather_than_recomputed() -> Fallible {
    let at_threshold = directive(common::SLICE, TaskShiftDirective::THRESHOLD)?;
    assert!(!at_threshold.defer);
    assert!(!act_on_task_shift(&at_threshold)?.defers());

    let just_above = directive(common::SLICE, TaskShiftDirective::THRESHOLD + 0.001)?;
    assert!(just_above.defer);
    assert!(act_on_task_shift(&just_above)?.defers());
    Ok(())
}
