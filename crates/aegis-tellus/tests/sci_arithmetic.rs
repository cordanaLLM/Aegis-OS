// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-1, the SCI formula: the hand-computed value, the fallback that must not
//! produce a NaN, and the bound the result can never leave.

mod common;

use aegis_tellus::{
    DEFAULT_EMBODIED_CARBON_G, DEFAULT_GRID_INTENSITY_G_PER_KWH, EmbodiedCarbon, EnergyKwh,
    FALLBACK_FUNCTIONAL_UNITS, FunctionalUnits, GridIntensity, JOULES_PER_KWH,
    MAX_EMBODIED_CARBON_G, MAX_ENERGY_KWH, MAX_GRID_INTENSITY_G_PER_KWH, MAX_NUMERATOR_G,
    MAX_SCI_RATE, MAX_SECONDS, MIN_FUNCTIONAL_UNITS, SciCalculation, SciEngine, SciRate, Seconds,
    Watts,
};

use common::{EPSILON, Fallible, SCAFFOLD_ENERGY_KWH, SCAFFOLD_SCI_RATE};

/// Every functional-unit count that must reach the fallback.
///
/// This is an enumeration of nine recorded inputs, not a survey of every `f64`
/// that should fall back. What it catches is a change to the admission rule
/// that stops handling one of these nine.
const HOSTILE_UNITS: [f64; 9] = [
    0.0,
    -0.0,
    -1.0,
    f64::NAN,
    f64::INFINITY,
    f64::NEG_INFINITY,
    f64::MIN_POSITIVE,
    5.0e-324,
    -f64::MAX,
];

// --- Positive -------------------------------------------------------------

/// Positive: the hand-computed SCI rate matches.
///
/// The imported scaffold benchmarks `calculate_sci_rate(0.0025, 1.0)` against
/// its default intensity of 220.0 gCO2eq/kWh and embodied carbon of 0.05
/// gCO2eq. `((0.0025 * 220.0) + 0.05) / 1.0` is `0.55 + 0.05`, which is 0.6.
#[test]
fn the_hand_computed_sci_rate_matches() -> Fallible {
    let engine = SciEngine::new();
    let sci = engine.sci_rate(EnergyKwh::new(SCAFFOLD_ENERGY_KWH)?, 1.0);
    assert!(
        (sci.rate() - SCAFFOLD_SCI_RATE).abs() < EPSILON,
        "the engine computed {}, not the hand-computed {SCAFFOLD_SCI_RATE}",
        sci.rate()
    );
    assert!(!sci.fell_back());
    assert!(
        (sci.units().get() - 1.0).abs() < EPSILON,
        "the offered functional-unit count must be the one used"
    );
    Ok(())
}

/// Positive: every input the calculation reports is the one it was given.
#[test]
fn the_calculation_reports_the_inputs_it_used() -> Fallible {
    let intensity = GridIntensity::new(410.0)?;
    let embodied = EmbodiedCarbon::new(0.25)?;
    let engine = SciEngine::with_inputs(intensity, embodied);
    let energy = EnergyKwh::new(0.5)?;
    let sci = engine.sci_rate(energy, 4.0);

    assert_eq!(sci.energy(), energy);
    assert_eq!(sci.intensity(), intensity);
    assert_eq!(sci.embodied(), embodied);
    assert_eq!(engine.intensity(), intensity);
    assert_eq!(engine.embodied(), embodied);

    let expected = (0.5 * 410.0 + 0.25) / 4.0;
    assert!((sci.rate() - expected).abs() < EPSILON);
    Ok(())
}

/// Positive: the default engine holds the two recorded scaffold constants.
#[test]
fn the_default_engine_holds_the_recorded_constants() {
    let engine = SciEngine::default();
    assert!(
        (engine.intensity().get() - DEFAULT_GRID_INTENSITY_G_PER_KWH).abs() < EPSILON,
        "the default intensity must be the scaffold's 220.0"
    );
    assert!(
        (engine.embodied().get() - DEFAULT_EMBODIED_CARBON_G).abs() < EPSILON,
        "the default embodied carbon must be the scaffold's 0.05"
    );
    assert_eq!(GridIntensity::DEFAULT, engine.intensity());
    assert_eq!(EmbodiedCarbon::DEFAULT, engine.embodied());
}

