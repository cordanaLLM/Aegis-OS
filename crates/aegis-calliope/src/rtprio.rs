// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The real-time priority grant and the reference-profile limits it was read
//! against (REQ-P08-02, REQ-P08-05).
//!
//! The P08 report states two resource limits as non-negotiable for the audio
//! threads: `RLIMIT_RTPRIO` 95 and `RLIMIT_MEMLOCK` infinity (export-017
//! `afbc0af8056d`). This module types the first as a grant P07 hands P08 and
//! records what the reference development machine actually offers for both.
//!
//! # Two figures, two kinds
//!
//! `RLIMIT_RTPRIO` is a ceiling on the priority a process may request, not a
//! priority anything runs at. The register records P08's requirement as
//! `RLIMIT_RTPRIO=95, CPUSchedulingPolicy=rr at priority 80`, so the rlimit is
//! [`REQUIRED_RTPRIO`] and the setting is [`UNIT_SCHED_PRIORITY`]. Reading the
//! 95 as the setting would make 80 look like a contradiction of it rather than
//! a value it authorises, which is why they are separate constants here and
//! why [`GrantedRtPrio::new`] bounds a grant by the first of them.
//!
//! # Measured, not assumed
//!
//! [`REFERENCE_PROFILE_LIMITS`] carries two figures read from the reference
//! profile with `ulimit -r` and `ulimit -l` on the date it records. They are a
//! dated reading of one developer account on one machine, which is what
//! `planning/hardware-profile.json` calls development evidence: they qualify
//! no hardware and close no gate. What they do is make the gap visible:
//!
//! * `ulimit -r` printed **99**, so the account may already ask for the 95 the
//!   report requires -- [`ReferenceProfileLimits::grants_required_rtprio`] is
//!   `true`;
//! * `ulimit -l` printed **8192** kibibytes, which is not infinity -- so
//!   [`ReferenceProfileLimits::meets_source_memlock`] is `false` and
//!   [`ReferenceProfileLimits::memlock_action`] is
//!   [`PrivilegedAction::RaiseMemlock`]. The gap is a privileged configuration
//!   action someone must take, not an assumption this crate may make.
//!
//! # What this module does not do
//!
//! **No limit is read, set or enforced at run time, and no thread is
//! scheduled.** `getrlimit`, `setrlimit`, `sched_setscheduler` and
//! `pthread_setschedparam` appear nowhere in this crate; the two figures above
//! are constants recorded from a shell reading, and
//! `tests/stubbed_effects.rs` fails if the machinery to read them ever
//! appears. **No latency or determinism figure is produced here.** Milestone
//! M23's fixture does use [`REQUIRED_RTPRIO`]: `cyclictest` runs its measuring
//! thread at priority 95 on both machines, so the figures in
//! [`crate::measured`] were taken at the priority this module records rather
//! than at the tool's customary 99.

use crate::error::CalliopeError;

/// The `RLIMIT_RTPRIO` ceiling the P08 report calls non-negotiable.
///
/// Recorded from export-017 `afbc0af8056d` (REQ-P08-02), which
/// `planning/components.json` carries for P08 as `RLIMIT_RTPRIO=95,
/// CPUSchedulingPolicy=rr at priority 80`. Those are two figures of different
/// kinds, and 95 is not the setting: `RLIMIT_RTPRIO` bounds the priority a
/// process may *request*, so 95 authorises `1..=95` and refuses anything
/// above. The setting the same requirement names is [`UNIT_SCHED_PRIORITY`],
/// which is 80 and which this ceiling therefore admits.
///
/// The constant is read twice, in opposite directions, and the crate keeps the
/// two apart:
///
/// * from **below**, as what an account's own limit must reach before the unit
///   can run -- which is why
///   [`ReferenceProfileLimits::grants_required_rtprio`] compares the measured
///   [`REFERENCE_PROFILE_RTPRIO_CEILING`] of 99 against it and reports `true`;
/// * from **above**, as the largest priority any grant may carry -- which is
///   what [`GrantedRtPrio::new`] implements, and why 96 is refused even on an
///   account whose own ceiling is 99.
pub const REQUIRED_RTPRIO: u8 = 95;

/// The `SCHED_RR` priority the recorded plugin-host unit sets.
///
/// The other half of the register's P08 requirement,
/// `CPUSchedulingPolicy=rr at priority 80`: this is the setting, where
/// [`REQUIRED_RTPRIO`] is the rlimit that authorises it. It is recorded rather
/// than used, because this crate schedules nothing;
/// `tests/realtime_limits.rs` checks the one relation between the two figures
/// that has to hold, which is that the ceiling admits the setting.
pub const UNIT_SCHED_PRIORITY: u8 = 80;

/// The lowest real-time priority a grant admits.
///
/// Zero is the non-real-time priority under `SCHED_RR`, so a grant of zero
/// would be a grant of nothing under a name that says otherwise.
pub const MIN_RTPRIO: u8 = 1;

/// The `RLIMIT_RTPRIO` ceiling read from the reference profile.
///
/// Measured with `ulimit -r`, which printed 99.
pub const REFERENCE_PROFILE_RTPRIO_CEILING: u8 = 99;

/// The `RLIMIT_MEMLOCK` ceiling read from the reference profile, in kibibytes.
///
/// Measured with `ulimit -l`, which printed 8192.
pub const REFERENCE_PROFILE_MEMLOCK_KIB: u64 = 8192;

/// The scheduling policies a grant may name.
///
/// One variant, on purpose. The P08 report's own plugin-host unit sets
/// `CPUSchedulingPolicy=rr`, and a payload naming any other policy should fail
/// to decode rather than be discouraged in prose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
#[non_exhaustive]
pub enum SchedPolicy {
    /// `SCHED_RR`, the round-robin real-time policy.
    #[serde(rename = "sched-rr")]
    RoundRobin,
}

