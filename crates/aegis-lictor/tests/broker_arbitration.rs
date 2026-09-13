// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-3, the broker half: the table bound and focus arbitration.
//!
//! Positive: the scaffold's own two processes register, and a focus switch to
//! the compositor throttles the one background agent. **Negative: a focus
//! switch to a process the broker never registered throttles nothing**, which
//! is the case the scaffold silently accepts. Boundary: the table is exact at
//! its bound, and the process-identifier range is exact at both ends.

mod common;

use aegis_lictor::{
    EBPF_TASK_MAP_ENTRIES, FocusOutcome, LictorError, MAX_PID, MAX_TRACKED_PROCESSES, Pid,
    ProcessRecord, ProcessTier, ResourceBroker, Tier,
};

use common::{AGENT_PID, COMPOSITOR_PID, Fallible, STRANGER_PID, filled_broker, label, pid};

// --- Positive -------------------------------------------------------------

/// Positive: the scaffold's two processes register and read back.
#[test]
fn the_scaffold_processes_register_and_read_back() -> Fallible {
    let broker = common::scaffold_broker()?;
    assert_eq!(broker.count(), 2);
    assert!(!broker.is_empty());
    let row: ProcessRecord = broker
        .get(pid(COMPOSITOR_PID)?)
        .ok_or("the broker must hold the compositor it registered")?;
    let fields = (
        row.pid,
        row.name.to_string(),
        row.tier,
        row.declared_vram_mib,
        row.focused,
    );
    assert_eq!(
        fields,
        (
            pid(COMPOSITOR_PID)?,
            common::COMPOSITOR.to_owned(),
            ProcessTier::T0WaylandCompositor,
            256,
            false,
        )
    );
    assert_eq!(row.burst.samples(), 0);
    Ok(())
}

/// Positive: a focus switch to the compositor throttles the background agent.
#[test]
fn a_focus_switch_throttles_the_background_agent() -> Fallible {
    let mut broker = common::scaffold_broker()?;
    assert_eq!(broker.active(), None);
    assert_eq!(broker.throttleable(), 1);
    let compositor = pid(COMPOSITOR_PID)?;
    let outcome = broker.on_focus_change(compositor);
    assert_eq!(
        outcome,
        FocusOutcome::Switched {
            pid: compositor,
            throttled: 1,
        }
    );
    assert_eq!(outcome.throttled(), 1);
    assert!(outcome.moved_focus());
    assert_eq!(broker.active(), Some(compositor));
    Ok(())
}

/// Positive: a second switch to the same process is reported as a no-op
/// rather than as a fresh arbitration.
#[test]
fn a_repeated_focus_switch_is_a_no_op() -> Fallible {
    let mut broker = common::scaffold_broker()?;
    let compositor = pid(COMPOSITOR_PID)?;
    broker.on_focus_change(compositor);
    let repeat = broker.on_focus_change(compositor);
    assert_eq!(repeat, FocusOutcome::AlreadyFocused { pid: compositor });
    assert_eq!(repeat.throttled(), 0);
    assert!(!repeat.moved_focus());
    Ok(())
}

/// Positive: a burst folded into a tracked process classifies it, and the
/// broker's own three-way class maps onto a burst tier.
#[test]
fn a_burst_folded_into_a_tracked_process_classifies_it() -> Fallible {
    let mut broker = common::scaffold_broker()?;
    let agent = pid(AGENT_PID)?;
    assert_eq!(broker.observe_burst(agent, 12_000_000)?, Tier::Bulk);
    // (12_000_000 * 3 + 0) / 4 = 9_000_000, still above the 8 ms edge.
    assert_eq!(broker.observe_burst(agent, 0)?, Tier::Bulk);
    // (9_000_000 * 3 + 0) / 4 = 6_750_000, which is below it.
    assert_eq!(broker.observe_burst(agent, 0)?, Tier::Frame);
    assert_eq!(
        ProcessTier::T0WaylandCompositor.burst_tier(),
        Tier::Critical
    );
    assert_eq!(
        ProcessTier::T1InteractiveApp.burst_tier(),
        Tier::Interactive
    );
    assert_eq!(ProcessTier::T2BackgroundAgent.burst_tier(), Tier::Bulk);
    Ok(())
}

