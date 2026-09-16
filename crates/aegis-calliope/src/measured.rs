// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Timing figures that were observed, and the kernel each one was observed on
//! (milestone M23, REQ-P07-01, REQ-P08-08).
//!
//! # Why this module exists beside [`crate::latency`]
//!
//! [`crate::latency`] holds [`Declared<T>`](crate::latency::Declared): a figure
//! recorded from a source that nothing here measured. Its `Display` says
//! `5 (declared, unmeasured)` and `tests/declared_literals.rs` stops compiling
//! if one becomes a bare integer. That wrapper answers one question -- *was
//! this observed?* -- and milestone M07 could answer it with a flat "no".
//!
//! M23 measures, so a second question appears that M07 never had to face:
//! **which machine produced the number?** A worst-case wakeup latency read on a
//! `PREEMPT_DYNAMIC` kernel and one read on a `PREEMPT_RT` kernel are the same
//! integer and are not the same claim. On the reference profile they are also
//! not reliably ordered: across five runs of `make verify-latency` recorded on
//! 2026-09-13 the non-realtime host's worst case was below the realtime guest's
//! four times and above it once. A wrapper that said only "measured" would let
//! either number stand in for the other.
//!
//! So provenance here is not a flag; it is the kernel. [`Measured<T>`] carries
//! a [`KernelIdentity`] -- a release string and a [`PreemptionModel`] -- and a
//! [`MeasurementTool`], and it cannot be built without all three. There is no
//! `From<Declared<T>>` for [`Measured<T>`] and no `From<Measured<T>>` for
//! [`Declared<T>`] in either direction, so no conversion can turn a target into
//! an observation or an observation into a target.
//!
//! This mirrors the `Provenance` label in M05's wattage seam
//! (`crates/aegis-tellus/src/power.rs`), where a figure carries how it was
//! obtained and the rule deciding the label is written down and tested. The
//! rule here is [`DeterminismVerdict::of`], and
//! the part worth reading twice is its order: **the kernel is checked before
//! the figure**. A number from a kernel that is not `PREEMPT_RT` is
//! [`DeterminismVerdict::KernelNotRealtime`] whatever its value, exactly as
//! M05 labels a `psys` figure `Modelled` whatever figure stands in.
//!
//! # Two layers, so a boundary case cannot fake a measurement
//!
//! [`TierAttainment::evaluate`] is arithmetic on two integers and carries no
//! provenance at all. That is deliberate: the boundary cases -- a worst case
//! one nanosecond below a threshold, exactly on it, and one nanosecond past it
//! -- exercise the comparison rule, and building a [`Measured`] for them would
//! mean minting three figures nobody observed. The provenance layer sits above
//! it in [`DeterminismVerdict::of`], and only [`Measured::verdict`] joins the
//! two.
//!
//! # What this module does not do
//!
//! **It measures nothing.** It reads no clock, opens no file, spawns no process
//! and schedules no thread; `tests/stubbed_effects.rs` is the sweep that keeps
//! it that way. The figures below were produced by `make verify-latency`
//! (`tools/verify_latency_fixture.py`) running `cyclictest` inside a guest and
//! on the reference host, and are recorded here as dated readings of those two
//! runs. Re-running the gate re-measures; it does not assert that a new run
//! reproduces these integers, because a worst case is not reproducible to the
//! nanosecond.

use core::fmt;

use crate::latency::Declared;

/// Scalar upper bound on a recorded kernel release string, in bytes.
///
/// Read out of the pinned source rather than chosen here:
/// `include/uapi/linux/utsname.h` of `linux-7.2.5` declares
/// `#define __NEW_UTS_LEN 64` and `char release[__NEW_UTS_LEN + 1]`, so the
/// field holds 64 bytes of text plus a terminator and the string itself is
/// bounded at 64. [`KernelIdentity::release_within_bound`] is the check, and
/// `tests/measured_figures.rs` runs it over every recorded identity.
pub const MAX_KERNEL_RELEASE_LEN: usize = 64;

