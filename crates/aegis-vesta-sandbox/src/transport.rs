// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The `AF_VSOCK` transport measured on the admitted monitor (milestone M22,
//! epic E22-2).
//!
//! M21 proved that a candidate evaluation round-trips over `AF_VSOCK` under
//! Firecracker. This module turns that round trip into a figure, and records
//! the mechanism it ran on instead of assuming it:
//!
//! * [`HostVsock`] reads what the host offers: the `/dev/vhost-vsock`
//!   character device, whether `vhost_vsock` is a loadable module
//!   (`CONFIG_VHOST_VSOCK=m`, which Firecracker's `docs/vsock.md` at v1.17.0
//!   lists as a host prerequisite) and the module's reference count;
//! * [`check_mechanism`] holds the mechanism that same document describes:
//!   Firecracker's own virtio-vsock device model carries the bytes between an
//!   `AF_UNIX` socket on the host and `AF_VSOCK` in the guest, "bypassing
//!   vhost kernel code on the host". A `vhost_vsock` reference count that rose
//!   while the microVM ran contradicts that, and is refused;
//! * [`RoundTripFigure`] is the figure, and it names its monitor: decision D58
//!   keeps Firecracker and `QEMU` figures apart, so a figure whose measured
//!   monitor is not the one the request recorded is refused rather than
//!   relabelled.
//!
//! One round trip is everything the host waits for: the `AF_UNIX` connect,
//! the `CONNECT`/`OK` handshake, one request line, the guest's Pareto
//! judgement, one answer line and the close. It is a wall-clock figure on the
//! reference profile, not a price: REQ-P10-03's pricing is not implemented.

use std::io::ErrorKind;
use std::os::unix::fs::FileTypeExt;
use std::path::Path;
use std::time::{Duration, Instant};

use aegis_vesta::{VmId, VmmIdentity};

use crate::error::SandboxError;
use crate::run::{SUPERIOR, request};
use crate::vm::{RunningVm, evaluate_in};

/// The character device the host's vhost transport is reached through.
pub const VHOST_DEVICE: &str = "/dev/vhost-vsock";

/// The sysfs directory of the host's vhost transport module.
pub const VHOST_MODULE: &str = "/sys/module/vhost_vsock";

/// Exchanges the boot case makes with the first microVM before this one runs.
pub const BOOT_CASE_EXCHANGES: u32 = 3;

/// Round trips timed for the figure.
///
/// The guest answers at most [`saeptum_microvm::MAX_GUEST_CONNECTIONS`]
/// connections and then restarts, which Firecracker answers by exiting; the
/// first microVM must still be running for the 64-microVM case, so the boot
/// case's exchanges and these stay below that bound;
/// `tests/transport.rs` holds the sum.
pub const ROUND_TRIPS: u32 = 12;

/// How the host's kernel carries `vhost_vsock`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModuleBuild {
    /// A loadable module, loaded: `CONFIG_VHOST_VSOCK=m`.
    Loadable,
    /// Built into the kernel: `CONFIG_VHOST_VSOCK=y`.
    BuiltIn,
    /// No `vhost_vsock` in sysfs: not built, or a module not loaded.
    Absent,
}

impl ModuleBuild {
    /// Returns the configuration line this build state stands for.
    #[must_use]
    pub const fn config_line(self) -> &'static str {
        match self {
            Self::Loadable => "CONFIG_VHOST_VSOCK=m",
            Self::BuiltIn => "CONFIG_VHOST_VSOCK=y",
            Self::Absent => "no vhost_vsock in /sys/module",
        }
    }
}

/// What the host offers for `AF_VSOCK`, read once.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostVsock {
    /// Whether the vhost device exists and is a character device.
    pub device: bool,
    /// How the kernel carries `vhost_vsock`.
    pub module: ModuleBuild,
    /// The module's reference count, when it is a loaded module.
    pub references: Option<u32>,
}

impl HostVsock {
    /// Reads the host's state from [`VHOST_DEVICE`] and [`VHOST_MODULE`].
    ///
    /// # Errors
    ///
    /// See [`HostVsock::read_from`].
    pub fn read() -> Result<Self, SandboxError> {
        Self::read_from(Path::new(VHOST_DEVICE), Path::new(VHOST_MODULE))
    }

