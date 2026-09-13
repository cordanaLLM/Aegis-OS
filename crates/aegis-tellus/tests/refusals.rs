// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Every P13 refusal is a value, and every refusal variant has a producer.
//!
//! HISS-07 asks that an error path return rather than abort. The imported
//! scaffold has no error path at all -- `calculate_sci_rate` takes bare `f64`s
//! and returns a bare struct -- so this file is where the vocabulary that
//! replaced it is exercised.
//!
//! A refusal variant with no producer would be dead surface dressed as a
//! guarantee, so each of the nine is raised here from the call that raises it.

mod common;

use core::num::NonZeroU32;

use aegis_tellus::{
    CandidateId, CorrelationId, EmbodiedCarbon, EnergyKwh, GridIntensity, MAX_CGROUP_SLICES,
    MAX_IDENTIFIER_LEN, Provenance, RaplZone, SampleDeadline, Seconds, SimulatedWattage, SliceDraw,
    SliceName, SliceTable, TellusError, WattageSource, Watts, ZoneList,
};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: every refusal renders a message that names what was refused.
#[test]
fn every_refusal_renders_a_message() {
    let refusals = [
        TellusError::Energy {
            reason: "a negative value",
        },
        TellusError::GridIntensity {
            reason: "a negative value",
        },
        TellusError::EmbodiedCarbon {
            reason: "a negative value",
        },
        TellusError::Wattage {
            reason: "a negative value",
        },
        TellusError::Interval {
            reason: "a value that is not positive",
        },
        TellusError::ZoneAbsent {
            zone: "dram",
            carried: 2,
        },
        TellusError::ZoneList {
            reason: "the zone list is full at its bound of four zones",
        },
        TellusError::SliceTableFull {
            bound: MAX_CGROUP_SLICES,
        },
        TellusError::WouldBlock {
            needed: 10,
            offered: 9,
        },
        TellusError::Identifier {
            reason: CandidateId::REFUSAL,
        },
    ];
    for refusal in refusals {
        let rendered = format!("{refusal}");
        assert!(
            rendered.len() > 10,
            "the refusal {refusal:?} renders too little"
        );
    }
}

/// Positive: each identifier type states its own refusal text.
#[test]
fn each_identifier_states_its_own_refusal() {
    assert!(CorrelationId::REFUSAL.contains("correlation identifier"));
    assert!(CandidateId::REFUSAL.contains("candidate identifier"));
    assert!(SliceName::REFUSAL.contains("cgroup slice name"));
    assert_ne!(CorrelationId::REFUSAL, CandidateId::REFUSAL);
    assert_ne!(CandidateId::REFUSAL, SliceName::REFUSAL);
}

