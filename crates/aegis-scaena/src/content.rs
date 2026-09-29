// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Decode checked against content, not exit status (M27 criterion 5).
//!
//! The committed Motion-JPEG fixture draws each frame's number, 1 to 60, as a
//! row of black and white 16 x 16 luma blocks on the MCU grid: one white guard
//! block, six bit blocks, most significant first, and one black guard block,
//! starting at [`ROW_X`], [`ROW_Y`]. [`read_index`] reads the row back from a
//! decoded luma plane, thresholding each block's mean luma at [`THRESHOLD`],
//! so a blank frame, a frame out of order, a frame whose blocks do not read
//! back and a zero-filled surface all fail. The expected number comes from
//! the fixture's generator, never from an earlier run of the same decoder.
//!
//! [`Crc32`] is CRC-32 as ISO-HDLC defines it (the polynomial `0x04c11db7`,
//! reflected, check value `0xcbf43926` for `123456789`), the one
//! `crc32fast` computes in cros-libva's own test, so the reference MPEG-2
//! frame is held to the value cros-libva asserts at the pinned revision.
//! [`crc_nv12`] covers the visible NV12 lines only, never a pitch's padding.

/// A block's edge in pixels: one 4:2:0 MCU.
pub const BLOCK: usize = 16;

/// Where the row starts, in pixels from the left.
pub const ROW_X: usize = 16;

/// Where the row starts, in rows from the top.
pub const ROW_Y: usize = 16;

/// The blocks in the row: a white guard, six bits and a black guard.
pub const ROW_BLOCKS: usize = 8;

/// The bits the frame number is drawn in.
pub const PAYLOAD_BITS: usize = 6;

/// A block whose mean luma is at or above this reads as white.
pub const THRESHOLD: u32 = 128;

/// The fixture's frame width.
pub const FIXTURE_WIDTH: u32 = 256;

/// The fixture's frame height.
pub const FIXTURE_HEIGHT: u32 = 64;

/// The fixture's frame count.
pub const FIXTURE_FRAMES: usize = 60;

/// Why a decoded frame's content was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum ContentError {
    /// The plane is too small for the row or its layout does not fit.
    #[error("the luma plane does not hold the index row")]
    Layout,
    /// The white guard block read as black.
    #[error("the white guard block's mean luma is {mean}, below {THRESHOLD}")]
    WhiteGuard {
        /// Its mean luma.
        mean: u32,
    },
    /// The black guard block read as white.
    #[error("the black guard block's mean luma is {mean}, at or above {THRESHOLD}")]
    BlackGuard {
        /// Its mean luma.
        mean: u32,
    },
    /// The blocks read back another number than the fixture drew.
    #[error("the blocks read back frame number {found}; the fixture drew {expected}")]
    Index {
        /// What the fixture drew.
        expected: u32,
        /// What was read.
        found: u32,
    },
}

/// A luma plane in a mapped image: its bytes, where it starts and its pitch.
#[derive(Debug, Clone, Copy)]
pub struct LumaPlane<'a> {
    /// The mapped image.
    pub bytes: &'a [u8],
    /// The plane's first byte.
    pub offset: usize,
    /// Bytes from one row to the next.
    pub pitch: usize,
}

/// The mean luma of the 16 x 16 block whose top-left pixel is `x`, `y`.
fn block_mean(plane: LumaPlane<'_>, x: usize, y: usize) -> Result<u32, ContentError> {
    let mut sum = 0_u32;
    for row in 0..BLOCK {
        let start = y
            .checked_add(row)
            .and_then(|line| line.checked_mul(plane.pitch))
            .and_then(|bytes| bytes.checked_add(plane.offset))
            .and_then(|bytes| bytes.checked_add(x))
            .ok_or(ContentError::Layout)?;
        let end = start.checked_add(BLOCK).ok_or(ContentError::Layout)?;
        let line = plane.bytes.get(start..end).ok_or(ContentError::Layout)?;
        let line_sum = line.iter().map(|value| u32::from(*value)).sum::<u32>();
        sum = sum.saturating_add(line_sum);
    }
    let area = u32::try_from(BLOCK.saturating_mul(BLOCK)).map_err(|_| ContentError::Layout)?;
    Ok(sum.checked_div(area).unwrap_or_default())
}