/// Positive: only the background class is throttleable, and each class carries
/// a distinct wire tag.
#[test]
fn only_the_background_class_is_throttleable() {
    assert!(ProcessTier::T2BackgroundAgent.is_throttleable());
    assert!(!ProcessTier::T0WaylandCompositor.is_throttleable());
    assert!(!ProcessTier::T1InteractiveApp.is_throttleable());
    let mut tags: Vec<&str> = ProcessTier::ALL.iter().map(|t| t.tag()).collect();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), 3);
    assert_eq!(
        ProcessTier::T0WaylandCompositor.tag(),
        "t0-wayland-compositor"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: a focus switch to an unregistered process throttles nothing.
///
/// This is the case the scaffold cannot express: it sets its active process
/// identifier to whatever it is handed and then sweeps a table with no
/// matching row, so an unknown identifier silently becomes the active one and
/// every background agent is throttled on its behalf.
#[test]
fn an_unregistered_focus_process_throttles_nothing() -> Fallible {
    let mut broker = common::scaffold_broker()?;
    let stranger = pid(STRANGER_PID)?;
    assert!(!broker.holds(stranger));
    let outcome = broker.on_focus_change(stranger);
    assert_eq!(outcome, FocusOutcome::UnknownPid { pid: stranger });
    assert_eq!(outcome.throttled(), 0);
    assert!(!outcome.moved_focus());
    assert_eq!(broker.active(), None);
    assert_eq!(broker.throttleable(), 1);
    Ok(())
}

/// Negative: an unregistered process has no burst average to fold into either.
#[test]
fn an_unregistered_process_has_no_burst_average() -> Fallible {
    let mut broker = common::scaffold_broker()?;
    assert_eq!(
        broker.observe_burst(pid(STRANGER_PID)?, 1_000),
        Err(LictorError::UnknownPid { pid: STRANGER_PID })
    );
    assert_eq!(broker.get(pid(STRANGER_PID)?), None);
    Ok(())
}

/// Negative: the table refuses past its bound and stores nothing.
#[test]
fn the_table_refuses_past_its_bound() -> Fallible {
    let mut broker = filled_broker(MAX_TRACKED_PROCESSES)?;
    let raw = u32::try_from(MAX_TRACKED_PROCESSES)
        .unwrap_or(u32::MAX)
        .saturating_add(1);
    let refusal = broker.register(
        pid(raw)?,
        label(common::AGENT)?,
        ProcessTier::T2BackgroundAgent,
        0,
    );
    assert_eq!(
        refusal,
        Err(LictorError::ProcessTableFull {
            max: MAX_TRACKED_PROCESSES
        })
    );
    assert_eq!(broker.count(), MAX_TRACKED_PROCESSES);
    Ok(())
}

/// Negative: an empty broker has nothing to focus and nothing to throttle.
#[test]
fn an_empty_broker_holds_nothing() -> Fallible {
    let mut broker = ResourceBroker::default();
    assert!(broker.is_empty());
    assert_eq!(broker.count(), 0);
    assert_eq!(broker.throttleable(), 0);
    let target = pid(COMPOSITOR_PID)?;
    assert_eq!(
        broker.on_focus_change(target),
        FocusOutcome::UnknownPid { pid: target }
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the table is exact at its bound.
#[test]
fn the_table_is_exact_at_its_bound() -> Fallible {
    let mut broker = filled_broker(MAX_TRACKED_PROCESSES.saturating_sub(1))?;
    assert_eq!(broker.count(), 127);
    let raw = u32::try_from(MAX_TRACKED_PROCESSES).unwrap_or(u32::MAX);
    broker.register(
        pid(raw)?,
        label(common::AGENT)?,
        ProcessTier::T2BackgroundAgent,
        0,
    )?;
    assert_eq!(broker.count(), MAX_TRACKED_PROCESSES);
    assert_eq!(MAX_TRACKED_PROCESSES, 128);
    Ok(())
}

/// Boundary: the process-identifier range is exact at both ends.
///
/// Process zero is the kernel's own swapper and never a focus target, so it
/// never becomes a value; the ceiling is the kernel's default `pid_max`.
#[test]
fn the_process_identifier_range_is_exact() {
    assert_eq!(
        Pid::new(0),
        Err(LictorError::PidOutOfRange {
            pid: 0,
            max: MAX_PID
        })
    );
    assert_eq!(Pid::new(1).map(Pid::get), Ok(1));
    assert_eq!(Pid::new(MAX_PID).map(Pid::get), Ok(MAX_PID));
    assert_eq!(
        Pid::new(MAX_PID.saturating_add(1)),
        Err(LictorError::PidOutOfRange {
            pid: MAX_PID.saturating_add(1),
            max: MAX_PID,
        })
    );
    assert_eq!(MAX_PID, 4_194_304);
}

/// Boundary: the user-space table bound and the recorded kernel map capacity
/// are different numbers, and are recorded apart so neither is read as the
/// other.
#[test]
fn the_user_space_bound_is_not_the_kernel_map_capacity() {
    assert_eq!(EBPF_TASK_MAP_ENTRIES, 65_536);
    assert_ne!(
        u64::from(EBPF_TASK_MAP_ENTRIES),
        MAX_TRACKED_PROCESSES as u64
    );
}
