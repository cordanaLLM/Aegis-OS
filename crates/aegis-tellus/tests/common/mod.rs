// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the P13 integration tests.
//!
//! Everything here reaches `aegis-tellus` through its public API only, and
//! nothing here uses `unwrap` or `expect`: a test that cannot build its own
//! input returns the failure instead of panicking through a lint the workspace
//! denies. The source-sweep helpers below are the ones `aegis-hestia` carries,
//! kept identical so a sweep behaves the same in every crate that has one.

#![allow(dead_code)]

use aegis_tellus::{
    CandidateId, CandidateList, CandidateSciQuery, CandidateSciResponse, CorrelationId, EdgeId,
    EnergyKwh, GridIntensity, RateEntry, RateList, SampleDeadline, SciQueryVersion,
    SciResponseVersion, SliceName, TaskShiftDirective, TaskShiftVersion,
};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The correlation identifier the fixtures thread through every payload.
pub const CORRELATION: &str = "m05-fixture-0001";

/// The candidate the fixtures ask about.
pub const CANDIDATE: &str = "candidate-001-scx-cake-v2";

/// The cgroup slice the fixtures defer.
pub const SLICE: &str = "build.slice";

/// The energy the scaffold benchmarks one thousand tokens at, in kWh.
pub const SCAFFOLD_ENERGY_KWH: f64 = 0.0025;

/// The SCI rate the scaffold's benchmark inputs hand-compute to.
///
/// `(0.0025 * 220.0 + 0.05) / 1.0`, which is `0.55 + 0.05`.
pub const SCAFFOLD_SCI_RATE: f64 = 0.6;

/// The tolerance every floating-point comparison in this suite uses.
pub const EPSILON: f64 = 1.0e-12;

/// Returns the fixture correlation identifier.
///
/// # Errors
///
/// Propagates [`CorrelationId::parse`].
pub fn correlation() -> Result<CorrelationId, Box<dyn std::error::Error>> {
    Ok(CorrelationId::parse(CORRELATION)?)
}

/// Returns the fixture candidate identifier.
///
/// # Errors
///
/// Propagates [`CandidateId::parse`].
pub fn candidate() -> Result<CandidateId, Box<dyn std::error::Error>> {
    Ok(CandidateId::parse(CANDIDATE)?)
}

/// Returns the fixture slice name.
///
/// # Errors
///
/// Propagates [`SliceName::parse`].
pub fn slice() -> Result<SliceName, Box<dyn std::error::Error>> {
    Ok(SliceName::parse(SLICE)?)
}

/// Returns a deadline comfortably longer than the simulated service time.
///
/// # Errors
///
/// Returns a failure when the recorded figure is not positive.
pub fn deadline() -> Result<SampleDeadline, Box<dyn std::error::Error>> {
    SampleDeadline::try_from_millis(5).ok_or_else(|| "a deadline must be positive".into())
}

/// Returns a query asking about the fixture candidate.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn query() -> Result<CandidateSciQuery, Box<dyn std::error::Error>> {
    let mut candidates = CandidateList::new();
    candidates.push(candidate()?)?;
    Ok(CandidateSciQuery {
        schema: SciQueryVersion::V1,
        edge: EdgeId::EvaluateCandidateCarbonSci,
        correlation_id: correlation()?,
        candidates,
        energy_kwh: EnergyKwh::new(SCAFFOLD_ENERGY_KWH)?,
        functional_units: 1.0,
    })
}

/// Returns the response the fixture query would be answered with.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn response() -> Result<CandidateSciResponse, Box<dyn std::error::Error>> {
    let engine = aegis_tellus::SciEngine::new();
    let sci = engine.sci_rate(EnergyKwh::new(SCAFFOLD_ENERGY_KWH)?, 1.0);
    let mut rates = RateList::new();
    rates.push(RateEntry::new(
        candidate()?,
        sci.as_rate()?,
        sci.fell_back(),
    ))?;
    Ok(CandidateSciResponse {
        schema: SciResponseVersion::V1,
        edge: EdgeId::EvaluateCandidateCarbonSci,
        correlation_id: correlation()?,
        intensity: engine.intensity(),
        rates,
        deferred: engine.should_defer(),
    })
}

