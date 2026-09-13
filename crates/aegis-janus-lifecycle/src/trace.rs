// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The machine-readable transition trace: the artefact M24 diffs against.
//!
//! Milestone M24 boots a real A/B sysupdate transfer under QEMU and has to say
//! whether what happened is what this model says should happen. A narrative
//! comparison cannot answer that, so the trace is designed to be compared as
//! bytes:
//!
//! * **ordered.** Records carry a dense ascending [`Transition::seq`] starting
//!   at zero, in the order the transitions were made. A rendered trace is one
//!   JSON object per line, header first, in the same order.
//! * **self-describing.** Every line carries the [`TRACE_SCHEMA`] tag and a
//!   [`RecordKind`], so a reader never has to infer what it is holding, and a
//!   later schema is rejected rather than misread. The header states the
//!   release version, both slots and the signed root hash, so a trace says what
//!   it is about and not only what happened.
//! * **stable.** Every value is an integer or one of this crate's own stable
//!   names; there is no floating point, no map ordering, no host timestamp and
//!   no free text. [`Version`] refuses the bytes that JSON would have to
//!   escape, so one value has exactly one spelling.
//! * **complete.** Every accepted step is recorded, including a tick that
//!   changed no state, and a refused step records nothing. A trace is therefore
//!   the whole accepted history, and the header's transition count makes a
//!   truncated trace a parse failure rather than a shorter history.
//!
//! Those four properties are what make `expected.render()? == observed_text`
//! a sound test rather than a coincidence. [`Trace::parse`] is the other half:
//! rendering is a fixed point, so a trace read back and rendered again is the
//! same bytes, and M24 may compare in whichever direction it has the input for.
//!
//! # Scope
//!
//! Rendering and parsing allocate; they are not the lifecycle path. The trace
//! the machine accumulates is a fixed-size array of `Copy` records, bounded by
//! [`MAX_TRANSITIONS`], so making a transition allocates nothing.

use crate::candidate::{Candidate, Version};
use crate::clock::Tick;
use crate::slot::Slot;
use crate::state::{EventKind, State};
use crate::verity::{ROOT_HASH_HEX_LEN, RootHash, VerityError};

/// The schema tag every rendered line carries.
///
/// It changes when the rendered shape changes, so a consumer pinned to one
/// version refuses a trace it would otherwise misread.
pub const TRACE_SCHEMA: &str = "aegis.p02.ab-transition.v1";

/// Scalar upper bound on the transitions one trace may hold.
pub const MAX_TRANSITIONS: usize = 32;

/// Which of the two line shapes a rendered line is.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum RecordKind {
    /// The first line: what the trace is about.
    Header,
    /// Every further line: one transition.
    Transition,
}

impl RecordKind {
    /// Both kinds, in declaration order.
    pub const ALL: [Self; 2] = [Self::Header, Self::Transition];

    /// Returns the stable name this kind is rendered under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Header => "header",
            Self::Transition => "transition",
        }
    }
}

