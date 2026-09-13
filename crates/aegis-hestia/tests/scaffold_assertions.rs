// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Exit criterion 2: the scaffold's assertions are refusals, not panics.
//!
//! The imported P15 scaffold (export-030 `689d175667d6`) states three rules as
//! `assert!` calls. This file is the enumeration of those three, each shown
//! returning a value rather than aborting, and each named against the
//! requirement it comes from.
//!
//! The enumeration is the point: it is a fixed list of the three assertions
//! that existed, so removing one from the library without removing it here
//! fails the gate. It is a regression gate over that named list, not a proof
//! that the crate can never panic anywhere.

mod common;

use aegis_hestia::{DmaBufFd, HestiaError, QueryLimit};

use common::{Fallible, embedding, store};

/// The three scaffold assertions, by the rule each states.
const SCAFFOLD_ASSERTIONS: [&str; 3] = [
    "the vector store must be initialised before it is queried",
    "the query limit must be within 1..=100",
    "the DMA-BUF file descriptor must not be negative",
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
#[test]
fn every_scaffold_assertion_is_now_a_result() -> Fallible {
    assert_eq!(SCAFFOLD_ASSERTIONS.len(), 3);

    let uninitialised = store()?;
    assert!(
        uninitialised
            .query(&embedding(), QueryLimit::new(5)?)
            .is_err()
    );

    let out_of_range: Result<QueryLimit, HestiaError> = QueryLimit::new(101);
    assert!(out_of_range.is_err());

    let negative: Result<DmaBufFd, HestiaError> = DmaBufFd::new(-1);
    assert!(negative.is_err());
    Ok(())
}

/// Positive: each refusal is a distinct, matchable variant.
#[test]
fn each_refusal_is_a_distinct_variant() -> Fallible {
    let refusals = [
        store()?.query(&embedding(), QueryLimit::new(5)?).err(),
        QueryLimit::new(0).err(),
        DmaBufFd::new(-1).err(),
    ];
    assert_eq!(
        refusals,
        [
            Some(HestiaError::NotInitialised),
            Some(HestiaError::QueryLimitOutOfRange { limit: 0 }),
            Some(HestiaError::InvalidDmaBufFd { fd: -1 }),
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
    let mut store = store()?;
    let before = store;
    assert!(store.query(&embedding(), QueryLimit::MIN).is_err());
    assert_eq!(store, before);

    store.initialize()?;
    let ready = store;
    assert!(store.initialize().is_err());
    assert_eq!(store, ready);
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
