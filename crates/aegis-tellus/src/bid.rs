// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The energy bidding contract (REQ-P13-06).
//!
//! REQ-P13-06 records that modules bid against global energy prices based on
//! their expected task-value improvement, written `Delta V` in the source. The
//! requirement states the shape of the contract and nothing numeric: there is
//! no price feed, no currency, no auction period and no clearing rule anywhere
//! in the imported material.
//!
//! So this module types exactly what the requirement states and no more. A
//! [`Bid`] carries an expected improvement and the price it is willing to pay
//! per unit of energy; [`Bid::clears`] is the comparison against an offered
//! price. The bid is won on a strict comparison, in the same direction as the
//! defer threshold in [`crate::sci`], so that a bid exactly at the asking
//! price does not clear.
//!
//! What is deliberately absent: any claim about what a unit of `Delta V` is
//! worth. `docs/roadmap/requirements.md` lists the SCI functional-unit
//! definitions among the recorded unknowns, and a bid is priced per functional
//! unit, so putting a number here would be an invention rather than a port.

use crate::error::TellusError;
use crate::sci::{MAX_ENERGY_KWH, SciCalculation};

/// Scalar upper bound on an admissible expected improvement.
pub const MAX_DELTA_V: f64 = 1.0e6;

/// Scalar upper bound on an admissible energy price, per kWh.
pub const MAX_ENERGY_PRICE: f64 = 1.0e6;

/// An expected task-value improvement, the `Delta V` REQ-P13-06 names.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct DeltaV(f64);

impl DeltaV {
    /// No expected improvement.
    pub const ZERO: Self = Self(0.0);

    /// Builds an expected improvement.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Energy`] for a non-finite, negative or
    /// over-bound value; the bidding quantities share the energy refusal
    /// because they are priced in it.
    pub fn new(value: f64) -> Result<Self, TellusError> {
        if !value.is_finite() || value < 0.0 || value > MAX_DELTA_V {
            return Err(TellusError::Energy {
                reason: "an expected improvement outside the admitted range",
            });
        }
        Ok(Self(value))
    }

    /// Returns the expected improvement.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A price per kilowatt-hour, in the unit the caller keeps its prices in.
///
/// The unit is deliberately unnamed: no imported source states one.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct EnergyPrice(f64);

impl EnergyPrice {
    /// Free energy.
    pub const ZERO: Self = Self(0.0);

    /// Builds a price.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Energy`] for a non-finite, negative or
    /// over-bound price.
    pub fn new(value: f64) -> Result<Self, TellusError> {
        if !value.is_finite() || value < 0.0 || value > MAX_ENERGY_PRICE {
            return Err(TellusError::Energy {
                reason: "an energy price outside the admitted range",
            });
        }
        Ok(Self(value))
    }

    /// Returns the price.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// One module's bid for energy.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Bid {
    delta_v: DeltaV,
    offer: EnergyPrice,
}

impl Bid {
    /// Builds a bid.
    #[must_use]
    pub const fn new(delta_v: DeltaV, offer: EnergyPrice) -> Self {
        Self { delta_v, offer }
    }

    /// Returns the expected improvement.
    #[must_use]
    pub const fn delta_v(&self) -> DeltaV {
        self.delta_v
    }

    /// Returns the price the bid offers.
    #[must_use]
    pub const fn offer(&self) -> EnergyPrice {
        self.offer
    }

    /// Returns `true` when the bid clears `asking`.
    ///
    /// A bid with no expected improvement never clears, whatever it offers: an
    /// improvement of zero is work worth nothing, and paying any price for it
    /// is the failure this contract exists to prevent. Otherwise the
    /// comparison is strict, so a bid exactly at the asking price does not
    /// clear.
    #[must_use]
    pub fn clears(&self, asking: EnergyPrice) -> bool {
        self.delta_v.get() > 0.0 && self.offer.get() > asking.get()
    }

    /// Returns the carbon a cleared bid commits to, per functional unit.
    ///
    /// This is the bridge REQ-P13-06 and REQ-P13-01 share: a module bids
    /// against a price, and what it spends is the SCI rate of the work it
    /// bought. The value is the rate itself, restated here so that the bidding
    /// contract has one recorded consequence rather than none.
    #[must_use]
    pub fn committed_carbon(sci: &SciCalculation) -> f64 {
        sci.rate()
    }

    /// Returns the scalar bound the bidding quantities share, in kWh.
    #[must_use]
    pub const fn energy_bound() -> f64 {
        MAX_ENERGY_KWH
    }
}
