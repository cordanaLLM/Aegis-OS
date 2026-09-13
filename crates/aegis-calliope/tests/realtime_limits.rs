// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-5, the priority half, and the milestone's reference-profile
//! record.
//!
//! Positive: a grant at the top of the recorded `RLIMIT_RTPRIO` range is
//! accepted, the priority the recorded unit sets is accepted under it, and the
//! reference-profile reading is what the shell printed. Negative: a priority
//! the rlimit does not authorise is refused, and so is zero. Boundary: 95 is
//! accepted and 96 is not.
//!
//! The register records P08's requirement as `RLIMIT_RTPRIO=95,
//! CPUSchedulingPolicy=rr at priority 80`. Those are two figures of different
//! kinds: 95 is a ceiling on what a process may request and 80 is what the
//! unit actually asks for, so the relation that has to hold between them is
//! that the first admits the second.
//!
//! The two figures this file pins are a dated reading of one developer account
//! on one machine, taken with `ulimit -r` and `ulimit -l` on 2026-09-13. They
//! qualify no hardware and close no gate; what they do is make the memlock gap
//! visible as a privileged-configuration action rather than an assumption.

mod common;

use aegis_calliope::{
    CalliopeError, GrantedRtPrio, MIN_RTPRIO, MemlockExpectation, PrivilegedAction,
    REFERENCE_PROFILE_LIMITS, REFERENCE_PROFILE_MEMLOCK_KIB, REFERENCE_PROFILE_RTPRIO_CEILING,
    REQUIRED_RTPRIO, ReferenceProfileLimits, SchedPolicy, UNIT_SCHED_PRIORITY,
};

// --- Positive -------------------------------------------------------------

/// Positive: the source's own priority is a grant, and reads back.
#[test]
fn the_source_priority_is_a_grant() -> Result<(), CalliopeError> {
    let granted = GrantedRtPrio::new(REQUIRED_RTPRIO)?;
    assert_eq!(granted.get(), 95);
    assert_eq!(granted, GrantedRtPrio::REQUIRED);
    assert!(granted.within_reference_profile_ceiling());
    assert_eq!(GrantedRtPrio::new(MIN_RTPRIO)?.get(), 1);
    Ok(())
}

/// Positive: the rlimit the register records admits the priority it records
/// beside it.
///
/// `RLIMIT_RTPRIO=95` is a ceiling on what may be requested and
/// `CPUSchedulingPolicy=rr at priority 80` is the request, so the two are
/// consistent exactly when the grant range admits 80. Reading the 95 as the
/// setting instead would put the register in conflict with itself.
#[test]
fn the_recorded_ceiling_admits_the_recorded_setting() -> Result<(), CalliopeError> {
    let setting = GrantedRtPrio::new(UNIT_SCHED_PRIORITY)?;
    assert_eq!(setting.get(), 80);
    assert!(setting < GrantedRtPrio::REQUIRED);
    assert_eq!(GrantedRtPrio::REQUIRED.get(), REQUIRED_RTPRIO);
    assert!(setting.within_reference_profile_ceiling());
    assert_eq!(SchedPolicy::ADMITTED.tag(), "sched-rr");
    Ok(())
}

/// Positive: the reference profile grants the priority the source requires.
///
/// `ulimit -r` printed 99 on the reference profile, which is four above the
/// 95 REQ-P08-02 calls non-negotiable.
#[test]
fn the_reference_profile_grants_the_required_priority() {
    let limits: ReferenceProfileLimits = REFERENCE_PROFILE_LIMITS;
    assert_eq!(limits.rtprio_ceiling, 99);
    assert_eq!(limits.rtprio_ceiling, REFERENCE_PROFILE_RTPRIO_CEILING);
    assert!(limits.rtprio_ceiling > REQUIRED_RTPRIO);
    assert!(limits.grants_required_rtprio());
    assert_eq!(limits.probed_on, "2026-09-13");
    assert_eq!(limits.evidence_command, "ulimit -r; ulimit -l");
}

