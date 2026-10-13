// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! A crate's built binary, run once under a deadline, as a transcript.
//!
//! One file, included by path into the CLI test of every crate that ships a
//! binary (`#[path = "../../binary_transcript.rs"]`), so the tests start the
//! artifact Cargo built (`env!("CARGO_BIN_EXE_<name>")`) the same way. It is
//! not a crate, for the reason `dependency_closure.rs` gives.
//!
//! The binary runs with an empty environment plus the pairs the test names,
//! stdin closed, and stdout and stderr written to files under
//! `CARGO_TARGET_TMPDIR` so no pipe can fill and stall it. A binary that
//! outlives its deadline is killed and the run fails (HISS-02).

#![allow(dead_code)]

use std::path::{Path, PathBuf};
use std::process::{Child, Command, ExitStatus, Stdio};
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

/// How long one binary run may take.
pub const DEADLINE: Duration = Duration::from_secs(60);

/// How often the deadline loop polls the child.
const POLL: Duration = Duration::from_millis(10);

/// The most polls one wait makes: 60 s / 10 ms.
const MAX_POLLS: u32 = 6_000;

/// Numbers each run within one test binary, so concurrent runs never share
/// an output file.
static RUNS: AtomicU32 = AtomicU32::new(0);

/// What one run of the binary left behind.
#[derive(Debug)]
pub struct Transcript {
    /// The exit code; `None` when a signal ended the binary.
    pub code: Option<i32>,
    /// Everything written to stdout.
    pub stdout: String,
    /// Everything written to stderr.
    pub stderr: String,
}

impl Transcript {
    /// The last line written to stdout, or an empty string.
    #[must_use]
    pub fn last_line(&self) -> &str {
        self.stdout.lines().last().unwrap_or_default()
    }
}

/// A directory under `CARGO_TARGET_TMPDIR` no other run of this test binary
/// uses, created empty.
///
/// # Errors
///
/// Returns why the directory could not be created.
pub fn scratch(label: &str) -> Result<PathBuf, String> {
    let run = RUNS.fetch_add(1, Ordering::Relaxed);
    let dir = PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(format!(
        "{}-{}-{label}-{run}",
        env!("CARGO_PKG_NAME"),
        std::process::id()
    ));
    if dir.exists() {
        std::fs::remove_dir_all(&dir).map_err(|error| error.to_string())?;
    }
    std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
    Ok(dir)
}

/// Waits for `child` under `deadline`, killing it past the deadline.
fn wait(child: &mut Child, deadline: Duration) -> Result<ExitStatus, String> {
    let started = Instant::now();
    for _ in 0..=MAX_POLLS {
        match child.try_wait() {
            Ok(Some(status)) => return Ok(status),
            Ok(None) if started.elapsed() < deadline => std::thread::sleep(POLL),
            Ok(None) => break,
            Err(error) => return Err(format!("waiting for the binary failed: {error}")),
        }
    }
    let killed = child.kill().and_then(|()| child.wait());
    Err(format!(
        "the binary missed its {deadline:?} deadline; killed: {killed:?}"
    ))
}

/// Runs `binary` with `args` and only the environment `env` names, in a
/// fresh scratch directory.
///
/// # Errors
///
/// Fails closed on a spawn error, a missed deadline and unreadable output.
pub fn run(binary: &str, args: &[&str], env: &[(&str, &str)]) -> Result<Transcript, String> {
    run_in(&scratch("transcript")?, binary, args, env)
}

/// Runs `binary` as [`run`] does, in `dir`, which also takes the transcript
/// files: a test passes paths relative to `dir` when their length matters.
///
/// # Errors
///
/// Fails closed on a spawn error, a missed deadline and unreadable output.
pub fn run_in(
    dir: &Path,
    binary: &str,
    args: &[&str],
    env: &[(&str, &str)],
) -> Result<Transcript, String> {
    let (out, err) = (dir.join("stdout"), dir.join("stderr"));
    let files = (std::fs::File::create(&out), std::fs::File::create(&err));
    let (Ok(stdout), Ok(stderr)) = files else {
        return Err("cannot create the transcript files".to_owned());
    };
    let mut child = Command::new(binary)
        .args(args)
        .env_clear()
        .envs(env.iter().copied())
        .current_dir(dir)
        .stdin(Stdio::null())
        .stdout(stdout)
        .stderr(stderr)
        .spawn()
        .map_err(|error| format!("{binary} could not be spawned: {error}"))?;
    let status = wait(&mut child, DEADLINE)?;
    let read = |path: &PathBuf| std::fs::read_to_string(path).map_err(|error| error.to_string());
    Ok(Transcript {
        code: status.code(),
        stdout: read(&out)?,
        stderr: read(&err)?,
    })
}
