// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Memory footprint and boot time measured on the admitted monitor
//! (milestone M22, epic E22-1).
//!
//! * [`parse_resident_kib`] reads a process's resident set from
//!   `/proc/<pid>/status`: the Firecracker process with the guest memory it
//!   has touched, not the guest's own view of its memory;
//! * [`identify`] names the monitor from the binary the process runs, not
//!   from what the request asked for;
//! * [`FootprintFigure`] is the figure. Decision D58 keeps Firecracker and
//!   `QEMU` figures apart, so a sample whose monitor could not be identified
//!   is refused rather than recorded, and one measured on another monitor
//!   than the request recorded is refused rather than relabelled;
//! * [`parse_boot_time`] reads the line Firecracker 1.17.0 logs to its stdout,
//!   the microVM's `console.log`, when its boot timer is on
//!   ([`saeptum_microvm::vm::LaunchOptions::with_boot_timer`]) and the guest
//!   writes [`BOOT_COMPLETE`] to [`BOOT_TIMER_MMIO`]
//!   (`src/vmm/src/devices/pseudo/boot_timer.rs`). The interval starts when
//!   Firecracker starts building the microVM (`src/vmm/src/builder.rs`,
//!   `request_ts`). [`read_boot_time`] reads it from a running microVM and
//!   [`BootTimeFigure`] is the figure, under the same D58 naming.
//!
//! The guest's write is [`saeptum_microvm::guest::signal_boot_complete`],
//! the first act of `aegis-vesta-guest`. Its one mapped store to the device
//! address lives in Saeptum under a dated exception, so this crate keeps
//! `unsafe_code = "forbid"`.

use std::io::Read;
use std::path::Path;
use std::time::Duration;

use aegis_vesta::{MAX_MICROVMS, VmmIdentity};

use crate::error::SandboxError;
use crate::vm::RunningVm;

/// The guest-physical address of Firecracker 1.17.0's boot timer device on
/// `x86_64`: `BOOT_DEVICE_MEM_START`, the start of the 32-bit MMIO gap
/// (`src/vmm/src/arch/x86_64/layout.rs`).
pub const BOOT_TIMER_MMIO: u64 = 0xc000_0000;

/// The byte that tells the boot timer the guest has booted
/// (`MAGIC_VALUE_SIGNAL_GUEST_BOOT_COMPLETE`).
pub const BOOT_COMPLETE: u8 = 123;

/// What Firecracker's boot timer line starts with.
pub const BOOT_TIME_MARKER: &str = "Guest-boot-time =";

/// Scalar upper bound, in bytes, on the `/proc/<pid>/status` read.
pub const MAX_STATUS_BYTES: u64 = 1 << 16;

/// Scalar upper bound, in bytes, on the `console.log` read: the guest
/// kernel's console and Firecracker's own log, a few KiB per boot.
pub const MAX_CONSOLE_BYTES: u64 = 1 << 20;

/// The file in a microVM's directory that holds Firecracker's stdout.
pub const CONSOLE_LOG: &str = "console.log";

/// One microVM's measured footprint.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FootprintSample {
    /// The monitor the process runs, or `None` when it could not be told.
    pub vmm: Option<VmmIdentity>,
    /// The monitor process's resident set after the first answer, in KiB.
    pub resident_kib: u64,
}

