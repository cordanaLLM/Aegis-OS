// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-2, the Pareto promotion gate: the four comparisons, and the two
//! recorded boundary values exercised exactly.
//!
//! The two values are 1.5 for latency and 0.99 for retention. They land on
//! opposite sides because the scaffold's comparisons do: latency is a ceiling
//! compared strictly, retention a floor compared non-strictly.

mod common;

use aegis_athena::{
    AthenaError, CARBON_RATE_BOUND, CandidateId, CandidateMetrics, CorrelationId, LATENCY_BOUND_MS,
    LatencyMs, MAX_LATENCY_MS, MAX_MEMORY_MB, MAX_SCI_CARBON_RATE, MEMORY_BOUND_MB,
    MIN_NULL_MODEL_RETENTION, MemoryMb, NullModelRetention, Objective, ParetoVerdict,
    SciCarbonRate, Sha256AthenaEngine, Stage, gate,
};

use common::{AT, EPSILON, Fallible, breaching, candidate, passing, published};

// --- Positive -------------------------------------------------------------

/// Positive: the scaffold's own sample candidate clears every objective.
#[test]
fn the_scaffold_sample_candidate_clears_every_objective() -> Fallible {
    let verdict = gate(&passing()?);
    assert!(verdict.is_superior());
    assert_eq!(verdict.cleared_count(), 4);
    assert_eq!(verdict.first_breach(), None);
    for objective in Objective::ALL {
        assert!(verdict.cleared(objective), "{objective} was not cleared");
    }
    assert_eq!(verdict.bits(), 0b1111);
    Ok(())
}

/// Positive: the objective vocabulary states each bound.
#[test]
fn the_objective_vocabulary_states_each_bound() {
    assert_eq!(Objective::ALL.len(), 4);
    let bounds: Vec<f64> = Objective::ALL.into_iter().map(Objective::bound).collect();
    let expected = [
        LATENCY_BOUND_MS,
        MEMORY_BOUND_MB,
        CARBON_RATE_BOUND,
        MIN_NULL_MODEL_RETENTION,
    ];
    for (found, want) in bounds.iter().zip(expected.iter()) {
        assert!(
            (found - want).abs() < EPSILON,
            "bound {found} is not {want}"
        );
    }
    let tags: Vec<u8> = Objective::ALL.into_iter().map(Objective::tag).collect();
    assert_eq!(tags, vec![1, 2, 3, 4]);
    let slots: Vec<usize> = Objective::ALL.into_iter().map(Objective::slot).collect();
    assert_eq!(slots, vec![0, 1, 2, 3]);
    assert_eq!(format!("{}", Objective::Carbon), "carbon");
}

/// Positive: exactly one of the four bounds is a floor.
///
/// That asymmetry is the imported scaffold's, and it is what puts the two
/// recorded boundary values on opposite sides of their bounds.
#[test]
fn exactly_one_bound_is_a_floor() {
    let floors: Vec<&str> = Objective::ALL
        .into_iter()
        .filter(|objective| objective.is_floor())
        .map(Objective::name)
        .collect();
    assert_eq!(floors, vec!["retention"]);
}

/// Positive: the recorded bounds are the scaffold's four literals.
#[test]
fn the_recorded_bounds_are_the_scaffold_literals() {
    assert!((LATENCY_BOUND_MS - 1.5).abs() < EPSILON);
    assert!((MEMORY_BOUND_MB - 64.0).abs() < EPSILON);
    assert!((CARBON_RATE_BOUND - 0.8).abs() < EPSILON);
    assert!((MIN_NULL_MODEL_RETENTION - 0.99).abs() < EPSILON);
}

// --- Negative -------------------------------------------------------------

/// Negative: exceeding any single bound reaches Invalidate, with a ledger
/// entry.
///
/// Each of the four objectives is breached on its own, with the other three
/// left at the passing values, so the test says which bound produced the
/// invalidation rather than only that one did.
#[test]
fn breaching_any_single_bound_reaches_invalidate_with_a_ledger_entry() -> Fallible {
    for objective in Objective::ALL {
        let mut engine = Sha256AthenaEngine::new();
        let metrics = breaching(objective)?;
        let (stage, lifecycle) = engine.evaluate(candidate()?, metrics, 100, AT)?;
        assert_eq!(
            stage,
            Stage::Invalidate,
            "breaching {objective} must invalidate"
        );
        assert_eq!(
            engine.ledger().len(),
            1,
            "an invalidation must be recorded, not only returned"
        );
        engine.verify()?;

        let verdict = lifecycle.verdict().ok_or("a check leaves a verdict")?;
        assert!(!verdict.is_superior());
        assert_eq!(verdict.first_breach(), Some(objective));
        assert_eq!(verdict.cleared_count(), 3);
        assert!(!verdict.cleared(objective));
    }
    Ok(())
}

/// Negative: an empty candidate identifier is rejected.
///
/// The imported scaffold guards this with `assert!(!candidate_id.is_empty())`,
/// which aborts the process. Here the identifier does not become a value at
/// all, so the negative case is an ordinary test (HISS-07).
#[test]
fn an_empty_candidate_identifier_is_rejected() {
    let refused: Result<CandidateId, _> = CandidateId::parse("");
    assert!(
        refused.is_err(),
        "the empty identifier must not become a value"
    );
    assert!(CorrelationId::parse("").is_err());
    assert!(
        CandidateId::parse("c").is_ok(),
        "one character is an identifier"
    );
}

