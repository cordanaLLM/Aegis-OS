// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The stubbed systemd boundary: every effect the lifecycle needs, described
//! and never performed.
//!
//! Milestone M15 says systemd and sysupdate calls are stubbed. That is a
//! structural property here rather than a promise: the crate has no
//! dependency that can start a process, and no module outside this one even
//! names an effect. [`SysupdateCall`] is a *description* of a call, carried as
//! data; [`SysupdatePort`] is the seam a real implementation would sit behind
//! at M24 or M11; [`StubSysupdate`] is the only implementation shipped, and it
//! answers from a script it was built with.
//!
//! Nothing here runs `systemd-sysupdate`, opens a block device, writes a
//! partition, changes a boot order or reboots. `tests/stubbed_effects.rs`
//! sweeps `src/` for the identifiers that would be needed to do any of it and
//! fails if one appears.
//!
//! # What the real calls would be
//!
//! | Call | What a real implementation would do |
//! | :--- | :--- |
//! | [`SysupdateCall::List`] | `systemd-sysupdate --definitions=<dir> list`, the invocation the M03 gate already runs offline |
//! | [`SysupdateCall::Verify`] | check the release signature; see the open point below |
//! | [`SysupdateCall::Update`] | `systemd-sysupdate --definitions=<dir> update <version>`, which writes the alternate slot |
//! | [`SysupdateCall::SwapSlot`] | set the `TriesLeft=`/`TriesDone=` partition flags the transfer's `[Target]` section defines, so the alternate slot is tried next |
//! | [`SysupdateCall::BlessBoot`] | `systemd-bless-boot good` |
//! | [`SysupdateCall::RollbackBoot`] | `systemd-bless-boot bad` |
//!
//! **Open point, not settled here.** `sysupdate.d(5)` defines a `Verify=` key
//! in `[Transfer]`, and the reviewed `build/sysupdate.d/10-root.transfer` does
//! not set it: the file carries `ProtectVersion=` only. This crate models the
//! signature stage because REQ-P02-01 requires it, and does not claim the
//! reviewed definition performs it. Which key carries the release signature,
//! and against which trust store, is work for a milestone with a real signed
//! artefact.

use crate::candidate::Version;
use crate::slot::Slot;
use crate::state::{Event, SignatureVerdict};
use crate::verity::RootHash;

/// Scalar upper bound on the calls one stub records.
pub const MAX_RECORDED_CALLS: usize = 16;

/// A call without its payload.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[non_exhaustive]
pub enum CallKind {
    /// List the slots the transfer definition manages.
    List,
    /// Check the signature over a candidate release.
    Verify,
    /// Acquire the release and write it into the alternate slot.
    Update,
    /// Make the alternate slot the one the firmware tries next.
    SwapSlot,
    /// Mark the current boot good.
    BlessBoot,
    /// Mark the current boot bad.
    RollbackBoot,
}

impl CallKind {
    /// Every call kind, in declaration order.
    pub const ALL: [Self; 6] = [
        Self::List,
        Self::Verify,
        Self::Update,
        Self::SwapSlot,
        Self::BlessBoot,
        Self::RollbackBoot,
    ];

    /// Returns the stable name this kind is reported under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::List => "list",
            Self::Verify => "verify",
            Self::Update => "update",
            Self::SwapSlot => "swap-slot",
            Self::BlessBoot => "bless-boot",
            Self::RollbackBoot => "rollback-boot",
        }
    }
}

impl core::fmt::Display for CallKind {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.name())
    }
}

/// A described call. Holding one performs nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum SysupdateCall {
    /// List the slots the transfer definition manages.
    List,
    /// Check the signature over the named release.
    Verify(Version),
    /// Acquire the named release into the alternate slot.
    Update(Version),
    /// Make `Slot` the one the firmware tries next.
    SwapSlot(Slot),
    /// Mark the current boot good.
    BlessBoot,
    /// Mark the current boot bad.
    RollbackBoot,
}

impl SysupdateCall {
    /// Returns the payload-free kind of this call.
    #[must_use]
    pub const fn kind(self) -> CallKind {
        match self {
            Self::List => CallKind::List,
            Self::Verify(_) => CallKind::Verify,
            Self::Update(_) => CallKind::Update,
            Self::SwapSlot(_) => CallKind::SwapSlot,
            Self::BlessBoot => CallKind::BlessBoot,
            Self::RollbackBoot => CallKind::RollbackBoot,
        }
    }
}

/// What a call produced.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CallOutcome {
    /// The slot the transfer reports as current.
    Listed {
        /// The slot the running system booted from.
        current: Slot,
    },
    /// The signature verdict for the candidate.
    Verified(SignatureVerdict),
    /// The dm-verity root hash measured from what landed in the slot.
    Acquired(RootHash),
    /// The firmware will try `into` next.
    Swapped {
        /// The slot that is now tried first.
        into: Slot,
    },
    /// The boot was marked good.
    BlessedBoot,
    /// The boot was marked bad.
    RolledBackBoot,
}

