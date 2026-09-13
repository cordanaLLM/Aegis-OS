// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded Alps expert router (REQ-P09-01, REQ-P09-02, REQ-P09-07).
//!
//! This is the scaffold's `AlpsRouter` with four changes, each of which makes
//! something checkable that was not:
//!
//! * the capacity refusal is a typed error rather than a `&'static str`;
//! * the power envelope is a validated value in milliwatts instead of an `f64`
//!   compared against another `f64`, so the 20 W cap of REQ-P09-01 is an exact
//!   boundary rather than a floating-point one;
//! * the routing cost is integer arithmetic in the same weights the scaffold
//!   uses, so two experts can be ordered without a float comparison;
//! * the latency cap of REQ-P09-02 is a predicate over a number a caller
//!   supplies, because nothing here has a clock.
//!
//! **No inference happens here, and no model is loaded.** An expert is a row
//! of numbers; routing picks a row. There is no `GPU`, no accelerator context,
//! no weight streaming and no power measurement: the envelope is a budget
//! declared by a caller and checked against a recorded constant.

use crate::error::MinervaError;
use crate::id::Label;

/// Scalar upper bound on the expert table (the scaffold's `MAX_SLM_EXPERTS`).
pub const MAX_SLM_EXPERTS: usize = 32;

/// The biomimetic power cap, in milliwatts (REQ-P09-01).
///
/// The source states 20.0 W as an `f64` constant. Milliwatts keep it exact:
/// the boundary between an admissible envelope and an inadmissible one is a
/// comparison of two integers, so it cannot move with the rounding of a
/// literal. Nothing here measures power, and this crate cannot enforce the cap
/// on anything -- it refuses to hand out an expert for a budget that exceeds
/// it, which is a bound on a request and not on a watt.
pub const POWER_CAP_MILLIWATTS: u32 = 20_000;

/// The smallest power envelope a caller may declare, in milliwatts.
pub const MIN_ENVELOPE_MILLIWATTS: u32 = 1;

/// The routing overhead cap, in microseconds (REQ-P09-02).
pub const ROUTER_LATENCY_CAP_US: u64 = 1_500;

/// The weight the routing cost gives average latency, in tenths.
pub const LATENCY_WEIGHT_TENTHS: u64 = 4;

/// The weight the routing cost gives energy per token, in tenths.
pub const ENERGY_WEIGHT_TENTHS: u64 = 6;

/// Returns `true` when an observed routing overhead is inside REQ-P09-02.
///
/// The number comes from the caller. This crate starts no timer, reads no
/// clock and routes nothing that takes time, so the predicate is the rule a
/// measurement would be judged by and never a measurement itself.
#[must_use]
pub const fn within_routing_budget(observed_us: u64) -> bool {
    observed_us <= ROUTER_LATENCY_CAP_US
}

/// The domains an expert may serve.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ExpertDomain {
    /// Generating or transforming code.
    CodeSynthesis,
    /// Policy and governance questions.
    PolicyGovernance,
    /// Symbolic mathematics.
    SymbolicMath,
    /// Tuning the running system.
    SystemTuning,
    /// Everything else.
    GeneralReasoning,
}

impl ExpertDomain {
    /// Every domain, in the order the scaffold declares them.
    pub const ALL: [Self; 5] = [
        Self::CodeSynthesis,
        Self::PolicyGovernance,
        Self::SymbolicMath,
        Self::SystemTuning,
        Self::GeneralReasoning,
    ];

    /// Returns the stable name this domain is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CodeSynthesis => "code-synthesis",
            Self::PolicyGovernance => "policy-governance",
            Self::SymbolicMath => "symbolic-math",
            Self::SystemTuning => "system-tuning",
            Self::GeneralReasoning => "general-reasoning",
        }
    }
}

/// An expert identifier.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct ExpertId(u32);

impl ExpertId {
    /// Names an expert by its raw identifier.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// A validated power budget, in milliwatts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PowerEnvelope(u32);

impl PowerEnvelope {
    /// The largest envelope REQ-P09-01 admits.
    pub const CAP: Self = Self(POWER_CAP_MILLIWATTS);

    /// Validates `milliwatts` against
    /// `MIN_ENVELOPE_MILLIWATTS..=POWER_CAP_MILLIWATTS`.
    ///
    /// # Errors
    ///
    /// Returns [`MinervaError::PowerEnvelopeOutOfRange`] outside that range.
    /// The scaffold instead compares the budget inside the routing loop and
    /// silently returns no expert, which makes an over-budget request
    /// indistinguishable from a domain with no expert in it.
    pub const fn new(milliwatts: u32) -> Result<Self, MinervaError> {
        if milliwatts < MIN_ENVELOPE_MILLIWATTS || milliwatts > POWER_CAP_MILLIWATTS {
            return Err(MinervaError::PowerEnvelopeOutOfRange {
                milliwatts,
                min: MIN_ENVELOPE_MILLIWATTS,
                max: POWER_CAP_MILLIWATTS,
            });
        }
        Ok(Self(milliwatts))
    }

