// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The injected clock and the A/B boot watchdog (HISS-02).
//!
//! Time is a parameter here, never ambient. [`Machine::step`] reads it through
//! a [`Clock`], and the only implementation this crate ships is [`StubClock`],
//! which holds whatever tick a caller set. Nothing in this crate names
//! `SystemTime`, so a lifecycle run is reproducible by construction and the
//! watchdog boundary is a value a test writes rather than a wall-clock race.
//!
//! [`Machine::step`]: crate::Machine::step
//!
//! # The boundary the milestone asks about
//!
//! REQ-P02-08 says that a hardware watchdog expiring before the bless signal
//! triggers an automatic A/B rollback. [`Watchdog`] is the half-open interval
//! `[armed_at, due_at)`: [`WatchdogStatus::Expired`] is `now >= due_at`, so a
//! tick exactly at the timeout has expired and the tick before it has not.
//! Equality expires because that is the fail-closed reading: a watchdog that
//! treated its own deadline as still running would extend every timeout by one
//! tick.
//!
//! # Why the type is not borrowed from `aegis-justitia`
//!
//! `aegis-justitia` carries a similar clock for approval windows. This crate
//! does not depend on it: the subsystem graph runs P02 to P06
//! (`PROVIDES_TPM2_ATTESTATION`), and a library dependency the other way would
//! invert that edge and pull a decision engine into a boot-path model. The
//! vocabulary differs too -- an approval window is wall-clock seconds, a boot
//! watchdog is ticks from an arbitrary origin -- so the duplication is a
//! recorded choice, not an oversight.

use core::num::NonZeroU32;

/// Reasons a clock reading is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ClockError {
    /// The clock moved backwards relative to an earlier reading.
    #[error("clock moved backwards: last tick {last}, observed {observed}")]
    NonMonotonic {
        /// The most recent previously accepted reading.
        last: u64,
        /// The reading just taken.
        observed: u64,
    },
    /// Advancing the clock would leave the representable tick range.
    #[error("advancing {at} by {by} ticks leaves the representable range")]
    Overflow {
        /// The tick the clock stood at.
        at: u64,
        /// The advance that was refused.
        by: u64,
    },
}

/// Reasons a watchdog cannot be armed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum WatchdogError {
    /// Arming would push the due tick past the representable range.
    #[error("a timeout of {timeout} ticks overflows from tick {armed_at}")]
    Overflow {
        /// The tick the watchdog would have been armed at.
        armed_at: u64,
        /// The timeout that was refused.
        timeout: u32,
    },
}

/// A tick count from the injected clock's own, arbitrary origin.
///
/// The origin is deliberately not the Unix epoch: nothing in this crate reads
/// the host clock, so a tick is only meaningful relative to the other ticks in
/// the same run. That is what makes a rendered trace comparable across
/// machines.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(transparent)]
pub struct Tick(u64);

impl Tick {
    /// The origin of the tick scale.
    pub const ORIGIN: Self = Self(0);

    /// Wraps a tick count.
    #[must_use]
    pub const fn new(ticks: u64) -> Self {
        Self(ticks)
    }

    /// Returns the wrapped tick count.
    #[must_use]
    pub const fn get(self) -> u64 {
        self.0
    }

    /// Adds `timeout`, returning `None` rather than a wrapped sum on overflow.
    #[must_use]
    pub const fn checked_add(self, timeout: Timeout) -> Option<Self> {
        match self.0.checked_add(timeout.ticks() as u64) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }
}

impl core::fmt::Display for Tick {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "{}", self.0)
    }
}

/// A strictly positive watchdog timeout, in ticks.
///
/// Zero is unrepresentable: a watchdog that expires at the tick it was armed
/// would roll back before the candidate ever ran.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Timeout(NonZeroU32);

impl Timeout {
    /// Wraps a strictly positive tick count.
    #[must_use]
    pub const fn new(ticks: NonZeroU32) -> Self {
        Self(ticks)
    }

    /// Wraps `ticks`, returning `None` when it is zero.
    #[must_use]
    pub const fn from_ticks(ticks: u32) -> Option<Self> {
        match NonZeroU32::new(ticks) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the timeout width in ticks.
    #[must_use]
    pub const fn ticks(self) -> u32 {
        self.0.get()
    }
}

/// Whether an armed watchdog has expired.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum WatchdogStatus {
    /// The deadline has not been reached; `now` is strictly before `due_at`.
    Running,
    /// The deadline has been reached or passed; `now >= due_at`.
    Expired,
}

