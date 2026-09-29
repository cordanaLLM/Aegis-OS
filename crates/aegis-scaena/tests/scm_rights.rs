// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E27-1: one descriptor line and exactly one fd per message, over a socket
//! pair, with a memfd standing in for the DMA-BUF.
//!
//! Positive: a descriptor and one memfd round-trip; the received fd, taken
//! with `MSG_CMSG_CLOEXEC` into an `OwnedFd`, is close-on-exec and has the
//! sender's `(st_dev, st_ino)`. Negative: a message with zero, two or three
//! fds is refused and every fd it carried is closed; a refused line closes
//! its fd; the attach step refuses the memfd because its filesystem magic is
//! not `DMA_BUF_MAGIC`. Boundary: a message one byte over the bound is
//! reported at its full length, refused, and its fd closed.
//!
//! Whether an fd was closed is read from `/proc/self/fd`: each case names its
//! memfd uniquely and counts the descriptors whose link names it, which no
//! other test in this binary can perturb.

mod common;

use core::mem::MaybeUninit;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use aegis_scaena::attach::{self, DMA_BUF_MAGIC, identity};
use aegis_scaena::line::encode_request;
use aegis_scaena::{AttachError, LineBuffer, LineError, MAX_LINE_BYTES, TransportError};
use rustix::io::{FdFlags, IoSlice, fcntl_getfd};
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};

use common::{Fallible, NV12_SIZE, borrow, endpoints, memfd, open_memfds_named, request};

/// `TMPFS_MAGIC` (`include/uapi/linux/magic.h`): where a memfd lives.
const TMPFS_MAGIC: u64 = 0x0102_1994;

/// Sends `message` with `fds` as its `SCM_RIGHTS`, bypassing the sender's own
/// checks, so the receiver meets what a faulty or hostile peer would send.
fn send_raw(socket: BorrowedFd<'_>, message: &[u8], fds: &[BorrowedFd<'_>]) -> Fallible {
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

/// A valid request line.
fn valid_line() -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut buffer = LineBuffer::new();
    Ok(encode_request(&request(3)?, &mut buffer)?.to_vec())
}

// --- Positive -------------------------------------------------------------

