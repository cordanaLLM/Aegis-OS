# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Unit tests for tools/gate_common.py adhering to HISS-15."""

import ast
import io
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

import gate_common
import testkit


class TestCacheDir(unittest.TestCase):
    """Positive, negative and boundary tests for cache_dir."""

    def test_override_env_set(self):
        """Positive: override variable set directs to override directory."""
        env = {"AEGIS_TEST_DIR": "/custom/path/cache"}
        self.assertEqual(
            gate_common.cache_dir("AEGIS_TEST_DIR", "aegis-test", environ=env),
            Path("/custom/path/cache"),
        )

    def test_xdg_cache_home_set(self):
        """Positive: XDG_CACHE_HOME set directs under that cache root."""
        env = {"XDG_CACHE_HOME": "/custom/xdg"}
        self.assertEqual(
            gate_common.cache_dir("AEGIS_TEST_DIR", "aegis-test", environ=env),
            Path("/custom/xdg/aegis-test"),
        )

    def test_neither_set_uses_home_cache(self):
        """Boundary: neither env var set falls back to ~/.cache/<name>."""
        with mock.patch.dict(os.environ, {}, clear=True):
            expected = Path.home() / ".cache" / "aegis-test"
            self.assertEqual(
                gate_common.cache_dir("AEGIS_TEST_DIR", "aegis-test", environ={}),
                expected,
            )

    def test_positive_override_tilde_expands_to_home(self):
        """Positive: override path starting with ~/ expands to user home directory."""
        env = {"AEGIS_TEST_DIR": "~/custom_cache"}
        self.assertEqual(
            gate_common.cache_dir("AEGIS_TEST_DIR", "aegis-test", environ=env),
            Path.home() / "custom_cache",
        )

    def test_negative_empty_override_ignored_falls_back(self):
        """Negative: empty override value is ignored and falls back to cache root."""
        env = {"AEGIS_TEST_DIR": "", "XDG_CACHE_HOME": "/custom/xdg"}
        self.assertEqual(
            gate_common.cache_dir("AEGIS_TEST_DIR", "aegis-test", environ=env),
            Path("/custom/xdg/aegis-test"),
        )

    def test_boundary_environ_none_reads_os_environ(self):
        """Boundary: environ=None reads directly from patched os.environ."""
        with mock.patch.dict(os.environ, {"AEGIS_TEST_DIR": "/patched/env/dir"}):
            self.assertEqual(
                gate_common.cache_dir("AEGIS_TEST_DIR", "aegis-test", environ=None),
                Path("/patched/env/dir"),
            )


class TestFileSha256(unittest.TestCase):
    """Positive, negative and boundary tests for file_sha256."""

    def setUp(self):
        """Create a temporary directory for file hashing tests."""
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_positive_computes_sha256(self):
        """Positive: computes correct sha256 for non-empty file."""
        target = self.root / "file.txt"
        target.write_bytes(b"hello world\n")
        # sha256 of "hello world\n"
        expected = "a948904f2f0f479b8f8197694b30184b0d2ed1c1cd2a1ec0fb85d299a192a447"
        self.assertEqual(gate_common.file_sha256(target), expected)

    def test_negative_missing_file_raises_os_error(self):
        """Negative: non-existent file raises OSError."""
        target = self.root / "nonexistent.txt"
        with self.assertRaises(OSError):
            gate_common.file_sha256(target)

    def test_boundary_empty_file(self):
        """Boundary: empty file hashes to known empty-string sha256."""
        target = self.root / "empty.txt"
        target.write_bytes(b"")
        expected = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855"
        self.assertEqual(gate_common.file_sha256(target), expected)


