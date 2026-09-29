// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03 on P17's own frame path (D83), made falsifiable where it can be.
//!
//! A per-frame allocation count needs a counting global allocator, which the
//! workspace's forbid lint does not admit, so none is produced (M27 criterion
//! 9). What is checked is structural: every value the frame path builds is
//! `Copy`, so it owns no heap, and a `Vec`, `String` or `Box` added to any of
//! them stops this file compiling; the line buffer is inline storage of the
//! full bound. What is not claimed: cros-libva allocates for every decoded
//! picture, the deviation D83 limits to that binding; `serde_json` copies an
//! escaped string into its own scratch buffer before a field sees it; and
//! wayland-client allocates for every protocol object it creates, which on
//! the display slice is a `zwp_linux_buffer_params_v1`, a `wl_buffer` and a
//! `wp_presentation_feedback` per frame -- third-party allocation outside
//! D83's scope as recorded, disclosed with M27's evidence.

mod common;

use core::mem::size_of;

use aegis_scaena::{
    AttachError, CorrelationId, DecodedFrame, DescriptorError, FileIdentity, FourCc,
    LayerSurfaceModel, LineBuffer, LineError, MAX_LINE_BYTES, Plane, Planes, Request, Response,
    SurfaceError, TransportError, WaitError,
};

use common::Fallible;

/// Accepts only a type that owns no heap, because it is `Copy`.
fn assert_no_heap<T: Copy>() {}

/// Positive: every value on the frame path owns no heap.
#[test]
fn every_value_on_the_frame_path_owns_no_heap() {
    assert_no_heap::<DecodedFrame>();
    assert_no_heap::<Planes>();
    assert_no_heap::<Plane>();
    assert_no_heap::<CorrelationId>();
    assert_no_heap::<FourCc>();
    assert_no_heap::<Request>();
    assert_no_heap::<Response>();
    assert_no_heap::<FileIdentity>();
    assert_no_heap::<LayerSurfaceModel>();
    assert_no_heap::<DescriptorError>();
    assert_no_heap::<LineError>();
    assert_no_heap::<TransportError>();
    assert_no_heap::<AttachError>();
    assert_no_heap::<SurfaceError>();
    assert_no_heap::<WaitError>();
    assert_no_heap::<aegis_scaena::jpeg::JpegFrame<'static>>();
    assert_no_heap::<aegis_scaena::jpeg::JpegError>();
    assert_no_heap::<aegis_scaena::content::ContentError>();
    assert_no_heap::<aegis_scaena::content::Crc32>();
    assert_no_heap::<aegis_scaena::content::LumaPlane<'static>>();
    assert_no_heap::<aegis_scaena::present::Counts>();
    assert_no_heap::<aegis_scaena::fixture::PinComparison>();
}

/// Negative: the line buffer is not a handle to heap storage; it holds the
/// whole bound inline.
#[test]
fn the_line_buffer_holds_the_whole_bound_inline() {
    assert!(size_of::<LineBuffer>() >= MAX_LINE_BYTES);
}

/// Boundary: a line whose correlation identifier is spelled with a JSON
/// escape decodes to the same heap-free value as the plain spelling.
#[test]
fn an_escaped_spelling_decodes_to_the_same_value() -> Fallible {
    let mut buffer = LineBuffer::new();
    let plain = aegis_scaena::line::encode_request(&common::request(2)?, &mut buffer)?.to_vec();
    let escaped = String::from_utf8(plain.clone())?.replacen("m27-", "m27\\u002d", 1);
    assert_ne!(escaped.as_bytes(), plain.as_slice());
    assert_eq!(
        aegis_scaena::line::decode_request(escaped.as_bytes())?,
        aegis_scaena::line::decode_request(&plain)?
    );
    Ok(())
}
