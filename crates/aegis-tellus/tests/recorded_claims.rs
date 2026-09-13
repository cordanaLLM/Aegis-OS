// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The recorded-claims register: well formed, cited, and honest about status.
//!
//! What this file checks is the shape of the register, not the truth of what
//! it records. A row with [`ClaimStatus::Recorded`] was read out of a source
//! and nothing here re-reads the source; a row with [`ClaimStatus::Measured`]
//! was observed on the reference profile with the command the row names, and
//! nothing here re-runs the command. Saying otherwise would be the category
//! error this register exists to avoid.

mod common;

use aegis_tellus::{
    ClaimSource, ClaimStatus, P13_RECORDED_CLAIMS, REFERENCE_ENERGY_UJ_MODE,
    REFERENCE_MAX_ENERGY_RANGE_UJ, RecordedClaim,
};

use common::Fallible;

/// Returns the row recorded for `requirement`, when there is one.
fn row(requirement: &str) -> Option<RecordedClaim> {
    P13_RECORDED_CLAIMS
        .into_iter()
        .find(|row| row.requirement == requirement)
}

// --- Positive -------------------------------------------------------------

/// Positive: every recorded claim names a requirement, a source and a citation.
#[test]
fn every_recorded_claim_is_cited() {
    assert_eq!(P13_RECORDED_CLAIMS.len(), 5);
    for row in P13_RECORDED_CLAIMS {
        assert!(!row.requirement.is_empty(), "a claim names no requirement");
        assert!(
            row.claim.len() > 20,
            "a claim of {:?} says too little",
            row.claim
        );
        assert!(!row.citation.is_empty(), "a claim carries no citation");
        assert!(
            row.requirement.starts_with("REQ-") || row.requirement.starts_with('D'),
            "{} is neither a requirement nor a decision",
            row.requirement
        );
    }
}

/// Positive: the D60 row is the measured one, and it names the command.
#[test]
fn the_zone_enumeration_is_recorded_as_measured() -> Fallible {
    let row = row("D60").ok_or("the register must record D60")?;
    assert_eq!(row.requirement, "D60");
    assert_eq!(row.source, ClaimSource::ReferenceProfile);
    assert_eq!(row.status, ClaimStatus::Measured);
    assert!(row.citation.contains("/sys/class/powercap"));
    assert!(row.claim.contains("package-0"));
    assert!(row.claim.contains("core"));
    assert!(
        row.claim.contains("no dram"),
        "the measured row must state the absence, not only the presence"
    );
    Ok(())
}

/// Positive: the M21 figures are recorded with enough precision to test against.
#[test]
fn the_m21_rollover_figures_are_recorded() {
    assert_eq!(REFERENCE_MAX_ENERGY_RANGE_UJ, 65_532_610_987);
    assert_eq!(REFERENCE_ENERGY_UJ_MODE, 0o400);
}

// --- Negative -------------------------------------------------------------

/// Negative: no row claims to be a measurement that this crate could not make.
///
/// The crate reads no counter, so only the reference-profile row may carry
/// [`ClaimStatus::Measured`]. An export row that claimed it would be asserting
/// a measurement out of a document.
#[test]
fn only_the_reference_profile_row_claims_a_measurement() {
    for row in P13_RECORDED_CLAIMS {
        if row.status == ClaimStatus::Measured {
            assert_eq!(
                row.source,
                ClaimSource::ReferenceProfile,
                "{} claims a measurement from a {} source",
                row.requirement,
                row.source.name()
            );
        }
    }
}

/// Negative: the eBPF row says the probe is not compiled or loaded here.
#[test]
fn the_ebpf_row_disclaims_the_probe() -> Fallible {
    let row = row("REQ-P13-02").ok_or("the register must record REQ-P13-02")?;
    assert!(
        row.claim.contains("no probe is compiled or loaded here"),
        "the eBPF row must disclaim what it does not do"
    );
    assert_eq!(row.status, ClaimStatus::Recorded);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the register has no duplicate requirement and no empty status.
#[test]
fn the_register_has_no_duplicate_requirement() {
    let mut seen: Vec<&str> = P13_RECORDED_CLAIMS
        .into_iter()
        .map(|row| row.requirement)
        .collect();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), before, "a requirement is recorded twice");

    for status in [
        ClaimStatus::Recorded,
        ClaimStatus::Measured,
        ClaimStatus::Held,
    ] {
        assert!(!status.name().is_empty());
    }
    for source in [
        ClaimSource::Export,
        ClaimSource::Decision,
        ClaimSource::ReferenceProfile,
    ] {
        assert!(!source.name().is_empty());
    }
}

/// Boundary: exactly one row is held by a test in this crate, and it is the
/// formula.
///
/// The register is an enumeration of five rows, not a survey. This states which
/// single claim the crate's own tests carry, so a reader does not take the
/// register for a coverage report.
#[test]
fn exactly_one_row_is_held_by_this_crate() {
    let held: Vec<&str> = P13_RECORDED_CLAIMS
        .into_iter()
        .filter(|row| row.status == ClaimStatus::Held)
        .map(|row| row.requirement)
        .collect();
    assert_eq!(held, vec!["REQ-P13-01"]);
}
