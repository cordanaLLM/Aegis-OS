// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M22 / E22-2: the transport figure names its monitor, the host's vhost
//! state is read as it is, and a rise in `vhost_vsock` references is refused.
//!
//! The sysfs directory is a fixture under the test's scratch directory and the
//! character device is `/dev/null`; no microVM starts here. The timed round
//! trips themselves run in `make verify-workstation`.

mod common;

#[path = "../../binary_transcript.rs"]
mod binary_transcript;

use std::path::{Path, PathBuf};
use std::time::Duration;

use aegis_vesta::VmmIdentity;
use aegis_vesta_sandbox::{
    BOOT_CASE_EXCHANGES, HostVsock, ModuleBuild, ROUND_TRIPS, RoundTripFigure, SandboxError,
    check_mechanism, micros,
};
use common::Fallible;

/// A sysfs module directory holding `files`.
fn module_dir(label: &str, files: &[(&str, &str)]) -> Result<PathBuf, String> {
    let dir = binary_transcript::scratch(label)?.join("vhost_vsock");
    std::fs::create_dir(&dir).map_err(|error| error.to_string())?;
    for (name, body) in files {
        std::fs::write(dir.join(name), body).map_err(|error| error.to_string())?;
    }
    Ok(dir)
}

/// A host whose loadable module holds `references`.
const fn loaded(references: u32) -> HostVsock {
    HostVsock {
        device: true,
        module: ModuleBuild::Loadable,
        references: Some(references),
    }
}

/// `count` round trips of 10, 20, ... microseconds, in reverse order.
fn samples(count: u64) -> Vec<Duration> {
    (1..=count)
        .rev()
        .map(|n| Duration::from_micros(n.saturating_mul(10)))
        .collect()
}

// --- Positive -------------------------------------------------------------

/// Positive: a loaded module with its reference count and a character device
/// read back as `CONFIG_VHOST_VSOCK=m`.
#[test]
fn a_loaded_module_reads_as_loadable() -> Fallible {
    let dir = module_dir("loaded", &[("initstate", "live\n"), ("refcnt", "0\n")])?;
    let host = HostVsock::read_from(Path::new("/dev/null"), &dir)?;
    assert_eq!(host, loaded(0));
    assert_eq!(host.module.config_line(), "CONFIG_VHOST_VSOCK=m");
    Ok(())
}

/// Positive: the figure names Firecracker at its pin, sorted samples give
/// the lower median, and the unsorted input does not matter.
#[test]
fn a_firecracker_figure_names_its_monitor() -> Fallible {
    let figure = RoundTripFigure::from_samples(
        VmmIdentity::Firecracker,
        VmmIdentity::Firecracker,
        &samples(4),
    )?;
    assert_eq!(figure.vmm, VmmIdentity::Firecracker);
    assert_eq!(figure.version, "1.17.0");
    assert_eq!(figure.samples, 4);
    assert_eq!(figure.min, Duration::from_micros(10));
    assert_eq!(figure.median, Duration::from_micros(20));
    assert_eq!(figure.max, Duration::from_micros(40));
    Ok(())
}