/// An armed boot watchdog over the half-open interval `[armed_at, due_at)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Watchdog {
    armed_at: Tick,
    due_at: Tick,
}

impl Watchdog {
    /// Arms a watchdog of `timeout` ticks at `armed_at`.
    ///
    /// # Errors
    ///
    /// Returns [`WatchdogError::Overflow`] when the due tick would leave the
    /// representable range. The addition is checked and never wraps.
    pub fn arm(armed_at: Tick, timeout: Timeout) -> Result<Self, WatchdogError> {
        let due_at = armed_at
            .checked_add(timeout)
            .ok_or(WatchdogError::Overflow {
                armed_at: armed_at.get(),
                timeout: timeout.ticks(),
            })?;
        Ok(Self { armed_at, due_at })
    }

    /// Returns the tick the watchdog was armed at.
    #[must_use]
    pub const fn armed_at(&self) -> Tick {
        self.armed_at
    }

    /// Returns the tick the watchdog expires at.
    #[must_use]
    pub const fn due_at(&self) -> Tick {
        self.due_at
    }

    /// Classifies `now` against the deadline; `now >= due_at` has expired.
    #[must_use]
    pub fn status(&self, now: Tick) -> WatchdogStatus {
        if now >= self.due_at {
            WatchdogStatus::Expired
        } else {
            WatchdogStatus::Running
        }
    }
}

/// A source of ticks.
pub trait Clock {
    /// Reads the current tick.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError`] when the source is unusable. Every caller on the
    /// lifecycle path treats the error as fail-closed and makes no transition.
    fn now(&self) -> Result<Tick, ClockError>;
}

/// The stubbed clock: the only [`Clock`] this crate ships.
///
/// It is library code rather than a test fixture on purpose. M24 drives this
/// machine from an observed QEMU run, and it needs to set the tick to the value
/// it read from that run rather than to whatever the host clock says.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StubClock {
    at: Tick,
}

impl StubClock {
    /// Builds a clock standing at `at`.
    #[must_use]
    pub const fn new(at: Tick) -> Self {
        Self { at }
    }

    /// Returns the tick the clock stands at, without the error channel.
    #[must_use]
    pub const fn at(&self) -> Tick {
        self.at
    }

    /// Moves the clock to `at`.
    ///
    /// Moving it backwards is permitted here and refused one layer up by
    /// [`MonotonicGuard`], so a stepped clock is an observable fault rather
    /// than an unrepresentable one.
    pub const fn set(&mut self, at: Tick) {
        self.at = at;
    }

    /// Advances the clock by `ticks`.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::Overflow`] when the sum would leave the
    /// representable range; the clock is left where it was.
    pub const fn advance(&mut self, ticks: u64) -> Result<(), ClockError> {
        match self.at.get().checked_add(ticks) {
            Some(value) => {
                self.at = Tick::new(value);
                Ok(())
            }
            None => Err(ClockError::Overflow {
                at: self.at.get(),
                by: ticks,
            }),
        }
    }
}

impl Default for StubClock {
    fn default() -> Self {
        Self::new(Tick::ORIGIN)
    }
}

impl Clock for StubClock {
    fn now(&self) -> Result<Tick, ClockError> {
        Ok(self.at)
    }
}

/// Refuses a reading that precedes the previous one.
///
/// The machine holds one of these. A watchdog whose clock could be wound back
/// would never expire, so a backwards step fails the transition instead of
/// being accepted and averaged away.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct MonotonicGuard {
    last: Option<Tick>,
}

impl MonotonicGuard {
    /// Builds a guard that has observed nothing.
    #[must_use]
    pub const fn new() -> Self {
        Self { last: None }
    }

    /// Returns the most recent accepted reading, if there is one.
    #[must_use]
    pub const fn last(&self) -> Option<Tick> {
        self.last
    }

    /// Accepts `observed` when it is at or after the previous reading.
    ///
    /// A repeated reading is accepted: two steps inside one tick are ordinary.
    ///
    /// # Errors
    ///
    /// Returns [`ClockError::NonMonotonic`] when `observed` precedes the
    /// previous reading. The guard keeps the earlier reading, so a stepped
    /// clock cannot be laundered by retrying.
    pub const fn observe(&mut self, observed: Tick) -> Result<Tick, ClockError> {
        if let Some(last) = self.last
            && observed.get() < last.get()
        {
            return Err(ClockError::NonMonotonic {
                last: last.get(),
                observed: observed.get(),
            });
        }
        self.last = Some(observed);
        Ok(observed)
    }
}
