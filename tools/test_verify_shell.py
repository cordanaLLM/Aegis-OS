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
    judge_ring_negative,
    judge_ring_positive,
    main,
    read_until,
    read_versions,
    write_no_activation_config,
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
            (f"case-{n}", True, "") for n in range(18)
        ]
        self.assertEqual(verify_shell.summary(results), "RESULT fail: 1 case(s) failed")

    def test_summary_of_zero_cases_fails(self):
        self.assertEqual(verify_shell.summary([]), "RESULT fail: 0 of 19 cases ran")

    def test_summary_of_nineteen_passes(self):
        results = [(f"case-{n}", True, "") for n in range(19)]
        self.assertEqual(verify_shell.summary(results), "RESULT pass")

    def test_run_all_cases_runs_19_cases(self):
        def fake_runner(argv, env):
            return 0, "OUTCOME configured 1280x48\n"

        def fake_atspi_runner(
            shell_bin, readback_bin, variant, env, read_only=False, delay=0.0, runner=None
        ):
            return True, ""

        def fake_ring_runner(shell_bin, ring_readback_bin, variant, is_positive, env, runner=None):
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
            ring_readback_bin="/bin/ring-readback",
            ring_env={},
            ring_case_runner=fake_ring_runner,
        )
        self.assertEqual(len(results), 19)


class TestNoActivationConfig(unittest.TestCase):
    """write_no_activation_config creates a minimal session bus configuration."""

    def test_omits_standard_session_servicedirs(self):
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            cfg = write_no_activation_config(Path(tmp))
            self.assertTrue(cfg.is_file())
            content = cfg.read_text(encoding="utf-8")
            self.assertIn("<type>session</type>", content)
            self.assertIn("<listen>", content)
            self.assertNotIn("standard_session_servicedirs", content)
            self.assertNotIn("<includedir>", content)


