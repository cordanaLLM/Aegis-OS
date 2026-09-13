// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E17-1, the ring half: the submission index advances and wraps.
//!
//! The scaffold writes `ring_tail = (ring_tail + 1) % NVME_RING_SIZE` on a
//! bare integer. [`RingIndex`] owns the modulus, so the wrap is a property of
//! the type rather than of every call site, and this file is where the wrap is
//! held.

mod common;

use aegis_vulcan::{NVME_RING_SIZE, RingIndex};

use common::{Fallible, mapped, request};

// --- Positive -------------------------------------------------------------

/// Positive: a dispatch advances the tail by exactly one slot.
#[test]
fn a_dispatch_advances_the_tail_by_one() -> Fallible {
    let mut driver = mapped()?;
    assert_eq!(driver.ring_tail().get(), 0);
    let first = driver.execute_direct_dma(request(64)?)?;
    assert_eq!(first.ring_tail.get(), 1);
    let second = driver.execute_direct_dma(request(64)?)?;
    assert_eq!(second.ring_tail.get(), 2);
    assert_eq!(driver.ring_tail().get(), 2);
    Ok(())
}

/// Positive: completing a submission advances the head behind the tail.
#[test]
fn completing_a_submission_advances_the_head() -> Fallible {
    let mut driver = mapped()?;
    driver.execute_direct_dma(request(1)?)?;
    driver.execute_direct_dma(request(1)?)?;
    assert_eq!(driver.outstanding(), 2);
    assert_eq!(driver.complete()?.get(), 1);
    assert_eq!(driver.outstanding(), 1);
    assert_eq!(driver.complete()?.get(), 2);
    assert_eq!(driver.outstanding(), 0);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an index at or past the ring size is refused, not folded.
#[test]
fn an_index_outside_the_ring_is_refused() {
    assert_eq!(RingIndex::new(NVME_RING_SIZE), None);
    assert_eq!(RingIndex::new(NVME_RING_SIZE.saturating_add(1)), None);
    assert_eq!(RingIndex::new(usize::MAX), None);
}

/// Negative: a head that has caught the tail does not run past it.
#[test]
fn a_caught_up_head_does_not_pass_the_tail() -> Fallible {
    let mut driver = mapped()?;
    assert_eq!(driver.complete()?.get(), 0);
    assert_eq!(driver.complete()?.get(), 0);
    assert_eq!(driver.outstanding(), 0);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the index wraps from the last slot to the first, and only there.
#[test]
fn the_ring_index_wraps_at_the_last_slot() {
    assert_eq!(RingIndex::LAST.get(), NVME_RING_SIZE.saturating_sub(1));
    assert_eq!(RingIndex::LAST.advance(), RingIndex::ZERO);
    assert_eq!(RingIndex::ZERO.advance().get(), 1);

    let penultimate = RingIndex::new(NVME_RING_SIZE.saturating_sub(2));
    assert_eq!(penultimate.map(RingIndex::advance), Some(RingIndex::LAST));
}

/// Boundary: the forward distance is circular, so the wrap costs one step.
#[test]
fn the_forward_distance_crosses_the_wrap() {
    assert_eq!(RingIndex::ZERO.distance_to(RingIndex::ZERO), 0);
    assert_eq!(
        RingIndex::ZERO.distance_to(RingIndex::LAST),
        NVME_RING_SIZE - 1
    );
    assert_eq!(RingIndex::LAST.distance_to(RingIndex::ZERO), 1);
}