/// Returns the wall-clock and CPU time of the one boot timer line in
/// `console`.
///
/// # Errors
///
/// Returns [`SandboxError::Refused`] when the console holds no such line
/// (the guest never signalled, or the timer was off), more than one, or one
/// that is not Firecracker 1.17.0's format.
pub fn parse_boot_time(console: &str) -> Result<(Duration, Duration), SandboxError> {
    let mut found = console
        .lines()
        .filter_map(|line| line.split_once(BOOT_TIME_MARKER).map(|(_, rest)| rest));
    let rest = found.next().ok_or_else(|| {
        SandboxError::Refused(format!(
            "the console holds no `{BOOT_TIME_MARKER}` line: the guest did not signal its boot"
        ))
    })?;
    if found.next().is_some() {
        return Err(SandboxError::Refused(format!(
            "the console holds more than one `{BOOT_TIME_MARKER}` line"
        )));
    }
    let words: Vec<&str> = rest.split_whitespace().take(11).collect();
    let (Some(wall), Some(cpu)) = (
        words.first().and_then(|word| word.parse().ok()),
        words.get(4).and_then(|word| word.parse().ok()),
    ) else {
        return Err(malformed(rest));
    };
    let units: Vec<&str> = [1, 3, 5, 6, 8, 9]
        .iter()
        .filter_map(|at| words.get(*at).copied())
        .collect();
    if units != ["us", "ms,", "CPU", "us", "CPU", "ms"] {
        return Err(malformed(rest));
    }
    Ok((Duration::from_micros(wall), Duration::from_micros(cpu)))
}

/// The refusal for a boot timer line that is not in the expected format.
fn malformed(rest: &str) -> SandboxError {
    SandboxError::Refused(format!(
        "`{BOOT_TIME_MARKER}{rest}` is not Firecracker 1.17.0's boot timer format"
    ))
}

/// Returns the `VmRSS` figure, in KiB, of a `/proc/<pid>/status` text.
///
/// # Errors
///
/// Returns [`SandboxError::Refused`] when there is no `VmRSS` line or its
/// value is not a count of kB.
pub fn parse_resident_kib(status: &str) -> Result<u64, SandboxError> {
    let line = status
        .lines()
        .find_map(|line| line.strip_prefix("VmRSS:"))
        .ok_or_else(|| SandboxError::Refused("the process status has no VmRSS line".to_owned()))?;
    let mut words = line.split_whitespace();
    match (words.next().map(str::parse), words.next(), words.next()) {
        (Some(Ok(kib)), Some("kB"), None) => Ok(kib),
        _ => Err(SandboxError::Refused(format!(
            "VmRSS:{line} is not a count of kB"
        ))),
    }
}

/// Returns the monitor process `pid` runs: Firecracker when its executable is
/// the pinned `firecracker` binary, and `None` when that cannot be told.
#[must_use]
pub fn identify(pid: u32, firecracker: &Path) -> Option<VmmIdentity> {
    let running = std::fs::read_link(format!("/proc/{pid}/exe")).ok()?;
    let pinned = std::fs::canonicalize(firecracker).ok()?;
    (running == pinned).then_some(VmmIdentity::Firecracker)
}

/// Reads at most `bound` bytes of `path` as text.
fn read_bounded(path: &Path, bound: u64, what: &'static str) -> Result<String, SandboxError> {
    let file = std::fs::File::open(path).map_err(|error| SandboxError::io(what, error))?;
    let mut text = String::new();
    file.take(bound)
        .read_to_string(&mut text)
        .map_err(|error| SandboxError::io(what, error))?;
    Ok(text)
}

/// Measures the footprint of `vm`, which has answered at least once.
///
/// # Errors
///
/// Returns [`SandboxError::Io`] when its process status cannot be read, and
/// propagates [`parse_resident_kib`].
pub fn measure(vm: &RunningVm, firecracker: &Path) -> Result<FootprintSample, SandboxError> {
    let status = read_bounded(
        Path::new(&format!("/proc/{}/status", vm.pid())),
        MAX_STATUS_BYTES,
        "reading the monitor's process status",
    )?;
    Ok(FootprintSample {
        vmm: identify(vm.pid(), firecracker),
        resident_kib: parse_resident_kib(&status)?,
    })
}

/// Reads the boot time of `vm`, which has answered at least once, from its
/// `console.log`.
///
/// # Errors
///
/// Returns [`SandboxError::Io`] when the console cannot be read, and
/// propagates [`parse_boot_time`].
pub fn read_boot_time(vm: &RunningVm) -> Result<(Duration, Duration), SandboxError> {
    let console = read_bounded(
        &vm.dir().join(CONSOLE_LOG),
        MAX_CONSOLE_BYTES,
        "reading the microVM's console",
    )?;
    parse_boot_time(&console)
}

