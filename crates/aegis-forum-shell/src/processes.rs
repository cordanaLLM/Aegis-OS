// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The managed process table, driven by stubbed compositor, Justitia and
//! Tellus inputs (E16-1).
//!
//! Every managed process carries the control-group slice it runs in
//! (REQ-P05-01), and the slice is what the shell hands the compositor when
//! the process gains focus, the field REQ-P04-07's focus-switch report
//! carries. Active capacity counts only processes that hold it (REQ-P05-07),
//! so an eligible, quarantined or deleted process costs nothing.
//!
//! No input here arrives over a transport. [`Input`] is what the three
//! producers would say, and the tests are its only source: P04's surface and
//! exit events, P13's budget windows and P06's human decisions.

use thiserror::Error;

use aegis_tellus::{CarbonTelemetry, SliceName};

use crate::lifecycle::{Lifecycle, LifecycleError, ProcessState, WindowVerdict};

/// The most processes the shell manages.
pub const MAX_PROCESSES: usize = 64;

/// The facility power budget, in watts (REQ-P05-01, export-014).
pub const FACILITY_BUDGET_WATTS: f64 = 20.0;

/// A managed process's identifier within the shell.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ProcessId(pub u32);

/// One input from a stubbed producer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Input {
    /// P04: the compositor mapped the process's surface.
    SurfaceMapped(ProcessId),
    /// P04: the process exited.
    Exited(ProcessId),
    /// P13: one budget window's verdict for the process.
    Window(ProcessId, WindowVerdict),
    /// P06: a human decision releasing a quarantined process.
    Released(ProcessId),
    /// P06: a human decision deleting the process.
    DeleteApproved(ProcessId),
}

impl Input {
    /// The process the input is about.
    #[must_use]
    pub const fn subject(self) -> ProcessId {
        match self {
            Self::SurfaceMapped(id)
            | Self::Exited(id)
            | Self::Window(id, _)
            | Self::Released(id)
            | Self::DeleteApproved(id) => id,
        }
    }

    /// Applies the input to one lifecycle.
    fn apply_to(self, lifecycle: &mut Lifecycle) -> Result<ProcessState, LifecycleError> {
        match self {
            Self::SurfaceMapped(_) => lifecycle.transition(ProcessState::Activated),
            Self::Exited(_) | Self::DeleteApproved(_) => {
                lifecycle.transition(ProcessState::Deleted)
            }
            Self::Released(_) => lifecycle.transition(ProcessState::EligibleButInactive),
            Self::Window(_, verdict) => lifecycle.observe(verdict),
        }
    }
}

/// Why a process-table operation was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum ProcessError {
    /// The table already manages [`MAX_PROCESSES`] processes.
    #[error("the shell already manages {MAX_PROCESSES} processes")]
    Full,
    /// A process with this identifier is already managed.
    #[error("process {0} is already managed")]
    Duplicate(u32),
    /// No process with this identifier is managed.
    #[error("no managed process {0}")]
    Unknown(u32),
    /// The lifecycle refused the input.
    #[error("process {id}: {error}")]
    Lifecycle {
        /// The process concerned.
        id: u32,
        /// The lifecycle's refusal.
        error: LifecycleError,
    },
}

/// One managed process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ManagedProcess {
    /// The identifier.
    pub id: ProcessId,
    /// The control-group slice the process runs in.
    pub slice: SliceName,
    /// Its lifecycle.
    pub lifecycle: Lifecycle,
}

/// Every managed process.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessTable {
    processes: Vec<ManagedProcess>,
}

impl ProcessTable {
    /// An empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            processes: Vec::new(),
        }
    }

    /// Starts managing `id`, eligible and inactive, in `slice`.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError::Full`] past [`MAX_PROCESSES`] and
    /// [`ProcessError::Duplicate`] for an identifier already managed.
    pub fn admit(&mut self, id: ProcessId, slice: SliceName) -> Result<(), ProcessError> {
        if self.processes.len() >= MAX_PROCESSES {
            return Err(ProcessError::Full);
        }
        if self.get(id).is_some() {
            return Err(ProcessError::Duplicate(id.0));
        }
        self.processes.push(ManagedProcess {
            id,
            slice,
            lifecycle: Lifecycle::new(),
        });
        Ok(())
    }

    /// The process with identifier `id`.
    #[must_use]
    pub fn get(&self, id: ProcessId) -> Option<&ManagedProcess> {
        self.processes.iter().find(|process| process.id == id)
    }

    /// The state of process `id`.
    #[must_use]
    pub fn state(&self, id: ProcessId) -> Option<ProcessState> {
        self.get(id).map(|process| process.lifecycle.state())
    }

    /// Applies one stubbed producer's input.
    ///
    /// # Errors
    ///
    /// Returns [`ProcessError::Unknown`] for an unmanaged process and
    /// [`ProcessError::Lifecycle`] for a transition D96 refuses.
    pub fn apply(&mut self, input: Input) -> Result<ProcessState, ProcessError> {
        let id = input.subject();
        let process = self
            .processes
            .iter_mut()
            .find(|process| process.id == id)
            .ok_or(ProcessError::Unknown(id.0))?;
        input
            .apply_to(&mut process.lifecycle)
            .map_err(|error| ProcessError::Lifecycle { id: id.0, error })
    }

    /// How many processes hold active capacity (REQ-P05-07).
    #[must_use]
    pub fn active_capacity(&self) -> usize {
        self.processes
            .iter()
            .filter(|process| process.lifecycle.state().holds_capacity())
            .count()
    }

    /// The slice the compositor re-allocates when process `id` gains focus
    /// (REQ-P04-07); `None` for an unmanaged or deleted process.
    #[must_use]
    pub fn focus_slice(&self, id: ProcessId) -> Option<SliceName> {
        self.get(id)
            .filter(|process| process.lifecycle.state().is_live())
            .map(|process| process.slice)
    }
}

/// The facility verdict a telemetry update carries: within budget at or
/// below [`FACILITY_BUDGET_WATTS`], over it above.
///
/// This is the shell's reading of P13's facility-wide draw. P13's payload
/// names no process, so a per-process window is a stubbed [`Input::Window`]
/// until a producer reports one.
#[must_use]
pub fn facility_verdict(telemetry: &CarbonTelemetry) -> WindowVerdict {
    if telemetry.power_watts.get() > FACILITY_BUDGET_WATTS {
        WindowVerdict::OverBudget
    } else {
        WindowVerdict::WithinBudget
    }
}
