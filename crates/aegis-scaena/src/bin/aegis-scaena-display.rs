// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-scaena-display`: the display slice's run on the reference profile
//! (M27), started by `make verify-display` through `tools/verify_display.py`.
//!
//! `run` is the positive run with the negatives and boundaries that live in
//! it; `driver-probe` is negative (a), started with the session's
//! `LIBVA_DRIVER_NAME=nvidia`. The exit status is 0 when every case passed,
//! 1 when a case failed, 2 when the host lacks a capability of the reference
//! profile (the gate prints the reason as a SKIP) and 3 when the run stopped
//! before its cases were decided. Every pass is development evidence for the
//! client half only (D79).

use std::io::Write;
use std::process::ExitCode;

use aegis_scaena::slice::{Verdict, run_display, run_driver_probe};

fn main() -> ExitCode {
    let mode = std::env::args().nth(1);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let result = match mode.as_deref() {
        Some("run") => run_display(&mut out),
        Some("driver-probe") => run_driver_probe(&mut out),
        _ => {
            let _ = writeln!(out, "usage: aegis-scaena-display run|driver-probe");
            return ExitCode::from(64);
        }
    };
    let (line, code) = match result {
        Ok(Verdict::Passed) => ("RESULT pass".to_owned(), 0),
        Ok(Verdict::Failed(count)) => (format!("RESULT fail: {count} case(s) failed"), 1),
        Ok(Verdict::Skipped(reason)) => (format!("RESULT skip: {reason}"), 2),
        Err(error) => (format!("RESULT error: {error}"), 3),
    };
    let written = writeln!(out, "{line}").and_then(|()| out.flush());
    if written.is_err() {
        return ExitCode::from(3);
    }
    ExitCode::from(code)
}
