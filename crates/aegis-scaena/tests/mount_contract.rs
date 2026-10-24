// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M28 E28-2: `MOUNT_SHELL_CANVAS`, the surface contract the P05 shell sits
//! behind, over the control plane (D77, D82).
//!
//! Positive: a mount request and both kinds of Response round-trip over a
//! control socket pair, with every optional member present and with none.
//! Negative: a `jsonrpc` member that is not `"2.0"`, another method, an
//! unknown schema, layer, edge or keyboard interactivity, a repeated edge, an
//! exclusive edge the surface is not anchored to, an unknown member, a
//! control message that carries a descriptor and a silent peer are each
//! refused with their own error. Boundary: a line exactly at
//! [`MAX_LINE_BYTES`] is accepted and one byte longer refused; a namespace of
//! [`MAX_NAMESPACE_BYTES`] is accepted and one byte longer refused; the
//! exclusive zone admits -1 and [`MAX_EXTENT`] and refuses -2 and one past;
//! a margin admits ±[`MAX_EXTENT`] and refuses one past; top-level `fd` and
//! `fds` members are refused as unknown JSON.

mod common;

use core::mem::MaybeUninit;
use std::os::fd::AsFd;
use std::time::Instant;

use aegis_scaena::mount::{
    MAX_EXTENT, MAX_NAMESPACE_BYTES, decode_request, decode_response, encode_request,
    encode_response,
};
use aegis_scaena::{
    Anchor, Edge, JsonCategory, KeyboardInteractivity, Layer, LineBuffer, LineError, Margin,
    MountCanvas, MountError, MountRequest, MountSchema, Namespace, Refusal, Response,
    TransportError, control_pair,
};
use rustix::io::IoSlice;
use rustix::net::{SendAncillaryBuffer, SendAncillaryMessage, SendFlags, sendmsg};

use common::{DEADLINES, Fallible, assert_line_length_boundary, line, memfd, request_line_string};

/// A top bar: anchored top, left and right, with every optional member set.
fn bar() -> Result<MountCanvas, MountError> {
    Ok(MountCanvas {
        schema: MountSchema::V1,
        namespace: Namespace::parse(b"aegis.shell")?,
        layer: Layer::Top,
        anchor: Anchor::NONE
            .with(Edge::Top)
            .with(Edge::Left)
            .with(Edge::Right),
        exclusive_zone: Some(32),
        exclusive_edge: Some(Edge::Top),
        margin: Some(Margin {
            top: 0,
            right: 4,
            bottom: 0,
            left: 4,
        }),
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
    })
}

/// A request carrying `canvas`.
const fn request(canvas: MountCanvas) -> MountRequest {
    MountRequest { id: 7, canvas }
}

/// The bar's request line, without its newline.
fn valid() -> Result<String, Box<dyn std::error::Error>> {
    request_line_string(&request(bar()?), encode_request)
}

/// Decodes the bar's line with `from` replaced by `to` once.
fn decode_with(
    from: &str,
    to: &str,
) -> Result<Result<MountRequest, LineError>, Box<dyn std::error::Error>> {
    let text = valid()?;
    if !text.contains(from) {
        return Err(format!("the fixture line holds no {from}").into());
    }
    Ok(decode_request(&line(&text.replacen(from, to, 1))))
}

/// The refusal [`MountCanvas::check`] gives the bar after `change`.
fn checked(change: impl FnOnce(&mut MountCanvas)) -> Result<Result<(), MountError>, MountError> {
    let mut canvas = bar()?;
    change(&mut canvas);
    Ok(canvas.check())
}

// --- Positive -------------------------------------------------------------

/// Positive: the request and both Responses round-trip over the control
/// plane.
#[test]
fn a_mount_and_both_responses_round_trip_over_the_control_plane() -> Fallible {
    let (shell, scaena) = control_pair(DEADLINES)?;
    let mut buffer = LineBuffer::new();
    shell.send_mount(&request(bar()?), &mut buffer)?;
    assert_eq!(scaena.receive_mount(&mut buffer)?, request(bar()?));
    for response in [
        Response {
            id: Some(7),
            outcome: Ok(()),
        },
        Response {
            id: Some(7),
            outcome: Err(Refusal::InvalidParams),
        },
    ] {
        scaena.send_mount_response(&response, &mut buffer)?;
        assert_eq!(shell.receive_mount_response(&mut buffer)?, response);
    }
    Ok(())
}

/// Positive: a centred overlay with no optional member encodes without them
/// and decodes to `None`, the compositor's default.
#[test]
fn absent_optional_members_decode_to_none() -> Fallible {
    let canvas = MountCanvas {
        anchor: Anchor::NONE,
        exclusive_zone: None,
        exclusive_edge: None,
        margin: None,
        layer: Layer::Overlay,
        keyboard_interactivity: KeyboardInteractivity::Exclusive,
        ..bar()?
    };
    let mut buffer = LineBuffer::new();
    let encoded = encode_request(&request(canvas), &mut buffer)?.to_vec();
    let text = String::from_utf8(encoded.clone())?;
    for member in ["exclusive_zone", "exclusive_edge", "margin"] {
        assert!(!text.contains(member), "{member} was encoded: {text}");
    }
    assert!(text.contains("\"anchor\":[]"), "{text}");
    assert_eq!(decode_request(&encoded)?, request(canvas));
    Ok(())
}

