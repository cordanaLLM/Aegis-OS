// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P13 `aegis-tellus`: the SCI rate arithmetic and the wattage seam.
//!
//! The imported P13 scaffold (export-036 `25813d240733`) is a daemon that
//! prints what it would do: it simulates four cgroup slices with hard-coded
//! watts, evaluates the ISO/IEC 21031:2024 SCI rate over them, and sleeps in a
//! bounded loop. Milestone M05 extracts the part that can be checked without a
//! RAPL counter or an eBPF probe -- the arithmetic, the threshold and the
//! bounds -- and puts the simulated draw behind a seam so that M21 can replace
//! it with a measured energy delta without touching any of it.
//!
//! # The four things a reviewer should look at
//!
//! * [`SciEngine::sci_rate`] is REQ-P13-01's formula, `((E * I) + M) / R`, and
//!   nothing else. Every input arrives through a validating constructor, and
//!   [`FunctionalUnits::admit`] is the fallback the recorded acceptance asks
//!   for: a non-positive, subnormal, infinite or NaN count becomes
//!   [`FALLBACK_FUNCTIONAL_UNITS`], which is why a fallback cannot produce a
//!   NaN. [`SciCalculation::as_rate`] is the falsifier for that claim.
//! * [`SciEngine::should_defer`] is the threshold, compared strictly: exactly
//!   [`DEFER_THRESHOLD_G_PER_KWH`] is not a deferral and the next
//!   representable value above it is.
//! * [`WattageSource`] is the seam. [`SimulatedWattage`] is the only
//!   implementation this milestone ships, it carries a recorded constant per
//!   zone, and it takes a [`ZoneList`]: a source that does not carry a zone
//!   refuses a sample of it with [`TellusError::ZoneAbsent`] instead of
//!   answering zero (decision D60).
//! * [`contracts`] carries the two typed edges, and [`register`] records the
//!   P13 claims this crate cannot check -- including the powercap zone
//!   enumeration, which was measured on the reference profile rather than
//!   assumed.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: every function here is a flat check, an arithmetic
//!   expression or a bounded scan, and no function in the crate calls itself
//!   directly or through another;
//! * every loop carries a scalar upper bound: [`MAX_CGROUP_SLICES`],
//!   [`MAX_RAPL_ZONES`], [`MAX_QUERY_CANDIDATES`], [`MAX_IDENTIFIER_LEN`] and
//!   [`MAX_CONTRACT_PAYLOAD_BYTES`];
//! * every I/O-shaped call takes an explicit deadline: [`WattageSource::sample`]
//!   cannot be implemented without receiving a [`SampleDeadline`], which is the
//!   HISS-02 obligation made structural before the reader that needs it exists;
//! * every value on a decision path is `Copy`, so no such path allocates;
//!   `tests/allocation_bounds.rs` is the falsifier, and the one place that is
//!   deliberately not claimed -- a JSON string carrying an escape, which
//!   `serde_json` unescapes into a heap scratch buffer -- is tested rather than
//!   denied;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or unchecked arithmetic,
//!   and no `unsafe` (forbidden at the workspace root).
//!
//! # What this crate does not do
//!
//! It measures nothing. There is no RAPL read, no `/sys/class/powercap`
//! access, no eBPF program compiled loaded or attached, no cgroup read, no
//! D-Bus connection, no thread and no sleep anywhere in it. Every wattage it
//! reports is a recorded constant labelled [`Provenance::Simulated`] or
//! [`Provenance::Modelled`]; nothing it produces carries
//! [`Provenance::Measured`]. `tests/stubbed_effects.rs` sweeps this crate's own
//! sources for a recorded list of identifiers any of those effects would have
//! to name and fails if one appears -- a regression gate over an enumeration,
//! not a proof over every such identifier.
//!
//! A pass of this crate's tests is evidence about the arithmetic, the bounds
//! and the payload shapes. It closes no hardware, energy-measurement, image,
//! boot or release gate.
//!
//! # Example
//!
//! ```
//! use aegis_tellus::{
//!     EnergyKwh, GridIntensity, RaplZone, SampleDeadline, Seconds, SciEngine, SimulatedWattage,
//!     WattageSource,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let engine = SciEngine::new();
//! let sci = engine.sci_rate(EnergyKwh::new(0.0025)?, 1.0);
//! assert!((sci.rate() - 0.6).abs() < 1.0e-12);
//! assert!(!sci.fell_back());
//! assert!(!engine.should_defer());
//!
//! // A non-positive functional-unit count falls back; it never yields a NaN.
//! let fallback = engine.sci_rate(EnergyKwh::new(0.0025)?, 0.0);
//! assert!(fallback.fell_back());
//! assert!(fallback.rate().is_finite());
//!
//! // The seam carries the zones the reference profile actually exposes.
//! let source = SimulatedWattage::reference_profile();
//! let deadline = SampleDeadline::try_from_millis(5).ok_or("a deadline must be positive")?;
//! let package = source.sample(RaplZone::Package0, deadline)?;
//! assert!(!package.provenance.is_measured());
//! assert!(source.sample(RaplZone::Dram, deadline).is_err());
//!
//! let measured = EnergyKwh::from_draw(package.watts, Seconds::new(60.0)?)?;
//! let hot = SciEngine::with_inputs(GridIntensity::new(300.001)?, engine.embodied());
//! assert!(hot.should_defer());
//! assert!(hot.sci_rate(measured, 1.0).rate().is_finite());
//! # Ok(())
//! # }
//! ```

