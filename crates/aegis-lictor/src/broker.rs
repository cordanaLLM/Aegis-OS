// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded resource-broker table and its focus arbitration (REQ-P07-03).
//!
//! This is the scaffold's `LictorResourceBroker` with the three things it
//! lacked: a scalar bound on the table, a typed capacity refusal, and an
//! arbitration outcome a caller can inspect instead of a line of printed text.
//!
//! # Where the bound comes from
//!
//! The scaffold declares none -- `registered_processes` is an unbounded `Vec`
//! -- which HISS-02 does not admit. [`MAX_TRACKED_PROCESSES`] is therefore
//! this crate's own bound and not a recorded one, and it is documented as
//! such. The recorded bound that does exist belongs to a different table: the
//! eBPF program's task map holds 65536 entries (export-056 `39af243568ad`),
//! which is the kernel-side map milestone M19 would load, not this user-space
//! register.
//!
//! # What arbitration means here
//!
//! [`ResourceBroker::on_focus_change`] reproduces the scaffold's sweep: the
//! focused process is elevated and every background agent is throttled.
//! [`FocusOutcome`] is what it returns, and the case the scaffold cannot
//! express is the one the negative test pins -- a focus switch to a process
//! the broker never registered elevates nothing and throttles nothing, and
//! says so.
//!
//! # What this module does not do
//!
//! **Nothing is elevated, throttled or swapped.** No `cgroup` is written, no
//! `drm_sched` priority is set, no accelerator is opened and no memory is
//! moved between device and host. A process is a row in a fixed array, and
//! "throttled" is a value of [`FocusOutcome`]. The VRAM figure a row carries
//! is the caller's declaration, never a reading.

use crate::error::LictorError;
use crate::id::Label;
use crate::tier::{EwmaBurst, Tier};

/// Scalar upper bound on the tracked-process table.
///
/// This crate's own bound: the scaffold declares none. See the module
/// documentation. One hundred and twenty-eight rows of a fixed array, which is
/// a table the whole broker can carry by value.
pub const MAX_TRACKED_PROCESSES: usize = 128;

/// The largest process identifier this build admits.
///
/// The kernel's own default `pid_max` on a 64-bit system is 4194304, which is
/// the ceiling a caller could legitimately observe.
pub const MAX_PID: u32 = 4_194_304;

/// The task-map capacity the eBPF source declares.
///
/// Recorded from export-056 `39af243568ad` so the two bounds are not confused
/// with one another. Nothing here allocates a map of this size, or of any
/// size.
pub const EBPF_TASK_MAP_ENTRIES: u32 = 65_536;

/// A validated process identifier.
///
/// The `Serialize`/`Deserialize` pair is hand-written in
/// [`crate::contracts::encoding`] rather than derived, because a derived one
/// would let a payload carry process zero straight past [`Pid::new`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Pid(u32);

impl Pid {
    /// Validates `pid` against `1..=MAX_PID`.
    ///
    /// # Errors
    ///
    /// Returns [`LictorError::PidOutOfRange`] outside that range, so process
    /// zero -- which is the kernel's own swapper and never a focus target --
    /// cannot become a value.
    pub const fn new(pid: u32) -> Result<Self, LictorError> {
        if pid == 0 || pid > MAX_PID {
            return Err(LictorError::PidOutOfRange { pid, max: MAX_PID });
        }
        Ok(Self(pid))
    }

    /// Returns the validated identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// The broker's own three-way process classification.
///
/// Recorded from export-032 `30d67bc52290`, where it drives the `drm_sched`
/// priority. It is **not** the scheduler's four-tier burst classification in
/// [`crate::tier`]: the two sources declare different tier counts for
/// different schedulers, and neither is derived from the other. Mapping
/// between them is [`ProcessTier::burst_tier`], which is this crate's reading
/// and is documented as such.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum ProcessTier {
    /// `DRM_SCHED_PRIORITY_HIGH`: the Wayland compositor.
    #[serde(rename = "t0-wayland-compositor")]
    T0WaylandCompositor,
    /// `DRM_SCHED_PRIORITY_NORMAL`: an interactive application.
    #[serde(rename = "t1-interactive-app")]
    T1InteractiveApp,
    /// `DRM_SCHED_PRIORITY_LOW`: a background agent.
    #[serde(rename = "t2-background-agent")]
    T2BackgroundAgent,
}

impl ProcessTier {
    /// All three classes, highest priority first.
    pub const ALL: [Self; 3] = [
        Self::T0WaylandCompositor,
        Self::T1InteractiveApp,
        Self::T2BackgroundAgent,
    ];

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::T0WaylandCompositor => "t0-wayland-compositor",
            Self::T1InteractiveApp => "t1-interactive-app",
            Self::T2BackgroundAgent => "t2-background-agent",
        }
    }

    /// Returns `true` when a process in this class may be throttled on a
    /// focus switch away from it.
    ///
    /// Only the background agent may, which is the scaffold's own rule.
    #[must_use]
    pub const fn is_throttleable(self) -> bool {
        matches!(self, Self::T2BackgroundAgent)
    }

    /// Returns the burst tier this crate reads this class as.
    ///
    /// A reading, not a recorded mapping: no source relates the two
    /// classifications, so this is stated here and nowhere else so that a
    /// reviewer can find it.
    #[must_use]
    pub const fn burst_tier(self) -> Tier {
        match self {
            Self::T0WaylandCompositor => Tier::Critical,
            Self::T1InteractiveApp => Tier::Interactive,
            Self::T2BackgroundAgent => Tier::Bulk,
        }
    }
}

