// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-3, the classifier half: the four-tier EWMA burst classification.
//!
//! Positive: a burst below each threshold classifies as the source's
//! comparison chain classifies it. Negative: a value that is not below a
//! threshold does not get that threshold's tier, and the running average is
//! not the last sample. Boundary: **a burst of exactly a threshold belongs to
//! the tier above it**, because the source writes `<` and not `<=`, and all
//! three edges are pinned here rather than the one a reader might check.
//!
//! Nothing in this file reads a clock. A burst duration is a number handed in,
//! and no latency or determinism figure is produced.

mod common;

use aegis_lictor::{
    BURST_CRITICAL_NS, BURST_FRAME_NS, BURST_INTERACTIVE_NS, DispatchQueue, EWMA_DIVISOR,
    EWMA_WEIGHT_NEW, EWMA_WEIGHT_OLD, EwmaBurst, Tier,
};

// --- Positive -------------------------------------------------------------

/// Positive: a burst below each threshold classifies per the source.
#[test]
fn bursts_below_each_threshold_classify_per_the_source() {
    let cases = [
        (0u64, Tier::Critical),
        (1, Tier::Critical),
        (99_999, Tier::Critical),
        (100_001, Tier::Interactive),
        (1_999_999, Tier::Interactive),
        (2_000_001, Tier::Frame),
        (7_999_999, Tier::Frame),
        (8_000_001, Tier::Bulk),
        (u64::MAX, Tier::Bulk),
    ];
    for (nanos, expected) in cases {
        assert_eq!(
            Tier::classify(EwmaBurst::seeded(nanos)),
            expected,
            "a running average of {nanos} ns should classify as {expected:?}"
        );
    }
}

/// Positive: the thresholds are the values the source declares.
#[test]
fn the_thresholds_are_the_recorded_values() {
    assert_eq!(BURST_CRITICAL_NS, 100_000);
    assert_eq!(BURST_INTERACTIVE_NS, 2_000_000);
    assert_eq!(BURST_FRAME_NS, 8_000_000);
    assert_eq!(EWMA_WEIGHT_OLD, 3);
    assert_eq!(EWMA_WEIGHT_NEW, 1);
    assert_eq!(EWMA_DIVISOR, 4);
    assert_eq!(
        EWMA_WEIGHT_OLD.saturating_add(EWMA_WEIGHT_NEW),
        EWMA_DIVISOR
    );
}

/// Positive: each tier names a distinct queue, index and name, in strict
/// arbitration order.
#[test]
fn each_tier_names_a_distinct_queue() {
    let queues: Vec<DispatchQueue> = Tier::ALL.iter().map(|t| t.dispatch_queue()).collect();
    assert_eq!(queues, DispatchQueue::ALL.to_vec());
    let ids: Vec<u64> = DispatchQueue::ALL.iter().map(|q| q.id()).collect();
    assert_eq!(ids, vec![0, 1, 2, 3]);
    let indices: Vec<u32> = Tier::ALL.iter().map(|tier| tier.index()).collect();
    assert_eq!(indices, vec![0, 1, 2, 3]);
    let mut names: Vec<&str> = Tier::ALL.iter().map(|t| t.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 4);
}

/// Positive: the running average is the source's own recurrence.
///
/// The first sample seeds the average; every later one is `(old * 3 + new) / 4`.
#[test]
fn the_running_average_is_the_source_recurrence() {
    let mut burst = EwmaBurst::new();
    assert_eq!(burst.samples(), 0);
    assert_eq!(burst.nanos(), 0);
    assert_eq!(burst.observe(8_000_000), 8_000_000);
    // (8_000_000 * 3 + 0) / 4 = 6_000_000
    assert_eq!(burst.observe(0), 6_000_000);
    // (6_000_000 * 3 + 0) / 4 = 4_500_000
    assert_eq!(burst.observe(0), 4_500_000);
    assert_eq!(burst.samples(), 3);
    assert_eq!(burst.tier(), Tier::Frame);
}

// --- Negative -------------------------------------------------------------

/// Negative: a fresh average is not a measurement, and the source's own
/// zero-valued context classifies as critical -- which is why the first sample
/// seeds rather than folds.
#[test]
fn a_fresh_average_is_zero_and_seeding_is_what_saves_it() {
    let fresh = EwmaBurst::new();
    assert_eq!(fresh.nanos(), 0);
    assert_eq!(fresh.tier(), Tier::Critical);
    assert_eq!(EwmaBurst::default(), fresh);

    // Folding instead of seeding would put a genuinely bulk task four
    // quarters of the way towards zero on its first observation.
    let mut seeded = EwmaBurst::new();
    assert_eq!(seeded.observe(8_000_000), 8_000_000);
    assert_eq!(seeded.tier(), Tier::Bulk);
}

/// Negative: the top tier has no upper edge, so nothing classifies above it.
#[test]
fn the_bulk_tier_has_no_upper_edge() {
    assert_eq!(Tier::Bulk.upper_edge_ns(), None);
    assert_eq!(Tier::Critical.upper_edge_ns(), Some(BURST_CRITICAL_NS));
    assert_eq!(
        Tier::Interactive.upper_edge_ns(),
        Some(BURST_INTERACTIVE_NS)
    );
    assert_eq!(Tier::Frame.upper_edge_ns(), Some(BURST_FRAME_NS));
    assert_eq!(Tier::classify(EwmaBurst::seeded(u64::MAX)), Tier::Bulk);
}

/// Negative: saturating arithmetic means an absurd burst does not wrap into a
/// lower tier.
#[test]
fn an_absurd_burst_does_not_wrap_into_a_lower_tier() {
    let mut burst = EwmaBurst::seeded(u64::MAX);
    assert_eq!(burst.tier(), Tier::Bulk);
    burst.observe(u64::MAX);
    assert_eq!(burst.tier(), Tier::Bulk);
    assert!(burst.nanos() > BURST_FRAME_NS);
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a burst of exactly a threshold belongs to the tier above.
///
/// The source writes `if (ewma < BURST_CRITICAL_NS)`, so 100000 nanoseconds is
/// interactive and not critical. All three edges are pinned, in both
/// directions, so a later `<=` is a failure here rather than a silently
/// friendlier classifier.
#[test]
fn every_threshold_is_strict() {
    let edges = [
        (BURST_CRITICAL_NS, Tier::Critical, Tier::Interactive),
        (BURST_INTERACTIVE_NS, Tier::Interactive, Tier::Frame),
        (BURST_FRAME_NS, Tier::Frame, Tier::Bulk),
    ];
    for (edge, below, at) in edges {
        assert_eq!(
            Tier::classify(EwmaBurst::seeded(edge.saturating_sub(1))),
            below,
            "one nanosecond below {edge} must still be {below:?}"
        );
        assert_eq!(
            Tier::classify(EwmaBurst::seeded(edge)),
            at,
            "exactly {edge} must be {at:?}, because the source comparison is strict"
        );
    }
}

/// Boundary: the tier a threshold names is the tier whose upper edge it is,
/// and a value at that edge is not in it.
#[test]
fn a_value_at_a_tier_upper_edge_is_not_in_that_tier() {
    for tier in Tier::ALL {
        let Some(edge) = tier.upper_edge_ns() else {
            continue;
        };
        assert_ne!(
            Tier::classify(EwmaBurst::seeded(edge)),
            tier,
            "{tier:?} must not contain its own upper edge"
        );
        assert_eq!(
            Tier::classify(EwmaBurst::seeded(edge.saturating_sub(1))),
            tier
        );
    }
}
