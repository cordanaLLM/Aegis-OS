// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! M22 / E22-1: the footprint figure names its monitor and refuses a sample
//! that names none or another one; the resident set and the boot timer line
//! are read as their sources write them.
//!
//! No microVM starts here: [`identify`] is held against this test process,
//! and the 64 measured Firecracker processes run in `make verify-workstation`.

mod common;

use std::path::Path;
use std::time::Duration;

use aegis_vesta::{MAX_MICROVMS, VmmIdentity};
use aegis_vesta_sandbox::{
    BOOT_COMPLETE, BOOT_TIMER_MMIO, FootprintFigure, FootprintSample, SandboxError, identify,
    parse_boot_time, parse_resident_kib,
};
use common::Fallible;

/// A Firecracker 1.17.0 boot timer line, as `boot_timer.rs` formats it.
const BOOT_LINE: &str = "2026-10-17T05:00:00.000000000 [anonymous-instance:main] \
                         Guest-boot-time =  95123 us 95 ms,   1904 CPU us 1 CPU ms";

/// A `/proc/<pid>/status` excerpt.
const STATUS: &str = "Name:\tfirecracker\nVmPeak:\t   20000 kB\nVmRSS:\t   41236 kB\nThreads:\t3\n";

/// A sample on `vmm` with `resident_kib`.
const fn sample(vmm: Option<VmmIdentity>, resident_kib: u64) -> FootprintSample {
    FootprintSample { vmm, resident_kib }
}

/// `count` Firecracker samples of 1000, 2000, ... KiB, in reverse order.
fn firecracker(count: u64) -> Vec<FootprintSample> {
    (1..=count)
        .rev()
        .map(|n| sample(Some(VmmIdentity::Firecracker), n.saturating_mul(1000)))
        .collect()
}

/// Asserts that `result` is a refusal whose text holds `needle`.
fn refused<T: std::fmt::Debug>(result: &Result<T, SandboxError>, needle: &str) {
    assert!(
        matches!(result, Err(SandboxError::Refused(reason)) if reason.contains(needle)),
        "expected a refusal holding {needle:?}, got {result:?}"
    );
}

// --- Positive -------------------------------------------------------------

/// Positive: the boot timer line reads as its wall-clock and CPU time.
#[test]
fn the_boot_timer_line_reads_as_wall_and_cpu_time() -> Fallible {
    let console = format!("[    0.000000] Linux version 6.18.44\n{BOOT_LINE}\nmore\n");
    let (wall, cpu) = parse_boot_time(&console)?;
    assert_eq!(wall, Duration::from_micros(95_123));
    assert_eq!(cpu, Duration::from_micros(1904));
    Ok(())
}

/// Positive: the resident set reads as its count of KiB.
#[test]
fn the_resident_set_reads_as_kib() -> Fallible {
    assert_eq!(parse_resident_kib(STATUS)?, 41_236);
    Ok(())
}

/// Positive: a process running the named binary is identified as Firecracker.
#[test]
fn the_running_binary_names_the_monitor() -> Fallible {
    let me = std::env::current_exe()?;
    assert_eq!(
        identify(std::process::id(), &me),
        Some(VmmIdentity::Firecracker)
    );
    Ok(())
}

