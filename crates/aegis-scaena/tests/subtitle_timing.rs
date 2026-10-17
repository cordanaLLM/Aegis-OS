// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E32-1 subtitle timing and bounded `SubRip` reader (REQ-P17-08).
//!
//! Positive: a 3-cue `SubRip` stream parses to exact presentation start and end
//! durations and cue text; presentation within 40 ms late or early passes
//! synchronization judgment. Negative: presentation 150 ms late is refused with
//! late=true; malformed input exceeding byte bounds, cue count limits, cue text
//! bounds, malformed timecodes, out-of-order timecodes, end before start, non-sequential
//! indices, and empty input are each refused with their own error. Boundary:
//! presentation exactly 100 ms late and early passes; 101 ms late and early
//! fails; exactly 256 cues are admitted and 257 refused; cue text of exactly 512
//! bytes is admitted and 513 bytes refused; timecodes at 00:00:00,000 and
//! 99:59:59,999 are admitted.

mod common;

use core::fmt::Write as _;
use core::time::Duration;

use aegis_scaena::{
    Cue, MAX_CUE_TEXT_BYTES, MAX_CUES, MAX_SRT_BYTES, Offset, SubtitleError, judge, parse_srt,
};
use common::Fallible;

/// Builds a valid `SubRip` stream containing `count` sequential cues with monotonic start times.
fn make_cues_srt(count: usize) -> String {
    let mut srt = String::new();
    for i in 1..=count {
        let sec = i.checked_div(10).unwrap_or(0);
        let ms = i.checked_rem(10).unwrap_or(0).saturating_mul(100);
        let _ = writeln!(srt, "{i}");
        let _ = writeln!(srt, "00:00:{sec:02},{ms:03} --> 00:00:{sec:02},999");
        let _ = writeln!(srt, "Cue payload {i}\n");
    }
    srt
}

// --- Positive -------------------------------------------------------------

fn check_cue(cue: &Cue, index: u32, start: Duration, end: Duration, text: &str) {
    assert_eq!(cue.index, index);
    assert_eq!(cue.start, start);
    assert_eq!(cue.end, end);
    assert_eq!(cue.text, text);
}

/// Positive: a 3-cue `SubRip` document parses into exact start, end durations and text.
#[test]
fn positive_three_cue_srt_parses_to_exact_durations() -> Fallible {
    let input = "1\n00:00:01,000 --> 00:00:04,500\nFirst line\n\n2\n00:00:05,250 --> 00:00:08,750\nSecond line 1\nSecond line 2\n\n3\n00:00:10,000 --> 00:00:15,000\nThird line\n";
    let cues = parse_srt(input)?;
    assert_eq!(cues.len(), 3);

    let first = cues.first().ok_or("missing first cue")?;
    check_cue(
        first,
        1,
        Duration::from_secs(1),
        Duration::from_millis(4_500),
        "First line",
    );

    let second = cues.get(1).ok_or("missing second cue")?;
    check_cue(
        second,
        2,
        Duration::from_millis(5_250),
        Duration::from_millis(8_750),
        "Second line 1\nSecond line 2",
    );

    let third = cues.get(2).ok_or("missing third cue")?;
    check_cue(
        third,
        3,
        Duration::from_secs(10),
        Duration::from_secs(15),
        "Third line",
    );

    Ok(())
}

/// Positive: presentation within 40 ms late or early passes synchronization judgment.
#[test]
fn positive_judge_forty_ms_late_and_early() {
    let t = Duration::from_secs(5);
    let delta = Duration::from_millis(40);
    assert_eq!(judge(t, t.saturating_add(delta)), Ok(delta));
    assert_eq!(judge(t, t.saturating_sub(delta)), Ok(delta));
}

