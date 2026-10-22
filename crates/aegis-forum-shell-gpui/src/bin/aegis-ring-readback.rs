// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-ring-readback`: focus-ring pixel measurement and verification client (M28 E28-4).
//!
//! Connects to `KWin`'s `org.kde.KWin.ScreenShot2` interface on the private session D-Bus,
//! captures the area containing the focus ring under an explicit deadline, measures
//! stroke width and WCAG contrast ratio on all four sides, and evaluates acceptance.
//!
//! Exit codes:
//! - 0: focus ring satisfied acceptance criteria
//! - 1: focus ring failed acceptance criteria
//! - 3: a D-Bus transport, deadline, or capture error occurred
//! - 64: invalid command-line invocation
//!
//! Every pass is development evidence for the client half only (D79).
//! It measures `KWin`'s composited workspace, not P04's painted output.

use std::io::Write;
use std::process::ExitCode;
use std::str::FromStr;

#[path = "common/mod.rs"]
mod common;

use common::finish_readback;

use aegis_concordia::tokens::COLOR_SURFACE;
use aegis_forum_shell_gpui::capture::{
    CaptureError, capture_area, session_connection, wait_for_pixel,
};
use aegis_forum_shell_gpui::judge::line;
use aegis_forum_shell_gpui::pixels::{judge_ring, measure};
use aegis_forum_shell_gpui::ring::{RingGeometry, capture_rect, parse_ring_line};
use aegis_forum_shell_gpui::ring_fixture::{RingVariant, SURFACE_PROBE};

/// Pixels of surface captured beyond the ring's outer edge on every side.
const CAPTURE_MARGIN: i32 = 4;

const USAGE: &str = "usage: aegis-ring-readback <variant> <ring-line>";

async fn run_readback(
    geom: &RingGeometry,
    variant: RingVariant,
    mut out: impl Write,
) -> Result<bool, CaptureError> {
    let rect = capture_rect(geom, CAPTURE_MARGIN);
    let conn = session_connection().await?;
    let surface = [COLOR_SURFACE.red, COLOR_SURFACE.green, COLOR_SURFACE.blue];
    wait_for_pixel(&conn, SURFACE_PROBE, surface).await?;
    let img = capture_area(&conn, rect).await?;

    let measures = measure(&img, geom, (rect.0, rect.1));
    for m in &measures {
        let _ = writeln!(
            out,
            "MEASURE {} {:.3} {:.2}",
            m.side.as_str(),
            m.width,
            m.contrast
        );
    }

    let result = judge_ring(&measures, variant.case_name());
    let _ = writeln!(out, "{}", line(&result));
    let _ = out.flush();

    Ok(result.passed)
}

fn parse_args(
    variant_arg: Option<String>,
    ring_line_arg: Option<String>,
    extra: Option<String>,
) -> Option<(RingVariant, RingGeometry)> {
    let (Some(v_str), Some(line_str), None) = (variant_arg, ring_line_arg, extra) else {
        return None;
    };
    let variant = RingVariant::from_str(&v_str).ok()?;
    let geom = parse_ring_line(&line_str).ok()?;
    Some((variant, geom))
}

fn main() -> ExitCode {
    let mut cli_args = std::env::args();
    let _bin = cli_args.next();
    let variant_arg = cli_args.next();
    let ring_line_arg = cli_args.next();
    let extra = cli_args.next();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let Some((variant, geom)) = parse_args(variant_arg, ring_line_arg, extra) else {
        let _ = writeln!(out, "{USAGE}");
        return ExitCode::from(64);
    };

    finish_readback(
        async_io::block_on(run_readback(&geom, variant, &mut out)),
        out,
    )
}
