// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The cases one readings document is judged on.
//!
//! Each case function returns its detail lines on a pass and its reason on a
//! failure, and [`evaluate`] prints them the way `aegis-scaena-display` does:
//! `PASS <case>` or `FAIL <case>`, then indented detail lines. The numbers in
//! the detail lines are the evidence; the case line is only the verdict.

use core::fmt::Display;

use aegis_tellus::{
    CounterRead, EnergyDelta, EnergyRange, EnergyReading, MeasuredWattage, RaplZone,
    SampleDeadline, SciEngine, TellusError, WattageSource,
};

use crate::input::{InputError, Mode, Readings};

/// The service time a sample of a finished measurement declares, in ms.
pub const SERVICE_MILLIS: u32 = 1;

/// The largest relative difference admitted between the energy the seam
/// yields (draw times interval) and the energy read straight off the delta.
pub const SEAM_TOLERANCE: f64 = 1.0e-9;

/// The band, as a factor either side of the median unwrapped draw, a wrapped
/// pair's draw must fall in to count as a correct delta rather than an absurd
/// one.
pub const WRAP_BAND: f64 = 2.0;

/// What every run prints about the number it produces (M21 accuracy
/// criterion).
pub const ACCURACY: &str = "AMD RAPL is a model-based estimate derived from activity counters, \
     not a measured power rail; only same-zone deltas are treated as trustworthy and absolute \
     watts carry vendor-defined error; the kernel's integer energy unit (15.258 uJ for a 2^-16 J \
     unit) understates every reading by a further 0.0052 per cent";

/// The cases a `pair` document is judged on, in the order they print.
pub const PAIR_CASES: [&str; 6] = [
    "rapl/recorded-range",
    "rapl/measured-sci",
    "rapl/unprivileged-read-fails-closed",
    "rapl/dram-zone-refused",
    "rapl/psys-zone-refused",
    "rapl/wrap-at-live-range",
];

/// The case a `wrap-watch` document adds.
pub const OBSERVED_WRAP_CASE: &str = "rapl/observed-wrap";

/// The lines a run prints and how many of its cases failed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Evaluation {
    lines: Vec<String>,
    passed: u32,
    failed: u32,
}

impl Evaluation {
    /// Records one fact.
    fn info(&mut self, key: &str, value: impl Display) {
        self.lines.push(format!("info {key}: {value}"));
    }

    /// Records one case's verdict and its detail lines.
    fn case(&mut self, name: &str, result: Result<Vec<String>, String>) {
        let (verdict, notes) = match result {
            Ok(notes) => {
                self.passed = self.passed.saturating_add(1);
                ("PASS", notes)
            }
            Err(reason) => {
                self.failed = self.failed.saturating_add(1);
                ("FAIL", vec![reason])
            }
        };
        self.lines.push(format!("{verdict} {name}"));
        self.lines
            .extend(notes.into_iter().map(|note| format!("     {note}")));
    }

    /// Returns the lines in the order they were recorded.
    #[must_use]
    pub fn lines(&self) -> &[String] {
        &self.lines
    }

    /// Returns how many cases passed.
    #[must_use]
    pub const fn passed(&self) -> u32 {
        self.passed
    }

    /// Returns how many cases failed.
    #[must_use]
    pub const fn failed(&self) -> u32 {
        self.failed
    }
}

/// The deadline a finished measurement is sampled under.
fn service() -> Result<SampleDeadline, String> {
    SampleDeadline::try_from_millis(SERVICE_MILLIS).ok_or_else(|| "a zero deadline".to_owned())
}

/// Judges one document.
///
/// # Errors
///
/// Returns [`InputError`] when the document names no powercap zone or no
/// admissible range; every other refusal is a failed case, not an error.
pub fn evaluate(document: &Readings) -> Result<Evaluation, InputError> {
    let zone = document.zone()?;
    let range = document.range()?;
    let mut run = Evaluation::default();
    run.info("zone", zone);
    run.info("max_energy_range_uj", range.microjoules());
    run.info("privileged readings", document.readings.len());
    run.info("accuracy", ACCURACY);
    let index = measured_index(document, zone, range);
    run.case("rapl/recorded-range", recorded_range(range));
    run.case(
        "rapl/measured-sci",
        measured_sci(document, zone, index, range),
    );
    run.case(
        "rapl/unprivileged-read-fails-closed",
        unprivileged(document, range),
    );
    run.case(
        "rapl/dram-zone-refused",
        zone_refused(document, RaplZone::Dram, range),
    );
    run.case(
        "rapl/psys-zone-refused",
        zone_refused(document, RaplZone::Psys, range),
    );
    run.case(
        "rapl/wrap-at-live-range",
        wrap_at_live_range(document, range),
    );
    if document.mode == Mode::WrapWatch {
        run.case(OBSERVED_WRAP_CASE, observed_wrap(document, zone, range));
    }
    Ok(run)
}

