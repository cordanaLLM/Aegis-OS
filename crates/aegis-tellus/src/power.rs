// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The wattage seam, and the bounded cgroup slice table behind it.
//!
//! # What the seam is for
//!
//! The SCI arithmetic in [`crate::sci`] needs a draw. At this milestone the
//! draw is a recorded simulated constant; at M21 it becomes a delta between
//! two reads of a RAPL energy counter. [`WattageSource`] is the boundary
//! between the two, and it is placed so that substituting the second for the
//! first changes no arithmetic: a source hands back [`Watts`],
//! [`crate::sci::EnergyKwh::from_draw`] turns a draw held over an interval
//! into energy, and [`crate::sci::SciEngine::sci_rate`] never learns where the
//! number came from.
//!
//! # Why the seam takes a zone list (decision D60)
//!
//! A source declares the zones it carries, and a request for a zone it does
//! not carry is [`TellusError::ZoneAbsent`]. This is not defensive
//! programming; it is the shape of the reference profile. `ls -d
//! /sys/class/powercap/*` on it yields exactly three entries -- `intel-rapl`,
//! `intel-rapl:0` whose `name` reads `package-0`, and `intel-rapl:0:0` whose
//! `name` reads `core` -- and no `dram` or `psys` zone exists at all. An
//! engine that assumed a DRAM domain would read a missing counter as zero
//! draw, which is a measurement claim rather than an absence, so the absence
//! is typed.
//!
//! [`RaplZone`] still *represents* `dram` and `psys`, because the powercap
//! vocabulary defines them and another machine has them. What the reference
//! profile fixes is which zones a source built for it declares:
//! [`REFERENCE_PROFILE_ZONES`].
//!
//! # Provenance travels with the number
//!
//! D60 also says DRAM energy is modelled and labelled as modelled, never
//! reported as measured. [`Provenance`] is that label, carried on every
//! [`ZoneSample`], and no value in this milestone carries
//! [`Provenance::Measured`].
//!
//! Two rules decide a label in [`SimulatedWattage::recorded`], and neither is
//! the figure alone. For a zone the profile exposes -- the two
//! [`RaplZone::on_reference_profile`] admits -- the label is the weaker of the
//! figure's ingredients: [`Provenance`] is ordered by how much a value may be
//! relied on, [`SliceTable::provenance`] takes the minimum for the same
//! reason, and `package-0` folds the modelled DRAM figure into a simulated
//! core constant, so it stays [`Provenance::Simulated`]. For a zone the
//! profile exposes no counter for, the label is [`Provenance::Modelled`] by
//! D60's definition of that variant, whatever figure stands in; that is why
//! `dram` and `psys` are both `Modelled`, and why `psys` is `Modelled` while
//! `package-0`, whose recorded constant is the same number, is not. Every
//! zone's label is pinned in both directions by `tests/power_seam.rs`.
//!
//! # What this module does not do
//!
//! It opens no file, reads no counter, loads no eBPF program and starts no
//! timer. `/sys/class/powercap` is named in this documentation and nowhere in
//! the code; `tests/stubbed_effects.rs` is the sweep that keeps it that way.

use core::fmt;
use core::num::NonZeroU32;

use crate::error::TellusError;
use crate::id::SliceName;
use crate::sci::{Seconds, Watts};

/// Scalar upper bound on monitored cgroup v2 slices.
///
/// 16, the scaffold's `MAX_CGROUP_SLICES`, recorded there against NASA rule 2.
pub const MAX_CGROUP_SLICES: usize = 16;

/// Scalar upper bound on the zones one wattage source may declare.
pub const MAX_RAPL_ZONES: usize = 4;

/// The default service time a simulated source declares, in milliseconds.
pub const SIMULATED_SERVICE_MILLIS: u32 = 1;

