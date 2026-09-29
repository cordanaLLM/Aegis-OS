// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E27-1 and E27-2: what the attach step checks before anything is attached.
//!
//! Positive: an NV12 descriptor whose planes fit an object of the size
//! `lseek(SEEK_END)` reads passes the layout check, and an advertised format
//! and modifier pair is admitted. Negative: a pair the compositor did not
//! advertise, such as NV12 with `INTEL_4_TILED_DG2_RC_CCS`, is refused
//! client-side; a plane count that is not the format's, a pitch below a row,
//! an unknown format and an fd that is not a DMA-BUF are refused. Boundary: a
//! plane ending exactly at the end of its object is accepted and one byte
//! further refused; the advertised table holds its bound and no more.

mod common;

use std::os::fd::AsFd;

use aegis_scaena::attach::{MAX_FORMAT_PAIRS, check_layout, object_size, require_dma_buf};
use aegis_scaena::descriptor::{MOD_INTEL_4_TILED, MOD_INTEL_4_TILED_DG2_RC_CCS, MOD_LINEAR};
use aegis_scaena::{AttachError, DecodedFrame, FormatTable, FourCc, Plane, Planes};

use common::{CHROMA_OFFSET, Fallible, HEIGHT, NV12_SIZE, PITCH, memfd, nv12_frame};

/// The fixture frame with its planes replaced.
fn with_planes(planes: &[Plane]) -> Result<DecodedFrame, Box<dyn std::error::Error>> {
    Ok(DecodedFrame {
        planes: Planes::new(planes)?,
        ..nv12_frame(MOD_LINEAR)?
    })
}

// --- Positive -------------------------------------------------------------

/// Positive: the fixture's packed NV12 layout fits an object of exactly its
/// size, read back with `lseek(SEEK_END)` from a memfd of that size.
#[test]
fn a_packed_nv12_frame_fits_its_object() -> Fallible {
    let object = memfd("scaena-layout-fit", NV12_SIZE)?;
    let size = object_size(object.as_fd())?;
    assert_eq!(size, NV12_SIZE);
    check_layout(&nv12_frame(MOD_LINEAR)?, size)?;
    Ok(())
}

/// Positive: an advertised pair is admitted, and the table keeps a repeated
/// advertisement once.
#[test]
fn an_advertised_pair_is_admitted() -> Fallible {
    let mut table = FormatTable::new();
    for modifier in [MOD_LINEAR, MOD_INTEL_4_TILED, MOD_INTEL_4_TILED] {
        table.advertise(FourCc::NV12, modifier)?;
    }
    assert_eq!(table.len(), 2);
    table.admit(FourCc::NV12, MOD_INTEL_4_TILED)?;
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: NV12 with `INTEL_4_TILED_DG2_RC_CCS`, which the reference
/// compositor does not advertise for NV12, is refused client-side, while the
/// next advertised pair is still admitted.
#[test]
fn an_unadvertised_pair_is_refused_client_side() -> Fallible {
    let mut table = FormatTable::new();
    table.advertise(FourCc::NV12, MOD_INTEL_4_TILED)?;
    table.advertise(FourCc::ARGB8888, MOD_INTEL_4_TILED_DG2_RC_CCS)?;
    assert_eq!(
        table.admit(FourCc::NV12, MOD_INTEL_4_TILED_DG2_RC_CCS),
        Err(AttachError::NotAdvertised {
            fourcc: FourCc::NV12,
            modifier: MOD_INTEL_4_TILED_DG2_RC_CCS
        })
    );
    table.admit(FourCc::NV12, MOD_INTEL_4_TILED)?;
    Ok(())
}

/// Negative: a plane count that is not the format's, a pitch below the bytes
/// of a row and a format with no known layout are refused.
#[test]
fn layouts_the_format_contradicts_are_refused() -> Fallible {
    let one = with_planes(&[Plane {
        offset: 0,
        pitch: PITCH,
    }])?;
    assert_eq!(
        check_layout(&one, NV12_SIZE),
        Err(AttachError::PlaneLayout {
            expected: 2,
            listed: 1
        })
    );
    let narrow = with_planes(&[
        Plane {
            offset: 0,
            pitch: PITCH.saturating_sub(1),
        },
        Plane {
            offset: CHROMA_OFFSET,
            pitch: PITCH,
        },
    ])?;
    assert_eq!(
        check_layout(&narrow, NV12_SIZE),
        Err(AttachError::PitchTooSmall { plane: 0 })
    );
    let unknown = DecodedFrame {
        fourcc: FourCc::parse(b"YUYV")?,
        ..nv12_frame(MOD_LINEAR)?
    };
    assert!(matches!(
        check_layout(&unknown, NV12_SIZE),
        Err(AttachError::UnsupportedFormat { .. })
    ));
    Ok(())
}

/// Negative: a memfd is not a DMA-BUF.
#[test]
fn a_memfd_is_not_a_dma_buf() -> Fallible {
    let object = memfd("scaena-not-dma-buf-direct", 64)?;
    assert!(matches!(
        require_dma_buf(object.as_fd()),
        Err(AttachError::NotDmaBuf { .. })
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the chroma plane ending exactly at the end of the object is
/// accepted; moved one byte further, it is refused, naming where it ends.
#[test]
fn a_plane_ending_at_the_object_end_is_accepted_and_one_byte_further_refused() -> Fallible {
    let object = memfd("scaena-layout-edge", NV12_SIZE)?;
    let size = object_size(object.as_fd())?;
    let chroma_rows = u64::from(HEIGHT.div_ceil(2));
    let end = u64::from(CHROMA_OFFSET)
        .checked_add(
            u64::from(PITCH)
                .checked_mul(chroma_rows)
                .ok_or("overflow")?,
        )
        .ok_or("overflow")?;
    assert_eq!(
        end, size,
        "the fixture's chroma plane ends at the object's end"
    );
    check_layout(&nv12_frame(MOD_LINEAR)?, size)?;
    let shifted = with_planes(&[
        Plane {
            offset: 0,
            pitch: PITCH,
        },
        Plane {
            offset: CHROMA_OFFSET.saturating_add(1),
            pitch: PITCH,
        },
    ])?;
    assert_eq!(
        check_layout(&shifted, size),
        Err(AttachError::PlaneOutOfBounds {
            plane: 1,
            end: end.saturating_add(1),
            size
        })
    );
    Ok(())
}

/// Boundary: the advertised table holds `MAX_FORMAT_PAIRS` distinct pairs
/// and refuses one more.
#[test]
fn the_format_table_holds_its_bound_and_no_more() -> Fallible {
    let mut table = FormatTable::new();
    let bound = u64::try_from(MAX_FORMAT_PAIRS)?;
    for modifier in 0..bound {
        table.advertise(FourCc::NV12, modifier)?;
    }
    assert_eq!(table.len(), MAX_FORMAT_PAIRS);
    assert_eq!(
        table.advertise(FourCc::NV12, bound),
        Err(AttachError::TableFull)
    );
    table.advertise(FourCc::NV12, 0)?;
    Ok(())
}