/// Why a trace could not be rendered, parsed or extended.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[non_exhaustive]
pub enum TraceError {
    /// The text held no header line at all.
    #[error("a trace needs a header line")]
    Empty,
    /// The trace is full at its scalar bound.
    #[error("the trace is full at its bound of {bound} records")]
    Full {
        /// The bound the trace holds.
        bound: usize,
    },
    /// A line was not one JSON object of the shape its position requires.
    #[error("line {line} is not one well-formed trace record")]
    Malformed {
        /// The zero-based line number, counting the header as line 0.
        line: usize,
    },
    /// A line did not carry this crate's schema tag.
    #[error("line {line} does not carry the schema tag {TRACE_SCHEMA}")]
    Schema {
        /// The zero-based line number.
        line: usize,
    },
    /// A line carried the wrong record kind for its position.
    #[error("line {line} is a {} record, which does not belong there", .found.name())]
    Kind {
        /// The zero-based line number.
        line: usize,
        /// The kind the line declared.
        found: RecordKind,
    },
    /// Sequence numbers are not the dense ascending run a trace renders.
    #[error("sequence {found} appears where {expected} was required")]
    Sequence {
        /// The sequence number the position requires.
        expected: u32,
        /// The sequence number the line declared.
        found: u32,
    },
    /// The header's transition count disagrees with the lines that followed.
    #[error("the header declares {declared} transitions and {found} followed")]
    Count {
        /// The count the header declared.
        declared: usize,
        /// The count actually read.
        found: usize,
    },
    /// The header named two slots that are not each other's alternate.
    #[error("target slot {target} and fallback slot {fallback} are not an A/B pair")]
    Slots {
        /// The declared target slot.
        target: Slot,
        /// The declared fallback slot.
        fallback: Slot,
    },
    /// The header's release version is not a version.
    #[error("header version: {0}")]
    Version(#[from] crate::candidate::VersionError),
    /// The header's root hash is not a root hash.
    #[error("header root hash: {0}")]
    Verity(#[from] VerityError),
    /// The trace holds no candidate, so there is no header to render.
    #[error("the trace holds no candidate and cannot be rendered")]
    NoCandidate,
    /// A record could not be serialised.
    #[error("a trace record could not be serialised")]
    Serialisation,
}

/// One recorded transition: fixed width, `Copy`, no heap.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Transition {
    seq: u32,
    at: Tick,
    event: EventKind,
    from: State,
    to: State,
    slot: Option<Slot>,
}

/// The record an unused array position holds. Never rendered.
const BLANK: Transition = Transition {
    seq: 0,
    at: Tick::ORIGIN,
    event: EventKind::Tick,
    from: State::Idle,
    to: State::Idle,
    slot: None,
};

impl Transition {
    /// Records one transition.
    ///
    /// `slot` is the candidate's target slot once one is declared, so a reader
    /// of a single line knows which slot the transition concerned.
    #[must_use]
    pub const fn new(
        seq: u32,
        at: Tick,
        event: EventKind,
        from: State,
        to: State,
        slot: Option<Slot>,
    ) -> Self {
        Self {
            seq,
            at,
            event,
            from,
            to,
            slot,
        }
    }

    /// Returns the position of this transition in the trace, from zero.
    #[must_use]
    pub const fn seq(self) -> u32 {
        self.seq
    }

    /// Returns the tick the transition was made at.
    #[must_use]
    pub const fn at(self) -> Tick {
        self.at
    }

    /// Returns the event that caused the transition.
    #[must_use]
    pub const fn event(self) -> EventKind {
        self.event
    }

    /// Returns the state the machine left.
    #[must_use]
    pub const fn from(self) -> State {
        self.from
    }

    /// Returns the state the machine entered.
    #[must_use]
    pub const fn to(self) -> State {
        self.to
    }

    /// Returns the slot the transition concerned, if a candidate was declared.
    #[must_use]
    pub const fn slot(self) -> Option<Slot> {
        self.slot
    }

    /// Returns `true` when the transition left the state unchanged.
    ///
    /// A tick inside the watchdog window is such a record: nothing moved, and
    /// the trace says so rather than staying silent.
    #[must_use]
    pub fn is_stationary(self) -> bool {
        self.from == self.to
    }

    /// Projects the transition onto its rendered form.
    ///
    /// The rendered form owns its schema tag, so this allocates. It is on the
    /// rendering path, never on the lifecycle path.
    #[must_use]
    pub fn to_line(self) -> TraceLine {
        TraceLine {
            schema: TRACE_SCHEMA.to_owned(),
            record: RecordKind::Transition,
            seq: self.seq,
            at: self.at,
            event: self.event,
            from: self.from,
            to: self.to,
            slot: self.slot,
        }
    }
}

/// The rendered first line of a trace.
///
/// Field order is the rendered order. Changing it changes the bytes, which is
/// why [`TRACE_SCHEMA`] is versioned.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TraceHeader {
    /// The schema tag, always [`TRACE_SCHEMA`].
    pub schema: String,
    /// The record kind, always [`RecordKind::Header`].
    pub record: RecordKind,
    /// The candidate release version.
    pub version: String,
    /// The slot the release is written into.
    pub target: Slot,
    /// The slot that stays bootable while the candidate is on trial.
    pub fallback: Slot,
    /// The dm-verity root hash the signed release declares, lower-case hex.
    pub expected_root_hash: String,
    /// How many transition lines follow.
    pub transitions: usize,
}

