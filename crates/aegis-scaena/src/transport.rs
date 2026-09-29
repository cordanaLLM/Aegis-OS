// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The D77 data plane: one descriptor line and exactly one DMA-BUF fd per
//! message, over an `AF_UNIX` socket pair (REQ-P17-04, REQ-P17-07).
//!
//! The socket is `SOCK_SEQPACKET`, so a message is one line and the fd sent
//! with it arrives with that line and no other; on a byte stream a receive can
//! join the tail of one message to the head of the next, and an fd would then
//! sit beside the wrong descriptor. The fd is sent with rustix `sendmsg` as
//! `SCM_RIGHTS` and received with `recvmsg` and `MSG_CMSG_CLOEXEC`, straight
//! into an `OwnedFd`, so it is closed on every path that does not hand it on.
//!
//! Every receive and every send has a deadline, set on the socket as
//! `SO_RCVTIMEO` and `SO_SNDTIMEO` when the endpoint is made (HISS-02): a
//! silent peer fails the receive with [`TransportError::ReceiveDeadline`] and a
//! peer that stops reading fails the send with
//! [`TransportError::SendDeadline`]. Every line has a byte bound,
//! [`MAX_LINE_BYTES`]; a longer message is reported at its full length and
//! refused.
//!
//! A message carrying zero descriptors, or two, is refused, and every
//! descriptor it did carry is closed: the control buffer holds two, so a
//! second one is received and dropped, and the kernel closes any beyond that
//! and reports `MSG_CTRUNC`. Receiving allocates nothing: the line lands in
//! the caller's [`LineBuffer`] and the control message in a stack buffer.

use core::mem::MaybeUninit;
use core::time::Duration;
use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::io::{Errno, IoSlice, IoSliceMut};
use rustix::net::{
    AddressFamily, RecvAncillaryBuffer, RecvAncillaryMessage, RecvFlags, ReturnFlags,
    SendAncillaryBuffer, SendAncillaryMessage, SendFlags, SocketFlags, SocketType, recvmsg,
    sendmsg, socketpair, sockopt,
};

use crate::error::{LineError, TransportError, errno};
use crate::line::{self, LineBuffer, MAX_LINE_BYTES, Request};

/// The shortest deadline an endpoint accepts.
pub const MIN_DEADLINE: Duration = Duration::from_millis(1);

/// The longest deadline an endpoint accepts.
pub const MAX_DEADLINE: Duration = Duration::from_secs(60);

/// How many descriptors the receive buffer is sized for: one admitted, one
/// to detect a surplus. Alignment padding can leave room for a few more; any
/// that arrive are counted and closed too.
const FD_SLOTS: usize = 2;

/// The most descriptors one message can carry on Linux (`SCM_MAX_FD`), which
/// bounds the loop that counts and closes them.
const SCM_MAX_FD: usize = 253;

/// The send and receive deadlines of one endpoint pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Deadlines {
    /// `SO_RCVTIMEO` on the receiving end.
    pub receive: Duration,
    /// `SO_SNDTIMEO` on the sending end.
    pub send: Duration,
}

/// Maps a socket-call error onto the refusal it stands for.
fn socket_error(error: Errno, deadline: TransportError) -> TransportError {
    match error {
        Errno::AGAIN => deadline,
        Errno::PIPE | Errno::CONNRESET => TransportError::PeerClosed,
        other => TransportError::Io {
            errno: errno(other),
        },
    }
}

/// Sets one deadline on `socket`, refusing one outside the admitted range.
fn set_deadline(
    socket: BorrowedFd<'_>,
    which: sockopt::Timeout,
    deadline: Duration,
) -> Result<(), TransportError> {
    if !(MIN_DEADLINE..=MAX_DEADLINE).contains(&deadline) {
        return Err(TransportError::InvalidDeadline);
    }
    sockopt::set_socket_timeout(socket, which, Some(deadline)).map_err(|error| TransportError::Io {
        errno: errno(error),
    })
}

/// A connected `SOCK_SEQPACKET` socket pair, close-on-exec: the sending end
/// and the receiving end, each with its deadline set.
///
/// # Errors
///
/// Returns [`TransportError::InvalidDeadline`] for a deadline outside
/// [`MIN_DEADLINE`]..=[`MAX_DEADLINE`] and [`TransportError::Io`] when the
/// kernel refuses the pair.
pub fn pair(deadlines: Deadlines) -> Result<(FrameSender, FrameReceiver), TransportError> {
    let (left, right) = socketpair(
        AddressFamily::UNIX,
        SocketType::SEQPACKET,
        SocketFlags::CLOEXEC,
        None,
    )
    .map_err(|error| TransportError::Io {
        errno: errno(error),
    })?;
    Ok((
        FrameSender::new(left, deadlines.send)?,
        FrameReceiver::new(right, deadlines.receive)?,
    ))
}

/// The sending end: one line and one fd per message.
#[derive(Debug)]
pub struct FrameSender {
    socket: OwnedFd,
}

impl FrameSender {
    /// Takes a connected `SOCK_SEQPACKET` socket and sets its send deadline.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::InvalidDeadline`] or [`TransportError::Io`].
    pub fn new(socket: OwnedFd, send_deadline: Duration) -> Result<Self, TransportError> {
        set_deadline(socket.as_fd(), sockopt::Timeout::Send, send_deadline)?;
        Ok(Self { socket })
    }

