// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `aegis-atspi-readback`: standalone AT-SPI tree readback and verification client (M28 E28-3).
//!
//! Connects to the accessibility D-Bus discovered from `DBUS_SESSION_BUS_ADDRESS`, locates
//! the shell fixture application, reads its entire accessibility tree, and evaluates the
//! five acceptance rules. In interactive mode (default), it additionally triggers the button's
//! click action and verifies incremental tree updates.
//!
//! Exit codes:
//! - 0: all evaluated cases passed
//! - 1: one or more CASE rules failed
//! - 3: a D-Bus transport, deadline, or protocol error occurred (`ERROR <msg>`)
//! - 64: invalid command-line invocation
//!
//! Note (D79): this binary provides development evidence for the client half only;
//! validating the full AT-SPI2 bridge of a real desktop session is deferred to milestone M29.

use std::io::Write;
use std::process::ExitCode;

#[path = "common/mod.rs"]
mod common;

use common::finish_readback;

use aegis_forum_shell_gpui::fixture::BUTTON_LABEL;
use aegis_forum_shell_gpui::judge::{judge_tree, line};
use aegis_forum_shell_gpui::readback::{
    APP_DEADLINE, ReadError, UPDATE_DEADLINE, a11y_connection, click_and_await_update,
    find_fixture, find_node_by_name, read_tree,
};

async fn run_readback(read_only: bool, mut out: impl Write) -> Result<bool, ReadError> {
    let conn = a11y_connection().await?;
    let app = find_fixture(&conn, APP_DEADLINE).await?;
    let nodes = read_tree(&conn, &app).await?;

    let _ = writeln!(out, "NODES {}", nodes.len());

    let results = judge_tree(&nodes);
    let mut all_passed = true;
    for res in &results {
        let _ = writeln!(out, "{}", line(res));
        if !res.passed {
            all_passed = false;
        }
    }

    if !read_only {
        let button_ref = find_node_by_name(&conn, &app, BUTTON_LABEL).await?;
        let update_res = click_and_await_update(&conn, &button_ref, UPDATE_DEADLINE).await?;
        let _ = writeln!(out, "{}", line(&update_res));
        if !update_res.passed {
            all_passed = false;
        }
    }

    let _ = out.flush();
    Ok(all_passed)
}

fn main() -> ExitCode {
    let mut cli_args = std::env::args();
    let _bin = cli_args.next();
    let first = cli_args.next();
    let second = cli_args.next();

    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let read_only = match (first.as_deref(), second.as_deref()) {
        (None, None) => false,
        (Some("--read-only"), None) => true,
        _ => {
            let _ = writeln!(out, "usage: aegis-atspi-readback [--read-only]");
            return ExitCode::from(64);
        }
    };

    finish_readback(async_io::block_on(run_readback(read_only, &mut out)), out)
}
