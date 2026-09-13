// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E06-1: expert registration and routing.
//!
//! Positive: registration and routing succeed, and routing picks the cheapest
//! expert that fits the declared budget. Negative: routing returns `None` when
//! no expert matches. Boundary: the 32nd expert is accepted and the 33rd
//! refused, and the power envelope is closed at exactly 20 W.
//!
//! Nothing here loads a model or measures a watt.

mod common;

use aegis_minerva::{
    AlpsRouter, ENERGY_WEIGHT_TENTHS, ExpertDomain, ExpertId, LATENCY_WEIGHT_TENTHS, Label,
    MAX_SLM_EXPERTS, MIN_ENVELOPE_MILLIWATTS, MinervaError, POWER_CAP_MILLIWATTS, PowerEnvelope,
    ROUTER_LATENCY_CAP_US, within_routing_budget,
};

use common::{Fallible, budget, expert, filled_router};

// --- Positive -------------------------------------------------------------

/// Positive: registration succeeds and routing returns the registered expert.
#[test]
fn registration_and_routing_succeed() -> Fallible {
    let mut router = AlpsRouter::new();
    assert!(router.is_empty());
    router.register(expert(1, ExpertDomain::CodeSynthesis, 18_500, 450)?)?;
    assert_eq!(router.count(), 1);
    let chosen = router
        .route(ExpertDomain::CodeSynthesis, budget(18_500)?)
        .ok_or("an expert in the domain and inside the budget must be routed")?;
    assert_eq!(chosen.id, ExpertId::new(1));
    assert_eq!(chosen.domain, ExpertDomain::CodeSynthesis);
    assert!(chosen.active);
    assert_eq!(router.routed(), 1);
    assert_eq!(router.get(ExpertId::new(1)), Some(chosen));
    Ok(())
}

/// Positive: a routed expert carries every field it was registered with.
#[test]
fn a_routed_expert_carries_its_registered_fields() -> Fallible {
    let mut router = AlpsRouter::new();
    router.register(expert(1, ExpertDomain::CodeSynthesis, 18_500, 450)?)?;
    let chosen = router
        .route(ExpertDomain::CodeSynthesis, budget(18_500)?)
        .ok_or("the registered expert must be routed")?;
    assert_eq!(chosen.parameter_count_millions, 1_500);
    assert_eq!(chosen.vram_footprint_mib, 1_200);
    assert_eq!(chosen.avg_latency_us, 450);
    assert_eq!(chosen.energy_per_token_ujoule, 120);
    assert_eq!(chosen.draw_milliwatts, 18_500);
    assert_eq!(chosen.name, Label::parse("aegis-slm-code-v1")?);
    Ok(())
}

/// Positive: routing picks the cheapest expert, by the scaffold's weights.
#[test]
fn routing_picks_the_cheapest_expert() -> Fallible {
    let mut router = AlpsRouter::new();
    let expensive = expert(1, ExpertDomain::SymbolicMath, 10_000, 900)?;
    let cheap = expert(2, ExpertDomain::SymbolicMath, 10_000, 280)?;
    router.register(expensive)?;
    router.register(cheap)?;
    assert!(cheap.routing_cost_tenths() < expensive.routing_cost_tenths());
    let chosen = router
        .route(ExpertDomain::SymbolicMath, budget(12_000)?)
        .ok_or("one of the two experts must be routed")?;
    assert_eq!(chosen.id, ExpertId::new(2));
    assert_eq!(
        cheap.routing_cost_tenths(),
        u64::from(cheap.avg_latency_us)
            .saturating_mul(LATENCY_WEIGHT_TENTHS)
            .saturating_add(
                u64::from(cheap.energy_per_token_ujoule).saturating_mul(ENERGY_WEIGHT_TENTHS)
            )
    );
    Ok(())
}

/// Positive: the recorded routing budget admits a value at the cap.
#[test]
fn the_recorded_routing_budget_admits_its_cap() {
    assert!(within_routing_budget(0));
    assert!(within_routing_budget(ROUTER_LATENCY_CAP_US));
    assert!(!within_routing_budget(
        ROUTER_LATENCY_CAP_US.saturating_add(1)
    ));
    assert_eq!(ROUTER_LATENCY_CAP_US, 1_500);
}

