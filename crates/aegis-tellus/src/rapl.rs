// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Measured energy: the powercap counter delta M21 puts behind the seam.
//!
//! # What this module is
//!
//! [`MeasuredWattage`] is the second implementation of [`WattageSource`], the
//! one M05 left room for. It is built from two readings of one zone's energy
//! counter and the interval between them, and every sample it hands back is
//! labelled [`Provenance::Measured`]. The SCI arithmetic in [`crate::sci`] is
//! not touched: a caller samples the seam exactly as it samples
//! [`crate::SimulatedWattage`], and [`crate::ZoneSample::energy`] and
//! [`crate::SciEngine::sci_rate`] never learn which source answered.
//!
//! # What this module does not do
//!
//! It reads nothing. The counter text arrives from a caller -- on the
//! reference profile, the gate `make verify-workstation` runs, which reads the
//! root-only counter through one scoped `sudo -n cat` and nothing else (the
//! maintainer's decision of 2026-09-29, recorded with M21's privileged-read
//! criterion). This crate still opens no file, starts no process and names no
//! sysfs path in code; `tests/stubbed_effects.rs` keeps it that way.
//!
//! # Rollover
//!
//! The counter is a 32-bit hardware register scaled into microjoules, and it
//! wraps to zero after `max_energy_range_uj`. [`EnergyDelta::between`] treats
//! a second reading below the first as exactly one wrap and returns
//! `(range - before) + after`. Two facts bound that:
//!
//! * the kernel scales the raw register by an integer unit
//!   (`drivers/powercap/intel_rapl_common.c`: `(ENERGY_UNIT_SCALE *
//!   MICROJOULE_PER_JOULE) >> value`, 15258 thousandths of a microjoule for
//!   the reference profile's 2^-16 J unit), so the reported range is
//!   `0xffff_ffff * 15.258` = 65532610987 uJ and the true modulus is one raw
//!   unit, 15.258 uJ, above it. A wrapped delta is therefore at most one
//!   counter unit short, never negative and never absurd;
//! * a second wrap inside one interval would be invisible, so
//!   [`MeasuredWattage::from_counters`] refuses any interval over which a draw
//!   of [`MAX_PLAUSIBLE_ZONE_WATTS`] could reach the range, and refuses any
//!   resulting draw above that bound -- which is what a counter reset read as
//!   a wrap would produce.
//!
//! # Accuracy, stated where the number is made
//!
//! On the reference profile's AMD package the counter is a model-based
//! estimate the processor derives from activity counters, not a measured
//! power rail. Only a delta within one zone is treated as trustworthy;
//! absolute watts carry the vendor's error, and the kernel's truncated unit
//! understates every reading by a further 0.0052 per cent (15.258 against
//! 15.2587890625 uJ). A [`Provenance::Measured`] label says the number came
//! from a counter, not that the counter is a meter.

use core::fmt;
use core::num::NonZeroU64;

use crate::error::TellusError;
use crate::power::{Provenance, RaplZone, SampleDeadline, WattageSource, ZoneList, ZoneSample};
use crate::register::REFERENCE_MAX_ENERGY_RANGE_UJ;
use crate::sci::{EnergyKwh, JOULES_PER_KWH, MAX_SECONDS, Seconds, Watts};

/// The largest draw one zone may be measured at, in watts.
///
/// A bound no single desktop package approaches. It exists so that a counter
/// reset, or a second wrap hidden inside one interval, becomes a refusal
/// rather than a figure: both read as an implausibly large delta.
pub const MAX_PLAUSIBLE_ZONE_WATTS: f64 = 1_000.0;

/// Scalar upper bound, in bytes, on one counter read's text.
///
/// Twenty decimal digits hold any `u64`, and the attribute ends in one
/// newline, so twenty-one bytes is the longest well-formed read.
pub const MAX_COUNTER_TEXT_BYTES: usize = 21;

/// Microjoules per joule.
pub const MICROJOULES_PER_JOULE: f64 = 1.0e6;

/// Nanoseconds per second.
pub const NANOS_PER_SECOND: f64 = 1.0e9;

/// The longest interval a measured sample may span, in nanoseconds.
///
/// [`MAX_SECONDS`], the bound [`Seconds`] already holds, in nanoseconds.
pub const MAX_INTERVAL_NANOS: u64 = 86_400_000_000_000;