/// The count of `len` samples as a figure records it.
fn sample_count(len: usize) -> Result<u32, SandboxError> {
    Some(len)
        .filter(|count| (1..=MAX_MICROVMS).contains(count))
        .and_then(|count| u32::try_from(count).ok())
        .ok_or_else(|| {
            SandboxError::Refused(format!(
                "{len} samples; a figure covers 1 to {MAX_MICROVMS}"
            ))
        })
}

/// The smallest, lower median and largest of `values`.
fn spread<T: Ord + Copy + Default>(mut values: Vec<T>) -> (T, T, T) {
    values.sort_unstable();
    let last = values.len().saturating_sub(1);
    let pick = |at: usize| values.get(at).copied().unwrap_or_default();
    (pick(0), pick(last / 2), pick(last))
}

/// A footprint figure and the monitor that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FootprintFigure {
    /// The monitor every sample was measured on.
    pub vmm: VmmIdentity,
    /// That monitor's version, the one D58 pins and the gate checked.
    pub version: &'static str,
    /// How many microVMs the figure covers.
    pub samples: u32,
    /// Smallest, lower median and largest resident set, in KiB.
    pub resident_kib: (u64, u64, u64),
}

impl FootprintFigure {
    /// Builds the figure for `samples` taken for requests that recorded
    /// `recorded`.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Refused`] when a sample names no monitor or
    /// another one than `recorded`, when there is no sample, or when there
    /// are more than [`MAX_MICROVMS`].
    pub fn from_samples(
        recorded: VmmIdentity,
        samples: &[FootprintSample],
    ) -> Result<Self, SandboxError> {
        let count = sample_count(samples.len())?;
        for (ordinal, sample) in samples.iter().enumerate().take(MAX_MICROVMS) {
            check_identity(recorded, sample.vmm, ordinal)?;
        }
        Ok(Self {
            vmm: recorded,
            version: recorded.pinned_version(),
            samples: count,
            resident_kib: spread(samples.iter().map(|s| s.resident_kib).collect()),
        })
    }
}

/// A boot time figure and the monitor whose boot timer produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootTimeFigure {
    /// The monitor every boot ran on; only Firecracker has the boot timer.
    pub vmm: VmmIdentity,
    /// That monitor's version, the one D58 pins and the gate checked.
    pub version: &'static str,
    /// How many microVMs the figure covers.
    pub samples: u32,
    /// Smallest, lower median and largest wall-clock boot time.
    pub wall: (Duration, Duration, Duration),
    /// Smallest, lower median and largest CPU time of the boot.
    pub cpu: (Duration, Duration, Duration),
}

impl BootTimeFigure {
    /// Builds the Firecracker figure for `samples`, each one a boot's
    /// wall-clock and CPU time as [`parse_boot_time`] reads them.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Refused`] when there is no sample, or when
    /// there are more than [`MAX_MICROVMS`].
    pub fn from_samples(samples: &[(Duration, Duration)]) -> Result<Self, SandboxError> {
        let count = sample_count(samples.len())?;
        let vmm = VmmIdentity::Firecracker;
        Ok(Self {
            vmm,
            version: vmm.pinned_version(),
            samples: count,
            wall: spread(samples.iter().map(|(wall, _)| *wall).collect()),
            cpu: spread(samples.iter().map(|(_, cpu)| *cpu).collect()),
        })
    }
}

/// Refuses a sample whose monitor is unknown or not the recorded one.
fn check_identity(
    recorded: VmmIdentity,
    measured: Option<VmmIdentity>,
    ordinal: usize,
) -> Result<(), SandboxError> {
    match measured {
        None => Err(SandboxError::Refused(format!(
            "sample {ordinal} names no monitor; a figure without one is refused, not recorded"
        ))),
        Some(vmm) if vmm != recorded => Err(SandboxError::Refused(format!(
            "sample {ordinal} was measured on {}, not on {}; D58 keeps their figures apart",
            vmm.tag(),
            recorded.tag()
        ))),
        Some(_) => Ok(()),
    }
}
