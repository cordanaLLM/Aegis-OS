// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-forum-shell-gpui`: surface mapping, accessibility fixture, and focus ring fixture entry point on gpui (M28 E28-2, E28-3, E28-4).
//!
//! Subcommands:
//! - `surface <request-line>`: decodes a `MOUNT_SHELL_CANVAS` request line, connects to the
//!   compositor via gpui, maps a `zwlr_layer_shell_v1` surface, and awaits the first configure event.
//! - `a11y <request-line> <variant>`: decodes the canvas, validates the test variant, and hosts
//!   the AT-SPI fixture surface exposing the accessible button, seeded list, and panels.
//! - `ring <request-line> <variant>`: decodes the canvas, validates the ring variant, and hosts
//!   the focus-ring fixture surface drawing the canvas node and focus ring.
//!
//! Exit codes:
//! - 0: surface configured / fixture completed cleanly
//! - 3: window open failed or stdout write error
//! - 4: compositor lacks `zwlr_layer_shell_v1`
//! - 5: configure event did not arrive before deadline
//! - 6: request line was refused before touching the display
//! - 64: invalid command-line arguments or unknown variant
//!
//! Every pass is development evidence for the client half only (D79).

use std::io::Write;
use std::process::ExitCode;
use std::str::FromStr;

use aegis_forum_shell_gpui::fixture::{Variant, run_fixture};
use aegis_forum_shell_gpui::ring_fixture::{RingVariant, run_ring_fixture};
use aegis_forum_shell_gpui::surface::{decode_canvas, exit_code, outcome_line, run_surface};

fn handle_surface(request_line: &str, mut out: impl Write) -> ExitCode {
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

fn handle_a11y(request_line: &str, variant_str: &str, mut out: impl Write) -> ExitCode {
    let canvas = match decode_canvas(request_line.as_bytes()) {
        Ok(canvas) => canvas,
        Err(refused) => {
            let formatted = outcome_line(&refused);
            let _ = writeln!(out, "{formatted}").and_then(|()| out.flush());
            return ExitCode::from(exit_code(&refused));
        }
    };

    let Ok(variant) = Variant::from_str(variant_str) else {
        let _ = writeln!(
            out,
            "usage: aegis-forum-shell-gpui a11y <request-line> <variant>"
        );
        return ExitCode::from(64);
    };

    let outcome = run_fixture(&canvas, variant, std::io::stdout());
    let formatted = outcome_line(&outcome);
    let written = writeln!(out, "{formatted}").and_then(|()| out.flush());
    if written.is_err() {
        return ExitCode::from(3);
    }

    ExitCode::from(exit_code(&outcome))
}

fn handle_ring(request_line: &str, variant_str: &str, mut out: impl Write) -> ExitCode {
    let canvas = match decode_canvas(request_line.as_bytes()) {
        Ok(canvas) => canvas,
        Err(refused) => {
            let formatted = outcome_line(&refused);
            let _ = writeln!(out, "{formatted}").and_then(|()| out.flush());
            return ExitCode::from(exit_code(&refused));
        }
    };

    let Ok(variant) = RingVariant::from_str(variant_str) else {
        let _ = writeln!(
            out,
            "usage: aegis-forum-shell-gpui ring <request-line> <variant>"
        );
        return ExitCode::from(64);
    };

    let outcome = run_ring_fixture(&canvas, variant, std::io::stdout());
    let formatted = outcome_line(&outcome);
    let written = writeln!(out, "{formatted}").and_then(|()| out.flush());
    if written.is_err() {
        return ExitCode::from(3);
    }

    ExitCode::from(exit_code(&outcome))
}

fn main() -> ExitCode {
    let mut cli_args = std::env::args();
    let _binary = cli_args.next();
    let subcmd = cli_args.next();
    let first = cli_args.next();
    let second = cli_args.next();
    let extra = cli_args.next();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    match (subcmd.as_deref(), first, second, extra) {
        (Some("surface"), Some(line), None, None) => handle_surface(&line, out),
        (Some("a11y"), Some(line), Some(variant), None) => handle_a11y(&line, &variant, out),
        (Some("ring"), Some(line), Some(variant), None) => handle_ring(&line, &variant, out),
        _ => {
            let _ = writeln!(
                out,
                "usage: aegis-forum-shell-gpui surface <request-line> | aegis-forum-shell-gpui a11y <request-line> <variant> | aegis-forum-shell-gpui ring <request-line> <variant>"
            );
            ExitCode::from(64)
        }
    }
}