/// The zones the reference profile actually exposes.
///
/// Measured on 2026-09-13 with `ls -d /sys/class/powercap/*` and `cat
/// /sys/class/powercap/*/name`: `intel-rapl:0` is `package-0` and
/// `intel-rapl:0:0` is `core`. There is no third zone, so this array has two
/// entries and not four.
pub const REFERENCE_PROFILE_ZONES: [RaplZone; 2] = [RaplZone::Package0, RaplZone::Core];

/// The simulated `core` draw, in watts.
///
/// 20.8. The imported scaffold's `poll_ebpf_power_probes` simulates four
/// slices with a CPU draw of `2.5 + i * 1.8` watts, which sums to
/// `2.5 + 4.3 + 6.1 + 7.9`. That sum is the recorded simulated constant for
/// the core domain. It is simulated, not measured, and
/// [`Provenance::Simulated`] says so on every sample carrying it.
pub const SIMULATED_CORE_WATTS: f64 = 20.8;

/// The modelled DRAM draw, in watts.
///
/// 5.6, the scaffold's `0.8 + i * 0.4` over the same four slices. The
/// reference profile has no `dram` zone, so this number can never be measured
/// there; D60 says such a value is modelled and must be labelled as modelled,
/// which is why a `dram` sample carries [`Provenance::Modelled`] and no sample
/// carrying this figure is ever labelled [`Provenance::Measured`].
///
/// The figure is *also* folded into [`SIMULATED_PACKAGE0_WATTS`], and the
/// `package-0` sample carrying that composite is labelled
/// [`Provenance::Simulated`] rather than `Modelled`. For a zone the reference
/// profile exposes a counter for, the label is the weakest of the figure's
/// ingredients and not the strongest: `Simulated` ranks below `Modelled` in
/// [`Provenance`]'s ordering, [`SliceTable::provenance`] takes the minimum for
/// exactly that reason, and the package figure carries a simulated core
/// constant. Labelling that sample `Modelled` would claim more for it than its
/// weakest part supports.
///
/// The same composite reaches `psys`, which is labelled `Modelled` instead.
/// The profile exposes no `psys` counter, so that arm is decided by D60's
/// definition of the variant rather than by the figure's ingredients, and one
/// number is deliberately labelled two ways for two different zones.
/// `tests/power_seam.rs::the_recorded_provenance_of_every_zone_is_pinned`
/// holds every zone's label, so no direction can be changed silently.
pub const MODELLED_DRAM_WATTS: f64 = 5.6;

/// The simulated `package-0` draw, in watts.
///
/// 26.4: [`SIMULATED_CORE_WATTS`] plus [`MODELLED_DRAM_WATTS`], because the
/// package domain is the enclosing one and the scaffold models no other
/// contribution to it. A `package-0` sample carrying it is labelled
/// [`Provenance::Simulated`], the weaker of its two ingredients' labels.
///
/// [`SimulatedWattage::recorded`] hands the same figure back for `psys`,
/// because the package domain is the only model of the platform domain the
/// scaffold supplies. That sample is [`Provenance::Modelled`], because the
/// reference profile exposes no `psys` counter.
pub const SIMULATED_PACKAGE0_WATTS: f64 = SIMULATED_CORE_WATTS + MODELLED_DRAM_WATTS;

/// Where a wattage number came from.
///
/// The variants are ordered by how much they may be relied on, and the
/// ordering is part of the type: [`Self::Measured`] is the only one a hardware
/// claim may rest on, and no value this milestone produces carries it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Provenance {
    /// A recorded constant standing in for a measurement (milestone M05).
    Simulated,
    /// Derived from a model because the platform exposes no counter (D60).
    Modelled,
    /// Read from a hardware counter. Nothing in this crate produces one.
    Measured,
}

impl Provenance {
    /// Every provenance, weakest claim first.
    pub const ALL: [Self; 3] = [Self::Simulated, Self::Modelled, Self::Measured];