class TestCLocale(unittest.TestCase):
    """Positive, negative and boundary tests for c_locale."""

    def test_positive_sets_fixed_locale(self):
        """Positive: c_locale sets LC_ALL, LANG and NO_COLOR while keeping other vars."""
        env = {"FOO": "bar"}
        result = gate_common.c_locale(environ=env)
        self.assertEqual(result["FOO"], "bar")
        self.assertEqual(result["LC_ALL"], "C")
        self.assertEqual(result["LANG"], "C")
        self.assertEqual(result["NO_COLOR"], "1")

    def test_negative_empty_environ(self):
        """Negative: empty environ correctly populated with fixed locale vars."""
        result = gate_common.c_locale(environ={})
        self.assertEqual(result, {"LC_ALL": "C", "LANG": "C", "NO_COLOR": "1"})

    def test_boundary_overrides_conflicting_locale(self):
        """Boundary: pre-existing locale variables are overwritten."""
        env = {"LC_ALL": "de_DE.UTF-8", "LANG": "fr_FR.UTF-8", "NO_COLOR": "0"}
        result = gate_common.c_locale(environ=env)
        self.assertEqual(result["LC_ALL"], "C")
        self.assertEqual(result["LANG"], "C")
        self.assertEqual(result["NO_COLOR"], "1")


class TestSignalHandling(unittest.TestCase):
    """Positive, negative and boundary tests for terminated and install_signal_handlers."""

    def test_positive_terminated_raises_system_exit(self):
        """Positive: terminated raises SystemExit with 128 + signum."""
        with self.assertRaises(SystemExit) as context:
            gate_common.terminated(15)
        self.assertEqual(context.exception.code, 143)

    def test_negative_terminated_different_signum(self):
        """Negative: terminated with signal 1 raises SystemExit 129."""
        with self.assertRaises(SystemExit) as context:
            gate_common.terminated(1)
        self.assertEqual(context.exception.code, 129)

    def test_boundary_terminated_zero(self):
        """Boundary: terminated with signal 0 raises SystemExit 128."""
        with self.assertRaises(SystemExit) as context:
            gate_common.terminated(0)
        self.assertEqual(context.exception.code, 128)

    def test_positive_install_signal_handlers(self):
        """Positive: install_signal_handlers installs handler for available signals."""
        with mock.patch("signal.signal") as mock_signal:
            handled = gate_common.install_signal_handlers(signals=("SIGTERM", "SIGHUP"))
            self.assertIn("SIGTERM", handled)
            self.assertEqual(mock_signal.call_count, len(handled))

    def test_negative_install_signal_handlers_absent_signal(self):
        """Negative: signals absent from signal module are skipped."""
        handled = gate_common.install_signal_handlers(signals=("BOGUS_SIGNAL_NOT_EXIST",))
        self.assertEqual(handled, [])

    def test_boundary_install_signal_handlers_empty(self):
        """Boundary: empty signal list returns empty list."""
        self.assertEqual(gate_common.install_signal_handlers(signals=()), [])


class TestReporting(unittest.TestCase):
    """Positive, negative and boundary tests for report and report_definition."""

    def test_positive_report_pass(self):
        """Positive: report prints PASS when problems are empty."""
        buf = io.StringIO()
        with mock.patch("sys.stdout", buf):
            gate_common.report("test-case", [])
        self.assertEqual(buf.getvalue().strip(), "PASS test-case")

    def test_negative_report_fail(self):
        """Negative: report prints FAIL and problem lines."""
        buf = io.StringIO()
        with mock.patch("sys.stdout", buf):
            gate_common.report("test-case", ["error line 1", "error line 2"])
        lines = buf.getvalue().splitlines()
        self.assertEqual(lines[0], "FAIL test-case")
        self.assertIn("error line 1", lines[1])

    def test_boundary_report_max_problems_and_notes(self):
        """Boundary: problem lines beyond max_problem_lines are truncated, notes shown."""
        buf = io.StringIO()
        problems = [f"err {i}" for i in range(10)]
        with mock.patch("sys.stdout", buf):
            gate_common.report("test-case", problems, notes=["note A"], max_problem_lines=3)
        lines = buf.getvalue().splitlines()
        self.assertIn("note A", lines[1])
        # Only 3 problem lines printed
        problem_lines = [item for item in lines if "err" in item]
        self.assertEqual(len(problem_lines), 3)

    def test_positive_report_definition_pass(self):
        """Positive: report_definition prints PASS when problems empty."""
        buf = io.StringIO()
        with mock.patch("sys.stdout", buf):
            gate_common.report_definition({"name": "case1"}, 0, [], "")
        self.assertEqual(buf.getvalue().strip(), "PASS case1: exit 0")

    def test_negative_report_definition_fail(self):
        """Negative: report_definition prints FAIL, problems and diagnostics."""
        buf = io.StringIO()
        with mock.patch("sys.stdout", buf):
            gate_common.report_definition({"name": "case1"}, 1, ["bad key"], "diag 1\ndiag 2")
        lines = buf.getvalue().splitlines()
        self.assertEqual(lines[0], "FAIL case1: exit 1")
        self.assertIn("bad key", lines[1])
        self.assertIn("| diag 1", lines[2])

    def test_boundary_report_definition_diagnostics_capped(self):
        """Boundary: diagnostics beyond max_diagnostic_lines are truncated."""
        buf = io.StringIO()
        diags = "\n".join(f"d{i}" for i in range(20))
        with mock.patch("sys.stdout", buf):
            gate_common.report_definition(
                {"name": "case1"}, 1, ["err"], diags, max_diagnostic_lines=5
            )
        diag_lines = [item for item in buf.getvalue().splitlines() if "|" in item]
        self.assertEqual(len(diag_lines), 5)


