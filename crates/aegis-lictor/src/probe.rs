// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fragility probes: shadowed structural change before promotion
//! (REQ-P07-05).
//!
//! The P07 report requires eBPF-driven fragility probes that test a shadowed
//! structural change on a **non-authoritative** path before it is promoted, so
//! a kernel update cannot cause catastrophic drift (export-004
//! `15831276a058`). The ordering that requirement asks for is a state machine,
//! and this module is that state machine: a change is [`ProbeStage::Shadowed`]
//! first, [`ProbeStage::Observed`] only after it has run somewhere that does
//! not decide anything, and [`ProbeStage::Promoted`] only from Observed.
//!
//! [`ProbeStage::Shadowed`] to [`ProbeStage::Promoted`] is
//! [`LictorError::IllegalProbeTransition`]. That refusal is the requirement;
//! the rest of it -- that the shadowed path is genuinely non-authoritative,
//! that the probe is eBPF-driven, that it observes anything at all -- needs a
//! loaded program and is milestone M19.
//!
//! # What this module does not do
//!
//! **No program is compiled, loaded or attached, and nothing is observed.** A
//! probe is a value; "shadowed" is a stage, not an execution context.

use crate::error::LictorError;

/// The stages a fragility probe passes through.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum ProbeStage {
    /// The change runs on a path that decides nothing.
    Shadowed,
    /// The shadowed run produced a result someone can compare.
    Observed,
    /// The change is on the authoritative path.
    Promoted,
    /// The change was withdrawn without being promoted.
    Rejected,
}

impl ProbeStage {
    /// All four stages, in the order this module declares them.
    pub const ALL: [Self; 4] = [
        Self::Shadowed,
        Self::Observed,
        Self::Promoted,
        Self::Rejected,
    ];

    /// Returns the stable name this stage is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Shadowed => "shadowed",
            Self::Observed => "observed",
            Self::Promoted => "promoted",
            Self::Rejected => "rejected",
        }
    }

    /// Returns `true` when a change in this stage is on the authoritative path.
    #[must_use]
    pub const fn is_authoritative(self) -> bool {
        matches!(self, Self::Promoted)
    }

    /// Returns `true` when nothing may follow this stage.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Promoted | Self::Rejected)
    }

    /// Returns `true` when the lifecycle admits a step from `self` to `next`.
    #[must_use]
    pub const fn may_advance_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Shadowed, Self::Observed | Self::Rejected)
                | (Self::Observed, Self::Promoted | Self::Rejected)
        )
    }
}

/// One fragility probe.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FragilityProbe {
    stage: ProbeStage,
}

impl Default for FragilityProbe {
    fn default() -> Self {
        Self::new()
    }
}

impl FragilityProbe {
    /// Builds a probe in [`ProbeStage::Shadowed`].
    ///
    /// There is no other constructor, which is what makes the shadowed run
    /// unavoidable: a probe cannot start anywhere else.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            stage: ProbeStage::Shadowed,
        }
    }

    /// Returns the stage the probe is in.
    #[must_use]
    pub const fn stage(self) -> ProbeStage {
        self.stage
    }

    /// Steps the probe to `next`.
    ///
    /// # Errors
    ///
    /// Returns [`LictorError::ProbeNotObserved`] for the one step the
    /// requirement is about -- Shadowed straight to Promoted -- and
    /// [`LictorError::IllegalProbeTransition`] for every other step the
    /// lifecycle does not admit.
    pub const fn advance(&mut self, next: ProbeStage) -> Result<ProbeStage, LictorError> {
        if !self.stage.may_advance_to(next) {
            if matches!(
                (self.stage, next),
                (ProbeStage::Shadowed, ProbeStage::Promoted)
            ) {
                return Err(LictorError::ProbeNotObserved);
            }
            return Err(LictorError::IllegalProbeTransition {
                from: self.stage,
                to: next,
            });
        }
        self.stage = next;
        Ok(next)
    }
}