/// Positive: the scheduling policy the recorded unit names is the admitted one.
#[test]
fn the_admitted_policy_is_round_robin() {
    assert_eq!(SchedPolicy::ADMITTED, SchedPolicy::RoundRobin);
    assert_eq!(SchedPolicy::ADMITTED.tag(), "sched-rr");
}

// --- Negative -------------------------------------------------------------

/// Negative: the reference profile does **not** meet the memlock requirement,
/// and the gap is reported as an action rather than rounded away.
///
/// `ulimit -l` printed 8192 kibibytes. The P08 report asks for infinity, and a
/// finite ceiling is not infinity however large it is.
#[test]
fn the_reference_profile_does_not_meet_the_memlock_requirement() {
    let limits = REFERENCE_PROFILE_LIMITS;
    assert_eq!(
        limits.memlock,
        MemlockExpectation::KibibyteCeiling(REFERENCE_PROFILE_MEMLOCK_KIB)
    );
    assert_eq!(REFERENCE_PROFILE_MEMLOCK_KIB, 8192);
    assert_eq!(limits.memlock_required, MemlockExpectation::Unlimited);
    assert!(!limits.meets_source_memlock());
    assert_eq!(limits.memlock_action(), PrivilegedAction::RaiseMemlock);
    assert_eq!(limits.memlock_action().name(), "raise-rlimit-memlock");
}

/// Negative: a priority of zero is not a grant.
#[test]
fn a_priority_of_zero_is_refused() {
    assert_eq!(
        GrantedRtPrio::new(0),
        Err(CalliopeError::RtPrioOutOfRange {
            value: 0,
            min: MIN_RTPRIO,
            max: REQUIRED_RTPRIO,
        })
    );
}

/// Negative: a finite ceiling never satisfies the unlimited requirement,
/// whatever its size, and an unlimited one satisfies anything.
#[test]
fn a_finite_ceiling_never_satisfies_an_unlimited_requirement() {
    let huge = MemlockExpectation::KibibyteCeiling(u64::MAX);
    assert!(!huge.satisfies(MemlockExpectation::Unlimited));
    assert!(MemlockExpectation::Unlimited.satisfies(MemlockExpectation::Unlimited));
    assert!(MemlockExpectation::Unlimited.satisfies(MemlockExpectation::KibibyteCeiling(1)));
    assert!(huge.satisfies(MemlockExpectation::KibibyteCeiling(8192)));
    assert!(
        !MemlockExpectation::KibibyteCeiling(8191)
            .satisfies(MemlockExpectation::KibibyteCeiling(8192))
    );
    assert_eq!(PrivilegedAction::NoneNeeded.name(), "none-needed");
}

// --- Boundary -------------------------------------------------------------

/// Boundary: 95 is accepted and 96 is refused.
///
/// The upper bound is the recorded `RLIMIT_RTPRIO` ceiling rather than the
/// kernel's 99. An rlimit bounds what a process may request, so 95 authorises
/// `1..=95` and refuses 96 -- and it does so even here, where the account's
/// own measured ceiling is 99. That account ceiling is a separate recorded
/// figure, and the two are deliberately not the same number.
#[test]
fn the_grant_is_exact_at_the_source_value() {
    assert!(GrantedRtPrio::new(94).is_ok());
    assert!(GrantedRtPrio::new(95).is_ok());
    assert_eq!(
        GrantedRtPrio::new(96),
        Err(CalliopeError::RtPrioOutOfRange {
            value: 96,
            min: MIN_RTPRIO,
            max: REQUIRED_RTPRIO,
        })
    );
    assert!(GrantedRtPrio::new(REFERENCE_PROFILE_RTPRIO_CEILING).is_err());
    assert!(GrantedRtPrio::new(u8::MAX).is_err());
}
