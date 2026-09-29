// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M27 criterion 5: the baseline JPEG reader that fills the VA-API buffers,
//! over the committed fixture and over frames built to break it.
//!
//! Positive: the committed fixture is exactly 60 baseline frames of 256 x 64,
//! 4:2:0, each with its tables and one scan, and nothing follows the 60th.
//! Negative: data without `SOI`, a progressive frame, a 12-bit table, a scan
//! over part of the spectrum, a frame without a Huffman table and a truncated
//! segment are each refused with their own error. Boundary: a DC table of 12
//! values is read and one of 13 refused; stuffed bytes and restart markers do
//! not end a scan; a frame of more segments than the bound is refused.

mod common;

use aegis_scaena::content::{Crc32, FIXTURE_FRAMES, FIXTURE_HEIGHT, FIXTURE_WIDTH};
use aegis_scaena::fixture::MJPEG;
use aegis_scaena::jpeg::{JpegError, MAX_DC_VALUES, MAX_SEGMENTS, parse};

use common::Fallible;

/// The fixture's length and CRC-32, as committed; its sha256 is pinned by
/// `tools/verify_display.py` and `tools/test_display_slice.py`.
const FIXTURE_LENGTH: usize = 48_477;

/// The first frame of the fixture, up to and including its `EOI`.
fn first_frame() -> Result<&'static [u8], JpegError> {
    let (_, used) = parse(MJPEG)?;
    MJPEG.get(..used).ok_or(JpegError::Truncated { at: 0 })
}

/// Where the first marker `0xff`, `marker` sits in `frame`.
fn marker_at(frame: &[u8], marker: u8) -> Option<usize> {
    frame.windows(2).position(|pair| pair == [0xff, marker])
}

/// `frame` with the byte at `at` replaced.
fn patched(frame: &[u8], at: usize, value: u8) -> Vec<u8> {
    let mut copy = frame.to_vec();
    if let Some(byte) = copy.get_mut(at) {
        *byte = value;
    }
    copy
}

/// A minimal frame: `SOI`, `segments` and `EOI`.
fn frame_of(segments: &[&[u8]]) -> Vec<u8> {
    let mut data = vec![0xff, 0xd8];
    for segment in segments {
        data.extend_from_slice(segment);
    }
    data.extend_from_slice(&[0xff, 0xd9]);
    data
}

/// A `DHT` segment holding one DC table in slot 0 with `count` values.
fn dc_table(count: u8) -> Vec<u8> {
    let length = 2_u16 + 1 + 16 + u16::from(count);
    let mut segment = vec![0xff, 0xc4];
    segment.extend_from_slice(&length.to_be_bytes());
    segment.push(0x00);
    let mut counts = [0_u8; 16];
    if let Some(first) = counts.get_mut(3) {
        *first = count;
    }
    segment.extend_from_slice(&counts);
    segment.extend((0..count).map(|value| value % 12));
    segment
}

// --- Positive -------------------------------------------------------------

/// Positive: 60 baseline 4:2:0 frames of 256 x 64 with their tables and
/// one scan each, filling the committed file exactly.
#[test]
fn the_fixture_is_sixty_baseline_frames() -> Fallible {
    assert_eq!(MJPEG.len(), FIXTURE_LENGTH);
    let mut rest = MJPEG;
    for _ in 0..FIXTURE_FRAMES {
        let (frame, used) = parse(rest)?;
        assert_eq!(
            (u32::from(frame.width), u32::from(frame.height)),
            (FIXTURE_WIDTH, FIXTURE_HEIGHT)
        );
        assert_eq!(frame.component_count, 3);
        let sampling: Vec<(u8, u8)> = frame
            .components
            .iter()
            .map(|component| (component.horizontal, component.vertical))
            .collect();
        assert_eq!(sampling, [(2, 2), (1, 1), (1, 1)]);
        assert_eq!(frame.scan_count, 3);
        assert_eq!(frame.mcu_count(), 16 * 4);
        assert!(!frame.scan_data.is_empty());
        rest = rest.get(used..).ok_or("past the end")?;
    }
    assert!(
        rest.is_empty(),
        "{} bytes follow the last frame",
        rest.len()
    );
    Ok(())
}

/// Positive: the file is the committed one, by its CRC-32 as a second guard
/// beside the sha256 the gate pins.
#[test]
fn the_fixture_is_the_committed_file() {
    let mut crc = Crc32::new();
    crc.update(MJPEG);
    assert_eq!(crc.finish(), 0xb7b1_e951, "the fixture changed");
}

// --- Negative -------------------------------------------------------------

/// Negative: data that does not start with `SOI` is refused.
#[test]
fn data_without_soi_is_refused() {
    assert_eq!(parse(&[0x00, 0xd8, 0xff, 0xd9]), Err(JpegError::NoStart));
    assert_eq!(parse(&[]), Err(JpegError::NoStart));
}