    /// Sends `message`, one newline-terminated line, with `fd` as its one
    /// `SCM_RIGHTS` descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Line`] for a message that is not one line
    /// within the bound, [`TransportError::SendDeadline`] when the peer does
    /// not take it in time, [`TransportError::PeerClosed`], and
    /// [`TransportError::Short`] or [`TransportError::Io`] otherwise.
    pub fn send(&self, message: &[u8], fd: BorrowedFd<'_>) -> Result<(), TransportError> {
        line::body(message)?;
        let fds = [fd];
        let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(1))];
        let mut control = SendAncillaryBuffer::new(&mut space);
        if !control.push(SendAncillaryMessage::ScmRights(&fds)) {
            return Err(TransportError::ControlTruncated);
        }
        let sent = sendmsg(
            &self.socket,
            &[IoSlice::new(message)],
            &mut control,
            SendFlags::NOSIGNAL,
        )
        .map_err(|error| socket_error(error, TransportError::SendDeadline))?;
        if sent == message.len() {
            Ok(())
        } else {
            Err(TransportError::Short {
                sent,
                expected: message.len(),
            })
        }
    }

    /// Encodes `request` into `buffer` and sends it with `fd`.
    ///
    /// # Errors
    ///
    /// As [`FrameSender::send`], and [`TransportError::Line`] when the request
    /// does not encode within the bound.
    pub fn send_request(
        &self,
        request: &Request,
        fd: BorrowedFd<'_>,
        buffer: &mut LineBuffer,
    ) -> Result<(), TransportError> {
        let message = line::encode_request(request, buffer)?;
        self.send(message, fd)
    }
}

impl AsFd for FrameSender {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.socket.as_fd()
    }
}

/// One received message: its line, in the caller's buffer, and its one fd.
#[derive(Debug)]
pub struct Received<'b> {
    /// The line, newline included.
    pub line: &'b [u8],
    /// The one descriptor, close-on-exec.
    pub fd: OwnedFd,
}

/// The receiving end.
#[derive(Debug)]
pub struct FrameReceiver {
    socket: OwnedFd,
}

/// Takes the descriptors out of a received control message: the first, and
/// how many arrived in all. Every one but the first is dropped, which closes
/// it.
fn take_fds(control: &mut RecvAncillaryBuffer<'_>) -> (Option<OwnedFd>, usize) {
    let mut first = None;
    let mut count = 0_usize;
    let rights = control.drain().filter_map(|message| match message {
        RecvAncillaryMessage::ScmRights(fds) => Some(fds),
        _ => None,
    });
    for fd in rights.flatten().take(SCM_MAX_FD) {
        count = count.saturating_add(1);
        // Every fd after the first is dropped here, which closes it.
        first = first.or(Some(fd));
    }
    (first, count)
}

impl FrameReceiver {
    /// Takes a connected `SOCK_SEQPACKET` socket and sets its receive deadline.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::InvalidDeadline`] or [`TransportError::Io`].
    pub fn new(socket: OwnedFd, receive_deadline: Duration) -> Result<Self, TransportError> {
        set_deadline(socket.as_fd(), sockopt::Timeout::Recv, receive_deadline)?;
        Ok(Self { socket })
    }

    /// Receives one message into `buffer`: its line and exactly one fd.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ReceiveDeadline`] when nothing arrives in
    /// time, [`TransportError::PeerClosed`] at the end of the stream,
    /// [`TransportError::Line`] with [`LineError::TooLong`] for a message over
    /// the bound, [`TransportError::ControlTruncated`] and
    /// [`TransportError::FdCount`] for a message without exactly one fd, and
    /// [`TransportError::Io`] otherwise. On every error each fd the message
    /// carried has been closed.
    pub fn receive<'b>(&self, buffer: &'b mut LineBuffer) -> Result<Received<'b>, TransportError> {
        let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(FD_SLOTS))];
        let mut control = RecvAncillaryBuffer::new(&mut space);
        let message = recvmsg(
            &self.socket,
            &mut [IoSliceMut::new(buffer.storage())],
            &mut control,
            RecvFlags::CMSG_CLOEXEC | RecvFlags::TRUNC,
        )
        .map_err(|error| socket_error(error, TransportError::ReceiveDeadline))?;
        let (fd, count) = take_fds(&mut control);
        if message.flags.contains(ReturnFlags::TRUNC) || message.bytes > MAX_LINE_BYTES {
            return Err(LineError::TooLong {
                length: message.bytes,
            }
            .into());
        }
        if message.flags.contains(ReturnFlags::CTRUNC) {
            return Err(TransportError::ControlTruncated);
        }
        if message.bytes == 0 && count == 0 {
            return Err(TransportError::PeerClosed);
        }
        let (Some(fd), 1) = (fd, count) else {
            return Err(TransportError::FdCount { count });
        };
        buffer.set_len(message.bytes);
        Ok(Received {
            line: buffer.line(),
            fd,
        })
    }

    /// Receives one message and decodes its request. A refused line closes
    /// the fd that came with it.
    ///
    /// # Errors
    ///
    /// As [`FrameReceiver::receive`], and [`TransportError::Line`] for a line
    /// the decoder refuses.
    pub fn receive_request(
        &self,
        buffer: &mut LineBuffer,
    ) -> Result<(Request, OwnedFd), TransportError> {
        let received = self.receive(buffer)?;
        let request = line::decode_request(received.line)?;
        Ok((request, received.fd))
    }
}

impl AsFd for FrameReceiver {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.socket.as_fd()
    }
}
