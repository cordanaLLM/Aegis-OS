// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The transition trace, held to the four properties M24 needs from it.
//!
//! M24 boots a real A/B sysupdate transfer under QEMU and has to decide
//! whether what happened matches this model. The comparison it makes is
//! `expected.render()? == observed_text`, so the tests here are about the
//! bytes: a golden rendering of the happy path, a round trip that is an exact
//! fixed point, and a refusal for every way a trace could be misread --
//! a foreign schema, a misplaced record kind, a broken sequence, a miscounted
//! header, a body cut short, a slot pair that is not an A/B pair, and an
//! unknown field.
//!
//! The cut-short case is tested in both directions on purpose. Tampering the
//! header's count downward and dropping a body line both end in
//! `TraceError::Count`, but only the second is what a lost trace actually
//! looks like, and the M15 verification found that only the first was covered.
//!
//! The golden text below is the whole artefact. If it changes, the schema tag
//! has to change with it, and this test is where that is noticed.

mod common;

use aegis_janus_lifecycle::{
    Event, EventKind, MAX_TRANSITIONS, RecordKind, Slot, State, TRACE_SCHEMA, Tick, Trace,
    TraceError, TraceHeader, TraceLine, Transition,
};

use common::{ARMED_AT, Fallible, SIGNED_HEX, armed, blessed, candidate};

/// The rendered happy path, byte for byte.
const GOLDEN: &str = concat!(
    r#"{"schema":"aegis.p02.ab-transition.v1","record":"header","version":"1.4.0","target":"b","fallback":"a","expected-root-hash":"a1b2c3d4e5f60718293a4b5c6d7e8f90a1b2c3d4e5f60718293a4b5c6d7e8f90","transitions":6}"#,
    "\n",
    r#"{"schema":"aegis.p02.ab-transition.v1","record":"transition","seq":0,"at":1000,"event":"declare","from":"idle","to":"candidate-declared","slot":"b"}"#,
    "\n",
    r#"{"schema":"aegis.p02.ab-transition.v1","record":"transition","seq":1,"at":1000,"event":"check-signature","from":"candidate-declared","to":"signature-verified","slot":"b"}"#,
    "\n",
    r#"{"schema":"aegis.p02.ab-transition.v1","record":"transition","seq":2,"at":1000,"event":"acquire-delta","from":"signature-verified","to":"delta-acquired","slot":"b"}"#,
    "\n",
    r#"{"schema":"aegis.p02.ab-transition.v1","record":"transition","seq":3,"at":1000,"event":"swap-slot","from":"delta-acquired","to":"slot-swapped","slot":"b"}"#,
    "\n",
    r#"{"schema":"aegis.p02.ab-transition.v1","record":"transition","seq":4,"at":1000,"event":"arm-watchdog","from":"slot-swapped","to":"watchdog-armed","slot":"b"}"#,
    "\n",
    r#"{"schema":"aegis.p02.ab-transition.v1","record":"transition","seq":5,"at":1000,"event":"bless","from":"watchdog-armed","to":"blessed","slot":"b"}"#,
    "\n",
);

/// Returns the golden text with `from` replaced by `to`, once.
fn tampered(from: &str, to: &str) -> Result<String, Box<dyn std::error::Error>> {
    if !GOLDEN.contains(from) {
        return Err(format!("the golden trace does not contain {from:?}").into());
    }
    Ok(GOLDEN.replacen(from, to, 1))
}

/// Returns the error `Trace::parse` produced, or a message when it accepted.
fn rejected(text: &str) -> Result<TraceError, Box<dyn std::error::Error>> {
    match Trace::parse(text) {
        Ok(trace) => Err(format!("the trace was accepted with {} records", trace.len()).into()),
        Err(error) => Ok(error),
    }
}

/// Returns the golden text with its last transition line dropped.
///
/// This is the direction a trace is actually lost in -- a write cut short, a
/// file copied while it was still being appended to -- and it is the opposite
/// of tampering the header's count downward: the header still declares six
/// transitions and only five follow.
fn truncated() -> String {
    let keep = GOLDEN.lines().count().saturating_sub(1);
    let mut out = String::new();
    for line in GOLDEN.lines().take(keep.min(MAX_TRANSITIONS)) {
        out.push_str(line);
        out.push('\n');
    }
    out
}

// --- Positive -------------------------------------------------------------