/// The mean luma of every block in the row, left to right.
///
/// # Errors
///
/// Returns [`ContentError::Layout`] when the plane does not hold the row.
pub fn row_means(plane: LumaPlane<'_>) -> Result<[u32; ROW_BLOCKS], ContentError> {
    let mut means = [0_u32; ROW_BLOCKS];
    for (index, mean) in means.iter_mut().enumerate() {
        let x = index
            .checked_mul(BLOCK)
            .and_then(|offset| offset.checked_add(ROW_X))
            .ok_or(ContentError::Layout)?;
        *mean = block_mean(plane, x, ROW_Y)?;
    }
    Ok(means)
}

/// The frame number the row's blocks carry, after both guards read back.
///
/// # Errors
///
/// Returns [`ContentError::Layout`], [`ContentError::WhiteGuard`] or
/// [`ContentError::BlackGuard`].
pub fn read_index(plane: LumaPlane<'_>) -> Result<u32, ContentError> {
    let means = row_means(plane)?;
    let (Some(white), Some(black)) = (means.first(), means.last()) else {
        return Err(ContentError::Layout);
    };
    if *white < THRESHOLD {
        return Err(ContentError::WhiteGuard { mean: *white });
    }
    if *black >= THRESHOLD {
        return Err(ContentError::BlackGuard { mean: *black });
    }
    let bits = means.iter().skip(1).take(PAYLOAD_BITS);
    Ok(bits.fold(0_u32, |value, mean| {
        (value << 1) | u32::from(*mean >= THRESHOLD)
    }))
}

/// Refuses a frame whose blocks do not read back `expected`.
///
/// # Errors
///
/// As [`read_index`], and [`ContentError::Index`] for another number.
pub fn check_index(plane: LumaPlane<'_>, expected: u32) -> Result<(), ContentError> {
    let found = read_index(plane)?;
    if found == expected {
        Ok(())
    } else {
        Err(ContentError::Index { expected, found })
    }
}

/// CRC-32 (ISO-HDLC), fed in pieces.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Crc32 {
    state: u32,
}

impl Crc32 {
    /// The reflected polynomial.
    const POLYNOMIAL: u32 = 0xedb8_8320;

    /// A fresh checksum.
    #[must_use]
    pub const fn new() -> Self {
        Self { state: u32::MAX }
    }

    /// Feeds `bytes`.
    pub fn update(&mut self, bytes: &[u8]) {
        for byte in bytes {
            self.state ^= u32::from(*byte);
            for _ in 0..8 {
                let mask = (self.state & 1).wrapping_neg();
                self.state = (self.state >> 1) ^ (Self::POLYNOMIAL & mask);
            }
        }
    }

    /// The checksum of everything fed.
    #[must_use]
    pub const fn finish(self) -> u32 {
        !self.state
    }
}

impl Default for Crc32 {
    fn default() -> Self {
        Self::new()
    }
}

/// An NV12 image as it is mapped: both planes' offsets and pitches.
#[derive(Debug, Clone, Copy)]
pub struct Nv12Layout {
    /// Where the luma and the chroma plane start.
    pub offsets: [usize; 2],
    /// Their pitches.
    pub pitches: [usize; 2],
    /// Visible width in pixels.
    pub width: usize,
    /// Visible height in rows.
    pub height: usize,
}

/// Feeds `rows` rows of `width` bytes each, `pitch` apart from `offset`.
fn feed_rows(
    crc: &mut Crc32,
    bytes: &[u8],
    (offset, pitch): (usize, usize),
    (width, rows): (usize, usize),
) -> Option<()> {
    for row in 0..rows {
        let start = row.checked_mul(pitch)?.checked_add(offset)?;
        crc.update(bytes.get(start..start.checked_add(width)?)?);
    }
    Some(())
}

/// The CRC-32 of an NV12 image's visible lines: every luma row, then every
/// chroma row, each `width` bytes, the way cros-libva's `crc_nv12_image`
/// computes it.
///
/// # Errors
///
/// Returns [`ContentError::Layout`] when a line falls outside `bytes`.
pub fn crc_nv12(bytes: &[u8], layout: Nv12Layout) -> Result<u32, ContentError> {
    let [luma_offset, chroma_offset] = layout.offsets;
    let [luma_pitch, chroma_pitch] = layout.pitches;
    let mut crc = Crc32::new();
    feed_rows(
        &mut crc,
        bytes,
        (luma_offset, luma_pitch),
        (layout.width, layout.height),
    )
    .ok_or(ContentError::Layout)?;
    feed_rows(
        &mut crc,
        bytes,
        (chroma_offset, chroma_pitch),
        (layout.width, layout.height.div_ceil(2)),
    )
    .ok_or(ContentError::Layout)?;
    Ok(crc.finish())
}
