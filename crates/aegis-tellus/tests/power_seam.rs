// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The wattage seam: the recorded simulated constant, the zone list, and
//! decision D60.
//!
//! This file holds the two exit criteria M05 records about the seam. The first
//! is that the draw sits behind a boundary with a recorded constant, so M21 can
//! substitute a measured RAPL delta without touching the arithmetic. The second
//! is that the boundary takes a zone list, because the reference profile
//! exposes `package-0` and `core` and has no `dram` or `psys` zone at all.
//!
//! # What the D60 tests are, and are not
//!
//! They check the seam's behaviour against a recorded enumeration of two
//! zones. They are **not** a measurement of the host this suite runs on:
//! nothing here reads `/sys/class/powercap`, so a machine with a DRAM zone
//! would pass these tests unchanged. The measurement is in
//! `crates/aegis-tellus/src/register.rs`, with the command that produced it.

mod common;

use aegis_tellus::{
    MAX_RAPL_ZONES, MODELLED_DRAM_WATTS, Provenance, REFERENCE_PROFILE_ZONES, RaplZone,
    SIMULATED_CORE_WATTS, SIMULATED_PACKAGE0_WATTS, SIMULATED_SERVICE_MILLIS, SampleDeadline,
    Seconds, SimulatedWattage, TellusError, WattageSource, ZoneList,
};

use common::{EPSILON, Fallible, deadline};

// --- Positive -------------------------------------------------------------

/// Positive: the seam hands back the recorded constant for each zone it carries.
///
/// The two constants are derived from the imported scaffold's simulated
/// per-slice draws: the core figure is its four CPU draws summed
/// (`2.5 + 4.3 + 6.1 + 7.9`), and the package figure adds its four DRAM draws
/// (`0.8 + 1.2 + 1.6 + 2.0`), which have no zone of their own on the reference
/// profile.
#[test]
fn the_seam_hands_back_the_recorded_constant() -> Fallible {
    let source = SimulatedWattage::reference_profile();
    let deadline = deadline()?;

    let core = source.sample(RaplZone::Core, deadline)?;
    assert_eq!(core.zone, RaplZone::Core);
    assert!((core.watts.get() - SIMULATED_CORE_WATTS).abs() < EPSILON);
    assert!(
        (SIMULATED_CORE_WATTS - 20.8).abs() < EPSILON,
        "the recorded core constant must be the scaffold's CPU sum, 20.8 W"
    );

    let package = source.sample(RaplZone::Package0, deadline)?;
    assert!((package.watts.get() - SIMULATED_PACKAGE0_WATTS).abs() < EPSILON);
    assert!(
        (SIMULATED_PACKAGE0_WATTS - 26.4).abs() < EPSILON,
        "the recorded package constant must be 26.4 W"
    );
    assert!(
        (SIMULATED_PACKAGE0_WATTS - SIMULATED_CORE_WATTS - MODELLED_DRAM_WATTS).abs() < EPSILON,
        "the package constant must be the core constant plus the modelled DRAM figure"
    );
    Ok(())
}

/// Positive: a sample converts to energy without the arithmetic knowing where
/// it came from.
///
/// This is the substitution property the M21 criterion rests on: the only
/// thing the arithmetic receives is a draw and an interval.
#[test]
fn a_sample_converts_to_energy_through_the_same_call() -> Fallible {
    let source = SimulatedWattage::reference_profile();
    let sample = source.sample(RaplZone::Package0, deadline()?)?;
    let energy = sample.energy(Seconds::new(3600.0)?)?;
    assert!(
        (energy.get() - SIMULATED_PACKAGE0_WATTS / 1000.0).abs() < EPSILON,
        "one hour at {SIMULATED_PACKAGE0_WATTS} W is {SIMULATED_PACKAGE0_WATTS} Wh"
    );
    Ok(())
}

/// Positive: the zone vocabulary spells each zone as powercap spells it.
#[test]
fn the_zone_names_are_the_powercap_names() {
    assert_eq!(RaplZone::Package0.name(), "package-0");
    assert_eq!(RaplZone::Core.name(), "core");
    assert_eq!(RaplZone::Dram.name(), "dram");
    assert_eq!(RaplZone::Psys.name(), "psys");
    assert_eq!(RaplZone::ALL.len(), MAX_RAPL_ZONES);
    assert_eq!(format!("{}", RaplZone::Core), "core");
}

/// Positive: the vocabulary records which zones the reference profile has.
///
/// Recorded, not read: this is the enumeration D60 settled, and nothing here
/// asks the machine the suite is running on.
#[test]
fn the_vocabulary_records_which_zones_the_profile_has() {
    assert!(RaplZone::Package0.on_reference_profile());
    assert!(RaplZone::Core.on_reference_profile());
    assert!(!RaplZone::Dram.on_reference_profile());
    assert!(!RaplZone::Psys.on_reference_profile());
}

