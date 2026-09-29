// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E16-1: the D96 process lifecycle, driven by stubbed compositor, Justitia
//! and Tellus inputs.
//!
//! Positive: each forward transition in the source order, and both recovery
//! edges. Negative: every transition out of Deleted, and a transition the
//! table does not list. Boundary: the third consecutive over-budget window
//! quarantines and the second does not; a window within budget between two
//! over-budget ones restarts the count.

mod common;

use aegis_forum_shell::lifecycle::{
    Lifecycle, LifecycleError, ProcessState, QUARANTINE_LIMIT, WindowVerdict,
};
use aegis_forum_shell::processes::{
    FACILITY_BUDGET_WATTS, Input, MAX_PROCESSES, ProcessError, ProcessId, ProcessTable,
    facility_verdict,
};
use aegis_tellus::SliceName;

use common::{Fallible, telemetry};

use ProcessState::{Activated, Deleted, EligibleButInactive, Quarantined, RateLimited};
use WindowVerdict::{OverBudget, WithinBudget};

/// D96's table as the decision records it, written out independently of the
/// crate's own table: (from, to).
const D96_EDGES: [(ProcessState, ProcessState); 9] = [
    (EligibleButInactive, Activated),
    (Activated, RateLimited),
    (RateLimited, Quarantined),
    (Quarantined, Deleted),
    (RateLimited, Activated),
    (Quarantined, EligibleButInactive),
    (EligibleButInactive, Deleted),
    (Activated, Deleted),
    (RateLimited, Deleted),
];

/// A lifecycle driven to `state` along D96's edges.
fn at(state: ProcessState) -> Fallible<Lifecycle> {
    let mut life = Lifecycle::new();
    let path: &[ProcessState] = match state {
        EligibleButInactive => &[],
        Activated => &[Activated],
        RateLimited => &[Activated, RateLimited],
        Quarantined => &[Activated, RateLimited, Quarantined],
        Deleted => &[Deleted],
    };
    for step in path {
        life.transition(*step)?;
    }
    Ok(life)
}

/// A managed process table with process 7 in `build.slice`.
fn table() -> Fallible<ProcessTable> {
    let mut table = ProcessTable::new();
    table.admit(ProcessId(7), SliceName::parse("build.slice")?)?;
    Ok(table)
}

// --- Positive -------------------------------------------------------------

/// Positive: the forward chain in the source's order, one transition at a
/// time.
#[test]
fn the_forward_chain_runs_in_the_source_order() -> Fallible {
    let mut life = Lifecycle::new();
    assert_eq!(life.state(), EligibleButInactive);
    for next in [Activated, RateLimited, Quarantined, Deleted] {
        assert_eq!(life.transition(next)?, next);
        assert_eq!(life.state(), next);
    }
    assert_eq!(
        ProcessState::ALL,
        [
            EligibleButInactive,
            Activated,
            RateLimited,
            Quarantined,
            Deleted
        ]
    );
    Ok(())
}

/// Positive: both recovery edges succeed.
#[test]
fn both_recovery_edges_succeed() -> Fallible {
    let mut life = at(RateLimited)?;
    assert_eq!(life.transition(Activated)?, Activated);
    let mut life = at(Quarantined)?;
    assert_eq!(life.transition(EligibleButInactive)?, EligibleButInactive);
    assert_eq!(life.transition(Activated)?, Activated);
    Ok(())
}

/// Positive: every live state may go to Deleted.
#[test]
fn every_live_state_may_be_deleted() -> Fallible {
    for state in [EligibleButInactive, Activated, RateLimited, Quarantined] {
        let mut life = at(state)?;
        assert_eq!(life.transition(Deleted)?, Deleted, "from {state:?}");
    }
    Ok(())
}

