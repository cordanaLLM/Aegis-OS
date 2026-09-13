// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E15-1's boundary: expiry exactly at the timeout takes `Rollback`, and one
//! tick before does not.
//!
//! The two cases are only comparable if they share a history, so the boundary
//! tests fork one armed machine and drive the copies one tick apart. That is
//! what [`Machine`](aegis_janus_lifecycle::Machine) being `Copy` is for: the
//! two runs differ in the clock reading and in nothing else.
//!
//! The deadline the fixtures arm is
//! `[ARMED_AT, ARMED_AT + WINDOW)` = `[1000, 1600)`, so `DUE_AT` is 1600.
//! REQ-P02-08 is read fail-closed: a watchdog that treated its own deadline as
//! still running would extend every timeout by one tick.
//!
//! Fail-closed has a second half, which the boundary is only half-tested
//! without: the deadline is a property of the watchdog and not of the
//! [`Event::Tick`] that happens to sample it. Past `due_at` *every* offered
//! event rolls the candidate back, so the tests below drive the boundary with
//! `Bless` and with `Remeasure` and never offer a tick at all. A driver that
//! simply never sampled the clock could otherwise bless a candidate whose
//! watchdog expired long ago, and the D13 maintenance window has the same
//! bound for the same reason.

mod common;

use aegis_janus_lifecycle::{
    ClockError, Event, EventKind, LifecycleError, State, Tick, WatchdogStatus,
};

use common::{ARMED_AT, DUE_AT, Fallible, WINDOW, armed, blessed, refusal, signed, timeout};

// --- Positive -------------------------------------------------------------

/// Positive: a tick well inside the window keeps the candidate on trial, and
/// the stationary step is still recorded.
#[test]
fn a_tick_inside_the_window_keeps_the_candidate_on_trial() -> Fallible {
    let (mut machine, mut clock) = armed()?;
    clock.set(Tick::new(ARMED_AT.saturating_add(1)));
    let record = machine.step(&clock, Event::Tick)?;
    assert_eq!(machine.state(), State::WatchdogArmed);
    assert!(record.is_stationary());
    assert_eq!(record.from(), State::WatchdogArmed);
    assert_eq!(record.to(), State::WatchdogArmed);
    assert_eq!(machine.trace().len(), 6);
    Ok(())
}

