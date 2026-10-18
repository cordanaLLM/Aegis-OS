#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Unit tests for tools/verify_shell.py."""

import io
import json
import sys
import unittest
from pathlib import Path
from unittest.mock import patch

TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import verify_shell  # noqa: E402
from verify_shell import (  # noqa: E402
    REFERENCE_LINE,
    capability_reasons,
    find_atspi_program,
    judge_atspi_late,
    judge_atspi_negative,
    judge_atspi_sound,
    judge_deadline,
    judge_filter_control,
    judge_first_configure,
    judge_layer_shell_missing,
    judge_refused_line,
    main,
    read_until,
    read_versions,
)


def readback_output(failures=None, nodes=1005, update=True, drop=()):
    """Builds readback stdout as aegis-atspi-readback prints it (measured 2026-10-18)."""
    failures = failures or {}
    lines = [f"NODES {nodes}"]
    names = verify_shell.READBACK_CASES + ((verify_shell.UPDATE_CASE,) if update else ())
    for name in names:
        if name in drop:
            continue
        if name in failures:
            lines.append(f"CASE {name} fail: {failures[name]}")
        else:
            lines.append(f"CASE {name} pass")
    return "\n".join(lines) + "\n"


MEASURED_NEGATIVES = {
    "button-as-container": {"atspi/button-exposed": "role panel, want button"},
    "unlabelled": {
        "atspi/seeded-reachable": "999 of 1000 seeded nodes reachable; first missing Node 517",
        "atspi/every-node-named": "list item at path 0/1/517 has no name",
    },
    "culled": {
        "atspi/seeded-reachable": "10 of 1000 seeded nodes reachable; first missing Node 10"
    },
    "hidden-shown": {
        "atspi/hidden-subtree-hidden": "hidden element 'Hidden panel' carries showing state"
    },
}


class FakePipe:
    """A pipe stand-in whose chunks read_until consumes through injected select/read."""

    def __init__(self, chunks):
        self.chunks = list(chunks)

    def fileno(self):
        return 99


def fake_select(pipe):
    return lambda r, w, x, t: (r if pipe.chunks else [], [], [])


def fake_read(pipe):
    return lambda fd, n: pipe.chunks.pop(0) if pipe.chunks else b""


class TestAtspiJudges(unittest.TestCase):
    """Positive, negative and boundary cases for the AT-SPI gate judges."""

    def test_sound_passes(self):
        self.assertEqual(judge_atspi_sound(0, readback_output(), True), (True, ""))

    def test_late_client_passes_without_update_line(self):
        self.assertEqual(judge_atspi_late(0, readback_output(update=False)), (True, ""))

    def test_each_measured_negative_passes(self):
        for variant, failures in MEASURED_NEGATIVES.items():
            with self.subTest(variant=variant):
                out = readback_output(failures)
                self.assertEqual(judge_atspi_negative(variant, 1, out), (True, ""))

    def test_sound_fails_on_empty_output(self):
        passed, why = judge_atspi_sound(0, "", True)
        self.assertFalse(passed)
        self.assertIn("None nodes", why)

    def test_sound_fails_when_a_case_line_is_missing(self):
        out = readback_output(drop=("atspi/focus-reported",))
        passed, why = judge_atspi_sound(0, out, True)
        self.assertFalse(passed)
        self.assertIn("no atspi/focus-reported line", why)

    def test_sound_fails_when_a_case_fails(self):
        out = readback_output({"atspi/focus-reported": "not focused"})
        passed, why = judge_atspi_sound(0, out, True)
        self.assertFalse(passed)
        self.assertIn("readback case failed", why)

    def test_sound_fails_without_click_event(self):
        passed, why = judge_atspi_sound(0, readback_output(), False)
        self.assertFalse(passed)
        self.assertIn("EVENT click", why)

    def test_sound_fails_on_nonzero_exit(self):
        passed, why = judge_atspi_sound(3, "ERROR D-Bus", True)
        self.assertFalse(passed)
        self.assertIn("exited 3", why)

    def test_negative_fails_when_readback_passes(self):
        passed, why = judge_atspi_negative("culled", 0, readback_output())
        self.assertFalse(passed)
        self.assertIn("unexpectedly passed", why)

    def test_negative_fails_on_read_error_exit(self):
        passed, why = judge_atspi_negative("culled", 3, "ERROR D-Bus call timed out\n")
        self.assertFalse(passed)
        self.assertIn("want 1", why)

    def test_negative_fails_when_another_case_also_fails(self):
        failures = dict(MEASURED_NEGATIVES["culled"])
        failures["atspi/focus-reported"] = "not focused"
        passed, why = judge_atspi_negative("culled", 1, readback_output(failures))
        self.assertFalse(passed)
        self.assertIn("atspi/focus-reported", why)

    def test_negative_fails_when_named_case_lacks_token(self):
        failures = {"atspi/button-exposed": "no click action"}
        passed, why = judge_atspi_negative("button-as-container", 1, readback_output(failures))
        self.assertFalse(passed)
        self.assertIn("did not contain 'panel'", why)

    def test_negative_fails_for_unknown_variant(self):
        passed, why = judge_atspi_negative("sound", 1, readback_output())
        self.assertFalse(passed)
        self.assertIn("unknown negative variant", why)

    def test_sound_boundary_seeded_count(self):
        self.assertTrue(judge_atspi_sound(0, readback_output(nodes=1000), True)[0])
        passed, why = judge_atspi_sound(0, readback_output(nodes=999), True)
        self.assertFalse(passed)
        self.assertIn("999 nodes", why)

    def test_negative_boundary_unlabelled_needs_both_failures(self):
        failures = {"atspi/every-node-named": "list item at path 0/1/517 has no name"}
        passed, why = judge_atspi_negative("unlabelled", 1, readback_output(failures))
        self.assertFalse(passed)
        self.assertIn("atspi/seeded-reachable passed, want fail", why)


