// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E06-1: the trajectory buffer and the relabeller.
//!
//! Positive: steps are recorded and read back. Negative: relabelling leaves a
//! neutral or positive step alone. Boundary: the 128th step is accepted and
//! the 129th refused, and a reward of exactly zero is not negative.
//!
//! Nothing here learns anything.

mod common;

use aegis_minerva::{
    AgentHerEngine, MAX_TRAJECTORY_STEPS, MinervaError, Reward, STATE_HASH_LEN, StateHash,
};

use common::{Fallible, achieved, step};

// --- Positive -------------------------------------------------------------

/// Positive: a recorded step reads back with what it was given.
#[test]
fn a_recorded_step_reads_back() -> Fallible {
    let mut engine = AgentHerEngine::new();
    assert!(engine.is_empty());
    engine.record(step(0, -500)?)?;
    assert_eq!(engine.count(), 1);
    let recorded = engine.get(0).ok_or("the buffer must hold the step")?;
    assert_eq!(recorded.index, 0);
    assert_eq!(recorded.reward, Reward::from_thousandths(-500));
    assert!(!recorded.terminal);
    assert!(!recorded.is_relabelled());
    assert_eq!(recorded.state.as_bytes().len(), STATE_HASH_LEN);
    assert_eq!(engine.failed(), 1);
    Ok(())
}

/// Positive: relabelling flips every failed step to the achieved goal.
#[test]
fn relabelling_flips_every_failed_step() -> Fallible {
    let mut engine = AgentHerEngine::default();
    engine.record(step(0, -1_000)?)?;
    engine.record(step(1, -1)?)?;
    let relabelled = engine.relabel(achieved());
    assert_eq!(relabelled, 2);
    assert_eq!(engine.failed(), 0);
    for position in 0..2usize {
        let row = engine
            .get(position)
            .ok_or("the buffer must hold the step")?;
        assert_eq!(row.reward, Reward::SUCCESS);
        assert_eq!(row.reward.thousandths(), 1_000);
        assert!(row.is_relabelled());
        assert_eq!(row.relabelled_goal, Some(achieved()));
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: relabelling flips only negative rewards.
///
/// This is the acceptance sentence of epic E06-1, and the case that makes it
/// worth stating: a neutral step keeps its reward *and* its goal, so replay
/// cannot quietly rewrite a trajectory that did not fail.
#[test]
fn relabelling_flips_only_negative_rewards() -> Fallible {
    let mut engine = AgentHerEngine::new();
    engine.record(step(0, -250)?)?;
    engine.record(step(1, 0)?)?;
    engine.record(step(2, 750)?)?;
    let relabelled = engine.relabel(achieved());
    assert_eq!(relabelled, 1, "only the failed step is relabelled");

    let failed = engine.get(0).ok_or("the failed step must be readable")?;
    assert_eq!(failed.reward, Reward::SUCCESS);
    assert_eq!(failed.relabelled_goal, Some(achieved()));

    let neutral = engine.get(1).ok_or("the neutral step must be readable")?;
    assert_eq!(neutral.reward, Reward::NEUTRAL);
    assert_eq!(neutral.relabelled_goal, None);
    assert!(!neutral.is_relabelled());

    let positive = engine.get(2).ok_or("the positive step must be readable")?;
    assert_eq!(positive.reward, Reward::from_thousandths(750));
    assert_eq!(positive.relabelled_goal, None);
    Ok(())
}

/// Negative: a second relabelling of an already relabelled buffer flips
/// nothing, because nothing is negative any more.
#[test]
fn a_second_relabelling_flips_nothing() -> Fallible {
    let mut engine = AgentHerEngine::new();
    engine.record(step(0, -10)?)?;
    assert_eq!(engine.relabel(achieved()), 1);
    assert_eq!(
        engine.relabel(StateHash::from_bytes([1u8; STATE_HASH_LEN])),
        0
    );
    let row = engine.get(0).ok_or("the step must be readable")?;
    assert_eq!(
        row.relabelled_goal,
        Some(achieved()),
        "the first goal is not overwritten by a later sweep"
    );
    assert!(engine.get(1).is_none());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the 128th step is accepted and the 129th is refused.
#[test]
fn the_one_hundred_and_twenty_ninth_step_is_refused() -> Fallible {
    let mut engine = AgentHerEngine::new();
    for index in 0..MAX_TRAJECTORY_STEPS.saturating_sub(1) {
        engine.record(step(u16::try_from(index).unwrap_or(u16::MAX), 1)?)?;
    }
    assert_eq!(engine.count(), 127);
    engine.record(step(127, 1)?)?;
    assert_eq!(engine.count(), MAX_TRAJECTORY_STEPS);
    let refused = engine.record(step(128, 1)?);
    assert_eq!(
        refused,
        Err(MinervaError::TrajectoryBufferFull {
            max: MAX_TRAJECTORY_STEPS
        })
    );
    assert_eq!(
        engine.count(),
        MAX_TRAJECTORY_STEPS,
        "the refusal stores nothing"
    );
    Ok(())
}

/// Boundary: a reward of exactly zero is not negative, so the relabelling
/// boundary is exact rather than a rounding.
#[test]
fn a_reward_of_exactly_zero_is_not_negative() {
    assert!(!Reward::NEUTRAL.is_negative());
    assert!(Reward::from_thousandths(-1).is_negative());
    assert!(!Reward::from_thousandths(1).is_negative());
    assert!(!Reward::SUCCESS.is_negative());
    assert_eq!(Reward::NEUTRAL.thousandths(), 0);
    assert!(Reward::from_thousandths(-1) < Reward::NEUTRAL);
}
