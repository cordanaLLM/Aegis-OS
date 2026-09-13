// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-4, the buffer half: the descriptor bound and the stride
//! arithmetic.
//!
//! Positive: 64 descriptors are accepted and each row reads back with the
//! stride its format implies. Negative: the 65th is refused, and the
//! scaffold's own four-character code is refused rather than carried.
//! Boundary: the stride is checked at width 0 and at `u32::MAX / 4`, which is
//! the last width a packed 32-bit stride fits in, and at one pixel past it.

mod common;

use aegis_calliope::{
    BufferHandle, BufferId, CalliopeError, DmaBufFrame, DmaBufTable, FourCc, FrameGeometry,
    MAX_DMA_BUFFERS, PixelFormat, SCAFFOLD_FOURCC, stride_bytes,
};

use common::{CAPTURE_HEIGHT, CAPTURE_WIDTH, Fallible, filled_table};

/// The largest width whose packed 32-bit stride still fits in `u32`.
const LAST_PACKED_WIDTH: u32 = u32::MAX / 4;

// --- Positive -------------------------------------------------------------

/// Positive: the table admits its full complement of descriptors.
#[test]
fn the_table_admits_sixty_four_descriptors() -> Fallible {
    let table = filled_table(MAX_DMA_BUFFERS)?;
    assert_eq!(table.count(), MAX_DMA_BUFFERS);
    assert_eq!(MAX_DMA_BUFFERS, 64);
    assert!(!table.is_empty());
    Ok(())
}

/// Positive: a registered row reads back with the stride its format implies.
#[test]
fn a_registered_row_reads_back_with_its_stride() -> Fallible {
    let mut table = DmaBufTable::new();
    assert!(table.is_empty());
    let geometry = FrameGeometry::new(CAPTURE_WIDTH, CAPTURE_HEIGHT);
    let id = table.register(geometry, PixelFormat::Argb8888)?;
    let frame: DmaBufFrame = table
        .get(id)
        .ok_or("the table must hold what it registered")?;
    let fields = (
        frame.buffer_id,
        frame.handle,
        frame.geometry,
        frame.format,
        frame.stride_bytes,
    );
    assert_eq!(
        fields,
        (
            id,
            BufferHandle::new(0),
            geometry,
            PixelFormat::Argb8888,
            CAPTURE_WIDTH.saturating_mul(4),
        )
    );
    assert_eq!(BufferId::new(0), id);
    Ok(())
}

/// Positive: the geometry a row carries is the one it was registered with,
/// and its stride is the one the geometry itself computes.
#[test]
fn a_registered_geometry_computes_its_own_stride() -> Fallible {
    let mut table = DmaBufTable::new();
    let geometry = FrameGeometry::new(CAPTURE_WIDTH, CAPTURE_HEIGHT);
    let id = table.register(geometry, PixelFormat::Argb8888)?;
    let frame = table
        .get(id)
        .ok_or("the table must hold what it registered")?;
    assert_eq!(
        (frame.geometry.width, frame.geometry.height),
        (CAPTURE_WIDTH, CAPTURE_HEIGHT)
    );
    assert_eq!(
        frame.stride_bytes,
        geometry.stride_bytes(PixelFormat::Argb8888)?
    );
    Ok(())
}

