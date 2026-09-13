// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The A/B candidate lifecycle itself.
//!
//! One [`Machine`] models one candidate, from the moment it is declared to the
//! moment it is blessed, rolled back or discarded. A second candidate gets a
//! second machine, which is what keeps a trace about exactly one release.
//!
//! ```text
//!                     check-signature(invalid)      fail-delta
//!                             |                          |
//!  idle --declare--> candidate-declared --valid--> signature-verified
//!                             |                          |
//!                             v                          v acquire-delta
//!                        discarded <-------------  delta-acquired
//!                                                         | swap-slot
//!                                                         v
//!                                                   slot-swapped
//!                                                         | arm-watchdog
//!                                                         v
//!                        any event (now >= due)     watchdog-armed
//!                         rolled-back <-----------------  |  bless (verity match)
//!                                                         v
//!                              reopen(window) ------> blessed
//!                                  |                     ^
//!                                  v                     | bless (verity match)
//!                              reopened --remeasure-->  --+
//! ```
//!
//! # The four invariants this file exists to hold
//!
//! * **The deadline outranks the event.** REQ-P02-08 is a property of the
//!   watchdog, not of one event kind, so an on-trial state evaluates the
//!   deadline *before* it dispatches the event. Past `due_at` every offered
//!   event plans the same rollback, a `Bless` included: a driver that never
//!   offers a `Tick` cannot bless a candidate whose watchdog has already
//!   expired, and the D13 maintenance window is bounded the same way.
//!
//! * **The clock is a parameter.** [`Machine::step`] takes a [`Clock`] and
//!   reads it once per step, through a [`MonotonicGuard`] that refuses a clock
//!   that stepped backwards. The machine never names the host clock.
//! * **A transition allocates nothing.** Every field is `Copy` and the trace is
//!   a fixed-size array, so a step writes into storage that already exists.
//!   `tests/allocation_bounds.rs` binds that claim to `Machine: Copy` and to a
//!   recorded size bound, so a `Vec`, `String` or `Box` anywhere in the machine
//!   breaks the gate rather than the invariant.
//! * **A refused step changes nothing.** Planning is separated from applying:
//!   the plan is computed against an immutable borrow, and only an accepted
//!   plan is applied and recorded. The one exception is deliberate and
//!   documented on [`Machine::step`]: the clock was read, so the monotonic
//!   guard has observed it.
//!
//! `Machine` is `Copy` on purpose. A boundary test forks a machine at the
//! moment the watchdog is armed and drives the two copies one tick apart,
//! which is the only honest way to compare "exactly at the timeout" with "one
//! tick before" on the same history.

use crate::candidate::Candidate;
use crate::clock::{Clock, MonotonicGuard, Tick, Timeout, Watchdog, WatchdogStatus};
use crate::error::LifecycleError;
use crate::ports::SysupdateCall;
use crate::slot::Slot;
use crate::state::{Event, SignatureVerdict, State};
use crate::trace::{MAX_TRANSITIONS, Trace, Transition};
use crate::verity::RootHash;

/// What an accepted plan changes besides the state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Effect {
    /// Nothing but the state moves.
    None,
    /// Record the candidate the lifecycle is about.
    Declare(Candidate),
    /// Hold a dm-verity measurement.
    Measure(RootHash),
    /// Arm a deadline of this width at the current tick.
    Arm(Timeout),
    /// Drop any armed deadline.
    Disarm,
    /// Drop the held measurement and arm a bounded maintenance window (D13).
    Reopen(Timeout),
}

/// An accepted transition, before it is applied.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Plan {
    next: State,
    effect: Effect,
}

impl Plan {
    /// A plan that only moves the state.
    const fn to(next: State) -> Self {
        Self {
            next,
            effect: Effect::None,
        }
    }

    /// A plan that moves the state and applies `effect`.
    const fn with(next: State, effect: Effect) -> Self {
        Self { next, effect }
    }
}

/// Builds the refusal for an event a state does not accept.
const fn unexpected(state: State, event: Event) -> LifecycleError {
    LifecycleError::Unexpected {
        state,
        event: event.kind(),
    }
}

/// Plans a step from [`State::Idle`].
fn plan_idle(event: Event) -> Result<Plan, LifecycleError> {
    match event {
        Event::Declare(candidate) => Ok(Plan::with(
            State::CandidateDeclared,
            Effect::Declare(candidate),
        )),
        other => Err(unexpected(State::Idle, other)),
    }
}

/// Plans a step from [`State::CandidateDeclared`]: the signature gate.
fn plan_declared(event: Event) -> Result<Plan, LifecycleError> {
    match event {
        Event::CheckSignature(SignatureVerdict::Valid) => Ok(Plan::to(State::SignatureVerified)),
        Event::CheckSignature(SignatureVerdict::Invalid) => Ok(Plan::to(State::Discarded)),
        other => Err(unexpected(State::CandidateDeclared, other)),
    }
}