/// Negative: a progressive frame header (`SOF2`) is refused as a marker the
/// baseline reader does not admit.
#[test]
fn a_progressive_frame_is_refused() -> Fallible {
    let frame = first_frame()?;
    let at = marker_at(frame, 0xc0).ok_or("no SOF0")?;
    let progressive = patched(frame, at + 1, 0xc2);
    assert!(matches!(
        parse(&progressive),
        Err(JpegError::Marker { marker: 0xc2, .. })
    ));
    Ok(())
}

/// Negative: a 12-bit quantisation table is outside the baseline process.
#[test]
fn a_twelve_bit_table_is_refused() -> Fallible {
    let frame = first_frame()?;
    let at = marker_at(frame, 0xdb).ok_or("no DQT")?;
    let wide = patched(frame, at + 4, 0x10);
    assert!(matches!(
        parse(&wide),
        Err(JpegError::NotBaseline { segment: "DQT", .. })
    ));
    Ok(())
}

/// Negative: a scan over part of the spectrum is refused.
#[test]
fn a_partial_spectrum_scan_is_refused() -> Fallible {
    let frame = first_frame()?;
    let at = marker_at(frame, 0xda).ok_or("no SOS")?;
    let length = usize::from(u16::from_be_bytes([
        *frame.get(at + 2).ok_or("short")?,
        *frame.get(at + 3).ok_or("short")?,
    ]));
    // The segment ends with Ss, Se and Ah/Al; Se drops from 63 to 5.
    let spectral_end = at + 2 + length - 2;
    let partial = patched(frame, spectral_end - 1, 5);
    assert!(matches!(
        parse(&partial),
        Err(JpegError::NotBaseline { segment: "SOS", .. })
    ));
    Ok(())
}

/// Negative: a frame whose scan names a Huffman table no `DHT` defined is
/// refused.
#[test]
fn a_frame_without_huffman_tables_is_refused() -> Fallible {
    let frame = first_frame()?;
    let at = marker_at(frame, 0xc4).ok_or("no DHT")?;
    let length = usize::from(u16::from_be_bytes([
        *frame.get(at + 2).ok_or("short")?,
        *frame.get(at + 3).ok_or("short")?,
    ]));
    let mut without = frame.get(..at).ok_or("short")?.to_vec();
    without.extend_from_slice(frame.get(at + 2 + length..).ok_or("short")?);
    assert_eq!(
        parse(&without),
        Err(JpegError::Missing {
            what: "Huffman table"
        })
    );
    Ok(())
}

/// Negative: a segment that runs past the data is refused.
#[test]
fn a_truncated_segment_is_refused() -> Fallible {
    let frame = first_frame()?;
    let at = marker_at(frame, 0xc4).ok_or("no DHT")?;
    let cut = frame.get(..at + 20).ok_or("short")?;
    assert!(matches!(parse(cut), Err(JpegError::Truncated { .. })));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a DC table of exactly 12 values is read; 13 are refused.
#[test]
fn a_dc_table_holds_twelve_values_and_no_more() {
    let at_bound = u8::try_from(MAX_DC_VALUES).unwrap_or(u8::MAX);
    let twelve = frame_of(&[&dc_table(at_bound)]);
    assert_eq!(
        parse(&twelve).map(|(_, used)| used),
        Err(JpegError::Missing {
            what: "frame header"
        }),
        "the table itself is read; the frame then lacks its header"
    );
    let thirteen = frame_of(&[&dc_table(at_bound + 1)]);
    assert!(matches!(
        parse(&thirteen),
        Err(JpegError::NotBaseline { segment: "DHT", .. })
    ));
}

/// Boundary: a stuffed `0xff 0x00` and a restart marker inside the scan do
/// not end it; the next other marker does.
#[test]
fn stuffed_bytes_and_restart_markers_do_not_end_a_scan() -> Fallible {
    let frame = first_frame()?;
    let (parsed, _) = parse(frame)?;
    let scan_start = frame.len() - 2 - parsed.scan_data.len();
    let mut spliced = frame.get(..scan_start).ok_or("short")?.to_vec();
    spliced.extend_from_slice(&[0xff, 0x00, 0xff, 0xd3]);
    spliced.extend_from_slice(frame.get(scan_start..).ok_or("short")?);
    let (again, used) = parse(&spliced)?;
    assert_eq!(used, spliced.len());
    assert_eq!(again.scan_data.len(), parsed.scan_data.len() + 4);
    Ok(())
}

/// Boundary: a frame of more segments than the bound is refused, never
/// read without end.
#[test]
fn more_segments_than_the_bound_are_refused() {
    let comment: &[u8] = &[0xff, 0xfe, 0x00, 0x02];
    let many = vec![comment; MAX_SEGMENTS + 1];
    assert_eq!(parse(&frame_of(&many)), Err(JpegError::TooManySegments));
}