class TestReadUntil(unittest.TestCase):
    """read_until stops at the token, at EOF, or at the deadline."""

    def test_finds_token_split_over_chunks(self):
        pipe = FakePipe([b"REA", b"DY\n"])
        found, text = read_until(pipe, "READY", 5, fake_select(pipe), fake_read(pipe))
        self.assertTrue(found)
        self.assertEqual(text, "READY\n")

    def test_eof_without_token_fails(self):
        pipe = FakePipe([b"OUTCOME open-failed\n"])
        found, text = read_until(pipe, "READY", 5, fake_select(pipe), fake_read(pipe))
        self.assertFalse(found)
        self.assertIn("open-failed", text)

    def test_zero_timeout_reads_nothing(self):
        pipe = FakePipe([b"READY\n"])
        found, text = read_until(pipe, "READY", 0, fake_select(pipe), fake_read(pipe))
        self.assertFalse(found)
        self.assertEqual(text, "")


class TestVerifyShellPositive(unittest.TestCase):
    """Positive test cases for judge functions, capabilities, and request lines."""

    def test_capabilities_all_present(self):
        fake_which = lambda prog: f"/usr/bin/{prog}"  # noqa: E731
        fake_atspi = lambda prog: Path(f"/usr/lib/{prog}")  # noqa: E731
        reasons = capability_reasons(
            platform="linux",
            which=fake_which,
            unix_fds=lambda: True,
            atspi_finder=fake_atspi,
        )
        self.assertEqual(reasons, [])

    def test_capabilities_name_missing_fd_passing(self):
        fake_which = lambda prog: f"/usr/bin/{prog}"  # noqa: E731
        fake_atspi = lambda prog: Path(f"/usr/lib/{prog}")  # noqa: E731
        reasons = capability_reasons(
            platform="linux",
            which=fake_which,
            unix_fds=lambda: False,
            atspi_finder=fake_atspi,
        )
        self.assertEqual(len(reasons), 1)
        self.assertIn("SCM_RIGHTS", reasons[0])

    def test_find_atspi_program_in_dirs(self):
        exists = lambda p: p == "/usr/lib/at-spi-bus-launcher"  # noqa: E731
        found = find_atspi_program("at-spi-bus-launcher", dirs=("/usr/lib",), exists=exists)
        self.assertEqual(str(found), "/usr/lib/at-spi-bus-launcher")

    def test_read_versions_returns_versions(self):
        def fake_runner(argv, timeout):
            if argv[0] == "pkg-config":
                return 0, "2.62.1\n", ""
            if argv[0] == "dbus-daemon":
                return 0, "D-Bus Message Bus Daemon 1.16.2\n", ""
            return 1, "", "unknown"

        vers = read_versions(runner=fake_runner)
        self.assertEqual(vers["atspi-2"], "2.62.1")
        self.assertEqual(vers["dbus"], "1.16.2")

    def test_judge_first_configure_passes(self):
        passed, why = judge_first_configure(0, "OUTCOME configured 1280x48\n", 1280, 48)
        self.assertTrue(passed)
        self.assertEqual(why, "")

    def test_judge_layer_shell_missing_passes(self):
        out = (
            "OUTCOME layer-shell-missing: LayerShellNotSupportedError: "
            "Compositor doesn't support zwlr_layer_shell_v1\n"
        )
        passed, why = judge_layer_shell_missing(4, out)
        self.assertTrue(passed)
        self.assertEqual(why, "")

    def test_judge_deadline_passes(self):
        out = "OUTCOME deadline: zwlr_layer_surface_v1.configure did not arrive within 5000 ms\n"
        passed, why = judge_deadline(5, out)
        self.assertTrue(passed)
        self.assertEqual(why, "")

    def test_judge_refused_line_passes(self):
        passed, why = judge_refused_line(
            6, 'OUTCOME refused: the jsonrpc member is absent or not "2.0"\n'
        )
        self.assertTrue(passed)
        self.assertEqual(why, "")

    def test_judge_filter_control_passes(self):
        hide_txt = "interface: 'wl_compositor', version: 4\ninterface: 'xdg_wm_base', version: 3\n"
        drop_txt = (
            "interface: 'wl_compositor', version: 4\n"
            "interface: 'xdg_wm_base', version: 3\n"
            "interface: 'zwlr_layer_shell_v1', version: 4\n"
        )
        passed, why = judge_filter_control(0, hide_txt, 0, drop_txt)
        self.assertTrue(passed)
        self.assertEqual(why, "")

    def test_reference_line_structure(self):
        data = json.loads(REFERENCE_LINE)
        self.assertEqual(data.get("jsonrpc"), "2.0")
        self.assertEqual(data.get("method"), "MOUNT_SHELL_CANVAS")
        self.assertEqual(data.get("id"), 1)
        params = data.get("params", {})
        expected_fields = {
            "schema",
            "namespace",
            "layer",
            "anchor",
            "exclusive_zone",
            "exclusive_edge",
            "keyboard_interactivity",
        }
        self.assertEqual(set(params.keys()), expected_fields)
        self.assertEqual(len(params), 7)