/// Plans a step from [`State::SignatureVerified`]: delta acquisition.
fn plan_verified(event: Event) -> Result<Plan, LifecycleError> {
    match event {
        Event::AcquireDelta(hash) => Ok(Plan::with(State::DeltaAcquired, Effect::Measure(hash))),
        Event::FailDelta => Ok(Plan::to(State::Discarded)),
        other => Err(unexpected(State::SignatureVerified, other)),
    }
}

/// Plans a step from [`State::DeltaAcquired`]: the slot swap.
fn plan_acquired(event: Event) -> Result<Plan, LifecycleError> {
    match event {
        Event::SwapSlot => Ok(Plan::to(State::SlotSwapped)),
        other => Err(unexpected(State::DeltaAcquired, other)),
    }
}

/// Plans a step from [`State::SlotSwapped`]: arming the boot watchdog.
fn plan_swapped(event: Event) -> Result<Plan, LifecycleError> {
    match event {
        Event::ArmWatchdog(timeout) => Ok(Plan::with(State::WatchdogArmed, Effect::Arm(timeout))),
        other => Err(unexpected(State::SlotSwapped, other)),
    }
}

/// Plans a step from [`State::Blessed`]: the D13 reopening.
fn plan_blessed(event: Event) -> Result<Plan, LifecycleError> {
    match event {
        Event::Reopen(timeout) => Ok(Plan::with(State::Reopened, Effect::Reopen(timeout))),
        other => Err(unexpected(State::Blessed, other)),
    }
}

/// Plans a step from any state that is not on trial and not terminal.
fn plan_pipeline(state: State, event: Event) -> Result<Plan, LifecycleError> {
    match state {
        State::Idle => plan_idle(event),
        State::CandidateDeclared => plan_declared(event),
        State::SignatureVerified => plan_verified(event),
        State::DeltaAcquired => plan_acquired(event),
        State::SlotSwapped => plan_swapped(event),
        State::Blessed => plan_blessed(event),
        other => Err(unexpected(other, event)),
    }
}

/// One candidate's A/B lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Machine {
    state: State,
    candidate: Option<Candidate>,
    measured: Option<RootHash>,
    watchdog: Option<Watchdog>,
    guard: MonotonicGuard,
    trace: Trace,
}

impl Default for Machine {
    fn default() -> Self {
        Self::new()
    }
}

