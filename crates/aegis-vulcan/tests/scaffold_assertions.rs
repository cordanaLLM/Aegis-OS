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

mod common;

use aegis_vulcan::{BarAddress, BlockCount, UserSpacePcieDriver, VulcanError};

use common::{Fallible, MISALIGNED_BAR, device, request};

/// The three scaffold assertions, by the rule each states.
const SCAFFOLD_ASSERTIONS: [&str; 3] = [
    "the BAR physical address must be page aligned",
    "a DMA transfer cannot be dispatched without a mapped BAR",
    "the block count must be within 1..=8192",
];

/// Every construct that states a rule by aborting rather than by returning.
///
/// The list is the sweep's whole reach: a construct that is not on it is not
/// checked. `unreachable!(` and `assert_ne!(` are on it because the M17
/// verification planted a reachable `unreachable!` in library code and this
/// sweep, the workspace clippy set and the whole suite all passed over it.
/// The lint half of that hole is closed too -- the workspace now denies
/// `clippy::unreachable` and `clippy::exit` as well as `panic`, `todo`,
/// `unimplemented`, `unwrap_used` and `expect_used` -- so this list is the
/// second reading, not the only one.
const ABORTING: [&str; 7] = [
    "assert!(",
    "assert_eq!(",
    "assert_ne!(",
    "panic!(",
    "unreachable!(",
    ".unwrap()",
    ".expect(",
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

/// Positive: every construct on the watched list is one the sweep reports.
///
/// A list is a gate only if each entry is reachable. This is what was missing
/// when `unreachable!(` was absent from it: nothing failed, because nothing
/// asked whether the list covered what it claimed to cover.
#[test]
fn every_watched_construct_is_one_the_sweep_reports() {
    for needle in ABORTING {
        let planted = format!("    let value = {needle}\"zzqqx\");");
        assert_eq!(
            common::scan_text("planted.rs", &planted, &ABORTING),
            vec![format!("planted.rs:1: {needle}")],
            "the sweep does not report a planted {needle}"
        );
    }
}

// --- Negative -------------------------------------------------------------

/// Negative: the crate's own sources carry none of the aborting constructs.
///
/// The workspace denies `panic`, `unreachable`, `exit`, `unwrap_used`,
/// `expect_used`, `todo` and `unimplemented` as lints, so most of [`ABORTING`]
/// would already fail the clippy gate. The sweep is here as well because a
/// lint can be allowed locally with an attribute, and `assert!`, `assert_eq!`
/// and `assert_ne!` are covered by no clippy lint this workspace can deny
/// without also denying them in every `#[test]` that returns a `Result`.
#[test]
fn no_source_file_states_a_rule_as_an_abort() -> Fallible {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let found = common::scan_sources(&root, &ABORTING)?;
    assert!(
        found.is_empty(),
        "aborting constructs in library code: {found:?}"
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

/// Boundary: a comment line is skipped whole, and a reported line number is
/// one-based, so a hit names the line a reader would open.
#[test]
fn the_sweep_skips_comments_and_numbers_lines_from_one() {
    let body = "//! this file names unreachable!( and assert!( in prose\n\
                fn ok() {}\n\
                let value = source.unwrap();\n";
    assert_eq!(
        common::scan_text("mixed.rs", body, &ABORTING),
        vec!["mixed.rs:3: .unwrap()".to_string()]
    );
}
