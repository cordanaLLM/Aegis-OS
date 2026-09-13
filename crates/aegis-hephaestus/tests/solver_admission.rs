// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The solver admission (REQ-P14-03), which starts no solver.
//!
//! The scaffold's dispatch prints which slice it would have used and returns
//! `Ok(())` either way. Here the admission returns a typed solver or a typed
//! refusal, and there is no method that runs anything at all.

mod common;

use aegis_hephaestus::{HephaestusError, SOLVER_SLICE, SolverAdmission, SolverTarget};

use common::Fallible;

// --- Positive -------------------------------------------------------------

/// Positive: every recorded solver tag is admitted, and each reports its
/// declared slice and domain.
#[test]
fn every_recorded_solver_is_admitted() -> Fallible {
    let mut admission = SolverAdmission::new();
    for target in SolverTarget::ALL {
        assert_eq!(admission.admit(target.tag())?, target);
        assert_eq!(target.declared_slice(), SOLVER_SLICE);
        assert!(!target.domain().is_empty());
    }
    assert_eq!(admission.admitted(), 3);
    assert_eq!(admission.refused(), 0);
    assert_eq!(SOLVER_SLICE, "agent-solver.slice");
    Ok(())
}

/// Positive: the tags are the ones the scaffold's own dispatch matches on.
#[test]
fn the_tags_are_the_scaffold_tags() {
    assert_eq!(
        SolverTarget::ALL.map(SolverTarget::tag),
        ["OpenFOAM", "CalculiX", "Elmer"]
    );
    assert_eq!(
        SolverTarget::OpenFoam.domain(),
        "computational fluid dynamics"
    );
    assert_eq!(
        SolverTarget::CalculiX.domain(),
        SolverTarget::Elmer.domain()
    );
    assert_eq!(SolverTarget::from_tag("Elmer"), Some(SolverTarget::Elmer));
    assert_eq!(SolverAdmission::default(), SolverAdmission::new());
}

// --- Negative -------------------------------------------------------------

/// Negative: a name the recorded set does not hold is refused, and the refusal
/// is counted.
#[test]
fn an_unknown_solver_is_refused() {
    let mut admission = SolverAdmission::new();
    for tag in ["Ansys", "", "openfoam", "OpenFOAM ", "Elmer\u{0}"] {
        assert_eq!(
            admission.admit(tag),
            Err(HephaestusError::UnknownSolver),
            "{tag:?} must not be admitted"
        );
    }
    assert_eq!(admission.refused(), 5);
    assert_eq!(admission.admitted(), 0);
}

/// Negative: the comparison is exact, so a near miss is not a solver.
#[test]
fn the_tag_comparison_is_exact() {
    assert_eq!(SolverTarget::from_tag("openfoam"), None);
    assert_eq!(SolverTarget::from_tag("OpenFOAM-v12"), None);
    assert_eq!(SolverTarget::from_tag("CalculiXX"), None);
    assert_eq!(SolverTarget::from_tag(""), None);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the recorded set is exactly three solvers and holds no duplicate,
/// so the count in the documentation is the count that is admitted.
#[test]
fn the_recorded_set_is_exactly_three_solvers() {
    assert_eq!(SolverTarget::ALL.len(), 3);
    let mut tags = SolverTarget::ALL.map(SolverTarget::tag).to_vec();
    tags.sort_unstable();
    tags.dedup();
    assert_eq!(tags.len(), 3);
    for target in SolverTarget::ALL {
        assert_eq!(SolverTarget::from_tag(target.tag()), Some(target));
    }
}

/// Boundary: admissions and refusals are counted apart, and neither counter
/// moves for the other outcome.
#[test]
fn the_counters_move_apart() -> Fallible {
    let mut admission = SolverAdmission::new();
    assert_eq!(admission.admitted(), 0);
    assert_eq!(admission.refused(), 0);
    admission.admit("Elmer")?;
    assert_eq!(admission.admitted(), 1);
    assert_eq!(admission.refused(), 0);
    assert!(admission.admit("Ansys").is_err());
    assert_eq!(admission.admitted(), 1);
    assert_eq!(admission.refused(), 1);
    Ok(())
}
