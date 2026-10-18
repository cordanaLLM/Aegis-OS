// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Pure measurement rules, percentiles, and refusals for frame and tree-update timing (M28 E28-6).
//!
//! Evaluates recorded frame samples against acceptance criteria, computing nearest-rank
//! percentiles for draw duration and residual tree-update time. Holds zero dependencies
//! on gpui, Wayland, or D-Bus so judgements run deterministically on any host.

/// The recorded frame count: 10 s at 60 Hz, chosen by E28-6, NOT a target.
pub const RECORDED_FRAMES: usize = 600;

/// The warm-up frame count dropped before evaluating measurements.
pub const WARMUP_FRAMES: usize = 60;

/// The minimum number of reachable AccessKit nodes required in an accepted run.
pub const MIN_NODES: usize = 1000;

/// A single frame timing sample with its AccessKit tree-update node count.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameSample {
    /// Duration of `Window::draw` in nanoseconds.
    pub draw_ns: u64,
    /// Sum of `request_layout`, `prepaint`, and `paint` phase durations in nanoseconds.
    pub phases_ns: u64,
    /// The node count of the AccessKit tree update sent during this frame, if any.
    pub nodes: Option<usize>,
}

impl FrameSample {
    /// Returns the post-paint residual duration (`draw_ns - phases_ns`) in nanoseconds.
    ///
    /// Saturates at 0 if measured phases exceed total draw time.
    #[must_use]
    pub const fn residual_ns(&self) -> u64 {
        self.draw_ns.saturating_sub(self.phases_ns)
    }
}

/// System, GPU, compositor, and commit metadata for a timing run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunInfo {
    /// The GPU device name reported by the platform.
    pub gpu: String,
    /// The GPU driver name.
    pub driver: String,
    /// Extended driver version or information.
    pub driver_info: String,
    /// Whether software emulation (e.g. llvmpipe) was active.
    pub software: bool,
    /// The compositor version or identifier.
    pub compositor: String,
    /// The git commit hash of zed gpui was built from.
    pub zed_commit: String,
}

/// Reasons why a timing run is refused as a valid measurement.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum Refusal {
    /// The run produced fewer than the required number of frames.
    #[error("too few frames: got {got}, want {want}")]
    TooFewFrames {
        /// Number of valid frames received after warm-up.
        got: usize,
        /// Required number of frames.
        want: usize,
    },
    /// The accessibility adapter was inactive during one or more measured frames.
    #[error("accessibility adapter inactive: {frames_without_update} frames without tree update")]
    AdapterInactive {
        /// Number of measured frames that lacked a tree update.
        frames_without_update: usize,
    },
    /// The accessibility tree contained fewer than the required node floor.
    #[error("too few nodes: min seen {min_seen}, want {want}")]
    TooFewNodes {
        /// Minimum node count observed across measured frames.
        min_seen: usize,
        /// Required node count floor.
        want: usize,
    },
    /// The run was executed on a software-emulated renderer.
    #[error("software renderer emulated: {0}")]
    SoftwareRenderer(String),
    /// A required run information field was empty.
    #[error("missing info field: {0}")]
    MissingInfo(&'static str),
}

/// Statistical distribution over a set of nanosecond timing samples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Distribution {
    /// Minimum observed value in nanoseconds.
    pub min: u64,
    /// 50th percentile (median) in nanoseconds.
    pub p50: u64,
    /// 95th percentile in nanoseconds.
    pub p95: u64,
    /// 99th percentile in nanoseconds.
    pub p99: u64,
    /// Maximum observed value in nanoseconds.
    pub max: u64,
    /// Arithmetic mean in nanoseconds.
    pub mean: u64,
}

fn nearest_rank(sorted: &[u64], percentile: u64) -> Option<u64> {
    let len = sorted.len();
    if len == 0 {
        return None;
    }
    let n = u64::try_from(len).ok()?;
    let rank = percentile
        .checked_mul(n)?
        .checked_add(99)?
        .checked_div(100)?;
    let rank_usize = usize::try_from(rank).ok()?;
    let idx = rank_usize.saturating_sub(1).min(len.saturating_sub(1));
    sorted.get(idx).copied()
}

