// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The ISO/IEC 21031:2024 SCI rate, and the quantities it is computed from.
//!
//! REQ-P13-01 states the formula the daemon's core carbon-calculation method
//! implements:
//!
//! ```text
//! SCI = ((E * I) + M) / R
//! ```
//!
//! `E` is energy in kWh, `I` is grid carbon intensity in gCO2eq/kWh, `M` is
//! amortised embodied carbon in gCO2eq, and `R` is the functional units the
//! rate is expressed per. [`SciEngine::sci_rate`] is that expression and
//! nothing else; every input reaches it through a validating constructor, and
//! the constructors are what make the result's finiteness a property of the
//! admitted domain rather than a hope.
//!
//! # The fallback, and why it cannot produce a NaN
//!
//! The imported scaffold writes `if functional_units > 0.0 { functional_units }
//! else { 1.0 }`, which already sends a NaN to the fallback branch, because
//! every comparison against a NaN is false. It does not, however, send an
//! infinity there, and it bounds nothing: a sufficiently small positive `R`
//! makes the division overflow to an infinity, and an unbounded `E` and `I`
//! make the product overflow before the division is reached.
//!
//! [`FunctionalUnits::admit`] closes both. `R` is taken only when it is finite
//! and at least [`MIN_FUNCTIONAL_UNITS`]; anything else -- zero, a negative
//! value, a subnormal, an infinity, a NaN -- becomes
//! [`FALLBACK_FUNCTIONAL_UNITS`]. With `E`, `I` and `M` bounded by their own
//! constructors, the largest numerator the engine can be handed is
//! [`MAX_NUMERATOR_G`] and the largest rate is [`MAX_SCI_RATE`], both finite.
//! `tests/sci_arithmetic.rs` holds that against a recorded list of hostile
//! inputs, so the claim is falsifiable rather than asserted.

use crate::error::TellusError;

/// The default grid carbon intensity, in gCO2eq/kWh.
///
/// 220.0, the value the imported P13 scaffold (export-036 `25813d240733`)
/// constructs its engine with. It is an offline default, not a measurement: no
/// regional API is called anywhere in this crate.
pub const DEFAULT_GRID_INTENSITY_G_PER_KWH: f64 = 220.0;

/// The default amortised embodied carbon `M`, in gCO2eq.
///
/// 0.05, the scaffold's `DEFAULT_EMBODIED_CARBON_G`.
pub const DEFAULT_EMBODIED_CARBON_G: f64 = 0.05;

/// The grid carbon intensity above which background work is deferred.
///
/// 300.0 gCO2eq/kWh, from the scaffold's
/// `should_defer_spatiotemporal_tasks`. The comparison is strict, so exactly
/// the threshold is not a deferral; see [`SciEngine::should_defer`].
pub const DEFER_THRESHOLD_G_PER_KWH: f64 = 300.0;

/// The functional-unit count substituted when the offered one is inadmissible.
pub const FALLBACK_FUNCTIONAL_UNITS: f64 = 1.0;

/// The smallest functional-unit count the engine divides by.
///
/// Below this the division could overflow to an infinity, so a smaller count
/// takes [`FALLBACK_FUNCTIONAL_UNITS`] instead.
pub const MIN_FUNCTIONAL_UNITS: f64 = 1.0e-6;

/// Scalar upper bound on an admissible energy quantity, in kWh.
pub const MAX_ENERGY_KWH: f64 = 1.0e6;

/// Scalar upper bound on an admissible grid carbon intensity, in gCO2eq/kWh.
///
/// 2000.0. The dirtiest grids reported by the intensity sources the roadmap
/// names sit near 1000 gCO2eq/kWh, so this is roughly double the worst real
/// value: high enough not to refuse a legitimate reading, low enough to keep
/// the product below the overflow bound.
pub const MAX_GRID_INTENSITY_G_PER_KWH: f64 = 2_000.0;

/// Scalar upper bound on an admissible embodied-carbon quantity, in gCO2eq.
pub const MAX_EMBODIED_CARBON_G: f64 = 1.0e3;

/// The largest numerator `(E * I) + M` the admitted domain can produce.
pub const MAX_NUMERATOR_G: f64 =
    MAX_ENERGY_KWH * MAX_GRID_INTENSITY_G_PER_KWH + MAX_EMBODIED_CARBON_G;