/// Positive: the happy path renders to exactly the recorded bytes.
#[test]
fn the_happy_path_renders_to_the_golden_trace() -> Fallible {
    let (machine, _clock) = blessed()?;
    assert_eq!(machine.trace().render()?, GOLDEN);
    Ok(())
}

/// Positive: rendering is a fixed point of parsing, in both directions.
///
/// This is what lets M24 compare in whichever direction it holds the input
/// for: a trace read from a file and rendered again is the same bytes, and a
/// trace built here and parsed back is the same value.
#[test]
fn rendering_is_a_fixed_point_of_parsing() -> Fallible {
    let (machine, _clock) = blessed()?;
    let rendered = machine.trace().render()?;
    let parsed = Trace::parse(&rendered)?;
    assert_eq!(&parsed, machine.trace());
    assert_eq!(parsed.render()?, rendered);
    assert_eq!(Trace::parse(GOLDEN)?.render()?, GOLDEN);
    Ok(())
}

/// Positive: the header says what the trace is about, not only what happened.
#[test]
fn the_header_names_the_candidate_and_both_slots() -> Fallible {
    let (machine, _clock) = blessed()?;
    let header: TraceHeader = machine.trace().header()?;
    assert_eq!(header.schema, TRACE_SCHEMA);
    assert_eq!(header.record, RecordKind::Header);
    assert_eq!(header.record.name(), "header");
    assert_eq!(header.version, "1.4.0");
    assert_eq!(header.target, Slot::B);
    assert_eq!(header.fallback, Slot::A);
    assert_eq!(header.expected_root_hash, SIGNED_HEX);
    assert_eq!(header.transitions, 6);
    assert_eq!(machine.trace().candidate(), Some(candidate()?));
    Ok(())
}

/// Positive: a trace can be built by hand, which is the direction M24 needs to
/// turn an observed run into something comparable.
#[test]
fn a_trace_can_be_built_from_observed_transitions() -> Fallible {
    let mut observed = Trace::new();
    assert!(observed.is_empty());
    observed.declare(candidate()?);
    let record = Transition::new(
        0,
        Tick::new(ARMED_AT),
        EventKind::Declare,
        State::Idle,
        State::CandidateDeclared,
        Some(Slot::B),
    );
    observed.push(record)?;
    assert_eq!(observed.len(), 1);
    assert_eq!(observed.last(), Some(record));
    assert_eq!(observed.records(), &[record]);
    assert_eq!(observed.render()?.lines().count(), 2);
    Ok(())
}

/// Positive: a transition round-trips through its rendered line, field by
/// field, so no field is dropped on the way to the bytes M24 compares.
#[test]
fn a_transition_round_trips_through_its_rendered_line() {
    let record = Transition::new(
        3,
        Tick::new(ARMED_AT),
        EventKind::SwapSlot,
        State::DeltaAcquired,
        State::SlotSwapped,
        Some(Slot::B),
    );
    let expected = TraceLine {
        schema: TRACE_SCHEMA.to_owned(),
        record: RecordKind::Transition,
        seq: 3,
        at: Tick::new(ARMED_AT),
        event: EventKind::SwapSlot,
        from: State::DeltaAcquired,
        to: State::SlotSwapped,
        slot: Some(Slot::B),
    };
    assert_eq!(record.to_line(), expected);
    assert_eq!(expected.to_transition(), record);
    assert_eq!(RecordKind::ALL.len(), 2);
}

// --- Negative -------------------------------------------------------------

/// Negative: every way a trace could be misread is a refusal, not a repair.
#[test]
fn a_trace_that_could_be_misread_is_refused() -> Fallible {
    assert_eq!(
        rejected(&tampered(
            "aegis.p02.ab-transition.v1\",\"record\":\"header",
            "aegis.p02.ab-transition.v2\",\"record\":\"header"
        )?)?,
        TraceError::Schema { line: 0 }
    );
    assert_eq!(
        rejected(&tampered(
            "\"record\":\"header\"",
            "\"record\":\"transition\""
        )?)?,
        TraceError::Kind {
            line: 0,
            found: RecordKind::Transition,
        }
    );
    assert_eq!(
        rejected(&tampered("\"fallback\":\"a\"", "\"fallback\":\"b\"")?)?,
        TraceError::Slots {
            target: Slot::B,
            fallback: Slot::B,
        }
    );
    assert_eq!(
        rejected(&tampered("\"transitions\":6", "\"transitions\":5")?)?,
        TraceError::Count {
            declared: 5,
            found: 6,
        }
    );
    let cut_short = truncated();
    assert_eq!(
        cut_short.lines().count(),
        GOLDEN.lines().count().saturating_sub(1),
        "the truncation drops exactly the last body line"
    );
    assert_eq!(
        rejected(&cut_short)?,
        TraceError::Count {
            declared: 6,
            found: 5,
        },
        "a trace cut short is refused, not read as the shorter history it looks like"
    );
    assert_eq!(
        rejected(&tampered("\"seq\":0", "\"seq\":1")?)?,
        TraceError::Sequence {
            expected: 0,
            found: 1,
        }
    );
    assert_eq!(rejected("")?, TraceError::Empty);
    Ok(())
}

