// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-vesta-sandbox`: the host half of E21-2, started by
//! `tools/verify_workstation.py`.
//!
//! `aegis-vesta-sandbox run <firecracker> <kernel> <initramfs> <run-dir>`
//! runs the five sandbox cases. The exit status is 0 when every case passed,
//! 1 when a case failed and 3 when the report could not be written.

use std::io::Write;
use std::path::PathBuf;
use std::process::ExitCode;

use aegis_vesta_sandbox::{Launch, Verdict, run_sandbox};

const USAGE: &str = "usage: aegis-vesta-sandbox run <firecracker> <kernel> <initramfs> <run-dir>";

fn main() -> ExitCode {
    let arguments: Vec<String> = std::env::args().skip(1).take(6).collect();
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let [mode, firecracker, kernel, initrd, run_dir] = arguments.as_slice() else {
        let _ = writeln!(out, "{USAGE}");
        return ExitCode::from(64);
    };
    if mode != "run" {
        let _ = writeln!(out, "{USAGE}");
        return ExitCode::from(64);
    }
    let paths = [firecracker, kernel, initrd, run_dir].map(PathBuf::from);
    let [firecracker, kernel, initrd, run_dir] = &paths;
    let launch = Launch {
        firecracker,
        kernel,
        initrd,
        run_dir,
    };
    let (line, code) = match run_sandbox(&launch, &mut out) {
        Ok(Verdict::Passed) => ("RESULT pass".to_owned(), 0),
        Ok(Verdict::Failed(count)) => (format!("RESULT fail: {count} case(s) failed"), 1),
        Err(error) => (format!("RESULT error: {error}"), 3),
    };
    if writeln!(out, "{line}").and_then(|()| out.flush()).is_err() {
        return ExitCode::from(3);
    }
    ExitCode::from(code)
}