class TestVerifyShellNegative(unittest.TestCase):
    """Negative tests for failed judgements and missing host capabilities."""

    def test_read_versions_fails_on_pkg_config_error(self):
        def fake_runner(argv, timeout):
            if argv[0] == "pkg-config":
                return 1, "", "not found"
            return 0, "D-Bus Message Bus Daemon 1.16.2\n", ""

        with self.assertRaises(verify_shell.GateError):
            read_versions(runner=fake_runner)

    def test_read_versions_fails_on_dbus_daemon_error(self):
        def fake_runner(argv, timeout):
            if argv[0] == "pkg-config":
                return 0, "2.62.1\n", ""
            return 1, "", "not found"

        with self.assertRaises(verify_shell.GateError):
            read_versions(runner=fake_runner)

    def test_first_configure_fails_on_wrong_width(self):
        passed, why = judge_first_configure(0, "OUTCOME configured 1279x48\n", 1280, 48)
        self.assertFalse(passed)
        self.assertIn("1280x48", why)

    def test_first_configure_fails_on_exit_3(self):
        passed, why = judge_first_configure(3, "OUTCOME open-failed: failed\n", 1280, 48)
        self.assertFalse(passed)
        self.assertIn("exited 3", why)

    def test_first_configure_fails_on_missing_line(self):
        passed, why = judge_first_configure(0, "some other output\n", 1280, 48)
        self.assertFalse(passed)
        self.assertIn("missing expected line", why)

    def test_layer_shell_missing_fails_on_exit_0(self):
        passed, why = judge_layer_shell_missing(0, "OUTCOME configured 1280x48\n")
        self.assertFalse(passed)
        self.assertIn("exited 0, expected 4", why)

    def test_layer_shell_missing_fails_on_open_failed_text(self):
        passed, why = judge_layer_shell_missing(4, "OUTCOME open-failed: other error\n")
        self.assertFalse(passed)
        self.assertIn("missing LayerShellNotSupportedError", why)

    def test_deadline_fails_on_exit_0(self):
        passed, why = judge_deadline(0, "OUTCOME configured 1280x48\n")
        self.assertFalse(passed)
        self.assertIn("exited 0, expected 5", why)

    def test_filter_control_fails_when_hide_lists_layer_shell(self):
        bad_hide = "interface: 'xdg_wm_base'\ninterface: 'zwlr_layer_shell_v1'\n"
        good_drop = "interface: 'zwlr_layer_shell_v1'\n"
        passed, why = judge_filter_control(0, bad_hide, 0, good_drop)
        self.assertFalse(passed)
        self.assertIn("unexpectedly listed zwlr_layer_shell_v1", why)

    def test_main_missing_kwin_prints_skip(self):
        with patch.object(
            verify_shell,
            "capability_reasons",
            return_value=["kwin_wayland is not installed"],
        ):
            buf = io.StringIO()
            with patch("sys.stdout", buf):
                code = main()
            self.assertEqual(code, 0)
            output = buf.getvalue()
            expected = "SKIP: kwin_wayland is not installed; the native shell gate did not run.\n"
            self.assertEqual(output, expected)
            self.assertTrue(output.startswith("SKIP: "))
            self.assertTrue(output.strip().endswith("; the native shell gate did not run."))