class TestRingJudges(unittest.TestCase):
    """Positive, negative, and boundary tests for focus ring judgements."""

    def test_positive_pass_on_exit_0_with_case_pass(self):
        stdout = "MEASURE Left 3.000 6.40\nCASE ring/unit pass\n"
        passed, why = judge_ring_positive("unit", 0, stdout)
        self.assertTrue(passed)
        self.assertEqual(why, "best Left 3.000 px, 6.40:1")

    def test_positive_fails_on_nonzero_exit(self):
        stdout = "ERROR D-Bus call failed\n"
        passed, why = judge_ring_positive("unit", 2, stdout)
        self.assertFalse(passed)
        self.assertIn("exited 2", why)

    def test_positive_fails_when_pass_line_missing(self):
        stdout = "MEASURE Left 3.000 6.40\n"
        passed, why = judge_ring_positive("unit", 0, stdout)
        self.assertFalse(passed)
        self.assertIn("missing pass line", why)

    def test_negative_pass_on_exit_1_with_case_fail(self):
        stdout = (
            "MEASURE Left 1.500 6.40\n"
            "CASE ring/scaled-layer fail: "
            "best side Left 1.500 px, 6.40:1; want >= 2 px and >= 3:1\n"
        )
        passed, why = judge_ring_negative("scaled-layer", 1, stdout)
        self.assertTrue(passed)
        self.assertEqual(why, "best Left 1.500 px, 6.40:1")

    def test_negative_fails_when_readback_exits_0(self):
        stdout = "CASE ring/scaled-layer pass\n"
        passed, why = judge_ring_negative("scaled-layer", 0, stdout)
        self.assertFalse(passed)
        self.assertIn("unexpectedly passed", why)

    def test_negative_fails_on_setup_error_exit(self):
        stdout = "ERROR D-Bus setup\n"
        passed, why = judge_ring_negative("scaled-layer", 2, stdout)
        self.assertFalse(passed)
        self.assertIn("want 1", why)

    def test_negative_fails_when_fail_reason_empty(self):
        stdout = "CASE ring/scaled-layer fail:\n"
        passed, why = judge_ring_negative("scaled-layer", 1, stdout)
        self.assertFalse(passed)
        self.assertIn("missing fail line", why)

    def test_boundary_width_2_0_positive(self):
        stdout = "CASE ring/width-2-0 pass\n"
        self.assertTrue(judge_ring_positive("width-2-0", 0, stdout)[0])

    def test_positive_pass_reports_best_side(self):
        stdout = "MEASURE left 2.000 6.44\nMEASURE top 3.000 6.44\nCASE ring/unit pass\n"
        self.assertEqual(
            judge_ring_positive("unit", 0, stdout), (True, "best top 3.000 px, 6.44:1")
        )

    def test_negative_pass_reports_best_side(self):
        stdout = "MEASURE left 1.902 5.85\nCASE ring/width-1-9 fail: narrow\n"
        self.assertEqual(
            judge_ring_negative("width-1-9", 1, stdout), (True, "best left 1.902 px, 5.85:1")
        )

    def test_measure_summary_without_measure_is_empty(self):
        self.assertEqual(verify_shell.measure_summary("CASE ring/under-panel fail: x\n"), "")

    def test_boundary_width_1_9_negative(self):
        stdout = (
            "MEASURE left 1.900 6.40\n"
            "CASE ring/width-1-9 fail: best side Left 1.900 px, 6.40:1; want >= 2 px and >= 3:1\n"
        )
        self.assertTrue(judge_ring_negative("width-1-9", 1, stdout)[0])

    def test_width_1_9_failing_at_2_0_is_wrong_reason(self):
        stdout = (
            "MEASURE left 2.000 1.20\n"
            "CASE ring/width-1-9 fail: best side Left 2.000 px, 1.20:1; want >= 2 px and >= 3:1\n"
        )
        passed, why = judge_ring_negative("width-1-9", 1, stdout)
        self.assertFalse(passed)
        self.assertIn("best measured width 2.0", why)

    def test_width_1_9_upper_edge_accepted(self):
        stdout = "MEASURE top 1.950 6.40\nCASE ring/width-1-9 fail: narrow\n"
        self.assertTrue(judge_ring_negative("width-1-9", 1, stdout)[0])

    def test_scaled_layer_at_1_5_accepted(self):
        stdout = (
            "MEASURE left 1.500 6.40\nMEASURE top 1.480 6.40\nCASE ring/scaled-layer fail: narrow\n"
        )
        self.assertTrue(judge_ring_negative("scaled-layer", 1, stdout)[0])

    def test_scaled_layer_reporting_2_9_fails(self):
        stdout = "MEASURE left 2.900 1.10\nCASE ring/scaled-layer fail: low contrast\n"
        passed, why = judge_ring_negative("scaled-layer", 1, stdout)
        self.assertFalse(passed)
        self.assertIn("want 1.4..1.6", why)

    def test_scaled_layer_without_measure_fails(self):
        stdout = "CASE ring/scaled-layer fail: no sides\n"
        self.assertFalse(judge_ring_negative("scaled-layer", 1, stdout)[0])

    def test_under_panel_with_visible_ring_fails(self):
        stdout = "MEASURE left 2.000 1.00\nCASE ring/under-panel fail: low contrast\n"
        passed, why = judge_ring_negative("under-panel", 1, stdout)
        self.assertFalse(passed)
        self.assertIn("want 0.0..0.5", why)

    def test_under_panel_with_zero_width_sides_accepted(self):
        stdout = (
            "MEASURE left 0.000 1.00\nMEASURE top 0.000 1.00\nCASE ring/under-panel fail: none\n"
        )
        self.assertTrue(judge_ring_negative("under-panel", 1, stdout)[0])

    def test_best_measured_width_ignores_malformed_lines(self):
        stdout = "MEASURE left x 1.0\nMEASURE top\nMEASURE right 1.250 3.00\n"
        self.assertEqual(verify_shell.best_measured_width(stdout), 1.25)

    def test_boundary_partly_covered_positive(self):
        stdout = "CASE ring/partly-covered pass\n"
        self.assertTrue(judge_ring_positive("partly-covered", 0, stdout)[0])

    def test_negative_under_panel(self):
        stdout = "CASE ring/under-panel fail: ring not found on any side\n"
        self.assertTrue(judge_ring_negative("under-panel", 1, stdout)[0])