// --- Negative -------------------------------------------------------------

/// Negative: routing returns `None` when no expert matches.
#[test]
fn routing_returns_none_when_no_expert_matches() -> Fallible {
    let mut empty = AlpsRouter::default();
    assert!(
        empty
            .route(ExpertDomain::GeneralReasoning, budget(20_000)?)
            .is_none()
    );

    let mut router = filled_router(2)?;
    assert!(
        router
            .route(ExpertDomain::PolicyGovernance, budget(20_000)?)
            .is_none(),
        "no expert serves the domain"
    );
    assert!(
        router
            .route(ExpertDomain::SystemTuning, budget(20_000)?)
            .is_none()
    );
    assert_eq!(router.routed(), 2, "a miss is still a routed intent");
    Ok(())
}

/// Negative: an inactive expert is not routed, and neither is one that draws
/// more than the declared budget.
#[test]
fn an_inactive_or_over_budget_expert_is_not_routed() -> Fallible {
    let mut router = AlpsRouter::new();
    let mut dormant = expert(1, ExpertDomain::CodeSynthesis, 1_000, 100)?;
    dormant.active = false;
    router.register(dormant)?;
    router.register(expert(2, ExpertDomain::CodeSynthesis, 19_000, 100)?)?;
    assert!(
        router
            .route(ExpertDomain::CodeSynthesis, budget(18_000)?)
            .is_none(),
        "the only expert that fits is dormant and the only active one is over budget"
    );
    assert!(!dormant.fits(budget(999)?));
    assert!(dormant.fits(budget(1_000)?));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the 32nd expert is accepted and the 33rd is refused.
#[test]
fn the_thirty_third_expert_is_refused() -> Fallible {
    let mut router = filled_router(MAX_SLM_EXPERTS.saturating_sub(1))?;
    assert_eq!(router.count(), 31);
    router.register(expert(31, ExpertDomain::CodeSynthesis, 18_500, 450)?)?;
    assert_eq!(router.count(), MAX_SLM_EXPERTS);
    let refused = router.register(expert(32, ExpertDomain::CodeSynthesis, 18_500, 450)?);
    assert_eq!(
        refused,
        Err(MinervaError::ExpertTableFull {
            max: MAX_SLM_EXPERTS
        })
    );
    assert_eq!(
        router.count(),
        MAX_SLM_EXPERTS,
        "the refusal stores nothing"
    );
    assert!(
        router
            .route(ExpertDomain::CodeSynthesis, budget(20_000)?)
            .is_some()
    );
    Ok(())
}

/// Boundary: the power envelope is closed at exactly the 20 W cap.
#[test]
fn the_power_envelope_is_closed_at_the_cap() -> Fallible {
    assert_eq!(
        PowerEnvelope::new(POWER_CAP_MILLIWATTS)?,
        PowerEnvelope::CAP
    );
    assert_eq!(PowerEnvelope::CAP.milliwatts(), 20_000);
    assert_eq!(
        PowerEnvelope::new(MIN_ENVELOPE_MILLIWATTS)?.milliwatts(),
        MIN_ENVELOPE_MILLIWATTS
    );
    assert_eq!(
        PowerEnvelope::new(POWER_CAP_MILLIWATTS.saturating_add(1)),
        Err(MinervaError::PowerEnvelopeOutOfRange {
            milliwatts: 20_001,
            min: MIN_ENVELOPE_MILLIWATTS,
            max: POWER_CAP_MILLIWATTS,
        })
    );
    assert!(PowerEnvelope::new(0).is_err());
    Ok(())
}

/// Boundary: an expert drawing exactly the budget is routed and one milliwatt
/// more is not.
#[test]
fn an_expert_at_the_budget_is_routed_and_one_over_is_not() -> Fallible {
    let mut router = AlpsRouter::new();
    router.register(expert(1, ExpertDomain::GeneralReasoning, 12_000, 100)?)?;
    assert!(
        router
            .route(ExpertDomain::GeneralReasoning, budget(12_000)?)
            .is_some()
    );
    assert!(
        router
            .route(ExpertDomain::GeneralReasoning, budget(11_999)?)
            .is_none()
    );
    Ok(())
}
