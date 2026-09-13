// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The D02 decision record and the P16 claims this crate cannot check.
//!
//! What this file checks is the shape of the register, not the truth of what
//! it records. A row with [`ClaimStatus::Recorded`] was read out of a source
//! and nothing here re-reads the source; a row with [`ClaimStatus::Deferred`]
//! names the milestone that will take it up. Saying otherwise would be the
//! category error this register exists to avoid.

mod common;

use aegis_athena::{
    Citation, ClaimStatus, DecisionRecord, DecisionState, LEDGER_ALGORITHM, LedgerAlgorithmReading,
    P16_RECORDED_CLAIMS, RecordedClaim, SANDBOX_DEFERRED_TO,
};
use aegis_justitia::HashAlgorithm;

use common::Fallible;

/// Returns the row recorded for `requirement`, when there is one.
fn row(requirement: &str) -> Option<RecordedClaim> {
    P16_RECORDED_CLAIMS
        .into_iter()
        .find(|row| row.requirement == requirement)
}

// --- Positive -------------------------------------------------------------

/// Positive: the D02 record names the three readings and the one chosen.
#[test]
fn the_decision_record_names_three_readings_and_one_choice() {
    let record: DecisionRecord = LEDGER_ALGORITHM;
    assert_eq!(record.state, DecisionState::Closed);
    assert_eq!(record.state.name(), "closed");
    assert_eq!(DecisionState::Unresolved.name(), "unresolved");
    assert_eq!(record.recorded_on, "2026-09-13");
    assert_eq!(record.citations.len(), 3);

    let readings: Vec<LedgerAlgorithmReading> = record
        .citations
        .into_iter()
        .map(|citation: Citation| citation.reading)
        .collect();
    assert_eq!(
        readings,
        vec![
            LedgerAlgorithmReading::Sha256,
            LedgerAlgorithmReading::Blake3,
            LedgerAlgorithmReading::Md5,
        ],
        "the register states the disagreement, not only its resolution"
    );
    assert_eq!(record.chosen, LedgerAlgorithmReading::Sha256);
    assert_eq!(record.admitted, HashAlgorithm::Sha256);
}

/// Positive: every citation names a requirement and a source.
#[test]
fn every_citation_names_a_requirement_and_a_source() {
    for citation in LEDGER_ALGORITHM.citations {
        assert!(
            citation.requirement.starts_with("REQ-"),
            "{} is not a requirement identifier",
            citation.requirement
        );
        assert!(
            citation.source.starts_with("export-"),
            "{} is not an export citation",
            citation.source
        );
        assert!(!citation.reading.name().is_empty());
    }
}

/// Positive: every recorded claim names a requirement, a claim and a citation.
#[test]
fn every_recorded_claim_is_cited() {
    assert_eq!(P16_RECORDED_CLAIMS.len(), 5);
    for entry in P16_RECORDED_CLAIMS {
        assert!(entry.requirement.starts_with("REQ-"));
        assert!(
            entry.claim.len() > 20,
            "a claim of {:?} says too little",
            entry.claim
        );
        assert!(entry.citation.starts_with("export-"));
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the two rejected readings are recorded as rejected.
///
/// BLAKE3 and MD5 stay representable so the register states a choice between
/// three readings rather than asserting the only one it can spell. Neither is
/// the chosen one, and neither has an implementation.
#[test]
fn the_rejected_readings_are_recorded_as_rejected() {
    assert!(!LedgerAlgorithmReading::Blake3.is_chosen());
    assert!(!LedgerAlgorithmReading::Md5.is_chosen());
    assert!(LedgerAlgorithmReading::Sha256.is_chosen());
    assert_eq!(LedgerAlgorithmReading::Blake3.name(), "BLAKE3");
    assert_eq!(LedgerAlgorithmReading::Md5.name(), "MD5");
    assert_eq!(
        LedgerAlgorithmReading::ALL
            .into_iter()
            .filter(|reading| reading.is_chosen())
            .count(),
        1,
        "exactly one reading is chosen"
    );
}

/// Negative: the deferred row names where it goes, not merely that it goes.
#[test]
fn the_deferred_row_names_its_milestone() -> Fallible {
    let entry = row("REQ-P16-04").ok_or("the register must record REQ-P16-04")?;
    assert_eq!(entry.status, ClaimStatus::Deferred);
    assert!(entry.claim.contains("Firecracker"));
    assert!(SANDBOX_DEFERRED_TO.contains("M06"));
    assert!(SANDBOX_DEFERRED_TO.contains("M22"));
    Ok(())
}

/// Negative: the gate-count row says which half of it is this milestone's
/// reading.
#[test]
fn the_gate_count_row_separates_the_counts_from_the_names() -> Fallible {
    let entry = row("REQ-P15-04").ok_or("the register must record REQ-P15-04")?;
    assert!(entry.claim.contains("six of nine"));
    assert!(
        entry.claim.contains("this milestone's reading"),
        "the row must say which half is not the requirement's"
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the register has no duplicate requirement, and every status name
/// renders.
#[test]
fn the_register_has_no_duplicate_requirement() {
    let mut seen: Vec<&str> = P16_RECORDED_CLAIMS
        .into_iter()
        .map(|entry| entry.requirement)
        .collect();
    seen.sort_unstable();
    let before = seen.len();
    seen.dedup();
    assert_eq!(seen.len(), before, "a requirement is recorded twice");

    for status in [
        ClaimStatus::Recorded,
        ClaimStatus::Held,
        ClaimStatus::Deferred,
    ] {
        assert!(!status.name().is_empty());
    }
}

/// Boundary: exactly one row is deferred, and the rest are held by this crate.
///
/// The register is an enumeration of five rows, not a survey of every P16
/// claim. This states which rows the crate's own tests carry, so a reader does
/// not take the register for a coverage report.
#[test]
fn exactly_one_row_is_deferred() {
    let deferred: Vec<&str> = P16_RECORDED_CLAIMS
        .into_iter()
        .filter(|entry| entry.status == ClaimStatus::Deferred)
        .map(|entry| entry.requirement)
        .collect();
    assert_eq!(deferred, vec!["REQ-P16-04"]);

    let held = P16_RECORDED_CLAIMS
        .into_iter()
        .filter(|entry| entry.status == ClaimStatus::Held)
        .count();
    assert_eq!(held, 4);
    assert_eq!(
        P16_RECORDED_CLAIMS
            .into_iter()
            .filter(|entry| entry.status == ClaimStatus::Recorded)
            .count(),
        0,
        "every non-deferred row here is held by a test in this crate"
    );
}