// --- Negative -------------------------------------------------------------

/// Negative: a zone the source does not carry is refused, not answered zero.
///
/// This is decision D60. A silent zero would read downstream as "the DRAM
/// domain drew no power" rather than "there is no DRAM domain", and the two
/// are different claims about the machine.
#[test]
fn a_zone_the_source_does_not_carry_is_refused() -> Fallible {
    let source = SimulatedWattage::reference_profile();
    let deadline = deadline()?;
    for absent in [RaplZone::Dram, RaplZone::Psys] {
        let refusal = source.sample(absent, deadline);
        assert_eq!(
            refusal,
            Err(TellusError::ZoneAbsent {
                zone: absent.name(),
                carried: 2,
            }),
            "{absent} must be refused explicitly, not answered with zero"
        );
    }
    Ok(())
}

/// Negative: a deadline shorter than the declared service time is refused.
///
/// HISS-02 asks for a deadline on every I/O-shaped call. The obligation is
/// only real if a source can demand more time than a caller offers, so the
/// source declares a service time and this is the refusal it produces.
#[test]
fn a_deadline_shorter_than_the_service_time_is_refused() -> Fallible {
    let service = SampleDeadline::try_from_millis(10).ok_or("a service time must be positive")?;
    let slow = SimulatedWattage::with_service_time(ZoneList::reference_profile(), service);
    assert_eq!(slow.service_time().millis(), 10);

    let offered = SampleDeadline::try_from_millis(9).ok_or("a deadline must be positive")?;
    assert_eq!(
        slow.sample(RaplZone::Core, offered),
        Err(TellusError::WouldBlock {
            needed: 10,
            offered: 9,
        })
    );
    assert!(
        slow.sample(RaplZone::Core, service).is_ok(),
        "exactly the service time is enough"
    );

    let quick = SimulatedWattage::default();
    assert_eq!(quick.service_time().millis(), SIMULATED_SERVICE_MILLIS);
    assert!(quick.sample(RaplZone::Core, quick.service_time()).is_ok());
    Ok(())
}

/// Negative: a zone list refuses a duplicate and refuses to overflow.
#[test]
fn a_zone_list_refuses_a_duplicate_and_an_overflow() -> Fallible {
    let mut list = ZoneList::new();
    assert!(list.is_empty());
    list.push(RaplZone::Package0)?;
    assert!(
        list.push(RaplZone::Package0).is_err(),
        "a duplicate zone would let a sum double-count a domain"
    );
    assert_eq!(list.len(), 1);

    for zone in [RaplZone::Core, RaplZone::Dram, RaplZone::Psys] {
        list.push(zone)?;
    }
    assert_eq!(list.len(), MAX_RAPL_ZONES);
    assert!(list.push(RaplZone::Package0).is_err());
    Ok(())
}

