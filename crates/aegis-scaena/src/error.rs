// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Typed refusals, one enum per stage of the D77 data plane.
//!
//! Every value here is `Copy` and owns no heap, so building a refusal on the
//! frame path allocates nothing (HISS-03, D83). An operating-system error is
//! carried as its raw errno rather than as a `std::io::Error`, which may box.

use thiserror::Error;

use crate::descriptor::{FourCc, MAX_PLANES};
use crate::line::MAX_LINE_BYTES;
use crate::wayland::WaitEvent;

/// Why a decoded-frame descriptor was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum DescriptorError {
    /// The descriptor names another schema than `aegis.p08-p17.decoded-frame.v1`.
    #[error("the descriptor names a schema this build does not admit")]
    Schema,
    /// The plane list is empty or longer than [`MAX_PLANES`].
    #[error("the descriptor carries {count} planes; 1 to {MAX_PLANES} are admitted")]
    PlaneCount {
        /// How many planes the descriptor listed.
        count: usize,
    },
    /// The correlation identifier is empty, too long or outside its alphabet.
    #[error("the correlation identifier is empty, too long or outside its alphabet")]
    CorrelationId,
    /// The fourcc is not four printable ASCII characters.
    #[error("the fourcc is not four printable ASCII characters")]
    Fourcc,
    /// A dimension is zero or above the admitted bound.
    #[error("the width or the height is zero or above the admitted bound")]
    Dimensions,
}

/// Why a JSON-RPC 2.0 line was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum LineError {
    /// The message is longer than [`MAX_LINE_BYTES`], its newline included.
    #[error("the line is {length} bytes; at most {MAX_LINE_BYTES} are admitted")]
    TooLong {
        /// The message's length in bytes, as the socket reported it.
        length: usize,
    },
    /// The message is not exactly one line ending in one newline.
    #[error("the message is not exactly one newline-terminated line")]
    Framing,
    /// The line is not JSON the envelope admits (syntax, type, unknown or
    /// duplicate member).
    #[error("the line is not an admitted JSON-RPC 2.0 object ({category:?} at column {column})")]
    Json {
        /// The decoder's classification.
        category: JsonCategory,
        /// The column the decoder stopped at.
        column: usize,
    },
    /// The `jsonrpc` member is absent or not `"2.0"`.
    #[error("the jsonrpc member is absent or not \"2.0\"")]
    Version,
    /// The method is not the one this socket serves.
    #[error("the method is not STREAM_DECODED_FRAME")]
    Method,
    /// The request carries no numeric `id`.
    #[error("the request carries no numeric id")]
    Id,
    /// The request carries no `params`, so no descriptor.
    #[error("the request carries no descriptor in params")]
    Params,
    /// The line names a file-descriptor number; descriptors travel only as
    /// `SCM_RIGHTS` (D77).
    #[error("the line names a file-descriptor number; descriptors travel only as SCM_RIGHTS")]
    FdNumber,
    /// The response is neither a result nor an error.
    #[error("the response carries neither a result nor an error")]
    Response,
    /// The descriptor inside the line was refused.
    #[error("the descriptor was refused: {0}")]
    Descriptor(#[from] DescriptorError),
}

/// A `Copy` mirror of `serde_json::error::Category`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonCategory {
    /// The input is not syntactically valid JSON.
    Syntax,
    /// The JSON is well formed but not the shape the envelope admits.
    Data,
    /// The input ended early.
    Eof,
    /// Reading or writing failed, which for a bounded buffer means it is full.
    Io,
}

impl From<&serde_json::Error> for LineError {
    fn from(error: &serde_json::Error) -> Self {
        let category = match error.classify() {
            serde_json::error::Category::Syntax => JsonCategory::Syntax,
            serde_json::error::Category::Data => JsonCategory::Data,
            serde_json::error::Category::Eof => JsonCategory::Eof,
            serde_json::error::Category::Io => JsonCategory::Io,
        };
        Self::Json {
            category,
            column: error.column(),
        }
    }
}