/// Positive: the armed watchdog reports its own status at the three ticks the
/// boundary is defined by.
#[test]
fn the_watchdog_reports_running_before_its_deadline_and_expired_at_it() -> Fallible {
    let (machine, _clock) = armed()?;
    let watchdog = machine.watchdog().ok_or("the watchdog is armed here")?;
    assert_eq!(
        watchdog.status(Tick::new(DUE_AT.saturating_sub(1))),
        WatchdogStatus::Running
    );
    assert_eq!(watchdog.status(Tick::new(DUE_AT)), WatchdogStatus::Expired);
    assert_eq!(
        watchdog.status(Tick::new(DUE_AT.saturating_add(1))),
        WatchdogStatus::Expired
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a clock that steps backwards is refused, so a watchdog cannot be
/// wound back out of expiry.
#[test]
fn a_clock_that_steps_backwards_is_refused() -> Fallible {
    let (mut machine, mut clock) = armed()?;
    clock.set(Tick::new(DUE_AT.saturating_sub(1)));
    machine.step(&clock, Event::Tick)?;
    clock.set(Tick::new(ARMED_AT));
    let error = refusal(&mut machine, clock, Event::Tick)?;
    assert_eq!(
        error,
        LifecycleError::Clock(ClockError::NonMonotonic {
            last: DUE_AT.saturating_sub(1),
            observed: ARMED_AT,
        })
    );
    assert_eq!(machine.state(), State::WatchdogArmed);
    assert_eq!(machine.trace().len(), 6, "a refusal traces nothing");
    Ok(())
}

/// Negative: a rolled-back candidate is terminal; a later tick cannot revive it.
#[test]
fn a_rolled_back_candidate_is_terminal() -> Fallible {
    let (mut machine, mut clock) = armed()?;
    clock.set(Tick::new(DUE_AT));
    machine.step(&clock, Event::Tick)?;
    assert_eq!(machine.state(), State::RolledBack);
    for later in [Event::Tick, Event::Bless] {
        let error = refusal(&mut machine, clock, later)?;
        assert!(matches!(
            error,
            LifecycleError::Terminal {
                state: State::RolledBack,
                ..
            }
        ));
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: expiry exactly at the timeout takes `Rollback`, one tick before
/// does not, and one tick after does.
///
/// All three runs are forks of the same armed machine, so the only difference
/// between them is the tick the clock was set to.
#[test]
fn expiry_exactly_at_the_timeout_rolls_back_and_one_tick_before_does_not() -> Fallible {
    let (armed_machine, clock) = armed()?;

    let mut early = armed_machine;
    let mut early_clock = clock;
    early_clock.set(Tick::new(DUE_AT.saturating_sub(1)));
    let early_record = early.step(&early_clock, Event::Tick)?;
    assert_eq!(
        early.state(),
        State::WatchdogArmed,
        "one tick before the timeout must not roll back"
    );
    assert!(early_record.is_stationary());

    let mut exact = armed_machine;
    let mut exact_clock = clock;
    exact_clock.set(Tick::new(DUE_AT));
    let exact_record = exact.step(&exact_clock, Event::Tick)?;
    assert_eq!(
        exact.state(),
        State::RolledBack,
        "expiry exactly at the timeout must roll back"
    );
    assert_eq!(exact_record.from(), State::WatchdogArmed);
    assert_eq!(exact_record.to(), State::RolledBack);
    assert_eq!(exact_record.at(), Tick::new(DUE_AT));
    assert_eq!(exact.watchdog(), None, "a rollback disarms the watchdog");

    let mut late = armed_machine;
    let mut late_clock = clock;
    late_clock.set(Tick::new(DUE_AT.saturating_add(1)));
    late.step(&late_clock, Event::Tick)?;
    assert_eq!(late.state(), State::RolledBack);

    Ok(())
}

/// Boundary: the last tick at which a bless is still accepted is the one
/// immediately before the deadline; at the deadline the tick has already
/// rolled the candidate back.
#[test]
fn a_bless_one_tick_before_the_deadline_still_wins_the_race() -> Fallible {
    let (armed_machine, clock) = armed()?;

    let mut blessing = armed_machine;
    let mut blessing_clock = clock;
    blessing_clock.set(Tick::new(DUE_AT.saturating_sub(1)));
    blessing.step(&blessing_clock, Event::Bless)?;
    assert_eq!(blessing.state(), State::Blessed);

    let mut rolling = armed_machine;
    let mut rolling_clock = clock;
    rolling_clock.set(Tick::new(DUE_AT));
    rolling.step(&rolling_clock, Event::Tick)?;
    let error = refusal(&mut rolling, rolling_clock, Event::Bless)?;
    assert!(matches!(
        error,
        LifecycleError::Terminal {
            state: State::RolledBack,
            ..
        }
    ));
    Ok(())
}

/// Boundary: the armed deadline is evaluated before the event is dispatched,
/// so a bless at the deadline rolls the candidate back with no `Tick` ever
/// offered, and one tick before the deadline is still a bless.
///
/// This is the case a driver that samples the clock only when it chooses to
/// would otherwise hide. All three runs are forks of the same armed machine,
/// the same clock and the same event, so the only difference between them is
/// the tick the clock was set to.
#[test]
fn a_bless_at_the_deadline_rolls_back_with_no_tick_offered() -> Fallible {
    let (armed_machine, clock) = armed()?;

    let mut early = armed_machine;
    let mut early_clock = clock;
    early_clock.set(Tick::new(DUE_AT.saturating_sub(1)));
    early.step(&early_clock, Event::Bless)?;
    assert_eq!(
        early.state(),
        State::Blessed,
        "one tick before the deadline the bless still wins"
    );

    let mut exact = armed_machine;
    let mut exact_clock = clock;
    exact_clock.set(Tick::new(DUE_AT));
    let exact_record = exact.step(&exact_clock, Event::Bless)?;
    assert_eq!(
        exact.state(),
        State::RolledBack,
        "a bless exactly at the deadline must roll back, with no tick offered"
    );
    assert_eq!(
        exact_record.event(),
        EventKind::Bless,
        "the trace records the event that was offered, not a tick nobody sent"
    );
    assert_eq!(exact_record.from(), State::WatchdogArmed);
    assert_eq!(exact_record.to(), State::RolledBack);
    assert_eq!(exact_record.at(), Tick::new(DUE_AT));
    assert_eq!(exact.watchdog(), None, "a rollback disarms the watchdog");
    assert_eq!(exact.trace().len(), 6);

    let mut late = armed_machine;
    let mut late_clock = clock;
    late_clock.set(Tick::new(DUE_AT.saturating_add(1)));
    late.step(&late_clock, Event::Bless)?;
    assert_eq!(
        late.state(),
        State::RolledBack,
        "past the deadline a bless must roll back, with no tick offered"
    );
    Ok(())
}

/// Boundary: the D13 maintenance window is bounded at exactly the same place
/// and by the same reading, without a `Tick`.
///
/// The slot is reopened and remeasured *inside* the window, so the three forks
/// differ only in the tick their re-bless is offered at: a window that had
/// already closed cannot be re-blessed on a measurement taken while it was
/// open.
#[test]
fn a_re_bless_at_the_maintenance_deadline_rolls_back_with_no_tick_offered() -> Fallible {
    let (mut reopened, clock) = blessed()?;
    reopened.step(&clock, Event::Reopen(timeout(WINDOW)?))?;
    reopened.step(&clock, Event::Remeasure(signed()?))?;
    assert_eq!(reopened.state(), State::Reopened);
    let window = reopened.watchdog().ok_or("reopening arms the window")?;
    assert_eq!(
        window.due_at(),
        Tick::new(DUE_AT),
        "the window is armed at ARMED_AT and is WINDOW ticks wide"
    );

    let mut early = reopened;
    let mut early_clock = clock;
    early_clock.set(Tick::new(DUE_AT.saturating_sub(1)));
    early.step(&early_clock, Event::Bless)?;
    assert_eq!(
        early.state(),
        State::Blessed,
        "one tick before the window closes the re-bless still wins"
    );

    let mut exact = reopened;
    let mut exact_clock = clock;
    exact_clock.set(Tick::new(DUE_AT));
    let exact_record = exact.step(&exact_clock, Event::Bless)?;
    assert_eq!(
        exact.state(),
        State::RolledBack,
        "a re-bless exactly at the window's end must roll back"
    );
    assert_eq!(exact_record.event(), EventKind::Bless);
    assert_eq!(exact_record.from(), State::Reopened);
    assert_eq!(exact_record.to(), State::RolledBack);
    assert_eq!(exact.watchdog(), None);

    let mut late = reopened;
    let mut late_clock = clock;
    late_clock.set(Tick::new(DUE_AT.saturating_add(1)));
    late.step(&late_clock, Event::Bless)?;
    assert_eq!(late.state(), State::RolledBack);
    Ok(())
}

/// Negative: a remeasure cannot hold a closed maintenance window open.
///
/// Reporting a fresh measurement is the one event a reopened slot accepts
/// repeatedly, so it is the event a driver could use to sit inside a window
/// indefinitely. Past the bound it rolls back like every other event.
#[test]
fn a_remeasure_does_not_hold_a_closed_maintenance_window_open() -> Fallible {
    let (mut machine, mut clock) = blessed()?;
    machine.step(&clock, Event::Reopen(timeout(WINDOW)?))?;

    let mut inside = machine;
    let mut inside_clock = clock;
    inside_clock.set(Tick::new(DUE_AT.saturating_sub(1)));
    inside.step(&inside_clock, Event::Remeasure(signed()?))?;
    assert_eq!(inside.state(), State::Reopened);
    assert_eq!(inside.measured_root_hash(), Some(signed()?));

    clock.set(Tick::new(DUE_AT));
    let record = machine.step(&clock, Event::Remeasure(signed()?))?;
    assert_eq!(record.event(), EventKind::Remeasure);
    assert_eq!(record.to(), State::RolledBack);
    assert_eq!(machine.state(), State::RolledBack);
    assert_eq!(
        machine.measured_root_hash(),
        None,
        "the rollback does not adopt the measurement the late remeasure carried"
    );
    Ok(())
}

/// Negative: past the deadline the machine rolls back rather than refusing,
/// even for an event it would have refused inside the window.
///
/// The deadline outranks the event, so `Unexpected` is a refusal only while
/// the candidate is still on trial. Afterwards there is nothing to refuse on
/// behalf of: the watchdog has already decided.
#[test]
fn an_event_the_window_refuses_rolls_back_once_the_window_has_closed() -> Fallible {
    let (armed_machine, clock) = armed()?;

    let mut inside = armed_machine;
    let mut inside_clock = clock;
    inside_clock.set(Tick::new(DUE_AT.saturating_sub(1)));
    let error = refusal(&mut inside, inside_clock, Event::SwapSlot)?;
    assert_eq!(
        error,
        LifecycleError::Unexpected {
            state: State::WatchdogArmed,
            event: EventKind::SwapSlot,
        }
    );
    assert_eq!(inside.trace().len(), 5, "a refusal traces nothing");

    let mut outside = armed_machine;
    let mut outside_clock = clock;
    outside_clock.set(Tick::new(DUE_AT));
    let record = outside.step(&outside_clock, Event::SwapSlot)?;
    assert_eq!(record.event(), EventKind::SwapSlot);
    assert_eq!(record.to(), State::RolledBack);
    assert_eq!(outside.state(), State::RolledBack);
    Ok(())
}