/// Positive: a figure covers its samples and names the pinned version.
#[test]
fn a_figure_names_its_monitor_and_spread() -> Fallible {
    let figure = FootprintFigure::from_samples(VmmIdentity::Firecracker, &firecracker(5))?;
    assert_eq!(figure.vmm, VmmIdentity::Firecracker);
    assert_eq!(figure.version, VmmIdentity::Firecracker.pinned_version());
    assert_eq!(figure.samples, 5);
    assert_eq!(figure.resident_kib, (1000, 3000, 5000));
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a console without the line is a guest that did not signal.
#[test]
fn a_console_without_the_line_is_refused() {
    refused(
        &parse_boot_time("[    0.000000] Linux version\n"),
        "did not signal",
    );
}

/// Negative: two boot timer lines are refused rather than one picked.
#[test]
fn two_boot_timer_lines_are_refused() {
    let console = format!("{BOOT_LINE}\n{BOOT_LINE}\n");
    refused(&parse_boot_time(&console), "more than one");
}

/// Negative: a line in another format is refused, not half read.
#[test]
fn another_format_is_refused() {
    refused(
        &parse_boot_time("Guest-boot-time = 95123 ms 95 s, 1904 CPU us 1 CPU ms"),
        "not Firecracker 1.17.0's",
    );
    refused(
        &parse_boot_time("Guest-boot-time = soon"),
        "not Firecracker 1.17.0's",
    );
}

/// Negative: a status without `VmRSS`, or with another unit, is refused.
#[test]
fn a_status_without_a_kib_resident_set_is_refused() {
    refused(&parse_resident_kib("Name:\tfirecracker\n"), "no VmRSS");
    refused(
        &parse_resident_kib("VmRSS:\t41236 MB\n"),
        "not a count of kB",
    );
    refused(
        &parse_resident_kib("VmRSS:\tmany kB\n"),
        "not a count of kB",
    );
}

/// Negative: another binary than the one named is no Firecracker.
#[test]
fn another_binary_names_no_monitor() {
    assert_eq!(identify(std::process::id(), Path::new("/bin/sh")), None);
    assert_eq!(
        identify(std::process::id(), Path::new("/nonexistent/firecracker")),
        None
    );
}

/// Negative: a sample naming no monitor is refused, not recorded.
#[test]
fn a_sample_naming_no_monitor_is_refused() {
    let mut samples = firecracker(3);
    samples.push(sample(None, 1));
    refused(
        &FootprintFigure::from_samples(VmmIdentity::Firecracker, &samples),
        "names no monitor",
    );
}

/// Negative: a sample measured on another monitor is refused, not relabelled.
#[test]
fn a_sample_on_another_monitor_is_refused() {
    let mut samples = firecracker(3);
    samples.push(sample(Some(VmmIdentity::QemuMicrovm), 1));
    refused(
        &FootprintFigure::from_samples(VmmIdentity::Firecracker, &samples),
        "D58 keeps their figures apart",
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: one sample and [`MAX_MICROVMS`] samples make a figure; none and
/// one more do not.
#[test]
fn the_sample_count_is_bounded() -> Fallible {
    let one = FootprintFigure::from_samples(VmmIdentity::Firecracker, &firecracker(1))?;
    assert_eq!(one.resident_kib, (1000, 1000, 1000));
    let bound = u64::try_from(MAX_MICROVMS)?;
    let full = FootprintFigure::from_samples(VmmIdentity::Firecracker, &firecracker(bound))?;
    assert_eq!(usize::try_from(full.samples)?, MAX_MICROVMS);
    refused(
        &FootprintFigure::from_samples(VmmIdentity::Firecracker, &[]),
        "a figure covers 1 to",
    );
    refused(
        &FootprintFigure::from_samples(
            VmmIdentity::Firecracker,
            &firecracker(bound.saturating_add(1)),
        ),
        "a figure covers 1 to",
    );
    Ok(())
}

/// Boundary: an even count takes the lower median.
#[test]
fn an_even_count_takes_the_lower_median() -> Fallible {
    let figure = FootprintFigure::from_samples(VmmIdentity::Firecracker, &firecracker(4))?;
    assert_eq!(figure.resident_kib, (1000, 2000, 4000));
    Ok(())
}

/// Boundary: the device address and magic byte are Firecracker 1.17.0's
/// `x86_64` values.
#[test]
fn the_boot_timer_constants_are_the_pinned_ones() {
    assert_eq!(BOOT_TIMER_MMIO, 0xc000_0000);
    assert_eq!(BOOT_COMPLETE, 123);
}
