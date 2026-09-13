// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-2, the seven-stage lifecycle: what advances it, what refuses it, and
//! what may still be reversed.

mod common;

use aegis_athena::{
    AthenaError, CHECK_LATENCY_BUDGET_MS, Event, EventKind, InvalidationReason, Lifecycle, Stage,
};

use common::{Fallible, passing};

// --- Positive -------------------------------------------------------------

/// Positive: a passing candidate walks all seven stages and reaches Publish.
#[test]
fn a_passing_candidate_reaches_publish() -> Fallible {
    let mut lifecycle = Lifecycle::new();
    assert_eq!(lifecycle.stage(), Stage::Propose);
    assert_eq!(lifecycle.step(Event::Challenge)?, Stage::Challenge);
    assert_eq!(lifecycle.step(Event::Decompose)?, Stage::Decompose);
    assert_eq!(lifecycle.step(Event::Prove)?, Stage::Prove);
    assert_eq!(
        lifecycle.step(Event::Check {
            metrics: passing()?,
            elapsed_ms: 120,
        })?,
        Stage::Publish
    );
    let verdict = lifecycle
        .verdict()
        .ok_or("a completed check must leave a verdict")?;
    assert!(verdict.is_superior());
    assert_eq!(lifecycle.reason(), None);
    assert_eq!(lifecycle.metrics(), Some(passing()?));
    Ok(())
}

/// Positive: the stage vocabulary is the seven steps REQ-P16-01 numbers.
#[test]
fn the_stage_vocabulary_is_the_seven_recorded_steps() {
    assert_eq!(Stage::ALL.len(), 7);
    let names: Vec<&str> = Stage::ALL.into_iter().map(Stage::name).collect();
    assert_eq!(
        names,
        vec![
            "propose",
            "challenge",
            "decompose",
            "prove",
            "check",
            "publish",
            "invalidate",
        ]
    );
    let tags: Vec<u8> = Stage::ALL.into_iter().map(Stage::tag).collect();
    assert_eq!(tags, vec![1, 2, 3, 4, 5, 6, 7]);
    assert_eq!(format!("{}", Stage::Publish), "publish");
}

/// Positive: the event vocabulary names every event a stage may be offered.
#[test]
fn the_event_vocabulary_covers_every_offer() -> Fallible {
    assert_eq!(EventKind::ALL.len(), 5);
    assert_eq!(Event::Challenge.kind(), EventKind::Challenge);
    assert_eq!(Event::Decompose.kind(), EventKind::Decompose);
    assert_eq!(Event::Prove.kind(), EventKind::Prove);
    assert_eq!(
        Event::Check {
            metrics: passing()?,
            elapsed_ms: 1,
        }
        .kind(),
        EventKind::Check
    );
    assert_eq!(
        Event::Invalidate {
            reason: InvalidationReason::Withdrawn,
        }
        .kind(),
        EventKind::Invalidate
    );
    for kind in EventKind::ALL {
        assert!(!kind.name().is_empty());
    }
    assert_eq!(format!("{}", EventKind::Check), "check");
    Ok(())
}