/// Positive: a draw held over an interval converts to energy.
///
/// This is the seam-to-arithmetic bridge: 26.4 watts for one hour is
/// `26.4 * 3600 / 3_600_000` kWh, which is 0.0264 kWh.
#[test]
fn a_draw_over_an_interval_converts_to_energy() -> Fallible {
    let energy = EnergyKwh::from_draw(Watts::new(26.4)?, Seconds::new(3600.0)?)?;
    assert!((energy.get() - 0.0264).abs() < EPSILON);
    assert!((JOULES_PER_KWH - 3_600_000.0).abs() < EPSILON);

    let one_second = EnergyKwh::from_draw(Watts::new(3600.0)?, Seconds::ONE)?;
    assert!((one_second.get() - 0.001).abs() < EPSILON);
    assert_eq!(
        EnergyKwh::from_draw(Watts::ZERO, Seconds::ONE)?,
        EnergyKwh::ZERO
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a non-positive functional-unit count falls back, without a NaN.
///
/// This is the acceptance the roadmap states, and the NaN half is the point:
/// a fallback that produced a NaN would still be a fallback and would still
/// be reported as one, so `fell_back()` alone is not the check.
#[test]
fn a_non_positive_functional_unit_count_falls_back_without_a_nan() -> Fallible {
    let engine = SciEngine::new();
    let energy = EnergyKwh::new(SCAFFOLD_ENERGY_KWH)?;
    for offered in HOSTILE_UNITS {
        let sci = engine.sci_rate(energy, offered);
        assert!(
            sci.fell_back(),
            "a functional-unit count of {offered} must fall back"
        );
        assert!(
            !sci.rate().is_nan(),
            "a fallback produced a NaN for {offered}"
        );
        assert!(
            sci.rate().is_finite(),
            "a fallback produced a non-finite rate for {offered}"
        );
        assert!(
            (sci.units().get() - FALLBACK_FUNCTIONAL_UNITS).abs() < EPSILON,
            "the fallback divisor must be {FALLBACK_FUNCTIONAL_UNITS}"
        );
        assert!(
            (sci.rate() - SCAFFOLD_SCI_RATE).abs() < EPSILON,
            "falling back to 1.0 must give the same rate as offering 1.0"
        );
        sci.as_rate()?;
    }
    Ok(())
}

/// A named constructor, as a call that reports whether it refused a value.
type Constructor = (&'static str, fn(f64) -> bool);

/// Every constructor, as a name and a fallible call over one `f64`.
///
/// The list is what makes the two refusal tests below a sweep over the six
/// quantities rather than six copies of the same three lines.
const CONSTRUCTORS: [Constructor; 6] = [
    ("energy", |value| EnergyKwh::new(value).is_err()),
    ("intensity", |value| GridIntensity::new(value).is_err()),
    ("embodied carbon", |value| {
        EmbodiedCarbon::new(value).is_err()
    }),
    ("wattage", |value| Watts::new(value).is_err()),
    ("interval", |value| Seconds::new(value).is_err()),
    ("rate", |value| SciRate::new(value).is_err()),
];

/// Negative: no constructor admits a non-finite or negative quantity.
#[test]
fn a_non_finite_or_negative_quantity_is_refused() {
    for bad in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY, -1.0] {
        for (label, refuses) in CONSTRUCTORS {
            assert!(refuses(bad), "the {label} constructor accepted {bad}");
        }
    }
}

/// Negative: no constructor admits a quantity past its own recorded bound.
#[test]
fn a_quantity_past_its_bound_is_refused() {
    assert!(EnergyKwh::new(MAX_ENERGY_KWH * 2.0).is_err());
    assert!(GridIntensity::new(MAX_GRID_INTENSITY_G_PER_KWH + 1.0).is_err());
    assert!(EmbodiedCarbon::new(MAX_EMBODIED_CARBON_G + 1.0).is_err());
    assert!(Watts::new(Watts::MAX + 1.0).is_err());
    assert!(Seconds::new(MAX_SECONDS + 1.0).is_err());
    assert!(
        Seconds::new(0.0).is_err(),
        "a zero interval is not positive"
    );
    assert!(SciRate::new(MAX_SCI_RATE * 2.0).is_err());
}

/// Negative: an inadmissible draw or interval is refused before any energy is
/// produced, so `from_draw` never sees a quantity its own bound would have to
/// catch.
#[test]
fn from_draw_cannot_be_reached_with_an_inadmissible_quantity() {
    for bad in [f64::NAN, f64::INFINITY, -1.0, Watts::MAX * 2.0] {
        assert!(Watts::new(bad).is_err(), "a draw of {bad} must be refused");
    }
    for bad in [f64::NAN, f64::INFINITY, 0.0, -1.0, MAX_SECONDS * 2.0] {
        assert!(
            Seconds::new(bad).is_err(),
            "an interval of {bad} must be refused"
        );
    }
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the admission rule turns exactly at [`MIN_FUNCTIONAL_UNITS`].
#[test]
fn the_functional_unit_admission_turns_at_its_floor() {
    let at_floor = FunctionalUnits::admit(MIN_FUNCTIONAL_UNITS);
    assert!(!at_floor.fell_back(), "exactly the floor must be admitted");
    assert!((at_floor.get() - MIN_FUNCTIONAL_UNITS).abs() < EPSILON);

    let below = FunctionalUnits::admit(MIN_FUNCTIONAL_UNITS / 2.0);
    assert!(below.fell_back(), "below the floor must fall back");
    assert!((below.get() - FALLBACK_FUNCTIONAL_UNITS).abs() < EPSILON);

    let above = FunctionalUnits::admit(1.0e12);
    assert!(!above.fell_back());
    assert!((above.get() - 1.0e12).abs() < 1.0);
}

/// Boundary: the extremes of the admitted domain stay inside the rate bound.
///
/// This is the falsifier for the finiteness claim in `sci.rs`. The largest
/// numerator the constructors can produce is [`MAX_NUMERATOR_G`] and the
/// smallest divisor is [`MIN_FUNCTIONAL_UNITS`], so the largest rate is
/// [`MAX_SCI_RATE`]; a change that widened any bound past the point where the
/// division overflows makes `as_rate` fail here.
#[test]
fn the_extremes_of_the_domain_stay_inside_the_rate_bound() -> Fallible {
    let engine = SciEngine::with_inputs(
        GridIntensity::new(MAX_GRID_INTENSITY_G_PER_KWH)?,
        EmbodiedCarbon::new(MAX_EMBODIED_CARBON_G)?,
    );
    let energy = EnergyKwh::new(MAX_ENERGY_KWH)?;
    let worst = engine.sci_rate(energy, MIN_FUNCTIONAL_UNITS);
    assert!(worst.rate().is_finite(), "the worst case must be finite");
    assert!(
        worst.rate() <= MAX_SCI_RATE,
        "the worst case {} must not exceed the recorded bound {MAX_SCI_RATE}",
        worst.rate()
    );
    worst.as_rate()?;

    let numerator = MAX_ENERGY_KWH * MAX_GRID_INTENSITY_G_PER_KWH + MAX_EMBODIED_CARBON_G;
    assert!((MAX_NUMERATOR_G - numerator).abs() < EPSILON);
    assert!(MAX_SCI_RATE.is_finite());

    for offered in HOSTILE_UNITS {
        assert!(engine.sci_rate(energy, offered).rate().is_finite());
    }
    Ok(())
}

/// Boundary: the seam's own maxima cannot overflow the energy bound.
///
/// This is why `EnergyKwh::from_draw` has no reachable refusal of its own:
/// the largest admissible draw held for the longest admissible interval is
/// 2400 kWh, far inside [`MAX_ENERGY_KWH`]. The test records the figure rather
/// than the reasoning, so widening either bound past the point where the
/// product leaves the energy range fails here.
#[test]
fn the_seam_maxima_cannot_overflow_the_energy_bound() -> Fallible {
    let energy = EnergyKwh::from_draw(Watts::new(Watts::MAX)?, Seconds::new(MAX_SECONDS)?)?;
    assert!(
        (energy.get() - 2400.0).abs() < EPSILON,
        "the worst-case conversion is {} kWh, not the recorded 2400",
        energy.get()
    );
    assert!(energy.get() <= MAX_ENERGY_KWH);
    Ok(())
}

/// Boundary: zero energy still carries the embodied term, and zero everything
/// is a rate of zero rather than a refusal.
#[test]
fn the_zero_cases_are_rates_and_not_refusals() -> Fallible {
    let engine = SciEngine::new();
    let none = engine.sci_rate(EnergyKwh::ZERO, 1.0);
    assert!(
        (none.rate() - DEFAULT_EMBODIED_CARBON_G).abs() < EPSILON,
        "zero energy still amortises the embodied carbon"
    );

    let bare = SciEngine::with_inputs(GridIntensity::new(0.0)?, EmbodiedCarbon::new(0.0)?);
    let zero = bare.sci_rate(EnergyKwh::ZERO, 1.0);
    assert!((zero.rate() - 0.0).abs() < EPSILON);
    assert!(zero.as_rate()?.get().abs() < EPSILON);
    Ok(())
}

/// Boundary: a calculation compares equal to itself and orders by its fields.
///
/// [`SciCalculation`] derives `PartialOrd` over floating-point fields, so this
/// states what that ordering means rather than leaving it to be discovered.
#[test]
fn a_calculation_compares_with_itself() -> Fallible {
    let engine = SciEngine::new();
    let energy = EnergyKwh::new(SCAFFOLD_ENERGY_KWH)?;
    let first: SciCalculation = engine.sci_rate(energy, 1.0);
    let second = engine.sci_rate(energy, 1.0);
    assert_eq!(first, second);
    assert!(first <= second);
    assert!(engine.sci_rate(energy, 2.0) < first);
    Ok(())
}