/// 2^32, the factor that reassembles a `u64` from its two `u32` halves.
const TWO_POW_32: f64 = 4_294_967_296.0;

/// Converts a `u64` to the nearest `f64` without an `as` cast.
fn u64_to_f64(value: u64) -> f64 {
    let high = u32::try_from(value.checked_shr(32).unwrap_or(0)).unwrap_or(u32::MAX);
    let low = u32::try_from(value & 0xFFFF_FFFF).unwrap_or(u32::MAX);
    f64::from(high) * TWO_POW_32 + f64::from(low)
}

/// Parses one counter read's text: decimal digits and at most one newline.
fn parse_counter(text: &str) -> Result<u64, TellusError> {
    let digits = text.strip_suffix('\n').unwrap_or(text);
    if digits.is_empty() {
        return Err(TellusError::Counter {
            reason: "an empty read",
        });
    }
    if text.len() > MAX_COUNTER_TEXT_BYTES {
        return Err(TellusError::Counter {
            reason: "a read longer than any counter value",
        });
    }
    if !digits.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(TellusError::Counter {
            reason: "a read carrying a byte that is not a decimal digit",
        });
    }
    digits.parse::<u64>().map_err(|_| TellusError::Counter {
        reason: "a read past the largest counter value",
    })
}

/// The range a zone's counter wraps at: its `max_energy_range_uj`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EnergyRange(NonZeroU64);

impl EnergyRange {
    /// The range both reference-profile zones report,
    /// [`REFERENCE_MAX_ENERGY_RANGE_UJ`].
    pub const REFERENCE: Self = match NonZeroU64::new(REFERENCE_MAX_ENERGY_RANGE_UJ) {
        Some(range) => Self(range),
        None => Self(NonZeroU64::MAX),
    };

    /// Builds a range of `microjoules`.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Counter`] for zero: a counter with no range
    /// cannot carry a delta.
    pub const fn new(microjoules: u64) -> Result<Self, TellusError> {
        match NonZeroU64::new(microjoules) {
            Some(range) => Ok(Self(range)),
            None => Err(TellusError::Counter {
                reason: "a range of zero microjoules",
            }),
        }
    }

    /// Parses the text of a `max_energy_range_uj` read.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Counter`] for text that is not one decimal
    /// value, and for zero.
    pub fn parse(text: &str) -> Result<Self, TellusError> {
        Self::new(parse_counter(text)?)
    }

    /// Returns the range in microjoules.
    #[must_use]
    pub const fn microjoules(self) -> u64 {
        self.0.get()
    }

    /// Returns the range in joules.
    #[must_use]
    pub fn joules(self) -> f64 {
        u64_to_f64(self.microjoules()) / MICROJOULES_PER_JOULE
    }
}

/// What one counter read returned: its text, or nothing a value can stand for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum CounterRead<'a> {
    /// The read returned this text.
    Text(&'a str),
    /// The read failed: no permission, no attribute, a missed deadline.
    Unreadable,
}

/// One validated reading of a zone's energy counter, in microjoules.
///
/// There is no `Default` and no zero constructor that skips validation: a
/// failed read is [`TellusError::Counter`], never a reading of zero.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct EnergyReading(u64);

impl EnergyReading {
    /// Builds a reading of `microjoules` against the zone's `range`.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Counter`] for a value above the range, which the
    /// counter cannot hold.
    pub const fn new(microjoules: u64, range: EnergyRange) -> Result<Self, TellusError> {
        if microjoules > range.microjoules() {
            return Err(TellusError::Counter {
                reason: "a value above the zone's max_energy_range_uj",
            });
        }
        Ok(Self(microjoules))
    }

    /// Parses the text of one `energy_uj` read.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Counter`] for empty text, a non-digit byte, a
    /// value past `u64`, and a value above the range.
    pub fn parse(text: &str, range: EnergyRange) -> Result<Self, TellusError> {
        Self::new(parse_counter(text)?, range)
    }