/// The largest SCI rate the admitted domain can produce.
///
/// Finite, and far below `f64::MAX`. This is the bound
/// `tests/sci_arithmetic.rs` checks the engine against.
pub const MAX_SCI_RATE: f64 = MAX_NUMERATOR_G / MIN_FUNCTIONAL_UNITS;

/// Scalar upper bound on an admissible sampling interval, in seconds.
pub const MAX_SECONDS: f64 = 86_400.0;

/// Watt-seconds in one kilowatt-hour.
pub const JOULES_PER_KWH: f64 = 3_600_000.0;

/// A validated energy quantity in kilowatt-hours.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct EnergyKwh(f64);

impl EnergyKwh {
    /// Zero energy.
    pub const ZERO: Self = Self(0.0);

    /// Builds an energy quantity.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Energy`] for a non-finite, negative, or
    /// over-bound quantity.
    pub fn new(kwh: f64) -> Result<Self, TellusError> {
        if !kwh.is_finite() {
            return Err(TellusError::Energy {
                reason: "a value that is not finite",
            });
        }
        if kwh < 0.0 {
            return Err(TellusError::Energy {
                reason: "a negative value",
            });
        }
        if kwh > MAX_ENERGY_KWH {
            return Err(TellusError::Energy {
                reason: "a value past the recorded bound",
            });
        }
        Ok(Self(kwh))
    }

    /// Converts a draw held over an interval into energy.
    ///
    /// This is the one place the wattage seam meets the arithmetic: M21
    /// replaces what produces the [`Watts`] and this conversion, the formula
    /// and every bound above stay exactly as they are.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Energy`] when the product is past the recorded
    /// bound.
    pub fn from_draw(watts: Watts, interval: Seconds) -> Result<Self, TellusError> {
        Self::new(watts.get() * interval.get() / JOULES_PER_KWH)
    }

    /// Returns the quantity in kilowatt-hours.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A validated grid carbon intensity in gCO2eq/kWh.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct GridIntensity(f64);

impl GridIntensity {
    /// The scaffold's offline default of 220.0 gCO2eq/kWh.
    pub const DEFAULT: Self = Self(DEFAULT_GRID_INTENSITY_G_PER_KWH);

    /// Exactly the defer threshold, 300.0 gCO2eq/kWh.
    pub const AT_THRESHOLD: Self = Self(DEFER_THRESHOLD_G_PER_KWH);

    /// Builds a grid carbon intensity.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::GridIntensity`] for a non-finite, negative, or
    /// over-bound value.
    pub fn new(g_per_kwh: f64) -> Result<Self, TellusError> {
        if !g_per_kwh.is_finite() {
            return Err(TellusError::GridIntensity {
                reason: "a value that is not finite",
            });
        }
        if g_per_kwh < 0.0 {
            return Err(TellusError::GridIntensity {
                reason: "a negative value",
            });
        }
        if g_per_kwh > MAX_GRID_INTENSITY_G_PER_KWH {
            return Err(TellusError::GridIntensity {
                reason: "a value past the recorded bound",
            });
        }
        Ok(Self(g_per_kwh))
    }

    /// Returns the intensity in gCO2eq/kWh.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A validated amortised embodied-carbon quantity in gCO2eq.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct EmbodiedCarbon(f64);

impl EmbodiedCarbon {
    /// The scaffold's default of 0.05 gCO2eq.
    pub const DEFAULT: Self = Self(DEFAULT_EMBODIED_CARBON_G);

    /// Builds an embodied-carbon quantity.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::EmbodiedCarbon`] for a non-finite, negative, or
    /// over-bound value.
    pub fn new(grams: f64) -> Result<Self, TellusError> {
        if !grams.is_finite() {
            return Err(TellusError::EmbodiedCarbon {
                reason: "a value that is not finite",
            });
        }
        if grams < 0.0 {
            return Err(TellusError::EmbodiedCarbon {
                reason: "a negative value",
            });
        }
        if grams > MAX_EMBODIED_CARBON_G {
            return Err(TellusError::EmbodiedCarbon {
                reason: "a value past the recorded bound",
            });
        }
        Ok(Self(grams))
    }

    /// Returns the quantity in gCO2eq.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// A validated instantaneous draw in watts.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Watts(f64);

impl Watts {
    /// Scalar upper bound on an admissible draw, in watts.
    pub const MAX: f64 = 100_000.0;

