// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Fixtures shared by the integration tests.
//!
//! A memfd stands in for a DMA-BUF wherever the test is about the transport
//! and not about the buffer: it is a real file descriptor with a real inode
//! and a size `lseek(SEEK_END)` reads, and the attach step must refuse it
//! because its filesystem magic is not `DMA_BUF_MAGIC`. Nothing here uses
//! `unwrap` or `expect`: a fixture that cannot be built returns the failure.

#![allow(dead_code)]

use core::time::Duration;
use std::os::fd::{AsFd, OwnedFd};

use aegis_scaena::{
    CorrelationId, Deadlines, DecodedFrame, FourCc, FrameReceiver, FrameSender, Plane, Planes,
    Request, pair,
};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};

/// What every test in this suite returns.
pub type Fallible = Result<(), Box<dyn std::error::Error>>;

/// The correlation identifier the fixtures carry.
pub const CORRELATION: &str = "m27-p08-fixture-0001";

/// A frame width that is not a multiple of the chroma subsampling, so the
/// half-height and half-width rounding is exercised.
pub const WIDTH: u32 = 64;

/// The fixture frame's height.
pub const HEIGHT: u32 = 35;

/// The luma pitch the fixture uses: the width rounded up to 64 bytes.
pub const PITCH: u32 = 64;

/// Where the chroma plane starts: right after the luma plane.
pub const CHROMA_OFFSET: u32 = PITCH * HEIGHT;

/// The size of an NV12 frame of the fixture's dimensions at `PITCH`: the
/// luma plane, then the half-height chroma plane, packed.
pub const NV12_SIZE: u64 =
    (PITCH as u64) * (HEIGHT as u64) + (PITCH as u64) * (HEIGHT.div_ceil(2) as u64);

/// The deadlines the socket fixtures run under: short, so a deadline case
/// finishes quickly, and well above scheduling noise.
pub const DEADLINES: Deadlines = Deadlines {
    receive: Duration::from_millis(200),
    send: Duration::from_millis(200),
};

/// A memfd named `name` of exactly `size` bytes.
///
/// # Errors
///
/// Returns why the memfd could not be created or sized.
pub fn memfd(name: &str, size: u64) -> Result<OwnedFd, Box<dyn std::error::Error>> {
    let fd = memfd_create(name, MemfdFlags::CLOEXEC)?;
    ftruncate(&fd, size)?;
    Ok(fd)
}

/// An NV12 descriptor of the fixture's dimensions, planes packed.
///
/// # Errors
///
/// Returns the descriptor's refusal.
pub fn nv12_frame(modifier: u64) -> Result<DecodedFrame, Box<dyn std::error::Error>> {
    let planes = Planes::new(&[
        Plane {
            offset: 0,
            pitch: PITCH,
        },
        Plane {
            offset: CHROMA_OFFSET,
            pitch: PITCH,
        },
    ])?;
    Ok(DecodedFrame::new(
        CorrelationId::parse(CORRELATION.as_bytes())?,
        FourCc::NV12,
        modifier,
        (WIDTH, HEIGHT),
        planes,
    )?)
}

/// A request carrying the fixture frame.
///
/// # Errors
///
/// Returns the descriptor's refusal.
pub fn request(id: u64) -> Result<Request, Box<dyn std::error::Error>> {
    Ok(Request {
        id,
        frame: nv12_frame(aegis_scaena::descriptor::MOD_LINEAR)?,
    })
}

/// A connected pair under the fixture deadlines.
///
/// # Errors
///
/// Returns the transport's refusal.
pub fn endpoints() -> Result<(FrameSender, FrameReceiver), Box<dyn std::error::Error>> {
    Ok(pair(DEADLINES)?)
}

/// How many descriptors this process holds whose `/proc/self/fd` link names
/// the memfd `name`. Unique memfd names make this immune to other tests
/// running in the same binary.
///
/// # Errors
///
/// Returns why `/proc/self/fd` could not be read.
pub fn open_memfds_named(name: &str) -> Result<usize, Box<dyn std::error::Error>> {
    let target = format!("/memfd:{name} (deleted)");
    let mut count = 0_usize;
    for entry in std::fs::read_dir("/proc/self/fd")?.take(65_536) {
        let Ok(link) = std::fs::read_link(entry?.path()) else {
            continue;
        };
        if link.as_os_str() == target.as_str() {
            count = count.saturating_add(1);
        }
    }
    Ok(count)
}

/// Borrows `fd` for a send.
#[must_use]
pub fn borrow(fd: &OwnedFd) -> std::os::fd::BorrowedFd<'_> {
    fd.as_fd()
}
