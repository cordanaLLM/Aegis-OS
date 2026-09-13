// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What a measured worst-case wakeup latency says about each P07 tier edge
//! (milestone M23, REQ-P07-01, REQ-P07-06).
//!
//! # The question this module answers, and the one it does not
//!
//! [`crate::tier`] classifies a *burst duration*: how long a task ran before it
//! yielded. `cyclictest` measures a *wakeup latency*: how late a thread that
//! asked to be woken at time `t` was actually woken. They are different
//! quantities and this module does not pretend otherwise.
//!
//! What relates them is a floor. A scheduler that sorts work into a tier whose
//! edge is 100 microseconds is describing a commitment at that time scale, and
//! a kernel whose own worst-case wakeup latency exceeds 100 microseconds cannot
//! keep such a commitment however the classifier sorts: the task is not running
//! yet. So a measured worst case is read here as **the smallest tier edge the
//! kernel could honour**, not as a burst duration and not as a P08 round-trip
//! latency, which also includes the device and the graph.
//!
//! That reading is stated so it can be disputed. What it rests on:
//! [`Tier::upper_edge_ns`] for the edges, the strict comparison
//! [`Tier::classify`] already uses, and one `cyclictest` run per kernel. What
//! it does not rest on: any measurement of `scx_cake` itself -- no eBPF program
//! is loaded here, no dispatch queue exists, and M19 is where an object goes
//! through a verifier.
//!
//! # Why the kernel decides before the number does
//!
//! [`TierDeterminism::evaluate`] hands the whole decision to
//! [`DeterminismVerdict::of`], which checks the kernel first. On the reference
//! profile that is not a formality: in the recorded run the non-realtime host
//! measured a **lower** worst case than the realtime guest, because the guest's
//! virtual CPU is itself scheduled by that host. Ranking the two by their
//! figures would put the machine without `CONFIG_PREEMPT_RT` ahead -- and would
//! not even do so consistently, since one of the five runs recorded in
//! `docs/build/latency.md` reversed the order. No method here ranks them by
//! figure.
//!
//! # What this module does not do
//!
//! It measures nothing, reads no clock and loads nothing. Every figure reaching
//! it was produced by `make verify-latency` on the machine named in its own
//! [`KernelIdentity`].

use aegis_calliope::{DeterminismVerdict, KernelIdentity, Measured, TierAttainment};

use crate::tier::Tier;

/// How many tiers carry a strict upper edge.
///
/// Three of the four: [`Tier::Bulk`] is where everything above the last
/// threshold lands, so it has no edge and no determinism claim attaches to it.
pub const BOUNDED_TIER_COUNT: usize = 3;

/// One tier's edge, and what one measured worst case did against it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TierDeterminism {
    /// The tier whose upper edge was tested.
    pub tier: Tier,
    /// That edge, in nanoseconds.
    pub threshold_ns: u64,
    /// The kernel the figure came from.
    pub kernel: KernelIdentity,
    /// The observed worst case, in nanoseconds.
    pub worst_ns: u64,
    /// Where the figure sits against the edge, ignoring the kernel.
    ///
    /// On its own this field says nothing about a determinism claim, and the
    /// pairing it produces is the intended reading rather than a
    /// contradiction: a figure below the edge is
    /// [`TierAttainment::Attained`] whatever kernel produced it, so a row for
    /// the non-realtime reference host reads `Attained` beside a
    /// [`DeterminismVerdict::KernelNotRealtime`] verdict. The kernel is
    /// checked before the figure, and only the `verdict` field feeds
    /// [`DeterminismVerdict::satisfies`]; nothing in this crate reads
    /// `attainment` to decide anything.
    pub attainment: TierAttainment,
    /// Whether the figure satisfies a determinism claim at that edge.
    pub verdict: DeterminismVerdict,
}

impl TierDeterminism {
    /// Evaluates `worst` against `tier`'s upper edge.
    ///
    /// Returns `None` for [`Tier::Bulk`], which has no upper edge: a tier with
    /// no threshold has nothing for a worst case to attain, and reporting one
    /// as attained would be a claim about an edge that does not exist.
    #[must_use]
    pub const fn evaluate(tier: Tier, worst: Measured<u64>) -> Option<Self> {
        let Some(threshold_ns) = tier.upper_edge_ns() else {
            return None;
        };
        Some(Self {
            tier,
            threshold_ns,
            kernel: worst.kernel(),
            worst_ns: worst.copied(),
            attainment: worst.attainment(threshold_ns),
            verdict: worst.verdict(threshold_ns),
        })
    }
}

/// Evaluates `worst` against every tier, in [`Tier::ALL`] order.
///
/// The array is fixed at four entries and the [`Tier::Bulk`] entry is always
/// `None`, so a caller sweeping it sees every tier and cannot mistake the
/// unbounded one for a tier that was tested and passed.
#[must_use]
pub fn tier_determinism(worst: Measured<u64>) -> [Option<TierDeterminism>; 4] {
    Tier::ALL.map(|tier| TierDeterminism::evaluate(tier, worst))
}

/// Returns the strictest tier `worst` satisfies a determinism claim at.
///
/// [`Tier::ALL`] is ordered highest priority first, so the first satisfied
/// entry is the strictest. A figure from a kernel that is not `PREEMPT_RT`
/// returns `None` at every edge and therefore `None` here, however small it
/// is; that is the whole point of checking the kernel first.
#[must_use]
pub fn strictest_satisfied_tier(worst: Measured<u64>) -> Option<Tier> {
    tier_determinism(worst)
        .into_iter()
        .flatten()
        .find(|row| row.verdict.satisfies())
        .map(|row| row.tier)
}

/// Returns how many tier edges `worst` satisfies a determinism claim at.
#[must_use]
pub fn satisfied_edge_count(worst: Measured<u64>) -> usize {
    tier_determinism(worst)
        .into_iter()
        .flatten()
        .filter(|row| row.verdict.satisfies())
        .count()
}
