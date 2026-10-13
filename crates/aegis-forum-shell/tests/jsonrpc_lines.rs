// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D77 framing, called directly: `LineReader::next_frame`,
//! `RequestId::to_value`, `error_line` and `result_line`.
//!
//! Positive: lines are cut at newlines whatever the read size, each read arms
//! its deadline first, ids convert to their JSON value, and a Response line
//! parses back to the object the specification names. Negative: a missed
//! deadline, a failed read, a failed arm and an endless line are errors, and
//! an error Response carries no `result` (and the reverse). Boundary: a line
//! of exactly `MAX_LINE_BYTES` is a line and one byte more is `TooLong`; a
//! discard of exactly `MAX_DISCARD_BYTES` is `TooLong` and one more is
//! `Flood`; empty lines, unterminated tails, extreme ids and codes, and text
//! with newlines and quotes stay on one line.

mod common;

use std::io::{self, Read, Write};
use std::time::Duration;

use aegis_forum_shell::jsonrpc::{
    DeadlineStream, Frame, FrameError, LineReader, MAX_DISCARD_BYTES, MAX_LINE_BYTES, PARSE_ERROR,
    RequestId, error_line, parse_line, result_line,
};
use serde_json::{Number, Value, json};

use common::{Fallible, Scripted};

/// The deadline the reader arms; the stubs ignore its value.
const DEADLINE: Duration = Duration::from_millis(50);

/// A stream whose reads, or whose deadline arming, fail with a fixed kind.
struct Failing {
    read_kind: Option<io::ErrorKind>,
    arm_kind: Option<io::ErrorKind>,
}

impl Read for Failing {
    fn read(&mut self, _buf: &mut [u8]) -> io::Result<usize> {
        self.read_kind
            .map_or(Ok(0), |kind| Err(io::Error::from(kind)))
    }
}

impl Write for Failing {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

impl DeadlineStream for Failing {
    fn arm_read_deadline(&mut self, _deadline: Duration) -> io::Result<()> {
        self.arm_kind
            .map_or(Ok(()), |kind| Err(io::Error::from(kind)))
    }

