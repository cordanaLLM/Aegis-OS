// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! REQ-P13-06, the energy bidding contract.
//!
//! The requirement states that modules bid against global energy prices based
//! on their expected task-value improvement. It states no price, no currency
//! and no clearing period, so what is checked here is the comparison and the
//! bounds, and nothing about what a unit of improvement is worth.

mod common;

use aegis_tellus::{
    Bid, DeltaV, EnergyKwh, EnergyPrice, MAX_DELTA_V, MAX_ENERGY_KWH, MAX_ENERGY_PRICE, SciEngine,
};

use common::{EPSILON, Fallible, SCAFFOLD_ENERGY_KWH, SCAFFOLD_SCI_RATE};

// --- Positive -------------------------------------------------------------

/// Positive: a bid above the asking price, with an improvement to show, clears.
#[test]
fn a_bid_above_the_asking_price_clears() -> Fallible {
    let bid = Bid::new(DeltaV::new(2.5)?, EnergyPrice::new(0.40)?);
    assert!(bid.clears(EnergyPrice::new(0.30)?));
    assert!((bid.delta_v().get() - 2.5).abs() < EPSILON);
    assert!((bid.offer().get() - 0.40).abs() < EPSILON);
    assert!(bid.clears(EnergyPrice::ZERO));
    Ok(())
}

/// Positive: what a cleared bid commits to is the SCI rate of the work bought.
#[test]
fn a_cleared_bid_commits_to_the_rate_of_the_work_it_buys() -> Fallible {
    let sci = SciEngine::new().sci_rate(EnergyKwh::new(SCAFFOLD_ENERGY_KWH)?, 1.0);
    let committed = Bid::committed_carbon(&sci);
    assert!((committed - SCAFFOLD_SCI_RATE).abs() < EPSILON);
    assert!((Bid::energy_bound() - MAX_ENERGY_KWH).abs() < EPSILON);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a bid below the asking price does not clear.
#[test]
fn a_bid_below_the_asking_price_does_not_clear() -> Fallible {
    let bid = Bid::new(DeltaV::new(2.5)?, EnergyPrice::new(0.20)?);
    assert!(!bid.clears(EnergyPrice::new(0.30)?));
    Ok(())
}

/// Negative: a bid with no expected improvement never clears, at any price.
///
/// Work worth nothing is not worth buying, however cheap the energy is, so the
/// improvement is checked before the price rather than only alongside it.
#[test]
fn a_bid_with_no_improvement_never_clears() -> Fallible {
    let free = Bid::new(DeltaV::ZERO, EnergyPrice::new(MAX_ENERGY_PRICE)?);
    assert!(!free.clears(EnergyPrice::ZERO));
    assert!(!free.clears(EnergyPrice::new(0.01)?));
    assert!((DeltaV::ZERO.get() - 0.0).abs() < EPSILON);
    Ok(())
}

/// Negative: an inadmissible bidding quantity never becomes a value.
#[test]
fn an_inadmissible_bidding_quantity_is_refused() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        assert!(
            DeltaV::new(bad).is_err(),
            "an improvement of {bad} was taken"
        );
        assert!(EnergyPrice::new(bad).is_err(), "a price of {bad} was taken");
    }
    assert!(DeltaV::new(MAX_DELTA_V * 2.0).is_err());
    assert!(EnergyPrice::new(MAX_ENERGY_PRICE * 2.0).is_err());
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a bid exactly at the asking price does not clear.
///
/// The comparison is strict, in the same direction as the defer threshold, so
/// the two thresholds in this crate turn the same way.
#[test]
fn a_bid_exactly_at_the_asking_price_does_not_clear() -> Fallible {
    let asking = EnergyPrice::new(0.30)?;
    let level = Bid::new(DeltaV::new(1.0)?, EnergyPrice::new(0.30)?);
    assert!(
        !level.clears(asking),
        "exactly the asking price is not above it"
    );

    let over = Bid::new(DeltaV::new(1.0)?, EnergyPrice::new(0.300_000_000_000_01)?);
    assert!(over.clears(asking));
    Ok(())
}

/// Boundary: the recorded maxima are admissible and one step past them is not.
#[test]
fn the_recorded_maxima_are_admissible() -> Fallible {
    assert!(DeltaV::new(MAX_DELTA_V).is_ok());
    assert!(EnergyPrice::new(MAX_ENERGY_PRICE).is_ok());
    assert!(DeltaV::new(MAX_DELTA_V + 1.0).is_err());
    assert!(EnergyPrice::new(MAX_ENERGY_PRICE + 1.0).is_err());

    let richest = Bid::new(
        DeltaV::new(MAX_DELTA_V)?,
        EnergyPrice::new(MAX_ENERGY_PRICE)?,
    );
    assert!(!richest.clears(EnergyPrice::new(MAX_ENERGY_PRICE)?));
    Ok(())
}
