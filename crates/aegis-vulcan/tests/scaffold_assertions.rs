// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Exit criterion 2: the scaffold's assertions are refusals, not panics.
//!
//! The imported P03 scaffold (export-038 `fe2cb01cfd13`) states three rules as
//! `assert!` calls. This file is the enumeration of those three, each shown
//! returning a value rather than aborting, and each named against the
//! requirement it comes from.
//!
//! The enumeration is the point: it is a fixed list of the three assertions
//! that existed, so removing one from the library without removing it here
//! fails the gate. It is a regression gate over that named list, not a proof
//! that the crate can never panic anywhere.

#[macro_use]
mod common;

use aegis_vulcan::{BarAddress, BlockCount, UserSpacePcieDriver, VulcanError};

use common::{Fallible, MISALIGNED_BAR, device, request};

/// The three scaffold assertions, by the rule each states.
const SCAFFOLD_ASSERTIONS: [&str; 3] = [
    "the BAR physical address must be page aligned",
    "a DMA transfer cannot be dispatched without a mapped BAR",
    "the block count must be within 1..=8192",
];

// --- Positive -------------------------------------------------------------

/// Positive: every rule the scaffold asserted still holds, as a value.
///
/// Each arm is the same rule as the corresponding `assert!`, checked by
/// reading a `Result` instead of by surviving the call.
#[test]
fn every_scaffold_assertion_is_now_a_result() -> Fallible {
    assert_eq!(SCAFFOLD_ASSERTIONS.len(), 3);

    let misaligned: Result<BarAddress, VulcanError> = BarAddress::new(MISALIGNED_BAR);
    assert!(misaligned.is_err());

    let mut unmapped = UserSpacePcieDriver::new(device()?);
    let dispatched = unmapped.execute_direct_dma(request(1)?);
    assert!(dispatched.is_err());

    let oversized: Result<BlockCount, VulcanError> = BlockCount::new(8193);
    assert!(oversized.is_err());
    Ok(())
}

/// Positive: each refusal is a distinct, matchable variant.
#[test]
fn each_refusal_is_a_distinct_variant() -> Fallible {
    let mut driver = UserSpacePcieDriver::new(device()?);
    let refusals = [
        BarAddress::new(MISALIGNED_BAR).err(),
        driver.execute_direct_dma(request(1)?).err(),
        BlockCount::new(0).err(),
    ];
    assert_eq!(
        refusals,
        [
            Some(VulcanError::BarMisaligned {
                address: MISALIGNED_BAR
            }),
            Some(VulcanError::BarUnmapped),
            Some(VulcanError::BlockCountOutOfRange { blocks: 0 }),
        ]
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a refused call changes nothing, so the next call sees the same
/// state a successful one would have.
#[test]
fn a_refused_call_leaves_the_model_where_it_was() -> Fallible {
    let mut driver = UserSpacePcieDriver::new(device()?);
    let before = driver;
    assert!(driver.execute_direct_dma(request(1)?).is_err());
    assert_eq!(driver, before);

    driver.map_bar()?;
    let mapped = driver;
    assert!(driver.map_bar().is_err());
    assert_eq!(driver, mapped);
    Ok(())
}

define_scaffold_sweep_tests!();
