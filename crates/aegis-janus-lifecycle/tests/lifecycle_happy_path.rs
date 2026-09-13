// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E15-1's positive half: the happy path reaches `Bless`.
//!
//! The positive test walks the six stages milestone M15 names -- candidate,
//! signature check, delta acquisition, slot swap, watchdog, bless -- and
//! checks the state after each, so a path that reached `Blessed` by skipping
//! one of them would fail here rather than pass on its destination.

mod common;

use aegis_janus_lifecycle::{
    Event, EventKind, LifecycleError, MAX_TRANSITIONS, Machine, SignatureVerdict, Slot, State,
    SysupdateCall, Tick, Version,
};

use common::{
    ARMED_AT, DUE_AT, Fallible, RELEASE, WINDOW, armed, candidate, refusal, signed, timeout,
};

// --- Positive -------------------------------------------------------------

/// Positive: the happy path reaches `Bless`, one declared stage at a time.
#[test]
fn the_happy_path_reaches_bless() -> Fallible {
    let (machine, _clock) = common::blessed()?;
    assert_eq!(machine.state(), State::Blessed);
    assert_eq!(machine.watchdog(), None, "a blessed slot is off trial");
    assert_eq!(machine.measured_root_hash(), Some(signed()?));
    Ok(())
}

/// Positive: every stage is visited in order, and none is skippable.
#[test]
fn every_stage_is_visited_in_the_declared_order() -> Fallible {
    let (mut machine, clock) = armed()?;
    let reached: Vec<State> = machine
        .trace()
        .records()
        .iter()
        .map(|record| record.to())
        .collect();
    assert_eq!(
        reached,
        vec![
            State::CandidateDeclared,
            State::SignatureVerified,
            State::DeltaAcquired,
            State::SlotSwapped,
            State::WatchdogArmed,
        ]
    );
    machine.step(&clock, Event::Bless)?;
    assert_eq!(machine.state(), State::Blessed);
    Ok(())
}

/// Positive: the machine describes the stubbed call each stage needs, and the
/// sequence is the one a real transfer would have to make.
#[test]
fn the_machine_describes_the_call_each_stage_needs() -> Fallible {
    let version = Version::parse(RELEASE)?;
    let candidate = candidate()?;
    let clock = common::armed()?.1;
    let mut machine = Machine::new();
    assert_eq!(machine.pending_call(), Some(SysupdateCall::List));
    machine.step(&clock, Event::Declare(candidate))?;
    assert_eq!(machine.pending_call(), Some(SysupdateCall::Verify(version)));
    machine.step(&clock, Event::CheckSignature(SignatureVerdict::Valid))?;
    assert_eq!(machine.pending_call(), Some(SysupdateCall::Update(version)));
    machine.step(&clock, Event::AcquireDelta(signed()?))?;
    assert_eq!(
        machine.pending_call(),
        Some(SysupdateCall::SwapSlot(Slot::B))
    );
    machine.step(&clock, Event::SwapSlot)?;
    assert_eq!(
        machine.pending_call(),
        None,
        "arming is not a sysupdate call"
    );
    machine.step(&clock, Event::ArmWatchdog(timeout(WINDOW)?))?;
    machine.step(&clock, Event::Bless)?;
    assert_eq!(machine.pending_call(), Some(SysupdateCall::BlessBoot));
    Ok(())
}

/// Positive: the watchdog the happy path arms is the window that was asked for.
#[test]
fn the_armed_watchdog_spans_exactly_the_requested_window() -> Fallible {
    let (machine, _clock) = armed()?;
    let watchdog = machine
        .watchdog()
        .ok_or("the watchdog is armed on this path")?;
    assert_eq!(watchdog.armed_at(), Tick::new(ARMED_AT));
    assert_eq!(watchdog.due_at(), Tick::new(DUE_AT));
    assert_eq!(machine.observed_at(), Some(Tick::new(ARMED_AT)));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: no stage may be skipped, and a refused step changes nothing.
#[test]
fn a_stage_cannot_be_skipped() -> Fallible {
    let clock = common::armed()?.1;
    let mut machine = Machine::new();
    for premature in [
        Event::CheckSignature(SignatureVerdict::Valid),
        Event::AcquireDelta(signed()?),
        Event::SwapSlot,
        Event::ArmWatchdog(timeout(WINDOW)?),
        Event::Bless,
    ] {
        let error = refusal(&mut machine, clock, premature)?;
        assert!(matches!(
            error,
            LifecycleError::Unexpected {
                state: State::Idle,
                ..
            }
        ));
        assert_eq!(machine.state(), State::Idle);
        assert!(machine.trace().is_empty(), "a refusal traces nothing");
    }
    Ok(())
}

/// Negative: a candidate cannot be blessed before it is on trial, even once
/// its delta has been acquired and measured.
///
/// The measurement alone is not the gate: REQ-P02-08 puts the candidate under
/// a watchdog first, so a bless offered at `DeltaAcquired` or `SlotSwapped` is
/// refused as an event those states do not accept.
#[test]
fn a_candidate_cannot_be_blessed_before_it_is_on_trial() -> Fallible {
    let clock = common::armed()?.1;
    let mut machine = Machine::new();
    machine.step(&clock, Event::Declare(candidate()?))?;
    machine.step(&clock, Event::CheckSignature(SignatureVerdict::Valid))?;
    machine.step(&clock, Event::AcquireDelta(signed()?))?;
    assert_eq!(machine.measured_root_hash(), Some(signed()?));
    for stage in [State::DeltaAcquired, State::SlotSwapped] {
        assert_eq!(machine.state(), stage);
        let error = refusal(&mut machine, clock, Event::Bless)?;
        assert_eq!(
            error,
            LifecycleError::Unexpected {
                state: stage,
                event: EventKind::Bless,
            }
        );
        machine.step(&clock, Event::SwapSlot).ok();
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the trace holds exactly `MAX_TRANSITIONS` records, and the step
/// that would be the next one is refused rather than made unrecorded.
#[test]
fn the_trace_is_full_at_its_bound_and_the_next_step_is_refused() -> Fallible {
    let (mut machine, clock) = armed()?;
    let filled = machine.trace().len();
    assert_eq!(filled, 5);
    for _ in 0..MAX_TRANSITIONS.saturating_sub(filled) {
        let record = machine.step(&clock, Event::Tick)?;
        assert!(record.is_stationary());
        assert_eq!(record.event(), EventKind::Tick);
    }
    assert_eq!(machine.trace().len(), MAX_TRANSITIONS);
    let error = refusal(&mut machine, clock, Event::Tick)?;
    assert_eq!(
        error,
        LifecycleError::TraceFull {
            bound: MAX_TRANSITIONS
        }
    );
    assert_eq!(machine.trace().len(), MAX_TRANSITIONS);
    assert_eq!(machine.state(), State::WatchdogArmed);
    Ok(())
}

/// Boundary: the first and last recorded transitions carry sequence 0 and
/// `len - 1`, so the run is dense at both ends.
#[test]
fn sequence_numbers_run_from_zero_to_the_last_record() -> Fallible {
    let (machine, _clock) = common::blessed()?;
    let records = machine.trace().records();
    let first = records.first().ok_or("the trace is not empty here")?;
    let last = machine
        .trace()
        .last()
        .ok_or("the trace is not empty here")?;
    assert_eq!(first.seq(), 0);
    assert_eq!(first.from(), State::Idle);
    assert_eq!(u64::from(last.seq()), 5);
    assert_eq!(last.to(), State::Blessed);
    assert_eq!(records.len(), 6);
    Ok(())
}