/// How many nanoseconds one millisecond holds.
pub const NANOS_PER_MILLI: u64 = 1_000_000;

/// The preemption model of the kernel a figure was observed on.
///
/// The variants are ordered by how much a determinism claim may rest on them,
/// the same ordering discipline M05 gives `Provenance`: [`Self::PreemptRt`] is
/// the only one [`DeterminismVerdict::of`] will look past, and it is the only
/// one that can reach [`DeterminismVerdict::Satisfied`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum PreemptionModel {
    /// `CONFIG_PREEMPT_RT` is not set in the running kernel's configuration.
    NotPreemptRt,
    /// `CONFIG_PREEMPT_RT=y` in the running kernel's own configuration.
    PreemptRt,
}

impl PreemptionModel {
    /// Every model, weakest claim first.
    pub const ALL: [Self; 2] = [Self::NotPreemptRt, Self::PreemptRt];

    /// Returns the stable name used in diagnostics and recorded evidence.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NotPreemptRt => "not PREEMPT_RT",
            Self::PreemptRt => "PREEMPT_RT",
        }
    }

    /// Returns the `/proc/config.gz` line the probe must read for this model.
    ///
    /// Both spellings are what `CONFIG_PREEMPT_RT` looks like in a kernel's own
    /// configuration, and `tools/guest/aegis-preempt-probe.sh` prints one of
    /// them verbatim from whichever kernel is running it. Recording the line
    /// rather than a boolean is what lets the guest reading and the host
    /// reading be compared as text.
    #[must_use]
    pub const fn config_line(self) -> &'static str {
        match self {
            Self::NotPreemptRt => "# CONFIG_PREEMPT_RT is not set",
            Self::PreemptRt => "CONFIG_PREEMPT_RT=y",
        }
    }

    /// Returns `true` when a determinism claim may rest on this model.
    #[must_use]
    pub const fn is_realtime(self) -> bool {
        matches!(self, Self::PreemptRt)
    }
}

impl fmt::Display for PreemptionModel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// The kernel one figure was observed on: its release, and its preemption model.
///
/// Both halves are read back from inside the machine that produced the figure
/// -- the release from `uname -r`, the model from that kernel's own
/// `/proc/config.gz` -- never from the build tree and never from the machine
/// running the gate.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KernelIdentity {
    release: &'static str,
    preemption: PreemptionModel,
}

impl KernelIdentity {
    /// Records the kernel `release` ran under `preemption`.
    #[must_use]
    pub const fn new(release: &'static str, preemption: PreemptionModel) -> Self {
        Self {
            release,
            preemption,
        }
    }

    /// Returns the release string, exactly as `uname -r` printed it.
    #[must_use]
    pub const fn release(self) -> &'static str {
        self.release
    }

    /// Returns the preemption model read from that kernel's configuration.
    #[must_use]
    pub const fn preemption(self) -> PreemptionModel {
        self.preemption
    }

    /// Returns `true` when a determinism claim may rest on this kernel.
    #[must_use]
    pub const fn is_realtime(self) -> bool {
        self.preemption.is_realtime()
    }

    /// Returns `true` when the release fits [`MAX_KERNEL_RELEASE_LEN`].
    ///
    /// The bound is checked rather than assumed: a release longer than the
    /// kernel's own UTS field could not have come from `uname -r`.
    #[must_use]
    pub const fn release_within_bound(self) -> bool {
        !self.release.is_empty() && self.release.len() <= MAX_KERNEL_RELEASE_LEN
    }
}

impl fmt::Display for KernelIdentity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}, {}", self.release, self.preemption)
    }
}

