// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P17 Scaena: the display runtime's first slice (milestone M27,
//! ADR-0003).
//!
//! Decoded video reaches the compositor as DMA-BUF descriptors, never as
//! copied pixels (D75): P08 decodes, exports each surface as one DMA-BUF fd
//! and sends it over an `AF_UNIX` socket pair as `SCM_RIGHTS` beside one
//! line-delimited JSON-RPC 2.0 descriptor (D77), and P17 checks what it
//! received and attaches it to a `zwlr_layer_shell_v1` surface through
//! `zwp_linux_dmabuf_v1`. This crate is P17's side of that path and the parts
//! of it that need no GPU:
//!
//! | Module | What it holds | Decision or requirement |
//! | :-- | :-- | :-- |
//! | [`descriptor`] | `aegis.p08-p17.decoded-frame.v1`, owned here as the consumer | REQ-P17-07, M14 rule |
//! | [`line`](mod@line) | the JSON-RPC 2.0 line codec under a byte bound | D77, HISS-02 |
//! | [`transport`] | one line and exactly one fd per `SOCK_SEQPACKET` message, with send and receive deadlines | REQ-P17-04, D77 |
//! | [`attach`] | the `DMA_BUF_MAGIC`, plane-layout and object-size checks, and the advertised format table | E27-1, E27-2 |
//! | [`surface`] | the layer surface's configure and acknowledge state machine | E27-2 |
//! | [`wayland`] | every Wayland wait under its own deadline, the registry roundtrip first | HISS-02 |
//! | [`error`] | one typed refusal per stage | HISS-07 |
//!
//! # What this is evidence of
//!
//! The planes are typed and tested over a socket pair and a memfd inside
//! `make verify-all`, on any Linux host. A run against a compositor is the
//! client half only: the reference profile's compositor is the host session's
//! KDE `KWin`, not P04, and nothing here is evidence that P04 serves
//! `zwlr_layer_shell_v1` or `zwp_linux_dmabuf_v1` (D79).
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! No recursion (HISS-01). Every loop has a scalar bound, and every receive,
//! send and Wayland wait has a deadline (HISS-02). The frame path allocates
//! nothing per frame: descriptors are `Copy`, lines land in caller-owned
//! buffers and control messages in stack buffers (HISS-03). The pinned VA-API
//! binding allocates for every decoded picture, a deviation D83 limits to
//! that binding. No `unwrap`, `expect` or panic (HISS-07). The workspace's
//! forbid lint on unchecked code applies here unchanged, and the hygiene test
//! finds no occurrence of that keyword in this crate's sources (HISS-09).

pub mod attach;
pub mod content;
pub mod decode;
pub mod descriptor;
pub mod device;
pub mod error;
pub mod fixture;
pub mod jpeg;
pub mod line;
pub mod present;
pub mod reference;
pub mod slice;
pub mod surface;
pub mod transport;
pub mod wayland;

pub use attach::{Checked, DMA_BUF_MAGIC, FileIdentity, FormatTable, check};
pub use descriptor::{
    CorrelationId, DecodedFrame, FourCc, MAX_PLANES, Plane, Planes, SCHEMA_V1, Schema,
};
pub use error::{
    AttachError, DescriptorError, JsonCategory, LineError, SurfaceError, TransportError, WaitError,
};
pub use line::{LineBuffer, MAX_LINE_BYTES, METHOD, Refusal, Request, Response};
pub use surface::{ConfiguredSize, LayerSurfaceModel, SurfaceState};
pub use transport::{Deadlines, FrameReceiver, FrameSender, Received, pair};
pub use wayland::{Registry, Wait, WaitEvent, dispatch_until};

/// The pinned VA-API binding (D80), re-exported so the display slice and its
/// tests name one version of it.
pub use cros_libva;

/// The Wayland client toolkit (D75), re-exported for the same reason.
pub use smithay_client_toolkit;