/// Positive: the wire names map onto the protocol's values.
#[test]
fn the_wire_names_carry_the_protocol_values() -> Fallible {
    let all = Anchor::NONE
        .with(Edge::Top)
        .with(Edge::Bottom)
        .with(Edge::Left)
        .with(Edge::Right);
    assert_eq!(all.bits(), 15);
    assert_eq!(Layer::parse(b"background")?.protocol_value(), 0);
    assert_eq!(Layer::parse(b"overlay")?.protocol_value(), 3);
    assert_eq!(
        KeyboardInteractivity::parse(b"on_demand")?.protocol_value(),
        2
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: every envelope and parameter refusal names its own error.
#[test]
fn each_refused_member_names_its_error() -> Fallible {
    let cases: [(&str, &str, LineError); 9] = [
        ("\"2.0\"", "\"1.0\"", LineError::Version),
        (
            "MOUNT_SHELL_CANVAS",
            "STREAM_DECODED_FRAME",
            LineError::Method,
        ),
        (
            "mount-shell-canvas.v1",
            "mount-shell-canvas.v2",
            MountError::Schema.into(),
        ),
        (
            "\"layer\":\"top\"",
            "\"layer\":\"dock\"",
            MountError::Layer.into(),
        ),
        ("[\"top\"", "[\"middle\"", MountError::Anchor.into()),
        ("[\"top\"", "[\"top\",\"top\"", MountError::Anchor.into()),
        (
            "\"exclusive_edge\":\"top\"",
            "\"exclusive_edge\":\"bottom\"",
            MountError::ExclusiveEdge.into(),
        ),
        (
            "\"on_demand\"",
            "\"always\"",
            MountError::KeyboardInteractivity.into(),
        ),
        (
            "\"aegis.shell\"",
            "\"Aegis Shell\"",
            MountError::Namespace.into(),
        ),
    ];
    for (from, to, error) in cases {
        assert_eq!(decode_with(from, to)?, Err(error), "{from} -> {to}");
    }
    Ok(())
}

/// Negative: an unknown member is refused as JSON of another shape.
#[test]
fn an_unknown_member_is_refused() -> Fallible {
    let refused = decode_with("\"layer\"", "\"output\":\"DP-1\",\"layer\"")?;
    assert!(
        matches!(refused, Err(LineError::Json { .. })),
        "{refused:?}"
    );
    Ok(())
}

/// Negative: the encoder refuses a canvas the decoder would refuse, so no
/// such line reaches the wire.
#[test]
fn the_encoder_refuses_an_unanchored_exclusive_edge() -> Fallible {
    let canvas = MountCanvas {
        exclusive_edge: Some(Edge::Bottom),
        ..bar()?
    };
    let mut buffer = LineBuffer::new();
    assert_eq!(
        encode_request(&request(canvas), &mut buffer).map(<[u8]>::len),
        Err(MountError::ExclusiveEdge.into())
    );
    Ok(())
}

/// Negative: a decoded-frame Response is not a mount Response.
#[test]
fn an_attached_response_is_not_a_mounted_one() -> Fallible {
    let mut buffer = LineBuffer::new();
    let attached = aegis_scaena::line::encode_response(
        &Response {
            id: Some(7),
            outcome: Ok(()),
        },
        &mut buffer,
    )?
    .to_vec();
    assert_eq!(decode_response(&attached), Err(LineError::Response));
    let mounted = encode_response(
        &Response {
            id: Some(7),
            outcome: Ok(()),
        },
        &mut buffer,
    )?
    .to_vec();
    assert_eq!(
        aegis_scaena::line::decode_response(&mounted),
        Err(LineError::Response)
    );
    Ok(())
}

/// Negative: a control message carrying a descriptor is refused, and the
/// descriptor is closed rather than handed on.
#[test]
fn a_descriptor_on_the_control_plane_is_refused() -> Fallible {
    let (shell, scaena) = control_pair(DEADLINES)?;
    let fd = memfd("aegis-mount-stray", 4_096)?;
    let fds = [fd.as_fd()];
    let mut space = [MaybeUninit::<u8>::uninit(); rustix::cmsg_space!(ScmRights(1))];
    let mut control = SendAncillaryBuffer::new(&mut space);
    if !control.push(SendAncillaryMessage::ScmRights(&fds)) {
        return Err("the control buffer did not take the descriptor".into());
    }
    let message = line(&valid()?);
    sendmsg(
        shell.as_fd(),
        &[IoSlice::new(&message)],
        &mut control,
        SendFlags::NOSIGNAL,
    )?;
    let mut buffer = LineBuffer::new();
    assert_eq!(
        scaena.receive_mount(&mut buffer),
        Err(TransportError::FdOnControl { count: 1 })
    );
    Ok(())
}

/// Negative: a silent peer fails the receive at its deadline.
#[test]
fn a_silent_peer_fails_the_receive_at_its_deadline() -> Fallible {
    let (_shell, scaena) = control_pair(DEADLINES)?;
    let mut buffer = LineBuffer::new();
    let start = Instant::now();
    assert_eq!(
        scaena.receive_mount(&mut buffer),
        Err(TransportError::ReceiveDeadline)
    );
    assert!(start.elapsed() >= DEADLINES.receive, "failed early");
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a line of exactly `MAX_LINE_BYTES`, newline included, is
/// accepted, and one byte longer is refused before it is parsed.
#[test]
fn a_line_at_the_byte_bound_is_accepted_and_one_byte_longer_refused() -> Fallible {
    assert_line_length_boundary(&valid()?, &request(bar()?), decode_request)
}

/// Boundary: a namespace of `MAX_NAMESPACE_BYTES` is accepted on the wire and
/// one byte longer refused; an empty one is refused.
#[test]
fn a_namespace_at_its_bound_is_accepted_and_one_byte_longer_refused() -> Fallible {
    let at = "a".repeat(MAX_NAMESPACE_BYTES);
    let decoded = decode_with("\"aegis.shell\"", &format!("\"{at}\""))??;
    assert_eq!(decoded.canvas.namespace.as_str(), at);
    let over = "a".repeat(MAX_NAMESPACE_BYTES.saturating_add(1));
    assert_eq!(
        decode_with("\"aegis.shell\"", &format!("\"{over}\""))?,
        Err(MountError::Namespace.into())
    );
    assert_eq!(Namespace::parse(b""), Err(MountError::Namespace));
    Ok(())
}

/// Boundary: the exclusive zone admits -1 and `MAX_EXTENT` and refuses -2
/// and one past, in the check and on the wire.
#[test]
fn the_exclusive_zone_is_bounded_at_both_ends() -> Fallible {
    for zone in [-1, 0, MAX_EXTENT] {
        assert_eq!(
            checked(|canvas| canvas.exclusive_zone = Some(zone))?,
            Ok(())
        );
    }
    for zone in [-2, MAX_EXTENT.saturating_add(1)] {
        assert_eq!(
            checked(|canvas| canvas.exclusive_zone = Some(zone))?,
            Err(MountError::ExclusiveZone)
        );
        assert_eq!(
            decode_with(
                "\"exclusive_zone\":32",
                &format!("\"exclusive_zone\":{zone}")
            )?,
            Err(MountError::ExclusiveZone.into())
        );
    }
    // Wider than the protocol's int: refused, not wrapped.
    assert_eq!(
        decode_with("\"exclusive_zone\":32", "\"exclusive_zone\":4294967328")?,
        Err(MountError::ExclusiveZone.into())
    );
    Ok(())
}

/// Boundary: a margin admits ±`MAX_EXTENT` and refuses one past either end.
#[test]
fn a_margin_is_bounded_at_both_ends() -> Fallible {
    for edge in [MAX_EXTENT, -MAX_EXTENT] {
        let margin = Margin {
            top: edge,
            right: edge,
            bottom: edge,
            left: edge,
        };
        assert_eq!(checked(|canvas| canvas.margin = Some(margin))?, Ok(()));
    }
    for past in [
        MAX_EXTENT.saturating_add(1),
        (-MAX_EXTENT).saturating_sub(1),
    ] {
        let margin = Margin {
            left: past,
            ..Margin::default()
        };
        assert_eq!(
            checked(|canvas| canvas.margin = Some(margin))?,
            Err(MountError::Margin)
        );
        assert_eq!(
            decode_with("[0,4,0,4]", &format!("[0,4,0,{past}]"))?,
            Err(MountError::Margin.into())
        );
    }
    Ok(())
}

/// Boundary: all four edges are admitted; a fifth entry can only repeat one
/// and is refused.
#[test]
fn four_edges_are_admitted_and_a_fifth_refused() -> Fallible {
    let four = "[\"top\",\"bottom\",\"left\",\"right\"]";
    let decoded = decode_with("[\"top\",\"left\",\"right\"]", four)??;
    assert_eq!(decoded.canvas.anchor.bits(), 15);
    assert_eq!(
        decode_with(
            "[\"top\",\"left\",\"right\"]",
            "[\"top\",\"bottom\",\"left\",\"right\",\"top\"]"
        )?,
        Err(MountError::Anchor.into())
    );
    Ok(())
}

/// Boundary: a mount request with top-level `fd` is refused as unknown JSON.
#[test]
fn a_top_level_fd_is_refused_as_unknown_json() -> Fallible {
    let refused = decode_with("{", "{\"fd\":3,")?;
    assert_eq!(
        refused,
        Err(LineError::Json {
            category: JsonCategory::Data,
            column: 5,
        })
    );
    Ok(())
}

/// Boundary: a mount request with top-level `fds` is refused as unknown JSON.
#[test]
fn a_top_level_fds_is_refused_as_unknown_json() -> Fallible {
    let refused = decode_with("{", "{\"fds\":[3],")?;
    assert_eq!(
        refused,
        Err(LineError::Json {
            category: JsonCategory::Data,
            column: 6,
        })
    );
    Ok(())
}