/// The tool that produced a figure, and what that tool actually measures.
///
/// The methodology travels with the figure so a reader cannot take a wakeup
/// latency for a round-trip latency or for a scheduler burst duration. Only an
/// admitted tool has a variant here: `docs/roadmap/toolchain-admission.md`
/// carries the pin, and nothing in this repository writes its own timing loop.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MeasurementTool {
    /// `cyclictest` from `rt-tests`, the standard realtime latency tool.
    Cyclictest,
}

impl MeasurementTool {
    /// Every admitted tool.
    pub const ALL: [Self; 1] = [Self::Cyclictest];

    /// Returns the program name, as it is invoked.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Cyclictest => "cyclictest",
        }
    }

    /// Returns the admitted version, as the tool prints it for itself.
    #[must_use]
    pub const fn admitted_version(self) -> &'static str {
        match self {
            Self::Cyclictest => "2.10",
        }
    }

    /// Returns what the tool measures, in one disputable sentence.
    #[must_use]
    pub const fn measures(self) -> &'static str {
        match self {
            Self::Cyclictest => {
                "the difference between the wakeup a periodic clock_nanosleep was \
                 programmed for and the wakeup a SCHED_FIFO thread actually got"
            }
        }
    }
}

impl fmt::Display for MeasurementTool {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// A figure that was observed, with the kernel and the tool that produced it.
///
/// The counterpart of [`Declared<T>`]. There is deliberately no conversion
/// between the two in either direction: a target cannot become an observation
/// by being wrapped differently, and an observation cannot lose the kernel it
/// came from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Measured<T> {
    value: T,
    kernel: KernelIdentity,
    tool: MeasurementTool,
}

impl<T> Measured<T> {
    /// Records `value` as observed on `kernel` by `tool`.
    #[must_use]
    pub const fn new(value: T, kernel: KernelIdentity, tool: MeasurementTool) -> Self {
        Self {
            value,
            kernel,
            tool,
        }
    }

    /// Returns the observed value.
    #[must_use]
    pub const fn get(&self) -> &T {
        &self.value
    }

    /// Returns the kernel the value was observed on.
    #[must_use]
    pub const fn kernel(&self) -> KernelIdentity {
        self.kernel
    }

    /// Returns the tool that observed it.
    #[must_use]
    pub const fn tool(&self) -> MeasurementTool {
        self.tool
    }
}

impl<T: Copy> Measured<T> {
    /// Returns a copy of the observed value.
    #[must_use]
    pub const fn copied(self) -> T {
        self.value
    }
}

impl<T: fmt::Display> fmt::Display for Measured<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} (measured by {} on {})",
            self.value, self.tool, self.kernel
        )
    }
}

/// Where one worst case sits against one threshold.
///
/// The comparison is **strict**, because `Tier::classify` in `aegis-lictor`
/// (`crates/aegis-lictor/src/tier.rs`) is strict: the `scx_cake` source writes
/// `if (ewma < BURST_CRITICAL_NS)`, so
/// a burst of exactly 100000 nanoseconds is not critical. A fixture that
/// rounded the edge the friendlier way would report a tier the classifier
/// would not agree with, which is why [`Self::AtEdge`] exists as its own
/// outcome instead of being folded into either neighbour.
///
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
#[non_exhaustive]
pub enum TierAttainment {
    /// The worst case is above the threshold.
    Exceeded,
    /// The worst case is exactly the threshold, which the strict rule refuses.
    AtEdge,
    /// The worst case is strictly below the threshold.
    Attained,
}

impl TierAttainment {
    /// Every outcome, weakest first.
    pub const ALL: [Self; 3] = [Self::Exceeded, Self::AtEdge, Self::Attained];

    /// Returns the stable name used in diagnostics and recorded evidence.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Exceeded => "exceeded",
            Self::AtEdge => "at-edge",
            Self::Attained => "attained",
        }
    }

    /// Places `worst_ns` against `threshold_ns` under the strict rule.
    ///
    /// This is arithmetic on two integers and carries no provenance: a
    /// boundary case may exercise it without minting a figure nobody observed.
    #[must_use]
    pub const fn evaluate(threshold_ns: u64, worst_ns: u64) -> Self {
        if worst_ns < threshold_ns {
            Self::Attained
        } else if worst_ns == threshold_ns {
            Self::AtEdge
        } else {
            Self::Exceeded
        }
    }
}

