// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P13 `aegis-tellus-rapl`: measured energy on the reference profile
//! (milestone M21, epic E21-1).
//!
//! `make verify-workstation` reads the powercap counters -- the zone names and
//! `max_energy_range_uj` unprivileged, `energy_uj` through the one scoped
//! `sudo -n cat` the maintainer admitted on 2026-09-29 -- and hands what it
//! read to the `aegis-tellus-rapl` binary as one readings document. This crate
//! turns that document into cases:
//!
//! * `rapl/measured-sci`: two readings and their interval become an
//!   [`aegis_tellus::MeasuredWattage`], the second implementation of the M05
//!   seam, and the unchanged [`aegis_tellus::SciEngine`] computes the SCI rate
//!   from the measured energy (E21-1 positive);
//! * `rapl/unprivileged-read-fails-closed`: the gate's unprivileged read of
//!   the root-only counter failed, and the reader refuses what it left with an
//!   error rather than a default value (E21-1 negative);
//! * `rapl/dram-zone-refused` and `rapl/psys-zone-refused`: decision D60 on
//!   live data;
//! * `rapl/recorded-range`, `rapl/wrap-at-live-range` and, when the gate
//!   watched the counter across a wrap, `rapl/observed-wrap`: the rollover
//!   boundary against the range read live (E21-1 boundary).
//!
//! # What this crate does not do
//!
//! It reads nothing and starts nothing: no file, no process, no `sudo`. The
//! binary takes the document as its one argument and writes report lines to
//! standard output. Every pass is development evidence on the reference
//! profile: it qualifies no hardware and closes no hardware or release gate.

pub mod cases;
pub mod input;

pub use crate::cases::{
    ACCURACY, Evaluation, OBSERVED_WRAP_CASE, PAIR_CASES, SEAM_TOLERANCE, SERVICE_MILLIS,
    WRAP_BAND, evaluate,
};
pub use crate::input::{
    InputError, MAX_DOCUMENT_BYTES, MAX_FIELD_BYTES, MAX_READINGS, MIN_READINGS, Mode,
    READINGS_SCHEMA, Read, Readings,
};