    /// Returns the stable name used in payloads and diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Simulated => "simulated",
            Self::Modelled => "modelled",
            Self::Measured => "measured",
        }
    }

    /// Returns `true` when the value may back a measurement claim.
    #[must_use]
    pub const fn is_measured(self) -> bool {
        matches!(self, Self::Measured)
    }
}

impl fmt::Display for Provenance {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A powercap energy zone, spelled as the `name` attribute spells it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum RaplZone {
    /// The socket package domain, `package-0`.
    Package0,
    /// The core subdomain, `core`.
    Core,
    /// The memory-controller domain, `dram`. Absent from the reference profile.
    Dram,
    /// The platform domain, `psys`. Absent from the reference profile.
    Psys,
}

impl RaplZone {
    /// Every zone the powercap vocabulary defines for this crate.
    pub const ALL: [Self; MAX_RAPL_ZONES] = [Self::Package0, Self::Core, Self::Dram, Self::Psys];

    /// Returns the zone name exactly as `/sys/class/powercap/*/name` spells it.
    ///
    /// Recorded, not read: nothing here opens that path.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Package0 => "package-0",
            Self::Core => "core",
            Self::Dram => "dram",
            Self::Psys => "psys",
        }
    }

    /// Returns `true` for a zone the reference profile exposes.
    #[must_use]
    pub const fn on_reference_profile(self) -> bool {
        matches!(self, Self::Package0 | Self::Core)
    }
}

impl fmt::Display for RaplZone {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A bounded, duplicate-free list of the zones one source carries.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ZoneList {
    zones: [Option<RaplZone>; MAX_RAPL_ZONES],
    len: usize,
}

impl ZoneList {
    /// Builds an empty list.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            zones: [None; MAX_RAPL_ZONES],
            len: 0,
        }
    }

    /// Builds the list the reference profile exposes: `package-0` and `core`.
    #[must_use]
    pub fn reference_profile() -> Self {
        let mut list = Self::new();
        for zone in REFERENCE_PROFILE_ZONES {
            if list.push(zone).is_err() {
                return list;
            }
        }
        list
    }

    /// Adds a zone.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::ZoneList`] at the bound and for a duplicate. A
    /// duplicate is refused rather than ignored: two entries for one counter
    /// would let a sum double-count a domain.
    pub fn push(&mut self, zone: RaplZone) -> Result<(), TellusError> {
        if self.contains(zone) {
            return Err(TellusError::ZoneList {
                reason: "the zone list already carries that zone",
            });
        }
        let slot = self.zones.get_mut(self.len).ok_or(TellusError::ZoneList {
            reason: "the zone list is full at its bound of four zones",
        })?;
        *slot = Some(zone);
        self.len = self.len.saturating_add(1);
        Ok(())
    }

    /// Returns `true` when the list carries `zone`.
    #[must_use]
    pub fn contains(&self, zone: RaplZone) -> bool {
        self.zones
            .iter()
            .take(self.len)
            .any(|slot| *slot == Some(zone))
    }

    /// Returns how many zones the list carries.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the list carries no zone.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the zones in the order they were added.
    #[must_use]
    pub fn zones(&self) -> [Option<RaplZone>; MAX_RAPL_ZONES] {
        self.zones
    }
}

/// The deadline within which a wattage sample must complete.
///
/// The parameter is in the trait signature so that the M21 reader, which does
/// open a file, cannot implement [`WattageSource`] without receiving one. That
/// is the HISS-02 obligation for I/O made structural rather than remembered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SampleDeadline(NonZeroU32);

impl SampleDeadline {
    /// Builds a deadline of `millis` milliseconds.
    #[must_use]
    pub const fn from_millis(millis: NonZeroU32) -> Self {
        Self(millis)
    }