/// The pair the SCI is computed over: the first wrapped pair in a watch,
/// otherwise the first pair.
fn measured_index(document: &Readings, zone: RaplZone, range: EnergyRange) -> usize {
    if document.mode != Mode::WrapWatch {
        return 0;
    }
    (0..document.pair_count())
        .find(|index| {
            measure(document, zone, *index, range).is_ok_and(|source| source.delta().wrapped())
        })
        .unwrap_or(0)
}

/// Builds the measured source over pair `index`.
fn measure(
    document: &Readings,
    zone: RaplZone,
    index: usize,
    range: EnergyRange,
) -> Result<MeasuredWattage, String> {
    let pair = document
        .pair(index, range)
        .map_err(|error| error.to_string())?;
    MeasuredWattage::from_counters(zone, pair, range, service()?).map_err(|error| error.to_string())
}

/// The range read live is the one the rollover boundary is recorded against.
fn recorded_range(range: EnergyRange) -> Result<Vec<String>, String> {
    if range != EnergyRange::REFERENCE {
        return Err(format!(
            "max_energy_range_uj reads {}, not the recorded {}",
            range.microjoules(),
            EnergyRange::REFERENCE.microjoules()
        ));
    }
    Ok(vec![format!(
        "max_energy_range_uj reads {}, the figure the rollover boundary tests are recorded against",
        range.microjoules()
    )])
}

/// E21-1 positive: the SCI rate computed from the measured energy through the
/// M05 seam and the unchanged engine.
fn measured_sci(
    document: &Readings,
    zone: RaplZone,
    index: usize,
    range: EnergyRange,
) -> Result<Vec<String>, String> {
    let source = measure(document, zone, index, range)?;
    let deadline = service()?;
    let sample = source
        .sample(zone, deadline)
        .map_err(|error| error.to_string())?;
    if !sample.provenance.is_measured() {
        return Err(format!("the sample is labelled {}", sample.provenance));
    }
    let seconds = source
        .interval()
        .seconds()
        .map_err(|error| error.to_string())?;
    let seam = sample.energy(seconds).map_err(|error| error.to_string())?;
    let direct = source.delta().energy().map_err(|error| error.to_string())?;
    let drift = (seam.get() - direct.get()).abs() / direct.get().max(f64::MIN_POSITIVE);
    if drift > SEAM_TOLERANCE {
        return Err(format!(
            "the seam's energy {} kWh and the delta's {} kWh differ by {drift}",
            seam.get(),
            direct.get()
        ));
    }
    let engine = SciEngine::new();
    let calculation = engine.sci_rate(seam, 1.0);
    let rate = calculation.as_rate().map_err(|error| error.to_string())?;
    Ok(sci_notes(document, index, &source, seam.get(), rate.get()))
}

/// The detail lines of the SCI case.
fn sci_notes(
    document: &Readings,
    index: usize,
    source: &MeasuredWattage,
    kwh: f64,
    rate: f64,
) -> Vec<String> {
    let text = |at: Option<usize>| {
        at.and_then(|position| document.readings.get(position))
            .and_then(|read| read.text.clone())
            .unwrap_or_default()
    };
    let engine = SciEngine::new();
    vec![
        format!(
            "pair {index}: energy_uj {} then {}, {} ns apart",
            text(Some(index)).trim(),
            text(index.checked_add(1)).trim(),
            source.interval().nanos()
        ),
        format!("delta {}", source.delta()),
        format!(
            "draw {} W, provenance {}",
            source.measured().watts.get(),
            source.measured().provenance
        ),
        format!("energy {kwh} kWh through the M05 seam"),
        format!(
            "SCI {rate} gCO2eq per functional unit = ((E * I) + M) / R with I {} gCO2eq/kWh, \
             M {} g, R 1 (the interval)",
            engine.intensity().get(),
            engine.embodied().get()
        ),
    ]
}