impl fmt::Display for TierAttainment {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

/// Whether one measured worst case satisfies a determinism claim at one
/// threshold.
///
/// [`Self::KernelNotRealtime`] is not an ordering on numbers and is not
/// "a larger figure". It is what every figure from a kernel without
/// `CONFIG_PREEMPT_RT` becomes, including one smaller than every figure the
/// realtime guest produced -- which is the case the reference profile actually
/// presents.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum DeterminismVerdict {
    /// The kernel that produced the figure is not `PREEMPT_RT`, so no figure
    /// from it satisfies the claim whatever its value.
    KernelNotRealtime,
    /// A realtime kernel whose worst case is above the threshold.
    WorstCaseExceeds,
    /// A realtime kernel whose worst case is exactly the threshold, refused by
    /// the same strict rule the classifier uses.
    WorstCaseAtEdge,
    /// A realtime kernel whose worst case is strictly below the threshold.
    Satisfied,
}

impl DeterminismVerdict {
    /// Every verdict.
    pub const ALL: [Self; 4] = [
        Self::KernelNotRealtime,
        Self::WorstCaseExceeds,
        Self::WorstCaseAtEdge,
        Self::Satisfied,
    ];

    /// Returns the stable name used in diagnostics and recorded evidence.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::KernelNotRealtime => "kernel-not-realtime",
            Self::WorstCaseExceeds => "worst-case-exceeds",
            Self::WorstCaseAtEdge => "worst-case-at-edge",
            Self::Satisfied => "satisfied",
        }
    }

    /// Returns `true` only for [`Self::Satisfied`].
    #[must_use]
    pub const fn satisfies(self) -> bool {
        matches!(self, Self::Satisfied)
    }

    /// Decides a verdict, **kernel first**.
    ///
    /// The order is the rule: a figure from a kernel that is not `PREEMPT_RT`
    /// never reaches the numeric comparison, so it cannot be reported as a
    /// smaller or larger number than a realtime one. Only after the kernel is
    /// admitted does `attainment` decide between the three remaining outcomes.
    #[must_use]
    pub const fn of(kernel: KernelIdentity, attainment: TierAttainment) -> Self {
        if !kernel.is_realtime() {
            return Self::KernelNotRealtime;
        }
        match attainment {
            TierAttainment::Attained => Self::Satisfied,
            TierAttainment::AtEdge => Self::WorstCaseAtEdge,
            TierAttainment::Exceeded => Self::WorstCaseExceeds,
        }
    }
}

impl fmt::Display for DeterminismVerdict {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

impl Measured<u64> {
    /// Returns where this figure sits against `threshold_ns`, ignoring the
    /// kernel.
    #[must_use]
    pub const fn attainment(&self, threshold_ns: u64) -> TierAttainment {
        TierAttainment::evaluate(threshold_ns, self.value)
    }