    fn arm_write_deadline(&mut self, _deadline: Duration) -> io::Result<()> {
        Ok(())
    }
}

/// `length` bytes of `a` followed by a newline.
fn line_of(length: usize) -> Vec<u8> {
    let mut bytes = vec![b'a'; length];
    bytes.push(b'\n');
    bytes
}

/// A Response line parsed back into a JSON value, after checking that it is
/// exactly one line.
fn parsed(line: &[u8]) -> Fallible<Value> {
    assert_eq!(line.last(), Some(&b'\n'));
    assert_eq!(
        line.iter().position(|byte| *byte == b'\n'),
        Some(line.len().saturating_sub(1))
    );
    Ok(serde_json::from_slice(line)?)
}

// --- Positive -------------------------------------------------------------

/// Positive: two lines and the end, cut at the newlines whatever the read
/// size (1 byte, 3 bytes, or everything at once); an empty line is a line.
#[test]
fn lines_are_cut_at_newlines_whatever_the_read_size() -> Fallible {
    for chunk in [1, 3, 4_096] {
        let mut stream = Scripted::new(b"abc\n\ndef\n", chunk, true);
        let mut reader = LineReader::new();
        assert_eq!(
            reader.next_frame(&mut stream, DEADLINE)?,
            Frame::Line(b"abc")
        );
        assert_eq!(reader.next_frame(&mut stream, DEADLINE)?, Frame::Line(b""));
        assert_eq!(
            reader.next_frame(&mut stream, DEADLINE)?,
            Frame::Line(b"def")
        );
        assert_eq!(reader.next_frame(&mut stream, DEADLINE)?, Frame::End);
    }
    Ok(())
}

/// Positive: every read is preceded by arming the deadline: 8 bytes in reads
/// of 2 is 4 reads for the line, so 4 arms; the next frame is served from the
/// buffer or the end and arms once more per read.
#[test]
fn every_read_arms_the_deadline_first() -> Fallible {
    let mut stream = Scripted::new(b"abcdefg\n", 2, true);
    let mut reader = LineReader::new();
    assert_eq!(
        reader.next_frame(&mut stream, DEADLINE)?,
        Frame::Line(b"abcdefg")
    );
    assert_eq!(stream.reads, 4);
    assert_eq!(stream.armed_reads, 4);
    assert_eq!(reader.next_frame(&mut stream, DEADLINE)?, Frame::End);
    assert_eq!(stream.reads, 5);
    assert_eq!(stream.armed_reads, 5);
    Ok(())
}

/// Positive: bytes after the last newline come back as `Unterminated` when
/// the peer ends the stream, then the stream is at its end.
#[test]
fn an_unterminated_tail_is_reported_then_the_end() -> Fallible {
    let mut stream = Scripted::new(b"one\ntail", 4_096, true);
    let mut reader = LineReader::new();
    assert_eq!(
        reader.next_frame(&mut stream, DEADLINE)?,
        Frame::Line(b"one")
    );
    assert_eq!(
        reader.next_frame(&mut stream, DEADLINE)?,
        Frame::Unterminated(b"tail")
    );
    assert_eq!(reader.next_frame(&mut stream, DEADLINE)?, Frame::End);
    Ok(())
}

/// Positive: the three id kinds convert to the JSON value they came from.
#[test]
fn a_request_id_converts_to_its_json_value() {
    assert_eq!(RequestId::Null.to_value(), Value::Null);
    assert_eq!(RequestId::Number(Number::from(7)).to_value(), json!(7));
    assert_eq!(
        RequestId::String("req-1".to_owned()).to_value(),
        json!("req-1")
    );
}

/// Positive: an error Response is `{"jsonrpc":"2.0","error":{code,message},
/// "id":...}` on one line.
#[test]
fn an_error_line_is_a_json_rpc_error_response() -> Fallible {
    let line = error_line(&RequestId::Number(Number::from(1)), PARSE_ERROR, "bad json");
    let expected = json!({
        "jsonrpc": "2.0",
        "error": { "code": -32_700, "message": "bad json" },
        "id": 1,
    });
    assert_eq!(parsed(&line)?, expected);
    Ok(())
}

/// Positive: a success Response is `{"jsonrpc":"2.0","result":...,"id":...}`
/// on one line, with a structured result kept whole.
#[test]
fn a_result_line_is_a_json_rpc_success_response() -> Fallible {
    let id = RequestId::String("call-9".to_owned());
    let line = result_line(&id, &json!({ "applied": "method", "n": [1, 2] }));
    let expected = json!({
        "jsonrpc": "2.0",
        "result": { "applied": "method", "n": [1, 2] },
        "id": "call-9",
    });
    assert_eq!(parsed(&line)?, expected);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a stream that stays silent after a partial line misses its
/// deadline: `Deadline`, not a hang and not a line.
#[test]
fn a_silent_stream_is_a_missed_deadline() {
    let mut stream = Scripted::new(b"partial", 4_096, false);
    let mut reader = LineReader::new();
    assert_eq!(
        reader.next_frame(&mut stream, DEADLINE).err(),
        Some(FrameError::Deadline)
    );
}

/// Negative: a failing read is `Io` with its kind, a timed-out read is
/// `Deadline`, and a deadline that cannot be armed fails before any read.
#[test]
fn failing_reads_and_arms_are_errors_with_their_kind() {
    let mut reader = LineReader::new();
    let mut reset = Failing {
        read_kind: Some(io::ErrorKind::ConnectionReset),
        arm_kind: None,
    };
    assert_eq!(
        reader.next_frame(&mut reset, DEADLINE).err(),
        Some(FrameError::Io(io::ErrorKind::ConnectionReset))
    );
    let mut timed_out = Failing {
        read_kind: Some(io::ErrorKind::TimedOut),
        arm_kind: None,
    };
    assert_eq!(
        reader.next_frame(&mut timed_out, DEADLINE).err(),
        Some(FrameError::Deadline)
    );
    let mut unarmed = Failing {
        read_kind: None,
        arm_kind: Some(io::ErrorKind::PermissionDenied),
    };
    assert_eq!(
        reader.next_frame(&mut unarmed, DEADLINE).err(),
        Some(FrameError::Io(io::ErrorKind::PermissionDenied))
    );
}

/// Negative: a line that never ends is `Flood` once more than
/// `MAX_DISCARD_BYTES` were discarded.
#[test]
fn an_endless_line_is_a_flood() {
    let endless = vec![b'a'; MAX_DISCARD_BYTES.saturating_mul(2)];
    let mut stream = Scripted::new(&endless, 65_536, false);
    let mut reader = LineReader::new();
    assert_eq!(
        reader.next_frame(&mut stream, DEADLINE).err(),
        Some(FrameError::Flood)
    );
}

/// Negative: an error Response has an `error` and no `result`; a success
/// Response has a `result` and no `error`.
#[test]
fn the_two_responses_never_share_a_member() -> Fallible {
    let error = parsed(&error_line(&RequestId::Null, -1, "m"))?;
    assert!(error.get("error").is_some() && error.get("result").is_none());
    let result = parsed(&result_line(&RequestId::Null, &json!(true)))?;
    assert!(result.get("result").is_some() && result.get("error").is_none());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a line of exactly `MAX_LINE_BYTES` is a line; one byte more is
/// `TooLong` with that length (8193), and the next line is read normally.
#[test]
fn the_line_bound_is_inclusive() -> Fallible {
    let mut at_limit = Scripted::new(&line_of(MAX_LINE_BYTES), 4_096, true);
    let mut first = LineReader::new();
    let frame = first.next_frame(&mut at_limit, DEADLINE)?;
    assert_eq!(frame, Frame::Line(&vec![b'a'; MAX_LINE_BYTES]));
    let mut input = line_of(MAX_LINE_BYTES.saturating_add(1));
    input.extend_from_slice(b"ok\n");
    let mut past = Scripted::new(&input, 4_096, true);
    let mut reader = LineReader::new();
    assert_eq!(
        reader.next_frame(&mut past, DEADLINE)?,
        Frame::TooLong { length: 8_193 }
    );
    assert_eq!(reader.next_frame(&mut past, DEADLINE)?, Frame::Line(b"ok"));
    Ok(())
}

/// Boundary: a 20,000-byte line is `TooLong` with its full length, and the
/// reader resynchronises on the next line; one that ends the stream without
/// a newline reports the discarded length too.
#[test]
fn a_long_line_is_discarded_whole_and_the_reader_resynchronises() -> Fallible {
    let mut input = line_of(20_000);
    input.extend_from_slice(b"ok\n");
    let mut stream = Scripted::new(&input, 4_096, true);
    let mut reader = LineReader::new();
    assert_eq!(
        reader.next_frame(&mut stream, DEADLINE)?,
        Frame::TooLong { length: 20_000 }
    );
    assert_eq!(
        reader.next_frame(&mut stream, DEADLINE)?,
        Frame::Line(b"ok")
    );
    let mut cut_off = Scripted::new(&vec![b'a'; 9_000], 4_096, true);
    assert_eq!(
        LineReader::new().next_frame(&mut cut_off, DEADLINE)?,
        Frame::TooLong { length: 9_000 }
    );
    Ok(())
}

/// Boundary: discarding exactly `MAX_DISCARD_BYTES` before the newline is
/// `TooLong`; one more byte with no newline in sight is `Flood`.
#[test]
fn the_discard_bound_is_inclusive() -> Fallible {
    let mut at_limit = Scripted::new(&line_of(MAX_DISCARD_BYTES), 4_096, false);
    assert_eq!(
        LineReader::new().next_frame(&mut at_limit, DEADLINE)?,
        Frame::TooLong {
            length: MAX_DISCARD_BYTES
        }
    );
    let endless = vec![b'a'; MAX_DISCARD_BYTES.saturating_add(1)];
    let mut past = Scripted::new(&endless, 4_096, false);
    assert_eq!(
        LineReader::new().next_frame(&mut past, DEADLINE).err(),
        Some(FrameError::Flood)
    );
    Ok(())
}

/// Boundary: the extreme ids survive: the largest u64, a negative number, a
/// float, the empty string, and a string that looks like a number (which stays
/// a string).
#[test]
fn extreme_request_ids_keep_their_kind() -> Fallible {
    let big = RequestId::Number(Number::from(u64::MAX));
    assert_eq!(big.to_value(), json!(18_446_744_073_709_551_615_u64));
    assert_eq!(RequestId::Number(Number::from(-5)).to_value(), json!(-5));
    let float = Number::from_f64(1.5).map(RequestId::Number);
    assert_eq!(float.map(|id| id.to_value()), Some(json!(1.5)));
    let empty = RequestId::String(String::new()).to_value();
    assert_eq!(empty, json!(""));
    assert!(empty.is_string());
    assert!(RequestId::String("7".to_owned()).to_value().is_string());
    let read = parse_line(br#"{"jsonrpc":"2.0","method":"m","id":9007199254740993}"#)
        .map_err(|(error, _)| error)?;
    assert_eq!(
        read.id.map(|id| id.to_value()),
        Some(json!(9_007_199_254_740_993_u64))
    );
    Ok(())
}

/// Boundary: a message with a newline, a quote and a backslash is escaped
/// onto one line and round-trips; the extreme codes and an empty message and
/// a null result survive; a 20,000-byte message is still one line.
#[test]
fn awkward_text_and_extreme_codes_stay_on_one_line() -> Fallible {
    let message = "line one\nline \"two\"\\end";
    let line = error_line(&RequestId::Null, i64::MIN, message);
    let value = parsed(&line)?;
    assert_eq!(value.pointer("/error/message"), Some(&json!(message)));
    assert_eq!(value.pointer("/error/code"), Some(&json!(i64::MIN)));
    assert_eq!(value.pointer("/id"), Some(&Value::Null));
    let max = parsed(&error_line(&RequestId::Null, i64::MAX, ""))?;
    assert_eq!(max.pointer("/error/code"), Some(&json!(i64::MAX)));
    assert_eq!(max.pointer("/error/message"), Some(&json!("")));
    let null = parsed(&result_line(&RequestId::Null, &Value::Null))?;
    assert_eq!(null.get("result"), Some(&Value::Null));
    let long = "x".repeat(20_000);
    let wide = parsed(&result_line(&RequestId::Null, &json!(long)))?;
    assert_eq!(wide.pointer("/result"), Some(&json!(long)));
    Ok(())
}