impl CallOutcome {
    /// Returns the event this outcome offers the machine, when it offers one.
    ///
    /// [`Self::Listed`], [`Self::BlessedBoot`] and [`Self::RolledBackBoot`]
    /// offer none: listing is a query, and the two boot-assessment calls are
    /// the consequence of a state the machine already reached rather than an
    /// input that moves it.
    #[must_use]
    pub const fn into_event(self) -> Option<Event> {
        match self {
            Self::Verified(verdict) => Some(Event::CheckSignature(verdict)),
            Self::Acquired(hash) => Some(Event::AcquireDelta(hash)),
            Self::Swapped { .. } => Some(Event::SwapSlot),
            Self::Listed { .. } | Self::BlessedBoot | Self::RolledBackBoot => None,
        }
    }
}

/// Why a call was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum PortError {
    /// The call was refused by the script the stub was built with.
    #[error("the {call} call was refused")]
    Refused {
        /// The call that was refused.
        call: CallKind,
    },
    /// More calls were made than the stub records.
    #[error("the stub records at most {bound} calls")]
    RecordFull {
        /// The bound the stub holds.
        bound: usize,
    },
}

/// The seam a real systemd-sysupdate implementation would sit behind.
pub trait SysupdatePort {
    /// Performs `call` and reports what it produced.
    ///
    /// # Errors
    ///
    /// Returns [`PortError`] when the call cannot be performed. A lifecycle
    /// driver treats every refusal as fail-closed: it offers the machine no
    /// event, so no transition is recorded.
    fn invoke(&mut self, call: SysupdateCall) -> Result<CallOutcome, PortError>;
}

/// The stubbed port: answers from a script, performs nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StubSysupdate {
    current: Slot,
    verdict: SignatureVerdict,
    measured: RootHash,
    refuse: Option<CallKind>,
    calls: [Option<CallKind>; MAX_RECORDED_CALLS],
    len: u8,
}

impl StubSysupdate {
    /// Builds a stub that reports `current` as the booted slot, answers a
    /// signature check with `verdict`, and reports `measured` for an update.
    #[must_use]
    pub const fn new(current: Slot, verdict: SignatureVerdict, measured: RootHash) -> Self {
        Self {
            current,
            verdict,
            measured,
            refuse: None,
            calls: [None; MAX_RECORDED_CALLS],
            len: 0,
        }
    }

    /// Returns the same stub with `call` scripted to be refused.
    #[must_use]
    pub const fn refusing(mut self, call: CallKind) -> Self {
        self.refuse = Some(call);
        self
    }

    /// Returns the slot the stub reports as booted.
    #[must_use]
    pub const fn current(&self) -> Slot {
        self.current
    }

    /// Returns how many calls the stub has been asked for.
    #[must_use]
    pub const fn call_count(&self) -> usize {
        self.len as usize
    }

    /// Returns the kind of the call at `index`, in call order.
    #[must_use]
    pub fn call(&self, index: usize) -> Option<CallKind> {
        if index >= self.call_count() {
            return None;
        }
        self.calls.get(index).copied().flatten()
    }

    /// Records `kind` in call order.
    fn record(&mut self, kind: CallKind) -> Result<(), PortError> {
        let at = self.call_count();
        let slot = self.calls.get_mut(at).ok_or(PortError::RecordFull {
            bound: MAX_RECORDED_CALLS,
        })?;
        *slot = Some(kind);
        self.len = self.len.saturating_add(1);
        Ok(())
    }

    /// Returns the scripted answer for `call`.
    const fn answer(&self, call: SysupdateCall) -> CallOutcome {
        match call {
            SysupdateCall::List => CallOutcome::Listed {
                current: self.current,
            },
            SysupdateCall::Verify(_) => CallOutcome::Verified(self.verdict),
            SysupdateCall::Update(_) => CallOutcome::Acquired(self.measured),
            SysupdateCall::SwapSlot(into) => CallOutcome::Swapped { into },
            SysupdateCall::BlessBoot => CallOutcome::BlessedBoot,
            SysupdateCall::RollbackBoot => CallOutcome::RolledBackBoot,
        }
    }
}

impl SysupdatePort for StubSysupdate {
    fn invoke(&mut self, call: SysupdateCall) -> Result<CallOutcome, PortError> {
        let kind = call.kind();
        self.record(kind)?;
        if self.refuse == Some(kind) {
            return Err(PortError::Refused { call: kind });
        }
        Ok(self.answer(call))
    }
}