    /// Returns whether this figure satisfies a determinism claim at
    /// `threshold_ns`.
    #[must_use]
    pub const fn verdict(&self, threshold_ns: u64) -> DeterminismVerdict {
        DeterminismVerdict::of(self.kernel, self.attainment(threshold_ns))
    }
}

/// The kernel this repository builds in M26, as the guest reported itself.
///
/// `uname -r` printed `7.2.5-aegis-m26` and the kernel's own
/// `/proc/config.gz` carried `CONFIG_PREEMPT_RT=y`, both read from inside the
/// guest on 2026-09-13. It is an interim source for this fixture only (D70):
/// the kernel the product ships is built by Nucleus against the M18 schema
/// once Nucleus is real.
pub const AEGIS_M26_GUEST: KernelIdentity =
    KernelIdentity::new("7.2.5-aegis-m26", PreemptionModel::PreemptRt);

/// The reference development host, as the same probe reported it.
///
/// `uname -r` printed `7.2.5-1-cachyos` and the same probe script run against
/// the host's own `/proc/config.gz` printed
/// `# CONFIG_PREEMPT_RT is not set`, on 2026-09-13. This is a dated reading of
/// one machine; `make verify-latency` re-reads both halves on every run and
/// refuses to proceed if the running host is no longer this one.
pub const REFERENCE_HOST: KernelIdentity =
    KernelIdentity::new("7.2.5-1-cachyos", PreemptionModel::NotPreemptRt);

/// The P08 round-trip-latency target, in nanoseconds.
///
/// The same declared figure as
/// [`TARGET_RTL_LATENCY_MS`](crate::latency::TARGET_RTL_LATENCY_MS), in the
/// unit the P07 tier edges use. It stays [`Declared`] because it is still a
/// target out of export-017: M23 measures what a kernel delivers, not what the
/// report asks for, and a threshold is not made into an observation by having
/// an observation compared against it.
pub const TARGET_RTL_LATENCY_NS: Declared<u64> = Declared::new(5_000_000);

/// The worst-case wakeup latency observed on the realtime guest, in nanoseconds.
///
/// One `cyclictest` run of 50000 cycles at a 200 microsecond interval, one
/// `SCHED_FIFO` thread at priority 95, inside a guest booting the kernel M26
/// builds. Alongside it that run recorded a minimum of 1510 nanoseconds and a
/// mean of 9664.
///
/// What the figure is: the largest lateness that run saw. What it is not: a
/// bound. Ten seconds of samples on a machine that was also carrying a
/// development session is a reading, not a worst case over all time, and this
/// constant would be a different integer on the next run. `make verify-latency`
/// re-measures and recomputes every verdict rather than comparing against it.
pub const GUEST_WORST_WAKEUP_NS: Measured<u64> =
    Measured::new(273_969, AEGIS_M26_GUEST, MeasurementTool::Cyclictest);

/// The worst-case wakeup latency observed on the reference host, in nanoseconds.
///
/// The same tool, the same argument vector -- both runs record their own
/// arguments and the gate compares them -- and the same ten seconds of samples,
/// on the machine that hosted the guest. Alongside it that run recorded a
/// minimum of 440 nanoseconds and a mean of 1449, so on the mean the
/// non-realtime host was roughly six times steadier than the realtime guest,
/// whose virtual CPU that host was scheduling.
///
/// **This figure is smaller than [`GUEST_WORST_WAKEUP_NS`] and satisfies
/// nothing.** [`Measured::verdict`] sends it to
/// [`DeterminismVerdict::KernelNotRealtime`] at every threshold, because
/// [`REFERENCE_HOST`] carries [`PreemptionModel::NotPreemptRt`] and the kernel
/// is checked before the number. `tests/measured_figures.rs` pins both halves:
/// that the host figure is the lower of the two, and that it still satisfies no
/// edge.
///
/// Which of the two maxima is smaller is not even stable. Five runs recorded
/// on 2026-09-13 gave guest maxima of 273969, 214129, 139079, 203358 and 290108
/// nanoseconds against host maxima of 267461, 192784, 151594, 184724 and
/// 257986, so the host's was lower four times and higher once. The means never
/// crossed -- 8329 to 9664 in the guest against 1048 to 2701 on the host --
/// which is the virtualisation cost showing through. Ordering the two kernels
/// by their worst case would therefore be both wrong in principle and unstable
/// in practice. `docs/build/latency.md` carries the table.
pub const HOST_WORST_WAKEUP_NS: Measured<u64> =
    Measured::new(267_461, REFERENCE_HOST, MeasurementTool::Cyclictest);
