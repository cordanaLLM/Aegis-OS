// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The decision D29 record and the P08 claim register.
//!
//! Positive: D29 is recorded with both readings representable, and every
//! recorded claim names a requirement, a status, a settlement and a source.
//! Negative: nothing here is recorded as settled that is not, and the claim
//! statuses are the ones this milestone actually stands behind. Boundary: the
//! register's length is its recorded length, with no duplicate requirement.
//!
//! This file exists so that a later edit which quietly promotes a deferred
//! claim, or closes an open decision, fails the gate instead of passing.

mod common;

use aegis_calliope::{
    CaptureDirection, Citation, ClaimSource, ClaimStatus, D29_REMOTE_PLAY_CAPTURE, DecisionRecord,
    DecisionState, P08_RECORDED_CLAIMS, RecordedClaim,
};

// --- Positive -------------------------------------------------------------

/// Positive: D29 is recorded, and both readings of the capture edge remain
/// representable.
#[test]
fn the_capture_direction_decision_is_recorded() {
    let record: DecisionRecord = D29_REMOTE_PLAY_CAPTURE;
    assert_eq!(
        (record.id, record.dispute, record.settled_at),
        ("D29", "DSP-11", "M12")
    );
    assert_eq!(record.touches, ["REQ-P08-01", "REQ-P08-04"]);
    assert!(record.question.contains("encode budget"));
    assert!(record.rationale.contains("PREEMPT_DYNAMIC"));
    assert_eq!(record.open_readings, CaptureDirection::ALL);
    assert_eq!(CaptureDirection::ALL.len(), 2);
}

/// Positive: the decision cites the two sources that disagree, by identifier
/// and digest prefix.
#[test]
fn the_capture_direction_decision_cites_both_sources() {
    let sources: [Citation; 2] = D29_REMOTE_PLAY_CAPTURE.sources;
    assert_eq!(
        (sources[0].export, sources[0].sha256_prefix),
        ("export-062", "1ce919ed54bb")
    );
    assert_eq!(
        (sources[1].export, sources[1].sha256_prefix),
        ("export-002", "7e0c95f4ea05")
    );
}

/// Positive: each reading carries a distinct name and a different answer to
/// the question the decision is actually about.
#[test]
fn each_reading_answers_the_budget_question_differently() {
    assert!(CaptureDirection::CalliopeProduces.calliope_owns_encode_budget());
    assert!(!CaptureDirection::LudusProduces.calliope_owns_encode_budget());
    assert_eq!(
        CaptureDirection::CalliopeProduces.name(),
        "p08-produces-for-p11"
    );
    assert_eq!(CaptureDirection::LudusProduces.name(), "p11-opens-from-p08");
    assert_eq!(DecisionState::Closed.name(), "closed");
    assert_eq!(DecisionState::Unresolved.name(), "unresolved");
}

/// Positive: every recorded claim is complete.
#[test]
fn every_recorded_claim_is_complete() {
    for claim in P08_RECORDED_CLAIMS {
        let row: RecordedClaim = claim;
        assert!(row.requirement.starts_with("REQ-P08-"));
        assert!(!row.summary.is_empty());
        assert!(!row.settled_by.is_empty());
        let source: ClaimSource = row.source;
        assert!(source.export.starts_with("export-"));
        assert_eq!(source.sha256_prefix.len(), 12);
        assert!(source.sha256_prefix.chars().all(|c| c.is_ascii_hexdigit()));
        assert!(!row.status.name().is_empty());
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: D29 is **not** settled, and this milestone does not settle it.
#[test]
fn the_capture_direction_decision_stays_open() {
    assert_eq!(D29_REMOTE_PLAY_CAPTURE.state, DecisionState::Unresolved);
    assert_ne!(D29_REMOTE_PLAY_CAPTURE.state, DecisionState::Closed);
    assert_ne!(D29_REMOTE_PLAY_CAPTURE.settled_at, "M07");
}

/// Negative: the four deferred claims stay deferred, each with the standing
/// this milestone can defend.
#[test]
fn the_deferred_claims_stay_deferred() {
    let expected = [
        ("REQ-P08-01", ClaimStatus::DeferredHardwareRequirement),
        ("REQ-P08-02", ClaimStatus::DeferredPrivilegedConfiguration),
        ("REQ-P08-03", ClaimStatus::DeferredHardwareRequirement),
        ("REQ-P08-08", ClaimStatus::DeferredTimingRequirement),
        ("REQ-P08-06", ClaimStatus::UnresolvedSourceConflict),
    ];
    for (requirement, status) in expected {
        let row = P08_RECORDED_CLAIMS
            .iter()
            .find(|claim| claim.requirement == requirement);
        assert_eq!(
            row.map(|claim| claim.status),
            Some(status),
            "{requirement} no longer carries the status this milestone recorded"
        );
    }
    assert_eq!(
        ClaimStatus::DeferredTimingRequirement.name(),
        "deferred-timing-requirement"
    );
    assert_eq!(
        ClaimStatus::DeferredHardwareRequirement.name(),
        "deferred-hardware-requirement"
    );
    assert_eq!(
        ClaimStatus::DeferredPrivilegedConfiguration.name(),
        "deferred-privileged-configuration"
    );
    assert_eq!(
        ClaimStatus::UnresolvedSourceConflict.name(),
        "unresolved-source-conflict"
    );
}

/// Negative: the timing claim names M23 and says why no figure exists here.
#[test]
fn the_timing_claim_defers_to_the_realtime_milestone() {
    let row = P08_RECORDED_CLAIMS
        .iter()
        .find(|claim| claim.requirement == "REQ-P08-08");
    let settled_by = row.map(|claim| claim.settled_by).unwrap_or_default();
    assert!(settled_by.contains("M23"));
    assert!(settled_by.contains("PREEMPT_RT"));
    assert!(settled_by.contains("PREEMPT_DYNAMIC"));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the register is exactly its recorded length and names each
/// requirement once.
#[test]
fn the_register_is_its_recorded_length() {
    assert_eq!(P08_RECORDED_CLAIMS.len(), 5);
    let mut names: Vec<&str> = P08_RECORDED_CLAIMS
        .iter()
        .map(|claim| claim.requirement)
        .collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 5);
}
