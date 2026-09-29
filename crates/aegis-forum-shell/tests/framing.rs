// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E16-3's D77 half: `DISPATCH_DECISION_REQUEST` and `EMIT_CARBON_TELEMETRY`
//! each arrive as one line-delimited JSON-RPC 2.0 message on a mocked
//! `AF_UNIX` stream, a `socketpair(2)`, whose every read has a deadline and
//! every line a byte bound (HISS-02).
//!
//! Positive: both messages arrive as one line each and update the state.
//! Negative: a line whose `jsonrpc` is not "2.0" is answered with a JSON-RPC
//! error and the stream continues; so are invalid JSON, a batch, an unknown
//! method and an unknown schema version, and an invalid Request whose id
//! could be read is answered with that id. Boundary: a line at the byte bound
//! is accepted and one byte longer is refused; a read that misses its
//! deadline ends the session instead of blocking it.

mod common;

use std::io::{BufRead, BufReader, Write};
use std::time::{Duration, Instant};

use aegis_forum_shell::canvas::seed;
use aegis_forum_shell::consumers::{DISPATCH_DECISION_REQUEST, EMIT_CARBON_TELEMETRY};
use aegis_forum_shell::jsonrpc::{
    EnvelopeError, INVALID_PARAMS, INVALID_REQUEST, LINE_TOO_LONG, MAX_LINE_BYTES,
    METHOD_NOT_FOUND, PARSE_ERROR,
};
use aegis_forum_shell::session::{Limits, LineOutcome, Refusal, SessionEnd, SessionReport, serve};
use aegis_forum_shell::state::{Applied, ShellState};
use serde_json::Value;

use common::{
    Fallible, Scripted, decision_payload, decision_request, message, socketpair, telemetry,
    telemetry_payload,
};

/// The deadline the socket tests arm: short, so a missed one ends quickly.
const DEADLINE: Duration = Duration::from_millis(200);

/// Limits with the short deadline.
fn limits() -> Limits {
    Limits {
        read_deadline: DEADLINE,
        write_deadline: DEADLINE,
        ..Limits::default()
    }
}

/// The two valid messages, as Notifications.
fn valid_lines() -> Fallible<(String, String)> {
    let decision = decision_payload(&decision_request("request-0001")?)?;
    let update = telemetry_payload(&telemetry(14.2, 0.6)?)?;
    Ok((
        message(DISPATCH_DECISION_REQUEST, &decision, None),
        message(EMIT_CARBON_TELEMETRY, &update, None),
    ))
}

/// Writes `input` from the producer's end, closes its write half, serves the
/// session on the shell's end and reads every Response back.
fn exchange(input: &str) -> Fallible<(ShellState, SessionReport, Vec<Value>)> {
    let (mut shell, mut producer) = socketpair()?;
    producer.write_all(input.as_bytes())?;
    producer.shutdown(std::net::Shutdown::Write)?;
    let mut state = ShellState::new(seed::empty()?);
    let report = serve(&mut shell, &mut state, &limits())?;
    drop(shell);
    let mut responses = Vec::new();
    for line in BufReader::new(producer).lines().take(64) {
        responses.push(serde_json::from_str(&line?)?);
    }
    Ok((state, report, responses))
}

/// The error code and id of one Response.
fn code_and_id(response: &Value) -> (Option<i64>, Value) {
    let code = response.pointer("/error/code").and_then(Value::as_i64);
    (
        code,
        response.get("id").cloned().unwrap_or(Value::Bool(false)),
    )
}

// --- Positive -------------------------------------------------------------

/// Positive: a decision request and a telemetry update each arrive as one
/// JSON-RPC 2.0 line and update the state; Notifications get no Response.
#[cfg(unix)]
#[test]
fn both_edges_arrive_as_one_line_each_and_update_the_state() -> Fallible {
    let (decision, update) = valid_lines()?;
    assert_eq!(decision.matches('\n').count(), 1);
    assert_eq!(update.matches('\n').count(), 1);
    let (state, report, responses) = exchange(&format!("{decision}{update}"))?;
    assert_eq!(report.end, SessionEnd::EndOfStream);
    assert_eq!(
        report.outcomes,
        vec![
            LineOutcome::Applied(Applied::Decision(0)),
            LineOutcome::Applied(Applied::Telemetry(
                aegis_forum_shell::lifecycle::WindowVerdict::WithinBudget
            )),
        ]
    );
    assert!(
        responses.is_empty(),
        "a Notification is never answered: {responses:?}"
    );
    assert_eq!(state.decisions().len(), 1);
    assert!(state.telemetry().is_some());
    Ok(())
}