impl SchedPolicy {
    /// The policy the recorded unit names, and the only one this build admits.
    pub const ADMITTED: Self = Self::RoundRobin;

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::RoundRobin => "sched-rr",
        }
    }
}

/// What a source says about `RLIMIT_MEMLOCK`, or what a machine offers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MemlockExpectation {
    /// No ceiling: the report's "infinity".
    Unlimited,
    /// A ceiling in kibibytes.
    KibibyteCeiling(u64),
}

impl MemlockExpectation {
    /// Returns `true` when `self` satisfies `required`.
    ///
    /// A finite ceiling never satisfies [`Self::Unlimited`], which is the
    /// whole point: 8192 kibibytes is not infinity, and rounding that up in
    /// prose is how an unmet requirement becomes an invisible one.
    #[must_use]
    pub const fn satisfies(self, required: Self) -> bool {
        match (self, required) {
            (Self::Unlimited, _) => true,
            (Self::KibibyteCeiling(_), Self::Unlimited) => false,
            (Self::KibibyteCeiling(have), Self::KibibyteCeiling(need)) => have >= need,
        }
    }
}

/// What the memlock gap asks someone to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PrivilegedAction {
    /// The reading already satisfies the requirement.
    NoneNeeded,
    /// `RLIMIT_MEMLOCK` must be raised before the requirement holds.
    RaiseMemlock,
}

impl PrivilegedAction {
    /// Returns the stable name this action is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NoneNeeded => "none-needed",
            Self::RaiseMemlock => "raise-rlimit-memlock",
        }
    }
}

/// The limits read from the reference development machine.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ReferenceProfileLimits {
    /// The `RLIMIT_RTPRIO` ceiling the account holds.
    pub rtprio_ceiling: u8,
    /// The `RLIMIT_MEMLOCK` ceiling the account holds.
    pub memlock: MemlockExpectation,
    /// What the P08 report requires of `RLIMIT_MEMLOCK`.
    pub memlock_required: MemlockExpectation,
    /// The date the two figures were read.
    pub probed_on: &'static str,
    /// The command that produced them.
    pub evidence_command: &'static str,
}

impl ReferenceProfileLimits {
    /// Returns `true` when the account may ask for [`REQUIRED_RTPRIO`].
    #[must_use]
    pub const fn grants_required_rtprio(&self) -> bool {
        self.rtprio_ceiling >= REQUIRED_RTPRIO
    }

    /// Returns `true` when the account's memlock ceiling meets the report.
    #[must_use]
    pub const fn meets_source_memlock(&self) -> bool {
        self.memlock.satisfies(self.memlock_required)
    }

    /// Returns the privileged action the memlock reading implies.
    #[must_use]
    pub const fn memlock_action(&self) -> PrivilegedAction {
        if self.meets_source_memlock() {
            PrivilegedAction::NoneNeeded
        } else {
            PrivilegedAction::RaiseMemlock
        }
    }
}

/// The reference-profile reading this milestone records.
///
/// A dated reading of one account on one machine. It is development evidence
/// and closes no gate; see the module documentation.
pub const REFERENCE_PROFILE_LIMITS: ReferenceProfileLimits = ReferenceProfileLimits {
    rtprio_ceiling: REFERENCE_PROFILE_RTPRIO_CEILING,
    memlock: MemlockExpectation::KibibyteCeiling(REFERENCE_PROFILE_MEMLOCK_KIB),
    memlock_required: MemlockExpectation::Unlimited,
    probed_on: "2026-09-13",
    evidence_command: "ulimit -r; ulimit -l",
};

/// A validated real-time priority, admissible in `MIN_RTPRIO..=REQUIRED_RTPRIO`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct GrantedRtPrio(u8);

impl GrantedRtPrio {
    /// The largest grant the recorded `RLIMIT_RTPRIO` ceiling authorises.
    ///
    /// Not the priority the unit sets -- that is [`UNIT_SCHED_PRIORITY`], 80.
    /// This is the top of the range [`Self::new`] admits.
    pub const REQUIRED: Self = Self(REQUIRED_RTPRIO);

    /// Validates `value` against `MIN_RTPRIO..=REQUIRED_RTPRIO`.
    ///
    /// The upper bound is the recorded `RLIMIT_RTPRIO` ceiling and not the
    /// kernel's 99. An rlimit bounds the priority a process may request, so 95
    /// authorises `1..=95`: a grant of 96 is one the source does not
    /// authorise, whatever the account's own ceiling happens to allow. That
    /// account ceiling is a different figure, recorded separately in
    /// [`REFERENCE_PROFILE_LIMITS`], and on the reference profile it is
    /// higher -- which is exactly why the two must not be conflated.
    ///
    /// # Errors
    ///
    /// Returns [`CalliopeError::RtPrioOutOfRange`] outside that range, so an
    /// unauthorised priority never becomes a value.
    pub const fn new(value: u8) -> Result<Self, CalliopeError> {
        if value < MIN_RTPRIO || value > REQUIRED_RTPRIO {
            return Err(CalliopeError::RtPrioOutOfRange {
                value,
                min: MIN_RTPRIO,
                max: REQUIRED_RTPRIO,
            });
        }
        Ok(Self(value))
    }

    /// Returns the validated priority.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// Returns `true` when the reference profile's ceiling admits this grant.
    ///
    /// A statement about one recorded reading, not about any machine.
    #[must_use]
    pub const fn within_reference_profile_ceiling(self) -> bool {
        self.0 <= REFERENCE_PROFILE_RTPRIO_CEILING
    }
}