/// Why a message could not be sent or received over the socket pair.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum TransportError {
    /// A deadline is zero or above the admitted bound.
    #[error("a socket deadline must be between 1 ms and 60 s")]
    InvalidDeadline,
    /// Nothing arrived before the receive deadline (`SO_RCVTIMEO`).
    #[error("the receive missed its deadline")]
    ReceiveDeadline,
    /// The peer did not take the message before the send deadline
    /// (`SO_SNDTIMEO`).
    #[error("the send missed its deadline")]
    SendDeadline,
    /// The peer closed its end.
    #[error("the peer closed the connection")]
    PeerClosed,
    /// The message carried another number of descriptors than exactly one;
    /// every descriptor it did carry has been closed.
    #[error("the message carried {count} file descriptors; exactly one is admitted")]
    FdCount {
        /// How many descriptors arrived.
        count: usize,
    },
    /// More descriptors arrived than the control buffer holds; the kernel
    /// closed the ones that did not fit and this side closed the rest.
    #[error("the control message was truncated: more file descriptors than admitted")]
    ControlTruncated,
    /// The kernel accepted fewer bytes than the line.
    #[error("the kernel sent {sent} of {expected} bytes")]
    Short {
        /// Bytes sent.
        sent: usize,
        /// Bytes in the line.
        expected: usize,
    },
    /// The line was refused.
    #[error("the line was refused: {0}")]
    Line(#[from] LineError),
    /// Any other operating-system error, as its errno.
    #[error("the socket call failed with errno {errno}")]
    Io {
        /// The raw errno.
        errno: i32,
    },
}

/// Why a received descriptor may not be attached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum AttachError {
    /// The fd's filesystem is not the DMA-BUF one (`fstatfs` magic).
    #[error("the fd is not a DMA-BUF: its filesystem magic is {magic:#x}")]
    NotDmaBuf {
        /// The magic `fstatfs` reported.
        magic: u64,
    },
    /// The fourcc is not one whose plane layout this crate knows.
    #[error("no plane layout is admitted for fourcc {fourcc}")]
    UnsupportedFormat {
        /// The fourcc the descriptor named.
        fourcc: FourCc,
    },
    /// The descriptor lists another number of planes than its format has.
    #[error("the format has {expected} planes and the descriptor lists {listed}")]
    PlaneLayout {
        /// Planes the format has.
        expected: usize,
        /// Planes the descriptor listed.
        listed: usize,
    },
    /// A plane's pitch is below the bytes its row holds.
    #[error("plane {plane}'s pitch is below the bytes one of its rows holds")]
    PitchTooSmall {
        /// The plane's index.
        plane: usize,
    },
    /// A plane ends past the end of its object.
    #[error("plane {plane} ends at byte {end}, past the object's {size} bytes")]
    PlaneOutOfBounds {
        /// The plane's index.
        plane: usize,
        /// Where the plane ends: offset + pitch x rows.
        end: u64,
        /// The object's size, read with `lseek(SEEK_END)`.
        size: u64,
    },
    /// The compositor did not advertise the pair for this surface.
    #[error("the compositor did not advertise fourcc {fourcc} with modifier {modifier:#018x}")]
    NotAdvertised {
        /// The fourcc.
        fourcc: FourCc,
        /// The modifier.
        modifier: u64,
    },
    /// The advertised-pair table is full.
    #[error("the advertised format table is full")]
    TableFull,
    /// A system call on the fd failed, as its errno.
    #[error("a call on the fd failed with errno {errno}")]
    Io {
        /// The raw errno.
        errno: i32,
    },
}

/// Why the layer-surface state machine refused a step.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum SurfaceError {
    /// A buffer was attached before the first `ack_configure`.
    #[error("a buffer may not be attached before the first ack_configure")]
    NotConfigured,
    /// An `ack_configure` named a serial the compositor did not send last.
    #[error("ack_configure named serial {serial}, not the last configure's")]
    UnknownSerial {
        /// The serial acknowledged.
        serial: u32,
    },
    /// The surface was closed by the compositor.
    #[error("the surface was closed")]
    Closed,
}

/// Why a wait on the Wayland connection failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[non_exhaustive]
pub enum WaitError {
    /// The awaited event did not arrive before its deadline (HISS-02).
    #[error("{event} did not arrive before its deadline")]
    Deadline {
        /// What was awaited.
        event: WaitEvent,
    },
    /// The connection failed or the compositor closed it while waiting.
    #[error("the Wayland connection failed while waiting for {event}")]
    Connection {
        /// What was awaited.
        event: WaitEvent,
    },
    /// The compositor sent a protocol error.
    #[error("the compositor sent a protocol error while waiting for {event}")]
    Protocol {
        /// What was awaited.
        event: WaitEvent,
    },
}

/// The raw errno of a rustix error.
#[must_use]
pub fn errno(error: rustix::io::Errno) -> i32 {
    error.raw_os_error()
}
