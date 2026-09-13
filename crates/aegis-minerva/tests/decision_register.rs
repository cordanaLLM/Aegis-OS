// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Decision D27 and the P09 claims this crate records but cannot check.
//!
//! Positive: D27 is recorded with both readings and its sources. Negative: it
//! is **not** settled, and no recorded claim is marked as discharged.
//! Boundary: the register's arity is exactly what it declares, so a reading or
//! a claim cannot be dropped without failing this file.

mod common;

use aegis_minerva::{
    CadVerificationDirection, Citation, ClaimSource, ClaimStatus, D27_CAD_VERIFICATION_DIRECTION,
    DecisionRecord, DecisionState, P09_RECORDED_CLAIMS, RecordedClaim,
};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: D27 is recorded, names both readings and cites both sources.
#[test]
fn d27_is_recorded_with_both_readings() {
    let record: DecisionRecord = D27_CAD_VERIFICATION_DIRECTION;
    assert_eq!(record.id, "D27");
    assert_eq!(record.readings, CadVerificationDirection::BOTH);
    assert_eq!(record.recorded_at, "M06");
    assert_eq!(record.dispute, "DSP-05");
    assert_eq!(record.touches, ["REQ-P14-05", "REQ-GRAPH-05"]);
    assert!(record.question.contains("P09 calls P14"));
    assert!(record.carried.contains("submission"));
    assert!(record.settled_by.contains("M08"));
}

/// Positive: D27 cites both sides of the dispute it carries.
#[test]
fn d27_cites_both_sides_of_its_dispute() {
    let [graph, dataflow]: [Citation; 2] = D27_CAD_VERIFICATION_DIRECTION.sources;
    assert_eq!(graph.export, "export-062");
    assert_eq!(graph.sha256_prefix, "1ce919ed54bb");
    assert_eq!(dataflow.export, "export-002");
    assert_eq!(dataflow.sha256_prefix, "7e0c95f4ea05");
    for citation in D27_CAD_VERIFICATION_DIRECTION.sources {
        assert_eq!(citation.sha256_prefix.len(), 12);
        assert!(
            citation
                .sha256_prefix
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
    }
}

/// Positive: each reading names its submitter, its verifier and its source.
#[test]
fn each_reading_names_its_sides_and_its_source() {
    let graph = CadVerificationDirection::MinervaSubmitsHephaestusVerifies;
    let dataflow = CadVerificationDirection::HephaestusSubmitsMinervaVerifies;
    assert_eq!(graph.submitter(), "P09_Minerva");
    assert_eq!(graph.verifier(), "P14_Hephaestus");
    assert_eq!(dataflow.submitter(), "P14_Hephaestus");
    assert_eq!(dataflow.verifier(), "P09_Minerva");
    assert!(graph.is_graph_of_record());
    assert!(!dataflow.is_graph_of_record());
}

/// Positive: each reading carries its own tag and its own source.
#[test]
fn each_reading_carries_its_tag_and_evidence() {
    let graph = CadVerificationDirection::MinervaSubmitsHephaestusVerifies;
    let dataflow = CadVerificationDirection::HephaestusSubmitsMinervaVerifies;
    assert_eq!(graph.evidence(), "export-062");
    assert_eq!(dataflow.evidence(), "export-002");
    assert_eq!(graph.tag(), "minerva-submits-hephaestus-verifies");
    assert_eq!(dataflow.tag(), "hephaestus-submits-minerva-verifies");
    assert_eq!(CadVerificationDirection::GRAPH_OF_RECORD, graph);
}

// --- Negative -------------------------------------------------------------

/// Negative: D27 is not settled, and nothing in this crate closes it.
#[test]
fn d27_is_not_settled() {
    let record = D27_CAD_VERIFICATION_DIRECTION;
    assert_eq!(record.state, DecisionState::Unresolved);
    assert_eq!(record.state.name(), "unresolved");
    assert!(!record.is_settled());
    assert_eq!(DecisionState::Closed.name(), "closed");
    assert_ne!(record.state, DecisionState::Closed);
}

/// Negative: no recorded claim is marked as discharged, because none is.
#[test]
fn no_recorded_claim_is_discharged() {
    for claim in P09_RECORDED_CLAIMS {
        let row: RecordedClaim = claim;
        assert!(!row.settled_by.is_empty());
        assert!(!row.summary.is_empty());
        assert!(row.requirement.starts_with("REQ-"));
        assert!(matches!(
            row.status,
            ClaimStatus::UnmeasuredTarget
                | ClaimStatus::DeferredDependency
                | ClaimStatus::UnresolvedGraphConflict
                | ClaimStatus::UnmodelledArchitecturalClaim
        ));
        let source: ClaimSource = row.source;
        assert!(source.export.starts_with("export-"));
        assert_eq!(source.sha256_prefix.len(), 12);
    }
}

/// Negative: the two figures this crate holds as constants are recorded as
/// unmeasured, so neither can be cited as a measurement.
#[test]
fn the_envelope_and_the_latency_cap_stay_unmeasured() -> Fallible {
    for requirement in ["REQ-P09-01", "REQ-P09-02"] {
        let claim = P09_RECORDED_CLAIMS
            .into_iter()
            .find(|row| row.requirement == requirement)
            .ok_or("the register must carry the recorded requirement")?;
        assert_eq!(claim.status, ClaimStatus::UnmeasuredTarget);
        assert_eq!(claim.status.name(), "unmeasured-target");
    }
    let solver = P09_RECORDED_CLAIMS
        .into_iter()
        .find(|row| row.requirement == "REQ-P09-08")
        .ok_or("the register must carry the solver requirement")?;
    assert_eq!(solver.status, ClaimStatus::DeferredDependency);
    assert_eq!(solver.status.name(), "deferred-dependency");
    assert!(
        solver
            .settled_by
            .contains("no Z3 foreign-function interface")
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the register's arity is exactly what it declares.
#[test]
fn the_register_arity_is_exact() {
    assert_eq!(CadVerificationDirection::BOTH.len(), 2);
    assert_eq!(D27_CAD_VERIFICATION_DIRECTION.readings.len(), 2);
    assert_eq!(P09_RECORDED_CLAIMS.len(), 5);
    let mut requirements: Vec<&str> = P09_RECORDED_CLAIMS
        .iter()
        .map(|row| row.requirement)
        .collect();
    requirements.sort_unstable();
    requirements.dedup();
    assert_eq!(requirements.len(), 5, "no requirement is recorded twice");
    let mut tags: Vec<&str> = CadVerificationDirection::BOTH
        .iter()
        .map(|row| row.tag())
        .collect();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), 2, "both readings have distinct tags");
    assert_eq!(
        P09_RECORDED_CLAIMS
            .iter()
            .filter(|row| row.status == ClaimStatus::UnresolvedGraphConflict)
            .count(),
        1,
        "exactly one recorded claim is the graph conflict D27 is about"
    );
}
