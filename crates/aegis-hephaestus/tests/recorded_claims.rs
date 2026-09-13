// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P14 claims this crate records and does not discharge, and the versions
//! nobody has pinned.
//!
//! Epic E08-2 asks for the CAD and solver versions to be recorded as unpinned.
//! [`UNPINNED_DEPENDENCIES`] is that record, and the cases below make it a value
//! a test reads rather than a sentence in a commit message.

mod common;

use aegis_hephaestus::{
    ClaimSource, ClaimStatus, P14_RECORDED_CLAIMS, RecordedClaim, SolverTarget,
    UNPINNED_DEPENDENCIES, UnpinnedDependency,
};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: the four recorded claims are recorded with a source each.
#[test]
fn the_four_claims_are_recorded() {
    assert_eq!(P14_RECORDED_CLAIMS.len(), 4);
    for claim in P14_RECORDED_CLAIMS {
        assert!(claim.requirement.starts_with("REQ-P14-"));
        assert!(!claim.summary.is_empty());
        assert!(!claim.settled_by.is_empty());
        let source: ClaimSource = claim.source;
        assert!(source.export.starts_with("export-"));
        assert_eq!(source.sha256_prefix.len(), 12);
        assert!(source.sha256_prefix.chars().all(|c| c.is_ascii_hexdigit()));
    }
}

/// Positive: every recorded solver has an unpinned dependency row, so the
/// register covers the set the admission admits rather than a subset of it.
#[test]
fn every_solver_has_an_unpinned_row() -> Fallible {
    for target in SolverTarget::ALL {
        let Some(row) = row_for(target.tag()) else {
            return Err(format!("{} must have an unpinned row", target.tag()).into());
        };
        assert!(!row.is_pinned());
        assert!(!row.role.is_empty());
        assert!(row.why_unpinned.contains("no version"));
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: no recorded claim is discharged, and the symbolic-solver gate in
/// particular is a deferred dependency rather than something this crate does.
///
/// The sweep is over every row, so a later edit that quietly promotes one fails
/// here rather than passing because the test named the other three.
#[test]
fn no_recorded_claim_is_discharged() -> Fallible {
    for claim in P14_RECORDED_CLAIMS {
        assert!(matches!(
            claim.status,
            ClaimStatus::DeferredDependency
                | ClaimStatus::DeferredRuntimeIsolation
                | ClaimStatus::ExcludedFromGates
        ));
    }
    let Some(gate) = claim_for("REQ-P14-04") else {
        return Err("the symbolic-solver gate must be recorded".into());
    };
    assert_eq!(gate.status, ClaimStatus::DeferredDependency);
    assert_eq!(gate.status.name(), "deferred-dependency");
    assert!(gate.settled_by.contains("no variant meaning verified"));

    let Some(isolation) = claim_for("REQ-P14-03") else {
        return Err("the cgroup isolation claim must be recorded".into());
    };
    assert_eq!(isolation.status, ClaimStatus::DeferredRuntimeIsolation);
    assert_eq!(isolation.status.name(), "deferred-runtime-isolation");
    Ok(())
}

/// Negative: the imported simulated-output target is excluded from the gates
/// and this crate does not run it.
#[test]
fn the_simulated_target_is_excluded_from_the_gates() -> Fallible {
    let Some(claim) = claim_for("REQ-P14-08") else {
        return Err("the simulated-output claim must be recorded".into());
    };
    assert_eq!(claim.status, ClaimStatus::ExcludedFromGates);
    assert_eq!(claim.status.name(), "excluded-from-gates");
    assert!(claim.summary.contains("simulated"));
    assert!(claim.settled_by.contains("Makefile recipe"));
    assert_eq!(claim.source.export, "export-009");
    Ok(())
}

/// Negative: nothing in the register is pinned, which is what E08-2 asks to be
/// recorded.
#[test]
fn nothing_in_the_register_is_pinned() {
    assert_eq!(UNPINNED_DEPENDENCIES.len(), 5);
    for row in UNPINNED_DEPENDENCIES {
        assert!(!row.is_pinned());
        assert!(!row.name.is_empty());
    }
    let pinned = UNPINNED_DEPENDENCIES
        .iter()
        .filter(|row| row.is_pinned())
        .count();
    assert_eq!(pinned, 0);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the register covers both kernels and all three solvers, and
/// nothing else, so it is neither short of the set it describes nor padded.
#[test]
fn the_register_covers_both_kernels_and_three_solvers() {
    let names: Vec<&str> = UNPINNED_DEPENDENCIES
        .iter()
        .map(|row: &UnpinnedDependency| row.name)
        .collect();
    assert_eq!(
        names,
        vec![
            "exact NURBS B-Rep kernel",
            "polyhedral CSG kernel",
            "OpenFOAM",
            "CalculiX",
            "Elmer",
        ]
    );
    let kernels = UNPINNED_DEPENDENCIES
        .iter()
        .filter(|row| row.name.contains("kernel"))
        .count();
    assert_eq!(kernels, 2);
    assert_eq!(names.len() - kernels, SolverTarget::ALL.len());
}

/// Boundary: a requirement the register does not hold has no row, so it cannot
/// be read as covering something it never recorded.
#[test]
fn an_unrecorded_requirement_has_no_row() {
    assert!(claim_for("REQ-P14-99").is_none());
    assert!(claim_for("").is_none());
    assert!(claim_for("REQ-P14-0").is_none());
    assert!(row_for("Ansys").is_none());
    assert!(row_for("").is_none());
}

/// Returns the recorded claim for `requirement`, if the register holds one.
fn claim_for(requirement: &str) -> Option<RecordedClaim> {
    P14_RECORDED_CLAIMS
        .into_iter()
        .take(P14_RECORDED_CLAIMS.len())
        .find(|claim| claim.requirement == requirement)
}

/// Returns the unpinned row for `name`, if the register holds one.
fn row_for(name: &str) -> Option<UnpinnedDependency> {
    UNPINNED_DEPENDENCIES
        .into_iter()
        .take(UNPINNED_DEPENDENCIES.len())
        .find(|row| row.name == name)
}
