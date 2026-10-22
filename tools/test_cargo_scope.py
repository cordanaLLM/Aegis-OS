#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative, and boundary tests for cargo_scope.py.

Verifies:
- Pressure-based job choice across boundary cases (29.45->4, 22.00->8, 25.00->8,
  25.50->4, 25.01->4, 0->8, 99.9->4, garbage/missing->exit 2).
- Dry-run argv structure and options.
- Link thread argument injection and flock serialization.
- Host prerequisite checks and platform skips.
"""

import io
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

TOOLS_DIR = pathlib.Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import cargo_scope  # noqa: E402

CARGO_SCOPE_PATH = TOOLS_DIR / "cargo_scope.py"
MAX_CONCURRENCY_LINES = 128

FAKE_CC_SCRIPT = """#!/usr/bin/env python3
import os
import sys
import time

log_path = os.environ.get("FAKELOG")
if log_path:
    with open(log_path, "a", encoding="utf-8") as f:
        f.write(f"{time.time():.4f} start {' '.join(sys.argv[1:])}\\n")
time.sleep(1.0)
if log_path:
    with open(log_path, "a", encoding="utf-8") as f:
        f.write(f"{time.time():.4f} end\\n")
"""


def _write_pressure_file(path, avg60_val):
    """Write mock pressure content with specified avg60 value."""
    content = (
        f"some avg10=1.00 avg60=1.00 avg300=1.00 total=1\n"
        f"full avg10=50.00 avg60={avg60_val} avg300=1.00 total=1\n"
    )
    pathlib.Path(path).write_text(content, encoding="utf-8")


def _calculate_max_concurrency(log_path):
    """Compute maximum overlapping active executions from timestamped log."""
    events = []
    with open(log_path, "r", encoding="utf-8") as f:
        lines = f.read().splitlines()[:MAX_CONCURRENCY_LINES]
    for line in lines:
        parts = line.strip().split()
        if len(parts) >= 2:
            ts = float(parts[0])
            action = parts[1]
            events.append((ts, 0 if action == "start" else 1, action))
    events.sort()
    current = 0
    max_concurrent = 0
    for _, _, action in events:
        if action == "start":
            current += 1
            if current > max_concurrent:
                max_concurrent = current
        elif action == "end":
            current -= 1
    return max_concurrent


class JobChoiceTests(unittest.TestCase):
    """Positive, negative, and boundary tests for pressure-based job choice."""

    def test_job_choice_positive_boundary(self):
        """Positive and boundary cases: verify exact job counts for avg60 values."""
        cases = [
            ("29.45", 4),
            ("22.00", 8),
            ("25.00", 8),
            ("25.50", 4),
            ("25.01", 4),
            ("0.00", 8),
            ("0", 8),
            ("99.90", 4),
            ("99.9", 4),
        ]
        with tempfile.TemporaryDirectory(prefix="aegis-scope-test-") as temp_dir:
            p_file = pathlib.Path(temp_dir) / "pressure"
            for avg60_str, want_jobs in cases:
                with self.subTest(avg60=avg60_str, want=want_jobs):
                    _write_pressure_file(p_file, avg60_str)
                    val, raw = cargo_scope.read_io_full_avg60(str(p_file))
                    self.assertEqual(raw, avg60_str)
                    self.assertEqual(cargo_scope.determine_jobs(val), want_jobs)

                    out = io.StringIO()
                    with mock.patch("sys.stdout", out), mock.patch.dict(
                        os.environ, {"CARGO_SCOPE_PRESSURE_FILE": str(p_file)}
                    ):
                        rc = cargo_scope.main(["run", "--dry-run", "--", "cargo", "check"])
                    self.assertEqual(rc, 0)
                    line = out.getvalue().splitlines()[0]
                    self.assertIn(f"jobs={want_jobs}", line)
                    self.assertIn(f"io_full_avg60={avg60_str}", line)

    def test_garbage_pressure_exits_2(self):
        """Negative case: non-numeric avg60 causes run to exit 2 with reason."""
        with tempfile.TemporaryDirectory(prefix="aegis-scope-test-") as temp_dir:
            p_file = pathlib.Path(temp_dir) / "pressure_bad"
            p_file.write_text("full avg10=1.00 avg60=garbage total=1\n", encoding="utf-8")
            err = io.StringIO()
            with mock.patch("sys.stderr", err), mock.patch.dict(
                os.environ, {"CARGO_SCOPE_PRESSURE_FILE": str(p_file)}
            ):
                rc = cargo_scope.main(["run", "--dry-run", "--", "cargo", "check"])
            self.assertEqual(rc, 2)
            self.assertIn("invalid avg60 float", err.getvalue())

    def test_missing_pressure_file_exits_2(self):
        """Negative case: missing pressure file causes run to exit 2."""
        err = io.StringIO()
        with mock.patch("sys.stderr", err), mock.patch.dict(
            os.environ, {"CARGO_SCOPE_PRESSURE_FILE": "/nonexistent/pressure/path"}
        ):
            rc = cargo_scope.main(["run", "--dry-run", "--", "cargo", "check"])
        self.assertEqual(rc, 2)
        self.assertIn("failed to read pressure source", err.getvalue())

    def test_missing_full_line_exits_2(self):
        """Boundary case: pressure file with only some line causes run to exit 2."""
        with tempfile.TemporaryDirectory(prefix="aegis-scope-test-") as temp_dir:
            p_file = pathlib.Path(temp_dir) / "pressure_no_full"
            p_file.write_text("some avg10=1.00 avg60=1.00 total=1\n", encoding="utf-8")
            err = io.StringIO()
            with mock.patch("sys.stderr", err), mock.patch.dict(
                os.environ, {"CARGO_SCOPE_PRESSURE_FILE": str(p_file)}
            ):
                rc = cargo_scope.main(["run", "--dry-run", "--", "cargo", "check"])
            self.assertEqual(rc, 2)
            self.assertIn("no io full avg60", err.getvalue())


class DryRunArgvTests(unittest.TestCase):
    """Verify systemd-run argument structure and dry-run output."""

    def test_dry_run_argv_contents(self):
        """Positive case: dry-run argv contains all required options and envs."""
        with tempfile.TemporaryDirectory(prefix="aegis-scope-test-") as temp_dir:
            p_file = pathlib.Path(temp_dir) / "pressure"
            _write_pressure_file(p_file, "22.00")
            out = io.StringIO()
            with mock.patch("sys.stdout", out), mock.patch.dict(
                os.environ, {"CARGO_SCOPE_PRESSURE_FILE": str(p_file)}
            ):
                rc = cargo_scope.main(["run", "--dry-run", "--", "cargo", "build", "--locked"])
            self.assertEqual(rc, 0)
            lines = out.getvalue().splitlines()
            self.assertGreaterEqual(len(lines), 2)
            argv_line = lines[1]
            self.assertIn("IOWeight=50", argv_line)
            self.assertIn("MemoryHigh=16G", argv_line)
            self.assertIn("--expand-environment=no", argv_line)
            self.assertIn("--setenv=CARGO_BUILD_JOBS=8", argv_line)
            self.assertIn("--setenv=RUST_TEST_THREADS=8", argv_line)
            self.assertIn("--setenv=AEGIS_LINK_THREADS=8", argv_line)
            self.assertIn("--setenv=CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=", argv_line)
            self.assertIn("cargo build --locked", argv_line)

    def test_dry_run_argv_builder(self):
        """Positive case: verify build_run_argv builds correct Python list."""
        argv = cargo_scope.build_run_argv(4, "/mock/linker", ["cargo", "test"])
        self.assertIsInstance(argv, list)
        self.assertIn("systemd-run", argv)
        self.assertIn("MemoryHigh=16G", argv)
        self.assertIn("IOWeight=50", argv)
        self.assertIn("--setenv=CARGO_BUILD_JOBS=4", argv)
        self.assertIn(
            "--setenv=CARGO_TARGET_X86_64_UNKNOWN_LINUX_GNU_LINKER=/mock/linker",
            argv,
        )
        self.assertEqual(argv[-2:], ["cargo", "test"])

    def test_dry_run_flag_before_run(self):
        """Boundary case: --dry-run before run subcommand is accepted."""
        with tempfile.TemporaryDirectory(prefix="aegis-scope-test-") as temp_dir:
            p_file = pathlib.Path(temp_dir) / "pressure"
            _write_pressure_file(p_file, "29.45")
            out = io.StringIO()
            with mock.patch("sys.stdout", out), mock.patch.dict(
                os.environ, {"CARGO_SCOPE_PRESSURE_FILE": str(p_file)}
            ):
                rc = cargo_scope.main(["--dry-run", "run", "--", "cargo", "test"])
            self.assertEqual(rc, 0)
            self.assertIn("jobs=4", out.getvalue())


class EnvironmentCheckTests(unittest.TestCase):
    """Verify failures when host prerequisites are missing."""

    def test_non_linux_host_exits_2(self):
        """Negative case: non-Linux platform fails run with exit 2 and printed reason."""
        with tempfile.TemporaryDirectory(prefix="aegis-scope-test-") as temp_dir:
            p_file = pathlib.Path(temp_dir) / "pressure"
            _write_pressure_file(p_file, "22.00")
            err = io.StringIO()
            out = io.StringIO()
            with mock.patch("sys.platform", "win32"), mock.patch("sys.stdout", out), mock.patch(
                "sys.stderr", err
            ), mock.patch.dict(os.environ, {"CARGO_SCOPE_PRESSURE_FILE": str(p_file)}):
                rc = cargo_scope.main(["run", "--", "cargo", "test"])
            self.assertEqual(rc, 2)
            self.assertIn("non-Linux host", err.getvalue())

    def test_missing_systemd_run_exits_2(self):
        """Negative case: missing systemd-run on PATH fails run with exit 2."""
        with tempfile.TemporaryDirectory(prefix="aegis-scope-test-") as temp_dir:
            p_file = pathlib.Path(temp_dir) / "pressure"
            _write_pressure_file(p_file, "22.00")
            err = io.StringIO()
            out = io.StringIO()
            with mock.patch("sys.platform", "linux"), mock.patch("sys.stdout", out), mock.patch(
                "shutil.which", return_value=None
            ), mock.patch("sys.stderr", err), mock.patch.dict(
                os.environ, {"CARGO_SCOPE_PRESSURE_FILE": str(p_file)}
            ):
                rc = cargo_scope.main(["run", "--", "cargo", "test"])
            self.assertEqual(rc, 2)
            self.assertIn("systemd-run not found on PATH", err.getvalue())

    def test_missing_command_exits_2(self):
        """Negative case: run without command fails with exit 2."""
        err = io.StringIO()
        with mock.patch("sys.stderr", err):
            rc = cargo_scope.main(["run"])
        self.assertEqual(rc, 2)
        self.assertIn("no command specified", err.getvalue())


class LinkTests(unittest.TestCase):
    """Verify linker wrapper behavior, thread argument injection, and flock."""

    def setUp(self):
        """Create temporary environment and fake cc executable."""
        self.temp_dir = tempfile.TemporaryDirectory(prefix="aegis-link-test-")
        self.fake_cc = pathlib.Path(self.temp_dir.name) / "fakecc.py"
        self.fake_cc.write_text(FAKE_CC_SCRIPT, encoding="utf-8")
        self.fake_cc.chmod(0o755)
        if sys.platform == "win32":
            cmd_file = pathlib.Path(self.temp_dir.name) / "fakecc.cmd"
            cmd_file.write_text(f'@"{sys.executable}" "{self.fake_cc}" %*\n', encoding="utf-8")
            self.fake_cc = cmd_file

    def tearDown(self):
        """Clean up temporary test resources."""
        self.temp_dir.cleanup()

    def test_thread_arg_appended(self):
        """Positive case: -Wl,--threads is appended to compiler arguments."""
        log_file = pathlib.Path(self.temp_dir.name) / "fakecc.log"
        p_file = pathlib.Path(self.temp_dir.name) / "pressure"
        _write_pressure_file(p_file, "10.00")

        with mock.patch.dict(
            os.environ,
            {
                "FAKELOG": str(log_file),
                "CARGO_SCOPE_CC": str(self.fake_cc),
                "CARGO_SCOPE_PRESSURE_FILE": str(p_file),
                "AEGIS_LINK_THREADS": "4",
            },
        ):
            rc = cargo_scope.main(["link", "-o", "out_binary"])
        self.assertEqual(rc, 0)
        content = log_file.read_text(encoding="utf-8")
        self.assertIn("-Wl,--threads=4", content)

        # Boundary case: AEGIS_LINK_THREADS override
        with mock.patch.dict(
            os.environ,
            {
                "FAKELOG": str(log_file),
                "CARGO_SCOPE_CC": str(self.fake_cc),
                "CARGO_SCOPE_PRESSURE_FILE": str(p_file),
                "AEGIS_LINK_THREADS": "8",
            },
        ):
            rc = cargo_scope.main(["link", "-o", "out_binary_2"])
        self.assertEqual(rc, 0)
        content = log_file.read_text(encoding="utf-8")
        self.assertIn("-Wl,--threads=8", content)

    def test_unreadable_pressure_serializes_and_silent_stdout(self):
        """Boundary case: unreadable pressure serializes and writes zero stdout."""
        log_file = pathlib.Path(self.temp_dir.name) / "fakecc.log"
        out = io.StringIO()
        with mock.patch("sys.stdout", out), mock.patch.dict(
            os.environ,
            {
                "FAKELOG": str(log_file),
                "CARGO_SCOPE_CC": str(self.fake_cc),
                "CARGO_SCOPE_PRESSURE_FILE": "/missing/pressure",
            },
        ):
            rc = cargo_scope.main(["link", "-o", "out_binary"])
        self.assertEqual(rc, 0)
        self.assertEqual(out.getvalue(), "")

    def _run_concurrent_links(self, pressure_file, log_path, lock_path):
        """Spawn 3 concurrent link processes and return measured concurrency."""
        procs = []
        env = os.environ.copy()
        env["FAKELOG"] = str(log_path)
        env["CARGO_SCOPE_CC"] = str(self.fake_cc)
        env["CARGO_SCOPE_LINK_LOCK"] = str(lock_path)
        env["CARGO_SCOPE_PRESSURE_FILE"] = str(pressure_file)
        env["AEGIS_LINK_THREADS"] = "4"
        for i in range(1, 4):
            proc = subprocess.Popen(
                [sys.executable, str(CARGO_SCOPE_PATH), "link", f"-o=out{i}"],
                env=env,
                stdout=subprocess.PIPE,
                stderr=subprocess.PIPE,
            )
            procs.append(proc)
        for proc in procs:
            proc.communicate(timeout=15.0)
            self.assertEqual(proc.returncode, 0)
        return _calculate_max_concurrency(log_path)

    def test_concurrent_links_flock_concurrency(self):
        """Positive and boundary concurrency: max parallel 1 at 40 and 25.5, 3 at 10."""
        if cargo_scope.fcntl is None:
            self.skipTest(
                "fcntl is POSIX-only and absent on this host; link flock tests run on Linux"
            )

        p_file = pathlib.Path(self.temp_dir.name) / "pressure"
        lock_file = pathlib.Path(self.temp_dir.name) / "link.lock"
        log_file = pathlib.Path(self.temp_dir.name) / "concurrency.log"

        # High pressure (40.0) -> max parallel 1
        _write_pressure_file(p_file, "40.00")
        if log_file.exists():
            log_file.unlink()
        m40 = self._run_concurrent_links(p_file, log_file, lock_file)
        self.assertEqual(m40, 1)

        # Boundary high pressure (25.50) -> max parallel 1
        _write_pressure_file(p_file, "25.50")
        if log_file.exists():
            log_file.unlink()
        m25 = self._run_concurrent_links(p_file, log_file, lock_file)
        self.assertEqual(m25, 1)

        # Low pressure (10.00) -> max parallel 3
        _write_pressure_file(p_file, "10.00")
        if log_file.exists():
            log_file.unlink()
        m10 = self._run_concurrent_links(p_file, log_file, lock_file)
        self.assertEqual(m10, 3)

    def test_link_lock_timeout(self):
        """Negative case: link lock acquisition timeout exits with code 2."""
        if cargo_scope.fcntl is None:
            self.skipTest(
                "fcntl is POSIX-only and absent on this host; link flock tests run on Linux"
            )

        p_file = pathlib.Path(self.temp_dir.name) / "pressure"
        _write_pressure_file(p_file, "40.00")
        lock_file = pathlib.Path(self.temp_dir.name) / "timeout.lock"

        # Hold exclusive lock from parent
        with open(lock_file, "a", encoding="utf-8") as held:
            cargo_scope.fcntl.flock(
                held.fileno(), cargo_scope.fcntl.LOCK_EX | cargo_scope.fcntl.LOCK_NB
            )
            err = io.StringIO()
            with mock.patch("sys.stderr", err), mock.patch.dict(
                os.environ,
                {
                    "CARGO_SCOPE_CC": str(self.fake_cc),
                    "CARGO_SCOPE_PRESSURE_FILE": str(p_file),
                    "CARGO_SCOPE_LINK_LOCK": str(lock_file),
                    "CARGO_SCOPE_LINK_TIMEOUT": "0.1",
                },
            ):
                rc = cargo_scope.main(["link", "-o", "out_blocked"])
            self.assertEqual(rc, 2)
            self.assertIn("timed out waiting for link lock", err.getvalue())


@unittest.skipUnless(sys.platform.startswith("linux"), "run mode starts systemd-run on Linux only")
class RunExecTests(unittest.TestCase):
    """The job choice line must survive the exec into systemd-run."""

    def test_scope_line_reaches_piped_stdout(self):
        """Positive case: with stdout piped, the line is written before exec."""
        with tempfile.TemporaryDirectory(prefix="aegis-scope-exec-") as temp_dir:
            fake = pathlib.Path(temp_dir) / "systemd-run"
            fake.write_text("#!/bin/sh\nexit 0\n", encoding="utf-8")
            fake.chmod(0o755)
            pressure = pathlib.Path(temp_dir) / "pressure"
            _write_pressure_file(pressure, "30.00")
            env = dict(os.environ)
            env["PATH"] = f"{temp_dir}{os.pathsep}{env.get('PATH', '')}"
            env["CARGO_SCOPE_PRESSURE_FILE"] = str(pressure)
            script = pathlib.Path(cargo_scope.__file__).resolve()
            res = subprocess.run(
                [sys.executable, str(script), "run", "--", "true"],
                env=env,
                capture_output=True,
                text=True,
                timeout=60,
            )
            self.assertEqual(res.returncode, 0)
            self.assertIn("jobs=4", res.stdout)
            self.assertIn("io_full_avg60=30.00", res.stdout)


def _rustc_link(linker, workdir):
    """Link a hello-world binary with rustc through linker; return (rc, binary)."""
    source = pathlib.Path(workdir) / "hello.rs"
    source.write_text("fn main() {}\n", encoding="utf-8")
    binary = pathlib.Path(workdir) / "hello"
    pressure = pathlib.Path(workdir) / "pressure"
    _write_pressure_file(pressure, "1.00")
    env = dict(os.environ)
    env["CARGO_SCOPE_PRESSURE_FILE"] = str(pressure)
    env["AEGIS_LINK_THREADS"] = "2"
    cmd = ["rustc", "-C", f"linker={linker}", str(source), "-o", str(binary)]
    res = subprocess.run(cmd, env=env, capture_output=True, timeout=120)
    return res.returncode, binary


@unittest.skipUnless(sys.platform.startswith("linux"), "rustc link check reads x86_64 Linux ELF")
@unittest.skipIf(shutil.which("rustc") is None, "rustc not on PATH; real link check skipped")
class RealRustcLinkTests(unittest.TestCase):
    """rustc infers the linker flavor from this file's stem; hold it to rust-lld."""

    def test_rustc_links_through_rust_lld(self):
        """Positive case: the real binary names LLD and the thread cap is accepted."""
        script = pathlib.Path(cargo_scope.__file__).resolve()
        with tempfile.TemporaryDirectory(prefix="aegis-rustc-link-") as temp_dir:
            rc, binary = _rustc_link(script, temp_dir)
            self.assertEqual(rc, 0)
            self.assertIn(b"Linker: LLD", binary.read_bytes())

    def test_gcc_stem_reaches_gnu_ld_and_fails(self):
        """Negative case: a copy whose stem ends in gcc loses rust-lld and fails."""
        script = pathlib.Path(cargo_scope.__file__).resolve()
        with tempfile.TemporaryDirectory(prefix="aegis-rustc-link-") as temp_dir:
            renamed = pathlib.Path(temp_dir) / "cargo-scope-gcc"
            shutil.copy(script, renamed)
            shutil.copy(script.parent / "host.py", pathlib.Path(temp_dir) / "host.py")
            renamed.chmod(0o755)
            rc, _ = _rustc_link(renamed, temp_dir)
            self.assertNotEqual(rc, 0)


if __name__ == "__main__":
    unittest.main()
