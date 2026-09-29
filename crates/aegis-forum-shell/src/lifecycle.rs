// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The typed process lifecycle (REQ-P05-03, decision D96).
//!
//! REQ-P05-03 names five states and export-014 section 6.3 refers to a figure
//! the export does not contain, so no source records an edge or a limit. D96
//! (2026-09-29) supplies them:
//!
//! | From | To | When |
//! | :-- | :-- | :-- |
//! | Eligible but Inactive | Activated | the compositor maps the process's surface |
//! | Activated | Rate-Limited | a window over budget |
//! | Rate-Limited | Quarantined | the [`QUARANTINE_LIMIT`]th consecutive window over budget |
//! | Quarantined | Deleted | a decision to delete |
//! | Rate-Limited | Activated | a window back within budget |
//! | Quarantined | Eligible but Inactive | a decision to release |
//! | any live state | Deleted | the process exits or is deleted |
//!
//! Deleted is terminal, and no other edge is admitted -- an idle return from
//! Activated to Eligible but Inactive included. A window within budget
//! between two over-budget ones restarts the count, so only *consecutive*
//! over-budget windows quarantine.
//!
//! The lifecycle holds no clock (REQ-P05-04: coordination without a
//! privileged clock): a window is an input, delivered by whoever measured it,
//! and the tests deliver it from a stub.

use thiserror::Error;

/// Consecutive over-budget windows after which a process is quarantined (D96).
pub const QUARANTINE_LIMIT: u8 = 3;

/// A managed process's lifecycle state (REQ-P05-03).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ProcessState {
    /// Known to the shell, holding no active capacity.
    EligibleButInactive,
    /// Running within its budget.
    Activated,
    /// Running, throttled for exceeding its budget.
    RateLimited,
    /// Stopped and isolated after exceeding its budget repeatedly.
    Quarantined,
    /// Gone; terminal.
    Deleted,
}

impl ProcessState {
    /// Every state, in the source's order.
    pub const ALL: [Self; 5] = [
        Self::EligibleButInactive,
        Self::Activated,
        Self::RateLimited,
        Self::Quarantined,
        Self::Deleted,
    ];

    /// Whether D96's table lists the edge `self` to `next`.
    #[must_use]
    pub const fn admits(self, next: Self) -> bool {
        match (self, next) {
            (Self::Deleted, _) => false,
            (_, Self::Deleted)
            | (Self::EligibleButInactive, Self::Activated)
            | (Self::Activated, Self::RateLimited)
            | (Self::RateLimited, Self::Quarantined | Self::Activated)
            | (Self::Quarantined, Self::EligibleButInactive) => true,
            _ => false,
        }
    }

    /// Whether the process still exists: every state but Deleted.
    #[must_use]
    pub const fn is_live(self) -> bool {
        !matches!(self, Self::Deleted)
    }

    /// Whether the process holds active capacity (REQ-P05-07): Activated and
    /// Rate-Limited do; an eligible, quarantined or deleted one does not.
    #[must_use]
    pub const fn holds_capacity(self) -> bool {
        matches!(self, Self::Activated | Self::RateLimited)
    }
}

/// A budget window's verdict, as the Tellus side reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowVerdict {
    /// The process stayed within its budget.
    WithinBudget,
    /// The process exceeded its budget.
    OverBudget,
}

/// Why a transition was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum LifecycleError {
    /// The process is deleted; nothing leaves Deleted.
    #[error("the process is deleted, and Deleted is terminal")]
    Terminal,
    /// D96's table lists no such edge.
    #[error("D96 admits no transition from {from:?} to {to:?}")]
    NotAdmitted {
        /// The current state.
        from: ProcessState,
        /// The state asked for.
        to: ProcessState,
    },
    /// A budget window was reported for a process that holds no capacity.
    #[error("a budget window was reported while the process is {state:?}")]
    NotRunning {
        /// The state the process was in.
        state: ProcessState,
    },
}

/// One process's lifecycle: its state and its run of over-budget windows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Lifecycle {
    state: ProcessState,
    over_budget: u8,
}

impl Default for Lifecycle {
    fn default() -> Self {
        Self::new()
    }
}

impl Lifecycle {
    /// A process that is eligible and inactive.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: ProcessState::EligibleButInactive,
            over_budget: 0,
        }
    }

    /// The current state.
    #[must_use]
    pub const fn state(&self) -> ProcessState {
        self.state
    }

    /// The current run of consecutive over-budget windows.
    #[must_use]
    pub const fn over_budget_windows(&self) -> u8 {
        self.over_budget
    }

    /// Moves to `next` along an edge D96 admits.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::Terminal`] from Deleted and
    /// [`LifecycleError::NotAdmitted`] for an edge the table does not list.
    pub fn transition(&mut self, next: ProcessState) -> Result<ProcessState, LifecycleError> {
        if !self.state.is_live() {
            return Err(LifecycleError::Terminal);
        }
        if !self.state.admits(next) {
            return Err(LifecycleError::NotAdmitted {
                from: self.state,
                to: next,
            });
        }
        if next != ProcessState::RateLimited {
            self.over_budget = 0;
        }
        self.state = next;
        Ok(next)
    }

    /// Applies one budget window's verdict to a running process.
    ///
    /// Over budget, an activated process becomes rate-limited, and the
    /// [`QUARANTINE_LIMIT`]th consecutive over-budget window quarantines it.
    /// Within budget, a rate-limited process returns to Activated and its run
    /// restarts; an activated one stays as it is.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::Terminal`] from Deleted and
    /// [`LifecycleError::NotRunning`] for a process holding no capacity.
    pub fn observe(&mut self, verdict: WindowVerdict) -> Result<ProcessState, LifecycleError> {
        if !self.state.is_live() {
            return Err(LifecycleError::Terminal);
        }
        if !self.state.holds_capacity() {
            return Err(LifecycleError::NotRunning { state: self.state });
        }
        match (verdict, self.state) {
            (WindowVerdict::WithinBudget, ProcessState::RateLimited) => {
                self.transition(ProcessState::Activated)
            }
            (WindowVerdict::WithinBudget, _) => Ok(self.state),
            (WindowVerdict::OverBudget, _) => self.over_budget_window(),
        }
    }

    /// Counts one more over-budget window and moves on at the thresholds.
    fn over_budget_window(&mut self) -> Result<ProcessState, LifecycleError> {
        let run = self.over_budget.saturating_add(1);
        if run >= QUARANTINE_LIMIT {
            return self.transition(ProcessState::Quarantined);
        }
        if self.state == ProcessState::Activated {
            self.transition(ProcessState::RateLimited)?;
        }
        self.over_budget = run;
        Ok(self.state)
    }
}