/// Positive: a published candidate can still be withdrawn (REQ-P16-05).
///
/// This is the reversible maturity gate. A change that looked stable and then
/// failed under compositional stress must not stay published because the stage
/// that published it was treated as final.
#[test]
fn a_published_candidate_can_still_be_withdrawn() -> Fallible {
    let mut lifecycle = Lifecycle::new();
    lifecycle.step(Event::Challenge)?;
    lifecycle.step(Event::Decompose)?;
    lifecycle.step(Event::Prove)?;
    lifecycle.step(Event::Check {
        metrics: passing()?,
        elapsed_ms: 10,
    })?;
    assert_eq!(lifecycle.stage(), Stage::Publish);
    assert!(!Stage::Publish.is_terminal());

    let withdrawn = lifecycle.step(Event::Invalidate {
        reason: InvalidationReason::PostPublicationRegression,
    })?;
    assert_eq!(withdrawn, Stage::Invalidate);
    assert_eq!(
        lifecycle.reason(),
        Some(InvalidationReason::PostPublicationRegression)
    );
    assert!(
        lifecycle
            .verdict()
            .is_some_and(|verdict| verdict.is_superior()),
        "withdrawal does not erase the verdict that published it"
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an out-of-order event is refused and the stage does not move.
#[test]
fn an_out_of_order_event_is_refused() -> Fallible {
    let mut lifecycle = Lifecycle::new();
    let refusal = lifecycle.step(Event::Prove);
    assert!(matches!(
        refusal,
        Err(AthenaError::Unexpected {
            stage: Stage::Propose,
            event: EventKind::Prove,
        })
    ));
    assert_eq!(lifecycle.stage(), Stage::Propose);

    lifecycle.step(Event::Challenge)?;
    assert!(matches!(
        lifecycle.step(Event::Challenge),
        Err(AthenaError::Unexpected { .. })
    ));
    assert_eq!(lifecycle.stage(), Stage::Challenge);
    Ok(())
}

/// Negative: a check offered before Prove is refused.
#[test]
fn a_check_before_prove_is_refused() -> Fallible {
    let mut lifecycle = Lifecycle::new();
    lifecycle.step(Event::Challenge)?;
    assert!(matches!(
        lifecycle.step(Event::Check {
            metrics: passing()?,
            elapsed_ms: 1,
        }),
        Err(AthenaError::Unexpected { .. })
    ));
    assert_eq!(lifecycle.metrics(), None);
    assert_eq!(lifecycle.verdict(), None);
    Ok(())
}

/// Negative: the terminal stage accepts nothing, including a second
/// withdrawal.
#[test]
fn the_terminal_stage_accepts_nothing() -> Fallible {
    let mut lifecycle = Lifecycle::default();
    lifecycle.step(Event::Invalidate {
        reason: InvalidationReason::Withdrawn,
    })?;
    assert_eq!(lifecycle.stage(), Stage::Invalidate);
    assert!(Stage::Invalidate.is_terminal());

    for event in [
        Event::Challenge,
        Event::Invalidate {
            reason: InvalidationReason::Withdrawn,
        },
    ] {
        assert!(matches!(
            lifecycle.step(event),
            Err(AthenaError::Terminal { .. })
        ));
    }
    Ok(())
}

/// Negative: a check that overruns the recorded budget is refused.
///
/// REQ-P16-10 assigns Athena a 500 ms candidate-check budget. A budget that is
/// only reported is not a budget, so an overrun fails closed and the candidate
/// stays at Prove rather than publishing late.
#[test]
fn a_check_past_the_latency_budget_is_refused() -> Fallible {
    let mut lifecycle = Lifecycle::new();
    lifecycle.step(Event::Challenge)?;
    lifecycle.step(Event::Decompose)?;
    lifecycle.step(Event::Prove)?;
    let refusal = lifecycle.step(Event::Check {
        metrics: passing()?,
        elapsed_ms: CHECK_LATENCY_BUDGET_MS.saturating_add(1),
    });
    assert!(matches!(
        refusal,
        Err(AthenaError::CheckBudgetExceeded {
            budget_ms: 500,
            elapsed_ms: 501,
        })
    ));
    assert_eq!(lifecycle.stage(), Stage::Prove);
    assert_eq!(lifecycle.verdict(), None);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a check taking exactly the budget is accepted.
#[test]
fn a_check_at_exactly_the_latency_budget_is_accepted() -> Fallible {
    assert_eq!(CHECK_LATENCY_BUDGET_MS, 500);
    let mut lifecycle = Lifecycle::new();
    lifecycle.step(Event::Challenge)?;
    lifecycle.step(Event::Decompose)?;
    lifecycle.step(Event::Prove)?;
    assert_eq!(
        lifecycle.step(Event::Check {
            metrics: passing()?,
            elapsed_ms: CHECK_LATENCY_BUDGET_MS,
        })?,
        Stage::Publish,
        "exactly the budget is inside it"
    );
    Ok(())
}

/// Boundary: a check taking no time at all is accepted, and the withdrawal
/// vocabulary covers every reason a record can carry.
#[test]
fn the_lifecycle_boundaries_are_representable() -> Fallible {
    let mut lifecycle = Lifecycle::new();
    lifecycle.step(Event::Challenge)?;
    lifecycle.step(Event::Decompose)?;
    lifecycle.step(Event::Prove)?;
    assert_eq!(
        lifecycle.step(Event::Check {
            metrics: passing()?,
            elapsed_ms: 0,
        })?,
        Stage::Publish
    );

    assert_eq!(InvalidationReason::ALL.len(), 3);
    let tags: Vec<u8> = InvalidationReason::ALL
        .into_iter()
        .map(InvalidationReason::tag)
        .collect();
    assert_eq!(tags, vec![1, 2, 3]);
    for reason in InvalidationReason::ALL {
        assert!(!reason.name().is_empty());
    }
    assert_eq!(
        format!("{}", InvalidationReason::ParetoBreach),
        "pareto-breach"
    );
    Ok(())
}

/// Boundary: a candidate may be withdrawn from any stage before the check.
#[test]
fn a_candidate_may_be_withdrawn_from_any_earlier_stage() -> Fallible {
    for prefix in 0..4 {
        let mut lifecycle = Lifecycle::new();
        for event in [Event::Challenge, Event::Decompose, Event::Prove]
            .into_iter()
            .take(prefix)
        {
            lifecycle.step(event)?;
        }
        assert_eq!(
            lifecycle.step(Event::Invalidate {
                reason: InvalidationReason::Withdrawn,
            })?,
            Stage::Invalidate
        );
        assert_eq!(lifecycle.metrics(), None);
    }
    Ok(())
}