    /// No draw.
    pub const ZERO: Self = Self(0.0);

    /// Builds a draw.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Wattage`] for a non-finite, negative, or
    /// over-bound draw.
    pub fn new(watts: f64) -> Result<Self, TellusError> {
        if !watts.is_finite() {
            return Err(TellusError::Wattage {
                reason: "a value that is not finite",
            });
        }
        if watts < 0.0 {
            return Err(TellusError::Wattage {
                reason: "a negative value",
            });
        }
        if watts > Self::MAX {
            return Err(TellusError::Wattage {
                reason: "a value past the recorded bound",
            });
        }
        Ok(Self(watts))
    }

    /// Returns the draw in watts.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }

    /// Adds two draws, refusing a sum past the bound.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Wattage`] when the sum leaves the admitted
    /// range.
    pub fn checked_add(self, other: Self) -> Result<Self, TellusError> {
        Self::new(self.0 + other.0)
    }
}

/// A validated positive interval in seconds.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct Seconds(f64);

impl Seconds {
    /// One second.
    pub const ONE: Self = Self(1.0);

    /// Builds an interval.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Interval`] for a non-finite, non-positive, or
    /// over-bound interval.
    pub fn new(seconds: f64) -> Result<Self, TellusError> {
        if !seconds.is_finite() {
            return Err(TellusError::Interval {
                reason: "a value that is not finite",
            });
        }
        if seconds <= 0.0 {
            return Err(TellusError::Interval {
                reason: "a value that is not positive",
            });
        }
        if seconds > MAX_SECONDS {
            return Err(TellusError::Interval {
                reason: "a value past the recorded bound",
            });
        }
        Ok(Self(seconds))
    }

    /// Returns the interval in seconds.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

/// The functional units `R` the rate is expressed per.
///
/// This type exists to make the fallback a named, inspectable step rather than
/// an `if` buried in the arithmetic: [`FunctionalUnits::admit`] never fails, it
/// reports whether the offered count was taken or replaced.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct FunctionalUnits {
    units: f64,
    fell_back: bool,
}

impl FunctionalUnits {
    /// Admits `offered`, or substitutes [`FALLBACK_FUNCTIONAL_UNITS`].
    ///
    /// An offered count is taken when it is finite and at least
    /// [`MIN_FUNCTIONAL_UNITS`]. Zero, a negative value, a subnormal, an
    /// infinity and a NaN all fall back, so the divisor is always a finite
    /// value of at least [`MIN_FUNCTIONAL_UNITS`].
    #[must_use]
    pub fn admit(offered: f64) -> Self {
        if offered.is_finite() && offered >= MIN_FUNCTIONAL_UNITS {
            return Self {
                units: offered,
                fell_back: false,
            };
        }
        Self {
            units: FALLBACK_FUNCTIONAL_UNITS,
            fell_back: true,
        }
    }

    /// Returns the divisor actually used.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.units
    }

    /// Returns `true` when the offered count was replaced.
    #[must_use]
    pub const fn fell_back(self) -> bool {
        self.fell_back
    }
}

/// One evaluated SCI rate, with every input it was computed from.
///
/// The scaffold's `SciCalculation` carries the same five numbers. The
/// difference is that each is a validated quantity here, and that
/// [`Self::fell_back`] records whether `R` is the offered count or the
/// substitute, which the scaffold's bare `f64` cannot say.
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SciCalculation {
    rate: f64,
    energy: EnergyKwh,
    intensity: GridIntensity,
    embodied: EmbodiedCarbon,
    units: FunctionalUnits,
}

impl SciCalculation {
    /// Returns the SCI rate, in gCO2eq per functional unit.
    #[must_use]
    pub const fn rate(&self) -> f64 {
        self.rate
    }

    /// Returns `E`.
    #[must_use]
    pub const fn energy(&self) -> EnergyKwh {
        self.energy
    }

    /// Returns `I`.
    #[must_use]
    pub const fn intensity(&self) -> GridIntensity {
        self.intensity
    }

    /// Returns `M`.
    #[must_use]
    pub const fn embodied(&self) -> EmbodiedCarbon {
        self.embodied
    }

