// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Why a P13 value or a P13 call was refused.
//!
//! Every refusal is a value, never a panic. The imported P13 scaffold
//! (export-036 `25813d240733`) has no refusal at all: `calculate_sci_rate`
//! takes two bare `f64` arguments and returns a bare struct, so a non-finite
//! input produces a non-finite rate and nothing says so. Here every input
//! reaches the arithmetic through a validating constructor, and the two
//! quantities the recorded acceptance asks to be tolerated rather than refused
//! -- a non-positive functional-unit count, and a zone the wattage source does
//! not carry -- are a documented fallback and an explicit error respectively.

use crate::sci::{
    MAX_EMBODIED_CARBON_G, MAX_ENERGY_KWH, MAX_GRID_INTENSITY_G_PER_KWH, MAX_SECONDS,
};

/// Reasons a P13 value or call is refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TellusError {
    /// An energy quantity is negative, non-finite, or past the recorded bound.
    #[error("an energy of {reason} is not an admissible quantity of kWh (bound {MAX_ENERGY_KWH})")]
    Energy {
        /// Why the quantity was refused.
        reason: &'static str,
    },
    /// A grid carbon intensity is negative, non-finite, or past the bound.
    #[error(
        "a grid carbon intensity of {reason} is not admissible \
         (bound {MAX_GRID_INTENSITY_G_PER_KWH} gCO2eq/kWh)"
    )]
    GridIntensity {
        /// Why the quantity was refused.
        reason: &'static str,
    },
    /// An embodied-carbon quantity is negative, non-finite, or past the bound.
    #[error("an embodied carbon of {reason} is not admissible (bound {MAX_EMBODIED_CARBON_G} g)")]
    EmbodiedCarbon {
        /// Why the quantity was refused.
        reason: &'static str,
    },
    /// A wattage is negative, non-finite, or past the recorded bound.
    #[error("a wattage of {reason} is not an admissible draw")]
    Wattage {
        /// Why the quantity was refused.
        reason: &'static str,
    },
    /// An interval is negative, non-finite, zero, or past the recorded bound.
    #[error("an interval of {reason} is not admissible (bound {MAX_SECONDS} s)")]
    Interval {
        /// Why the quantity was refused.
        reason: &'static str,
    },
    /// The wattage source does not carry the requested zone.
    ///
    /// This is decision D60 made structural. The reference profile exposes
    /// `package-0` and `core` and nothing else, so a request for a `dram` or
    /// `psys` zone has no counter behind it. It is refused here rather than
    /// answered with zero, because a silent zero reads downstream as "that
    /// domain drew no power" instead of "that domain does not exist".
    #[error("the wattage source does not carry the {zone} zone; it carries {carried}")]
    ZoneAbsent {
        /// The zone that was asked for.
        zone: &'static str,
        /// How many zones the source does carry.
        carried: usize,
    },
    /// The zone list is full, or names the same zone twice.
    #[error("{reason}")]
    ZoneList {
        /// Why the zone could not be added.
        reason: &'static str,
    },
    /// The slice table is full at its scalar bound.
    #[error("the slice table is full at its bound of {bound} cgroup slices")]
    SliceTableFull {
        /// The scalar bound the table holds.
        bound: usize,
    },
    /// The offered deadline is shorter than the source's declared service time.
    #[error("the wattage source would block past the {offered}ms deadline; it needs {needed}ms")]
    WouldBlock {
        /// The service time the source declares, in milliseconds.
        needed: u32,
        /// The deadline the caller offered, in milliseconds.
        offered: u32,
    },
    /// An identifier is empty, over-long, or outside the permitted charset.
    #[error("{reason}")]
    Identifier {
        /// Why the identifier was refused.
        reason: &'static str,
    },
}
