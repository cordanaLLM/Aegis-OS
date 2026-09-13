// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E17-2: an initialised store answers queries, a query before
//! initialisation fails, and the limit bound is exact at 0, 1, 100 and 101.
//!
//! REQ-P15-05 is both rules, and the scaffold states both as assertions inside
//! the query call. Here the first is a state check and the second is
//! [`QueryLimit::new`], so both negative cases are ordinary tests.

mod common;

use aegis_hestia::{
    HestiaError, MAX_QUERY_LIMIT, MIN_QUERY_LIMIT, PGLITE_OPFS_BUFFER_MB, QueryLimit, StoreState,
};

use common::{Fallible, embedding, initialised, store};

// --- Positive -------------------------------------------------------------

/// Positive: an initialised store answers a query with its hit count.
#[test]
fn an_initialised_store_answers_queries() -> Fallible {
    let store = initialised()?;
    assert_eq!(store.state(), StoreState::Initialised);
    assert_eq!(store.state().name(), "initialised");
    let hits = store.query(&embedding(), QueryLimit::new(5)?)?;
    assert_eq!(hits.get(), 5);
    Ok(())
}

/// Positive: initialising records the buffer the requirement allocates.
#[test]
fn initialising_records_the_recorded_buffer() -> Fallible {
    let mut store = store()?;
    assert_eq!(store.state(), StoreState::Uninitialised);
    assert_eq!(store.state().name(), "uninitialised");
    assert_eq!(store.buffer_mb(), PGLITE_OPFS_BUFFER_MB);
    store.initialize()?;
    assert_eq!(store.buffer_mb(), 256);
    assert_eq!(store.path().to_string(), common::STORAGE);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a query before initialisation fails, and changes nothing.
#[test]
fn a_query_before_initialisation_fails() -> Fallible {
    let store = store()?;
    assert_eq!(
        store.query(&embedding(), QueryLimit::new(5)?),
        Err(HestiaError::NotInitialised)
    );
    assert_eq!(store.state(), StoreState::Uninitialised);
    Ok(())
}

/// Negative: initialising twice is refused rather than repeated.
#[test]
fn initialising_twice_is_refused() -> Fallible {
    let mut store = initialised()?;
    assert_eq!(store.initialize(), Err(HestiaError::AlreadyInitialised));
    assert_eq!(store.state(), StoreState::Initialised);
    Ok(())
}

/// Negative: an out-of-range limit is refused, and the refusal names it.
#[test]
fn an_out_of_range_limit_is_refused() {
    assert_eq!(
        QueryLimit::new(0),
        Err(HestiaError::QueryLimitOutOfRange { limit: 0 })
    );
    assert_eq!(
        QueryLimit::new(101),
        Err(HestiaError::QueryLimitOutOfRange { limit: 101 })
    );
    assert_eq!(
        QueryLimit::new(usize::MAX),
        Err(HestiaError::QueryLimitOutOfRange { limit: usize::MAX })
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: 0 and 101 fail, and 1 and 100 pass.
#[test]
fn the_query_limit_bound_is_exact_on_both_sides() {
    assert!(QueryLimit::new(0).is_err());
    assert_eq!(QueryLimit::new(1).map(QueryLimit::get), Ok(1));
    assert_eq!(QueryLimit::new(100).map(QueryLimit::get), Ok(100));
    assert!(QueryLimit::new(101).is_err());
    assert_eq!(MIN_QUERY_LIMIT, 1);
    assert_eq!(MAX_QUERY_LIMIT, 100);
    assert_eq!(QueryLimit::MIN.get(), MIN_QUERY_LIMIT);
    assert_eq!(QueryLimit::MAX.get(), MAX_QUERY_LIMIT);
}

/// Boundary: an initialised store answers at both ends of the range.
#[test]
fn an_initialised_store_answers_at_both_ends_of_the_range() -> Fallible {
    let store = initialised()?;
    assert_eq!(store.query(&embedding(), QueryLimit::MIN)?.get(), 1);
    assert_eq!(store.query(&embedding(), QueryLimit::MAX)?.get(), 100);
    Ok(())
}