/// Negative: nothing this milestone produces claims to be measured.
#[test]
fn no_sample_this_milestone_produces_is_measured() -> Fallible {
    let mut all = ZoneList::new();
    for zone in RaplZone::ALL {
        all.push(zone)?;
    }
    let source = SimulatedWattage::with_zones(all);
    let deadline = deadline()?;
    for zone in RaplZone::ALL {
        let sample = source.sample(zone, deadline)?;
        assert!(
            !sample.provenance.is_measured(),
            "{zone} reported {} provenance; no M05 value may be measured",
            sample.provenance
        );
    }
    assert!(Provenance::Measured.is_measured());
    assert_eq!(Provenance::ALL.len(), 3);
    assert_eq!(Provenance::Simulated.name(), "simulated");
    assert_eq!(format!("{}", Provenance::Modelled), "modelled");
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the reference-profile zone list is exactly the two zones D60
/// records, and the `dram` zone's figure is labelled modelled.
#[test]
fn the_reference_profile_carries_exactly_two_zones() {
    assert_eq!(
        REFERENCE_PROFILE_ZONES,
        [RaplZone::Package0, RaplZone::Core],
        "D60 records package-0 and core, and no third zone"
    );
    let list = ZoneList::reference_profile();
    assert_eq!(list.len(), 2);
    assert!(list.contains(RaplZone::Package0));
    assert!(list.contains(RaplZone::Core));
    assert!(!list.contains(RaplZone::Dram));
    assert!(!list.contains(RaplZone::Psys));
    assert_eq!(list.zones().len(), MAX_RAPL_ZONES);

    let (watts, provenance) = SimulatedWattage::recorded(RaplZone::Dram);
    assert!((watts - MODELLED_DRAM_WATTS).abs() < EPSILON);
    assert_eq!(
        provenance,
        Provenance::Modelled,
        "D60: DRAM energy is modelled and labelled as modelled"
    );
}

/// Boundary: a source built over an empty zone list carries nothing, and says
/// so for every zone rather than for some of them.
#[test]
fn an_empty_zone_list_carries_nothing() -> Fallible {
    let source = SimulatedWattage::with_zones(ZoneList::default());
    assert!(source.zones().is_empty());
    for zone in RaplZone::ALL {
        assert_eq!(
            source.sample(zone, deadline()?),
            Err(TellusError::ZoneAbsent {
                zone: zone.name(),
                carried: 0,
            })
        );
    }
    Ok(())
}

/// Boundary: a source given a DRAM zone answers for it, which is what makes
/// the reference profile's refusal a property of the list rather than of the
/// engine.
///
/// M21 substitutes a reader whose zone list comes from the machine. On a host
/// that does expose `dram`, that reader carries it and this is the path that
/// runs; the engine is unchanged either way.
#[test]
fn a_source_that_carries_dram_answers_for_it() -> Fallible {
    let mut zones = ZoneList::new();
    zones.push(RaplZone::Package0)?;
    zones.push(RaplZone::Dram)?;
    let source = SimulatedWattage::with_zones(zones);

    let sample = source.sample(RaplZone::Dram, deadline()?)?;
    assert!((sample.watts.get() - MODELLED_DRAM_WATTS).abs() < EPSILON);
    assert_eq!(sample.provenance, Provenance::Modelled);
    assert!(
        source.sample(RaplZone::Core, deadline()?).is_err(),
        "a list without core still refuses core"
    );
    Ok(())
}

/// Boundary: a deadline of zero is not representable, and one millisecond is.
#[test]
fn the_deadline_turns_at_one_millisecond() {
    assert!(SampleDeadline::try_from_millis(0).is_none());
    let one = SampleDeadline::try_from_millis(1);
    assert!(one.is_some());
    assert_eq!(one.map(SampleDeadline::millis), Some(1));
    assert_eq!(SIMULATED_SERVICE_MILLIS, 1);
}

/// Boundary: every zone's recorded provenance label is pinned, in both
/// directions.
///
/// The labels are not free-floating, and one rule does not cover all four
/// arms. [`Provenance`] is ordered by how much a number may be relied on. For
/// a zone the reference profile exposes, the label is the weaker of the
/// figure's ingredients -- the rule `SliceTable::provenance` implements by
/// taking the minimum, exercised in `slice_table.rs` -- so `package-0`, which
/// folds the modelled DRAM figure into a simulated core constant, is
/// `Simulated` and not `Modelled`: calling it `Modelled` would claim more for
/// the sum than its core half supports. For a zone the profile exposes no
/// counter for, the label is `Modelled` by D60's definition of that variant
/// whatever figure stands in, so `dram` is `Modelled` and so is `psys`, which
/// carries the `package-0` figure because the package domain is the only model
/// of the platform domain the scaffold supplies.
///
/// This is the falsifier for that sentence. Changing any arm of
/// `SimulatedWattage::recorded` in either direction fails here, which the
/// `no_sample_this_milestone_produces_is_measured` sweep above cannot do: it
/// refuses `Measured` and is indifferent between the other two.
#[test]
fn the_recorded_provenance_of_every_zone_is_pinned() -> Fallible {
    let expected = [
        (
            RaplZone::Package0,
            SIMULATED_PACKAGE0_WATTS,
            Provenance::Simulated,
        ),
        (RaplZone::Core, SIMULATED_CORE_WATTS, Provenance::Simulated),
        (RaplZone::Dram, MODELLED_DRAM_WATTS, Provenance::Modelled),
        (
            RaplZone::Psys,
            SIMULATED_PACKAGE0_WATTS,
            Provenance::Modelled,
        ),
    ];
    assert_eq!(expected.len(), RaplZone::ALL.len(), "a zone is unpinned");

    let mut all = ZoneList::new();
    for zone in RaplZone::ALL {
        all.push(zone)?;
    }
    let source = SimulatedWattage::with_zones(all);
    let deadline = deadline()?;
    for (zone, watts, provenance) in expected {
        let (recorded_watts, recorded_provenance) = SimulatedWattage::recorded(zone);
        assert!(
            (recorded_watts - watts).abs() < EPSILON,
            "{zone} carries the wrong recorded constant"
        );
        assert_eq!(
            recorded_provenance, provenance,
            "{zone} carries the wrong recorded provenance"
        );
        let sample = source.sample(zone, deadline)?;
        assert_eq!(
            sample.provenance, provenance,
            "a {zone} sample must carry the recorded provenance"
        );
    }

    assert!(
        Provenance::Simulated < Provenance::Modelled,
        "an exposed zone's weakest-ingredient rule needs Simulated below Modelled"
    );
    assert!(Provenance::Modelled < Provenance::Measured);
    Ok(())
}