class TestVerifyShellBoundary(unittest.TestCase):
    """Boundary tests for platform compatibility and result line formatting."""

    def test_non_linux_platform_skips(self):
        reasons = capability_reasons(platform="darwin")
        self.assertEqual(len(reasons), 1)
        self.assertIn("this host is darwin", reasons[0])

    def test_reference_line_is_one_newline_terminated_line(self):
        self.assertTrue(REFERENCE_LINE.endswith("\n"))
        self.assertEqual(REFERENCE_LINE.count("\n"), 1)

    def test_every_binary_case_sends_one_terminated_line(self):
        calls = []

        def fake_runner(argv, env):
            calls.append(argv)
            return 0, ""

        verify_shell.run_surface_cases("/bin/shell", "/k", "/h", "/d", runner=fake_runner)
        lines = [argv[-1] for argv in calls if argv[0] == "/bin/shell"]
        self.assertEqual(len(lines), 4)
        for line in lines:
            self.assertTrue(line.endswith("\n"), line)
            self.assertEqual(line.count("\n"), 1, line)

    def test_refusal_for_another_reason_fails(self):
        out = "OUTCOME refused: the message is not exactly one newline-terminated line\n"
        passed, why = judge_refused_line(6, out)
        self.assertFalse(passed)
        self.assertIn("jsonrpc", why)

    def test_summary_with_one_fail(self):
        results = [("surface/first-configure", False, "exited 1")] + [
            (f"case-{n}", True, "") for n in range(10)
        ]
        self.assertEqual(verify_shell.summary(results), "RESULT fail: 1 case(s) failed")

    def test_summary_of_zero_cases_fails(self):
        self.assertEqual(verify_shell.summary([]), "RESULT fail: 0 of 11 cases ran")

    def test_summary_of_eleven_passes(self):
        results = [(f"case-{n}", True, "") for n in range(11)]
        self.assertEqual(verify_shell.summary(results), "RESULT pass")

    def test_run_all_cases_runs_11_cases(self):
        def fake_runner(argv, env):
            return 0, "OUTCOME configured 1280x48\n"

        def fake_atspi_runner(
            shell_bin, readback_bin, variant, env, read_only=False, delay=0.0, runner=None
        ):
            return True, ""

        results = verify_shell.run_all_cases(
            "/bin/shell",
            "/k",
            "/h",
            "/d",
            runner=fake_runner,
            readback_bin="/bin/readback",
            a11y_env={},
            atspi_case_runner=fake_atspi_runner,
        )
        self.assertEqual(len(results), 11)


class TestStartKwin(unittest.TestCase):
    """start_kwin releases its log file when the compositor never opens a socket."""

    def test_log_closed_when_kwin_exits_early(self):
        import tempfile
        from pathlib import Path as P

        seen = {}

        class Exited:
            def poll(self):
                return 1

        def fake_popen(cmd, **kwargs):
            seen["log"] = kwargs["stdout"]
            return Exited()

        with tempfile.TemporaryDirectory() as tmp:
            with self.assertRaises(verify_shell.GateError):
                verify_shell.start_kwin(P(tmp), popen_fn=fake_popen)
        self.assertTrue(seen["log"].closed)


if __name__ == "__main__":
    unittest.main()
