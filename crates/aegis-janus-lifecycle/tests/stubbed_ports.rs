// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The stubbed systemd boundary, driven end to end.
//!
//! These tests wire the stub to the machine the way a real driver would: the
//! machine describes the call its state needs, the port answers, and the
//! answer becomes the next event. That is the whole of the "systemd and
//! sysupdate calls are stubbed" exit criterion in one loop -- the shape is
//! real, the effects are not.

mod common;

use aegis_janus_lifecycle::{
    CallKind, CallOutcome, Event, MAX_RECORDED_CALLS, Machine, PortError, SignatureVerdict, Slot,
    State, StubSysupdate, SysupdateCall, SysupdatePort, Version,
};

use common::{Fallible, RELEASE, WINDOW, armed, candidate, signed, timeout};

// --- Positive -------------------------------------------------------------

/// Positive: the stub answers every call the lifecycle describes, in order,
/// and the answers drive the machine to `Blessed`.
#[test]
fn the_stub_drives_the_lifecycle_to_bless() -> Fallible {
    let clock = armed()?.1;
    let mut port = StubSysupdate::new(Slot::A, SignatureVerdict::Valid, signed()?);
    let mut machine = Machine::new();
    machine.step(&clock, Event::Declare(candidate()?))?;
    for _ in 0..MAX_RECORDED_CALLS {
        let Some(call) = machine.pending_call() else {
            break;
        };
        let Some(event) = port.invoke(call)?.into_event() else {
            break;
        };
        machine.step(&clock, event)?;
    }
    assert_eq!(machine.state(), State::SlotSwapped);
    machine.step(&clock, Event::ArmWatchdog(timeout(WINDOW)?))?;
    machine.step(&clock, Event::Bless)?;
    assert_eq!(machine.state(), State::Blessed);
    assert_eq!(machine.pending_call(), Some(SysupdateCall::BlessBoot));
    Ok(())
}

/// Positive: the stub answers the three calls that feed the machine.
#[test]
fn the_stub_answers_the_input_calls() -> Fallible {
    let version = Version::parse(RELEASE)?;
    let hash = signed()?;
    let mut port = StubSysupdate::new(Slot::A, SignatureVerdict::Valid, hash);
    assert_eq!(port.current(), Slot::A);
    assert_eq!(port.call(0), None);
    assert_eq!(
        port.invoke(SysupdateCall::List)?,
        CallOutcome::Listed { current: Slot::A }
    );
    assert_eq!(
        port.invoke(SysupdateCall::Verify(version))?,
        CallOutcome::Verified(SignatureVerdict::Valid)
    );
    assert_eq!(
        port.invoke(SysupdateCall::Update(version))?,
        CallOutcome::Acquired(hash)
    );
    assert_eq!(port.call_count(), 3);
    Ok(())
}

/// Positive: the stub answers the swap and the two boot-assessment calls.
#[test]
fn the_stub_answers_the_swap_and_boot_assessment_calls() -> Fallible {
    let mut port = StubSysupdate::new(Slot::B, SignatureVerdict::Valid, signed()?);
    assert_eq!(
        port.invoke(SysupdateCall::SwapSlot(Slot::B))?,
        CallOutcome::Swapped { into: Slot::B }
    );
    assert_eq!(
        port.invoke(SysupdateCall::BlessBoot)?,
        CallOutcome::BlessedBoot
    );
    assert_eq!(
        port.invoke(SysupdateCall::RollbackBoot)?,
        CallOutcome::RolledBackBoot
    );
    assert_eq!(port.call_count(), 3);
    Ok(())
}

/// Positive: the recorded calls come back in the order they were made.
#[test]
fn the_stub_records_calls_in_call_order() -> Fallible {
    let version = Version::parse(RELEASE)?;
    let mut port = StubSysupdate::new(Slot::A, SignatureVerdict::Valid, signed()?);
    for call in [
        SysupdateCall::List,
        SysupdateCall::Verify(version),
        SysupdateCall::Update(version),
        SysupdateCall::SwapSlot(Slot::B),
        SysupdateCall::BlessBoot,
        SysupdateCall::RollbackBoot,
    ] {
        port.invoke(call)?;
    }
    for (index, kind) in CallKind::ALL.iter().enumerate() {
        assert_eq!(port.call(index), Some(*kind));
    }
    Ok(())
}

