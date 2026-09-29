// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E27-1: the decoded-frame descriptor, `aegis.p08-p17.decoded-frame.v1`.
//!
//! Positive: a descriptor encodes into one line and decodes back unchanged,
//! the modifier `export_prime` returned included, bit for bit. Negative: an
//! empty or over-long correlation identifier, a fourcc that is not four
//! printable characters, a zero dimension and another schema are refused.
//! Boundary: 1 and 4 planes are accepted and 0 and 5 refused, and a
//! 64-byte correlation identifier is accepted and a 65-byte one refused.

mod common;

use aegis_scaena::descriptor::{
    MAX_CORRELATION_BYTES, MAX_DIMENSION, MOD_INTEL_4_TILED, MOD_INTEL_4_TILED_DG2_RC_CCS,
    MOD_INVALID,
};
use aegis_scaena::line::{decode_request, encode_request};
use aegis_scaena::{
    CorrelationId, DecodedFrame, DescriptorError, FourCc, LineBuffer, LineError, MAX_PLANES, Plane,
    Planes, Request, SCHEMA_V1,
};

use common::{Fallible, nv12_frame};

/// Encodes and decodes `request` through one line.
fn round_trip(request: &Request) -> Result<Request, LineError> {
    let mut buffer = LineBuffer::new();
    let line = encode_request(request, &mut buffer)?.to_vec();
    decode_request(&line)
}

/// `count` planes, each one row further into the object.
fn planes(count: usize) -> Vec<Plane> {
    (0..count)
        .map(|index| Plane {
            offset: u32::try_from(index).unwrap_or(0).saturating_mul(4_096),
            pitch: 64,
        })
        .collect()
}

/// A request line with `count` planes spliced in, built by hand so counts
/// the type cannot hold still reach the decoder.
fn line_with_planes(count: usize) -> Vec<u8> {
    let list: Vec<String> = planes(count)
        .iter()
        .map(|plane| format!("{{\"offset\":{},\"pitch\":{}}}", plane.offset, plane.pitch))
        .collect();
    format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"STREAM_DECODED_FRAME\",\"params\":{{\"schema\":\"{SCHEMA_V1}\",\"correlation_id\":\"c-1\",\"fourcc\":\"NV12\",\"modifier\":0,\"width\":64,\"height\":35,\"planes\":[{}]}},\"id\":7}}\n",
        list.join(",")
    )
    .into_bytes()
}

// --- Positive -------------------------------------------------------------

/// Positive: a descriptor survives the line unchanged, and the modifier is
/// carried bit for bit, including the one the compositor does not advertise.
#[test]
fn a_descriptor_round_trips_unchanged() -> Fallible {
    for modifier in [
        MOD_INTEL_4_TILED,
        MOD_INTEL_4_TILED_DG2_RC_CCS,
        MOD_INVALID,
        u64::MAX,
    ] {
        let request = Request {
            id: u64::MAX,
            frame: nv12_frame(modifier)?,
        };
        let back = round_trip(&request)?;
        assert_eq!(back, request);
        assert_eq!(
            back.frame.modifier, modifier,
            "the modifier is carried unchanged"
        );
    }
    Ok(())
}

/// Positive: the wire form names every field the schema carries and no fd.
#[test]
fn the_line_names_the_schema_fields_and_no_descriptor() -> Fallible {
    let mut buffer = LineBuffer::new();
    let request = Request {
        id: 1,
        frame: nv12_frame(0)?,
    };
    let line = String::from_utf8(encode_request(&request, &mut buffer)?.to_vec())?;
    for member in [
        "\"jsonrpc\":\"2.0\"",
        "\"method\":\"STREAM_DECODED_FRAME\"",
        "\"schema\":\"aegis.p08-p17.decoded-frame.v1\"",
        "\"correlation_id\":\"m27-p08-fixture-0001\"",
        "\"fourcc\":\"NV12\"",
        "\"modifier\":0",
        "\"width\":64",
        "\"height\":35",
        "\"planes\":[{\"offset\":0,\"pitch\":64},{\"offset\":2240,\"pitch\":64}]",
        "\"id\":1",
    ] {
        assert!(line.contains(member), "{member} is missing from {line}");
    }
    assert!(!line.contains("\"fd"), "{line}");
    assert!(line.ends_with("}\n"), "one line, one newline");
    Ok(())
}