/// Negative: an unknown field, a missing field and text that is not JSON at
/// all are all malformed, and the refusal names the line.
#[test]
fn a_malformed_line_is_refused_with_its_position() -> Fallible {
    assert_eq!(
        rejected(&tampered(
            "\"transitions\":6",
            "\"extra\":1,\"transitions\":6"
        )?)?,
        TraceError::Malformed { line: 0 }
    );
    assert_eq!(
        rejected(&tampered("\"version\":\"1.4.0\",", "")?)?,
        TraceError::Malformed { line: 0 }
    );
    assert_eq!(
        rejected(&tampered(
            "\"event\":\"declare\"",
            "\"event\":\"teleport\""
        )?)?,
        TraceError::Malformed { line: 1 }
    );
    assert_eq!(
        rejected("not json at all\n")?,
        TraceError::Malformed { line: 0 }
    );
    Ok(())
}

/// Negative: a header whose candidate cannot be read is refused, and the
/// refusal says which half of it failed.
#[test]
fn a_header_that_is_not_about_a_candidate_is_refused() -> Fallible {
    assert!(matches!(
        rejected(&tampered(
            "\"version\":\"1.4.0\"",
            "\"version\":\"1.4.0 beta\""
        )?)?,
        TraceError::Version(_)
    ));
    assert!(matches!(
        rejected(&tampered(SIGNED_HEX, "abc")?)?,
        TraceError::Verity(_)
    ));
    Ok(())
}

/// Negative: an empty trace has nothing to render, and the two rendering
/// refusals are distinguishable from each other.
#[test]
fn an_empty_trace_cannot_be_rendered() {
    let empty = Trace::new();
    assert_eq!(empty.render().err(), Some(TraceError::NoCandidate));
    assert_eq!(empty.header().err(), Some(TraceError::NoCandidate));
    assert_eq!(empty.last(), None);
    assert_ne!(TraceError::NoCandidate, TraceError::Serialisation);
    assert_ne!(TraceError::Serialisation.to_string(), String::new());
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a trace holding exactly `MAX_TRANSITIONS` records renders and
/// parses, and one record more is refused rather than truncated.
#[test]
fn a_trace_at_its_bound_round_trips_and_one_record_more_is_refused() -> Fallible {
    let (mut machine, clock) = armed()?;
    for _ in 0..MAX_TRANSITIONS.saturating_sub(machine.trace().len()) {
        machine.step(&clock, Event::Tick)?;
    }
    assert_eq!(machine.trace().len(), MAX_TRANSITIONS);
    let rendered = machine.trace().render()?;
    assert_eq!(rendered.lines().count(), MAX_TRANSITIONS.saturating_add(1));
    assert_eq!(&Trace::parse(&rendered)?, machine.trace());

    let mut full = *machine.trace();
    let overflow = Transition::new(
        u32::try_from(MAX_TRANSITIONS)?,
        Tick::new(ARMED_AT),
        EventKind::Tick,
        State::WatchdogArmed,
        State::WatchdogArmed,
        Some(Slot::B),
    );
    assert_eq!(
        full.push(overflow).err(),
        Some(TraceError::Full {
            bound: MAX_TRANSITIONS
        })
    );
    Ok(())
}

/// Boundary: a rendered trace is newline-terminated and holds exactly one
/// record per line, so line boundaries are record boundaries.
#[test]
fn every_record_occupies_exactly_one_newline_terminated_line() -> Fallible {
    let (machine, _clock) = blessed()?;
    let rendered = machine.trace().render()?;
    assert!(rendered.ends_with('\n'));
    assert_eq!(
        rendered.matches('\n').count(),
        machine.trace().len().saturating_add(1)
    );
    for line in rendered.lines() {
        assert!(line.starts_with('{'));
        assert!(line.ends_with('}'));
        assert!(line.contains(TRACE_SCHEMA));
    }
    Ok(())
}