/// Positive: two admissible draws add to their sum.
#[test]
fn two_admissible_draws_add() -> Fallible {
    let total = Watts::new(20.8)?.checked_add(Watts::new(5.6)?)?;
    assert!((total.get() - 26.4).abs() < 1.0e-12);
    assert_eq!(Watts::ZERO.checked_add(Watts::ZERO)?, Watts::ZERO);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: each of the six quantity refusals has a producer.
///
/// This and the two tests after it enumerate ten variants between them. That
/// is a check over an enumeration, not a proof that every refusal the crate
/// can ever raise is reachable. What it catches is a variant added to the
/// vocabulary with nothing behind it.
#[test]
fn every_quantity_refusal_has_a_producer() {
    let raised = [
        matches!(EnergyKwh::new(-1.0), Err(TellusError::Energy { .. })),
        matches!(
            GridIntensity::new(f64::NAN),
            Err(TellusError::GridIntensity { .. })
        ),
        matches!(
            EmbodiedCarbon::new(-0.1),
            Err(TellusError::EmbodiedCarbon { .. })
        ),
        matches!(Watts::new(f64::INFINITY), Err(TellusError::Wattage { .. })),
        matches!(Seconds::new(0.0), Err(TellusError::Interval { .. })),
        matches!(
            CorrelationId::parse(""),
            Err(TellusError::Identifier { .. })
        ),
    ];
    assert_eq!(
        raised, [true; 6],
        "one of the six quantity refusals has no producer"
    );
}

/// Negative: each of the three seam refusals has a producer.
#[test]
fn every_seam_refusal_has_a_producer() -> Fallible {
    let deadline = SampleDeadline::from_millis(NonZeroU32::new(5).ok_or("five is not zero")?);
    let source = SimulatedWattage::reference_profile();
    assert!(matches!(
        source.sample(RaplZone::Dram, deadline),
        Err(TellusError::ZoneAbsent { .. })
    ));

    let mut zones = ZoneList::reference_profile();
    assert!(matches!(
        zones.push(RaplZone::Core),
        Err(TellusError::ZoneList { .. })
    ));

    let service = SampleDeadline::from_millis(NonZeroU32::new(10).ok_or("ten is not zero")?);
    let slow = SimulatedWattage::with_service_time(ZoneList::reference_profile(), service);
    assert!(matches!(
        slow.sample(RaplZone::Core, deadline),
        Err(TellusError::WouldBlock { .. })
    ));
    Ok(())
}

/// Negative: the slice-table refusal has a producer.
#[test]
fn the_slice_table_refusal_has_a_producer() -> Fallible {
    let mut table = SliceTable::new();
    for index in 0..MAX_CGROUP_SLICES {
        table.push(SliceDraw::new(
            SliceName::parse(&format!("s{index}"))?,
            Watts::new(1.0)?,
            Provenance::Simulated,
        ))?;
    }
    assert!(matches!(
        table.push(SliceDraw::new(
            SliceName::parse("overflow")?,
            Watts::ZERO,
            Provenance::Simulated,
        )),
        Err(TellusError::SliceTableFull { .. })
    ));
    Ok(())
}

/// Negative: a sum past the wattage ceiling is a refusal, not a saturation.
#[test]
fn a_sum_past_the_ceiling_is_refused() -> Fallible {
    let ceiling = Watts::new(Watts::MAX)?;
    assert!(matches!(
        ceiling.checked_add(Watts::new(1.0)?),
        Err(TellusError::Wattage { .. })
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: an identifier refuses at the empty string and past the bound, and
/// accepts the longest admissible one.
#[test]
fn the_identifier_length_bound_turns_at_both_ends() -> Fallible {
    assert!(CorrelationId::parse("").is_err());
    assert!(CandidateId::parse("").is_err());
    assert!(SliceName::parse("").is_err());

    let longest = "c".repeat(MAX_IDENTIFIER_LEN);
    assert_eq!(CandidateId::parse(&longest)?.len(), MAX_IDENTIFIER_LEN);
    let over = "c".repeat(MAX_IDENTIFIER_LEN.saturating_add(1));
    assert!(CandidateId::parse(&over).is_err());
    Ok(())
}

/// Boundary: the identifier charset admits exactly the bytes it records.
#[test]
fn the_identifier_charset_turns_at_its_recorded_set() {
    for outside in ["a b", "a\tb", "a\"b", "a\nb", "a{b", "a,b", "a\\b"] {
        assert!(
            CandidateId::parse(outside).is_err(),
            "the charset accepted {outside:?}"
        );
    }
    for inside in ["a.b", "a-b", "a_b", "a:b", "a@b", "a/b", "A0"] {
        assert!(
            CandidateId::parse(inside).is_ok(),
            "the charset refused {inside:?}"
        );
    }
}

/// Boundary: a deadline built from a non-zero value carries exactly it.
#[test]
fn a_deadline_carries_exactly_what_it_was_built_from() -> Fallible {
    let one = SampleDeadline::from_millis(NonZeroU32::MIN);
    assert_eq!(one.millis(), 1);
    let large = SampleDeadline::from_millis(NonZeroU32::new(60_000).ok_or("not zero")?);
    assert_eq!(large.millis(), 60_000);
    assert!(one < large);
    Ok(())
}
