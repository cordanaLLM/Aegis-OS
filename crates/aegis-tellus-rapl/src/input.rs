// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The readings document the gate hands over: what it read, when, and how.
//!
//! `tools/verify_workstation.py` reads the powercap attributes -- the range and
//! the zone name unprivileged, `energy_uj` through its one scoped `sudo -n
//! cat` -- stamps each read with `time.monotonic_ns()` at the read's midpoint,
//! and passes the result as one JSON argument. Nothing here re-reads anything:
//! a read that failed arrives as an `error`, and [`Read::outcome`] turns it
//! into [`CounterRead::Unreadable`], which `aegis-tellus` refuses.

use aegis_tellus::{
    CounterPair, CounterRead, EnergyRange, EnergyReading, RaplZone, SampleInterval, TellusError,
};
use serde::Deserialize;

/// The schema tag the document must carry.
pub const READINGS_SCHEMA: &str = "aegis.m21.rapl-readings.v1";

/// The fewest privileged readings a document may carry: one pair.
pub const MIN_READINGS: usize = 2;

/// Scalar upper bound on the privileged readings one document carries.
pub const MAX_READINGS: usize = 256;

/// Scalar upper bound, in bytes, on the whole document.
pub const MAX_DOCUMENT_BYTES: usize = 65_536;

/// Scalar upper bound, in bytes, on one read's text or error before it is
/// validated.
pub const MAX_FIELD_BYTES: usize = 256;

/// How the gate sampled the counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Mode {
    /// Two readings separated by a recorded interval.
    Pair,
    /// Readings at a fixed spacing until the counter was seen to wrap.
    WrapWatch,
}

/// Why a readings document was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum InputError {
    /// The document is longer than [`MAX_DOCUMENT_BYTES`].
    #[error("the readings document is {0} bytes; the bound is {MAX_DOCUMENT_BYTES}")]
    TooLong(usize),
    /// The document is not the schema's JSON.
    #[error("the readings document does not parse: {0}")]
    Malformed(String),
    /// The document parsed and breaks a rule this crate holds.
    #[error("the readings document is refused: {0}")]
    Refused(&'static str),
    /// A value in it was refused by `aegis-tellus`.
    #[error(transparent)]
    Tellus(#[from] TellusError),
}

/// One read the gate made: its text or its error, and when it happened.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Read {
    /// What the read returned, when it returned anything.
    pub text: Option<String>,
    /// Why the read failed, when it failed.
    pub error: Option<String>,
    /// The read's midpoint on the gate's monotonic clock, in nanoseconds.
    pub monotonic_ns: u64,
}

impl Read {
    /// Returns what the read yielded, for `aegis-tellus` to validate.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::Refused`] when the read carries both a text and
    /// an error, or neither: the document must say which happened.
    pub fn outcome(&self) -> Result<CounterRead<'_>, InputError> {
        match (&self.text, &self.error) {
            (Some(text), None) => Ok(CounterRead::Text(text)),
            (None, Some(_)) => Ok(CounterRead::Unreadable),
            _ => Err(InputError::Refused(
                "a read must carry exactly one of text and error",
            )),
        }
    }

    /// Returns `true` when both fields are within [`MAX_FIELD_BYTES`].
    fn bounded(&self) -> bool {
        let text = self.text.as_ref().map_or(0, String::len);
        let error = self.error.as_ref().map_or(0, String::len);
        text <= MAX_FIELD_BYTES && error <= MAX_FIELD_BYTES
    }
}

/// The whole document.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct Readings {
    /// Must be [`READINGS_SCHEMA`].
    pub schema: String,
    /// The zone's `name` attribute as the gate read it.
    pub zone: String,
    /// The zone's `max_energy_range_uj` text as the gate read it.
    pub max_energy_range_uj: String,
    /// How the gate sampled.
    pub mode: Mode,
    /// The privileged readings, oldest first.
    pub readings: Vec<Read>,
    /// The one read of `energy_uj` the gate made without privilege.
    pub unprivileged: Read,
}

