// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the M23 measurement says about each P07 tier edge.
//!
//! `crates/aegis-calliope/tests/measured_figures.rs` holds the rule that turns
//! a figure into a verdict. This file holds the other half: that the rule is
//! applied to **the tier edges this crate actually declares**, that
//! [`Tier::Bulk`] -- which has no edge -- is reported as having none rather
//! than as passing, and that the non-realtime host attains no tier despite
//! having measured the lower worst case.
//!
//! Every threshold is pinned to its literal value here before it is used, so a
//! test cannot pass by comparing a constant with itself.

mod common;

use aegis_calliope::{
    AEGIS_M26_GUEST, DeterminismVerdict, GUEST_WORST_WAKEUP_NS, HOST_WORST_WAKEUP_NS, Measured,
    MeasurementTool, REFERENCE_HOST, TierAttainment,
};
use aegis_lictor::{
    BOUNDED_TIER_COUNT, BURST_CRITICAL_NS, BURST_FRAME_NS, BURST_INTERACTIVE_NS, EwmaBurst, Tier,
    TierDeterminism, satisfied_edge_count, strictest_satisfied_tier, tier_determinism,
};

use common::Fallible;

/// Returns the row for one tier, or a failure naming the tier that has none.
fn row(worst: Measured<u64>, tier: Tier) -> Result<TierDeterminism, Box<dyn std::error::Error>> {
    TierDeterminism::evaluate(tier, worst)
        .ok_or_else(|| format!("{} has no upper edge", tier.name()).into())
}

// --- Positive -------------------------------------------------------------

/// Positive: the three tier edges are the recorded literals, pinned before
/// anything is evaluated against them.
#[test]
fn the_tier_edges_are_the_recorded_literals() {
    assert_eq!(BURST_CRITICAL_NS, 100_000);
    assert_eq!(BURST_INTERACTIVE_NS, 2_000_000);
    assert_eq!(BURST_FRAME_NS, 8_000_000);
    assert_eq!(BOUNDED_TIER_COUNT, 3);
}

/// Positive: the guest figure is evaluated against the edges this crate
/// declares, and each row names the kernel that produced the figure.
#[test]
fn the_guest_figure_is_evaluated_against_the_declared_edges() -> Fallible {
    let critical = row(GUEST_WORST_WAKEUP_NS, Tier::Critical)?;
    assert_eq!(critical.threshold_ns, BURST_CRITICAL_NS);
    assert_eq!(critical.worst_ns, 273_969);
    assert_eq!(critical.kernel, AEGIS_M26_GUEST);
    assert_eq!(critical.attainment, TierAttainment::Exceeded);
    assert_eq!(critical.verdict, DeterminismVerdict::WorstCaseExceeds);
    let interactive = row(GUEST_WORST_WAKEUP_NS, Tier::Interactive)?;
    assert_eq!(interactive.tier, Tier::Interactive);
    assert_eq!(interactive.threshold_ns, BURST_INTERACTIVE_NS);
    assert_eq!(interactive.verdict, DeterminismVerdict::Satisfied);
    Ok(())
}

/// Positive: the strictest tier the guest run satisfies is `Interactive`, and
/// it satisfies two of the three bounded edges.
#[test]
fn the_guest_run_reaches_the_interactive_tier_and_no_further() {
    assert_eq!(
        strictest_satisfied_tier(GUEST_WORST_WAKEUP_NS),
        Some(Tier::Interactive)
    );
    assert_eq!(satisfied_edge_count(GUEST_WORST_WAKEUP_NS), 2);
    assert_eq!(
        tier_determinism(GUEST_WORST_WAKEUP_NS)
            .into_iter()
            .flatten()
            .count(),
        BOUNDED_TIER_COUNT
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: the host attains no tier at all, and the reason is not the figure.
#[test]
fn the_host_attains_no_tier_despite_the_lower_worst_case() -> Fallible {
    assert!(HOST_WORST_WAKEUP_NS.copied() < GUEST_WORST_WAKEUP_NS.copied());
    assert_eq!(strictest_satisfied_tier(HOST_WORST_WAKEUP_NS), None);
    assert_eq!(satisfied_edge_count(HOST_WORST_WAKEUP_NS), 0);
    for tier in [Tier::Critical, Tier::Interactive, Tier::Frame] {
        let found = row(HOST_WORST_WAKEUP_NS, tier)?;
        assert_eq!(found.kernel, REFERENCE_HOST);
        assert_eq!(found.verdict, DeterminismVerdict::KernelNotRealtime);
    }
    // The interactive edge is where the two kernels differ only by provenance:
    // both figures are below it, and only one of them satisfies anything.
    let host = row(HOST_WORST_WAKEUP_NS, Tier::Interactive)?;
    let guest = row(GUEST_WORST_WAKEUP_NS, Tier::Interactive)?;
    assert_eq!(host.attainment, TierAttainment::Attained);
    assert_eq!(guest.attainment, TierAttainment::Attained);
    assert_ne!(host.verdict, guest.verdict);
    Ok(())
}

/// Negative: a tier without an upper edge yields no row, so nothing can be
/// reported as having satisfied a threshold that does not exist.
#[test]
fn the_unbounded_tier_yields_no_row() {
    assert_eq!(Tier::Bulk.upper_edge_ns(), None);
    assert!(TierDeterminism::evaluate(Tier::Bulk, GUEST_WORST_WAKEUP_NS).is_none());
    let rows = tier_determinism(GUEST_WORST_WAKEUP_NS);
    assert_eq!(rows.len(), Tier::ALL.len());
    assert_eq!(rows.iter().filter(|row| row.is_none()).count(), 1);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a worst case one nanosecond under the critical edge reaches the
/// critical tier; exactly on it does not, and one past it does not either, and
/// the three are reported by three different names.
#[test]
fn the_critical_edge_is_strict_in_both_directions() {
    assert_eq!(BURST_CRITICAL_NS, 100_000);
    let measured = |value: u64| Measured::new(value, AEGIS_M26_GUEST, MeasurementTool::Cyclictest);
    let below = measured(BURST_CRITICAL_NS.saturating_sub(1));
    let exact = measured(BURST_CRITICAL_NS);
    let beyond = measured(BURST_CRITICAL_NS.saturating_add(1));
    assert_eq!(strictest_satisfied_tier(below), Some(Tier::Critical));
    assert_eq!(strictest_satisfied_tier(exact), Some(Tier::Interactive));
    assert_eq!(strictest_satisfied_tier(beyond), Some(Tier::Interactive));
    let names = [below, exact, beyond].map(|one| {
        TierDeterminism::evaluate(Tier::Critical, one)
            .map_or("no-edge", |found| found.verdict.name())
    });
    assert_eq!(
        names,
        ["satisfied", "worst-case-at-edge", "worst-case-exceeds"]
    );
}

/// Boundary: the classifier agrees with the fixture at the same edge, so the
/// two cannot disagree about what "exactly 100 microseconds" means.
#[test]
fn the_classifier_places_the_edge_the_way_the_fixture_reports_it() {
    assert_eq!(BURST_CRITICAL_NS, 100_000);
    assert_eq!(
        Tier::classify(EwmaBurst::seeded(BURST_CRITICAL_NS)),
        Tier::Interactive
    );
    assert_eq!(
        Tier::classify(EwmaBurst::seeded(BURST_CRITICAL_NS.saturating_sub(1))),
        Tier::Critical
    );
}
