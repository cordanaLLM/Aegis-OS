// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded `AgentHER` trajectory buffer and its relabeller (REQ-P09-06).
//!
//! Hindsight experience replay turns a failed trajectory into a successful
//! goal-conditioned one: the goal is rewritten to the state that was actually
//! reached, and the step that failed against the original goal succeeded
//! against the new one. The scaffold does exactly that, and this module keeps
//! its rule while making two things checkable:
//!
//! * the reward is a fixed-point value rather than an `f32`, so "negative"
//!   is an exact test and relabelling has an exact boundary at zero;
//! * the buffer's capacity refusal is a typed error rather than a
//!   `&'static str`.
//!
//! **Nothing is learned here.** No model is updated, no gradient is computed
//! and no episode is generated. The buffer is a fixed array and relabelling is
//! a bounded sweep over it.

use crate::error::MinervaError;
use crate::id::Label;

/// Scalar upper bound on the buffer (the scaffold's `MAX_TRAJECTORY_STEPS`).
pub const MAX_TRAJECTORY_STEPS: usize = 128;

/// The width of a state hash, in bytes.
pub const STATE_HASH_LEN: usize = 32;

/// A reward, in thousandths.
///
/// The scaffold uses `f32` and compares `reward < 0.0`. Thousandths keep the
/// comparison exact and make the relabelling boundary a property of the type:
/// a reward of exactly zero is not negative, so it is not relabelled, and no
/// rounding can move it across the line.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Reward(i32);

impl Reward {
    /// The reward a relabelled step is given: 1.0.
    pub const SUCCESS: Self = Self(1_000);

    /// The neutral reward: exactly zero, and deliberately not negative.
    pub const NEUTRAL: Self = Self(0);

    /// Records a reward in thousandths.
    #[must_use]
    pub const fn from_thousandths(value: i32) -> Self {
        Self(value)
    }

    /// Returns the reward in thousandths.
    #[must_use]
    pub const fn thousandths(self) -> i32 {
        self.0
    }

    /// Returns `true` when the reward is strictly below zero.
    #[must_use]
    pub const fn is_negative(self) -> bool {
        self.0 < 0
    }
}

/// The hash of a state a trajectory passed through.
///
/// An opaque 32-byte value: this crate computes no hash and interprets none.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct StateHash([u8; STATE_HASH_LEN]);

impl StateHash {
    /// Records a state hash.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; STATE_HASH_LEN]) -> Self {
        Self(bytes)
    }

    /// Returns the recorded bytes.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; STATE_HASH_LEN] {
        &self.0
    }
}

/// One step of an agent trajectory.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TrajectoryStep {
    /// The step's place in the trajectory.
    pub index: u16,
    /// The state the step passed through.
    pub state: StateHash,
    /// What the step did.
    pub action: Label,
    /// What the step earned.
    pub reward: Reward,
    /// Whether the step ended the episode.
    pub terminal: bool,
    /// The goal the step was relabelled against, once it has been.
    pub relabelled_goal: Option<StateHash>,
}

impl TrajectoryStep {
    /// Returns `true` when the step has been relabelled.
    #[must_use]
    pub const fn is_relabelled(&self) -> bool {
        self.relabelled_goal.is_some()
    }
}

/// The bounded trajectory buffer.
#[derive(Debug, Clone, Copy)]
pub struct AgentHerEngine {
    steps: [Option<TrajectoryStep>; MAX_TRAJECTORY_STEPS],
    count: usize,
}

impl Default for AgentHerEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl AgentHerEngine {
    /// Builds an empty buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            steps: [None; MAX_TRAJECTORY_STEPS],
            count: 0,
        }
    }

    /// Records one step.
    ///
    /// # Errors
    ///
    /// Returns [`MinervaError::TrajectoryBufferFull`] at
    /// [`MAX_TRAJECTORY_STEPS`].
    pub fn record(&mut self, step: TrajectoryStep) -> Result<(), MinervaError> {
        let slot = self
            .steps
            .get_mut(self.count)
            .ok_or(MinervaError::TrajectoryBufferFull {
                max: MAX_TRAJECTORY_STEPS,
            })?;
        *slot = Some(step);
        self.count = self.count.saturating_add(1);
        Ok(())
    }

    /// Relabels every failed step against the state that was reached, and
    /// returns how many were relabelled.
    ///
    /// Only a strictly negative reward is relabelled. A neutral or positive
    /// step keeps its reward and its goal, which is what stops the sweep from
    /// rewriting a trajectory that did not fail.
    pub fn relabel(&mut self, achieved: StateHash) -> usize {
        let mut relabelled = 0usize;
        for slot in self.steps.iter_mut().take(MAX_TRAJECTORY_STEPS) {
            if let Some(step) = slot.as_mut()
                && step.reward.is_negative()
            {
                step.relabelled_goal = Some(achieved);
                step.reward = Reward::SUCCESS;
                relabelled = relabelled.saturating_add(1);
            }
        }
        relabelled
    }

    /// Returns how many steps the buffer holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns `true` when the buffer holds no step.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns the step at `position`, when the buffer holds one.
    #[must_use]
    pub fn get(&self, position: usize) -> Option<TrajectoryStep> {
        self.steps.get(position).copied().flatten()
    }

    /// Returns how many recorded steps still carry a negative reward.
    #[must_use]
    pub fn failed(&self) -> usize {
        self.steps
            .iter()
            .take(MAX_TRAJECTORY_STEPS)
            .flatten()
            .filter(|step| step.reward.is_negative())
            .count()
    }
}
