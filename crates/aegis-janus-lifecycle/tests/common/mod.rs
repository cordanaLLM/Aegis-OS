// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! Everything here reaches `aegis-janus-lifecycle` through its public API
//! only, and nothing here uses `unwrap` or `expect`: a test that cannot build
//! its own input returns the failure instead of panicking through a lint the
//! workspace denies.

#![allow(dead_code)]

use aegis_janus_lifecycle::{
    Candidate, Event, LifecycleError, Machine, RootHash, SignatureVerdict, Slot, StubClock, Tick,
    Timeout, Version,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The release version the fixtures declare.
pub const RELEASE: &str = "1.4.0";

/// The dm-verity root hash the signed release declares.
pub const SIGNED_HEX: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90";

/// The same hash with its last nibble changed: one slot that is not the release.
pub const DRIFTED_HEX: &str = "a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f91";

/// The tick the fixtures arm the watchdog at.
pub const ARMED_AT: u64 = 1_000;

/// The watchdog window the fixtures use, in ticks.
pub const WINDOW: u32 = 600;

/// The tick the fixture watchdog expires at.
pub const DUE_AT: u64 = 1_600;

/// Returns the signed release's root hash.
///
/// # Errors
///
/// Returns the parse failure, so a fixture that stops being a root hash fails
/// the suite instead of being quietly replaced.
pub fn signed() -> Result<RootHash, Box<dyn std::error::Error>> {
    Ok(RootHash::parse_hex(SIGNED_HEX)?)
}

/// Returns a root hash that differs from [`signed`] in one nibble.
///
/// # Errors
///
/// Returns the parse failure.
pub fn drifted() -> Result<RootHash, Box<dyn std::error::Error>> {
    Ok(RootHash::parse_hex(DRIFTED_HEX)?)
}

/// Returns the fixture candidate: release [`RELEASE`] into [`Slot::B`].
///
/// # Errors
///
/// Returns the failure that stopped the candidate being built.
pub fn candidate() -> Result<Candidate, Box<dyn std::error::Error>> {
    Ok(Candidate::new(Version::parse(RELEASE)?, Slot::B, signed()?))
}

/// Returns a timeout of `ticks`.
///
/// # Errors
///
/// Returns a message when `ticks` is zero, which no fixture uses.
pub fn timeout(ticks: u32) -> Result<Timeout, Box<dyn std::error::Error>> {
    Timeout::from_ticks(ticks).ok_or_else(|| "a watchdog timeout must be positive".into())
}

/// Drives a fresh machine to [`State::WatchdogArmed`](aegis_janus_lifecycle::State::WatchdogArmed).
///
/// The clock stands at [`ARMED_AT`] throughout, so the returned watchdog is
/// due at [`DUE_AT`] and a test can place a tick on either side of it.
///
/// # Errors
///
/// Returns the first refusal, so a change that breaks the happy path fails
/// every test that depends on it rather than only the one that names it.
pub fn armed() -> Result<(Machine, StubClock), Box<dyn std::error::Error>> {
    let candidate = candidate()?;
    let clock = StubClock::new(Tick::new(ARMED_AT));
    let mut machine = Machine::new();
    machine.step(&clock, Event::Declare(candidate))?;
    machine.step(&clock, Event::CheckSignature(SignatureVerdict::Valid))?;
    machine.step(&clock, Event::AcquireDelta(signed()?))?;
    machine.step(&clock, Event::SwapSlot)?;
    machine.step(&clock, Event::ArmWatchdog(timeout(WINDOW)?))?;
    Ok((machine, clock))
}

/// Drives a fresh machine all the way to
/// [`State::Blessed`](aegis_janus_lifecycle::State::Blessed).
///
/// # Errors
///
/// Returns the first refusal.
pub fn blessed() -> Result<(Machine, StubClock), Box<dyn std::error::Error>> {
    let (mut machine, clock) = armed()?;
    machine.step(&clock, Event::Bless)?;
    Ok((machine, clock))
}

/// Returns the error a step produced, or a message when it was accepted.
///
/// # Errors
///
/// Returns a message when the step was accepted, so a test that expects a
/// refusal cannot pass by the step succeeding.
pub fn refusal(
    machine: &mut Machine,
    clock: StubClock,
    event: Event,
) -> Result<LifecycleError, Box<dyn std::error::Error>> {
    match machine.step(&clock, event) {
        Ok(transition) => {
            Err(format!("the step was accepted and moved to {}", transition.to()).into())
        }
        Err(error) => Ok(error),
    }
}