/// Positive: only the three input calls offer the machine an event; a query
/// and the two boot assessments offer none.
#[test]
fn only_the_input_calls_offer_an_event() -> Fallible {
    assert_eq!(
        CallOutcome::Verified(SignatureVerdict::Invalid).into_event(),
        Some(Event::CheckSignature(SignatureVerdict::Invalid))
    );
    assert_eq!(
        CallOutcome::Acquired(signed()?).into_event(),
        Some(Event::AcquireDelta(signed()?))
    );
    assert_eq!(
        CallOutcome::Swapped { into: Slot::B }.into_event(),
        Some(Event::SwapSlot)
    );
    assert_eq!(CallOutcome::Listed { current: Slot::B }.into_event(), None);
    assert_eq!(CallOutcome::BlessedBoot.into_event(), None);
    assert_eq!(CallOutcome::RolledBackBoot.into_event(), None);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a scripted refusal is returned as an error and still recorded, so
/// a refused call is visible rather than invisible.
#[test]
fn a_refused_call_is_an_error_and_is_still_recorded() -> Fallible {
    let version = Version::parse(RELEASE)?;
    let mut port =
        StubSysupdate::new(Slot::A, SignatureVerdict::Valid, signed()?).refusing(CallKind::Update);
    port.invoke(SysupdateCall::Verify(version))?;
    assert_eq!(
        port.invoke(SysupdateCall::Update(version)).err(),
        Some(PortError::Refused {
            call: CallKind::Update
        })
    );
    assert_eq!(port.call_count(), 2);
    assert_eq!(port.call(1), Some(CallKind::Update));
    Ok(())
}

/// Negative: a refused signature check is reported as an invalid verdict, and
/// that verdict discards the candidate rather than stalling it.
#[test]
fn an_invalid_verdict_from_the_port_discards_the_candidate() -> Fallible {
    let clock = armed()?.1;
    let mut port = StubSysupdate::new(Slot::A, SignatureVerdict::Invalid, signed()?);
    let mut machine = Machine::new();
    machine.step(&clock, Event::Declare(candidate()?))?;
    let call = machine.pending_call().ok_or("a verify call is pending")?;
    assert_eq!(call.kind(), CallKind::Verify);
    let event = port
        .invoke(call)?
        .into_event()
        .ok_or("a verify offers one")?;
    machine.step(&clock, event)?;
    assert_eq!(machine.state(), State::Discarded);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the stub records exactly `MAX_RECORDED_CALLS` calls, and the next
/// one is refused rather than dropped.
#[test]
fn the_call_record_is_full_at_its_bound() -> Fallible {
    let mut port = StubSysupdate::new(Slot::B, SignatureVerdict::Valid, signed()?);
    for _ in 0..MAX_RECORDED_CALLS {
        port.invoke(SysupdateCall::List)?;
    }
    assert_eq!(port.call_count(), MAX_RECORDED_CALLS);
    assert_eq!(
        port.invoke(SysupdateCall::List).err(),
        Some(PortError::RecordFull {
            bound: MAX_RECORDED_CALLS
        })
    );
    assert_eq!(port.call(MAX_RECORDED_CALLS), None);
    Ok(())
}

/// Boundary: every call kind has a stable name and a call maps to exactly its
/// own kind, so the record cannot confuse two calls.
#[test]
fn every_call_maps_to_exactly_its_own_kind() -> Fallible {
    let version = Version::parse(RELEASE)?;
    let pairs = [
        (SysupdateCall::List, CallKind::List, "list"),
        (SysupdateCall::Verify(version), CallKind::Verify, "verify"),
        (SysupdateCall::Update(version), CallKind::Update, "update"),
        (
            SysupdateCall::SwapSlot(Slot::A),
            CallKind::SwapSlot,
            "swap-slot",
        ),
        (SysupdateCall::BlessBoot, CallKind::BlessBoot, "bless-boot"),
        (
            SysupdateCall::RollbackBoot,
            CallKind::RollbackBoot,
            "rollback-boot",
        ),
    ];
    assert_eq!(pairs.len(), CallKind::ALL.len());
    for (call, kind, name) in pairs {
        assert_eq!(call.kind(), kind);
        assert_eq!(kind.name(), name);
        assert_eq!(kind.to_string(), name);
    }
    Ok(())
}
