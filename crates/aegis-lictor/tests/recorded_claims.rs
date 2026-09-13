// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P07 claim register and the dependency pins this repository does not
//! inherit.
//!
//! Positive: every recorded claim names a requirement, a status, a settlement
//! and a source, and every proposed pin names its crate, its version and where
//! a real selection would happen. Negative: none of the deferred claims is
//! promoted, and **neither proposed pin is inherited**. Boundary: each
//! register is exactly its recorded length with no duplicate.

mod common;

use aegis_lictor::{
    ClaimSource, ClaimStatus, P07_PROPOSAL_PINS, P07_RECORDED_CLAIMS, ProposalPin, RecordedClaim,
};

// --- Positive -------------------------------------------------------------

/// Positive: every recorded claim is complete.
#[test]
fn every_recorded_claim_is_complete() {
    for claim in P07_RECORDED_CLAIMS {
        let row: RecordedClaim = claim;
        assert!(row.requirement.starts_with("REQ-"));
        assert!(!row.summary.is_empty());
        assert!(!row.settled_by.is_empty());
        let source: ClaimSource = row.source;
        assert!(source.export.starts_with("export-"));
        assert_eq!(source.sha256_prefix.len(), 12);
        assert!(source.sha256_prefix.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

/// Positive: every proposed pin names its crate, its version and where a real
/// selection would happen.
#[test]
fn every_proposal_pin_is_complete() {
    for pin in P07_PROPOSAL_PINS {
        let row: ProposalPin = pin;
        assert!(!row.crate_name.is_empty());
        assert!(!row.proposed_version.is_empty());
        assert!(row.selected_at.contains("M19"));
        assert!(row.selected_at.contains("current upstream"));
        assert_eq!(row.source.export, "export-006");
        assert_eq!(row.source.sha256_prefix, "6e694e01e136");
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: **neither proposed pin is inherited.**
///
/// REQ-P01-07 records that the imported workspace manifest declares `aya` at
/// 0.12. The repository contract says an imported dependency version does not
/// become an active pin by being imported, and this is that statement in a
/// form a test can falsify.
#[test]
fn no_proposal_pin_is_inherited() {
    assert_eq!(P07_PROPOSAL_PINS.len(), 2);
    for pin in P07_PROPOSAL_PINS {
        assert!(
            !pin.inherited,
            "{} {} must not be inherited from proposal data",
            pin.crate_name, pin.proposed_version
        );
    }
    let aya = P07_PROPOSAL_PINS.iter().find(|pin| pin.crate_name == "aya");
    assert_eq!(aya.map(|pin| pin.proposed_version), Some("0.12"));
}

/// Negative: the deferred claims stay deferred, each with the standing this
/// milestone can defend.
#[test]
fn the_deferred_claims_stay_deferred() {
    let expected = [
        ("REQ-P07-01", ClaimStatus::DeferredTimingRequirement),
        ("REQ-P07-02", ClaimStatus::DeferredPrivilegedConfiguration),
        ("REQ-P07-03", ClaimStatus::DeferredToolchainAdmission),
        ("REQ-P07-05", ClaimStatus::DeferredToolchainAdmission),
        ("REQ-P01-07", ClaimStatus::DeferredToolchainAdmission),
        ("REQ-P07-04", ClaimStatus::UnresolvedSourceConflict),
    ];
    for (requirement, status) in expected {
        let row = P07_RECORDED_CLAIMS
            .iter()
            .find(|claim| claim.requirement == requirement);
        assert_eq!(
            row.map(|claim| claim.status),
            Some(status),
            "{requirement} no longer carries the status this milestone recorded"
        );
    }
}

/// Negative: the timing claim names M23 and says why no figure exists here.
#[test]
fn the_timing_claim_defers_to_the_realtime_milestone() {
    let row = P07_RECORDED_CLAIMS
        .iter()
        .find(|claim| claim.requirement == "REQ-P07-01");
    let settled_by = row.map(|claim| claim.settled_by).unwrap_or_default();
    assert!(settled_by.contains("M23"));
    assert!(settled_by.contains("PREEMPT_RT"));
    assert!(settled_by.contains("PREEMPT_DYNAMIC"));
    assert!(settled_by.contains("no latency or determinism figure"));
}

/// Negative: the eBPF claims name M19 and the open decision, not this
/// milestone.
#[test]
fn the_ebpf_claims_name_the_later_milestone() {
    for requirement in ["REQ-P07-03", "REQ-P07-05", "REQ-P01-07"] {
        let row = P07_RECORDED_CLAIMS
            .iter()
            .find(|claim| claim.requirement == requirement);
        let settled_by = row.map(|claim| claim.settled_by).unwrap_or_default();
        assert!(
            settled_by.contains("M19"),
            "{requirement} must defer its eBPF half to milestone M19"
        );
    }
}

// --- Boundary -------------------------------------------------------------

/// Boundary: each register is exactly its recorded length with no duplicate.
#[test]
fn each_register_is_its_recorded_length() {
    assert_eq!(P07_RECORDED_CLAIMS.len(), 6);
    let mut requirements: Vec<&str> = P07_RECORDED_CLAIMS
        .iter()
        .map(|claim| claim.requirement)
        .collect();
    requirements.sort_unstable();
    requirements.dedup();
    assert_eq!(requirements.len(), 6);

    let mut crates: Vec<&str> = P07_PROPOSAL_PINS.iter().map(|pin| pin.crate_name).collect();
    crates.sort_unstable();
    crates.dedup();
    assert_eq!(crates.len(), 2);
    assert_eq!(
        ClaimStatus::DeferredToolchainAdmission.name(),
        "deferred-toolchain-admission"
    );
    assert_eq!(
        ClaimStatus::DeferredTimingRequirement.name(),
        "deferred-timing-requirement"
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
