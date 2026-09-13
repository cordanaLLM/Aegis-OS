// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Decision D29, recorded and left **open**: which way the P08/P11 capture
//! edge runs.
//!
//! The graph of record draws `P08_Calliope -> P11_Ludus` for
//! `REMOTE_PLAY_CAPTURE`, a sub-5 ms encoded video stream (export-062
//! `1ce919ed54bb`). The architecture document draws the same buffer-sharing
//! capture stream in the opposite direction, `P11 -> P08` (export-002
//! `7e0c95f4ea05`). The M01 register carries the collision as dispute DSP-11
//! and poses it as D29.
//!
//! **Milestone M07 does not settle it.** What decides the direction is which
//! subsystem opens the stream and owns the encode budget, and a budget is a
//! timing claim: the reference profile runs `PREEMPT_DYNAMIC`, so this
//! milestone produces no timing figure and has nothing to settle it with. Both
//! readings stay representable in [`CaptureDirection`], and the P08 payload
//! this crate does type -- the DMA-BUF stream descriptor to P04 -- is a
//! different edge that both sources agree on.

/// Whether a recorded decision is settled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DecisionState {
    /// The decision is settled and the register names what was chosen.
    Closed,
    /// The decision is recorded but not settled.
    Unresolved,
}

impl DecisionState {
    /// Returns the stable name this state is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::Unresolved => "unresolved",
        }
    }
}

/// The two directions the capture edge could run.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum CaptureDirection {
    /// The graph of record: P08 Calliope produces the stream for P11 Ludus.
    CalliopeProduces,
    /// The architecture document: P11 Ludus opens the stream from P08.
    LudusProduces,
}

impl CaptureDirection {
    /// Both readings, in the order the register lists them.
    pub const ALL: [Self; 2] = [Self::CalliopeProduces, Self::LudusProduces];

    /// Returns the stable name this reading is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::CalliopeProduces => "p08-produces-for-p11",
            Self::LudusProduces => "p11-opens-from-p08",
        }
    }

    /// Returns `true` when P08 owns the sub-5 ms encode budget in this reading.
    ///
    /// Which reading is right decides who owns a timing budget, and this
    /// milestone measures no time at all -- which is why D29 stays open.
    #[must_use]
    pub const fn calliope_owns_encode_budget(self) -> bool {
        matches!(self, Self::CalliopeProduces)
    }
}

/// A source, cited by export identifier and digest prefix only.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Citation {
    /// The export identifier.
    pub export: &'static str,
    /// The first twelve hexadecimal characters of that source's sha256.
    pub sha256_prefix: &'static str,
}

/// One recorded decision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DecisionRecord {
    /// The decision identifier used in `docs/roadmap/README.md`.
    pub id: &'static str,
    /// The question the decision answers.
    pub question: &'static str,
    /// Whether the decision is settled.
    pub state: DecisionState,
    /// The readings that remain open.
    pub open_readings: [CaptureDirection; 2],
    /// Why this milestone does not settle it.
    pub rationale: &'static str,
    /// The milestone that could settle it.
    pub settled_at: &'static str,
    /// The requirements the decision touches.
    pub touches: [&'static str; 2],
    /// The dispute this decision carries.
    pub dispute: &'static str,
    /// The sources that posed the question.
    pub sources: [Citation; 2],
}

/// D29: which direction the P08/P11 remote-play capture edge runs.
pub const D29_REMOTE_PLAY_CAPTURE: DecisionRecord = DecisionRecord {
    id: "D29",
    question: "Which subsystem opens the capture stream between P08 and P11, and owns its \
               sub-5 millisecond encode budget?",
    state: DecisionState::Unresolved,
    open_readings: CaptureDirection::ALL,
    rationale: "the direction decides who owns an encode budget, and a budget is a timing \
                claim; the reference profile runs PREEMPT_DYNAMIC rather than PREEMPT_RT, so \
                milestone M07 produces no timing figure and has nothing to settle it with. \
                The P11 slice is milestone M08 and the GPU path is M12",
    settled_at: "M12",
    touches: ["REQ-P08-01", "REQ-P08-04"],
    dispute: "DSP-11",
    sources: [
        Citation {
            export: "export-062",
            sha256_prefix: "1ce919ed54bb",
        },
        Citation {
            export: "export-002",
            sha256_prefix: "7e0c95f4ea05",
        },
    ],
};
