// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! REQ-P15-04, the structural promotion gates: six of nine reaches
//! Established, five does not.
//!
//! The two counts are the requirement's. The nine gate names are this
//! milestone's reading, because no imported source names them, and the module
//! documentation says so; what is checked here is the counting rule.

mod common;

use aegis_athena::{
    AthenaError, MIN_PASSING_GATES, Maturity, PROMOTION_GATE_COUNT, PromotionGate, PromotionGates,
};

use common::Fallible;

/// Records the first `count` gates as passing.
fn pass(gates: &mut PromotionGates, count: usize) -> Fallible {
    for gate in PromotionGate::ALL.into_iter().take(count) {
        gates.record(gate, true)?;
    }
    Ok(())
}

// --- Positive -------------------------------------------------------------

/// Positive: all nine passing reaches Established.
#[test]
fn all_nine_passing_reaches_established() -> Fallible {
    let mut gates = PromotionGates::new();
    pass(&mut gates, PROMOTION_GATE_COUNT)?;
    assert_eq!(gates.passing(), PROMOTION_GATE_COUNT);
    assert_eq!(gates.recorded(), PROMOTION_GATE_COUNT);
    assert_eq!(gates.maturity(), Maturity::Established);
    for gate in PromotionGate::ALL {
        assert_eq!(gates.result(gate), Some(true));
    }
    Ok(())
}

/// Positive: the gate vocabulary is nine distinct, named, slotted gates.
#[test]
fn the_gate_vocabulary_is_nine_distinct_gates() {
    assert_eq!(PROMOTION_GATE_COUNT, 9);
    assert_eq!(PromotionGate::ALL.len(), PROMOTION_GATE_COUNT);
    let slots: Vec<usize> = PromotionGate::ALL
        .into_iter()
        .map(PromotionGate::slot)
        .collect();
    assert_eq!(slots, (0..PROMOTION_GATE_COUNT).collect::<Vec<usize>>());

    let mut names: Vec<&str> = PromotionGate::ALL
        .into_iter()
        .map(PromotionGate::name)
        .collect();
    names.sort_unstable();
    let before = names.len();
    names.dedup();
    assert_eq!(before, names.len(), "a gate name is used twice");
    assert_eq!(format!("{}", PromotionGate::BoundedLoops), "bounded-loops");

    assert_eq!(
        names,
        vec![
            "bounded-loops",
            "cited-claims",
            "committed-manifest",
            "deadlined-io",
            "no-abort-path",
            "no-decision-path-heap",
            "swept-surface",
            "three-dimensional-tests",
            "typed-interface",
        ],
        "the nine gate names are this milestone's reading, and they are these nine"
    );
    for gate in [
        PromotionGate::TypedInterface,
        PromotionGate::ThreeDimensionalTests,
        PromotionGate::BoundedLoops,
        PromotionGate::NoAbortPath,
        PromotionGate::DeadlinedIo,
        PromotionGate::CommittedManifest,
        PromotionGate::SweptSurface,
        PromotionGate::CitedClaims,
        PromotionGate::NoDecisionPathHeap,
    ] {
        assert!(PromotionGate::ALL.contains(&gate));
    }
}

/// Positive: the maturity vocabulary names both levels.
#[test]
fn the_maturity_vocabulary_names_both_levels() {
    assert_eq!(Maturity::ALL.len(), 2);
    assert_eq!(Maturity::Provisional.name(), "provisional");
    assert_eq!(Maturity::Established.name(), "established");
    assert!(Maturity::Provisional < Maturity::Established);
    assert_eq!(format!("{}", Maturity::Established), "established");
}

// --- Negative -------------------------------------------------------------

/// Negative: recording a gate twice is refused rather than overwritten.
///
/// "This gate passed" and "this gate passed, then failed" are different
/// findings, and only the second is safe to discard deliberately.
#[test]
fn recording_a_gate_twice_is_refused() -> Fallible {
    let mut gates = PromotionGates::new();
    gates.record(PromotionGate::TypedInterface, true)?;
    assert!(matches!(
        gates.record(PromotionGate::TypedInterface, false),
        Err(AthenaError::MaturityGates { .. })
    ));
    assert_eq!(gates.result(PromotionGate::TypedInterface), Some(true));
    assert_eq!(gates.recorded(), 1);
    Ok(())
}

/// Negative: a gate that was never recorded is not a pass.
#[test]
fn an_unrecorded_gate_is_not_a_pass() -> Fallible {
    let gates = PromotionGates::default();
    assert_eq!(gates.passing(), 0);
    assert_eq!(gates.recorded(), 0);
    assert_eq!(gates.maturity(), Maturity::Provisional);
    for gate in PromotionGate::ALL {
        assert_eq!(gates.result(gate), None);
    }

    let mut six_unrecorded = PromotionGates::new();
    for gate in PromotionGate::ALL.into_iter().take(3) {
        six_unrecorded.record(gate, true)?;
    }
    assert_eq!(
        six_unrecorded.maturity(),
        Maturity::Provisional,
        "six unrecorded gates are not six passes"
    );
    Ok(())
}

/// Negative: nine recorded failures are Provisional, not Established.
#[test]
fn nine_failures_stay_provisional() -> Fallible {
    let mut gates = PromotionGates::new();
    for gate in PromotionGate::ALL {
        gates.record(gate, false)?;
    }
    assert_eq!(gates.recorded(), PROMOTION_GATE_COUNT);
    assert_eq!(gates.passing(), 0);
    assert_eq!(gates.maturity(), Maturity::Provisional);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: five passes are Provisional and six are Established.
#[test]
fn the_maturity_threshold_turns_between_five_and_six() -> Fallible {
    assert_eq!(MIN_PASSING_GATES, 6);

    let mut five = PromotionGates::new();
    pass(&mut five, MIN_PASSING_GATES.saturating_sub(1))?;
    assert_eq!(five.passing(), 5);
    assert_eq!(
        five.maturity(),
        Maturity::Provisional,
        "five of nine is not Established"
    );

    let mut six = PromotionGates::new();
    pass(&mut six, MIN_PASSING_GATES)?;
    assert_eq!(six.passing(), 6);
    assert_eq!(
        six.maturity(),
        Maturity::Established,
        "exactly six of nine is Established"
    );
    Ok(())
}

/// Boundary: the six that pass need not be the first six.
#[test]
fn the_six_that_pass_need_not_be_the_first_six() -> Fallible {
    let mut gates = PromotionGates::new();
    for gate in PromotionGate::ALL.into_iter().rev().take(MIN_PASSING_GATES) {
        gates.record(gate, true)?;
    }
    for gate in PromotionGate::ALL.into_iter().take(3) {
        gates.record(gate, false)?;
    }
    assert_eq!(gates.passing(), MIN_PASSING_GATES);
    assert_eq!(gates.recorded(), PROMOTION_GATE_COUNT);
    assert_eq!(gates.maturity(), Maturity::Established);
    assert_eq!(gates.result(PromotionGate::TypedInterface), Some(false));
    assert_eq!(gates.result(PromotionGate::NoDecisionPathHeap), Some(true));
    Ok(())
}