/// Positive: the stubbed producers drive a process into quarantine: P04
/// maps its surface, then P13 reports three over-budget windows.
#[test]
fn stubbed_producers_drive_a_process_into_quarantine() -> Fallible {
    let mut table = table()?;
    let id = ProcessId(7);
    assert_eq!(
        table.active_capacity(),
        0,
        "an eligible process costs nothing"
    );
    assert_eq!(table.apply(Input::SurfaceMapped(id))?, Activated);
    assert_eq!(table.active_capacity(), 1);
    assert_eq!(table.apply(Input::Window(id, OverBudget))?, RateLimited);
    assert_eq!(table.apply(Input::Window(id, OverBudget))?, RateLimited);
    assert_eq!(table.apply(Input::Window(id, OverBudget))?, Quarantined);
    assert_eq!(
        table.active_capacity(),
        0,
        "a quarantined process holds no capacity"
    );
    Ok(())
}

/// Positive: P06 releases a quarantined process, P04 maps it again, the
/// focus slice travels with it (REQ-P04-07), and P04 reports its exit.
#[test]
fn stubbed_producers_release_refocus_and_end_a_process() -> Fallible {
    let mut table = table()?;
    let id = ProcessId(7);
    table.apply(Input::SurfaceMapped(id))?;
    for _ in 0..QUARANTINE_LIMIT {
        table.apply(Input::Window(id, OverBudget))?;
    }
    assert_eq!(table.apply(Input::Released(id))?, EligibleButInactive);
    assert_eq!(table.apply(Input::SurfaceMapped(id))?, Activated);
    let slice = table.focus_slice(id).map(|slice| slice.to_string());
    assert_eq!(slice.as_deref(), Some("build.slice"));
    assert_eq!(table.apply(Input::Exited(id))?, Deleted);
    assert_eq!(
        table.focus_slice(id),
        None,
        "a deleted process gains no focus"
    );
    Ok(())
}

