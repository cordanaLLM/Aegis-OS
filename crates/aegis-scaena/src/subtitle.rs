// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Subtitle timing arithmetic and bounded `SubRip` reader (REQ-P17-08, E32-1).
//!
//! Evaluates presentation synchronization against timecodes under [`SYNC_BOUND`]
//! and parses bounded `SubRip` (`.srt`) subtitles without unbounded heap growth.

use core::time::Duration;
use thiserror::Error;

use crate::error::SubtitleError;

/// Subtitle synchronization tolerance bound (100 ms).
pub const SYNC_BOUND: Duration = Duration::from_millis(100);

/// Maximum number of cues admitted in a single `SubRip` stream (256).
pub const MAX_CUES: usize = 256;

/// Maximum admitted text payload length for a single cue in bytes (512).
pub const MAX_CUE_TEXT_BYTES: usize = 512;

/// Maximum admitted size of a complete `SubRip` input in bytes (64 KiB).
pub const MAX_SRT_BYTES: usize = 64 * 1024;

/// Upper bound on the number of lines iterated during `SubRip` parsing.
pub const MAX_SRT_LINES: usize = MAX_SRT_BYTES;

/// A single parsed subtitle cue from a `SubRip` stream.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cue {
    /// Sequential 1-based cue index.
    pub index: u32,
    /// Presentation start timecode.
    pub start: Duration,
    /// Presentation end timecode.
    pub end: Duration,
    /// Subtitle payload text.
    pub text: String,
}

/// The offset of a presented frame relative to a subtitle timecode outside [`SYNC_BOUND`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("subtitle presentation offset of {offset:?} is outside sync bound (late: {late})")]
pub struct Offset {
    /// Absolute offset between presentation time and timecode.
    pub offset: Duration,
    /// True if presented after the timecode; false if presented before.
    pub late: bool,
}

/// Evaluates whether a frame's presentation time matches the subtitle timecode within [`SYNC_BOUND`].
///
/// Returns the absolute offset if within [`SYNC_BOUND`].
///
/// # Errors
///
/// Returns [`Offset`] if the absolute offset exceeds [`SYNC_BOUND`].
pub fn judge(timecode: Duration, presented: Duration) -> Result<Duration, Offset> {
    let (offset, late) = if presented >= timecode {
        let diff = match presented.checked_sub(timecode) {
            Some(d) => d,
            None => Duration::ZERO,
        };
        (diff, presented > timecode)
    } else {
        let diff = match timecode.checked_sub(presented) {
            Some(d) => d,
            None => Duration::ZERO,
        };
        (diff, false)
    };

    if offset <= SYNC_BOUND {
        Ok(offset)
    } else {
        Err(Offset { offset, late })
    }
}

/// A draft cue accumulated during parsing.
#[derive(Debug)]
struct CueDraft {
    index: u32,
    start: Duration,
    end: Duration,
    text: String,
}

impl CueDraft {
    const fn empty() -> Self {
        Self {
            index: 0,
            start: Duration::ZERO,
            end: Duration::ZERO,
            text: String::new(),
        }
    }
}

/// Internal states of the bounded `SubRip` line parser.
#[derive(Debug)]
enum ParserState {
    AwaitingIndex,
    AwaitingTiming { index: u32 },
    CollectingText(CueDraft),
}

fn parse_digit(byte: Option<&u8>) -> Result<u64, SubtitleError> {
    match byte {
        Some(&b) if b.is_ascii_digit() => match u64::from(b).checked_sub(u64::from(b'0')) {
            Some(digit) => Ok(digit),
            None => Err(SubtitleError::MalformedTimecode),
        },
        _ => Err(SubtitleError::MalformedTimecode),
    }
}

fn parse_timecode_digits(bytes: &[u8]) -> Result<Duration, SubtitleError> {
    if bytes.len() != 12 {
        return Err(SubtitleError::MalformedTimecode);
    }
    if bytes.get(2) != Some(&b':') || bytes.get(5) != Some(&b':') || bytes.get(8) != Some(&b',') {
        return Err(SubtitleError::MalformedTimecode);
    }
    let h1 = parse_digit(bytes.first())?;
    let h2 = parse_digit(bytes.get(1))?;
    let m1 = parse_digit(bytes.get(3))?;
    let m2 = parse_digit(bytes.get(4))?;
    let s1 = parse_digit(bytes.get(6))?;
    let s2 = parse_digit(bytes.get(7))?;
    let ms1 = parse_digit(bytes.get(9))?;
    let ms2 = parse_digit(bytes.get(10))?;
    let ms3 = parse_digit(bytes.get(11))?;

    let hours = h1
        .checked_mul(10)
        .and_then(|h| h.checked_add(h2))
        .ok_or(SubtitleError::MalformedTimecode)?;
    let minutes = m1
        .checked_mul(10)
        .and_then(|m| m.checked_add(m2))
        .ok_or(SubtitleError::MalformedTimecode)?;
    let seconds = s1
        .checked_mul(10)
        .and_then(|s| s.checked_add(s2))
        .ok_or(SubtitleError::MalformedTimecode)?;
    let ms2_ten = ms2
        .checked_mul(10)
        .ok_or(SubtitleError::MalformedTimecode)?;
    let millis = ms1
        .checked_mul(100)
        .and_then(|ms| ms.checked_add(ms2_ten))
        .and_then(|ms| ms.checked_add(ms3))
        .ok_or(SubtitleError::MalformedTimecode)?;

    if minutes > 59 || seconds > 59 || millis > 999 {
        return Err(SubtitleError::MalformedTimecode);
    }

    let h_ms = hours
        .checked_mul(3_600_000)
        .ok_or(SubtitleError::MalformedTimecode)?;
    let m_ms = minutes
        .checked_mul(60_000)
        .ok_or(SubtitleError::MalformedTimecode)?;
    let s_ms = seconds
        .checked_mul(1_000)
        .ok_or(SubtitleError::MalformedTimecode)?;
    let total_ms = h_ms
        .checked_add(m_ms)
        .and_then(|ms| ms.checked_add(s_ms))
        .and_then(|ms| ms.checked_add(millis))
        .ok_or(SubtitleError::MalformedTimecode)?;

    Ok(Duration::from_millis(total_ms))
}