/// The rendered form of one transition.
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "kebab-case", deny_unknown_fields)]
pub struct TraceLine {
    /// The schema tag, always [`TRACE_SCHEMA`].
    pub schema: String,
    /// The record kind, always [`RecordKind::Transition`].
    pub record: RecordKind,
    /// The position of this transition in the trace, from zero.
    pub seq: u32,
    /// The tick the transition was made at.
    pub at: Tick,
    /// The event that caused the transition.
    pub event: EventKind,
    /// The state the machine left.
    pub from: State,
    /// The state the machine entered.
    pub to: State,
    /// The slot the transition concerned, if a candidate was declared.
    pub slot: Option<Slot>,
}

impl TraceLine {
    /// Reads the line back as a transition.
    #[must_use]
    pub const fn to_transition(&self) -> Transition {
        Transition {
            seq: self.seq,
            at: self.at,
            event: self.event,
            from: self.from,
            to: self.to,
            slot: self.slot,
        }
    }
}

/// An ordered, bounded record of every transition one machine made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Trace {
    candidate: Option<Candidate>,
    records: [Transition; MAX_TRANSITIONS],
    len: u16,
}

impl Default for Trace {
    fn default() -> Self {
        Self::new()
    }
}

/// Serialises one record and appends it to `out` as a complete line.
fn append<T: serde::Serialize>(out: &mut String, value: &T) -> Result<(), TraceError> {
    let encoded = serde_json::to_string(value).map_err(|_| TraceError::Serialisation)?;
    out.push_str(&encoded);
    out.push('\n');
    Ok(())
}

