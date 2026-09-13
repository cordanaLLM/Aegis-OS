// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Triples for the bounded, validated values the lifecycle is built from.
//!
//! Every value the machine holds is validated once, on construction, and then
//! never re-checked: the slot pair, the release version, the dm-verity root
//! hash, the tick scale and the watchdog window. These are the tests for that
//! construction, including the exact widths at which each one starts refusing.

mod common;

use core::num::NonZeroU32;

use aegis_janus_lifecycle::{
    Candidate, Clock, ClockError, Event, EventKind, LifecycleError, MAX_VERSION_LEN,
    MonotonicGuard, ROOT_HASH_BYTES, ROOT_HASH_HEX_LEN, RootHash, SignatureVerdict, Slot, State,
    StubClock, Tick, Timeout, VerityError, Version, VersionError, Watchdog, WatchdogError,
    WatchdogStatus,
};

use common::{Fallible, RELEASE, SIGNED_HEX, signed};

// --- Positive -------------------------------------------------------------

/// Positive: the two slots are each other's alternate, and both spellings the
/// reviewed definitions use are available.
#[test]
fn the_slot_pair_is_the_one_the_definitions_declare() {
    assert_eq!(Slot::ALL, [Slot::A, Slot::B]);
    assert_eq!(Slot::A.other(), Slot::B);
    assert_eq!(Slot::B.other(), Slot::A);
    assert_eq!(Slot::A.other().other(), Slot::A);
    assert_eq!(Slot::A.name(), "a");
    assert_eq!(Slot::B.name(), "b");
    assert_eq!(Slot::A.label(), "root-a");
    assert_eq!(Slot::B.label(), "root-b");
    assert_eq!(Slot::B.to_string(), "b");
}

/// Positive: a candidate carries the release, the target and the signed hash,
/// and reports the slot that stays bootable while it is on trial.
#[test]
fn a_candidate_names_the_release_the_target_and_the_fallback() -> Fallible {
    let version = Version::parse(RELEASE)?;
    let candidate = Candidate::new(version, Slot::B, signed()?);
    assert_eq!(candidate.version(), version);
    assert_eq!(candidate.target(), Slot::B);
    assert_eq!(candidate.fallback(), Slot::A);
    assert_eq!(candidate.expected_root_hash(), signed()?);
    assert_eq!(version.as_str()?, RELEASE);
    assert_eq!(version.as_bytes(), RELEASE.as_bytes());
    assert_eq!(version.len(), RELEASE.len());
    assert!(!version.is_empty());
    assert_eq!(version.to_string(), RELEASE);
    Ok(())
}

/// Positive: a root hash round-trips through its hexadecimal spelling.
#[test]
fn a_root_hash_round_trips_through_hexadecimal() -> Fallible {
    let hash = RootHash::parse_hex(SIGNED_HEX)?;
    let mut buffer = [0u8; ROOT_HASH_HEX_LEN];
    assert_eq!(hash.encode_hex(&mut buffer)?, SIGNED_HEX);
    assert_eq!(hash.to_string(), SIGNED_HEX);
    assert_eq!(hash.as_bytes().len(), ROOT_HASH_BYTES);
    assert_eq!(RootHash::from_bytes(*hash.as_bytes()), hash);
    assert_eq!(RootHash::ZERO.as_bytes(), &[0u8; ROOT_HASH_BYTES]);
    assert_ne!(RootHash::ZERO, hash);
    Ok(())
}

/// Positive: the state vocabulary is complete and stably named.
#[test]
fn the_state_vocabulary_is_complete_and_stably_named() {
    assert_eq!(State::ALL.len(), 10);
    for state in State::ALL {
        assert!(!state.name().is_empty());
        assert_eq!(state.to_string(), state.name());
    }
}

/// Positive: the event vocabulary is complete and stably named.
#[test]
fn the_event_vocabulary_is_complete_and_stably_named() {
    assert_eq!(EventKind::ALL.len(), 10);
    for kind in EventKind::ALL {
        assert!(!kind.name().is_empty());
        assert_eq!(kind.to_string(), kind.name());
    }
    assert_eq!(Event::Tick.kind(), EventKind::Tick);
    assert_eq!(Event::FailDelta.kind(), EventKind::FailDelta);
}