/// Positive: CRLF line terminators are accepted identically to LF.
#[test]
fn positive_accepts_crlf_line_endings() -> Fallible {
    let input = "1\r\n00:00:01,000 --> 00:00:02,000\r\nHello CRLF\r\n\r\n2\r\n00:00:03,000 --> 00:00:04,000\r\nWorld CRLF\r\n";
    let cues = parse_srt(input)?;
    assert_eq!(cues.len(), 2);
    let first = cues.first().ok_or("missing first cue")?;
    assert_eq!(first.text, "Hello CRLF");
    let second = cues.get(1).ok_or("missing second cue")?;
    assert_eq!(second.text, "World CRLF");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: presentation 150 ms late produces an error with late=true.
#[test]
fn negative_judge_one_hundred_fifty_ms_late() {
    let t = Duration::from_secs(10);
    let presented = t.saturating_add(Duration::from_millis(150));
    assert_eq!(
        judge(t, presented),
        Err(Offset {
            offset: Duration::from_millis(150),
            late: true,
        })
    );
}

/// Negative: presentation 150 ms early produces an error with late=false.
#[test]
fn negative_judge_one_hundred_fifty_ms_early() {
    let t = Duration::from_secs(10);
    let presented = t.saturating_sub(Duration::from_millis(150));
    assert_eq!(
        judge(t, presented),
        Err(Offset {
            offset: Duration::from_millis(150),
            late: false,
        })
    );
}

/// Negative: input over `MAX_SRT_BYTES` is refused.
#[test]
fn negative_input_over_max_srt_bytes() {
    let too_long = " ".repeat(MAX_SRT_BYTES.saturating_add(1));
    assert_eq!(parse_srt(&too_long), Err(SubtitleError::TooLong));
}

/// Negative: input with more than `MAX_CUES` cues is refused.
#[test]
fn negative_more_than_max_cues() {
    let srt = make_cues_srt(MAX_CUES.saturating_add(1));
    assert_eq!(parse_srt(&srt), Err(SubtitleError::TooManyCues));
}

/// Negative: cue text exceeding `MAX_CUE_TEXT_BYTES` is refused.
#[test]
fn negative_text_over_max_cue_text_bytes() {
    let text = "x".repeat(MAX_CUE_TEXT_BYTES.saturating_add(1));
    let srt = format!("1\n00:00:01,000 --> 00:00:02,000\n{text}\n");
    assert_eq!(parse_srt(&srt), Err(SubtitleError::TextTooLong));
}

/// Negative: malformed timecode with wrong separators is refused.
#[test]
fn negative_malformed_timecode_wrong_separators() {
    let dot_sep = "1\n00:00:01.000 --> 00:00:02,000\nText\n";
    assert_eq!(parse_srt(dot_sep), Err(SubtitleError::MalformedTimecode));

    let dash_sep = "1\n00-00-01,000 --> 00:00:02,000\nText\n";
    assert_eq!(parse_srt(dash_sep), Err(SubtitleError::MalformedTimecode));

    let bad_arrow = "1\n00:00:01,000 -> 00:00:02,000\nText\n";
    assert_eq!(parse_srt(bad_arrow), Err(SubtitleError::MalformedTimecode));
}

/// Negative: malformed timecode with minutes > 59 is refused.
#[test]
fn negative_malformed_timecode_minutes_out_of_range() {
    let srt = "1\n00:60:00,000 --> 00:00:01,000\nText\n";
    assert_eq!(parse_srt(srt), Err(SubtitleError::MalformedTimecode));
}

/// Negative: malformed timecode with seconds > 59 is refused.
#[test]
fn negative_malformed_timecode_seconds_out_of_range() {
    let srt = "1\n00:00:60,000 --> 00:00:01,000\nText\n";
    assert_eq!(parse_srt(srt), Err(SubtitleError::MalformedTimecode));
}

/// Negative: malformed timecode with non-digit characters in milliseconds is refused.
#[test]
fn negative_malformed_timecode_millis_invalid_digits() {
    let srt = "1\n00:00:00,abc --> 00:00:01,000\nText\n";
    assert_eq!(parse_srt(srt), Err(SubtitleError::MalformedTimecode));
}

/// Negative: cue with end <= start is refused.
#[test]
fn negative_end_before_or_equal_start() {
    let equal = "1\n00:00:05,000 --> 00:00:05,000\nText\n";
    assert_eq!(parse_srt(equal), Err(SubtitleError::EndBeforeStart));

    let before = "1\n00:00:05,000 --> 00:00:04,999\nText\n";
    assert_eq!(parse_srt(before), Err(SubtitleError::EndBeforeStart));
}

/// Negative: cue start before previous cue start is refused.
#[test]
fn negative_cue_start_before_previous_cue_start() {
    let srt =
        "1\n00:00:05,000 --> 00:00:07,000\nFirst\n\n2\n00:00:04,999 --> 00:00:08,000\nSecond\n";
    assert_eq!(parse_srt(srt), Err(SubtitleError::CueStartBeforePrevious));
}

/// Negative: cue index not previous + 1 (first must be 1) is refused.
#[test]
fn negative_index_not_previous_plus_one() {
    let starts_at_two = "2\n00:00:01,000 --> 00:00:02,000\nText\n";
    assert_eq!(parse_srt(starts_at_two), Err(SubtitleError::InvalidIndex));

    let starts_at_zero = "0\n00:00:01,000 --> 00:00:02,000\nText\n";
    assert_eq!(parse_srt(starts_at_zero), Err(SubtitleError::InvalidIndex));

    let gap_in_index =
        "1\n00:00:01,000 --> 00:00:02,000\nText\n\n3\n00:00:03,000 --> 00:00:04,000\nText\n";
    assert_eq!(parse_srt(gap_in_index), Err(SubtitleError::InvalidIndex));
}

/// Negative: empty input or whitespace-only input is refused.
#[test]
fn negative_empty_input() {
    assert_eq!(parse_srt(""), Err(SubtitleError::Empty));
    assert_eq!(parse_srt("   \r\n\n\t  "), Err(SubtitleError::Empty));
}

/// Negative: cue block missing text lines is refused.
#[test]
fn negative_missing_text_lines() {
    let srt = "1\n00:00:01,000 --> 00:00:02,000\n\n";
    assert_eq!(parse_srt(srt), Err(SubtitleError::MalformedBlock));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: presentation exactly 100 ms late or early passes synchronization.
#[test]
fn boundary_judge_exactly_one_hundred_ms_late_and_early_ok() {
    let t = Duration::from_secs(5);
    let bound = Duration::from_millis(100);
    assert_eq!(judge(t, t.saturating_add(bound)), Ok(bound));
    assert_eq!(judge(t, t.saturating_sub(bound)), Ok(bound));
}

/// Boundary: presentation 101 ms late or early fails synchronization.
#[test]
fn boundary_judge_one_hundred_one_ms_late_and_early_err() {
    let t = Duration::from_secs(5);
    let delta = Duration::from_millis(101);
    assert_eq!(
        judge(t, t.saturating_add(delta)),
        Err(Offset {
            offset: delta,
            late: true,
        })
    );
    assert_eq!(
        judge(t, t.saturating_sub(delta)),
        Err(Offset {
            offset: delta,
            late: false,
        })
    );
}

/// Boundary: exactly `MAX_CUES` cues accepted and `MAX_CUES + 1` refused.
#[test]
fn boundary_max_cues_accepted_and_plus_one_refused() -> Fallible {
    let srt_max = make_cues_srt(MAX_CUES);
    let cues = parse_srt(&srt_max)?;
    assert_eq!(cues.len(), MAX_CUES);

    let srt_plus_one = make_cues_srt(MAX_CUES.saturating_add(1));
    assert_eq!(parse_srt(&srt_plus_one), Err(SubtitleError::TooManyCues));
    Ok(())
}

/// Boundary: text of exactly `MAX_CUE_TEXT_BYTES` accepted and +1 refused.
#[test]
fn boundary_text_max_cue_text_bytes_accepted_and_plus_one_refused() -> Fallible {
    let text_exact = "c".repeat(MAX_CUE_TEXT_BYTES);
    let srt_exact = format!("1\n00:00:01,000 --> 00:00:02,000\n{text_exact}\n");
    let cues = parse_srt(&srt_exact)?;
    assert_eq!(cues.len(), 1);
    let first = cues.first().ok_or("missing cue")?;
    assert_eq!(first.text.len(), MAX_CUE_TEXT_BYTES);

    let text_plus_one = "c".repeat(MAX_CUE_TEXT_BYTES.saturating_add(1));
    let srt_plus_one = format!("1\n00:00:01,000 --> 00:00:02,000\n{text_plus_one}\n");
    assert_eq!(parse_srt(&srt_plus_one), Err(SubtitleError::TextTooLong));
    Ok(())
}

/// Boundary: 00:00:00,000 and 99:59:59,999 timecodes are accepted.
#[test]
fn boundary_timecode_zero_and_ninety_nine_hours_accepted() -> Fallible {
    let srt = "1\n00:00:00,000 --> 99:59:59,999\nBoundary text\n";
    let cues = parse_srt(srt)?;
    assert_eq!(cues.len(), 1);
    let first = cues.first().ok_or("missing cue")?;
    assert_eq!(first.start, Duration::ZERO);

    let expected_ms = 99_u64
        .saturating_mul(3_600_000)
        .saturating_add(59_u64.saturating_mul(60_000))
        .saturating_add(59_u64.saturating_mul(1_000))
        .saturating_add(999);
    assert_eq!(first.end, Duration::from_millis(expected_ms));
    Ok(())
}
