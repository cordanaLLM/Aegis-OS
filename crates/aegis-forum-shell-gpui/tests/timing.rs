// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Tests for pure frame-time and AccessKit tree-update measurement rules (M28 E28-6).
//!
//! Evaluates positive, negative, and boundary criteria for timing judgements, percentiles,
//! line formats, and refusals without requiring a display, GPU, or D-Bus session.

use aegis_forum_shell_gpui::timing::{
    Distribution, FrameSample, MIN_NODES, RECORDED_FRAMES, Refusal, RunInfo, TimingLineError,
    WARMUP_FRAMES, distribution, frame_line, judge_run, measurement_lines, parse_frame_line,
    refusal_line,
};

fn reference_run_info() -> RunInfo {
    RunInfo {
        gpu: "Intel(R) Arc(TM) A380 Graphics (DG2)".to_owned(),
        driver: "Intel open-source Mesa driver".to_owned(),
        driver_info: "Mesa 25.0.0".to_owned(),
        software: false,
        compositor: "KWin 6.8.0".to_owned(),
        zed_commit: "322cd509dcccaee1f6bc44725d270598ae8171ad".to_owned(),
    }
}

fn build_samples(
    warmup_count: usize,
    measured_count: usize,
    nodes_val: Option<usize>,
) -> Vec<FrameSample> {
    let mut samples = Vec::with_capacity(warmup_count.saturating_add(measured_count));
    for i in 0..warmup_count {
        samples.push(FrameSample {
            draw_ns: u64::try_from(i).unwrap_or(0),
            phases_ns: 0,
            nodes: nodes_val,
        });
    }
    for i in 1..=measured_count {
        let val = u64::try_from(i).unwrap_or(0);
        samples.push(FrameSample {
            draw_ns: val,
            phases_ns: 0,
            nodes: nodes_val,
        });
    }
    samples
}

fn assert_distribution(d: &Distribution, expected: (u64, u64, u64, u64, u64, u64)) {
    assert_eq!((d.min, d.p50, d.p95, d.p99, d.max, d.mean), expected);
}

#[test]
fn positive_run_with_1000_seeded_nodes_measured() {
    let info = reference_run_info();
    let samples = build_samples(WARMUP_FRAMES, RECORDED_FRAMES, Some(1005));

    let measurement = judge_run(&samples, &info, RECORDED_FRAMES);
    assert!(measurement.is_ok(), "expected Ok, got {measurement:?}");
    let Ok(m) = measurement else {
        return;
    };
    assert_eq!((m.frames, m.nodes_min, m.nodes_max), (600, 1005, 1005));
    assert_distribution(&m.frame, (1, 300, 570, 594, 600, 300));
    assert_distribution(&m.tree_update, (1, 300, 570, 594, 600, 300));
}

#[test]
fn positive_run_measurement_lines_format() {
    let info = reference_run_info();
    let samples = build_samples(WARMUP_FRAMES, RECORDED_FRAMES, Some(1005));
    let Ok(m) = judge_run(&samples, &info, RECORDED_FRAMES) else {
        return;
    };

    let lines = measurement_lines(&m, &info);
    assert_eq!(lines.len(), 4);
    assert_eq!(
        lines.first().map_or("", String::as_str),
        "TIMING frames 600 nodes 1005..1005"
    );
    assert_eq!(
        lines.get(1).map_or("", String::as_str),
        "TIMING frame_ns min 1 p50 300 p95 570 p99 594 max 600 mean 300"
    );
    assert_eq!(
        lines.get(2).map_or("", String::as_str),
        "TIMING tree_update_ns min 1 p50 300 p95 570 p99 594 max 600 mean 300"
    );
    assert_eq!(
        lines.get(3).map_or("", String::as_str),
        "TIMING gpu Intel(R) Arc(TM) A380 Graphics (DG2) | driver Intel open-source Mesa driver Mesa 25.0.0 | compositor KWin 6.8.0 | zed 322cd509dcccaee1f6bc44725d270598ae8171ad"
    );
}

#[test]
fn positive_frame_line_round_trip() {
    let sample_with_nodes = FrameSample {
        draw_ns: 12_345_678,
        phases_ns: 10_000_000,
        nodes: Some(1005),
    };
    let formatted_nodes = frame_line(&sample_with_nodes);
    assert_eq!(formatted_nodes, "FRAME 12345678 10000000 1005");
    let parsed_nodes = parse_frame_line(&formatted_nodes);
    assert_eq!(parsed_nodes, Ok(sample_with_nodes));

    let sample_without_nodes = FrameSample {
        draw_ns: 12_345_678,
        phases_ns: 10_000_000,
        nodes: None,
    };
    let formatted_without = frame_line(&sample_without_nodes);
    assert_eq!(formatted_without, "FRAME 12345678 10000000 -");
    let parsed_without = parse_frame_line(&formatted_without);
    assert_eq!(parsed_without, Ok(sample_without_nodes));
}

#[test]
fn negative_adapter_inactive_refused() {
    let info = reference_run_info();
    let mut samples = build_samples(WARMUP_FRAMES, RECORDED_FRAMES, Some(1005));
    if let Some(s) = samples.get_mut(WARMUP_FRAMES.saturating_add(10)) {
        s.nodes = None;
    }

    let result = judge_run(&samples, &info, RECORDED_FRAMES);
    assert_eq!(
        result,
        Err(Refusal::AdapterInactive {
            frames_without_update: 1
        })
    );
}