    /// Builds a deadline of `millis` milliseconds, or `None` when it is zero.
    #[must_use]
    pub const fn try_from_millis(millis: u32) -> Option<Self> {
        match NonZeroU32::new(millis) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the deadline in milliseconds.
    #[must_use]
    pub const fn millis(self) -> u32 {
        self.0.get()
    }
}

/// One zone's draw, with the provenance of the number.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ZoneSample {
    /// The zone the draw is attributed to.
    pub zone: RaplZone,
    /// The draw.
    pub watts: Watts,
    /// Where the number came from.
    pub provenance: Provenance,
}

impl ZoneSample {
    /// Builds a sample.
    #[must_use]
    pub const fn new(zone: RaplZone, watts: Watts, provenance: Provenance) -> Self {
        Self {
            zone,
            watts,
            provenance,
        }
    }

    /// Converts the draw held over `interval` into energy.
    ///
    /// # Errors
    ///
    /// Propagates [`crate::sci::EnergyKwh::from_draw`].
    pub fn energy(&self, interval: Seconds) -> Result<crate::sci::EnergyKwh, TellusError> {
        crate::sci::EnergyKwh::from_draw(self.watts, interval)
    }
}

/// The seam the SCI arithmetic reads its draw through.
///
/// Milestone M05 ships [`SimulatedWattage`] only. M21 adds a reader over the
/// powercap energy counters and substitutes it here; nothing in
/// [`crate::sci`] changes when it does.
pub trait WattageSource {
    /// Returns the zones this source carries.
    fn zones(&self) -> ZoneList;

    /// Returns the service time this source demands of a caller's deadline.
    fn service_time(&self) -> SampleDeadline;

    /// Samples one zone.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::ZoneAbsent`] for a zone this source does not
    /// carry, and [`TellusError::WouldBlock`] when `deadline` is shorter than
    /// [`Self::service_time`].
    fn sample(&self, zone: RaplZone, deadline: SampleDeadline) -> Result<ZoneSample, TellusError>;
}

/// The recorded simulated source: constants, no counter.
///
/// It declares the zones it was built for, so a [`ZoneList`] without
/// [`RaplZone::Dram`] refuses a DRAM sample exactly as the reference profile
/// would.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SimulatedWattage {
    zones: ZoneList,
    service: SampleDeadline,
}

impl Default for SimulatedWattage {
    fn default() -> Self {
        Self::reference_profile()
    }
}

impl SimulatedWattage {
    /// Builds the source the reference profile supports: `package-0` and `core`.
    #[must_use]
    pub fn reference_profile() -> Self {
        Self::with_zones(ZoneList::reference_profile())
    }

    /// Builds a source over an explicit zone list.
    #[must_use]
    pub fn with_zones(zones: ZoneList) -> Self {
        Self::with_service_time(zones, default_service_time())
    }

    /// Builds a source over an explicit zone list and service time.
    ///
    /// A simulated source needs no time at all, so the default is one
    /// millisecond and the parameter exists for two reasons: it makes the
    /// [`TellusError::WouldBlock`] path reachable and therefore testable, and
    /// it is where the M21 reader declares what a privileged `energy_uj` read
    /// actually costs.
    #[must_use]
    pub const fn with_service_time(zones: ZoneList, service: SampleDeadline) -> Self {
        Self { zones, service }
    }

    /// Returns the recorded constant for `zone`, and how it was arrived at.
    ///
    /// The pair is public so a test can state the constant it expects without
    /// going through the deadline check.
    #[must_use]
    pub const fn recorded(zone: RaplZone) -> (f64, Provenance) {
        match zone {
            RaplZone::Package0 => (SIMULATED_PACKAGE0_WATTS, Provenance::Simulated),
            RaplZone::Core => (SIMULATED_CORE_WATTS, Provenance::Simulated),
            RaplZone::Dram => (MODELLED_DRAM_WATTS, Provenance::Modelled),
            RaplZone::Psys => (SIMULATED_PACKAGE0_WATTS, Provenance::Modelled),
        }
    }

