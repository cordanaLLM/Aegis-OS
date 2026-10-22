// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Shared helpers for readback CLI binaries (M28).

use std::fmt::Display;
use std::io::Write;
use std::process::ExitCode;

/// Formats readback result or error, returning the corresponding exit code.
pub fn finish_readback<E: Display>(result: Result<bool, E>, mut out: impl Write) -> ExitCode {
    match result {
        Ok(true) => ExitCode::SUCCESS,
        Ok(false) => ExitCode::from(1),
        Err(err) => {
            let _ = writeln!(out, "ERROR {err}");
            let _ = out.flush();
            ExitCode::from(3)
        }
    }
}
