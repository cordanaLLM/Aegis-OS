// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E15-1's negative half: a signature failure reaches `Discard`.
//!
//! The signature gate is the first thing that can refuse a candidate, and it
//! refuses it before anything has been written: the tests here check the
//! destination *and* that no slot-writing call was ever described on the way
//! to it, so a machine that discarded after acquiring a delta would fail even
//! though it reached the same state.

mod common;

use aegis_janus_lifecycle::{
    Event, EventKind, LifecycleError, Machine, SignatureVerdict, State, SysupdateCall,
};

use common::{Fallible, WINDOW, armed, candidate, refusal, signed, timeout};

/// Returns a machine standing at [`State::CandidateDeclared`].
fn declared() -> Result<(Machine, aegis_janus_lifecycle::StubClock), Box<dyn std::error::Error>> {
    let clock = armed()?.1;
    let mut machine = Machine::new();
    machine.step(&clock, Event::Declare(candidate()?))?;
    Ok((machine, clock))
}

// --- Positive -------------------------------------------------------------

/// Positive: a signature failure reaches `Discard`, and the trace says why.
#[test]
fn a_signature_failure_reaches_discard() -> Fallible {
    let (mut machine, clock) = declared()?;
    let record = machine.step(&clock, Event::CheckSignature(SignatureVerdict::Invalid))?;
    assert_eq!(machine.state(), State::Discarded);
    assert_eq!(record.from(), State::CandidateDeclared);
    assert_eq!(record.to(), State::Discarded);
    assert_eq!(record.event(), EventKind::CheckSignature);
    Ok(())
}

/// Positive: a delta that cannot be acquired discards the candidate too.
#[test]
fn a_failed_delta_acquisition_reaches_discard() -> Fallible {
    let (mut machine, clock) = declared()?;
    machine.step(&clock, Event::CheckSignature(SignatureVerdict::Valid))?;
    machine.step(&clock, Event::FailDelta)?;
    assert_eq!(machine.state(), State::Discarded);
    assert_eq!(machine.measured_root_hash(), None);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a discarded candidate accepts nothing further, including a bless.
#[test]
fn a_discarded_candidate_is_terminal() -> Fallible {
    let (mut machine, clock) = declared()?;
    machine.step(&clock, Event::CheckSignature(SignatureVerdict::Invalid))?;
    for later in [
        Event::CheckSignature(SignatureVerdict::Valid),
        Event::AcquireDelta(signed()?),
        Event::SwapSlot,
        Event::ArmWatchdog(timeout(WINDOW)?),
        Event::Tick,
        Event::Bless,
        Event::Reopen(timeout(WINDOW)?),
        Event::Remeasure(signed()?),
    ] {
        let error = refusal(&mut machine, clock, later)?;
        assert!(matches!(
            error,
            LifecycleError::Terminal {
                state: State::Discarded,
                ..
            }
        ));
        assert_eq!(machine.state(), State::Discarded);
    }
    assert_eq!(machine.trace().len(), 2);
    Ok(())
}

/// Negative: a second candidate cannot be declared over a live lifecycle.
#[test]
fn a_second_candidate_cannot_be_declared() -> Fallible {
    let (mut machine, clock) = declared()?;
    let error = refusal(&mut machine, clock, Event::Declare(candidate()?))?;
    assert!(matches!(
        error,
        LifecycleError::Unexpected {
            state: State::CandidateDeclared,
            event: EventKind::Declare,
        }
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the discard happens at the signature gate, so no slot is written.
///
/// The machine describes the calls a real transfer would make. On this path it
/// never describes [`SysupdateCall::Update`], which is the call that writes a
/// slot, and it describes no boot assessment either.
#[test]
fn a_discarded_candidate_never_described_a_slot_write() -> Fallible {
    let (mut machine, clock) = declared()?;
    assert!(matches!(
        machine.pending_call(),
        Some(SysupdateCall::Verify(_))
    ));
    machine.step(&clock, Event::CheckSignature(SignatureVerdict::Invalid))?;
    assert_eq!(machine.pending_call(), None);
    for record in machine.trace().records() {
        assert_ne!(record.event(), EventKind::AcquireDelta);
        assert_ne!(record.event(), EventKind::SwapSlot);
    }
    Ok(())
}

/// Boundary: the last stage at which a signature verdict is still read is
/// `CandidateDeclared`; one stage later the same event is refused.
#[test]
fn a_signature_verdict_is_read_at_exactly_one_stage() -> Fallible {
    let (mut machine, clock) = declared()?;
    machine.step(&clock, Event::CheckSignature(SignatureVerdict::Valid))?;
    let error = refusal(
        &mut machine,
        clock,
        Event::CheckSignature(SignatureVerdict::Invalid),
    )?;
    assert!(matches!(
        error,
        LifecycleError::Unexpected {
            state: State::SignatureVerified,
            event: EventKind::CheckSignature,
        }
    ));
    assert_eq!(machine.state(), State::SignatureVerified);
    Ok(())
}