impl Trace {
    /// Builds an empty trace.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            candidate: None,
            records: [BLANK; MAX_TRANSITIONS],
            len: 0,
        }
    }

    /// Records the candidate this trace is about.
    pub const fn declare(&mut self, candidate: Candidate) {
        self.candidate = Some(candidate);
    }

    /// Returns the candidate this trace is about, once one is declared.
    #[must_use]
    pub const fn candidate(&self) -> Option<Candidate> {
        self.candidate
    }

    /// Appends one transition.
    ///
    /// # Errors
    ///
    /// Returns [`TraceError::Full`] at [`MAX_TRANSITIONS`] records, and
    /// [`TraceError::Sequence`] when `record` does not carry the next sequence
    /// number. The dense ascending run is an invariant of the rendered form,
    /// so it is enforced where records enter rather than where they leave.
    pub fn push(&mut self, record: Transition) -> Result<(), TraceError> {
        let next = u32::try_from(self.len()).map_err(|_| TraceError::Full {
            bound: MAX_TRANSITIONS,
        })?;
        if record.seq() != next {
            return Err(TraceError::Sequence {
                expected: next,
                found: record.seq(),
            });
        }
        let at = self.len();
        let slot = self.records.get_mut(at).ok_or(TraceError::Full {
            bound: MAX_TRANSITIONS,
        })?;
        *slot = record;
        self.len = self.len.saturating_add(1);
        Ok(())
    }

    /// Returns the recorded transitions, in order.
    #[must_use]
    pub fn records(&self) -> &[Transition] {
        self.records.get(..self.len()).unwrap_or(&[])
    }

    /// Returns how many transitions are recorded.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len as usize
    }

    /// Returns `true` when nothing has been recorded.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    /// Returns the most recent transition, if there is one.
    #[must_use]
    pub fn last(&self) -> Option<Transition> {
        self.records().last().copied()
    }

    /// Builds the header line for this trace.
    ///
    /// # Errors
    ///
    /// Returns [`TraceError::NoCandidate`] when no candidate is declared, and
    /// [`TraceError::Version`] or [`TraceError::Verity`] when the candidate
    /// cannot be rendered.
    pub fn header(&self) -> Result<TraceHeader, TraceError> {
        let candidate = self.candidate.ok_or(TraceError::NoCandidate)?;
        let mut buffer = [0u8; ROOT_HASH_HEX_LEN];
        let hash = candidate.expected_root_hash();
        let expected_root_hash = hash.encode_hex(&mut buffer)?.to_owned();
        Ok(TraceHeader {
            schema: TRACE_SCHEMA.to_owned(),
            record: RecordKind::Header,
            version: candidate.version().as_str()?.to_owned(),
            target: candidate.target(),
            fallback: candidate.fallback(),
            expected_root_hash,
            transitions: self.len(),
        })
    }

    /// Renders the trace as JSON Lines, header first, newline-terminated.
    ///
    /// This allocates and is not the lifecycle path.
    ///
    /// # Errors
    ///
    /// Returns [`TraceError`] when the header cannot be built or a record
    /// cannot be serialised.
    pub fn render(&self) -> Result<String, TraceError> {
        let header = self.header()?;
        let mut out = String::new();
        append(&mut out, &header)?;
        for record in self.records().iter().take(MAX_TRANSITIONS) {
            append(&mut out, &record.to_line())?;
        }
        Ok(out)
    }

    /// Parses a rendered trace back.
    ///
    /// Rendering is a fixed point of this function: a trace parsed and
    /// rendered again is the same bytes, which is what lets M24 compare in
    /// either direction.
    ///
    /// # Errors
    ///
    /// Returns [`TraceError`] for an absent, malformed, mis-tagged,
    /// out-of-order or miscounted line. Nothing is repaired.
    pub fn parse(text: &str) -> Result<Self, TraceError> {
        let mut lines = text.lines();
        let first = lines.next().ok_or(TraceError::Empty)?;
        let header = read_header(first)?;
        let mut trace = Self::new();
        trace.declare(candidate_of(&header)?);
        let bound = MAX_TRANSITIONS.saturating_add(1);
        for (index, raw) in lines.enumerate().take(bound) {
            trace.push(read_line(index, raw)?)?;
        }
        if header.transitions != trace.len() {
            return Err(TraceError::Count {
                declared: header.transitions,
                found: trace.len(),
            });
        }
        Ok(trace)
    }
}

/// Reads and validates the header line.
fn read_header(raw: &str) -> Result<TraceHeader, TraceError> {
    let header: TraceHeader =
        serde_json::from_str(raw).map_err(|_| TraceError::Malformed { line: 0 })?;
    if header.schema != TRACE_SCHEMA {
        return Err(TraceError::Schema { line: 0 });
    }
    if header.record != RecordKind::Header {
        return Err(TraceError::Kind {
            line: 0,
            found: header.record,
        });
    }
    if header.fallback != header.target.other() {
        return Err(TraceError::Slots {
            target: header.target,
            fallback: header.fallback,
        });
    }
    Ok(header)
}

/// Reads the candidate a validated header describes.
fn candidate_of(header: &TraceHeader) -> Result<Candidate, TraceError> {
    let version = Version::parse(&header.version)?;
    let hash = RootHash::parse_hex(&header.expected_root_hash)?;
    Ok(Candidate::new(version, header.target, hash))
}

/// Reads one transition line; `index` is its position after the header.
fn read_line(index: usize, raw: &str) -> Result<Transition, TraceError> {
    let line = index.saturating_add(1);
    let parsed: TraceLine =
        serde_json::from_str(raw).map_err(|_| TraceError::Malformed { line })?;
    if parsed.schema != TRACE_SCHEMA {
        return Err(TraceError::Schema { line });
    }
    if parsed.record != RecordKind::Transition {
        return Err(TraceError::Kind {
            line,
            found: parsed.record,
        });
    }
    Ok(parsed.to_transition())
}
