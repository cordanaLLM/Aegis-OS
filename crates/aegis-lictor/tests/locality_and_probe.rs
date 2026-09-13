// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Locality enforcement and fragility probes (REQ-P07-02, REQ-P07-05).
//!
//! Positive: a mask names the cores it was given, and a probe walks the
//! recorded path. Negative: an index the mask cannot hold is refused, and a
//! probe promoted straight from its shadowed run is refused with the variant
//! that names why. Boundary: the mask is exact at index 63 and 64, and the
//! probe transition table is checked at all sixteen ordered pairs.

mod common;

use aegis_lictor::{CORE_MASK_WIDTH, CoreMask, FragilityProbe, LictorError, ProbeStage};

// --- Positive -------------------------------------------------------------

/// Positive: a mask names the cores it was given.
#[test]
fn a_mask_names_the_cores_it_was_given() -> Result<(), LictorError> {
    let mask = CoreMask::new().with(0)?.with(1)?.with(16)?;
    assert_eq!(mask.count(), 3);
    assert!(mask.contains(0));
    assert!(mask.contains(1));
    assert!(mask.contains(16));
    assert!(!mask.contains(2));
    assert!(!mask.is_empty());
    assert_eq!(mask.bits(), 1 | 2 | (1u64 << 16));
    Ok(())
}

/// Positive: an empty mask names nothing, and adding the same core twice adds
/// it once.
#[test]
fn an_empty_mask_names_nothing_and_adding_twice_adds_once() -> Result<(), LictorError> {
    let empty = CoreMask::default();
    assert!(empty.is_empty());
    assert_eq!(empty.count(), 0);
    assert_eq!(empty.bits(), 0);
    let once = empty.with(7)?;
    assert_eq!(once.with(7)?, once);
    assert_eq!(once.count(), 1);
    Ok(())
}

/// Positive: a probe walks the recorded path, and only a promoted change is on
/// the authoritative path.
#[test]
fn a_probe_walks_the_recorded_path() -> Result<(), LictorError> {
    let mut probe = FragilityProbe::new();
    assert_eq!(probe.stage(), ProbeStage::Shadowed);
    assert!(!probe.stage().is_authoritative());
    assert_eq!(probe.advance(ProbeStage::Observed)?, ProbeStage::Observed);
    assert!(!probe.stage().is_authoritative());
    assert_eq!(probe.advance(ProbeStage::Promoted)?, ProbeStage::Promoted);
    assert!(probe.stage().is_authoritative());
    assert!(probe.stage().is_terminal());
    assert_eq!(FragilityProbe::default(), FragilityProbe::new());
    Ok(())
}

/// Positive: every stage carries a distinct stable name.
#[test]
fn every_probe_stage_carries_a_distinct_name() {
    let mut names: Vec<&str> = ProbeStage::ALL.iter().map(|stage| stage.name()).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), 4);
    assert_eq!(ProbeStage::Shadowed.name(), "shadowed");
    assert!(!ProbeStage::Shadowed.is_terminal());
    assert!(ProbeStage::Rejected.is_terminal());
    assert!(!ProbeStage::Rejected.is_authoritative());
}

// --- Negative -------------------------------------------------------------

/// Negative: an index the mask cannot hold is refused rather than shifted out.
#[test]
fn an_index_the_mask_cannot_hold_is_refused() {
    let refusal = CoreMask::new().with(CORE_MASK_WIDTH);
    assert_eq!(
        refusal,
        Err(LictorError::CoreOutOfRange {
            core: CORE_MASK_WIDTH,
            width: CORE_MASK_WIDTH,
        })
    );
    assert!(CoreMask::new().with(u32::MAX).is_err());
    assert!(!CoreMask::new().contains(CORE_MASK_WIDTH));
    assert!(!CoreMask::new().contains(u32::MAX));
}

/// Negative: REQ-P07-05, as a refusal. A change cannot be promoted from its
/// shadowed run.
#[test]
fn promotion_without_observation_is_refused() {
    let mut probe = FragilityProbe::new();
    assert_eq!(
        probe.advance(ProbeStage::Promoted),
        Err(LictorError::ProbeNotObserved)
    );
    assert_eq!(probe.stage(), ProbeStage::Shadowed);
    assert!(
        LictorError::ProbeNotObserved
            .to_string()
            .contains("before it is observed")
    );
}

/// Negative: a terminal probe is finished, and every other inadmissible step
/// names the pair it refused.
#[test]
fn a_terminal_probe_is_finished() -> Result<(), LictorError> {
    let mut probe = FragilityProbe::new();
    probe.advance(ProbeStage::Rejected)?;
    assert_eq!(
        probe.advance(ProbeStage::Observed),
        Err(LictorError::IllegalProbeTransition {
            from: ProbeStage::Rejected,
            to: ProbeStage::Observed,
        })
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the mask is exact at its last index and at one past it.
#[test]
fn the_mask_is_exact_at_its_last_index() -> Result<(), LictorError> {
    let last = CORE_MASK_WIDTH.saturating_sub(1);
    let mask = CoreMask::new().with(last)?;
    assert!(mask.contains(last));
    assert_eq!(mask.count(), 1);
    assert_eq!(mask.bits(), 1u64 << 63);
    assert!(CoreMask::new().with(CORE_MASK_WIDTH).is_err());
    assert_eq!(CORE_MASK_WIDTH, 64);
    Ok(())
}

/// Boundary: the probe transition table is checked at all sixteen ordered
/// pairs.
///
/// A test that listed only the interesting steps would pass while an
/// unintended one crept in beside them. This one enumerates the whole product
/// of the stage set and compares it against the recorded lifecycle.
#[test]
fn every_ordered_pair_of_probe_stages_matches_the_lifecycle() {
    let admitted: [(ProbeStage, ProbeStage); 4] = [
        (ProbeStage::Shadowed, ProbeStage::Observed),
        (ProbeStage::Shadowed, ProbeStage::Rejected),
        (ProbeStage::Observed, ProbeStage::Promoted),
        (ProbeStage::Observed, ProbeStage::Rejected),
    ];
    let mut seen = 0usize;
    for from in ProbeStage::ALL {
        for to in ProbeStage::ALL {
            seen = seen.saturating_add(1);
            assert_eq!(
                from.may_advance_to(to),
                admitted.contains(&(from, to)),
                "the pair {from:?} -> {to:?} disagrees with the recorded lifecycle"
            );
        }
    }
    assert_eq!(seen, 16);
}