/// Positive: the descriptor and one memfd round-trip; the received fd is the
/// same file, close-on-exec, and the request decodes unchanged.
#[test]
fn a_descriptor_and_one_memfd_round_trip() -> Fallible {
    let (sender, receiver) = endpoints()?;
    let exported = memfd("scaena-round-trip", NV12_SIZE)?;
    let mut out = LineBuffer::new();
    sender.send_request(&request(3)?, borrow(&exported), &mut out)?;
    let mut buffer = LineBuffer::new();
    let (decoded, fd) = receiver.receive_request(&mut buffer)?;
    assert_eq!(decoded, request(3)?);
    assert_eq!(identity(fd.as_fd())?, identity(exported.as_fd())?);
    assert!(
        fcntl_getfd(&fd)?.contains(FdFlags::CLOEXEC),
        "MSG_CMSG_CLOEXEC"
    );
    assert_ne!(
        std::os::fd::AsRawFd::as_raw_fd(&fd),
        std::os::fd::AsRawFd::as_raw_fd(&exported)
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: zero fds for one descriptor is refused.
#[test]
fn a_message_without_an_fd_is_refused() -> Fallible {
    let (sender, receiver) = endpoints()?;
    send_raw(sender.as_fd(), &valid_line()?, &[])?;
    let mut buffer = LineBuffer::new();
    let refused = receiver
        .receive(&mut buffer)
        .map(|received| received.line.len());
    assert_eq!(refused, Err(TransportError::FdCount { count: 0 }));
    Ok(())
}

/// Negative: two or three fds for one descriptor are refused, and every one
/// the receiver was handed is closed.
#[test]
fn a_message_with_a_surplus_fd_is_refused_and_the_surplus_closed() -> Fallible {
    for (name, copies) in [("scaena-surplus-two", 2_usize), ("scaena-surplus-three", 3)] {
        let (sender, receiver) = endpoints()?;
        let exported = memfd(name, NV12_SIZE)?;
        let fds: Vec<BorrowedFd<'_>> = (0..copies).map(|_| exported.as_fd()).collect();
        send_raw(sender.as_fd(), &valid_line()?, &fds)?;
        let mut buffer = LineBuffer::new();
        let refused = receiver
            .receive(&mut buffer)
            .map(|received| received.line.len());
        assert!(
            matches!(
                refused,
                Err(TransportError::FdCount { count: 2.. } | TransportError::ControlTruncated)
            ),
            "{copies} fds: {refused:?}"
        );
        assert_eq!(
            open_memfds_named(name)?,
            1,
            "only the sender's own fd is open"
        );
    }
    Ok(())
}

/// Negative: a line the decoder refuses closes the fd that came with it.
#[test]
fn a_refused_line_closes_its_fd() -> Fallible {
    let (sender, receiver) = endpoints()?;
    let name = "scaena-refused-line";
    let exported = memfd(name, NV12_SIZE)?;
    let line = String::from_utf8(valid_line()?)?;
    for (bad, expected) in [
        (line.replace("\"2.0\"", "\"1.0\""), LineError::Version),
        (line.replacen('{', "{\"fd\":7,", 1), LineError::FdNumber),
    ] {
        send_raw(sender.as_fd(), bad.as_bytes(), &[exported.as_fd()])?;
        let mut buffer = LineBuffer::new();
        let refused = receiver
            .receive_request(&mut buffer)
            .map(|(request, _)| request.id);
        assert_eq!(refused, Err(TransportError::Line(expected)));
        assert_eq!(open_memfds_named(name)?, 1);
    }
    Ok(())
}

/// Negative: the attach step refuses the memfd, whose filesystem magic is
/// tmpfs's rather than `DMA_BUF_MAGIC`, and closes it.
#[test]
fn the_attach_step_refuses_a_memfd() -> Fallible {
    let (sender, receiver) = endpoints()?;
    let name = "scaena-not-dma-buf";
    let exported = memfd(name, NV12_SIZE)?;
    let mut out = LineBuffer::new();
    sender.send_request(&request(4)?, borrow(&exported), &mut out)?;
    let mut buffer = LineBuffer::new();
    let (decoded, fd): (_, OwnedFd) = receiver.receive_request(&mut buffer)?;
    assert_eq!(open_memfds_named(name)?, 2);
    let refused = attach::check(decoded.frame, fd).map(|checked| checked.size());
    assert_eq!(refused, Err(AttachError::NotDmaBuf { magic: TMPFS_MAGIC }));
    assert_ne!(TMPFS_MAGIC, DMA_BUF_MAGIC);
    assert_eq!(open_memfds_named(name)?, 1, "the refused fd is closed");
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a message one byte over the bound is reported at its full
/// length, not at the buffer's, refused, and its fd closed.
#[test]
fn a_message_over_the_bound_is_refused_at_its_full_length() -> Fallible {
    let (sender, receiver) = endpoints()?;
    let name = "scaena-over-bound";
    let exported = memfd(name, NV12_SIZE)?;
    let line = valid_line()?;
    let pad = MAX_LINE_BYTES.saturating_sub(line.len()).saturating_add(1);
    let over = String::from_utf8(line)?.replacen('{', &format!("{{{}", " ".repeat(pad)), 1);
    assert_eq!(over.len(), MAX_LINE_BYTES.saturating_add(1));
    send_raw(sender.as_fd(), over.as_bytes(), &[exported.as_fd()])?;
    let mut buffer = LineBuffer::new();
    let refused = receiver
        .receive(&mut buffer)
        .map(|received| received.line.len());
    assert_eq!(
        refused,
        Err(TransportError::Line(LineError::TooLong {
            length: MAX_LINE_BYTES.saturating_add(1)
        }))
    );
    assert_eq!(open_memfds_named(name)?, 1);
    Ok(())
}