/// Positive: terminal and on-trial states are classified, and `Blessed` is
/// neither, because D13 chose reversible consolidation.
#[test]
fn terminal_and_on_trial_states_are_classified() {
    assert!(State::RolledBack.is_terminal());
    assert!(State::Discarded.is_terminal());
    assert!(!State::Blessed.is_terminal());
    assert!(State::WatchdogArmed.is_on_trial());
    assert!(State::Reopened.is_on_trial());
    assert!(!State::Blessed.is_on_trial());
    assert_eq!(
        SignatureVerdict::ALL,
        [SignatureVerdict::Valid, SignatureVerdict::Invalid]
    );
    assert_eq!(SignatureVerdict::Valid.name(), "valid");
    assert_eq!(SignatureVerdict::Invalid.name(), "invalid");
}

/// Positive: the stubbed clock reports what it was set to, and the guard
/// accepts a repeated reading.
#[test]
fn the_stub_clock_reports_what_it_was_set_to() -> Fallible {
    let mut clock = StubClock::new(Tick::ORIGIN);
    assert_eq!(clock.at(), Tick::ORIGIN);
    assert_eq!(clock.now()?, Tick::ORIGIN);
    clock.advance(42)?;
    assert_eq!(clock.now()?, Tick::new(42));
    clock.set(Tick::new(7));
    assert_eq!(clock.now()?.get(), 7);
    let mut guard = MonotonicGuard::new();
    assert_eq!(guard.last(), None);
    assert_eq!(guard.observe(Tick::new(7))?, Tick::new(7));
    assert_eq!(guard.observe(Tick::new(7))?, Tick::new(7));
    assert_eq!(guard.last(), Some(Tick::new(7)));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a release version that is empty, over-long or outside the
/// permitted character set is refused, never truncated or sanitised.
#[test]
fn a_malformed_release_version_is_refused() {
    assert_eq!(Version::parse("").err(), Some(VersionError::Empty));
    for rejected in [
        "1.4.0 beta",
        "1.4.0/next",
        "1.4.0\"",
        "1.4.0\\",
        "1,4,0",
        "ünf",
    ] {
        assert_eq!(
            Version::parse(rejected).err(),
            Some(VersionError::Charset),
            "{rejected:?} must not parse"
        );
    }
}

/// Negative: a root hash that is the wrong width or not lower-case
/// hexadecimal is refused; upper case is not folded.
#[test]
fn a_malformed_root_hash_is_refused() {
    assert!(matches!(
        RootHash::parse_hex(&SIGNED_HEX.to_uppercase()).err(),
        Some(VerityError::Encoding)
    ));
    assert!(matches!(
        RootHash::parse_hex("zz").err(),
        Some(VerityError::Width { .. })
    ));
    let wrong_charset: String = SIGNED_HEX.replacen('a', "g", 1);
    assert_eq!(
        RootHash::parse_hex(&wrong_charset).err(),
        Some(VerityError::Encoding)
    );
}

/// Negative: a clock that steps backwards, and a watchdog window that would
/// overflow the tick scale, are both refused.
#[test]
fn a_backwards_clock_and_an_overflowing_window_are_refused() -> Fallible {
    let mut guard = MonotonicGuard::new();
    guard.observe(Tick::new(10))?;
    assert_eq!(
        guard.observe(Tick::new(9)).err(),
        Some(ClockError::NonMonotonic {
            last: 10,
            observed: 9,
        })
    );
    assert_eq!(
        guard.last(),
        Some(Tick::new(10)),
        "the guard keeps the earlier reading"
    );

    let mut clock = StubClock::new(Tick::new(u64::MAX));
    assert_eq!(
        clock.advance(1).err(),
        Some(ClockError::Overflow {
            at: u64::MAX,
            by: 1,
        })
    );
    assert_eq!(clock.at(), Tick::new(u64::MAX), "the clock did not move");

    let timeout = Timeout::from_ticks(2).ok_or("two ticks is a timeout")?;
    assert_eq!(
        Watchdog::arm(Tick::new(u64::MAX), timeout).err(),
        Some(WatchdogError::Overflow {
            armed_at: u64::MAX,
            timeout: 2,
        })
    );
    assert_eq!(Tick::new(u64::MAX).checked_add(timeout), None);
    Ok(())
}

/// Negative: a zero watchdog window is unrepresentable, so a watchdog cannot
/// expire at the tick it was armed.
#[test]
fn a_zero_watchdog_window_is_unrepresentable() -> Fallible {
    assert_eq!(Timeout::from_ticks(0), None);
    let one = Timeout::new(NonZeroU32::new(1).ok_or("one is not zero")?);
    assert_eq!(one.ticks(), 1);
    assert_eq!(Timeout::from_ticks(1), Some(one));
    Ok(())
}

/// Negative: arming a watchdog that overflows refuses the whole step, and the
/// refusal is the watchdog's, not the clock's.
///
/// [`LifecycleError::NoCandidate`] is named here rather than triggered: the
/// bless planner reads the candidate, and no on-trial state is reachable
/// without one. The variant exists so that planner is total without an
/// `unwrap`, and this assertion records that it is a distinct refusal.
#[test]
fn an_overflowing_arm_refuses_the_step() -> Fallible {
    let clock = StubClock::new(Tick::new(u64::MAX));
    let mut machine = aegis_janus_lifecycle::Machine::new();
    machine.step(&clock, Event::Declare(common::candidate()?))?;
    machine.step(&clock, Event::CheckSignature(SignatureVerdict::Valid))?;
    machine.step(&clock, Event::AcquireDelta(signed()?))?;
    machine.step(&clock, Event::SwapSlot)?;
    let timeout = Timeout::from_ticks(1).ok_or("one tick is a timeout")?;
    let error = common::refusal(&mut machine, clock, Event::ArmWatchdog(timeout))?;
    assert_eq!(
        error,
        LifecycleError::Watchdog(WatchdogError::Overflow {
            armed_at: u64::MAX,
            timeout: 1,
        })
    );
    assert_eq!(machine.state(), State::SlotSwapped);
    assert_ne!(
        LifecycleError::NoCandidate.to_string(),
        LifecycleError::VerityUnmeasured.to_string()
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a version of exactly `MAX_VERSION_LEN` bytes parses and one byte
/// more does not.
#[test]
fn a_version_is_accepted_at_exactly_its_bound() -> Fallible {
    let at_bound = "9".repeat(MAX_VERSION_LEN);
    let over_bound = "9".repeat(MAX_VERSION_LEN.saturating_add(1));
    assert_eq!(Version::parse(&at_bound)?.len(), MAX_VERSION_LEN);
    assert_eq!(
        Version::parse(&over_bound).err(),
        Some(VersionError::TooLong {
            max: MAX_VERSION_LEN,
            actual: MAX_VERSION_LEN.saturating_add(1),
        })
    );
    assert_eq!(Version::parse("1")?.len(), 1);
    Ok(())
}

/// Boundary: a root hash is accepted at exactly `ROOT_HASH_HEX_LEN`
/// characters, and refused one character on either side.
#[test]
fn a_root_hash_is_accepted_at_exactly_its_width() {
    assert_eq!(SIGNED_HEX.len(), ROOT_HASH_HEX_LEN);
    assert!(RootHash::parse_hex(SIGNED_HEX).is_ok());
    let short = SIGNED_HEX
        .get(..ROOT_HASH_HEX_LEN.saturating_sub(1))
        .unwrap_or_default();
    assert_eq!(
        RootHash::parse_hex(short).err(),
        Some(VerityError::Width {
            expected: ROOT_HASH_HEX_LEN,
            actual: ROOT_HASH_HEX_LEN.saturating_sub(1),
        })
    );
    let long = format!("{SIGNED_HEX}0");
    assert_eq!(
        RootHash::parse_hex(&long).err(),
        Some(VerityError::Width {
            expected: ROOT_HASH_HEX_LEN,
            actual: ROOT_HASH_HEX_LEN.saturating_add(1),
        })
    );
}

/// Boundary: the watchdog interval is half-open, so the tick before the
/// deadline runs, the deadline itself has expired, and a one-tick window
/// expires at the very next tick.
#[test]
fn the_watchdog_interval_is_half_open() -> Fallible {
    let timeout = Timeout::from_ticks(1).ok_or("one tick is a timeout")?;
    let watchdog = Watchdog::arm(Tick::new(100), timeout)?;
    assert_eq!(watchdog.armed_at(), Tick::new(100));
    assert_eq!(watchdog.due_at(), Tick::new(101));
    assert_eq!(watchdog.status(Tick::new(100)), WatchdogStatus::Running);
    assert_eq!(watchdog.status(Tick::new(101)), WatchdogStatus::Expired);
    assert_eq!(watchdog.status(Tick::new(102)), WatchdogStatus::Expired);
    assert_eq!(Tick::new(100).checked_add(timeout), Some(Tick::new(101)));
    assert_eq!(Tick::ORIGIN.get(), 0);
    Ok(())
}