    /// Reads the host's state from `device` and the module directory `module`.
    ///
    /// A loadable module is the one whose directory carries `initstate`; a
    /// built-in one has a directory without it.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Io`] when a path exists but cannot be read, and
    /// [`SandboxError::Refused`] when `refcnt` is not a count.
    pub fn read_from(device: &Path, module: &Path) -> Result<Self, SandboxError> {
        let device = match std::fs::metadata(device) {
            Ok(meta) => meta.file_type().is_char_device(),
            Err(error) if error.kind() == ErrorKind::NotFound => false,
            Err(error) => return Err(SandboxError::io("reading the vhost device", error)),
        };
        let build = if module.join("initstate").is_file() {
            ModuleBuild::Loadable
        } else if module.is_dir() {
            ModuleBuild::BuiltIn
        } else {
            ModuleBuild::Absent
        };
        let references = if build == ModuleBuild::Loadable {
            Some(read_count(&module.join("refcnt"))?)
        } else {
            None
        };
        Ok(Self {
            device,
            module: build,
            references,
        })
    }
}

/// Reads one decimal count from a sysfs file.
fn read_count(path: &Path) -> Result<u32, SandboxError> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| SandboxError::io("reading the vhost_vsock reference count", error))?;
    text.trim().parse().map_err(|_| {
        SandboxError::Refused(format!(
            "{} holds {:?}, not a reference count",
            path.display(),
            text.trim()
        ))
    })
}

/// Holds the mechanism `vmm` documents against the host read `before` any
/// microVM started and `during` one.
///
/// # Errors
///
/// Returns [`SandboxError::Refused`] when `vmm` is not Firecracker, the only
/// monitor whose transport this crate starts, or when the `vhost_vsock`
/// reference count rose while the microVM ran: Firecracker's transport does
/// not use it.
pub fn check_mechanism(
    vmm: VmmIdentity,
    before: HostVsock,
    during: HostVsock,
) -> Result<(), SandboxError> {
    if vmm != VmmIdentity::Firecracker {
        return Err(SandboxError::Refused(format!(
            "the sandbox starts Firecracker; a {} transport is not what ran",
            vmm.tag()
        )));
    }
    match (before.references, during.references) {
        (Some(was), Some(now)) if now > was => Err(SandboxError::Refused(format!(
            "vhost_vsock references rose from {was} to {now} while the microVM ran; \
             Firecracker's hybrid vsock does not use vhost"
        ))),
        _ => Ok(()),
    }
}

/// One measured round-trip figure and the monitor that produced it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RoundTripFigure {
    /// The monitor the figure was measured on.
    pub vmm: VmmIdentity,
    /// That monitor's version, the one D58 pins and the gate checked.
    pub version: &'static str,
    /// How many round trips the figure covers.
    pub samples: u32,
    /// The fastest round trip.
    pub min: Duration,
    /// The lower median round trip.
    pub median: Duration,
    /// The slowest round trip.
    pub max: Duration,
}

impl RoundTripFigure {
    /// Builds the figure for `samples` measured on `measured`, for a request
    /// that recorded `recorded`.
    ///
    /// # Errors
    ///
    /// Returns [`SandboxError::Refused`] when the two monitors differ, when
    /// there is no sample, or when there are more than [`ROUND_TRIPS`].
    pub fn from_samples(
        recorded: VmmIdentity,
        measured: VmmIdentity,
        samples: &[Duration],
    ) -> Result<Self, SandboxError> {
        if recorded != measured {
            return Err(SandboxError::Refused(format!(
                "a figure measured on {} is not a figure for a request that recorded {}",
                measured.tag(),
                recorded.tag()
            )));
        }
        let count = u32::try_from(samples.len())
            .ok()
            .filter(|count| (1..=ROUND_TRIPS).contains(count))
            .ok_or_else(|| {
                SandboxError::Refused(format!(
                    "{} round trips; a figure covers 1 to {ROUND_TRIPS}",
                    samples.len()
                ))
            })?;
        let mut sorted = samples.to_vec();
        sorted.sort_unstable();
        let last = sorted.len().saturating_sub(1);
        let pick = |at: usize| sorted.get(at).copied().unwrap_or_default();
        Ok(Self {
            vmm: measured,
            version: measured.pinned_version(),
            samples: count,
            min: pick(0),
            median: pick(last / 2),
            max: pick(last),
        })
    }
}

/// Times [`ROUND_TRIPS`] evaluations of the superior candidate in `vm`.
///
/// # Errors
///
/// Propagates [`evaluate_in`] and [`request`].
pub fn time_round_trips(vm: &mut RunningVm, vm_id: VmId) -> Result<Vec<Duration>, SandboxError> {
    let mut samples = Vec::new();
    for index in 0..ROUND_TRIPS {
        let request = request(vm_id, &format!("rt{index:02}"), SUPERIOR)?;
        let started = Instant::now();
        evaluate_in(vm, &request)?;
        samples.push(started.elapsed());
    }
    Ok(samples)
}

/// Formats a duration in microseconds with one decimal.
#[must_use]
pub fn micros(duration: Duration) -> String {
    let tenths = duration.as_nanos() / 100;
    format!("{}.{} us", tenths / 10, tenths % 10)
}
