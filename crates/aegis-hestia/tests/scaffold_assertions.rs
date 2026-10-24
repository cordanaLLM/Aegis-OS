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

#[macro_use]
mod common;

use aegis_hestia::{DmaBufFd, HestiaError, QueryLimit};

use common::{Fallible, embedding, store};

/// The three scaffold assertions, by the rule each states.
const SCAFFOLD_ASSERTIONS: [&str; 3] = [
    "the vector store must be initialised before it is queried",
    "the query limit must be within 1..=100",
    "the DMA-BUF file descriptor must not be negative",
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

define_scaffold_sweep_tests!();