/// Positive: unchanged references under Firecracker hold the mechanism.
#[test]
fn unchanged_references_hold_the_mechanism() -> Fallible {
    check_mechanism(VmmIdentity::Firecracker, loaded(0), loaded(0))?;
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a figure measured on one monitor is refused for a request that
/// recorded the other, in both directions.
#[test]
fn a_figure_from_another_monitor_is_refused() {
    for (recorded, measured) in [
        (VmmIdentity::Firecracker, VmmIdentity::QemuMicrovm),
        (VmmIdentity::QemuMicrovm, VmmIdentity::Firecracker),
    ] {
        let refused = RoundTripFigure::from_samples(recorded, measured, &samples(1));
        assert!(
            matches!(&refused, Err(SandboxError::Refused(text)) if text.contains("is not a figure")),
            "{refused:?}"
        );
    }
}

/// Negative: a rise in `vhost_vsock` references while the microVM ran is
/// refused, and so is a transport claimed for `QEMU`.
#[test]
fn a_vhost_transport_is_refused() {
    let rose = check_mechanism(VmmIdentity::Firecracker, loaded(0), loaded(1));
    assert!(
        matches!(&rose, Err(SandboxError::Refused(text)) if text.contains("rose from 0 to 1")),
        "{rose:?}"
    );
    let qemu = check_mechanism(VmmIdentity::QemuMicrovm, loaded(0), loaded(0));
    assert!(matches!(qemu, Err(SandboxError::Refused(_))), "{qemu:?}");
}

/// Negative: a reference count that is not a number is refused, not read as
/// zero.
#[test]
fn a_garbled_reference_count_is_refused() -> Fallible {
    let dir = module_dir("garbled", &[("initstate", "live\n"), ("refcnt", "many\n")])?;
    let read = HostVsock::read_from(Path::new("/dev/null"), &dir);
    assert!(matches!(read, Err(SandboxError::Refused(_))), "{read:?}");
    Ok(())
}

/// Negative: no sample is no figure.
#[test]
fn no_sample_is_no_figure() {
    let none =
        RoundTripFigure::from_samples(VmmIdentity::Firecracker, VmmIdentity::Firecracker, &[]);
    assert!(matches!(none, Err(SandboxError::Refused(_))), "{none:?}");
}

// --- Boundary -------------------------------------------------------------

/// Boundary: [`ROUND_TRIPS`] samples make a figure, one more does not.
#[test]
fn the_round_trip_bound_is_inclusive() -> Fallible {
    let at = RoundTripFigure::from_samples(
        VmmIdentity::Firecracker,
        VmmIdentity::Firecracker,
        &samples(u64::from(ROUND_TRIPS)),
    )?;
    assert_eq!(at.samples, ROUND_TRIPS);
    let over = RoundTripFigure::from_samples(
        VmmIdentity::Firecracker,
        VmmIdentity::Firecracker,
        &samples(u64::from(ROUND_TRIPS).saturating_add(1)),
    );
    assert!(matches!(over, Err(SandboxError::Refused(_))), "{over:?}");
    Ok(())
}

/// Boundary: the boot case's exchanges and the timed ones leave the guest
/// below its connection bound, so the first microVM is still running.
#[test]
fn the_timed_round_trips_stay_below_the_guest_bound() {
    let used = BOOT_CASE_EXCHANGES.checked_add(ROUND_TRIPS);
    assert_eq!(used, Some(15));
    assert_eq!(
        used.and_then(|used| saeptum_microvm::MAX_GUEST_CONNECTIONS.checked_sub(used)),
        Some(1),
        "the first microVM's guest must keep one connection in hand"
    );
}

/// Boundary: a module directory without `initstate` is built in, a missing
/// one is absent, a regular file is no device, and neither has a count.
#[test]
fn built_in_and_absent_modules_carry_no_count() -> Fallible {
    let dir = module_dir("built-in", &[])?;
    let file = dir.join("not-a-device");
    std::fs::write(&file, b"")?;
    let built_in = HostVsock::read_from(&file, &dir)?;
    assert_eq!(built_in.module, ModuleBuild::BuiltIn);
    assert_eq!(built_in.references, None);
    assert!(!built_in.device);
    let absent = HostVsock::read_from(&dir.join("missing"), &dir.join("missing"))?;
    assert_eq!(absent.module, ModuleBuild::Absent);
    assert!(!absent.device);
    check_mechanism(VmmIdentity::Firecracker, absent, absent)?;
    Ok(())
}

/// Boundary: sub-microsecond and whole figures format with one decimal.
#[test]
fn micros_keep_one_decimal() {
    assert_eq!(micros(Duration::from_nanos(0)), "0.0 us");
    assert_eq!(micros(Duration::from_nanos(99)), "0.0 us");
    assert_eq!(micros(Duration::from_nanos(123_456)), "123.4 us");
}
