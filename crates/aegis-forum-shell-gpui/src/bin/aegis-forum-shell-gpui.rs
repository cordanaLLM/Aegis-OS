// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-forum-shell-gpui`: the P05 Forum shell's surface mapping entry point on gpui (M28 E28-2).
//!
//! Started by `make verify-shell` through `tools/verify_shell.py`. Decodes a
//! `MOUNT_SHELL_CANVAS` request line, connects to the compositor via gpui, maps
//! a `zwlr_layer_shell_v1` surface, and awaits the first configure event.
//!
//! The exit status is 0 when the surface was configured, 4 when the compositor
//! lacks `zwlr_layer_shell_v1`, 5 when the first configure timed out at its
//! deadline, 6 when the request line was refused, 3 when window creation failed
//! or an output write error occurred, and 64 on invalid arguments. Every pass is
//! development evidence for the client half only (D79).

use std::io::Write;
use std::process::ExitCode;

use aegis_forum_shell_gpui::surface::{decode_canvas, exit_code, outcome_line, run_surface};

fn main() -> ExitCode {
    let mut args = std::env::args();
    let _binary = args.next();
    let subcmd = args.next();
    let line = args.next();
    let extra = args.next();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    if subcmd.as_deref() != Some("surface") || line.is_none() || extra.is_some() {
        let _ = writeln!(out, "usage: aegis-forum-shell-gpui surface <request-line>");
        return ExitCode::from(64);
    }

    let Some(request_line) = line else {
        return ExitCode::from(64);
    };

    let outcome = match decode_canvas(request_line.as_bytes()) {
        Ok(canvas) => run_surface(&canvas),
        Err(refused_outcome) => refused_outcome,
    };

    let formatted = outcome_line(&outcome);
    let written = writeln!(out, "{formatted}").and_then(|()| out.flush());
    if written.is_err() {
        return ExitCode::from(3);
    }

    ExitCode::from(exit_code(&outcome))
}
