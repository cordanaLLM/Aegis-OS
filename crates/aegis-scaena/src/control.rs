// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The D77 control plane: one line per message and no descriptor, over an
//! `AF_UNIX` `SOCK_SEQPACKET` socket pair.
//!
//! The shell's `MOUNT_SHELL_CANVAS` request and P17's Response travel here
//! ([`mount`]). Unlike the [`transport`](crate::transport) data
//! plane, a control message carries no `SCM_RIGHTS` at all: one that does is
//! refused with [`TransportError::FdOnControl`] and every descriptor it
//! carried is closed, so a peer cannot pass a buffer through the plane that
//! does not check buffers. Each endpoint both sends and receives, with the
//! same deadlines and byte bound as the data plane (HISS-02), and receiving
//! allocates nothing: the line lands in the caller's [`LineBuffer`] and the
//! control message in a stack buffer (HISS-03).

use std::os::fd::{AsFd, BorrowedFd, OwnedFd};

use rustix::net::{SendAncillaryBuffer, sockopt};

use crate::error::TransportError;
use crate::line::{LineBuffer, Response, body};
use crate::mount::{self, MountRequest};
use crate::transport::{
    Deadlines, recv_datagram, send_message_slice, seqpacket_pair, set_deadline,
};

/// A connected control socket pair, close-on-exec: the shell's end and P17's
/// end, each with both deadlines set.
///
/// # Errors
///
/// Returns [`TransportError::InvalidDeadline`] for a deadline outside
/// [`MIN_DEADLINE`](crate::transport::MIN_DEADLINE)..=[`MAX_DEADLINE`](crate::transport::MAX_DEADLINE)
/// and [`TransportError::Io`] when the kernel refuses the pair.
pub fn control_pair(
    deadlines: Deadlines,
) -> Result<(ControlEndpoint, ControlEndpoint), TransportError> {
    let (left, right) = seqpacket_pair()?;
    Ok((
        ControlEndpoint::new(left, deadlines)?,
        ControlEndpoint::new(right, deadlines)?,
    ))
}

/// One end of the control plane.
#[derive(Debug)]
pub struct ControlEndpoint {
    socket: OwnedFd,
}

impl ControlEndpoint {
    /// Takes a connected `SOCK_SEQPACKET` socket and sets both its deadlines.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::InvalidDeadline`] or [`TransportError::Io`].
    pub fn new(socket: OwnedFd, deadlines: Deadlines) -> Result<Self, TransportError> {
        set_deadline(socket.as_fd(), sockopt::Timeout::Send, deadlines.send)?;
        set_deadline(socket.as_fd(), sockopt::Timeout::Recv, deadlines.receive)?;
        Ok(Self { socket })
    }

    /// Sends `message`, one newline-terminated line, with no descriptor.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::Line`] for a message that is not one line
    /// within the bound, [`TransportError::SendDeadline`] when the peer does
    /// not take it in time, [`TransportError::PeerClosed`], and
    /// [`TransportError::Short`] or [`TransportError::Io`] otherwise.
    pub fn send(&self, message: &[u8]) -> Result<(), TransportError> {
        body(message)?;
        send_message_slice(
            self.socket.as_fd(),
            message,
            &mut SendAncillaryBuffer::default(),
        )
    }

    /// Receives one message into `buffer` and returns its line.
    ///
    /// # Errors
    ///
    /// Returns [`TransportError::ReceiveDeadline`] when nothing arrives in
    /// time, [`TransportError::PeerClosed`] at the end of the stream,
    /// [`TransportError::Line`] with [`crate::LineError::TooLong`] for a message over
    /// the bound, [`TransportError::FdOnControl`] or
    /// [`TransportError::ControlTruncated`] for a message that carried
    /// descriptors, and [`TransportError::Io`] otherwise. On every path each
    /// descriptor the message carried has been closed.
    pub fn receive<'b>(&self, buffer: &'b mut LineBuffer) -> Result<&'b [u8], TransportError> {
        let raw = recv_datagram(self.socket.as_fd(), buffer.storage())?;
        if raw.fd_count != 0 {
            return Err(TransportError::FdOnControl {
                count: raw.fd_count,
            });
        }
        if raw.bytes == 0 {
            return Err(TransportError::PeerClosed);
        }
        buffer.set_len(raw.bytes);
        Ok(buffer.line())
    }

    /// Encodes `request` into `buffer` and sends it.
    ///
    /// # Errors
    ///
    /// As [`ControlEndpoint::send`], and [`TransportError::Line`] for a
    /// request [`mount::encode_request`] refuses.
    pub fn send_mount(
        &self,
        request: &MountRequest,
        buffer: &mut LineBuffer,
    ) -> Result<(), TransportError> {
        let message = mount::encode_request(request, buffer)?;
        self.send(message)
    }

    /// Receives one message and decodes its mount request.
    ///
    /// # Errors
    ///
    /// As [`ControlEndpoint::receive`], and [`TransportError::Line`] for a
    /// line [`mount::decode_request`] refuses.
    pub fn receive_mount(&self, buffer: &mut LineBuffer) -> Result<MountRequest, TransportError> {
        let line = self.receive(buffer)?;
        Ok(mount::decode_request(line)?)
    }

    /// Encodes `response` into `buffer` and sends it.
    ///
    /// # Errors
    ///
    /// As [`ControlEndpoint::send`].
    pub fn send_mount_response(
        &self,
        response: &Response,
        buffer: &mut LineBuffer,
    ) -> Result<(), TransportError> {
        let message = mount::encode_response(response, buffer)?;
        self.send(message)
    }

    /// Receives one message and decodes its mount Response.
    ///
    /// # Errors
    ///
    /// As [`ControlEndpoint::receive`], and [`TransportError::Line`] for a
    /// line [`mount::decode_response`] refuses.
    pub fn receive_mount_response(
        &self,
        buffer: &mut LineBuffer,
    ) -> Result<Response, TransportError> {
        let line = self.receive(buffer)?;
        Ok(mount::decode_response(line)?)
    }
}

impl AsFd for ControlEndpoint {
    fn as_fd(&self) -> BorrowedFd<'_> {
        self.socket.as_fd()
    }
}
