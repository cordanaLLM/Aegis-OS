#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Wrap local cargo builds and link invocations under host I/O pressure.

``run`` starts a command in a memory-capped, I/O-weighted systemd user scope
with cargo's build jobs chosen from /proc/pressure/io full avg60, and names
this file as cargo's linker. rustc then calls this file with the cc driver's
arguments: it caps the linker threads and, while the host is under I/O
pressure, links one binary at a time.

The file stem must not name a linker to rustc, which infers the linker
flavor from it: a stem of ``link`` or ending in ``ld`` gets bare ``ld``
arguments the cc driver refuses, and one ending in ``gcc`` gets the cc flavor
without ``-fuse-ld=lld``, so GNU ld runs and refuses ``--threads``. With this
file's stem rustc keeps the target default, which on x86_64 Linux links
through its self-contained rust-lld. tools/test_cargo_scope.py links a real
binary to hold this.
"""

import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import time

try:
    import fcntl
except ImportError:
    fcntl = None

# Ensure tools directory is on sys.path for host helpers.
TOOLS_DIR = pathlib.Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))
import host  # noqa: E402

DEFAULT_PRESSURE_FILE = "/proc/pressure/io"
HIGH_PRESSURE_THRESHOLD = 25.0
HIGH_PRESSURE_JOBS = 4
NORMAL_JOBS = 8
DEFAULT_LINK_THREADS = "4"
DEFAULT_LINK_TIMEOUT = 1800.0
LOCK_POLL_INTERVAL = 0.05
MAX_PRESSURE_LINES = 64
MAX_PRESSURE_TOKENS = 64
MAX_ARGS = 4096


def _extract_avg60_token(parts):
    """Find avg60 token in parts and return (float_val, raw_val) or None."""
    for token in parts[1:]:
        if token.startswith("avg60="):
            raw_val = token.split("=", 1)[1]
            return float(raw_val), raw_val
    return None


def read_io_full_avg60(path=None):
    """Read the full line's avg60 field from /proc/pressure/io or override.

    Returns (avg60_float, avg60_str) on success.
    Raises ValueError with a descriptive reason on any failure.
    """
    source = path or os.environ.get("CARGO_SCOPE_PRESSURE_FILE") or DEFAULT_PRESSURE_FILE

    try:
        with open(source, "r", encoding="utf-8") as handle:
            content = handle.read(4096)
    except OSError as err:
        raise ValueError(f"failed to read pressure source {source}: {err}") from err

    for line in content.splitlines()[:MAX_PRESSURE_LINES]:
        parts = line.strip().split()[:MAX_PRESSURE_TOKENS]
        if parts and parts[0] == "full":
            try:
                res = _extract_avg60_token(parts)
                if res is not None:
                    return res
            except ValueError as err:
                raise ValueError(f"invalid avg60 float in {source}") from err

    raise ValueError(f"no io full avg60 in {source}")


def determine_jobs(avg60):
    """Return 4 jobs when avg60 > 25.0, else 8."""
    return HIGH_PRESSURE_JOBS if avg60 > HIGH_PRESSURE_THRESHOLD else NORMAL_JOBS


def build_run_argv(jobs, linker_path, cmd):
    """Construct the systemd-run invocation argument list."""
    return [
        "systemd-run",
        "--user",
        "--scope",
        "--quiet",
        "--expand-environment=no",
        "-p",
        "MemoryHigh=16G",
        "-p",
        "IOWeight=50",
        f"--setenv=CARGO_BUILD_JOBS={jobs}",
        f"--setenv=RUST_TEST_THREADS={jobs}",
        f"--setenv=AEGIS_LINK_THREADS={jobs}",
        f"--setenv=CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER={linker_path}",
        *cmd,
    ]


def _parse_run_options(argv):
    """Extract dry-run flag and command from run arguments."""
    dry_run = False
    cmd = []
    idx = 0
    while idx < len(argv) and idx < MAX_ARGS:
        arg = argv[idx]
        if arg == "--dry-run":
            dry_run = True
            idx += 1
        elif arg == "--":
            cmd = argv[idx + 1 :]
            break
        else:
            cmd = argv[idx:]
            break
    return dry_run, cmd


def _check_run_prerequisites():
    """Verify that systemd-run is available on Linux."""
    if not sys.platform.startswith("linux"):
        sys.stderr.write("cargo-scope: non-Linux host; systemd-run is Linux-only\n")
        return False
    if shutil.which("systemd-run") is None:
        sys.stderr.write("cargo-scope: systemd-run not found on PATH\n")
        return False
    return True


def cmd_run(argv):
    """Execute the run subcommand, launching cmd under systemd-run."""
    dry_run, cmd = _parse_run_options(argv)
    if not cmd:
        sys.stderr.write("cargo-scope: no command specified to run\n")
        return 2

    try:
        avg60_val, avg60_str = read_io_full_avg60()
    except ValueError as err:
        sys.stderr.write(f"cargo-scope: {err}\n")
        return 2

    jobs = determine_jobs(avg60_val)
    line = (
        f"cargo-scope: systemd-run --user --scope --expand-environment=no "
        f"-p MemoryHigh=16G -p IOWeight=50 jobs={jobs} io_full_avg60={avg60_str}"
    )
    # Flushed, because execvp below replaces the process and drops the buffer.
    print(line, flush=True)

    linker_path = host.target(pathlib.Path(__file__).resolve())
    run_argv = build_run_argv(jobs, linker_path, cmd)

    if dry_run:
        print(" ".join(run_argv))
        return 0

    if not _check_run_prerequisites():
        return 2

    try:
        os.execvp("systemd-run", run_argv)
    except OSError as err:
        sys.stderr.write(f"cargo-scope: execvp failed: {err}\n")
        return 2
    return 0


def get_link_lock_path():
    """Return the resolved lock file path for link serialization."""
    override = os.environ.get("CARGO_SCOPE_LINK_LOCK")
    if override:
        return override
    runtime_dir = os.environ.get("XDG_RUNTIME_DIR")
    if runtime_dir and os.path.isdir(runtime_dir):
        return os.path.join(runtime_dir, "cargo-scope-link.lock")
    target_dir = pathlib.Path.cwd() / "target"
    if target_dir.is_dir():
        return str(target_dir / "cargo-scope-link.lock")
    return os.path.join(tempfile.gettempdir(), "cargo-scope-link.lock")


def acquire_link_lock(lock_file, timeout):
    """Wait up to timeout seconds to acquire exclusive flock on lock_file."""
    if fcntl is None:
        sys.stderr.write("cargo-scope: fcntl is absent on this host\n")
        return False
    max_attempts = int(timeout / LOCK_POLL_INTERVAL) + 1
    deadline = time.monotonic() + timeout
    for _ in range(max_attempts):
        if time.monotonic() >= deadline:
            break
        try:
            fcntl.flock(lock_file.fileno(), fcntl.LOCK_EX | fcntl.LOCK_NB)
            return True
        except (BlockingIOError, OSError):
            time.sleep(LOCK_POLL_INTERVAL)
    return False


def run_compiler(cmd, timeout):
    """Execute compiler command with timeout and return its exit code."""
    try:
        res = subprocess.run(cmd, timeout=timeout)
        return res.returncode
    except subprocess.TimeoutExpired:
        sys.stderr.write(f"cargo-scope: cc timed out after {timeout}s\n")
        return 2
    except OSError as err:
        sys.stderr.write(f"cargo-scope: failed to execute cc: {err}\n")
        return 2


def cmd_link(argv):
    """Execute the link subcommand, serializing with flock when under pressure."""
    cc = os.environ.get("CARGO_SCOPE_CC") or "cc"
    threads = os.environ.get("AEGIS_LINK_THREADS", DEFAULT_LINK_THREADS)
    timeout = float(os.environ.get("CARGO_SCOPE_LINK_TIMEOUT", DEFAULT_LINK_TIMEOUT))

    try:
        avg60_val, _ = read_io_full_avg60()
        should_serialize = avg60_val > HIGH_PRESSURE_THRESHOLD
    except ValueError:
        # Unreadable pressure in link mode -> treat as pressure, print nothing on stdout
        should_serialize = True

    cmd = [cc, *argv, f"-Wl,--threads={threads}"]
    if not should_serialize:
        return run_compiler(cmd, timeout)

    lock_path = get_link_lock_path()
    os.makedirs(os.path.dirname(os.path.abspath(lock_path)), exist_ok=True)
    try:
        with open(lock_path, "a", encoding="utf-8") as lock_file:
            if not acquire_link_lock(lock_file, timeout):
                sys.stderr.write(f"cargo-scope: timed out waiting for link lock {lock_path}\n")
                return 2
            return run_compiler(cmd, timeout)
    except OSError as err:
        sys.stderr.write(f"cargo-scope: lock file error on {lock_path}: {err}\n")
        return 2


def main(argv=None):
    """CLI dispatcher for run and link modes."""
    if argv is None:
        argv = sys.argv[1:]

    if not argv:
        sys.stderr.write("usage: cargo_scope.py run [--dry-run] -- <cmd...> | link <cc args...>\n")
        return 2

    if argv[0] in ("-h", "--help"):
        print("usage: cargo_scope.py run [--dry-run] -- <cmd...> | link <cc args...>")
        return 0

    if argv[0] == "--dry-run" and len(argv) > 1 and argv[1] == "run":
        return cmd_run(["--dry-run"] + argv[2:])

    if argv[0] == "run":
        return cmd_run(argv[1:])

    if argv[0] == "link":
        return cmd_link(argv[1:])

    # rustc calls the linker with cc driver arguments, never "run" or "link".
    return cmd_link(argv)


if __name__ == "__main__":
    sys.exit(main())
