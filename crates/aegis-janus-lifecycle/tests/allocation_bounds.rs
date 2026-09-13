// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that a transition allocates nothing, made falsifiable.
//!
//! The crate documentation says a transition allocates nothing. The reason it
//! can say that is structural: every value the machine holds is `Copy`, and
//! the trace is a fixed-size array rather than a growable one, so a step
//! writes into storage that already exists. The claim is therefore equivalent
//! to a property a test can hold: no heap-owning type may appear anywhere
//! inside [`Machine`].
//!
//! That is what this file checks. `assert_no_heap::<Machine>()` requires
//! `Machine: Copy`, and a `Vec`, `String` or `Box` anywhere inside it -- at any
//! depth -- makes the derive fail and this file stop compiling, which fails the
//! gate. The recorded size bound catches the other direction: a field that is
//! `Copy` but large enough to matter cannot be added unnoticed.
//!
//! # What is deliberately not claimed
//!
//! Rendering a trace to text allocates, and so does parsing one. Both are off
//! the lifecycle path and the documentation says so rather than claiming the
//! crate allocates nowhere. The size figures below are computed with
//! `size_of` on the building target, so this file, not the prose, is the
//! authority for the numbers.

mod common;

use core::mem::size_of;

use aegis_janus_lifecycle::{
    Candidate, Event, LifecycleError, MAX_TRANSITIONS, Machine, RootHash, SignatureVerdict, Slot,
    State, StubSysupdate, SysupdateCall, Tick, Trace, Transition, Version, Watchdog,
};

use common::{Fallible, armed, blessed};

/// The largest a machine may be before the size is worth a second look.
const MAX_MACHINE_BYTES: usize = 1024;

/// Accepts only a type that owns no heap, because it is `Copy`.
fn assert_no_heap<T: Copy>() {}

// --- Positive -------------------------------------------------------------

/// Positive: every value the lifecycle path touches owns no heap.
///
/// This is the falsifier. Adding a `Vec`, `String` or `Box` to any of these
/// types, or to anything they contain, removes `Copy` and this test stops
/// compiling.
#[test]
fn every_value_on_the_lifecycle_path_owns_no_heap() {
    assert_no_heap::<Machine>();
    assert_no_heap::<Trace>();
    assert_no_heap::<Transition>();
    assert_no_heap::<Candidate>();
    assert_no_heap::<Version>();
    assert_no_heap::<RootHash>();
    assert_no_heap::<Event>();
    assert_no_heap::<State>();
    assert_no_heap::<Slot>();
    assert_no_heap::<Tick>();
    assert_no_heap::<Watchdog>();
    assert_no_heap::<LifecycleError>();
    assert_no_heap::<SysupdateCall>();
    assert_no_heap::<StubSysupdate>();
    assert_no_heap::<SignatureVerdict>();
}

/// Positive: driving the whole happy path changes no capacity anywhere,
/// because there is no capacity to change: the machine is the same size after
/// as before, and a copy taken before the run is byte-identical in width.
#[test]
fn driving_the_lifecycle_changes_no_storage() -> Fallible {
    let before = size_of::<Machine>();
    let (machine, _clock) = blessed()?;
    assert_eq!(size_of_val(&machine), before);
    assert_eq!(machine.trace().records().len(), 6);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the trace refuses a record past its bound rather than growing.
///
/// A growable trace would satisfy the prose and break the invariant, so the
/// refusal is the evidence that the storage is fixed.
#[test]
fn the_trace_refuses_a_record_rather_than_growing() -> Fallible {
    let (mut machine, clock) = armed()?;
    for _ in 0..MAX_TRANSITIONS.saturating_sub(machine.trace().len()) {
        machine.step(&clock, Event::Tick)?;
    }
    let before = size_of_val(machine.trace());
    let error = common::refusal(&mut machine, clock, Event::Tick)?;
    assert_eq!(
        error,
        LifecycleError::TraceFull {
            bound: MAX_TRANSITIONS
        }
    );
    assert_eq!(size_of_val(machine.trace()), before);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the machine's width is dominated by the trace, and the trace is
/// exactly its bound of fixed-width records.
#[test]
fn the_machine_is_its_trace_plus_a_little() {
    let record = size_of::<Transition>();
    let trace = size_of::<Trace>();
    let machine = size_of::<Machine>();
    assert!(
        trace >= MAX_TRANSITIONS.saturating_mul(record),
        "the trace holds {MAX_TRANSITIONS} records of {record} bytes but measures {trace}"
    );
    assert!(
        trace < MAX_TRANSITIONS.saturating_add(8).saturating_mul(record),
        "the trace measures {trace}, more than its records account for"
    );
    assert!(
        machine <= MAX_MACHINE_BYTES,
        "the machine measures {machine} bytes, past the recorded bound of {MAX_MACHINE_BYTES}"
    );
    assert!(machine >= trace, "the machine contains its trace");
}

/// Boundary: a copy of the machine is a value, not a handle.
///
/// The boundary tests fork an armed machine and drive the copies one tick
/// apart. That is only sound if a copy shares nothing with its original, which
/// is what this checks: the copy that was driven to a terminal state leaves
/// the original exactly where it was.
#[test]
fn a_copied_machine_shares_nothing_with_its_original() -> Fallible {
    let (original, clock) = armed()?;
    let mut fork = original;
    fork.step(&clock, Event::Bless)?;
    assert_eq!(fork.state(), State::Blessed);
    assert_eq!(original.state(), State::WatchdogArmed);
    assert_eq!(original.trace().len(), 5);
    assert_eq!(fork.trace().len(), 6);
    Ok(())
}
