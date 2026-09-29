// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M27 criterion 5 and E27-2: decode is checked against content, not exit
//! status. These cases hold the check itself on planes built here, so they
//! run without a GPU inside `make verify-all`; `make verify-display` runs the
//! same functions on the surfaces the Arc A380 decodes.
//!
//! Positive: every frame number 1 to 60 drawn as the fixture draws it reads
//! back, with a pitch wider than the row. Negative: a zero-filled plane fails
//! the white guard, an all-white plane fails the black guard, a frame read
//! out of order fails with both numbers named, and a plane too small for the
//! row is refused. Boundary: a block mean of exactly 128 reads as white and
//! 127 as black; CRC-32 has the ISO-HDLC check value; the NV12 checksum reads
//! visible lines only, so padding does not change it and a visible byte does.

mod common;

use aegis_scaena::content::{
    BLOCK, ContentError, Crc32, FIXTURE_FRAMES, FIXTURE_HEIGHT, FIXTURE_WIDTH, LumaPlane,
    Nv12Layout, PAYLOAD_BITS, ROW_BLOCKS, ROW_X, ROW_Y, check_index, crc_nv12, read_index,
    row_means,
};

use common::Fallible;

/// A pitch wider than the fixture's width, as a tiled or aligned surface has.
const PITCH: usize = 384;

/// A luma plane of the fixture's height at [`PITCH`], filled with `value`.
fn plane(value: u8) -> Vec<u8> {
    vec![value; PITCH * FIXTURE_HEIGHT as usize]
}

/// Fills the 16 x 16 block `block` of the row with `value`.
fn fill_block(bytes: &mut [u8], block: usize, value: u8) {
    let left = block.saturating_mul(BLOCK).saturating_add(ROW_X);
    for row in (ROW_Y..).take(BLOCK) {
        let start = row.saturating_mul(PITCH).saturating_add(left);
        if let Some(line) = bytes.get_mut(start..start.saturating_add(BLOCK)) {
            line.fill(value);
        }
    }
}

/// A plane with frame number `number` drawn as the fixture draws it: white
/// guard, six bits most significant first, black guard.
fn drawn(number: u32) -> Vec<u8> {
    let mut bytes = plane(0x80);
    fill_block(&mut bytes, 0, 235);
    for (block, shift) in (1..).zip((0..PAYLOAD_BITS).rev()) {
        let shift = u32::try_from(shift).unwrap_or(u32::MAX);
        let set = number.checked_shr(shift).unwrap_or(0) & 1 == 1;
        fill_block(&mut bytes, block, if set { 235 } else { 16 });
    }
    fill_block(&mut bytes, ROW_BLOCKS - 1, 16);
    bytes
}

/// The plane as the check reads it.
fn luma(bytes: &[u8]) -> LumaPlane<'_> {
    LumaPlane {
        bytes,
        offset: 0,
        pitch: PITCH,
    }
}

// --- Positive -------------------------------------------------------------

/// Positive: every frame number the fixture draws reads back.
#[test]
fn every_drawn_frame_number_reads_back() -> Fallible {
    let frames = u32::try_from(FIXTURE_FRAMES)?;
    for number in 1..=frames {
        check_index(luma(&drawn(number)), number)?;
    }
    assert_eq!((FIXTURE_WIDTH, FIXTURE_HEIGHT), (256, 64));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a zero-filled surface fails the white guard, which is what the
/// planted surface on the reference profile must do.
#[test]
fn a_zero_filled_plane_fails_the_white_guard() {
    assert_eq!(
        read_index(luma(&plane(0))),
        Err(ContentError::WhiteGuard { mean: 0 })
    );
}

/// Negative: an all-white plane fails the black guard.
#[test]
fn an_all_white_plane_fails_the_black_guard() {
    assert_eq!(
        read_index(luma(&plane(255))),
        Err(ContentError::BlackGuard { mean: 255 })
    );
}

/// Negative: a frame out of order is refused naming both numbers.
#[test]
fn a_frame_out_of_order_is_refused() {
    assert_eq!(
        check_index(luma(&drawn(6)), 5),
        Err(ContentError::Index {
            expected: 5,
            found: 6
        })
    );
}

/// Negative: a plane too small for the row is refused, not read past.
#[test]
fn a_plane_too_small_for_the_row_is_refused() {
    let short = vec![255_u8; PITCH * (ROW_Y + BLOCK - 1)];
    assert_eq!(read_index(luma(&short)), Err(ContentError::Layout));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a block mean of exactly 128 reads as white, 127 as black.
#[test]
fn the_threshold_is_inclusive_at_128() -> Fallible {
    let mut bytes = drawn(1);
    fill_block(&mut bytes, ROW_BLOCKS - 2, 128);
    assert_eq!(row_means(luma(&bytes))?.get(ROW_BLOCKS - 2), Some(&128));
    assert_eq!(read_index(luma(&bytes))?, 1);
    fill_block(&mut bytes, ROW_BLOCKS - 2, 127);
    assert_eq!(read_index(luma(&bytes))?, 0);
    Ok(())
}

/// Boundary: CRC-32 as ISO-HDLC defines it, fed whole or in pieces.
#[test]
fn crc32_has_the_iso_hdlc_check_value() {
    let mut whole = Crc32::new();
    whole.update(b"123456789");
    assert_eq!(whole.finish(), 0xcbf4_3926);
    let mut pieces = Crc32::new();
    pieces.update(b"1234");
    pieces.update(b"56789");
    assert_eq!(pieces.finish(), 0xcbf4_3926);
    assert_eq!(Crc32::new().finish(), 0);
}

/// Boundary: the NV12 checksum covers visible lines only.
#[test]
fn the_nv12_checksum_skips_padding() -> Fallible {
    let layout = Nv12Layout {
        offsets: [0, PITCH * 16],
        pitches: [PITCH, PITCH],
        width: 16,
        height: 16,
    };
    let mut bytes = vec![7_u8; PITCH * 24];
    let base = crc_nv12(&bytes, layout)?;
    if let Some(padding) = bytes.get_mut(16) {
        *padding = 9;
    }
    assert_eq!(crc_nv12(&bytes, layout)?, base, "padding is not hashed");
    if let Some(visible) = bytes.get_mut(PITCH * 16 + 3) {
        *visible = 9;
    }
    assert_ne!(crc_nv12(&bytes, layout)?, base, "a visible chroma byte is");
    assert_eq!(
        crc_nv12(bytes.get(..PITCH * 20).unwrap_or_default(), layout),
        Err(ContentError::Layout)
    );
    Ok(())
}
