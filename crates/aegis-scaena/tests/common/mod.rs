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

use core::mem::MaybeUninit;
use core::time::Duration;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use aegis_scaena::{
    CorrelationId, Cue, Deadlines, DecodedFrame, FourCc, FrameReceiver, FrameSender, LineBuffer,
    LineError, MAX_LINE_BYTES, Plane, Planes, Request, pair,
};
use rustix::fs::{MemfdFlags, ftruncate, memfd_create};
use rustix::io::IoSlice;
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};

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

/// `text` with one newline appended.
#[must_use]
pub fn line(text: &str) -> Vec<u8> {
    format!("{text}\n").into_bytes()
}

/// Encodes `item` into a `LineBuffer` and returns the UTF-8 payload without trailing newline.
///
/// # Errors
///
/// Returns an error if encoding or UTF-8 decoding fails.
pub fn request_line_string<T>(
    item: &T,
    encode: impl for<'a> FnOnce(&T, &'a mut LineBuffer) -> Result<&'a [u8], LineError>,
) -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = LineBuffer::new();
    let line = encode(item, &mut buffer)?;
    Ok(String::from_utf8(aegis_scaena::line::body(line)?.to_vec())?)
}

/// Boundary check: a line of exactly `MAX_LINE_BYTES`, newline included, is
/// accepted, and one byte longer is refused before it is parsed.
///
/// # Errors
///
/// Returns an error if padding calculation fails.
pub fn assert_line_length_boundary<T: PartialEq + core::fmt::Debug>(
    text: &str,
    expected: &T,
    decode: impl Fn(&[u8]) -> Result<T, LineError>,
) -> Fallible {
    let pad = MAX_LINE_BYTES
        .checked_sub(text.len())
        .and_then(|room| room.checked_sub(1))
        .ok_or("the fixture line leaves no room to pad")?;
    // JSON admits whitespace between tokens; it pads without changing meaning.
    let at_bound = line(&text.replacen('{', &format!("{{{}", " ".repeat(pad)), 1));
    assert_eq!(at_bound.len(), MAX_LINE_BYTES);
    assert_eq!(&decode(&at_bound)?, expected);
    let over = line(&text.replacen('{', &format!("{{{}", " ".repeat(pad.saturating_add(1))), 1));
    assert_eq!(over.len(), MAX_LINE_BYTES.saturating_add(1));
    assert_eq!(
        decode(&over),
        Err(LineError::TooLong {
            length: MAX_LINE_BYTES.saturating_add(1),
        })
    );
    Ok(())
}

/// Sends `message` with `fds` as its `SCM_RIGHTS`, bypassing the sender's own checks.
///
/// # Errors
///
/// Returns an error if the control buffer cannot hold the descriptors or sending fails.
pub fn send_raw(socket: BorrowedFd<'_>, message: &[u8], fds: &[BorrowedFd<'_>]) -> Fallible {
    let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(3))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    if !fds.is_empty() && !control.push(SendAncillaryMessage::ScmRights(fds)) {
        return Err("the control buffer does not hold the fds".into());
    }
    sendmsg(
        socket,
        &[IoSlice::new(message)],
        &mut control,
        SendFlags::NOSIGNAL,
    )?;
    Ok(())
}

/// Asserts that `cue` has the expected index, duration range, and text.
pub fn check_cue(cue: &Cue, index: u32, start: Duration, end: Duration, text: &str) {
    assert_eq!(cue.index, index);
    assert_eq!(cue.start, start);
    assert_eq!(cue.end, end);
    assert_eq!(cue.text, text);
}
