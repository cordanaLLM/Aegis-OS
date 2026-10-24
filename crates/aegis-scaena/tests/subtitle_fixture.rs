// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Subtitle fixture regression verification (E32-1, milestone M32).
//!
//! Positive: parses the committed subtitle fixture with `parse_srt` and asserts
//! that the 3 cues match their expected index, start/end durations, and text.

mod common;

use core::time::Duration;

use aegis_scaena::parse_srt;
use common::{Fallible, check_cue};

/// The committed `SubRip` subtitle fixture for milestone M32.
const COMMITTED_SRT: &str = include_str!("../fixtures/subtitle-clip-90.srt");

/// Positive: the committed subtitle fixture parses into exactly 3 cues with expected timing.
#[test]
fn committed_subtitle_fixture_parses_into_three_cues() -> Fallible {
    let cues = parse_srt(COMMITTED_SRT)?;
    assert_eq!(cues.len(), 3);

    let first = cues.first().ok_or("missing first cue")?;
    check_cue(
        first,
        1,
        Duration::from_millis(500),
        Duration::from_millis(1000),
        "Aegis subtitle one",
    );

    let second = cues.get(1).ok_or("missing second cue")?;
    check_cue(
        second,
        2,
        Duration::from_millis(1200),
        Duration::from_millis(1800),
        "Aegis subtitle two",
    );

    let third = cues.get(2).ok_or("missing third cue")?;
    check_cue(
        third,
        3,
        Duration::from_millis(2000),
        Duration::from_millis(2700),
        "Aegis subtitle three",
    );

    Ok(())
}