class TestRunRingCase(unittest.TestCase):
    """run_ring_case executes fixture and readback with proper error handling."""

    def test_fixture_not_ready_fails(self):
        class FakeProc:
            def __init__(self):
                self.stdout = FakePipe([b"ERROR\n"])
                self.pid = 12345

            def poll(self):
                return 1

        def fake_popen(cmd, **kwargs):
            return FakeProc()

        with patch.object(verify_shell, "read_until", return_value=(False, "ERROR\n")):
            with patch.object(verify_shell, "teardown_process"):
                passed, why = verify_shell.run_ring_case(
                    "/bin/shell",
                    "/bin/readback",
                    "unit",
                    True,
                    {},
                    popen_fn=fake_popen,
                )
                self.assertFalse(passed)
                self.assertIn("failed to announce READY", why)

    def test_fixture_missing_ring_line_fails(self):
        class FakeProc:
            def __init__(self):
                self.stdout = FakePipe([b"READY\n"])
                self.pid = 12345

            def poll(self):
                return None

        def fake_popen(cmd, **kwargs):
            return FakeProc()

        with patch.object(verify_shell, "read_until", return_value=(True, "READY\n")):
            with patch.object(verify_shell, "teardown_process"):
                passed, why = verify_shell.run_ring_case(
                    "/bin/shell",
                    "/bin/readback",
                    "unit",
                    True,
                    {},
                    popen_fn=fake_popen,
                )
                self.assertFalse(passed)
                self.assertIn("did not emit RING line", why)

    def test_happy_path_runs_readback_with_ring_line(self):
        class FakeProc:
            def __init__(self):
                self.stdout = FakePipe([b"RING 590 310 690 410 3 2\nREADY\n"])
                self.pid = 12345

            def poll(self):
                return None

        def fake_popen(cmd, **kwargs):
            return FakeProc()

        rb_calls = []

        def fake_runner(cmd, env):
            rb_calls.append(cmd)
            return 0, "CASE ring/unit pass\n"

        ready_out = "RING 590 310 690 410 3 2\nREADY\n"
        with patch.object(verify_shell, "read_until", return_value=(True, ready_out)):
            with patch.object(verify_shell, "teardown_process"):
                passed, why = verify_shell.run_ring_case(
                    "/bin/shell",
                    "/bin/readback",
                    "unit",
                    True,
                    {},
                    popen_fn=fake_popen,
                    runner=fake_runner,
                )
                self.assertTrue(passed)
                self.assertEqual(why, "")
                self.assertEqual(len(rb_calls), 1)
                self.assertEqual(rb_calls[0], ["/bin/readback", "unit", "RING 590 310 690 410 3 2"])


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


class ReportNoteTests(unittest.TestCase):
    """A passing case with a reason prints it as a NOTE; the CASE line keeps its form."""

    def run_report(self, results):
        import io

        out = io.StringIO()
        with patch.object(verify_shell.sys, "stdout", out):
            verify_shell.report_run_summary(results, "kwin 6.8.0", {"atspi-2": "x", "dbus": "y"})
        return out.getvalue().splitlines()

    def test_pass_with_reason_prints_note(self):
        lines = self.run_report([("ring/unit", True, "best top 3.000 px, 6.44:1")])
        self.assertIn("CASE ring/unit pass", lines)
        self.assertIn("NOTE ring/unit: best top 3.000 px, 6.44:1", lines)

    def test_pass_without_reason_prints_no_note(self):
        lines = self.run_report([("atspi/sound", True, "")])
        self.assertIn("CASE atspi/sound pass", lines)
        self.assertFalse(any(line.startswith("NOTE atspi/sound") for line in lines))

    def test_fail_prints_reason_on_case_line_only(self):
        lines = self.run_report([("ring/min", False, "narrow")])
        self.assertIn("CASE ring/min fail: narrow", lines)
        self.assertFalse(any(line.startswith("NOTE ring/min") for line in lines))


class KwinConfigTests(unittest.TestCase):
    """start_kwin reads a gate-owned configuration with animations off, never the host's."""

    def launch(self, tmp, environ):
        from pathlib import Path as P

        seen = {}

        class Exited:
            def poll(self):
                return 1

        def fake_popen(cmd, **kwargs):
            seen["env"] = kwargs["env"]
            kwargs["stdout"].close()
            return Exited()

        with patch.dict(verify_shell.os.environ, environ, clear=False):
            with self.assertRaises(verify_shell.GateError):
                verify_shell.start_kwin(P(tmp), popen_fn=fake_popen)
        return seen["env"]

    def test_animations_off_in_gate_config(self):
        import tempfile
        from pathlib import Path as P

        with tempfile.TemporaryDirectory() as tmp:
            env = self.launch(tmp, {})
            config = P(env["XDG_CONFIG_HOME"])
            self.assertEqual(config.parent, P(tmp))
            for name in ("kdeglobals", "kwinrc"):
                text = (config / name).read_text(encoding="utf-8")
                self.assertIn("[KDE]\nAnimationDurationFactor=0\n", text)

    def test_host_config_home_replaced(self):
        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            env = self.launch(tmp, {"XDG_CONFIG_HOME": "/home/someone/.config"})
            self.assertNotEqual(env["XDG_CONFIG_HOME"], "/home/someone/.config")

    def test_existing_config_dir_reused(self):
        import tempfile
        from pathlib import Path as P

        with tempfile.TemporaryDirectory() as tmp:
            (P(tmp) / "config").mkdir()
            env = self.launch(tmp, {})
            self.assertTrue((P(env["XDG_CONFIG_HOME"]) / "kwinrc").is_file())


if __name__ == "__main__":
    unittest.main()
