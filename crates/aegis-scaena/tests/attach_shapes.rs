// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The attach step's building blocks on their own: the plane shapes a format
//! implies, the filesystem magic an fd reports, and the errno a failed call
//! carries.
//!
//! Positive: NV12 at 1920 x 1080 is a full-size luma plane and a half-height
//! chroma plane of the same row length; XRGB8888 and ARGB8888 are one plane
//! of four bytes a pixel; a memfd reports tmpfs's magic. Negative: a format
//! this crate does not know has no shape; a socket and a pipe report their
//! own magic, which `require_dma_buf` names in its refusal; a seek on a pipe
//! fails with `ESPIPE`. Boundary: odd NV12 sizes round the chroma plane up,
//! a zero-sized frame has empty planes, the widest frame does not overflow,
//! and the highest errno is carried unchanged.

mod common;

use std::os::fd::AsFd;
use std::os::unix::net::UnixStream;

use aegis_scaena::attach::{
    PlaneShape, filesystem_magic, object_size, plane_shapes, require_dma_buf,
};
use aegis_scaena::error::errno;
use aegis_scaena::{AttachError, DMA_BUF_MAGIC, FourCc};
use rustix::io::Errno;

use common::{Fallible, memfd};

/// `TMPFS_MAGIC` (`include/uapi/linux/magic.h`): where a memfd lives.
const TMPFS_MAGIC: u64 = 0x0102_1994;

/// `SOCKFS_MAGIC` (`include/uapi/linux/magic.h`).
const SOCKFS_MAGIC: u64 = 0x534f_434b;

/// `PIPEFS_MAGIC` (`include/uapi/linux/magic.h`).
const PIPEFS_MAGIC: u64 = 0x5049_5045;

/// `ESPIPE` on Linux (`include/uapi/asm-generic/errno-base.h`).
const ESPIPE: i32 = 29;

/// `MAX_ERRNO` (`include/linux/err.h`): the highest errno the kernel returns.
const MAX_ERRNO: i32 = 4_095;

/// A plane shape from its rows and row length.
const fn shape(rows: u64, row_bytes: u64) -> PlaneShape {
    PlaneShape { rows, row_bytes }
}

/// The empty second plane of a single-plane format.
const EMPTY: PlaneShape = shape(0, 0);

// --- Positive -------------------------------------------------------------

/// Positive: NV12 at 1920 x 1080 is 1080 rows of 1920 bytes of luma and 540
/// rows of 960 interleaved chroma pairs, 1920 bytes.
#[test]
fn nv12_is_a_luma_plane_and_a_half_height_chroma_plane() {
    let expected = ([shape(1080, 1920), shape(540, 1920)], 2);
    assert_eq!(plane_shapes(FourCc::NV12, 1920, 1080), Some(expected));
}

/// Positive: XRGB8888 and ARGB8888 at 3 x 2 are one plane of 2 rows of 12
/// bytes, and no second plane.
#[test]
fn packed_rgb_is_one_plane_of_four_bytes_a_pixel() {
    for fourcc in [FourCc::XRGB8888, FourCc::ARGB8888] {
        let expected = ([shape(2, 12), EMPTY], 1);
        assert_eq!(plane_shapes(fourcc, 3, 2), Some(expected), "{fourcc}");
    }
}

/// Positive: a memfd lives on tmpfs, and the magic says so.
#[test]
fn a_memfd_reports_the_tmpfs_magic() -> Fallible {
    let object = memfd("scaena-magic-tmpfs", 64)?;
    assert_eq!(filesystem_magic(object.as_fd())?, TMPFS_MAGIC);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a format outside NV12, XRGB8888 and ARGB8888 has no shape, so
/// the layout check refuses it.
#[test]
fn an_unknown_format_has_no_shape() -> Fallible {
    for code in [b"YUYV", b"XB24", b"NV21"] {
        assert_eq!(plane_shapes(FourCc::parse(code)?, 4, 4), None);
    }
    Ok(())
}

/// Negative: a socket and a pipe report their own magic, and the DMA-BUF
/// check refuses each naming exactly that magic.
#[test]
fn a_socket_and_a_pipe_report_their_own_magic() -> Fallible {
    let (socket, _peer) = UnixStream::pair()?;
    let (pipe, _writer) = std::io::pipe()?;
    for (fd, expected) in [(socket.as_fd(), SOCKFS_MAGIC), (pipe.as_fd(), PIPEFS_MAGIC)] {
        let magic = filesystem_magic(fd)?;
        assert_eq!(magic, expected);
        assert_ne!(magic, DMA_BUF_MAGIC);
        assert_eq!(require_dma_buf(fd), Err(AttachError::NotDmaBuf { magic }));
    }
    Ok(())
}

/// Negative: a pipe cannot seek, and the failure carries `ESPIPE`.
#[test]
fn a_failed_seek_carries_its_errno() -> Fallible {
    let (pipe, _writer) = std::io::pipe()?;
    let expected = AttachError::Io { errno: ESPIPE };
    assert_eq!(object_size(pipe.as_fd()), Err(expected));
    assert_eq!(errno(Errno::SPIPE), ESPIPE);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: NV12 at 5 x 3 rounds the chroma plane up to 2 rows of 3 pairs,
/// 6 bytes; at 0 x 0 both planes are empty.
#[test]
fn odd_and_zero_nv12_sizes_round_up() {
    let odd = ([shape(3, 5), shape(2, 6)], 2);
    assert_eq!(plane_shapes(FourCc::NV12, 5, 3), Some(odd));
    assert_eq!(plane_shapes(FourCc::NV12, 0, 0), Some(([EMPTY, EMPTY], 2)));
    assert_eq!(
        plane_shapes(FourCc::XRGB8888, 0, 0),
        Some(([EMPTY, EMPTY], 1))
    );
}

/// Boundary: the widest frame a `u32` names does not overflow: 4294967295
/// pixels of four bytes are 17179869180 bytes, and an NV12 row of that width
/// rounds up to 4294967296 bytes of chroma.
#[test]
fn the_widest_frame_does_not_overflow() {
    let packed = ([shape(1, 17_179_869_180), EMPTY], 1);
    assert_eq!(plane_shapes(FourCc::XRGB8888, u32::MAX, 1), Some(packed));
    let nv12 = ([shape(1, 4_294_967_295), shape(1, 4_294_967_296)], 2);
    assert_eq!(plane_shapes(FourCc::NV12, u32::MAX, 1), Some(nv12));
}

/// Boundary: the lowest and the highest errno pass through unchanged.
#[test]
fn errno_carries_the_raw_value_at_both_ends() {
    assert_eq!(errno(Errno::PERM), 1);
    assert_eq!(errno(Errno::from_raw_os_error(MAX_ERRNO)), MAX_ERRNO);
}
