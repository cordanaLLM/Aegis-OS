// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E27-1: the JSON-RPC 2.0 line the descriptor travels in (D77).
//!
//! Positive: a request and both kinds of Response round-trip. Negative: a
//! `jsonrpc` member that is absent or not `"2.0"`, a line that names a
//! file-descriptor number at any level, another method, a missing or
//! non-numeric `id`, an unknown or duplicate member and a message that is not
//! exactly one line are each refused with their own error. Boundary: a line
//! exactly at [`MAX_LINE_BYTES`] is accepted and one byte longer refused.

mod common;

use aegis_scaena::line::{body, decode_request, decode_response, encode_response};
use aegis_scaena::{LineBuffer, LineError, MAX_LINE_BYTES, Refusal, Response};

use common::Fallible;

/// A valid request line, without its newline, from the fixture.
fn valid() -> Result<String, Box<dyn std::error::Error>> {
    let mut buffer = LineBuffer::new();
    let line = aegis_scaena::line::encode_request(&common::request(9)?, &mut buffer)?;
    Ok(String::from_utf8(body(line)?.to_vec())?)
}

/// `text` with one newline appended.
fn line(text: &str) -> Vec<u8> {
    format!("{text}\n").into_bytes()
}

/// `text` with `member` inserted as the first member of the object that
/// starts at `anchor`.
fn with_member(text: &str, anchor: &str, member: &str) -> String {
    text.replacen(anchor, &format!("{anchor}{member},"), 1)
}

// --- Positive -------------------------------------------------------------