/// Negative: a non-finite or non-positive metric never becomes a value.
///
/// The scaffold's second assertion is `latency_ms > 0.0`; that becomes
/// [`AthenaError::Latency`] here, and the other three metrics gain the
/// refusals the scaffold never had.
#[test]
fn a_non_finite_metric_is_refused() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0, 0.0] {
        assert!(
            matches!(LatencyMs::new(bad), Err(AthenaError::Latency { .. })),
            "a latency of {bad} was taken"
        );
    }
    for bad in [f64::NAN, f64::INFINITY, -1.0] {
        let refused = [
            MemoryMb::new(bad).is_err(),
            SciCarbonRate::new(bad).is_err(),
            NullModelRetention::new(bad).is_err(),
        ];
        assert_eq!(refused, [true; 3], "a metric accepted {bad}");
    }
}

/// Negative: a metric past its own recorded bound is refused.
#[test]
fn a_metric_past_its_bound_is_refused() {
    assert!(LatencyMs::new(MAX_LATENCY_MS + 1.0).is_err());
    assert!(MemoryMb::new(MAX_MEMORY_MB + 1.0).is_err());
    assert!(SciCarbonRate::new(MAX_SCI_CARBON_RATE + 1.0).is_err());
    assert!(
        NullModelRetention::new(1.0001).is_err(),
        "a retention is a proportion, so above one is not a value"
    );
}

/// Negative: three breaches at once are all reported, not only the first.
///
/// The gate keeps every comparison rather than short-circuiting, which is what
/// makes a breach count meaningful.
#[test]
fn several_breaches_are_all_reported() -> Fallible {
    let metrics = CandidateMetrics::new(
        LatencyMs::new(9.0)?,
        MemoryMb::new(512.0)?,
        SciCarbonRate::new(2.0)?,
        NullModelRetention::new(0.995)?,
    );
    let verdict = ParetoVerdict::evaluate(&metrics);
    assert!(!verdict.is_superior());
    assert_eq!(verdict.cleared_count(), 1);
    assert_eq!(verdict.first_breach(), Some(Objective::Latency));
    assert!(verdict.cleared(Objective::Retention));
    assert_eq!(verdict.bits(), 0b1000);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a latency of exactly 1.5 does not clear.
///
/// The comparison is `< 1.5`, so the bound itself is a breach. This is one of
/// the two values the recorded acceptance names.
#[test]
fn a_latency_of_exactly_one_point_five_does_not_clear() -> Fallible {
    let mut metrics = passing()?;
    metrics.latency = LatencyMs::AT_BOUND;
    assert!((metrics.latency.get() - 1.5).abs() < EPSILON);

    let verdict = gate(&metrics);
    assert!(!verdict.cleared(Objective::Latency));
    assert!(!verdict.is_superior());
    assert_eq!(verdict.first_breach(), Some(Objective::Latency));

    let mut below = passing()?;
    below.latency = LatencyMs::new(1.499_999_999_9)?;
    assert!(gate(&below).is_superior(), "just below the bound clears");
    Ok(())
}

/// Boundary: a retention of exactly 0.99 does clear.
///
/// The comparison is `>= 0.99`, so the floor itself is admissible. This is the
/// other value the recorded acceptance names, and it lands on the opposite
/// side of its bound from the latency one.
#[test]
fn a_retention_of_exactly_zero_point_nine_nine_clears() -> Fallible {
    let mut metrics = passing()?;
    metrics.retention = NullModelRetention::AT_FLOOR;
    assert!((metrics.retention.get() - 0.99).abs() < EPSILON);

    let verdict = gate(&metrics);
    assert!(verdict.cleared(Objective::Retention));
    assert!(
        verdict.is_superior(),
        "exactly the retention floor is inside it"
    );

    let mut below = passing()?;
    below.retention = NullModelRetention::new(0.989_999_999_9)?;
    assert!(!gate(&below).cleared(Objective::Retention));
    assert_eq!(gate(&below).first_breach(), Some(Objective::Retention));
    Ok(())
}

/// Boundary: both other ceilings turn the same way as the latency one.
#[test]
fn the_other_two_ceilings_turn_the_same_way() -> Fallible {
    let mut at_memory = passing()?;
    at_memory.memory = MemoryMb::AT_BOUND;
    assert!(!gate(&at_memory).cleared(Objective::Memory));

    let mut at_carbon = passing()?;
    at_carbon.carbon = SciCarbonRate::AT_BOUND;
    assert!(!gate(&at_carbon).cleared(Objective::Carbon));

    let mut just_inside = passing()?;
    just_inside.memory = MemoryMb::new(63.999_999_999)?;
    just_inside.carbon = SciCarbonRate::new(0.799_999_999)?;
    assert!(gate(&just_inside).is_superior());
    Ok(())
}

/// Boundary: full retention clears, and zero retention is a value that fails.
///
/// The unit interval's two endpoints are both admissible quantities; only one
/// of them clears the floor, which is the difference between a refusal and a
/// breach.
#[test]
fn the_retention_interval_endpoints_are_both_values() -> Fallible {
    let mut full = passing()?;
    full.retention = NullModelRetention::FULL;
    assert!((full.retention.get() - 1.0).abs() < EPSILON);
    assert!(gate(&full).is_superior());

    let mut none = passing()?;
    none.retention = NullModelRetention::new(0.0)?;
    assert!(!gate(&none).cleared(Objective::Retention));
    assert_eq!(gate(&none).first_breach(), Some(Objective::Retention));

    let engine = published()?;
    assert_eq!(engine.ledger().len(), 1);
    Ok(())
}