/// One tracked-process row.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProcessRecord {
    /// The process identifier.
    pub pid: Pid,
    /// The process name.
    pub name: Label,
    /// The broker's classification of the process.
    pub tier: ProcessTier,
    /// The accelerator memory the caller declares the process holds, in
    /// mebibytes. A declaration, never a reading: this crate opens no device.
    pub declared_vram_mib: u32,
    /// Whether the process currently holds focus.
    pub focused: bool,
    /// The running burst average the scheduler side would carry.
    pub burst: EwmaBurst,
}

/// What one focus switch did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum FocusOutcome {
    /// The broker tracks no such process; nothing was elevated or throttled.
    ///
    /// The case the scaffold cannot express: it sets `active_pid` to whatever
    /// it is handed and then sweeps a table that contains no matching row, so
    /// an unknown identifier silently becomes the active one.
    UnknownPid {
        /// The identifier that was offered.
        pid: Pid,
    },
    /// The process already held focus; nothing changed.
    AlreadyFocused {
        /// The identifier that already held focus.
        pid: Pid,
    },
    /// Focus moved.
    Switched {
        /// The identifier that gained focus.
        pid: Pid,
        /// How many background agents were throttled.
        throttled: usize,
    },
}

impl FocusOutcome {
    /// Returns how many background agents this outcome throttled.
    #[must_use]
    pub const fn throttled(self) -> usize {
        match self {
            Self::Switched { throttled, .. } => throttled,
            Self::UnknownPid { .. } | Self::AlreadyFocused { .. } => 0,
        }
    }

    /// Returns `true` when the outcome changed which process holds focus.
    #[must_use]
    pub const fn moved_focus(self) -> bool {
        matches!(self, Self::Switched { .. })
    }
}

/// The bounded tracked-process table.
#[derive(Debug, Clone, Copy)]
pub struct ResourceBroker {
    rows: [Option<ProcessRecord>; MAX_TRACKED_PROCESSES],
    count: usize,
    active: Option<Pid>,
}

impl Default for ResourceBroker {
    fn default() -> Self {
        Self::new()
    }
}

impl ResourceBroker {
    /// Builds an empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            rows: [None; MAX_TRACKED_PROCESSES],
            count: 0,
            active: None,
        }
    }

    /// Registers one process.
    ///
    /// # Errors
    ///
    /// Returns [`LictorError::ProcessTableFull`] at
    /// [`MAX_TRACKED_PROCESSES`].
    pub fn register(
        &mut self,
        pid: Pid,
        name: Label,
        tier: ProcessTier,
        declared_vram_mib: u32,
    ) -> Result<(), LictorError> {
        if self.count >= MAX_TRACKED_PROCESSES {
            return Err(LictorError::ProcessTableFull {
                max: MAX_TRACKED_PROCESSES,
            });
        }
        let row = ProcessRecord {
            pid,
            name,
            tier,
            declared_vram_mib,
            focused: false,
            burst: EwmaBurst::new(),
        };
        let cell = self
            .rows
            .get_mut(self.count)
            .ok_or(LictorError::ProcessTableFull {
                max: MAX_TRACKED_PROCESSES,
            })?;
        *cell = Some(row);
        self.count = self.count.saturating_add(1);
        Ok(())
    }

    /// Moves focus to `pid` and arbitrates the table.
    ///
    /// Returns [`FocusOutcome::UnknownPid`] without touching a row when the
    /// broker tracks no such process, which is the negative case
    /// REQ-P07-03 leaves the scaffold unable to report.
    pub fn on_focus_change(&mut self, pid: Pid) -> FocusOutcome {
        if !self.holds(pid) {
            return FocusOutcome::UnknownPid { pid };
        }
        if self.active == Some(pid) {
            return FocusOutcome::AlreadyFocused { pid };
        }
        let mut throttled = 0usize;
        for cell in self.rows.iter_mut().take(MAX_TRACKED_PROCESSES) {
            let Some(row) = cell.as_mut() else { continue };
            row.focused = row.pid == pid;
            if !row.focused && row.tier.is_throttleable() {
                throttled = throttled.saturating_add(1);
            }
        }
        self.active = Some(pid);
        FocusOutcome::Switched { pid, throttled }
    }

    /// Folds one burst duration into `pid`'s running average.
    ///
    /// # Errors
    ///
    /// Returns [`LictorError::UnknownPid`] when the broker tracks no such
    /// process.
    pub fn observe_burst(&mut self, pid: Pid, burst_ns: u64) -> Result<Tier, LictorError> {
        for cell in self.rows.iter_mut().take(MAX_TRACKED_PROCESSES) {
            let Some(row) = cell.as_mut() else { continue };
            if row.pid != pid {
                continue;
            }
            row.burst.observe(burst_ns);
            return Ok(row.burst.tier());
        }
        Err(LictorError::UnknownPid { pid: pid.get() })
    }

    /// Returns the row for `pid`, when the table holds one.
    #[must_use]
    pub fn get(&self, pid: Pid) -> Option<ProcessRecord> {
        self.rows
            .iter()
            .take(MAX_TRACKED_PROCESSES)
            .flatten()
            .find(|row| row.pid == pid)
            .copied()
    }

    /// Returns `true` when the table holds a row for `pid`.
    #[must_use]
    pub fn holds(&self, pid: Pid) -> bool {
        self.get(pid).is_some()
    }

    /// Returns the process that currently holds focus, if any.
    #[must_use]
    pub const fn active(&self) -> Option<Pid> {
        self.active
    }

    /// Returns how many processes the table holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns `true` when the table holds no process.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns how many rows are background agents not holding focus.
    #[must_use]
    pub fn throttleable(&self) -> usize {
        self.rows
            .iter()
            .take(MAX_TRACKED_PROCESSES)
            .flatten()
            .filter(|row| !row.focused && row.tier.is_throttleable())
            .count()
    }
}