fn parse_timing_line(line: &str) -> Result<(Duration, Duration), SubtitleError> {
    let bytes = line.as_bytes();
    if bytes.len() != 29 {
        return Err(SubtitleError::MalformedTimecode);
    }
    if bytes.get(12..17) != Some(b" --> ") {
        return Err(SubtitleError::MalformedTimecode);
    }
    let start_bytes = bytes.get(0..12).ok_or(SubtitleError::MalformedTimecode)?;
    let end_bytes = bytes.get(17..29).ok_or(SubtitleError::MalformedTimecode)?;
    let start = parse_timecode_digits(start_bytes)?;
    let end = parse_timecode_digits(end_bytes)?;
    if end <= start {
        return Err(SubtitleError::EndBeforeStart);
    }
    Ok((start, end))
}

fn parse_index_line(line: &str, expected: u32) -> Result<u32, SubtitleError> {
    match line.trim().parse::<u32>() {
        Ok(idx) if idx == expected => Ok(idx),
        _ => Err(SubtitleError::InvalidIndex),
    }
}

fn append_text_line(text: &mut String, line: &str) -> Result<(), SubtitleError> {
    let additional = if text.is_empty() {
        line.len()
    } else {
        let Some(add) = line.len().checked_add(1) else {
            return Err(SubtitleError::TextTooLong);
        };
        add
    };
    let Some(new_len) = text.len().checked_add(additional) else {
        return Err(SubtitleError::TextTooLong);
    };
    if new_len > MAX_CUE_TEXT_BYTES {
        return Err(SubtitleError::TextTooLong);
    }
    if !text.is_empty() {
        text.push('\n');
    }
    text.push_str(line);
    Ok(())
}

fn commit_cue(draft: CueDraft, cues: &mut Vec<Cue>) -> Result<(), SubtitleError> {
    if draft.text.is_empty() {
        return Err(SubtitleError::MalformedBlock);
    }
    if cues.len() >= MAX_CUES {
        return Err(SubtitleError::TooManyCues);
    }
    if draft.text.len() > MAX_CUE_TEXT_BYTES {
        return Err(SubtitleError::TextTooLong);
    }
    cues.push(Cue {
        index: draft.index,
        start: draft.start,
        end: draft.end,
        text: draft.text,
    });
    Ok(())
}

fn process_line(
    line: &str,
    state: &mut ParserState,
    cues: &mut Vec<Cue>,
    expected_index: &mut u32,
    last_start: &mut Option<Duration>,
) -> Result<(), SubtitleError> {
    match state {
        ParserState::AwaitingIndex => {
            if !line.is_empty() {
                let index = parse_index_line(line, *expected_index)?;
                *state = ParserState::AwaitingTiming { index };
            }
        }
        ParserState::AwaitingTiming { index } => {
            let (start, end) = parse_timing_line(line)?;
            if let Some(prev) = *last_start
                && start < prev
            {
                return Err(SubtitleError::CueStartBeforePrevious);
            }
            *last_start = Some(start);
            *state = ParserState::CollectingText(CueDraft {
                index: *index,
                start,
                end,
                text: String::new(),
            });
        }
        ParserState::CollectingText(draft) => {
            if line.is_empty() {
                let finished = core::mem::replace(draft, CueDraft::empty());
                commit_cue(finished, cues)?;
                *expected_index = expected_index
                    .checked_add(1)
                    .ok_or(SubtitleError::TooManyCues)?;
                *state = ParserState::AwaitingIndex;
            } else {
                append_text_line(&mut draft.text, line)?;
            }
        }
    }
    Ok(())
}

fn finish_parse(state: ParserState, cues: &mut Vec<Cue>) -> Result<(), SubtitleError> {
    match state {
        ParserState::AwaitingIndex => Ok(()),
        ParserState::CollectingText(draft) => commit_cue(draft, cues),
        ParserState::AwaitingTiming { .. } => Err(SubtitleError::MalformedBlock),
    }
}

/// Parses a `SubRip` (`.srt`) subtitle document under strict resource and syntax bounds.
///
/// # Errors
///
/// Returns [`SubtitleError`] if the input is malformed, out of order, or exceeds
/// memory bounds.
pub fn parse_srt(input: &str) -> Result<Vec<Cue>, SubtitleError> {
    if input.len() > MAX_SRT_BYTES {
        return Err(SubtitleError::TooLong);
    }
    if input.trim().is_empty() {
        return Err(SubtitleError::Empty);
    }
    let mut cues = Vec::new();
    let mut state = ParserState::AwaitingIndex;
    let mut expected_index = 1_u32;
    let mut last_start = None;

    for (line_idx, raw_line) in input.lines().enumerate() {
        if line_idx >= MAX_SRT_LINES {
            return Err(SubtitleError::TooLong);
        }
        let line = match raw_line.strip_suffix('\r') {
            Some(stripped) => stripped,
            None => raw_line,
        };
        process_line(
            line,
            &mut state,
            &mut cues,
            &mut expected_index,
            &mut last_start,
        )?;
    }

    finish_parse(state, &mut cues)?;
    if cues.is_empty() {
        return Err(SubtitleError::Empty);
    }
    Ok(cues)
}