    /// Returns `R`, as it was actually used.
    #[must_use]
    pub const fn units(&self) -> FunctionalUnits {
        self.units
    }

    /// Returns `true` when `R` was substituted for an inadmissible offer.
    #[must_use]
    pub const fn fell_back(&self) -> bool {
        self.units.fell_back()
    }
}

/// The SCI rate engine: the formula, the defer threshold, and nothing else.
///
/// It holds the grid intensity and the embodied-carbon constant the scaffold's
/// engine holds. It holds no probe, no socket and no counter; see
/// [`crate::power`] for the seam where a measurement would arrive.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SciEngine {
    intensity: GridIntensity,
    embodied: EmbodiedCarbon,
}

impl Default for SciEngine {
    fn default() -> Self {
        Self::new()
    }
}

impl SciEngine {
    /// Builds the engine with the scaffold's recorded defaults.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            intensity: GridIntensity::DEFAULT,
            embodied: EmbodiedCarbon::DEFAULT,
        }
    }

    /// Builds the engine with an explicit intensity and embodied constant.
    #[must_use]
    pub const fn with_inputs(intensity: GridIntensity, embodied: EmbodiedCarbon) -> Self {
        Self {
            intensity,
            embodied,
        }
    }

    /// Returns the grid carbon intensity the engine holds.
    #[must_use]
    pub const fn intensity(&self) -> GridIntensity {
        self.intensity
    }

    /// Returns the amortised embodied carbon the engine holds.
    #[must_use]
    pub const fn embodied(&self) -> EmbodiedCarbon {
        self.embodied
    }

    /// Evaluates `SCI = ((E * I) + M) / R`.
    ///
    /// `offered_units` is a bare `f64` on purpose: REQ-P13-01's acceptance
    /// asks for a non-positive count to fall back rather than to be refused,
    /// so the tolerated case has to be representable at the call site.
    #[must_use]
    pub fn sci_rate(&self, energy: EnergyKwh, offered_units: f64) -> SciCalculation {
        let units = FunctionalUnits::admit(offered_units);
        let numerator = energy.get() * self.intensity.get() + self.embodied.get();
        SciCalculation {
            rate: numerator / units.get(),
            energy,
            intensity: self.intensity,
            embodied: self.embodied,
            units,
        }
    }

    /// Returns `true` when background work should be shifted in time or space.
    ///
    /// The comparison is strict, exactly as the scaffold writes it: an
    /// intensity of exactly [`DEFER_THRESHOLD_G_PER_KWH`] is not a deferral,
    /// and the next representable value above it is.
    #[must_use]
    pub fn should_defer(&self) -> bool {
        self.intensity.get() > DEFER_THRESHOLD_G_PER_KWH
    }
}

/// A validated SCI rate, in gCO2eq per functional unit.
///
/// The constructor is what binds the finiteness claim above to something a
/// test can falsify: [`SciCalculation::as_rate`] must succeed for every value
/// the engine produces from admitted inputs, and it refuses anything outside
/// `0.0..=`[`MAX_SCI_RATE`].
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct SciRate(f64);

impl SciRate {
    /// The largest admissible rate, [`MAX_SCI_RATE`].
    pub const MAX: f64 = MAX_SCI_RATE;

    /// Builds a rate.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Energy`] for a non-finite, negative or
    /// over-bound rate.
    pub fn new(rate: f64) -> Result<Self, TellusError> {
        if !rate.is_finite() || rate < 0.0 || rate > Self::MAX {
            return Err(TellusError::Energy {
                reason: "an SCI rate outside the admitted range",
            });
        }
        Ok(Self(rate))
    }

    /// Returns the rate.
    #[must_use]
    pub const fn get(self) -> f64 {
        self.0
    }
}

impl SciCalculation {
    /// Returns the rate as a validated quantity.
    ///
    /// # Errors
    ///
    /// Returns [`TellusError::Energy`] when the rate is outside
    /// `0.0..=`[`MAX_SCI_RATE`]. Over the domain the constructors admit that
    /// cannot happen, and `tests/sci_arithmetic.rs` is the falsifier: it drives
    /// this method with the extremes of every input and with a recorded list of
    /// hostile functional-unit counts, and a single `Err` fails the gate.
    pub fn as_rate(&self) -> Result<SciRate, TellusError> {
        SciRate::new(self.rate)
    }
}