/// The unprivileged read must fail, and the reader must refuse what it left.
fn unprivileged(document: &Readings, range: EnergyRange) -> Result<Vec<String>, String> {
    let outcome = document
        .unprivileged
        .outcome()
        .map_err(|error| error.to_string())?;
    if let CounterRead::Text(text) = outcome {
        return Err(format!(
            "the unprivileged read returned {:?}; energy_uj is not root-only here",
            text.trim()
        ));
    }
    match EnergyReading::from_read(outcome, range) {
        Err(refusal @ TellusError::Counter { .. }) => Ok(vec![
            format!(
                "the unprivileged read failed: {}",
                document.unprivileged.error.clone().unwrap_or_default()
            ),
            format!("the reader refused it: {refusal}"),
        ]),
        Err(other) => Err(format!("refused for another reason: {other}")),
        Ok(reading) => Err(format!(
            "the reader produced {} uJ from a failed read",
            reading.microjoules()
        )),
    }
}

/// D60: a request for a zone the profile has no counter for fails explicitly.
fn zone_refused(
    document: &Readings,
    absent: RaplZone,
    range: EnergyRange,
) -> Result<Vec<String>, String> {
    let pair = document.pair(0, range).map_err(|error| error.to_string())?;
    let built = MeasuredWattage::from_counters(absent, pair, range, service()?);
    let Err(TellusError::ZoneAbsent { .. }) = built else {
        return Err(format!(
            "building a {absent} source was not refused: {built:?}"
        ));
    };
    let package = MeasuredWattage::from_counters(RaplZone::Package0, pair, range, service()?)
        .map_err(|error| error.to_string())?;
    let sampled = package.sample(absent, service()?);
    let Err(refusal @ TellusError::ZoneAbsent { .. }) = sampled else {
        return Err(format!("sampling {absent} was not refused: {sampled:?}"));
    };
    Ok(vec![
        format!("building a {absent} source over the live pair: refused"),
        format!("sampling {absent} from the package-0 source: {refusal}"),
    ])
}

/// E21-1 boundary on the live range: the live delta, moved so that it
/// straddles the wrap, comes back unchanged and marked wrapped.
fn wrap_at_live_range(document: &Readings, range: EnergyRange) -> Result<Vec<String>, String> {
    let pair = document.pair(0, range).map_err(|error| error.to_string())?;
    let live =
        EnergyDelta::between(pair.before, pair.after, range).map_err(|error| error.to_string())?;
    let half = live.microjoules().checked_div(2).unwrap_or(0);
    let rest = live.microjoules().saturating_sub(half);
    let start = range.microjoules().saturating_sub(half);
    let before = EnergyReading::new(start, range).map_err(|error| error.to_string())?;
    let after = EnergyReading::new(rest, range).map_err(|error| error.to_string())?;
    let wrapped = EnergyDelta::between(before, after, range).map_err(|error| error.to_string())?;
    if !wrapped.wrapped() || wrapped.microjoules() != live.microjoules() {
        return Err(format!(
            "the straddled delta is {wrapped}, the live one {live}"
        ));
    }
    Ok(vec![format!(
        "live delta {live} placed across the wrap at {} (from {start} to {rest}): {wrapped}",
        range.microjoules()
    )])
}

/// E21-1 boundary on the counter itself: a wrap observed between two
/// readings gives a draw within [`WRAP_BAND`] of the unwrapped pairs' median.
fn observed_wrap(
    document: &Readings,
    zone: RaplZone,
    range: EnergyRange,
) -> Result<Vec<String>, String> {
    let mut unwrapped = Vec::new();
    let mut wrapped = Vec::new();
    for index in 0..document.pair_count() {
        let source = measure(document, zone, index, range)
            .map_err(|error| format!("pair {index} was refused: {error}"))?;
        let draw = source.measured().watts.get();
        if source.delta().wrapped() {
            wrapped.push((index, source.delta(), draw));
        } else {
            unwrapped.push(draw);
        }
    }
    unwrapped.sort_by(f64::total_cmp);
    let middle = unwrapped.len().checked_div(2).unwrap_or(0);
    let median = *unwrapped
        .get(middle)
        .ok_or("no unwrapped pair to compare the wrap with")?;
    let Some((index, delta, draw)) = wrapped.first().copied() else {
        return Err(format!(
            "no wrap was observed across {} pairs",
            document.pair_count()
        ));
    };
    let ratio = draw / median;
    if !(1.0 / WRAP_BAND..=WRAP_BAND).contains(&ratio) {
        return Err(format!(
            "the wrapped pair {index} draws {draw} W against a median of {median} W"
        ));
    }
    Ok(vec![
        format!("pair {index} wrapped: {delta}"),
        format!(
            "wrapped draw {draw} W against the median {median} W of {} unwrapped pairs",
            unwrapped.len()
        ),
        format!(
            "{} wrapped pair(s) in {} pairs",
            wrapped.len(),
            document.pair_count()
        ),
    ])
}