impl Readings {
    /// Parses and checks a document.
    ///
    /// # Errors
    ///
    /// Returns [`InputError`] for a document past its bound, one that does not
    /// parse, a wrong schema tag, a reading count outside
    /// `MIN_READINGS..=MAX_READINGS`, an over-long field, or timestamps that do
    /// not strictly increase.
    pub fn parse(text: &str) -> Result<Self, InputError> {
        if text.len() > MAX_DOCUMENT_BYTES {
            return Err(InputError::TooLong(text.len()));
        }
        let document: Self =
            serde_json::from_str(text).map_err(|error| InputError::Malformed(error.to_string()))?;
        document.check()?;
        Ok(document)
    }

    /// Applies the rules the field types cannot state.
    fn check(&self) -> Result<(), InputError> {
        if self.schema != READINGS_SCHEMA {
            return Err(InputError::Refused(
                "the schema tag is not the one this build reads",
            ));
        }
        let count = self.readings.len();
        if !(MIN_READINGS..=MAX_READINGS).contains(&count) {
            return Err(InputError::Refused("the reading count is outside 2..=256"));
        }
        let bounded = self.readings.iter().take(MAX_READINGS).all(Read::bounded);
        if !bounded || !self.unprivileged.bounded() || self.zone.len() > MAX_FIELD_BYTES {
            return Err(InputError::Refused("a field is longer than its bound"));
        }
        let increasing = self
            .readings
            .iter()
            .zip(self.readings.iter().skip(1))
            .take(MAX_READINGS)
            .all(|(earlier, later)| later.monotonic_ns > earlier.monotonic_ns);
        if !increasing {
            return Err(InputError::Refused(
                "the reading timestamps do not strictly increase",
            ));
        }
        Ok(())
    }

    /// Returns the zone the document names, by its powercap `name`.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::Refused`] for a name the powercap vocabulary
    /// does not define.
    pub fn zone(&self) -> Result<RaplZone, InputError> {
        let name = self.zone.strip_suffix('\n').unwrap_or(&self.zone);
        RaplZone::ALL
            .into_iter()
            .find(|zone| zone.name() == name)
            .ok_or(InputError::Refused("the zone name is not a powercap zone"))
    }

    /// Returns the range the document read.
    ///
    /// # Errors
    ///
    /// Propagates [`EnergyRange::parse`].
    pub fn range(&self) -> Result<EnergyRange, InputError> {
        Ok(EnergyRange::parse(&self.max_energy_range_uj)?)
    }

    /// Returns readings `index` and `index + 1` as a validated pair.
    ///
    /// # Errors
    ///
    /// Returns [`InputError::Refused`] past the last pair, and
    /// [`InputError::Tellus`] for a read `aegis-tellus` refuses -- an
    /// unreadable one included -- or an interval it refuses.
    pub fn pair(&self, index: usize, range: EnergyRange) -> Result<CounterPair, InputError> {
        let later = index
            .checked_add(1)
            .ok_or(InputError::Refused("no such pair"))?;
        let (Some(first), Some(second)) = (self.readings.get(index), self.readings.get(later))
        else {
            return Err(InputError::Refused("no such pair"));
        };
        let nanos =
            second
                .monotonic_ns
                .checked_sub(first.monotonic_ns)
                .ok_or(InputError::Refused(
                    "the reading timestamps do not strictly increase",
                ))?;
        Ok(CounterPair {
            before: EnergyReading::from_read(first.outcome()?, range)?,
            after: EnergyReading::from_read(second.outcome()?, range)?,
            interval: SampleInterval::from_nanos(nanos)?,
        })
    }

    /// Returns how many consecutive pairs the document carries.
    #[must_use]
    pub fn pair_count(&self) -> usize {
        self.readings.len().saturating_sub(1)
    }
}