/// Positive: a call (a message with an id) is answered with a result
/// carrying the same id.
#[cfg(unix)]
#[test]
fn a_call_is_answered_with_its_id() -> Fallible {
    let update = telemetry_payload(&telemetry(14.2, 0.6)?)?;
    let (_, report, responses) = exchange(&message(EMIT_CARBON_TELEMETRY, &update, Some(41)))?;
    assert_eq!(report.responses, 1);
    let response = responses.first().ok_or("no Response")?;
    assert_eq!(response.get("jsonrpc"), Some(&Value::from("2.0")));
    assert_eq!(response.get("id"), Some(&Value::from(41)));
    assert_eq!(
        response.pointer("/result/applied"),
        Some(&Value::from(EMIT_CARBON_TELEMETRY))
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: `jsonrpc` other than "2.0" is answered with Invalid Request, and
/// the next line is still read and applied.
#[cfg(unix)]
#[test]
fn a_wrong_jsonrpc_version_is_answered_and_the_stream_continues() -> Fallible {
    let (decision, update) = valid_lines()?;
    let wrong = update.replacen("\"jsonrpc\":\"2.0\"", "\"jsonrpc\":\"1.0\"", 1);
    let missing = update.replacen("\"jsonrpc\":\"2.0\",", "", 1);
    assert_ne!(wrong, update);
    let (state, report, responses) = exchange(&format!("{wrong}{missing}{decision}"))?;
    assert_eq!(responses.len(), 2);
    for response in &responses {
        assert_eq!(code_and_id(response), (Some(INVALID_REQUEST), Value::Null));
    }
    let refused = LineOutcome::Refused {
        refusal: Refusal::Envelope(EnvelopeError::Version),
        answered: true,
    };
    assert_eq!(report.outcomes.first(), Some(&refused));
    assert_eq!(
        report.outcomes.get(2),
        Some(&LineOutcome::Applied(Applied::Decision(0)))
    );
    assert_eq!(
        state.decisions().len(),
        1,
        "the stream continued past the refusals"
    );
    Ok(())
}

/// Negative: invalid JSON, a batch, a non-object, an unknown method and a
/// wrong-typed id are each answered with the specification's code.
#[cfg(unix)]
#[test]
fn every_invalid_line_is_answered_with_the_specifications_code() -> Fallible {
    let (_, update) = valid_lines()?;
    let payload = telemetry_payload(&telemetry(14.2, 0.6)?)?;
    let object_id = format!(
        "{{\"jsonrpc\":\"2.0\",\"method\":\"{EMIT_CARBON_TELEMETRY}\",\"params\":{payload},\"id\":{{\"nested\":true}}}}\n"
    );
    let input = [
        "{not json\n".to_owned(),
        format!("[{}]\n", update.trim_end()),
        "42\n".to_owned(),
        message("SYNC_DESKTOP_SHELL", "{}", Some(7)),
        object_id,
        update.clone(),
    ]
    .concat();
    let (state, report, responses) = exchange(&input)?;
    let codes: Vec<(Option<i64>, Value)> = responses.iter().map(code_and_id).collect();
    assert_eq!(codes.first(), Some(&(Some(PARSE_ERROR), Value::Null)));
    assert_eq!(
        codes.get(1),
        Some(&(Some(INVALID_REQUEST), Value::Null)),
        "a batch"
    );
    assert_eq!(
        codes.get(2),
        Some(&(Some(INVALID_REQUEST), Value::Null)),
        "not an object"
    );
    assert_eq!(
        codes.get(3),
        Some(&(Some(METHOD_NOT_FOUND), Value::from(7)))
    );
    assert_eq!(
        codes.get(4),
        Some(&(Some(INVALID_REQUEST), Value::Null)),
        "an object id"
    );
    assert_eq!(codes.len(), 5, "the valid Notification is not answered");
    assert_eq!(report.outcomes.len(), 6);
    assert!(
        state.telemetry().is_some(),
        "the last, valid line was applied"
    );
    Ok(())
}

/// Negative: an invalid Request whose id could be read -- a wrong `jsonrpc`,
/// a missing `method`, a scalar `params` -- is answered with Invalid Request
/// carrying that id, as the specification requires, and the stream
/// continues; an explicit null id is answered with null.
#[cfg(unix)]
#[test]
fn an_invalid_request_with_a_readable_id_is_answered_with_that_id() -> Fallible {
    let (decision, _) = valid_lines()?;
    let payload = telemetry_payload(&telemetry(14.2, 0.6)?)?;
    let input = [
        format!(
            "{{\"jsonrpc\":\"1.0\",\"method\":\"{EMIT_CARBON_TELEMETRY}\",\"params\":{payload},\"id\":5}}\n"
        ),
        format!("{{\"jsonrpc\":\"2.0\",\"params\":{payload},\"id\":\"x\"}}\n"),
        format!(
            "{{\"jsonrpc\":\"2.0\",\"method\":\"{EMIT_CARBON_TELEMETRY}\",\"params\":7,\"id\":9}}\n"
        ),
        format!("{{\"jsonrpc\":\"1.0\",\"method\":\"{EMIT_CARBON_TELEMETRY}\",\"id\":null}}\n"),
        decision,
    ]
    .concat();
    let (state, report, responses) = exchange(&input)?;
    let codes: Vec<(Option<i64>, Value)> = responses.iter().map(code_and_id).collect();
    assert_eq!(
        codes,
        vec![
            (Some(INVALID_REQUEST), Value::from(5)),
            (Some(INVALID_REQUEST), Value::from("x")),
            (Some(INVALID_REQUEST), Value::from(9)),
            (Some(INVALID_REQUEST), Value::Null),
        ]
    );
    let refused = |error| LineOutcome::Refused {
        refusal: Refusal::Envelope(error),
        answered: true,
    };
    assert_eq!(
        report.outcomes,
        vec![
            refused(EnvelopeError::Version),
            refused(EnvelopeError::Method),
            refused(EnvelopeError::Params),
            refused(EnvelopeError::Version),
            LineOutcome::Applied(Applied::Decision(0)),
        ]
    );
    assert_eq!(
        state.decisions().len(),
        1,
        "the stream continued past the refusals"
    );
    Ok(())
}

/// Negative: an unknown schema version in a call is answered with Invalid
/// params carrying the producer's typed error; in a Notification it is not
/// answered, and the report carries the typed error instead.
#[cfg(unix)]
#[test]
fn an_unknown_schema_version_is_refused_with_a_typed_error_on_the_wire() -> Fallible {
    let update = telemetry_payload(&telemetry(14.2, 0.6)?)?.replacen(
        "carbon-telemetry.v1",
        "carbon-telemetry.v2",
        1,
    );
    let input = format!(
        "{}{}",
        message(EMIT_CARBON_TELEMETRY, &update, Some(3)),
        message(EMIT_CARBON_TELEMETRY, &update, None)
    );
    let (state, report, responses) = exchange(&input)?;
    assert_eq!(responses.len(), 1, "only the call is answered");
    let response = responses.first().ok_or("no Response")?;
    assert_eq!(
        code_and_id(response),
        (Some(INVALID_PARAMS), Value::from(3))
    );
    let text = response
        .pointer("/error/message")
        .and_then(Value::as_str)
        .unwrap_or_default();
    assert!(
        text.contains("contract version this build does not admit"),
        "{text}"
    );
    let typed = |outcome: Option<&LineOutcome>| {
        matches!(
            outcome,
            Some(LineOutcome::Refused {
                refusal: Refusal::Consume(aegis_forum_shell::consumers::ConsumeError::Telemetry(
                    aegis_tellus::ContractError::UnknownVersion { .. }
                )),
                ..
            })
        )
    };
    assert!(
        typed(report.outcomes.first()) && typed(report.outcomes.get(1)),
        "{report:?}"
    );
    assert!(state.telemetry().is_none());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Pads a valid message line to exactly `bytes` bytes before its newline with
/// JSON whitespace, which leaves its meaning unchanged.
fn padded(line: &str, bytes: usize) -> Fallible<String> {
    let body = line
        .trim_end_matches('\n')
        .strip_suffix('}')
        .ok_or("not an object")?;
    let pad = bytes
        .checked_sub(body.len().saturating_add(1))
        .ok_or("line already too long")?;
    Ok(format!("{body}{}}}\n", " ".repeat(pad)))
}

/// Boundary: a line of exactly `MAX_LINE_BYTES` is accepted; one byte longer
/// is refused with the implementation-defined code, and the stream continues.
#[cfg(unix)]
#[test]
fn a_line_at_the_bound_is_accepted_and_one_byte_longer_is_refused() -> Fallible {
    let (decision, update) = valid_lines()?;
    let at_bound = padded(&update, MAX_LINE_BYTES)?;
    let past_bound = padded(&update, MAX_LINE_BYTES.saturating_add(1))?;
    assert_eq!(at_bound.len(), MAX_LINE_BYTES.saturating_add(1));
    assert_eq!(past_bound.len(), MAX_LINE_BYTES.saturating_add(2));
    let (state, report, responses) = exchange(&format!("{at_bound}{past_bound}{decision}"))?;
    assert!(matches!(
        report.outcomes.first(),
        Some(LineOutcome::Applied(Applied::Telemetry(_)))
    ));
    let too_long = Refusal::TooLong {
        length: MAX_LINE_BYTES.saturating_add(1),
    };
    assert_eq!(
        report.outcomes.get(1),
        Some(&LineOutcome::Refused {
            refusal: too_long,
            answered: true
        })
    );
    assert_eq!(responses.len(), 1);
    assert_eq!(
        responses.first().map(code_and_id),
        Some((Some(LINE_TOO_LONG), Value::Null))
    );
    assert_eq!(
        state.decisions().len(),
        1,
        "the line after the refused one was applied"
    );
    Ok(())
}

/// Boundary: with no line arriving, the read misses its deadline and the
/// session ends, instead of blocking; a line already read was applied. The
/// bound on the elapsed time sits below the fixture's backstop timeout, so a
/// read the code under test left without its deadline fails here.
#[cfg(unix)]
#[test]
fn a_missed_read_deadline_ends_the_session() -> Fallible {
    let bound = DEADLINE.saturating_mul(25);
    assert!(
        bound < common::BACKSTOP,
        "the bound {bound:?} does not sit below the backstop"
    );
    let (mut shell, mut producer) = socketpair()?;
    let (_, update) = valid_lines()?;
    producer.write_all(update.as_bytes())?;
    let mut state = ShellState::new(seed::empty()?);
    let started = Instant::now();
    let report = serve(&mut shell, &mut state, &limits())?;
    let elapsed = started.elapsed();
    assert_eq!(report.end, SessionEnd::Deadline);
    assert_eq!(report.outcomes.len(), 1);
    assert!(
        elapsed >= DEADLINE,
        "ended after {elapsed:?}, before the deadline"
    );
    assert!(elapsed < bound, "ended only after {elapsed:?}");
    Ok(())
}

/// Boundary: every read is preceded by arming its deadline, and every write
/// too, whatever size the reads come in; a stream ending mid-line is
/// answered.
#[test]
fn every_read_and_every_write_arms_its_deadline() -> Fallible {
    let (decision, update) = valid_lines()?;
    let unterminated = update.trim_end();
    let input = format!("{decision}{}{unterminated}", message("NOPE", "{}", Some(1)));
    let mut stream = Scripted::new(input.as_bytes(), 7, true);
    let mut state = ShellState::new(seed::empty()?);
    let report = serve(&mut stream, &mut state, &Limits::default())?;
    assert_eq!(report.end, SessionEnd::EndOfStream);
    assert_eq!(stream.armed_reads, stream.reads);
    assert!(
        stream.reads > input.len() / 7,
        "reads came in 7-byte pieces"
    );
    assert_eq!(stream.armed_writes, report.responses);
    assert_eq!(
        report.responses, 2,
        "the unknown method and the unterminated line"
    );
    let last = report.outcomes.last();
    assert_eq!(
        last,
        Some(&LineOutcome::Refused {
            refusal: Refusal::Unterminated,
            answered: true
        })
    );
    Ok(())
}

/// Boundary: the session reads at most `max_lines` lines.
#[test]
fn the_session_stops_at_its_line_bound() -> Fallible {
    let (_, update) = valid_lines()?;
    let input = update.repeat(5);
    let mut stream = Scripted::new(input.as_bytes(), 4_096, false);
    let mut state = ShellState::new(seed::empty()?);
    let bounded = Limits {
        max_lines: 3,
        ..Limits::default()
    };
    let report = serve(&mut stream, &mut state, &bounded)?;
    assert_eq!(report.end, SessionEnd::LineLimit);
    assert_eq!(report.outcomes.len(), 3);
    let mut stream = Scripted::new(input.as_bytes(), 4_096, false);
    let report = serve(&mut stream, &mut state, &Limits::default())?;
    assert_eq!(
        report.end,
        SessionEnd::Deadline,
        "the scripted stream then misses its deadline"
    );
    assert_eq!(report.outcomes.len(), 5);
    Ok(())
}

/// Boundary: an over-long line is discarded up to `MAX_DISCARD_BYTES`
/// while the reader looks for its end; one that runs past that gives the
/// stream up with a read error rather than reading on without bound.
#[test]
fn an_endless_line_gives_the_stream_up_past_the_discard_bound() -> Fallible {
    use aegis_forum_shell::jsonrpc::{FrameError, MAX_DISCARD_BYTES};
    use aegis_forum_shell::session::SessionError;
    let mut ended = vec![b'x'; MAX_DISCARD_BYTES];
    ended.push(b'\n');
    let mut stream = Scripted::new(&ended, 65_536, true);
    let mut state = ShellState::new(seed::empty()?);
    let report = serve(&mut stream, &mut state, &Limits::default())?;
    let too_long = Refusal::TooLong {
        length: MAX_DISCARD_BYTES,
    };
    let refused = LineOutcome::Refused {
        refusal: too_long,
        answered: true,
    };
    assert_eq!(report.outcomes.first(), Some(&refused));
    let endless = vec![b'x'; MAX_DISCARD_BYTES.saturating_add(MAX_LINE_BYTES)];
    let mut stream = Scripted::new(&endless, 65_536, true);
    let failed = serve(&mut stream, &mut state, &Limits::default());
    assert_eq!(failed, Err(SessionError::Read(FrameError::Flood)));
    Ok(())
}
