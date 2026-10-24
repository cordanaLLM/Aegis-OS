// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-forum-shell-gpui`: surface mapping, accessibility fixture, focus ring, and timing entry point (M28).
//!
//! Subcommands:
//! - `surface <request-line>`: decodes a `MOUNT_SHELL_CANVAS` request line, connects to the
//!   compositor via gpui, maps a `zwlr_layer_shell_v1` surface, and awaits the first configure event.
//! - `a11y <request-line> <variant>`: decodes the canvas, validates the test variant, and hosts
//!   the AT-SPI fixture surface exposing the accessible button, seeded list, and panels.
//! - `ring <request-line> <variant>`: decodes the canvas, validates the ring variant, and hosts
//!   the focus-ring fixture surface drawing the canvas node and focus ring.
//! - `timing <request-line> <seeded-count>`: decodes the canvas, validates the seeded count, and hosts
//!   the timing measurement surface collecting frame and tree-update durations (requires `timing` feature).
//!
//! Exit codes:
//! - 0: surface configured / fixture completed cleanly
//! - 3: window open failed or stdout write error
//! - 4: compositor lacks `zwlr_layer_shell_v1`
//! - 5: configure event did not arrive before deadline
//! - 6: request line was refused before touching the display
//! - 64: invalid command-line arguments, unknown variant, or count out of range
//!
//! Every pass is development evidence for the client half only (D79).

use std::io::Write;
use std::process::ExitCode;
use std::str::FromStr;

use aegis_forum_shell_gpui::fixture::{Variant, run_fixture};
use aegis_forum_shell_gpui::ring_fixture::{RingVariant, run_ring_fixture};
use aegis_forum_shell_gpui::slider_fixture::{SliderVariant, run_slider_fixture};
use aegis_forum_shell_gpui::surface::{
    Outcome, decode_canvas, exit_code, outcome_line, run_surface,
};

const USAGE: &str = "usage: aegis-forum-shell-gpui surface <request-line> | aegis-forum-shell-gpui a11y <request-line> <variant> | aegis-forum-shell-gpui ring <request-line> <variant> | aegis-forum-shell-gpui slider <request-line> <variant> | aegis-forum-shell-gpui timing <request-line> <seeded-count>";

fn write_outcome(outcome: &Outcome, mut out: impl Write) -> std::io::Result<()> {
    let formatted = outcome_line(outcome);
    writeln!(out, "{formatted}").and_then(|()| out.flush())
}

/// Writes a subcommand's usage line best-effort and exits 64.
fn usage_error(mut out: impl Write, usage: &str) -> ExitCode {
    let _ = writeln!(out, "{usage}");
    ExitCode::from(64)
}

/// Writes the outcome line; an unwritable line exits 3.
fn emit_outcome(outcome: &Outcome, out: impl Write) -> ExitCode {
    if write_outcome(outcome, out).is_err() {
        return ExitCode::from(3);
    }

    ExitCode::from(exit_code(outcome))
}

/// Writes a refusal line best-effort; the refusal's own exit code wins.
fn emit_refusal(refused: &Outcome, out: impl Write) -> ExitCode {
    let _ = write_outcome(refused, out);
    ExitCode::from(exit_code(refused))
}

fn handle_surface(request_line: &str, out: impl Write) -> ExitCode {
    let outcome = match decode_canvas(request_line.as_bytes()) {
        Ok(canvas) => run_surface(&canvas),
        Err(refused_outcome) => refused_outcome,
    };

    emit_outcome(&outcome, out)
}

fn handle_a11y(request_line: &str, variant_str: &str, out: impl Write) -> ExitCode {
    let canvas = match decode_canvas(request_line.as_bytes()) {
        Ok(canvas) => canvas,
        Err(refused) => return emit_refusal(&refused, out),
    };

    let Ok(variant) = Variant::from_str(variant_str) else {
        return usage_error(
            out,
            "usage: aegis-forum-shell-gpui a11y <request-line> <variant>",
        );
    };

    let outcome = run_fixture(&canvas, variant, std::io::stdout());
    emit_outcome(&outcome, out)
}

fn handle_ring(request_line: &str, variant_str: &str, out: impl Write) -> ExitCode {
    let canvas = match decode_canvas(request_line.as_bytes()) {
        Ok(canvas) => canvas,
        Err(refused) => return emit_refusal(&refused, out),
    };

    let Ok(variant) = RingVariant::from_str(variant_str) else {
        return usage_error(
            out,
            "usage: aegis-forum-shell-gpui ring <request-line> <variant>",
        );
    };

    let outcome = run_ring_fixture(&canvas, variant, std::io::stdout());
    emit_outcome(&outcome, out)
}

fn handle_slider(request_line: &str, variant_str: &str, out: impl Write) -> ExitCode {
    let canvas = match decode_canvas(request_line.as_bytes()) {
        Ok(canvas) => canvas,
        Err(refused) => return emit_refusal(&refused, out),
    };

    let Ok(variant) = SliderVariant::from_str(variant_str) else {
        return usage_error(
            out,
            "usage: aegis-forum-shell-gpui slider <request-line> <variant>",
        );
    };

    let outcome = run_slider_fixture(&canvas, variant, std::io::stdout());
    emit_outcome(&outcome, out)
}

#[cfg(feature = "timing")]
fn handle_timing(request_line: &str, count_str: &str, out: impl Write) -> ExitCode {
    let canvas = match decode_canvas(request_line.as_bytes()) {
        Ok(canvas) => canvas,
        Err(refused) => return emit_refusal(&refused, out),
    };

    let Ok(count) = count_str.parse::<usize>() else {
        return usage_error(
            out,
            "usage: aegis-forum-shell-gpui timing <request-line> <seeded-count>",
        );
    };

    if !(1..=5000).contains(&count) {
        return usage_error(
            out,
            "usage: aegis-forum-shell-gpui timing <request-line> <seeded-count>",
        );
    }

    let outcome = aegis_forum_shell_gpui::timing_fixture::run_timing_fixture(
        &canvas,
        count,
        std::io::stdout(),
    );
    emit_outcome(&outcome, out)
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
        (Some("slider"), Some(line), Some(variant), None) => handle_slider(&line, &variant, out),
        #[cfg(feature = "timing")]
        (Some("timing"), Some(line), Some(count), None) => handle_timing(&line, &count, out),
        #[cfg(feature = "timing")]
        (Some("timing"), _, _, _) => {
            let _ = writeln!(
                out,
                "usage: aegis-forum-shell-gpui timing <request-line> <seeded-count>"
            );
            ExitCode::from(64)
        }
        #[cfg(not(feature = "timing"))]
        (Some("timing"), _, _, _) => {
            let _ = writeln!(out, "timing needs the timing feature");
            ExitCode::from(64)
        }
        _ => {
            let _ = writeln!(out, "{USAGE}");
            ExitCode::from(64)
        }
    }
}
