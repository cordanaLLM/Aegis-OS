// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The `scx_cake` four-tier EWMA burst classifier (REQ-P07-04).
//!
//! This is the arithmetic of `scx_cake.bpf.c` (export-056 `39af243568ad`)
//! lifted out of kernel space so it can be checked without loading anything.
//! Three things are carried across exactly, because changing any of them
//! would mean this module classifies differently from the program it models:
//!
//! * the running average is integer, `(old * 3 + new) / 4`, with the first
//!   sample seeding the average outright rather than being pulled three
//!   quarters of the way towards zero;
//! * the thresholds are 100 microseconds, 2 milliseconds and 8 milliseconds,
//!   expressed in nanoseconds;
//! * **every comparison is strict**. The source writes
//!   `if (ewma < BURST_CRITICAL_NS)`, so a burst of exactly 100000 nanoseconds
//!   is *not* critical; it is interactive. That is the behaviour of the
//!   program, and [`Tier::classify`] reproduces it rather than rounding it to
//!   the friendlier reading. The boundary tests pin all three edges.
//!
//! # What this module does not do
//!
//! **No eBPF program is compiled, loaded, attached or verified**, no dispatch
//! queue exists, no task is enqueued and no scheduler is installed. The host
//! kernel does run a `sched_ext` scheduler and does ship `scx_cake` as a
//! package, and neither fact is used here: milestone M19 is where an object
//! goes through the verifier. A [`DispatchQueue`] is a value, not a queue.
//!
//! **No timing figure is produced.** A burst duration is a number a caller
//! hands in; nothing here reads a clock, and the reference kernel is
//! `PREEMPT_DYNAMIC` rather than `PREEMPT_RT`, so the determinism the tiers
//! exist to deliver is not demonstrated by this milestone at all.

/// The upper edge of the critical tier, in nanoseconds (100 microseconds).
pub const BURST_CRITICAL_NS: u64 = 100_000;

/// The upper edge of the interactive tier, in nanoseconds (2 milliseconds).
pub const BURST_INTERACTIVE_NS: u64 = 2_000_000;

/// The upper edge of the frame tier, in nanoseconds (8 milliseconds).
pub const BURST_FRAME_NS: u64 = 8_000_000;

/// The weight the running average keeps.
pub const EWMA_WEIGHT_OLD: u64 = 3;

/// The weight one new burst carries.
pub const EWMA_WEIGHT_NEW: u64 = 1;

/// The divisor, `EWMA_WEIGHT_OLD + EWMA_WEIGHT_NEW`, a power of two.
pub const EWMA_DIVISOR: u64 = 4;

/// The four classification tiers, highest priority first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum Tier {
    /// Bursts under 100 microseconds: the compositor and the audio graph.
    Critical,
    /// Bursts under 2 milliseconds: active input and shell events.
    Interactive,
    /// Bursts under 8 milliseconds: frame rendering and games.
    Frame,
    /// Bursts of 8 milliseconds and above: background inference and builds.
    Bulk,
}

impl Tier {
    /// All four tiers, highest priority first.
    pub const ALL: [Self; 4] = [Self::Critical, Self::Interactive, Self::Frame, Self::Bulk];

    /// Returns the stable name this tier is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Critical => "critical",
            Self::Interactive => "interactive",
            Self::Frame => "frame",
            Self::Bulk => "bulk",
        }
    }

    /// Returns the tier index the source assigns.
    #[must_use]
    pub const fn index(self) -> u32 {
        match self {
            Self::Critical => 0,
            Self::Interactive => 1,
            Self::Frame => 2,
            Self::Bulk => 3,
        }
    }

    /// Returns the strict upper edge of this tier, when it has one.
    ///
    /// [`Tier::Bulk`] has none: it is where everything above the last
    /// threshold lands.
    #[must_use]
    pub const fn upper_edge_ns(self) -> Option<u64> {
        match self {
            Self::Critical => Some(BURST_CRITICAL_NS),
            Self::Interactive => Some(BURST_INTERACTIVE_NS),
            Self::Frame => Some(BURST_FRAME_NS),
            Self::Bulk => None,
        }
    }

    /// Returns the dispatch queue the source sends this tier to.
    #[must_use]
    pub const fn dispatch_queue(self) -> DispatchQueue {
        match self {
            Self::Critical => DispatchQueue::Critical,
            Self::Interactive => DispatchQueue::Interactive,
            Self::Frame => DispatchQueue::Frame,
            Self::Bulk => DispatchQueue::Bulk,
        }
    }

    /// Classifies one running average, exactly as the source does.
    ///
    /// Every comparison is strict, so a value equal to a threshold belongs to
    /// the tier above it. See the module documentation.
    #[must_use]
    pub const fn classify(ewma: EwmaBurst) -> Self {
        let value = ewma.nanos();
        if value < BURST_CRITICAL_NS {
            Self::Critical
        } else if value < BURST_INTERACTIVE_NS {
            Self::Interactive
        } else if value < BURST_FRAME_NS {
            Self::Frame
        } else {
            Self::Bulk
        }
    }
}

/// The priority dispatch queues the source declares.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum DispatchQueue {
    /// `DSQ_CRITICAL`, identifier 0.
    Critical,
    /// `DSQ_INTERACTIVE`, identifier 1.
    Interactive,
    /// `DSQ_FRAME`, identifier 2.
    Frame,
    /// `DSQ_BULK`, identifier 3.
    Bulk,
}

impl DispatchQueue {
    /// All four queues, in strict arbitration order.
    pub const ALL: [Self; 4] = [Self::Critical, Self::Interactive, Self::Frame, Self::Bulk];

    /// Returns the dispatch-queue identifier the source assigns.
    #[must_use]
    pub const fn id(self) -> u64 {
        match self {
            Self::Critical => 0,
            Self::Interactive => 1,
            Self::Frame => 2,
            Self::Bulk => 3,
        }
    }
}

/// The running average of one task's burst duration, in nanoseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default)]
pub struct EwmaBurst {
    nanos: u64,
    samples: u32,
}

impl EwmaBurst {
    /// Builds an average that has seen nothing.
    ///
    /// Its value is zero, which classifies as [`Tier::Critical`]. That is the
    /// source's own behaviour -- a task context is created with
    /// `ewma_burst_ns = 0` -- and the source guards it by seeding the average
    /// with the first real burst instead of folding it in.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            nanos: 0,
            samples: 0,
        }
    }

    /// Builds an average already holding `nanos`, as if seeded by one burst.
    #[must_use]
    pub const fn seeded(nanos: u64) -> Self {
        Self { nanos, samples: 1 }
    }

    /// Folds one burst duration into the average and returns the new value.
    ///
    /// The first burst seeds the average outright; every later one is
    /// `(old * 3 + new) / 4`.
    pub const fn observe(&mut self, burst_ns: u64) -> u64 {
        self.nanos = if self.samples == 0 {
            burst_ns
        } else {
            let kept = self.nanos.saturating_mul(EWMA_WEIGHT_OLD);
            let added = burst_ns.saturating_mul(EWMA_WEIGHT_NEW);
            kept.saturating_add(added).wrapping_div(EWMA_DIVISOR)
        };
        self.samples = self.samples.saturating_add(1);
        self.nanos
    }

    /// Returns the running average in nanoseconds.
    #[must_use]
    pub const fn nanos(self) -> u64 {
        self.nanos
    }

    /// Returns how many bursts have been folded in.
    #[must_use]
    pub const fn samples(self) -> u32 {
        self.samples
    }

    /// Returns the tier this average classifies as.
    #[must_use]
    pub const fn tier(self) -> Tier {
        Tier::classify(self)
    }
}