class TestTail(unittest.TestCase):
    """Positive, negative and boundary tests for tail."""

    def test_positive_tail_returns_last_lines(self):
        """Positive: returns the last N non-empty lines."""
        text = "line 1\nline 2\nline 3\nline 4\n"
        self.assertEqual(gate_common.tail(text, lines=2), ["line 3", "line 4"])

    def test_negative_tail_skips_blank_lines(self):
        """Negative: blank and whitespace-only lines are omitted."""
        text = "line 1\n   \nline 2\n\n\n"
        self.assertEqual(gate_common.tail(text, lines=2), ["line 1", "line 2"])

    def test_boundary_tail_empty_and_short(self):
        """Boundary: empty string returns []; fewer lines than requested returns all."""
        self.assertEqual(gate_common.tail("", lines=5), [])
        self.assertEqual(gate_common.tail("single\n", lines=5), ["single"])


class TestExecutableFrom(unittest.TestCase):
    """Positive, negative and boundary tests for executable_from."""

    def test_positive_finds_executable(self):
        """Positive: finds executable from matching compiler-artifact message."""
        messages = (
            '{"reason": "build-script-executed"}\n'
            '{"reason": "compiler-artifact", "target": {"name": "target-bin"}, '
            '"executable": "/path/to/target-bin"}\n'
        )
        found = gate_common.executable_from(messages, "target-bin")
        self.assertEqual(found, Path("/path/to/target-bin"))

    def test_negative_no_matching_artifact(self):
        """Negative: returns None when no artifact matches."""
        messages = (
            '{"reason": "compiler-artifact", "target": {"name": "other"}, "executable": "/x"}\n'
        )
        self.assertIsNone(gate_common.executable_from(messages, "target-bin"))

    def test_boundary_malformed_json_and_empty(self):
        """Boundary: malformed JSON lines ignored; empty string returns None."""
        messages = "not-json\n{incomplete\n"
        self.assertIsNone(gate_common.executable_from(messages, "target-bin"))
        self.assertIsNone(gate_common.executable_from("", "target-bin"))