    /// Turns one read's outcome into a reading, failing closed.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Counter`] for [`CounterRead::Unreadable`] and
    /// for any text [`Self::parse`] refuses. No outcome yields a default.
    pub fn from_read(read: CounterRead<'_>, range: EnergyRange) -> Result<Self, TellusError> {
        match read {
            CounterRead::Text(text) => Self::parse(text, range),
            CounterRead::Unreadable => Err(TellusError::Counter {
                reason: "an unreadable counter",
            }),
        }
    }

    /// Returns the reading in microjoules.
    #[must_use]
    pub const fn microjoules(self) -> u64 {
        self.0
    }
}

/// The energy one zone accumulated between two readings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EnergyDelta {
    microjoules: u64,
    wrapped: bool,
}

impl EnergyDelta {
    /// Returns the energy accumulated from `before` to `after`.
    ///
    /// A second reading below the first is one wrap at `range`, and the
    /// delta is `(range - before) + after`; see the module documentation for
    /// why that is at most one counter unit short of the true increment.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Counter`] when either reading lies above
    /// `range`, which a reading built against another range could.
    pub fn between(
        before: EnergyReading,
        after: EnergyReading,
        range: EnergyRange,
    ) -> Result<Self, TellusError> {
        let limit = range.microjoules();
        if before.microjoules() > limit || after.microjoules() > limit {
            return Err(TellusError::Counter {
                reason: "a reading above the range the delta is taken against",
            });
        }
        if let Some(microjoules) = after.microjoules().checked_sub(before.microjoules()) {
            return Ok(Self {
                microjoules,
                wrapped: false,
            });
        }
        let to_wrap = limit
            .checked_sub(before.microjoules())
            .ok_or(TellusError::Counter {
                reason: "a reading above the range the delta is taken against",
            })?;
        let microjoules = to_wrap
            .checked_add(after.microjoules())
            .ok_or(TellusError::Counter {
                reason: "a wrapped delta past the largest counter value",
            })?;
        Ok(Self {
            microjoules,
            wrapped: true,
        })
    }

    /// Returns the delta in microjoules.
    #[must_use]
    pub const fn microjoules(self) -> u64 {
        self.microjoules
    }

    /// Returns `true` when the counter wrapped between the two readings.
    #[must_use]
    pub const fn wrapped(self) -> bool {
        self.wrapped
    }

    /// Returns the delta in joules.
    #[must_use]
    pub fn joules(self) -> f64 {
        u64_to_f64(self.microjoules) / MICROJOULES_PER_JOULE
    }

    /// Returns the delta as energy, straight from the counter.
    ///
    /// # Errors
    ///
    /// Propagates [`EnergyKwh::new`].
    pub fn energy(self) -> Result<EnergyKwh, TellusError> {
        EnergyKwh::new(self.joules() / JOULES_PER_KWH)
    }
}

/// A validated, positive interval between two readings, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SampleInterval(NonZeroU64);

impl SampleInterval {
    /// Builds an interval of `nanos` nanoseconds.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Interval`] for zero and for anything past
    /// [`MAX_INTERVAL_NANOS`].
    pub const fn from_nanos(nanos: u64) -> Result<Self, TellusError> {
        if nanos > MAX_INTERVAL_NANOS {
            return Err(TellusError::Interval {
                reason: "a value past the recorded bound",
            });
        }
        match NonZeroU64::new(nanos) {
            Some(value) => Ok(Self(value)),
            None => Err(TellusError::Interval {
                reason: "a value that is not positive",
            }),
        }
    }

    /// Returns the interval in nanoseconds.
    #[must_use]
    pub const fn nanos(self) -> u64 {
        self.0.get()
    }

    /// Returns the interval as validated seconds.
    ///
    /// # Errors
    ///
    /// Propagates [`Seconds::new`], which cannot refuse a value this type
    /// admitted.
    pub fn seconds(self) -> Result<Seconds, TellusError> {
        Seconds::new(u64_to_f64(self.nanos()) / NANOS_PER_SECOND)
    }
}

/// Two readings of one zone and the interval between them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CounterPair {
    /// The earlier reading.
    pub before: EnergyReading,
    /// The later reading.
    pub after: EnergyReading,
    /// The time between them.
    pub interval: SampleInterval,
}