    /// Refuses a deadline shorter than the declared service time.
    const fn admit(&self, deadline: SampleDeadline) -> Result<(), TellusError> {
        if deadline.millis() < self.service.millis() {
            return Err(TellusError::WouldBlock {
                needed: self.service.millis(),
                offered: deadline.millis(),
            });
        }
        Ok(())
    }
}

/// Returns the default service time, one millisecond.
fn default_service_time() -> SampleDeadline {
    SampleDeadline::try_from_millis(SIMULATED_SERVICE_MILLIS)
        .unwrap_or(SampleDeadline::from_millis(NonZeroU32::MIN))
}

impl WattageSource for SimulatedWattage {
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
        self.admit(deadline)?;
        let (watts, provenance) = Self::recorded(zone);
        Ok(ZoneSample::new(zone, Watts::new(watts)?, provenance))
    }
}

/// One cgroup slice's attributed draw.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliceDraw {
    /// The slice the draw is attributed to.
    pub slice: SliceName,
    /// The draw attributed to it.
    pub watts: Watts,
    /// Where the number came from.
    pub provenance: Provenance,
}

impl SliceDraw {
    /// Builds an attributed draw.
    #[must_use]
    pub const fn new(slice: SliceName, watts: Watts, provenance: Provenance) -> Self {
        Self {
            slice,
            watts,
            provenance,
        }
    }
}

/// A fixed table of at most [`MAX_CGROUP_SLICES`] attributed draws.
///
/// The table is an array, so recording a slice allocates nothing and the bound
/// is the array length rather than a check that could be forgotten. A push at
/// the bound is refused; nothing is overwritten and nothing wraps.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SliceTable {
    slices: [Option<SliceDraw>; MAX_CGROUP_SLICES],
    len: usize,
}

impl Default for SliceTable {
    fn default() -> Self {
        Self::new()
    }
}

impl SliceTable {
    /// Builds an empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slices: [None; MAX_CGROUP_SLICES],
            len: 0,
        }
    }

    /// Records one slice's attributed draw.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::SliceTableFull`] at [`MAX_CGROUP_SLICES`].
    pub fn push(&mut self, draw: SliceDraw) -> Result<(), TellusError> {
        let slot = self
            .slices
            .get_mut(self.len)
            .ok_or(TellusError::SliceTableFull {
                bound: MAX_CGROUP_SLICES,
            })?;
        *slot = Some(draw);
        self.len = self.len.saturating_add(1);
        Ok(())
    }

    /// Returns how many slices the table holds.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the table holds no slice.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the scalar bound.
    #[must_use]
    pub const fn bound(&self) -> usize {
        MAX_CGROUP_SLICES
    }

    /// Returns the recorded draw for `slice`, when the table holds one.
    #[must_use]
    pub fn get(&self, slice: SliceName) -> Option<SliceDraw> {
        self.slices
            .iter()
            .take(self.len)
            .flatten()
            .find(|draw| draw.slice == slice)
            .copied()
    }

    /// Returns the total attributed draw.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Wattage`] when the sum leaves the admitted
    /// range, which is why the total is a `Result` rather than a bare sum.
    pub fn total(&self) -> Result<Watts, TellusError> {
        let mut total = Watts::ZERO;
        for draw in self.slices.iter().take(self.len).flatten() {
            total = total.checked_add(draw.watts)?;
        }
        Ok(total)
    }

    /// Returns the weakest provenance any recorded draw carries.
    ///
    /// A total is only as good as its worst input, so a table holding one
    /// modelled draw reports `Modelled` however many measured ones sit beside
    /// it. An empty table reports `Simulated`: it has measured nothing.
    #[must_use]
    pub fn provenance(&self) -> Provenance {
        self.slices
            .iter()
            .take(self.len)
            .flatten()
            .map(|draw| draw.provenance)
            .min()
            .unwrap_or(Provenance::Simulated)
    }
}