pub mod bid;
pub mod contracts;
pub mod error;
pub mod id;
pub mod power;
pub mod register;
pub mod sci;

pub use crate::bid::{Bid, DeltaV, EnergyPrice, MAX_DELTA_V, MAX_ENERGY_PRICE};
pub use crate::contracts::graph::EdgeId;
pub use crate::contracts::sci_query::{
    CandidateList, CandidateSciQuery, CandidateSciResponse, MAX_QUERY_CANDIDATES, RateEntry,
    RateList, SciQueryVersion, SciResponseVersion,
};
pub use crate::contracts::task_shift::{TaskShiftDirective, TaskShiftVersion};
pub use crate::contracts::{
    ContractError, Correlation, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, SchemaId,
};
pub use crate::error::TellusError;
pub use crate::id::{CandidateId, CorrelationId, MAX_IDENTIFIER_LEN, SliceName};
pub use crate::power::{
    MAX_CGROUP_SLICES, MAX_RAPL_ZONES, MODELLED_DRAM_WATTS, Provenance, REFERENCE_PROFILE_ZONES,
    RaplZone, SIMULATED_CORE_WATTS, SIMULATED_PACKAGE0_WATTS, SIMULATED_SERVICE_MILLIS,
    SampleDeadline, SimulatedWattage, SliceDraw, SliceTable, WattageSource, ZoneList, ZoneSample,
};
pub use crate::register::{
    ClaimSource, ClaimStatus, P13_RECORDED_CLAIMS, REFERENCE_ENERGY_UJ_MODE,
    REFERENCE_MAX_ENERGY_RANGE_UJ, RecordedClaim,
};
pub use crate::sci::{
    DEFAULT_EMBODIED_CARBON_G, DEFAULT_GRID_INTENSITY_G_PER_KWH, DEFER_THRESHOLD_G_PER_KWH,
    EmbodiedCarbon, EnergyKwh, FALLBACK_FUNCTIONAL_UNITS, FunctionalUnits, GridIntensity,
    JOULES_PER_KWH, MAX_EMBODIED_CARBON_G, MAX_ENERGY_KWH, MAX_GRID_INTENSITY_G_PER_KWH,
    MAX_NUMERATOR_G, MAX_SCI_RATE, MAX_SECONDS, MIN_FUNCTIONAL_UNITS, SciCalculation, SciEngine,
    SciRate, Seconds, Watts,
};
