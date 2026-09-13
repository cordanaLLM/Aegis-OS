// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P15 claim register: what is recorded, and with what standing.
//!
//! REQ-P15-01, REQ-P15-06 and REQ-GRAPH-03 are written down so they are not
//! lost. None of them is checked by anything in this repository, and the
//! middle one is marked unverified by the requirement register itself. This
//! file holds the statuses, so a later edit cannot quietly promote a recorded
//! claim into an established one.

mod common;

use aegis_hestia::{ClaimSource, ClaimStatus, P15_RECORDED_CLAIMS, RecordedClaim};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: the register names the three recorded requirements.
#[test]
fn the_register_names_the_recorded_requirements() {
    let requirements: Vec<&'static str> = P15_RECORDED_CLAIMS
        .iter()
        .map(|claim| claim.requirement)
        .collect();
    assert_eq!(
        requirements,
        vec!["REQ-P15-01", "REQ-P15-06", "REQ-GRAPH-03"]
    );
}

/// Positive: every claim says what would settle it, and cites its source.
#[test]
fn every_claim_names_what_would_settle_it() {
    for claim in P15_RECORDED_CLAIMS {
        let row: RecordedClaim = claim;
        assert!(!row.summary.is_empty());
        assert!(!row.settled_by.is_empty());
        let citation: ClaimSource = row.source;
        assert!(citation.export.starts_with("export-"));
        assert_eq!(citation.sha256_prefix.len(), 12);
        assert!(
            citation
                .sha256_prefix
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the third-party accuracy figure stays marked unverified.
///
/// REQ-P15-06 records the figure **and** records that it is a proposal claim.
/// Carrying the number without the marking would turn a register entry into an
/// endorsement, so the marking is what this case holds.
#[test]
fn the_third_party_figure_stays_unverified() -> Fallible {
    let claim = P15_RECORDED_CLAIMS
        .iter()
        .find(|row| row.requirement == "REQ-P15-06")
        .ok_or("the register lost REQ-P15-06")?;
    assert_eq!(claim.status, ClaimStatus::UnverifiedProposalClaim);
    assert_eq!(claim.status.name(), "unverified-proposal-claim");
    assert!(claim.settled_by.contains("nothing in this repository"));
    Ok(())
}

/// Negative: the footprint target stays unmeasured, not asserted.
#[test]
fn the_footprint_target_stays_unmeasured() -> Fallible {
    let claim = P15_RECORDED_CLAIMS
        .iter()
        .find(|row| row.requirement == "REQ-P15-01")
        .ok_or("the register lost REQ-P15-01")?;
    assert_eq!(claim.status, ClaimStatus::UnmeasuredTarget);
    assert_eq!(claim.status.name(), "unmeasured-target");
    assert!(claim.settled_by.contains("measures anything"));
    Ok(())
}

/// Negative: the storage edge stays an unresolved conflict, not an edge.
///
/// REQ-GRAPH-03 is a disagreement between two sources, and no decision closes
/// it. The crate types the storage path and claims no edge at all, which is
/// what the register has to keep saying.
#[test]
fn the_storage_edge_stays_unresolved() -> Fallible {
    let claim = P15_RECORDED_CLAIMS
        .iter()
        .find(|row| row.requirement == "REQ-GRAPH-03")
        .ok_or("the register lost REQ-GRAPH-03")?;
    assert_eq!(claim.status, ClaimStatus::UnresolvedGraphConflict);
    assert_eq!(claim.status.name(), "unresolved-graph-conflict");
    assert!(claim.settled_by.contains("claims no edge"));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: exactly three claims, each with a distinct status.
#[test]
fn exactly_three_claims_with_distinct_statuses() {
    assert_eq!(P15_RECORDED_CLAIMS.len(), 3);
    let mut statuses: Vec<&'static str> = P15_RECORDED_CLAIMS
        .iter()
        .map(|claim| claim.status.name())
        .collect();
    statuses.sort_unstable();
    statuses.dedup();
    assert_eq!(statuses.len(), 3);
}

/// Boundary: no claim is recorded as established, because none is.
///
/// The register has no such status to spell, which is the boundary: adding one
/// would be adding a way to say something this crate cannot support.
#[test]
fn no_claim_can_be_recorded_as_established() {
    for claim in P15_RECORDED_CLAIMS {
        assert!(matches!(
            claim.status,
            ClaimStatus::UnmeasuredTarget
                | ClaimStatus::UnverifiedProposalClaim
                | ClaimStatus::UnresolvedGraphConflict
        ));
    }
}
