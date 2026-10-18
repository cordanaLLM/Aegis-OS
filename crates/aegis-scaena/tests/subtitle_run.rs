// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M32 subtitle run arithmetic (E32-1, REQ-P17-08).
//!
//! Positive: a cue's start timecode maps to the frame it falls on at 30 frames
//! per second, frame n is paced to n/30 s, correlation ids are zero-padded, and
//! offsets are measured from frame 0's presentation timestamp. Negative: a
//! frame with no timestamp, or one earlier than frame 0, has no offset.
//! Boundary: frame 0 and the clip's last frame, 2,999 ms on frame 89 and
//! 3,000 ms one past the clip.

mod common;

use core::time::Duration;

use aegis_scaena::subtitle_run::{
    SUBTITLE_FRAMES, compute_offsets, correlation_id, cue_frame_index, pacing_target_duration,
};
use common::Fallible;

#[test]
fn cue_start_maps_to_the_frame_it_falls_on() {
    assert_eq!(cue_frame_index(Duration::from_millis(0)), 0);
    assert_eq!(cue_frame_index(Duration::from_millis(500)), 15);
    assert_eq!(cue_frame_index(Duration::from_millis(1200)), 36);
    assert_eq!(cue_frame_index(Duration::from_millis(2000)), 60);
}

#[test]
fn boundary_last_frame_and_one_past_the_clip() {
    assert_eq!(cue_frame_index(Duration::from_millis(2999)), 89);
    assert_eq!(
        cue_frame_index(Duration::from_millis(3000)),
        SUBTITLE_FRAMES
    );
}

#[test]
fn frame_n_is_paced_to_n_thirtieths_of_a_second() {
    assert_eq!(pacing_target_duration(0), Duration::ZERO);
    assert_eq!(pacing_target_duration(30), Duration::from_secs(1));
    assert_eq!(
        pacing_target_duration(SUBTITLE_FRAMES),
        Duration::from_secs(3)
    );
    assert_eq!(pacing_target_duration(1).as_nanos(), 33_333_333);
    assert_eq!(pacing_target_duration(89).as_nanos(), 2_966_666_666);
}

#[test]
fn correlation_ids_are_zero_padded() -> Fallible {
    assert_eq!(correlation_id(0)?.as_str(), "m32-subtitle-00");
    assert_eq!(correlation_id(1)?.as_str(), "m32-subtitle-01");
    assert_eq!(correlation_id(SUBTITLE_FRAMES)?.as_str(), "m32-subtitle-90");
    Ok(())
}

#[test]
fn offsets_are_measured_from_frame_zero() {
    let mut stamps = [None; SUBTITLE_FRAMES];
    if let Some(slot) = stamps.first_mut() {
        *slot = Some(Duration::from_secs(10));
    }
    if let Some(slot) = stamps.get_mut(15) {
        *slot = Some(Duration::from_millis(10_500));
    }
    if let Some(slot) = stamps.get_mut(89) {
        *slot = Some(Duration::from_millis(12_966));
    }
    let offsets = compute_offsets(&stamps);
    assert_eq!(offsets.first().copied().flatten(), Some(Duration::ZERO));
    assert_eq!(
        offsets.get(15).copied().flatten(),
        Some(Duration::from_millis(500))
    );
    assert_eq!(
        offsets.get(89).copied().flatten(),
        Some(Duration::from_millis(2966))
    );
}

#[test]
fn missing_or_earlier_timestamps_have_no_offset() {
    let mut stamps = [None; SUBTITLE_FRAMES];
    assert_eq!(compute_offsets(&stamps), [None; SUBTITLE_FRAMES]);
    if let Some(slot) = stamps.first_mut() {
        *slot = Some(Duration::from_secs(10));
    }
    if let Some(slot) = stamps.get_mut(1) {
        *slot = Some(Duration::from_secs(9));
    }
    let offsets = compute_offsets(&stamps);
    assert_eq!(offsets.get(1).copied().flatten(), None);
    assert_eq!(offsets.get(36).copied().flatten(), None);
}