/// Positive: P06's delete decision takes a quarantined process to Deleted.
#[test]
fn a_delete_decision_deletes_a_quarantined_process() -> Fallible {
    let mut table = table()?;
    let id = ProcessId(7);
    table.apply(Input::SurfaceMapped(id))?;
    for _ in 0..QUARANTINE_LIMIT {
        table.apply(Input::Window(id, OverBudget))?;
    }
    assert_eq!(table.state(id), Some(Quarantined));
    assert_eq!(table.apply(Input::DeleteApproved(id))?, Deleted);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: nothing leaves Deleted, whether asked for directly or by a
/// window.
#[test]
fn every_transition_out_of_deleted_is_refused() -> Fallible {
    for next in ProcessState::ALL {
        let mut life = at(Deleted)?;
        assert_eq!(
            life.transition(next),
            Err(LifecycleError::Terminal),
            "to {next:?}"
        );
        assert_eq!(life.state(), Deleted);
    }
    let mut life = at(Deleted)?;
    assert_eq!(life.observe(OverBudget), Err(LifecycleError::Terminal));
    assert_eq!(life.observe(WithinBudget), Err(LifecycleError::Terminal));
    Ok(())
}

/// Negative: Eligible but Inactive to Quarantined is not in the table.
#[test]
fn a_transition_the_table_does_not_list_is_refused() -> Fallible {
    let mut life = Lifecycle::new();
    assert_eq!(
        life.transition(Quarantined),
        Err(LifecycleError::NotAdmitted {
            from: EligibleButInactive,
            to: Quarantined
        })
    );
    assert_eq!(
        life.state(),
        EligibleButInactive,
        "a refusal changes nothing"
    );
    let mut idle = at(Activated)?;
    assert_eq!(
        idle.transition(EligibleButInactive),
        Err(LifecycleError::NotAdmitted {
            from: Activated,
            to: EligibleButInactive
        }),
        "no idle return from Activated (D96)"
    );
    Ok(())
}

/// Negative: the crate admits exactly D96's nine edges out of the 25 pairs.
#[test]
fn the_table_is_exactly_d96s() {
    for from in ProcessState::ALL {
        for to in ProcessState::ALL {
            let listed = D96_EDGES.contains(&(from, to));
            assert_eq!(from.admits(to), listed, "{from:?} -> {to:?}");
        }
    }
}

/// Negative: a window for a process holding no capacity, and any input for
/// an unmanaged process, are refused.
#[test]
fn a_window_without_capacity_and_an_unknown_process_are_refused() -> Fallible {
    let mut table = table()?;
    assert_eq!(
        table.apply(Input::Window(ProcessId(7), OverBudget)),
        Err(ProcessError::Lifecycle {
            id: 7,
            error: LifecycleError::NotRunning {
                state: EligibleButInactive
            }
        })
    );
    assert_eq!(
        table.apply(Input::SurfaceMapped(ProcessId(8))),
        Err(ProcessError::Unknown(8))
    );
    assert_eq!(
        table.admit(ProcessId(7), SliceName::parse("other.slice")?),
        Err(ProcessError::Duplicate(7))
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the limit is the named constant 3; the second over-budget window
/// leaves the process rate-limited and the third quarantines it.
#[test]
fn the_third_consecutive_window_quarantines_and_the_second_does_not() -> Fallible {
    assert_eq!(QUARANTINE_LIMIT, 3);
    let mut life = at(Activated)?;
    assert_eq!(life.observe(OverBudget)?, RateLimited);
    assert_eq!(life.over_budget_windows(), 1);
    assert_eq!(life.observe(OverBudget)?, RateLimited, "the second window");
    assert_eq!(life.over_budget_windows(), 2);
    assert_eq!(life.observe(OverBudget)?, Quarantined, "the third window");
    assert_eq!(life.over_budget_windows(), 0);
    Ok(())
}

/// Boundary: a window within budget between two over-budget ones returns the
/// process to Activated and restarts the count, so two more over-budget
/// windows do not quarantine it and a third does.
#[test]
fn a_window_within_budget_restarts_the_count() -> Fallible {
    let mut life = at(Activated)?;
    life.observe(OverBudget)?;
    life.observe(OverBudget)?;
    assert_eq!(life.observe(WithinBudget)?, Activated);
    assert_eq!(life.over_budget_windows(), 0);
    assert_eq!(life.observe(OverBudget)?, RateLimited);
    assert_eq!(life.observe(OverBudget)?, RateLimited);
    assert_eq!(life.observe(OverBudget)?, Quarantined);
    assert_eq!(
        life.observe(WithinBudget),
        Err(LifecycleError::NotRunning { state: Quarantined })
    );
    Ok(())
}

/// Boundary: a process within budget stays Activated, window after window.
#[test]
fn windows_within_budget_leave_an_activated_process_alone() -> Fallible {
    let mut life = at(Activated)?;
    for _ in 0..QUARANTINE_LIMIT.saturating_mul(2) {
        assert_eq!(life.observe(WithinBudget)?, Activated);
    }
    Ok(())
}

/// Boundary: the facility verdict P13's telemetry carries is within budget at
/// exactly 20 W and over it just above; the table is full at its bound.
#[test]
fn the_facility_budget_and_the_table_bound_hold_at_their_edges() -> Fallible {
    assert!((FACILITY_BUDGET_WATTS - 20.0).abs() < f64::EPSILON);
    assert_eq!(facility_verdict(&telemetry(20.0, 0.6)?), WithinBudget);
    assert_eq!(facility_verdict(&telemetry(20.000_001, 0.6)?), OverBudget);
    assert_eq!(facility_verdict(&telemetry(0.0, 0.05)?), WithinBudget);
    let mut table = ProcessTable::new();
    let slice = SliceName::parse("app.slice")?;
    for id in 0..u32::try_from(MAX_PROCESSES)? {
        table.admit(ProcessId(id), slice)?;
    }
    assert_eq!(
        table.admit(ProcessId(u32::MAX), slice),
        Err(ProcessError::Full)
    );
    Ok(())
}
