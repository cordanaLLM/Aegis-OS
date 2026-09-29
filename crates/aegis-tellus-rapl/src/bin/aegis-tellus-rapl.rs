// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-tellus-rapl`: the measured-energy half of `make verify-workstation`
//! (M21, E21-1), started by `tools/verify_workstation.py`.
//!
//! `aegis-tellus-rapl run <readings>` judges one readings document, passed as
//! the argument itself. The exit status is 0 when every case passed, 1 when a
//! case failed and 3 when the document was refused before any case ran.

use std::io::Write;
use std::process::ExitCode;

use aegis_tellus_rapl::{Readings, evaluate};

fn main() -> ExitCode {
    let mut arguments = std::env::args().skip(1);
    let stdout = std::io::stdout();
    let mut out = stdout.lock();
    let (Some(mode), Some(document), None) = (arguments.next(), arguments.next(), arguments.next())
    else {
        let _ = writeln!(out, "usage: aegis-tellus-rapl run <readings-json>");
        return ExitCode::from(64);
    };
    if mode != "run" {
        let _ = writeln!(out, "usage: aegis-tellus-rapl run <readings-json>");
        return ExitCode::from(64);
    }
    let evaluated = Readings::parse(&document).and_then(|readings| evaluate(&readings));
    let (lines, result, code) = match evaluated {
        Ok(run) if run.failed() == 0 => (run.lines().to_vec(), "RESULT pass".to_owned(), 0),
        Ok(run) => {
            let result = format!("RESULT fail: {} case(s) failed", run.failed());
            (run.lines().to_vec(), result, 1)
        }
        Err(error) => (Vec::new(), format!("RESULT error: {error}"), 3),
    };
    let written = lines
        .iter()
        .chain(core::iter::once(&result))
        .try_for_each(|line| writeln!(out, "{line}"))
        .and_then(|()| out.flush());
    if written.is_err() {
        return ExitCode::from(3);
    }
    ExitCode::from(code)
}