/// The measured source: one zone's draw, derived from two counter readings.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct MeasuredWattage {
    sample: ZoneSample,
    zones: ZoneList,
    delta: EnergyDelta,
    interval: SampleInterval,
    service: SampleDeadline,
}

impl MeasuredWattage {
    /// Builds the source for `zone` from `pair`, against the zone's `range`.
    ///
    /// `service` is what a sample of the finished measurement demands of a
    /// caller's deadline; the reads themselves were taken under the gate's.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::ZoneAbsent`] for a zone the reference profile
    /// exposes no counter for (`dram`, `psys`; decision D60), rather than a
    /// zero; [`TellusError::Interval`] for an interval long enough for a draw
    /// of [`MAX_PLAUSIBLE_ZONE_WATTS`] to wrap the counter, since a second wrap
    /// would be invisible; [`TellusError::Wattage`] for a resulting draw above
    /// that bound; and [`TellusError::Counter`] from [`EnergyDelta::between`].
    pub fn from_counters(
        zone: RaplZone,
        pair: CounterPair,
        range: EnergyRange,
        service: SampleDeadline,
    ) -> Result<Self, TellusError> {
        let zones = measured_zone_list(zone)?;
        let seconds = pair.interval.seconds()?;
        if MAX_PLAUSIBLE_ZONE_WATTS * seconds.get() >= range.joules() {
            return Err(TellusError::Interval {
                reason: "a value long enough for a second wrap to hide inside it",
            });
        }
        let delta = EnergyDelta::between(pair.before, pair.after, range)?;
        let draw = delta.joules() / seconds.get();
        if draw > MAX_PLAUSIBLE_ZONE_WATTS {
            return Err(TellusError::Wattage {
                reason: "a measured draw past the plausibility bound, as a counter reset reads",
            });
        }
        Ok(Self {
            sample: ZoneSample::new(zone, Watts::new(draw)?, Provenance::Measured),
            zones,
            delta,
            interval: pair.interval,
            service,
        })
    }

    /// Returns the measured sample.
    #[must_use]
    pub const fn measured(&self) -> ZoneSample {
        self.sample
    }

    /// Returns the energy delta the draw was derived from.
    #[must_use]
    pub const fn delta(&self) -> EnergyDelta {
        self.delta
    }

    /// Returns the interval the delta spans.
    #[must_use]
    pub const fn interval(&self) -> SampleInterval {
        self.interval
    }
}

/// Returns the one-zone list a measured source carries, refusing an absent zone.
fn measured_zone_list(zone: RaplZone) -> Result<ZoneList, TellusError> {
    let profile = ZoneList::reference_profile();
    if !profile.contains(zone) {
        return Err(TellusError::ZoneAbsent {
            zone: zone.name(),
            carried: profile.len(),
        });
    }
    let mut zones = ZoneList::new();
    zones.push(zone)?;
    Ok(zones)
}

impl WattageSource for MeasuredWattage {
    fn zones(&self) -> ZoneList {
        self.zones
    }

    fn service_time(&self) -> SampleDeadline {
        self.service
    }

    fn sample(&self, zone: RaplZone, deadline: SampleDeadline) -> Result<ZoneSample, TellusError> {
        if !self.zones.contains(zone) {
            return Err(TellusError::ZoneAbsent {
                zone: zone.name(),
                carried: self.zones.len(),
            });
        }
        if deadline.millis() < self.service.millis() {
            return Err(TellusError::WouldBlock {
                needed: self.service.millis(),
                offered: deadline.millis(),
            });
        }
        Ok(self.sample)
    }
}

impl fmt::Display for EnergyDelta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let wrap = if self.wrapped { ", wrapped once" } else { "" };
        write!(f, "{} uJ{wrap}", self.microjoules)
    }
}

/// The largest interval, in seconds, that [`MeasuredWattage::from_counters`]
/// admits against `range`: the time a draw of [`MAX_PLAUSIBLE_ZONE_WATTS`]
/// takes to reach it, capped at [`MAX_SECONDS`].
#[must_use]
pub fn unambiguous_interval_seconds(range: EnergyRange) -> f64 {
    let limit = range.joules() / MAX_PLAUSIBLE_ZONE_WATTS;
    if limit < MAX_SECONDS {
        limit
    } else {
        MAX_SECONDS
    }
}