#[test]
fn negative_too_few_nodes_refused() {
    let info = reference_run_info();
    let mut samples = build_samples(WARMUP_FRAMES, RECORDED_FRAMES, Some(1005));
    if let Some(s) = samples.get_mut(WARMUP_FRAMES.saturating_add(42)) {
        s.nodes = Some(999);
    }

    let result = judge_run(&samples, &info, RECORDED_FRAMES);
    assert_eq!(
        result,
        Err(Refusal::TooFewNodes {
            min_seen: 999,
            want: MIN_NODES
        })
    );
}

#[test]
fn negative_software_renderer_refused() {
    let mut info = reference_run_info();
    info.software = true;
    let samples = build_samples(WARMUP_FRAMES, RECORDED_FRAMES, Some(1005));

    let result = judge_run(&samples, &info, RECORDED_FRAMES);
    assert_eq!(
        result,
        Err(Refusal::SoftwareRenderer(
            "Intel(R) Arc(TM) A380 Graphics (DG2)".to_owned()
        ))
    );
}

#[test]
fn negative_missing_info_fields_refused() {
    let samples = build_samples(WARMUP_FRAMES, RECORDED_FRAMES, Some(1005));

    let mut info = reference_run_info();
    info.zed_commit.clear();
    assert_eq!(
        judge_run(&samples, &info, RECORDED_FRAMES),
        Err(Refusal::MissingInfo("zed_commit"))
    );

    let mut info = reference_run_info();
    info.gpu.clear();
    assert_eq!(
        judge_run(&samples, &info, RECORDED_FRAMES),
        Err(Refusal::MissingInfo("gpu"))
    );

    let mut info = reference_run_info();
    info.driver.clear();
    assert_eq!(
        judge_run(&samples, &info, RECORDED_FRAMES),
        Err(Refusal::MissingInfo("driver"))
    );

    let mut info = reference_run_info();
    info.compositor.clear();
    assert_eq!(
        judge_run(&samples, &info, RECORDED_FRAMES),
        Err(Refusal::MissingInfo("compositor"))
    );
}

#[test]
fn negative_parse_frame_line_errors() {
    assert_eq!(
        parse_frame_line("FRAME 100 50"),
        Err(TimingLineError::FieldCount(2))
    );
    assert_eq!(
        parse_frame_line("FRAME 100 50 1005 extra"),
        Err(TimingLineError::FieldCount(4))
    );
    assert_eq!(
        parse_frame_line("NOT_FRAME 100 50 1005"),
        Err(TimingLineError::InvalidPrefix)
    );
    assert!(matches!(
        parse_frame_line("FRAME not_a_num 50 1005"),
        Err(TimingLineError::InvalidDraw(_))
    ));
    assert!(matches!(
        parse_frame_line("FRAME 100 not_a_num 1005"),
        Err(TimingLineError::InvalidPhases(_))
    ));
    assert!(matches!(
        parse_frame_line("FRAME 100 50 not_a_num"),
        Err(TimingLineError::InvalidNodes(_))
    ));
}

#[test]
fn boundary_frame_counts() {
    let info = reference_run_info();
    // Exactly WARMUP_FRAMES + RECORDED_FRAMES accepted
    let exact_samples = build_samples(WARMUP_FRAMES, RECORDED_FRAMES, Some(1005));
    assert!(judge_run(&exact_samples, &info, RECORDED_FRAMES).is_ok());

    // One frame short refused
    let short_samples = build_samples(WARMUP_FRAMES, RECORDED_FRAMES.saturating_sub(1), Some(1005));
    assert_eq!(
        judge_run(&short_samples, &info, RECORDED_FRAMES),
        Err(Refusal::TooFewFrames {
            got: 599,
            want: 600
        })
    );
}

#[test]
fn boundary_nodes_at_exact_floor() {
    let info = reference_run_info();
    let samples = build_samples(WARMUP_FRAMES, RECORDED_FRAMES, Some(1000));
    let result = judge_run(&samples, &info, RECORDED_FRAMES);
    assert!(result.is_ok(), "expected Ok at 1000 nodes, got {result:?}");
    if let Ok(m) = result {
        assert_eq!(m.nodes_min, 1000);
        assert_eq!(m.nodes_max, 1000);
    }
}

#[test]
fn boundary_residual_saturates_at_zero() {
    let sample = FrameSample {
        draw_ns: 50,
        phases_ns: 100,
        nodes: Some(1000),
    };
    assert_eq!(sample.residual_ns(), 0);
}

#[test]
fn boundary_distribution_single_and_empty() {
    assert_eq!(
        distribution(&[42]),
        Some(Distribution {
            min: 42,
            p50: 42,
            p95: 42,
            p99: 42,
            max: 42,
            mean: 42,
        })
    );
    assert_eq!(distribution(&[]), None);
}

#[test]
fn boundary_refusal_line_formatting() {
    let line = refusal_line(&Refusal::TooFewFrames {
        got: 599,
        want: 600,
    });
    assert!(line.starts_with("REFUSED "));
    assert!(line.contains("599"));
    assert!(line.contains("600"));
}