/// Positive: the fixture request decodes, and both Responses round-trip.
#[test]
fn a_request_and_both_responses_round_trip() -> Fallible {
    let request = decode_request(&line(&valid()?))?;
    assert_eq!(request, common::request(9)?);
    let mut buffer = LineBuffer::new();
    for response in [
        Response {
            id: Some(9),
            outcome: Ok(()),
        },
        Response {
            id: Some(9),
            outcome: Err(Refusal::AttachRefused),
        },
        Response {
            id: None,
            outcome: Err(Refusal::Parse),
        },
    ] {
        let text = encode_response(&response, &mut buffer)?.to_vec();
        assert_eq!(decode_response(&text)?, response);
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a `jsonrpc` member that is absent or not `"2.0"` is refused.
#[test]
fn a_line_whose_jsonrpc_member_is_not_2_0_is_refused() -> Fallible {
    let text = valid()?;
    for version in ["\"1.0\"", "\"2.0 \"", "\"2\"", "2.0", "null"] {
        let other = text.replace("\"jsonrpc\":\"2.0\"", &format!("\"jsonrpc\":{version}"));
        let refused = decode_request(&line(&other));
        assert!(
            matches!(refused, Err(LineError::Version | LineError::Json { .. })),
            "{version}: {refused:?}"
        );
    }
    let absent = text.replace("\"jsonrpc\":\"2.0\",", "");
    assert_eq!(decode_request(&line(&absent)), Err(LineError::Version));
    Ok(())
}

/// Negative: a line that names a file-descriptor number, as `fd` or `fds`, at
/// the top, in the descriptor or in a plane, is refused as such.
#[test]
fn a_line_that_names_a_file_descriptor_number_is_refused() -> Fallible {
    let text = valid()?;
    let cases = [
        with_member(&text, "{", "\"fd\":5"),
        with_member(&text, "{", "\"fds\":[5]"),
        with_member(&text, "\"params\":{", "\"fd\":5"),
        with_member(&text, "\"params\":{", "\"fds\":[5]"),
        with_member(&text, "\"planes\":[{", "\"fd\":5"),
        with_member(&text, "\"planes\":[{", "\"fds\":[5]"),
    ];
    for case in &cases {
        assert_eq!(
            decode_request(&line(case)),
            Err(LineError::FdNumber),
            "{case}"
        );
    }
    Ok(())
}

/// Negative: another method, a missing or textual `id`, unknown and
/// duplicate members are refused.
#[test]
fn malformed_envelopes_are_refused() -> Fallible {
    let text = valid()?;
    let method = text.replace("STREAM_DECODED_FRAME", "MOUNT_SHELL_CANVAS");
    assert_eq!(decode_request(&line(&method)), Err(LineError::Method));
    let no_id = text.replace(",\"id\":9", "");
    assert_eq!(decode_request(&line(&no_id)), Err(LineError::Id));
    let start = text.find(",\"params\":").ok_or("no params")?;
    let end = text.find(",\"id\"").ok_or("no id")?;
    let no_params = format!(
        "{}{}",
        text.get(..start).unwrap_or(""),
        text.get(end..).unwrap_or("")
    );
    assert_eq!(decode_request(&line(&no_params)), Err(LineError::Params));
    for refused in [
        text.replace("\"id\":9", "\"id\":\"9\""),
        with_member(&text, "{", "\"extra\":1"),
        with_member(&text, "{", "\"method\":\"STREAM_DECODED_FRAME\""),
        with_member(&text, "\"params\":{", "\"stride\":64"),
        format!("[{text}]"),
        "{}".to_owned(),
    ] {
        let result = decode_request(&line(&refused));
        assert!(
            matches!(result, Err(LineError::Json { .. } | LineError::Version)),
            "{refused}: {result:?}"
        );
    }
    Ok(())
}

/// Negative: a message that is not exactly one newline-terminated line is
/// refused before it is parsed.
#[test]
fn a_message_that_is_not_one_line_is_refused() -> Fallible {
    let text = valid()?;
    for message in [
        text.clone().into_bytes(),
        format!("{text}\n\n").into_bytes(),
        format!("{text}\r\n").into_bytes(),
        text.replacen(',', ",\n", 1).into_bytes(),
        b"\n".to_vec(),
    ] {
        assert_eq!(decode_request(&message), Err(LineError::Framing));
    }
    Ok(())
}

/// Negative: the array form of the envelope, of the descriptor and of a
/// plane is refused, though serde's derived decoder would read each: the
/// values are right and in declaration order, and only the object form is
/// admitted.
#[test]
fn the_array_form_is_refused_at_every_level() -> Fallible {
    let text = valid()?;
    let frame = "[\"aegis.p08-p17.decoded-frame.v1\",\"c-1\",\"NV12\",0,64,35,\
                 [{\"offset\":0,\"pitch\":64},{\"offset\":2240,\"pitch\":64}]]";
    let params_start = text.find("\"params\":").ok_or("no params")?;
    let params_end = text.find(",\"id\"").ok_or("no id")?;
    let params = text
        .get(params_start..params_end)
        .ok_or("no params slice")?;
    let cases = [
        format!("[\"2.0\",\"STREAM_DECODED_FRAME\",{frame},9]"),
        text.replace(params, &format!("\"params\":{frame}")),
        text.replace("{\"offset\":0,\"pitch\":64}", "[0,64]"),
    ];
    for case in &cases {
        let refused = decode_request(&line(case));
        assert!(
            matches!(refused, Err(LineError::Json { .. })),
            "{case}: {refused:?}"
        );
    }
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a line of exactly `MAX_LINE_BYTES`, newline included, is
/// accepted, and one byte longer is refused before it is parsed.
#[test]
fn a_line_at_the_byte_bound_is_accepted_and_one_byte_longer_refused() -> Fallible {
    let text = valid()?;
    let pad = MAX_LINE_BYTES
        .checked_sub(text.len())
        .and_then(|room| room.checked_sub(1))
        .ok_or("the fixture line leaves no room to pad")?;
    // JSON admits whitespace between tokens; it pads without changing meaning.
    let at = text.replacen('{', &format!("{{{}", " ".repeat(pad)), 1);
    let at_bound = line(&at);
    assert_eq!(at_bound.len(), MAX_LINE_BYTES);
    assert_eq!(decode_request(&at_bound)?, common::request(9)?);
    let over = line(&text.replacen('{', &format!("{{{}", " ".repeat(pad.saturating_add(1))), 1));
    assert_eq!(over.len(), MAX_LINE_BYTES.saturating_add(1));
    assert_eq!(
        decode_request(&over),
        Err(LineError::TooLong {
            length: MAX_LINE_BYTES.saturating_add(1)
        })
    );
    Ok(())
}
