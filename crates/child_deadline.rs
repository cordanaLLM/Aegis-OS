// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Shared child process deadline waiter for path-included test helpers.
//!
//! One file, included by path into path-included test helpers such as
//! `binary_transcript.rs` and `dependency_closure.rs`. It is not a crate,
//! preserving non-crate status for its includers so test helpers do not
//! enter workspace dependency closures (D78).
//!
//! Waits for a spawned child process under an explicit deadline, polling at
//! a fixed interval with a scalar upper bound on iterations (HISS-02). A child
//! that outlives its deadline is killed and the wait fails closed.

#![allow(dead_code)]

use std::process::{Child, ExitStatus};
use std::time::{Duration, Instant};

/// How often the deadline loop polls the child.
pub const POLL: Duration = Duration::from_millis(10);

/// The upper bound on polling iterations: 120 s / 10 ms = `12_000`.
pub const MAX_POLLS: u32 = 12_000;

/// Waits for `child` under `deadline`, killing it past the deadline.
///
/// Returns the exit status on success, or an error formatted with `what` if
/// polling fails or the child misses its deadline.
///
/// # Errors
///
/// Returns an error if `child.try_wait()` fails, or if `deadline` is exceeded.
pub fn wait(child: &mut Child, deadline: Duration, what: &str) -> Result<ExitStatus, String> {
    let started = Instant::now();
    for _ in 0..=MAX_POLLS {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < deadline => std::thread::sleep(POLL),
            Ok(None) => break,
            Err(error) => return Err(format!("waiting for {what} failed: {error}")),
        }
    }
    let killed = child.kill().and_then(|()| child.wait());
    Err(format!(
        "{what} missed its {deadline:?} deadline; killed: {killed:?}"
    ))
}
