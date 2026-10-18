#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Unit tests for tools/verify_shell.py."""

import io
import json
import unittest
from unittest.mock import patch

import verify_shell
from verify_shell import (
    REFERENCE_LINE,
    capability_reasons,
    judge_deadline,
    judge_filter_control,
    judge_first_configure,
    judge_layer_shell_missing,
    judge_refused_line,
    main,
)


class TestVerifyShellPositive(unittest.TestCase):
    """Positive test cases for judge functions, capabilities, and request lines."""

    def test_capabilities_all_present(self):
        fake_which = lambda prog: f"/usr/bin/{prog}"  # noqa: E731
        reasons = capability_reasons(platform="linux", which=fake_which)
        self.assertEqual(reasons, [])

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

        verify_shell.run_all_cases("/bin/shell", "/k", "/h", "/d", runner=fake_runner)
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
            (f"surface/case-{n}", True, "") for n in range(4)
        ]
        self.assertEqual(verify_shell.summary(results), "RESULT fail: 1 case(s) failed")

    def test_summary_of_zero_cases_fails(self):
        self.assertEqual(verify_shell.summary([]), "RESULT fail: 0 of 5 cases ran")

    def test_summary_of_five_passes(self):
        results = [(f"surface/case-{n}", True, "") for n in range(5)]
        self.assertEqual(verify_shell.summary(results), "RESULT pass")


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