/// Computes nearest-rank percentiles and mean on a slice of timing values.
///
/// Returns `None` if `values` is empty or if arithmetic overflows.
#[must_use]
pub fn distribution(values: &[u64]) -> Option<Distribution> {
    if values.is_empty() {
        return None;
    }
    let mut sorted = values.to_vec();
    sorted.sort_unstable();

    let min = sorted.first().copied()?;
    let max = sorted.last().copied()?;
    let p50 = nearest_rank(&sorted, 50)?;
    let p95 = nearest_rank(&sorted, 95)?;
    let p99 = nearest_rank(&sorted, 99)?;

    let mut sum: u128 = 0;
    for &v in &sorted {
        sum = sum.saturating_add(u128::from(v));
    }
    let count = u128::try_from(sorted.len()).ok()?;
    let mean_u128 = sum.checked_div(count)?;
    let mean = u64::try_from(mean_u128).ok()?;

    Some(Distribution {
        min,
        p50,
        p95,
        p99,
        max,
        mean,
    })
}

/// An accepted timing measurement covering frame and tree-update distributions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Measurement {
    /// Number of measured frames evaluated.
    pub frames: usize,
    /// Minimum node count observed.
    pub nodes_min: usize,
    /// Maximum node count observed.
    pub nodes_max: usize,
    /// Distribution of total `Window::draw` durations in nanoseconds.
    pub frame: Distribution,
    /// Distribution of post-paint residual tree-update durations in nanoseconds.
    pub tree_update: Distribution,
}

fn validate_info(info: &RunInfo) -> Result<(), Refusal> {
    if info.software {
        return Err(Refusal::SoftwareRenderer(info.gpu.clone()));
    }
    if info.gpu.trim().is_empty() {
        return Err(Refusal::MissingInfo("gpu"));
    }
    if info.driver.trim().is_empty() {
        return Err(Refusal::MissingInfo("driver"));
    }
    if info.compositor.trim().is_empty() {
        return Err(Refusal::MissingInfo("compositor"));
    }
    if info.zed_commit.trim().is_empty() {
        return Err(Refusal::MissingInfo("zed_commit"));
    }
    Ok(())
}

fn check_samples(samples: &[FrameSample]) -> Result<(usize, usize), Refusal> {
    let mut frames_without_update = 0usize;
    let mut min_seen = usize::MAX;
    let mut max_seen = 0usize;

    for s in samples {
        match s.nodes {
            Some(n) => {
                min_seen = min_seen.min(n);
                max_seen = max_seen.max(n);
            }
            None => {
                frames_without_update = frames_without_update.saturating_add(1);
            }
        }
    }
    if frames_without_update > 0 {
        return Err(Refusal::AdapterInactive {
            frames_without_update,
        });
    }
    if min_seen < MIN_NODES {
        let reported = if min_seen == usize::MAX { 0 } else { min_seen };
        return Err(Refusal::TooFewNodes {
            min_seen: reported,
            want: MIN_NODES,
        });
    }
    Ok((min_seen, max_seen))
}

/// Evaluates a run of frame samples against acceptance criteria.
///
/// Drops the first [`WARMUP_FRAMES`], checks that at least `frames` valid samples remain,
/// and verifies node count and adapter activity before computing distributions.
///
/// # Errors
///
/// Returns [`Refusal`] if any acceptance invariant fails.
pub fn judge_run(
    samples: &[FrameSample],
    info: &RunInfo,
    frames: usize,
) -> Result<Measurement, Refusal> {
    validate_info(info)?;

    let kept = samples.get(WARMUP_FRAMES..).unwrap_or(&[]);
    if kept.len() < frames {
        return Err(Refusal::TooFewFrames {
            got: kept.len(),
            want: frames,
        });
    }

    let eval_samples = kept.get(..frames).unwrap_or(kept);
    let (nodes_min, nodes_max) = check_samples(eval_samples)?;

    let mut draw_values = Vec::with_capacity(frames);
    let mut residual_values = Vec::with_capacity(frames);
    for s in eval_samples {
        draw_values.push(s.draw_ns);
        residual_values.push(s.residual_ns());
    }

    let frame = distribution(&draw_values).ok_or(Refusal::TooFewFrames {
        got: 0,
        want: frames,
    })?;
    let tree_update = distribution(&residual_values).ok_or(Refusal::TooFewFrames {
        got: 0,
        want: frames,
    })?;

    Ok(Measurement {
        frames,
        nodes_min,
        nodes_max,
        frame,
        tree_update,
    })
}