impl Machine {
    /// Builds a machine with no candidate, standing at [`State::Idle`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: State::Idle,
            candidate: None,
            measured: None,
            watchdog: None,
            guard: MonotonicGuard::new(),
            trace: Trace::new(),
        }
    }

    /// Returns the state the machine stands in.
    #[must_use]
    pub const fn state(&self) -> State {
        self.state
    }

    /// Returns the candidate, once one is declared.
    #[must_use]
    pub const fn candidate(&self) -> Option<Candidate> {
        self.candidate
    }

    /// Returns the dm-verity measurement currently held, if any.
    ///
    /// Reopening a blessed slot clears it (D13): a slot that was opened for
    /// maintenance is not the slot that was measured, so the model holds no
    /// measurement for it until a fresh one is reported.
    #[must_use]
    pub const fn measured_root_hash(&self) -> Option<RootHash> {
        self.measured
    }

    /// Returns the armed deadline, if one is armed.
    #[must_use]
    pub const fn watchdog(&self) -> Option<Watchdog> {
        self.watchdog
    }

    /// Returns the tick most recently read from the clock.
    #[must_use]
    pub const fn observed_at(&self) -> Option<Tick> {
        self.guard.last()
    }

    /// Returns the recorded transition trace.
    #[must_use]
    pub const fn trace(&self) -> &Trace {
        &self.trace
    }

    /// Returns the stubbed call this state needs next, when it needs one.
    ///
    /// The machine describes the call; nothing in this crate performs it. The
    /// sequence of descriptions is the second artefact M24 can compare a real
    /// transfer against, alongside the trace.
    #[must_use]
    pub fn pending_call(&self) -> Option<SysupdateCall> {
        match self.state {
            State::Idle => Some(SysupdateCall::List),
            State::CandidateDeclared => self
                .candidate
                .map(|candidate| SysupdateCall::Verify(candidate.version())),
            State::SignatureVerified => self
                .candidate
                .map(|candidate| SysupdateCall::Update(candidate.version())),
            State::DeltaAcquired => self
                .candidate
                .map(|candidate| SysupdateCall::SwapSlot(candidate.target())),
            State::Blessed => Some(SysupdateCall::BlessBoot),
            State::RolledBack => Some(SysupdateCall::RollbackBoot),
            State::SlotSwapped | State::WatchdogArmed | State::Reopened | State::Discarded => None,
        }
    }

    /// Offers `event` to the machine, reading the current tick from `clock`.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError`] when the clock is unusable or moved
    /// backwards, when the state does not accept the event, when a bless is
    /// asked for without a matching dm-verity measurement, or when the trace
    /// is full. On every one of those the state, candidate, measurement and
    /// deadline are left exactly as they were and nothing is traced. The
    /// monotonic guard is the single exception: the clock was read, so the
    /// guard has observed that reading.
    pub fn step<C: Clock>(
        &mut self,
        clock: &C,
        event: Event,
    ) -> Result<Transition, LifecycleError> {
        let full = LifecycleError::TraceFull {
            bound: MAX_TRANSITIONS,
        };
        if self.trace.len() >= MAX_TRANSITIONS {
            return Err(full);
        }
        let now = self.guard.observe(clock.now()?)?;
        let from = self.state;
        let plan = self.plan(event, now)?;
        self.apply(plan, now)?;
        let seq = u32::try_from(self.trace.len()).map_err(|_| full)?;
        let slot: Option<Slot> = self.candidate.map(|candidate| candidate.target());
        let record = Transition::new(seq, now, event.kind(), from, self.state, slot);
        self.trace.push(record).map_err(|_| full)?;
        Ok(record)
    }

    /// Computes the transition `event` would make, without making it.
    fn plan(&self, event: Event, now: Tick) -> Result<Plan, LifecycleError> {
        match self.state {
            State::WatchdogArmed | State::Reopened => self.plan_on_trial(event, now),
            State::RolledBack | State::Discarded => Err(LifecycleError::Terminal {
                state: self.state,
                event: event.kind(),
            }),
            other => plan_pipeline(other, event),
        }
    }

    /// Plans a step from a state that is under a deadline.
    ///
    /// The deadline is evaluated before the event is dispatched. REQ-P02-08
    /// makes the rollback a property of the watchdog rather than of the
    /// [`Event::Tick`] that happens to sample it, so past `due_at` every
    /// offered event plans the same rollback -- a [`Event::Bless`] included.
    /// Dispatching first would let a driver that simply never offers a tick
    /// bless a candidate whose watchdog expired long ago, and would let a
    /// [`Event::Remeasure`] extend a D13 maintenance window past its bound.
    ///
    /// The forced rollback is recorded under the event that was offered, not
    /// under a tick the driver never sent: the trace records what happened,
    /// and "a bless was offered at this tick and the candidate rolled back" is
    /// what happened.
    ///
    /// Inside the window a tick leaves the state where it is; the step is
    /// still recorded, because the trace is the whole accepted history and a
    /// tick that changed nothing is part of it.
    fn plan_on_trial(&self, event: Event, now: Tick) -> Result<Plan, LifecycleError> {
        if self.deadline_expired(now) {
            return Ok(Plan::with(State::RolledBack, Effect::Disarm));
        }
        match event {
            Event::Tick => Ok(Plan::to(self.state)),
            Event::Bless => self.plan_bless(),
            Event::Remeasure(hash) if self.state == State::Reopened => {
                Ok(Plan::with(State::Reopened, Effect::Measure(hash)))
            }
            other => Err(unexpected(self.state, other)),
        }
    }

    /// Returns `true` when the deadline this state stands under has passed.
    ///
    /// An unarmed on-trial state is unreachable by construction -- both
    /// on-trial states are entered by an effect that arms a watchdog -- and is
    /// treated as not expired rather than as a rollback, so an unreachable
    /// case cannot invent a terminal state.
    fn deadline_expired(&self, now: Tick) -> bool {
        matches!(
            self.watchdog.map(|watchdog| watchdog.status(now)),
            Some(WatchdogStatus::Expired)
        )
    }

    /// Plans a bless, which REQ-P02-01 gates on a dm-verity match.
    fn plan_bless(&self) -> Result<Plan, LifecycleError> {
        let candidate = self.candidate.ok_or(LifecycleError::NoCandidate)?;
        let measured = self.measured.ok_or(LifecycleError::VerityUnmeasured)?;
        let expected = candidate.expected_root_hash();
        if measured != expected {
            return Err(LifecycleError::VerityMismatch { expected, measured });
        }
        Ok(Plan::with(State::Blessed, Effect::Disarm))
    }

    /// Applies an accepted plan.
    fn apply(&mut self, plan: Plan, now: Tick) -> Result<(), LifecycleError> {
        match plan.effect {
            Effect::None => {}
            Effect::Declare(candidate) => {
                self.candidate = Some(candidate);
                self.trace.declare(candidate);
            }
            Effect::Measure(hash) => self.measured = Some(hash),
            Effect::Arm(timeout) => self.watchdog = Some(Watchdog::arm(now, timeout)?),
            Effect::Disarm => self.watchdog = None,
            Effect::Reopen(timeout) => {
                let watchdog = Watchdog::arm(now, timeout)?;
                self.measured = None;
                self.watchdog = Some(watchdog);
            }
        }
        self.state = plan.next;
        Ok(())
    }
}