/// Positive: the two admitted formats differ in exactly the way the recorded
/// conflict is about -- their bytes per pixel, and therefore their stride.
#[test]
fn the_two_admitted_formats_have_different_strides() -> Fallible {
    assert_eq!(PixelFormat::Argb8888.bytes_per_pixel(), 4);
    assert_eq!(PixelFormat::Nv12.bytes_per_pixel(), 1);
    assert_eq!(stride_bytes(PixelFormat::Argb8888, 1920)?, 7680);
    assert_eq!(stride_bytes(PixelFormat::Nv12, 1920)?, 1920);
    for format in PixelFormat::ALL {
        assert_eq!(PixelFormat::from_tag(format.tag()), Some(format));
        assert_eq!(PixelFormat::from_fourcc(format.fourcc())?, format);
    }
    assert_eq!(PixelFormat::ALL.len(), 2);
    assert_ne!(PixelFormat::Argb8888.fourcc(), PixelFormat::Nv12.fourcc());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the 65th descriptor is refused with the bound it hit.
#[test]
fn the_sixty_fifth_descriptor_is_refused() -> Fallible {
    let mut table = filled_table(MAX_DMA_BUFFERS)?;
    let refusal = table.register(
        FrameGeometry::new(CAPTURE_WIDTH, CAPTURE_HEIGHT),
        PixelFormat::Argb8888,
    );
    assert_eq!(
        refusal,
        Err(CalliopeError::BufferTableFull {
            max: MAX_DMA_BUFFERS
        })
    );
    assert_eq!(table.count(), MAX_DMA_BUFFERS);
    Ok(())
}

/// Negative: the scaffold's own four-character code names no admitted format.
///
/// The recorded conflict, as a refusal: `0x34325641` spells `AV24`, which is
/// neither the `NV12` nor the `AR24` the scaffold's comment names.
#[test]
fn the_scaffold_four_character_code_is_refused() {
    assert_eq!(SCAFFOLD_FOURCC.get(), 0x3432_5641);
    assert_eq!(SCAFFOLD_FOURCC.tag_bytes(), *b"AV24");
    assert_eq!(SCAFFOLD_FOURCC.to_string(), "AV24");
    assert_ne!(SCAFFOLD_FOURCC, PixelFormat::Nv12.fourcc());
    assert_ne!(SCAFFOLD_FOURCC, PixelFormat::Argb8888.fourcc());
    assert_eq!(
        PixelFormat::from_fourcc(SCAFFOLD_FOURCC),
        Err(CalliopeError::UnknownPixelFormat {
            fourcc: SCAFFOLD_FOURCC
        })
    );
    assert_eq!(PixelFormat::from_tag("av24"), None);
}

/// Negative: the two codes the comment names spell what the standard spells,
/// and a code carrying a byte no terminal can print renders it as `?` rather
/// than losing it.
#[test]
fn the_admitted_codes_spell_their_standard_names() {
    assert_eq!(PixelFormat::Argb8888.fourcc().tag_bytes(), *b"AR24");
    assert_eq!(PixelFormat::Nv12.fourcc().tag_bytes(), *b"NV12");
    assert_eq!(PixelFormat::Nv12.fourcc().to_string(), "NV12");
    assert_eq!(FourCc::new(0).to_string(), "????");
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the stride is checked at zero and at `u32::MAX / 4`.
///
/// Zero is a width, and its stride is zero rather than a refusal: a
/// zero-width frame is degenerate but its arithmetic is defined.
/// `u32::MAX / 4` is the last width whose packed stride fits, and one pixel
/// past it is [`CalliopeError::StrideOverflow`] rather than a wrapped product
/// -- which is what the scaffold's unchecked `width * 4` would produce.
#[test]
fn the_stride_is_exact_at_zero_and_at_the_packed_ceiling() -> Fallible {
    assert_eq!(stride_bytes(PixelFormat::Argb8888, 0)?, 0);
    assert_eq!(stride_bytes(PixelFormat::Nv12, 0)?, 0);
    assert_eq!(
        FrameGeometry::new(0, 0).stride_bytes(PixelFormat::Argb8888)?,
        0
    );

    assert_eq!(LAST_PACKED_WIDTH, 1_073_741_823);
    assert_eq!(
        stride_bytes(PixelFormat::Argb8888, LAST_PACKED_WIDTH)?,
        4_294_967_292
    );
    assert_eq!(
        stride_bytes(PixelFormat::Argb8888, LAST_PACKED_WIDTH.saturating_add(1)),
        Err(CalliopeError::StrideOverflow {
            width: LAST_PACKED_WIDTH.saturating_add(1),
            bytes_per_pixel: 4,
        })
    );
    assert_eq!(
        stride_bytes(PixelFormat::Argb8888, u32::MAX),
        Err(CalliopeError::StrideOverflow {
            width: u32::MAX,
            bytes_per_pixel: 4,
        })
    );

    // The planar format has no ceiling below u32::MAX, because its stride is
    // the width. Stating that is what keeps the bound a property of the
    // format rather than a constant someone copied.
    assert_eq!(stride_bytes(PixelFormat::Nv12, u32::MAX)?, u32::MAX);
    Ok(())
}

/// Boundary: a table refuses a descriptor whose stride would overflow, and
/// stores nothing when it does.
#[test]
fn a_table_refuses_an_overflowing_descriptor_and_stores_nothing() {
    let mut table = DmaBufTable::new();
    let refusal = table.register(FrameGeometry::new(u32::MAX, 1), PixelFormat::Argb8888);
    assert_eq!(
        refusal,
        Err(CalliopeError::StrideOverflow {
            width: u32::MAX,
            bytes_per_pixel: 4,
        })
    );
    assert_eq!(table.count(), 0);
    assert!(table.is_empty());
    assert_eq!(table.get(BufferId::new(0)), None);
}
