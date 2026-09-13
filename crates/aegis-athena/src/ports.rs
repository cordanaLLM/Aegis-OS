// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The stubbed effects P16 would have, and the recording that proves they are
//! stubbed.
//!
//! REQ-P16-06 puts P16 Athena and P02 Janus on an asynchronous local D-Bus
//! edge carried by `systemd-sysupdate`, for dual-slot A/B deployment
//! (REQ-P01-04). Nothing here connects to D-Bus, runs `systemd-sysupdate`,
//! writes a partition or reboots. [`SysupdatePort`] is the seam where a real
//! implementation would attach, and [`StubSysupdate`] is the only
//! implementation in the workspace: it records what it was asked to do and
//! reports success, so a test can assert that a promotion produced exactly one
//! stubbed call and no effect.
//!
//! The port takes a deadline for the same reason the wattage seam does: the
//! call it stands in for is I/O, and HISS-02 wants the deadline in the
//! signature before the implementation that blocks exists.

use core::num::NonZeroU32;

use aegis_tellus::CandidateId;

use crate::stage::InvalidationReason;

/// Scalar upper bound on the calls one stub records.
pub const MAX_RECORDED_CALLS: usize = 32;

/// The default service time the stub declares, in milliseconds.
pub const STUB_SERVICE_MILLIS: u32 = 1;

/// Reasons the port refuses a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PortError {
    /// The stub reached its recording bound.
    #[error("the stub recorded its bound of {max} calls")]
    RecordingFull {
        /// The scalar bound.
        max: usize,
    },
    /// The offered deadline is shorter than the declared service time.
    #[error("the port would block past the {offered}ms deadline; it needs {needed}ms")]
    WouldBlock {
        /// The service time the port declares, in milliseconds.
        needed: u32,
        /// The deadline the caller offered, in milliseconds.
        offered: u32,
    },
}

/// The deadline within which a port call must complete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CallDeadline(NonZeroU32);

impl CallDeadline {
    /// Builds a deadline of `millis` milliseconds.
    #[must_use]
    pub const fn from_millis(millis: NonZeroU32) -> Self {
        Self(millis)
    }

    /// Builds a deadline of `millis` milliseconds, or `None` when it is zero.
    #[must_use]
    pub const fn try_from_millis(millis: u32) -> Option<Self> {
        match NonZeroU32::new(millis) {
            Some(value) => Some(Self(value)),
            None => None,
        }
    }

    /// Returns the deadline in milliseconds.
    #[must_use]
    pub const fn millis(self) -> u32 {
        self.0.get()
    }
}

/// What a recorded call asked for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CallKind {
    /// Deploy a published candidate into the alternate slot.
    Deploy,
    /// Roll the alternate slot back.
    Rollback,
}

impl CallKind {
    /// Every call kind.
    pub const ALL: [Self; 2] = [Self::Deploy, Self::Rollback];

    /// Returns the call name used in diagnostics.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Deploy => "deploy",
            Self::Rollback => "rollback",
        }
    }
}

/// One call the stub recorded rather than made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SysupdateCall {
    /// What was asked for.
    pub kind: CallKind,
    /// The candidate the call is about.
    pub candidate: CandidateId,
    /// Why a rollback was asked for, when one was.
    pub reason: Option<InvalidationReason>,
}

/// The seam a real `systemd-sysupdate` client would attach to.
pub trait SysupdatePort {
    /// Asks the update mechanism to act on `candidate`.
    ///
    /// # Errors
    ///
    /// Returns [`PortError`] when the call cannot be accepted within
    /// `deadline`.
    fn call(&mut self, call: SysupdateCall, deadline: CallDeadline) -> Result<(), PortError>;

    /// Returns the service time the port demands of a caller's deadline.
    fn service_time(&self) -> CallDeadline;
}

/// A port that records calls and performs none.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StubSysupdate {
    calls: [Option<SysupdateCall>; MAX_RECORDED_CALLS],
    len: usize,
    service: CallDeadline,
}

impl Default for StubSysupdate {
    fn default() -> Self {
        Self::new()
    }
}

impl StubSysupdate {
    /// Builds a stub that has recorded nothing.
    #[must_use]
    pub fn new() -> Self {
        Self {
            calls: [None; MAX_RECORDED_CALLS],
            len: 0,
            service: default_service_time(),
        }
    }

    /// Returns how many calls the stub recorded.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the stub recorded nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the call at `index`, when there is one.
    #[must_use]
    pub fn get(&self, index: usize) -> Option<SysupdateCall> {
        self.calls.get(index).copied().flatten()
    }
}

/// Returns the default service time, one millisecond.
fn default_service_time() -> CallDeadline {
    CallDeadline::try_from_millis(STUB_SERVICE_MILLIS)
        .unwrap_or(CallDeadline::from_millis(NonZeroU32::MIN))
}

impl SysupdatePort for StubSysupdate {
    fn call(&mut self, call: SysupdateCall, deadline: CallDeadline) -> Result<(), PortError> {
        if deadline.millis() < self.service.millis() {
            return Err(PortError::WouldBlock {
                needed: self.service.millis(),
                offered: deadline.millis(),
            });
        }
        let slot = self
            .calls
            .get_mut(self.len)
            .ok_or(PortError::RecordingFull {
                max: MAX_RECORDED_CALLS,
            })?;
        *slot = Some(call);
        self.len = self.len.saturating_add(1);
        Ok(())
    }

    fn service_time(&self) -> CallDeadline {
        self.service
    }
}
