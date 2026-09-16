// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The falsifier for "a figure from the non-realtime host cannot satisfy a
//! determinism claim".
//!
//! The claim M23 has to make good is not that the realtime guest measured a
//! smaller number -- it did not. In the recorded run the non-realtime host
//! measured the *lower* worst case of the two, because the guest's virtual CPU
//! is scheduled by that host, and across the five runs
//! `docs/build/latency.md` records the ordering flipped once. So the separation
//! cannot be a comparison of figures, and this file is where that is held:
//!
//! * the positive tests pin what the guest run satisfies and what it does not;
//! * the negative tests pin that the host's figure satisfies nothing at any
//!   threshold **and** that it is the smaller figure, so the two facts are
//!   asserted together and neither can be quietly dropped;
//! * the boundary tests pin the strict rule at a threshold, one nanosecond
//!   below it and one nanosecond past it, with the threshold itself pinned to
//!   its literal value rather than to itself.
//!
//! Every literal here is the recorded value of one run on 2026-09-13, named in
//! the constant's own documentation. Re-running `make verify-latency` produces
//! different integers and the gate does not compare against these; what this
//! file holds is the rule that turns a figure into a verdict.

use aegis_calliope::{
    AEGIS_M26_GUEST, Declared, DeterminismVerdict, GUEST_WORST_WAKEUP_NS, HOST_WORST_WAKEUP_NS,
    KernelIdentity, MAX_KERNEL_RELEASE_LEN, Measured, MeasurementTool, NANOS_PER_MILLI,
    PreemptionModel, REFERENCE_HOST, TARGET_RTL_LATENCY_MS, TARGET_RTL_LATENCY_NS, TierAttainment,
};

/// The P07 critical-tier edge, in nanoseconds.
///
/// Restated here rather than imported because `aegis-calliope` does not depend
/// on `aegis-lictor` -- the dependency runs the other way.
/// `crates/aegis-lictor/tests/determinism_fixture.rs` pins the same number
/// against the constant it belongs to, and `tools/verify_latency_fixture.py`
/// reads it out of that crate's source instead of restating it at all.
const CRITICAL_EDGE_NS: u64 = 100_000;

/// Accepts only a value already marked as observed rather than declared.
const fn require_measured(value: Measured<u64>) -> u64 {
    value.copied()
}

// --- Positive -------------------------------------------------------------

/// Positive: the guest figure names the kernel that produced it, and that
/// kernel is the one M26 builds.
#[test]
fn the_guest_figure_names_the_kernel_that_produced_it() {
    assert_eq!(require_measured(GUEST_WORST_WAKEUP_NS), 273_969);
    assert_eq!(GUEST_WORST_WAKEUP_NS.kernel(), AEGIS_M26_GUEST);
    assert_eq!(GUEST_WORST_WAKEUP_NS.kernel().release(), "7.2.5-aegis-m26");
    assert_eq!(
        GUEST_WORST_WAKEUP_NS.kernel().preemption(),
        PreemptionModel::PreemptRt
    );
    assert_eq!(GUEST_WORST_WAKEUP_NS.tool(), MeasurementTool::Cyclictest);
    assert_eq!(GUEST_WORST_WAKEUP_NS.tool().admitted_version(), "2.10");
    assert!(GUEST_WORST_WAKEUP_NS.tool().measures().contains("wakeup"));
    assert!(GUEST_WORST_WAKEUP_NS.kernel().is_realtime());
}

/// Positive: rendering a measured value carries its provenance, the way
/// rendering a declared one carries its absence.
#[test]
fn rendering_a_measured_value_names_its_tool_and_kernel() {
    assert_eq!(
        GUEST_WORST_WAKEUP_NS.to_string(),
        "273969 (measured by cyclictest on 7.2.5-aegis-m26, PREEMPT_RT)"
    );
    assert_eq!(
        HOST_WORST_WAKEUP_NS.to_string(),
        "267461 (measured by cyclictest on 7.2.5-1-cachyos, not PREEMPT_RT)"
    );
}

/// Positive: the guest run satisfies three of the four thresholds, and the one
/// it does not is named rather than rounded away.
#[test]
fn the_guest_run_satisfies_every_threshold_but_the_critical_edge() {
    assert_eq!(
        GUEST_WORST_WAKEUP_NS.verdict(CRITICAL_EDGE_NS),
        DeterminismVerdict::WorstCaseExceeds
    );
    for threshold in [2_000_000, 8_000_000, TARGET_RTL_LATENCY_NS.copied()] {
        assert_eq!(
            GUEST_WORST_WAKEUP_NS.verdict(threshold),
            DeterminismVerdict::Satisfied,
            "the guest run should satisfy {threshold} ns"
        );
        assert!(GUEST_WORST_WAKEUP_NS.verdict(threshold).satisfies());
    }
}