/// Formats an accepted measurement into development evidence report lines.
#[must_use]
pub fn measurement_lines(m: &Measurement, info: &RunInfo) -> Vec<String> {
    let driver_field = if info.driver_info.is_empty() {
        info.driver.clone()
    } else {
        format!("{} {}", info.driver, info.driver_info)
    };
    vec![
        format!(
            "TIMING frames {} nodes {}..{}",
            m.frames, m.nodes_min, m.nodes_max
        ),
        format!(
            "TIMING frame_ns min {} p50 {} p95 {} p99 {} max {} mean {}",
            m.frame.min, m.frame.p50, m.frame.p95, m.frame.p99, m.frame.max, m.frame.mean
        ),
        format!(
            "TIMING tree_update_ns min {} p50 {} p95 {} p99 {} max {} mean {}",
            m.tree_update.min,
            m.tree_update.p50,
            m.tree_update.p95,
            m.tree_update.p99,
            m.tree_update.max,
            m.tree_update.mean
        ),
        format!(
            "TIMING gpu {} | driver {} | compositor {} | zed {}",
            info.gpu, driver_field, info.compositor, info.zed_commit
        ),
    ]
}

/// Formats a refusal reason into a single `REFUSED <message>` line.
#[must_use]
pub fn refusal_line(r: &Refusal) -> String {
    format!("REFUSED {r}")
}

/// Errors parsing a `FRAME` log line.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum TimingLineError {
    /// The line did not begin with the `FRAME` keyword.
    #[error("invalid prefix, want FRAME")]
    InvalidPrefix,
    /// The line contained an unexpected number of tokens after `FRAME`.
    #[error("expected exactly 3 fields after FRAME, got {0}")]
    FieldCount(usize),
    /// The `draw_ns` value could not be parsed as an unsigned 64-bit integer.
    #[error("invalid draw_ns: {0}")]
    InvalidDraw(String),
    /// The `phases_ns` value could not be parsed as an unsigned 64-bit integer.
    #[error("invalid phases_ns: {0}")]
    InvalidPhases(String),
    /// The `nodes` value was not `-` and could not be parsed as an unsigned integer.
    #[error("invalid nodes: {0}")]
    InvalidNodes(String),
}

/// Serializes a frame sample to a `FRAME <draw_ns> <phases_ns> <nodes|->` string.
#[must_use]
pub fn frame_line(sample: &FrameSample) -> String {
    match sample.nodes {
        Some(nodes) => format!("FRAME {} {} {}", sample.draw_ns, sample.phases_ns, nodes),
        None => format!("FRAME {} {} -", sample.draw_ns, sample.phases_ns),
    }
}

/// Parses a frame sample from a `FRAME <draw_ns> <phases_ns> <nodes|->` line.
///
/// # Errors
///
/// Returns [`TimingLineError`] if any token is missing, unexpected, or invalid.
pub fn parse_frame_line(line: &str) -> Result<FrameSample, TimingLineError> {
    let mut tokens = line.split_whitespace();
    let prefix = tokens.next().ok_or(TimingLineError::InvalidPrefix)?;
    if prefix != "FRAME" {
        return Err(TimingLineError::InvalidPrefix);
    }
    let rest: Vec<&str> = tokens.collect();
    if rest.len() != 3 {
        return Err(TimingLineError::FieldCount(rest.len()));
    }

    let draw_str = rest.first().copied().unwrap_or("");
    let phases_str = rest.get(1).copied().unwrap_or("");
    let nodes_str = rest.get(2).copied().unwrap_or("");

    let draw_ns = draw_str
        .parse::<u64>()
        .map_err(|e| TimingLineError::InvalidDraw(format!("{e}")))?;
    let phases_ns = phases_str
        .parse::<u64>()
        .map_err(|e| TimingLineError::InvalidPhases(format!("{e}")))?;
    let nodes = if nodes_str == "-" {
        None
    } else {
        let n = nodes_str
            .parse::<usize>()
            .map_err(|e| TimingLineError::InvalidNodes(format!("{e}")))?;
        Some(n)
    };

    Ok(FrameSample {
        draw_ns,
        phases_ns,
        nodes,
    })
}