/// Positive: the fourcc is the DRM code of its four characters.
#[test]
fn the_fourcc_is_the_drm_code() -> Fallible {
    assert_eq!(FourCc::NV12.code(), 0x3231_564e);
    assert_eq!(FourCc::parse(b"NV12")?, FourCc::NV12);
    assert_eq!(FourCc::from_code(0x3231_564e)?, FourCc::NV12);
    assert_eq!(FourCc::NV12.to_string(), "NV12");
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: identifiers, fourccs, dimensions and schemas outside their
/// bounds are refused, each with its own error.
#[test]
fn out_of_bound_fields_are_refused() -> Fallible {
    assert_eq!(
        CorrelationId::parse(b""),
        Err(DescriptorError::CorrelationId)
    );
    assert_eq!(
        CorrelationId::parse(b"with space"),
        Err(DescriptorError::CorrelationId)
    );
    assert_eq!(FourCc::parse(b"NV1"), Err(DescriptorError::Fourcc));
    assert_eq!(FourCc::parse(b"NV\x0012"), Err(DescriptorError::Fourcc));
    assert_eq!(FourCc::from_code(0), Err(DescriptorError::Fourcc));
    let planes = Planes::new(&planes(2))?;
    let id = CorrelationId::parse(b"c-1")?;
    for size in [(0, 35), (64, 0), (MAX_DIMENSION.saturating_add(1), 35)] {
        assert_eq!(
            DecodedFrame::new(id, FourCc::NV12, 0, size, planes),
            Err(DescriptorError::Dimensions)
        );
    }
    let other = String::from_utf8(line_with_planes(2))?.replace(".v1", ".v2");
    assert_eq!(
        decode_request(other.as_bytes()),
        Err(LineError::Descriptor(DescriptorError::Schema))
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: 1 and 4 planes are accepted, 0 and 5 refused, by the type and
/// by the decoder alike.
#[test]
fn one_and_four_planes_are_accepted_zero_and_five_refused() -> Fallible {
    assert_eq!(MAX_PLANES, 4);
    for count in [1, MAX_PLANES] {
        assert_eq!(Planes::new(&planes(count))?.len(), count);
        let request = decode_request(&line_with_planes(count))?;
        assert_eq!(request.frame.planes.len(), count);
        assert_eq!(request.frame.planes.as_slice(), planes(count).as_slice());
    }
    for count in [0, MAX_PLANES.saturating_add(1)] {
        let refused = DescriptorError::PlaneCount { count };
        assert_eq!(Planes::new(&planes(count)), Err(refused));
        assert_eq!(
            decode_request(&line_with_planes(count)),
            Err(LineError::Descriptor(refused))
        );
    }
    Ok(())
}

/// Boundary: a 64-byte correlation identifier is accepted and a 65-byte one
/// refused, never truncated.
#[test]
fn the_correlation_identifier_bound_is_exact() -> Fallible {
    let at = "a".repeat(MAX_CORRELATION_BYTES);
    assert_eq!(CorrelationId::parse(at.as_bytes())?.as_str(), at);
    let over = "a".repeat(MAX_CORRELATION_BYTES.saturating_add(1));
    assert_eq!(
        CorrelationId::parse(over.as_bytes()),
        Err(DescriptorError::CorrelationId)
    );
    let line = String::from_utf8(line_with_planes(2))?;
    let long = line.replace("\"c-1\"", &format!("\"{over}\""));
    assert_eq!(
        decode_request(long.as_bytes()),
        Err(LineError::Descriptor(DescriptorError::CorrelationId))
    );
    let exact = line.replace("\"c-1\"", &format!("\"{at}\""));
    assert_eq!(
        decode_request(exact.as_bytes())?
            .frame
            .correlation_id
            .as_str(),
        at
    );
    Ok(())
}