/// Positive: the recorded identities carry the two probe spellings, which is
/// what lets the guest reading and the host reading be compared as text.
#[test]
fn the_recorded_identities_carry_the_two_probe_lines() {
    assert_eq!(
        AEGIS_M26_GUEST.preemption().config_line(),
        "CONFIG_PREEMPT_RT=y"
    );
    assert_eq!(
        REFERENCE_HOST.preemption().config_line(),
        "# CONFIG_PREEMPT_RT is not set"
    );
    assert_eq!(AEGIS_M26_GUEST.to_string(), "7.2.5-aegis-m26, PREEMPT_RT");
    assert_eq!(PreemptionModel::ALL.len(), 2);
}

/// Positive: both recorded releases fit the bound the kernel's own UTS field
/// imposes, and the admitted tool set is the one the matrix records.
#[test]
fn the_recorded_releases_fit_the_uts_bound() {
    assert_eq!(MAX_KERNEL_RELEASE_LEN, 64);
    for identity in [AEGIS_M26_GUEST, REFERENCE_HOST] {
        assert!(
            identity.release_within_bound(),
            "{identity} is out of bound"
        );
    }
    assert_eq!(MeasurementTool::ALL, [MeasurementTool::Cyclictest]);
    assert_eq!(MeasurementTool::Cyclictest.name(), "cyclictest");
    assert_eq!(MeasurementTool::Cyclictest.to_string(), "cyclictest");
}

// --- Negative -------------------------------------------------------------

/// Negative: the host figure is the **smaller** of the two and satisfies
/// nothing.
///
/// Both halves are asserted in one test on purpose. Splitting them would let
/// the ordering assertion be deleted on its own, and the ordering is what makes
/// the verdict assertion mean something: a rule that only refused larger
/// numbers would pass the second half and fail the first.
#[test]
fn the_host_figure_is_the_smaller_one_and_still_satisfies_nothing() {
    assert!(
        HOST_WORST_WAKEUP_NS.copied() < GUEST_WORST_WAKEUP_NS.copied(),
        "the recorded host worst case {} is not below the guest's {}",
        HOST_WORST_WAKEUP_NS.copied(),
        GUEST_WORST_WAKEUP_NS.copied()
    );
    for threshold in [
        CRITICAL_EDGE_NS,
        2_000_000,
        8_000_000,
        TARGET_RTL_LATENCY_NS.copied(),
        u64::MAX,
    ] {
        assert_eq!(
            HOST_WORST_WAKEUP_NS.verdict(threshold),
            DeterminismVerdict::KernelNotRealtime,
            "the host figure decided something other than kernel-not-realtime at {threshold}"
        );
        assert!(!HOST_WORST_WAKEUP_NS.verdict(threshold).satisfies());
    }
}

/// Negative: the kernel is checked before the figure, so a host reading of zero
/// nanoseconds is still refused.
///
/// Zero is the best conceivable reading. If the rule ever ordered the two
/// kernels by figure, this is the case that would flip.
#[test]
fn a_perfect_reading_on_a_non_realtime_kernel_is_still_refused() {
    let perfect = Measured::new(0_u64, REFERENCE_HOST, MeasurementTool::Cyclictest);
    assert_eq!(
        perfect.attainment(CRITICAL_EDGE_NS),
        TierAttainment::Attained
    );
    assert_eq!(
        perfect.verdict(CRITICAL_EDGE_NS),
        DeterminismVerdict::KernelNotRealtime
    );
    assert_eq!(
        DeterminismVerdict::of(REFERENCE_HOST, TierAttainment::Attained),
        DeterminismVerdict::KernelNotRealtime
    );
    assert_eq!(
        DeterminismVerdict::of(AEGIS_M26_GUEST, TierAttainment::Attained),
        DeterminismVerdict::Satisfied
    );
    assert_eq!(DeterminismVerdict::ALL.len(), 4);
    assert_eq!(
        DeterminismVerdict::KernelNotRealtime.name(),
        "kernel-not-realtime"
    );
    assert_eq!(
        DeterminismVerdict::KernelNotRealtime.to_string(),
        "kernel-not-realtime"
    );
}