class TestToolchainChecks(unittest.TestCase):
    """Positive, negative and boundary tests for version_tuple and check_toolchain."""

    def test_positive_version_tuple(self):
        """Positive: parses dotted version string into integer tuple."""
        self.assertEqual(gate_common.version_tuple("1.2.3"), (1, 2, 3))

    def test_negative_version_tuple_invalid(self):
        """Negative: non-numeric parts raise ValueError."""
        with self.assertRaises(ValueError):
            gate_common.version_tuple("1.2.beta")

    def test_boundary_version_tuple_single(self):
        """Boundary: single numeric part returns 1-tuple."""
        self.assertEqual(gate_common.version_tuple("42"), (42,))

    def test_positive_check_toolchain_meets_floor(self):
        """Positive: toolchain satisfying floor returns empty list."""
        toolchain = [("tool1", ["tool1"], r"(\d+\.\d+)", "1.0", "1.0")]

        def fake_read(row):
            """Return admitted version meeting floor for test row."""
            return "tool1 1.2", "1.2"

        buf = io.StringIO()
        with mock.patch("sys.stdout", buf):
            reasons = gate_common.check_toolchain(toolchain, fake_read)
        self.assertEqual(reasons, [])

    def test_negative_check_toolchain_below_floor(self):
        """Negative: toolchain below floor returns floor failure."""
        toolchain = [("tool1", ["tool1"], r"(\d+\.\d+)", "2.0", "2.0")]

        def fake_read(row):
            """Return version below floor for test row."""
            return "tool1 1.0", "1.0"

        buf = io.StringIO()
        with mock.patch("sys.stdout", buf):
            reasons = gate_common.check_toolchain(toolchain, fake_read)
        self.assertEqual(len(reasons), 1)
        self.assertIn("below the admitted floor 2.0", reasons[0])

    def test_boundary_check_toolchain_missing_and_none_floor(self):
        """Boundary: missing tool returns banner reason; floor=None skips floor check."""
        toolchain = [
            ("tool1", ["tool1"], r".*", None, "1.0"),
            ("tool2", ["tool2"], r".*", "1.0", "1.0"),
        ]

        def fake_read(row):
            """Return missing toolchain banner for missing tools."""
            if row[0] == "tool1":
                return "tool1 1.0", "1.0"
            return "tool2 is not on PATH", None

        buf = io.StringIO()
        with mock.patch("sys.stdout", buf):
            reasons = gate_common.check_toolchain(toolchain, fake_read)
        self.assertEqual(reasons, ["tool2 is not on PATH"])


class TestRunCommand(unittest.TestCase):
    """Positive, negative and boundary tests for run_command."""

    def test_positive_run_command_success(self):
        """Positive: command executes and returns (code, stdout, stderr)."""
        code, out, err = gate_common.run_command([sys.executable, "-c", "print('hello')"])
        self.assertEqual(code, 0)
        self.assertEqual(out.strip(), "hello")
        self.assertEqual(err, "")

    def test_negative_run_command_nonzero_exit(self):
        """Negative: command exiting non-zero returns non-zero code without raising."""
        code, _out, _err = gate_common.run_command(
            [sys.executable, "-c", "import sys; sys.exit(1)"]
        )
        self.assertEqual(code, 1)

    def test_boundary_run_command_missing_and_timeout(self):
        """Boundary: missing command or timeout raises GateError."""
        with self.assertRaises(gate_common.GateError):
            gate_common.run_command(["/nonexistent_program_path_xyz"])
        with self.assertRaises(gate_common.GateError) as context:
            gate_common.run_command(
                [sys.executable, "-c", "import time; time.sleep(10)"], timeout=0.01
            )
        self.assertIn("exceeded", str(context.exception))


class TestSummary(unittest.TestCase):
    """Positive, negative and boundary tests for summary."""

    def test_positive_summary_all_pass(self):
        """Positive: all cases ran and passed returns RESULT pass."""
        cases = [("case1", True, ""), ("case2", True, "")]
        self.assertEqual(gate_common.summary(cases, 2), "RESULT pass")

    def test_negative_summary_with_failures(self):
        """Negative: failing cases report failure count."""
        cases = [("case1", True, ""), ("case2", False, "failed")]
        self.assertEqual(gate_common.summary(cases, 2), "RESULT fail: 1 case(s) failed")

    def test_boundary_summary_count_mismatch(self):
        """Boundary: fewer cases ran than case_count reports count mismatch."""
        cases = [("case1", True, "")]
        self.assertEqual(gate_common.summary(cases, 3), "RESULT fail: 1 of 3 cases ran")


