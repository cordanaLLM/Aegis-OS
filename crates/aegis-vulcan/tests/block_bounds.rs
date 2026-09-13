// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E17-1, the block-count half: 0 and 8193 are rejected, 8192 is
//! accepted, and the byte arithmetic follows the count.
//!
//! REQ-P03-05 bounds the count at `1..=8192`. The scaffold asserts it at
//! dispatch; [`BlockCount::new`] refuses it at construction, so a dispatch
//! cannot be reached with a count the requirement refuses.

mod common;

use aegis_vulcan::{BlockCount, MAX_BLOCK_COUNT, MIN_BLOCK_COUNT, NVME_BLOCK_BYTES, VulcanError};

use common::{Fallible, mapped, request};

// --- Positive -------------------------------------------------------------

/// Positive: an admissible count is accepted and reports its transfer size.
#[test]
fn an_admissible_block_count_is_accepted() -> Fallible {
    let blocks = BlockCount::new(64)?;
    assert_eq!(blocks.get(), 64);
    assert_eq!(blocks.transfer_bytes(), 64 * NVME_BLOCK_BYTES);
    Ok(())
}

/// Positive: dispatching an admissible count reports the same arithmetic.
#[test]
fn a_dispatch_reports_the_recorded_byte_count() -> Fallible {
    let mut driver = mapped()?;
    let receipt = driver.execute_direct_dma(request(64)?)?;
    assert_eq!(receipt.bytes, 64 * NVME_BLOCK_BYTES);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: zero blocks is refused, and the refusal names the count.
#[test]
fn a_block_count_of_zero_is_rejected() {
    assert_eq!(
        BlockCount::new(0),
        Err(VulcanError::BlockCountOutOfRange { blocks: 0 })
    );
}

/// Negative: one past the bound is refused, and the refusal names the count.
#[test]
fn a_block_count_past_the_bound_is_rejected() {
    assert_eq!(
        BlockCount::new(8193),
        Err(VulcanError::BlockCountOutOfRange { blocks: 8193 })
    );
    assert_eq!(
        BlockCount::new(u32::MAX),
        Err(VulcanError::BlockCountOutOfRange { blocks: u32::MAX })
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: 0 and 8193 are rejected, 1 and 8192 are accepted.
#[test]
fn the_block_count_bound_is_exact_on_both_sides() {
    assert!(BlockCount::new(0).is_err());
    assert!(BlockCount::new(MIN_BLOCK_COUNT).is_ok());
    assert_eq!(BlockCount::new(8192).map(BlockCount::get), Ok(8192));
    assert!(BlockCount::new(8193).is_err());
    assert_eq!(MAX_BLOCK_COUNT, 8192);
    assert_eq!(BlockCount::MAX.get(), MAX_BLOCK_COUNT);
    assert_eq!(BlockCount::MIN.get(), MIN_BLOCK_COUNT);
}

/// Boundary: the widest admissible transfer is exactly four mebibytes.
#[test]
fn the_widest_admissible_transfer_is_four_mebibytes() -> Fallible {
    assert_eq!(BlockCount::MAX.transfer_bytes(), 4 * 1024 * 1024);
    let mut driver = mapped()?;
    let receipt = driver.execute_direct_dma(request(MAX_BLOCK_COUNT)?)?;
    assert_eq!(receipt.bytes, 4 * 1024 * 1024);
    Ok(())
}