    /// Returns the validated budget in milliwatts.
    #[must_use]
    pub const fn milliwatts(self) -> u32 {
        self.0
    }
}

/// One ephemeral tiny expert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SlmExpert {
    /// The expert identifier.
    pub id: ExpertId,
    /// The expert name.
    pub name: Label,
    /// The domain the expert serves.
    pub domain: ExpertDomain,
    /// The parameter count, in millions.
    pub parameter_count_millions: u32,
    /// The resident weight footprint, in mebibytes.
    pub vram_footprint_mib: u32,
    /// The average recorded latency, in microseconds.
    pub avg_latency_us: u32,
    /// The recorded energy per token, in microjoules.
    pub energy_per_token_ujoule: u32,
    /// The power the expert draws while resident, in milliwatts.
    pub draw_milliwatts: u32,
    /// Whether the expert is available to the router.
    pub active: bool,
}

impl SlmExpert {
    /// Returns the multi-objective routing cost, in tenths of a unit.
    ///
    /// The scaffold weights latency by 0.4 and energy by 0.6 in `f64`. The same
    /// weights as integer tenths give the same ordering without a float
    /// comparison, which matters because `clippy::float_cmp` is denied here for
    /// a reason: two experts whose costs differ in the last bit of a `f64`
    /// would order arbitrarily.
    ///
    /// Saturating on purpose. The inputs are bounded by the table that holds
    /// them, so saturation is unreachable for any expert a caller can register
    /// and a saturated cost would still sort last rather than wrap to first.
    #[must_use]
    pub fn routing_cost_tenths(&self) -> u64 {
        let latency = u64::from(self.avg_latency_us).saturating_mul(LATENCY_WEIGHT_TENTHS);
        let energy = u64::from(self.energy_per_token_ujoule).saturating_mul(ENERGY_WEIGHT_TENTHS);
        latency.saturating_add(energy)
    }

    /// Returns `true` when this expert fits inside `envelope`.
    ///
    /// The scaffold compares the caller's whole budget against the 20 W cap
    /// and never looks at the expert at all, so every expert fits every budget
    /// under the cap. Comparing the expert's own draw against the declared
    /// budget is what makes the envelope of REQ-P09-01 select anything.
    #[must_use]
    pub const fn fits(&self, envelope: PowerEnvelope) -> bool {
        self.draw_milliwatts <= envelope.milliwatts()
    }
}

/// The bounded expert table.
///
/// `Copy`, like every value on a decision path in this crate: the whole table
/// is a fixed array, so registering an expert allocates nothing.
#[derive(Debug, Clone, Copy)]
pub struct AlpsRouter {
    experts: [Option<SlmExpert>; MAX_SLM_EXPERTS],
    count: usize,
    routed: u64,
}

impl Default for AlpsRouter {
    fn default() -> Self {
        Self::new()
    }
}

impl AlpsRouter {
    /// Builds an empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            experts: [None; MAX_SLM_EXPERTS],
            count: 0,
            routed: 0,
        }
    }

    /// Registers one expert.
    ///
    /// # Errors
    ///
    /// Returns [`MinervaError::ExpertTableFull`] at [`MAX_SLM_EXPERTS`].
    pub fn register(&mut self, expert: SlmExpert) -> Result<(), MinervaError> {
        let slot = self
            .experts
            .get_mut(self.count)
            .ok_or(MinervaError::ExpertTableFull {
                max: MAX_SLM_EXPERTS,
            })?;
        *slot = Some(expert);
        self.count = self.count.saturating_add(1);
        Ok(())
    }

    /// Routes one intent to the cheapest active expert in `domain` that fits
    /// `envelope`.
    ///
    /// Returns `None` when no active expert serves the domain, and also when
    /// every expert that does serve it draws more than the declared budget.
    /// The sweep is bounded by [`MAX_SLM_EXPERTS`]; ties are broken by
    /// [`ExpertId`] so the answer does not depend on registration order.
    pub fn route(&mut self, domain: ExpertDomain, envelope: PowerEnvelope) -> Option<SlmExpert> {
        self.routed = self.routed.saturating_add(1);
        self.experts
            .iter()
            .take(MAX_SLM_EXPERTS)
            .flatten()
            .filter(|expert| expert.active && expert.domain == domain && expert.fits(envelope))
            .min_by_key(|expert| (expert.routing_cost_tenths(), expert.id))
            .copied()
    }

    /// Returns how many experts the table holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns `true` when the table holds no expert.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns how many intents have been routed.
    ///
    /// A plain counter, not the scaffold's atomic: this slice is
    /// single-threaded by construction, and an atomic would suggest a
    /// concurrency story the milestone does not deliver.
    #[must_use]
    pub const fn routed(&self) -> u64 {
        self.routed
    }

    /// Returns the expert with `id`, when the table holds one.
    #[must_use]
    pub fn get(&self, id: ExpertId) -> Option<SlmExpert> {
        self.experts
            .iter()
            .take(MAX_SLM_EXPERTS)
            .flatten()
            .find(|expert| expert.id == id)
            .copied()
    }
}