/// Returns a directive that does not defer, at the default intensity.
///
/// # Errors
///
/// Propagates the field constructors.
pub fn directive() -> Result<TaskShiftDirective, Box<dyn std::error::Error>> {
    Ok(TaskShiftDirective {
        schema: TaskShiftVersion::V1,
        edge: EdgeId::SpatiotemporalTaskShift,
        correlation_id: correlation()?,
        slice: slice()?,
        intensity: GridIntensity::DEFAULT,
        defer: false,
    })
}

/// Scalar bound on directories visited and entries read per directory.
pub const WALK_BOUND: usize = 64;

/// Returns the `.rs` files under `root`, as (file name, body) pairs.
///
/// The walk is iterative and doubly bounded: at most [`WALK_BOUND`]
/// directories and at most [`WALK_BOUND`] entries per directory.
///
/// # Errors
///
/// Returns the first read failure, so a sweep cannot pass by reading nothing.
pub fn rust_sources(
    root: &std::path::Path,
) -> Result<Vec<(String, String)>, Box<dyn std::error::Error>> {
    let mut out: Vec<(String, String)> = Vec::new();
    let mut stack: Vec<std::path::PathBuf> = vec![root.to_path_buf()];
    for _ in 0..WALK_BOUND {
        let Some(directory) = stack.pop() else { break };
        for entry in std::fs::read_dir(&directory)?.take(WALK_BOUND) {
            let path = entry?.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().is_none_or(|ext| ext != "rs") {
                continue;
            }
            let name = path
                .file_name()
                .map(|raw| raw.to_string_lossy().into_owned())
                .unwrap_or_default();
            out.push((name, std::fs::read_to_string(&path)?));
        }
    }
    if out.is_empty() {
        return Err(format!("no Rust source was read under {}", root.display()).into());
    }
    Ok(out)
}

/// Scalar bound on the lines read from any one source file.
pub const LINE_BOUND: usize = 4096;

/// Returns the `"<file>:<line>: <identifier>"` hits `text` carries.
fn hits(name: &str, number: usize, text: &str, watched: &[&str]) -> Vec<String> {
    watched
        .iter()
        .filter(|needle| text.contains(**needle))
        .map(|needle| format!("{name}:{}: {needle}", number.saturating_add(1)))
        .collect()
}

/// Returns the hits one body of text carries, skipping comment lines.
///
/// Comment lines are skipped whole: the crate documentation names the very
/// identifiers a sweep watches for, precisely in order to say it does not use
/// them, so a sweep that read comments would fail on its own documentation.
///
/// This is public so a watched list can be exercised against planted text.
/// A list of constructs is a gate only if each entry is one the scan reports,
/// and that is checkable without writing a file.
pub fn scan_text(name: &str, body: &str, watched: &[&str]) -> Vec<String> {
    let mut found: Vec<String> = Vec::new();
    for (number, line) in body.lines().take(LINE_BOUND).enumerate() {
        let text = line.trim();
        if text.starts_with("//") {
            continue;
        }
        found.extend(hits(name, number, text, watched));
    }
    found
}

/// Returns every watched identifier the sources under `root` carry.
///
/// # Errors
///
/// Propagates [`rust_sources`], so a sweep cannot pass by reading nothing.
pub fn scan_sources(
    root: &std::path::Path,
    watched: &[&str],
) -> Result<Vec<String>, Box<dyn std::error::Error>> {
    let mut found: Vec<String> = Vec::new();
    for (name, body) in rust_sources(root)? {
        found.extend(scan_text(&name, &body, watched));
    }
    Ok(found)
}