/// Negative: a target is not made into an observation by being compared
/// against one.
///
/// [`TARGET_RTL_LATENCY_NS`] stays a [`Declared`] value, and its rendering
/// still says so. The type separation is the point: `require_measured` above
/// takes a `Measured<u64>` and nothing in this crate converts a `Declared<T>`
/// into one.
#[test]
fn the_round_trip_target_is_still_declared() {
    let millis: Declared<u32> = TARGET_RTL_LATENCY_MS;
    assert_eq!(millis.copied(), 5);
    assert_eq!(NANOS_PER_MILLI, 1_000_000);
    assert_eq!(TARGET_RTL_LATENCY_NS.copied(), 5_000_000);
    assert_eq!(
        TARGET_RTL_LATENCY_NS.copied(),
        u64::from(millis.copied()).saturating_mul(NANOS_PER_MILLI)
    );
    assert!(
        TARGET_RTL_LATENCY_NS
            .to_string()
            .ends_with("(declared, unmeasured)")
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: at the threshold, one nanosecond below it and one past it are
/// three different outcomes, and the threshold is pinned to its own value.
#[test]
fn the_three_points_around_a_threshold_report_differently() {
    assert_eq!(CRITICAL_EDGE_NS, 100_000);
    let below = CRITICAL_EDGE_NS.saturating_sub(1);
    let beyond = CRITICAL_EDGE_NS.saturating_add(1);
    assert_eq!(below, 99_999);
    assert_eq!(beyond, 100_001);
    let placed = [
        TierAttainment::evaluate(CRITICAL_EDGE_NS, below),
        TierAttainment::evaluate(CRITICAL_EDGE_NS, CRITICAL_EDGE_NS),
        TierAttainment::evaluate(CRITICAL_EDGE_NS, beyond),
    ];
    assert_eq!(
        placed,
        [
            TierAttainment::Attained,
            TierAttainment::AtEdge,
            TierAttainment::Exceeded
        ]
    );
    let decided = placed.map(|one| DeterminismVerdict::of(AEGIS_M26_GUEST, one));
    assert_eq!(
        decided,
        [
            DeterminismVerdict::Satisfied,
            DeterminismVerdict::WorstCaseAtEdge,
            DeterminismVerdict::WorstCaseExceeds
        ]
    );
    assert_eq!(
        placed.map(TierAttainment::name),
        ["attained", "at-edge", "exceeded"]
    );
}

/// Boundary: a worst case exactly on a threshold is refused, because the
/// classifier this fixture reports against compares strictly.
///
/// `Tier::classify` in `aegis-lictor` writes `ewma < BURST_CRITICAL_NS`, so a
/// burst of exactly the edge is not in that tier. A fixture that reported the
/// edge as attained would claim a tier the classifier would not agree with.
#[test]
fn a_worst_case_exactly_on_the_edge_is_refused_by_its_own_name() {
    let exact = Measured::new(
        CRITICAL_EDGE_NS,
        AEGIS_M26_GUEST,
        MeasurementTool::Cyclictest,
    );
    assert_eq!(exact.attainment(CRITICAL_EDGE_NS), TierAttainment::AtEdge);
    assert_eq!(
        exact.verdict(CRITICAL_EDGE_NS),
        DeterminismVerdict::WorstCaseAtEdge
    );
    assert_eq!(exact.verdict(CRITICAL_EDGE_NS).name(), "worst-case-at-edge");
    assert!(!exact.verdict(CRITICAL_EDGE_NS).satisfies());
    assert_eq!(*exact.get(), CRITICAL_EDGE_NS);
}

/// Boundary: an empty release is out of bound at the bottom, and the weaker
/// preemption model renders and answers as itself.
#[test]
fn the_identity_constructor_is_exact_at_its_extremes() {
    assert_eq!(TierAttainment::ALL.len(), 3);
    assert_eq!(TierAttainment::AtEdge.to_string(), "at-edge");
    assert_eq!(
        KernelIdentity::new("x", PreemptionModel::NotPreemptRt).release(),
        "x"
    );
    assert!(KernelIdentity::new("x", PreemptionModel::NotPreemptRt).release_within_bound());
    assert!(!KernelIdentity::new("", PreemptionModel::PreemptRt).release_within_bound());
    assert!(!PreemptionModel::NotPreemptRt.is_realtime());
    assert_eq!(PreemptionModel::NotPreemptRt.to_string(), "not PREEMPT_RT");
    assert_eq!(PreemptionModel::PreemptRt.name(), "PREEMPT_RT");
}