class TestStop(unittest.TestCase):
    """Positive, negative and boundary tests for stop."""

    def test_positive_stop_completed_process(self):
        """Positive: already completed process is waited without kill."""
        proc = mock.Mock()
        proc.poll.return_value = 0
        proc.wait.return_value = 0
        gate_common.stop(proc)
        proc.kill.assert_not_called()

    def test_negative_stop_end_session_fails(self):
        """Negative: end_session fails, fallback to kill."""
        proc = mock.Mock()
        proc.poll.return_value = None
        proc.pid = 1234
        fake_end = mock.Mock(return_value=(False, "failed"))
        gate_common.stop(proc, end_session_fn=fake_end)
        proc.kill.assert_called_once()

    def test_boundary_stop_timeout_expired(self):
        """Boundary: wait raises TimeoutExpired, kill and wait called."""
        proc = mock.Mock()
        proc.poll.return_value = None
        proc.pid = 1234
        fake_end = mock.Mock(return_value=(True, None))
        proc.wait.side_effect = [subprocess.TimeoutExpired(cmd="test", timeout=1), 0]
        gate_common.stop(proc, end_session_fn=fake_end)
        proc.kill.assert_called_once()
        self.assertEqual(proc.wait.call_count, 2)


class TestUserArguments(unittest.TestCase):
    """Positive, negative and boundary tests for user_arguments."""

    def test_positive_docker_engine(self):
        """Positive: docker engine returns --user uid:gid."""
        args = gate_common.user_arguments(
            "docker", uid_fn=lambda: (1001, None), gid_fn=lambda: (1002, None)
        )
        self.assertEqual(args, ["--user", "1001:1002"])

    def test_negative_non_docker_engine(self):
        """Negative: podman engine returns empty list."""
        args = gate_common.user_arguments("podman")
        self.assertEqual(args, [])

    def test_boundary_docker_missing_uid(self):
        """Boundary: docker engine with None uid returns empty list."""
        args = gate_common.user_arguments(
            "docker", uid_fn=lambda: (None, "unavailable"), gid_fn=lambda: (1000, None)
        )
        self.assertEqual(args, [])


class TestRunId(unittest.TestCase):
    """Positive, negative and boundary tests for run_id."""

    def test_positive_run_id_format(self):
        """Positive: run_id matches regex format."""
        rid = gate_common.run_id()
        self.assertRegex(rid, r"^r\d{8}T\d{6}-[0-9a-f]{4}$")

    def test_negative_run_id_distinct(self):
        """Negative: two consecutive run_ids are not identical."""
        # Using mock secrets or just calling
        with mock.patch("secrets.token_hex", side_effect=["aaaa", "bbbb"]):
            self.assertNotEqual(gate_common.run_id(), gate_common.run_id())

    def test_boundary_run_id_length(self):
        """Boundary: length is 16 chars timestamp + 1 dash + 4 chars token = 21 chars."""
        self.assertEqual(len(gate_common.run_id()), 21)


GATE_COMMON = Path(__file__).resolve().parent / "gate_common.py"
MAX_FUNCTION_LINES = 60
MAX_COMPLEXITY = 10
MAX_STATEMENTS = 50


class SourceSweeps(unittest.TestCase):
    """AST invariant sweeps over tools/gate_common.py (HISS-02, HISS-04, HISS-08)."""

    tree = ast.parse(GATE_COMMON.read_text(encoding="utf-8"))
    functions = testkit.function_index(tree)

    def test_only_run_starts_a_process_and_every_wait_is_bounded(self):
        """Assert only run_command starts a process and every wait call is bounded."""
        testkit.assert_only_run_starts_a_process_and_every_wait_is_bounded(
            self, self.functions, self.tree, expected_starters=[("run_command", "run")]
        )

    def test_no_function_exceeds_the_length_or_complexity_limit(self):
        """Assert no function in gate_common exceeds length, complexity or statement thresholds."""
        testkit.assert_function_limits(
            self, self.functions, MAX_FUNCTION_LINES, MAX_COMPLEXITY, MAX_STATEMENTS
        )

    def test_the_gate_never_evaluates_code(self):
        """Assert gate_common never references eval, exec, or compile."""
        testkit.assert_never_evaluates_code(self, self.tree)


if __name__ == "__main__":
    unittest.main()
