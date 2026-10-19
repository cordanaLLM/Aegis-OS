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
    ALLOCATOR_SYMBOLS,
    REFERENCE_LINE,
    TIMING_CASES,
    WAYLAND_CLIENT_FLOOR,
    XKBCOMMON_FLOOR,
    allocator_share,
    capability_reasons,
    check_library_floors,
    compute_distribution,
    evaluate_timing,
    find_atspi_program,
    format_timing_lines,
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
    judge_timing,
    main,
    nearest_rank,
    parse_frame_line,
    parse_gpu_line,
    read_until,
    read_versions,
    read_zed_commit,
    run_timing_cases,
    timing_argv,
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
        self.assertEqual(len(lines), 6)
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
            (f"case-{n}", True, "") for n in range(24)
        ]
        self.assertEqual(verify_shell.summary(results), "RESULT fail: 1 case(s) failed")

    def test_summary_of_zero_cases_fails(self):
        self.assertEqual(verify_shell.summary([]), "RESULT fail: 0 of 25 cases ran")

    def test_summary_of_twenty_five_passes(self):
        results = [(f"case-{n}", True, "") for n in range(25)]
        self.assertEqual(verify_shell.summary(results), "RESULT pass")

    def test_run_all_cases_runs_25_cases(self):
        def fake_runner(argv, env):
            return 0, "OUTCOME configured 1280x48\n"

        def fake_atspi_runner(
            shell_bin, readback_bin, variant, env, read_only=False, delay=0.0, runner=None
        ):
            return True, ""

        def fake_ring_runner(shell_bin, ring_readback_bin, variant, is_positive, env, runner=None):
            return True, ""

        def fake_read_back_runner(binary, runtime_dir, runner):
            return "surface/parameters-read-back", True, ""

        def fake_timing_runner(shell_bin, env, seeded, **kwargs):
            return 0, ["GPU mock\tdrv\tinfo\t0"]

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
            timing_case_runner=fake_timing_runner,
            read_back_runner=fake_read_back_runner,
        )
        self.assertEqual(len(results), 25)


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


def make_frame_lines(warmup_count, measured_count, nodes_val=1005):
    """Generates synthetic FRAME lines matching Rust tests/timing.rs."""
    lines = []
    for i in range(warmup_count):
        node_str = "-" if nodes_val is None else str(nodes_val)
        lines.append(f"FRAME {i} 0 {node_str}")
    for i in range(1, measured_count + 1):
        node_str = "-" if nodes_val is None else str(nodes_val)
        lines.append(f"FRAME {i} 0 {node_str}")
    return lines


def reference_info():
    """Returns standard reference RunInfo matching Rust tests/timing.rs."""
    return {
        "gpu": "Intel(R) Arc(TM) A380 Graphics (DG2)",
        "driver": "Intel open-source Mesa driver",
        "driver_info": "Mesa 25.0.0",
        "software": False,
        "compositor": "KWin 6.8.0",
        "zed_commit": "322cd509dcccaee1f6bc44725d270598ae8171ad",
    }


class TestTimingJudges(unittest.TestCase):
    """Positive, negative, and boundary tests for timing judges and distributions."""

    def test_positive_run_judged_ok(self):
        lines = make_frame_lines(60, 600, 1005)
        info = reference_info()
        ok, note = judge_timing(lines, info, frames=600, warmup=60, min_nodes=1000)
        self.assertTrue(ok)
        self.assertIn("frame p50=", note)
        self.assertIn("tree-update p50=", note)
        self.assertIn("Intel(R) Arc(TM) A380 Graphics (DG2)", note)
        self.assertIn("KWin 6.8.0", note)
        self.assertIn("322cd509dcccaee1f6bc44725d270598ae8171ad", note)

        meas, ref = evaluate_timing(lines, info, frames=600, warmup=60, min_nodes=1000)
        self.assertIsNone(ref)
        self.assertEqual(meas["frames"], 600)
        self.assertEqual((meas["nodes_min"], meas["nodes_max"]), (1005, 1005))
        expected_dist = {"min": 1, "p50": 300, "p95": 570, "p99": 594, "max": 600, "mean": 300}
        self.assertEqual(meas["frame"], expected_dist)
        self.assertEqual(meas["tree_update"], expected_dist)

    def test_positive_measurement_lines_format(self):
        lines = make_frame_lines(60, 600, 1005)
        info = reference_info()
        meas, _ = evaluate_timing(lines, info, frames=600, warmup=60, min_nodes=1000)
        t_lines = format_timing_lines(meas, info)
        self.assertEqual(len(t_lines), 4)
        self.assertEqual(t_lines[0], "TIMING frames 600 nodes 1005..1005")
        self.assertEqual(
            t_lines[1],
            "TIMING frame_ns min 1 p50 300 p95 570 p99 594 max 600 mean 300",
        )
        self.assertEqual(
            t_lines[2],
            "TIMING tree_update_ns min 1 p50 300 p95 570 p99 594 max 600 mean 300",
        )
        self.assertIn("TIMING gpu Intel(R) Arc(TM) A380 Graphics (DG2)", t_lines[3])
        self.assertIn("driver Intel open-source Mesa driver Mesa 25.0.0", t_lines[3])
        self.assertIn("compositor KWin 6.8.0", t_lines[3])
        self.assertIn("zed 322cd509dcccaee1f6bc44725d270598ae8171ad", t_lines[3])

    def test_negative_adapter_inactive_refused(self):
        lines = make_frame_lines(60, 600, 1005)
        lines[70] = "FRAME 10 0 -"
        ok, why = judge_timing(lines, reference_info())
        self.assertFalse(ok)
        self.assertEqual(
            why, "REFUSED accessibility adapter inactive: 1 frames without tree update"
        )

    def test_negative_too_few_nodes_refused(self):
        lines = make_frame_lines(60, 600, 1005)
        lines[102] = "FRAME 10 0 999"
        ok, why = judge_timing(lines, reference_info())
        self.assertFalse(ok)
        self.assertEqual(why, "REFUSED too few nodes: min seen 999, want 1000")

    def test_negative_software_renderer_refused(self):
        info = dict(reference_info(), software=True)
        lines = make_frame_lines(60, 600, 1005)
        ok, why = judge_timing(lines, info)
        self.assertFalse(ok)
        self.assertEqual(
            why,
            "REFUSED software renderer emulated: Intel(R) Arc(TM) A380 Graphics (DG2)",
        )

    def test_negative_missing_info_fields_refused(self):
        lines = make_frame_lines(60, 600, 1005)
        for field in ("gpu", "driver", "compositor", "zed_commit"):
            with self.subTest(field=field):
                info = dict(reference_info(), **{field: ""})
                ok, why = judge_timing(lines, info)
                self.assertFalse(ok)
                self.assertEqual(why, f"REFUSED missing info field: {field}")

    def test_negative_parse_frame_line_errors(self):
        self.assertIsNone(parse_frame_line("FRAME 100 50"))
        self.assertIsNone(parse_frame_line("FRAME 100 50 1005 extra"))
        self.assertIsNone(parse_frame_line("NOT_FRAME 100 50 1005"))
        self.assertIsNone(parse_frame_line("FRAME not_a_num 50 1005"))
        self.assertIsNone(parse_frame_line("FRAME 100 not_a_num 1005"))
        self.assertIsNone(parse_frame_line("FRAME 100 50 not_a_num"))

    def test_boundary_frame_counts(self):
        exact_lines = make_frame_lines(60, 600, 1005)
        self.assertTrue(judge_timing(exact_lines, reference_info())[0])

        short_lines = make_frame_lines(60, 599, 1005)
        ok, why = judge_timing(short_lines, reference_info())
        self.assertFalse(ok)
        self.assertEqual(why, "REFUSED too few frames: got 599, want 600")

    def test_boundary_nodes_at_exact_floor(self):
        lines = make_frame_lines(60, 600, 1000)
        ok, _ = judge_timing(lines, reference_info())
        self.assertTrue(ok)
        meas, _ = evaluate_timing(lines, reference_info())
        self.assertEqual(meas["nodes_min"], 1000)
        self.assertEqual(meas["nodes_max"], 1000)

    def test_boundary_residual_saturates_at_zero(self):
        sample = parse_frame_line("FRAME 50 100 1000")
        self.assertIsNotNone(sample)
        self.assertEqual(sample["residual_ns"], 0)

    def test_boundary_distribution_single_and_empty(self):
        self.assertEqual(
            compute_distribution([42]),
            {"min": 42, "p50": 42, "p95": 42, "p99": 42, "max": 42, "mean": 42},
        )
        self.assertIsNone(compute_distribution([]))

    def test_nearest_rank_cases(self):
        self.assertEqual(nearest_rank([10, 20, 30], 50), 20)
        self.assertIsNone(nearest_rank([], 50))

    def test_parse_gpu_line(self):
        line = "GPU Intel Arc\tIntel Mesa\tMesa 25.0\t0"
        info = parse_gpu_line(line)
        self.assertEqual(info["gpu"], "Intel Arc")
        self.assertEqual(info["driver"], "Intel Mesa")
        self.assertEqual(info["driver_info"], "Mesa 25.0")
        self.assertFalse(info["software"])

        empty_info = parse_gpu_line("GPU -")
        expected_empty = {"gpu": "", "driver": "", "driver_info": "", "software": False}
        self.assertEqual(empty_info, expected_empty)
        self.assertIsNone(parse_gpu_line("NOT_GPU"))
        self.assertIsNone(parse_gpu_line("GPU too_few_fields"))


class TestAllocatorShare(unittest.TestCase):
    """Tests for parsing perf report stdio output and calculating allocator share."""

    def test_allocator_symbols_contents(self):
        self.assertIn("malloc", ALLOCATOR_SYMBOLS)
        self.assertIn("__rust_alloc", ALLOCATOR_SYMBOLS)
        self.assertIn("alloc::alloc::", ALLOCATOR_SYMBOLS)

    def test_matching_allocator_symbols(self):
        report = (
            "# Overhead  Command  Shared Object      Symbol\n"
            "    12.34%  binary   libc.so.6          [.] malloc\n"
            "     5.60%  binary   aegis-forum-shell  [.] __rust_alloc\n"
            "     1.20%  binary   libc.so.6          [.] free\n"
            "     0.50%  binary   libc.so.6          [.] _int_malloc\n"
            "     0.10%  binary   aegis-forum-shell  [.] alloc::alloc::realloc\n"
            "     2.00%  binary   other.so           [.] some_other_func\n"
        )
        self.assertEqual(allocator_share(report), 19.74)

    def test_non_matching_symbols_yields_zero(self):
        report = (
            "# Overhead  Command  Shared Object  Symbol\n"
            "    99.00%  binary   libc.so.6      [.] render_stuff\n"
        )
        self.assertEqual(allocator_share(report), 0.0)

    def test_empty_report_returns_none(self):
        self.assertIsNone(allocator_share(""))
        self.assertIsNone(allocator_share("# Overhead  Symbol\n# ...\n"))

    def test_various_perf_stdio_formats(self):
        rep1 = "  12.34%  [.] malloc\n"
        self.assertEqual(allocator_share(rep1), 12.34)
        rep2 = "  10.00%  binary  [k] __libc_free\n"
        self.assertEqual(allocator_share(rep2), 10.0)
        rep3 = "  5.50 %  malloc\n"
        self.assertEqual(allocator_share(rep3), 5.5)


class TestTimingArgv(unittest.TestCase):
    """Tests for timing_argv command-line construction."""

    def test_timing_argv_without_perf(self):
        argv = timing_argv("/path/bin", 1000, None, None)
        self.assertEqual(argv, ["/path/bin", "timing", REFERENCE_LINE, "1000"])

    def test_timing_argv_with_perf(self):
        argv = timing_argv("/path/bin", 1000, "/usr/bin/perf", "/tmp/p.data")
        expected = [
            "/usr/bin/perf",
            "record",
            "-F",
            "999",
            "-o",
            "/tmp/p.data",
            "--",
            "/path/bin",
            "timing",
            REFERENCE_LINE,
            "1000",
        ]
        self.assertEqual(argv, expected)


class TestZedCommit(unittest.TestCase):
    """Tests for reading pinned zed commit from Cargo.toml."""

    def test_read_real_zed_commit(self):
        commit = read_zed_commit()
        self.assertEqual(commit, "56934ec0560bce8929025497411de7f5322fb6ca")

    def test_read_missing_cargo_toml_returns_empty(self):
        import tempfile
        from pathlib import Path as P

        with tempfile.TemporaryDirectory() as tmp:
            self.assertEqual(read_zed_commit(P(tmp) / "Cargo.toml"), "")


class TestTimingCases(unittest.TestCase):
    """Tests for timing cases execution and judgements."""

    def test_case_count_and_timing_cases_structure(self):
        self.assertEqual(verify_shell.CASE_COUNT, 25)
        self.assertEqual(len(TIMING_CASES), 3)
        names = [c[0] for c in TIMING_CASES]
        self.assertIn("timing/1000-nodes", names)
        self.assertIn("timing/999-nodes", names)
        self.assertIn("timing/adapter-inactive", names)

    def test_run_timing_cases_with_mocks(self):
        def fake_case_runner(shell_bin, env, seeded, **kwargs):
            gpu_line = "GPU mock_gpu\tmock_drv\tmock_ver\t0"
            if seeded == 1000 and "no-session-bus" in env.get("DBUS_SESSION_BUS_ADDRESS", ""):
                # adapter inactive case
                lines = make_frame_lines(60, 600, nodes_val=None)
            elif seeded == 994:
                # too few nodes case (999 nodes)
                lines = make_frame_lines(60, 600, nodes_val=999)
            else:
                # positive case
                lines = make_frame_lines(60, 600, nodes_val=1005)
            return 0, [gpu_line] + lines

        results = run_timing_cases(
            "/bin/shell",
            a11y_env={},
            kwin_ver="6.8.0",
            zed_commit="322cd509dcccaee1f6bc44725d270598ae8171ad",
            case_runner=fake_case_runner,
        )
        self.assertEqual(len(results), 3)
        for name, passed, why in results:
            self.assertTrue(passed, f"case {name} failed: {why}")


class TestTimingProcessHelpers(unittest.TestCase):
    """The adapter-inactive environment and the wait after DONE."""

    def test_timing_out_absent_is_none_and_ok(self):
        self.assertEqual(verify_shell.parse_timing_out([]), (None, True))

    def test_timing_out_reads_the_following_path(self):
        self.assertEqual(
            verify_shell.parse_timing_out(["--timing-out", "e.txt"]), (Path("e.txt"), True)
        )

    def test_timing_out_as_last_argument_is_refused(self):
        self.assertEqual(verify_shell.parse_timing_out(["--timing-out"]), (None, False))

    def test_inactive_env_points_the_bus_into_the_runtime_dir(self):
        a11y = {"DBUS_SESSION_BUS_ADDRESS": "unix:path=/rt/bus", "AT_SPI_BUS_ADDRESS": "x"}
        env = verify_shell.inactive_adapter_env(a11y, Path("/rt"))
        self.assertEqual(env["DBUS_SESSION_BUS_ADDRESS"], "unix:path=/rt/no-session-bus")
        self.assertNotIn("AT_SPI_BUS_ADDRESS", env)
        self.assertEqual(a11y["DBUS_SESSION_BUS_ADDRESS"], "unix:path=/rt/bus")

    def test_inactive_env_never_leaves_the_bus_unset(self):
        env = verify_shell.inactive_adapter_env({"XDG_RUNTIME_DIR": "/xdg"}, None)
        self.assertEqual(env["DBUS_SESSION_BUS_ADDRESS"], "unix:path=/xdg/no-session-bus")

    def test_wait_child_returns_the_exit_code(self):
        class Done:
            def wait(self, timeout):
                return 0

        self.assertEqual(verify_shell.wait_child(Done(), 30), 0)

    def test_wait_child_gives_none_when_the_child_outlives_the_wait(self):
        class Hung:
            def wait(self, timeout):
                raise verify_shell.subprocess.TimeoutExpired("fixture", timeout)

        self.assertIsNone(verify_shell.wait_child(Hung(), 0))


# The layer-shell requests a WAYLAND_DEBUG=server KWin 6.8.0 logged for the reference
# request (measured 2026-10-18), with one registry event to show events are skipped.
SERVER_LOG = """\
[09:54:22.901369] wl_registry#2.bind(63, "zwlr_layer_shell_v1", 5, new id [unknown]#16)
[09:54:23.019174] zwlr_layer_shell_v1#16.get_layer_surface(new id zwlr_layer_surface_v1#28, \
wl_surface#27, nil, 2, "aegis-forum")
[09:54:23.019232] zwlr_layer_surface_v1#28.set_size(0, 48)
[09:54:23.019234] zwlr_layer_surface_v1#28.set_anchor(13)
[09:54:23.019235] zwlr_layer_surface_v1#28.set_keyboard_interactivity(2)
[09:54:23.019236] zwlr_layer_surface_v1#28.set_exclusive_zone(48)
[09:54:23.019237] zwlr_layer_surface_v1#28.set_exclusive_edge(1)
[09:54:23.183092]  -> zwlr_layer_surface_v1#28.configure(1, 1280, 48)
[09:54:23.188899] zwlr_layer_surface_v1#28.ack_configure(1)
[09:54:23.188900] zwlr_layer_surface_v1#28.set_size(1280, 48)
"""
CONFIGURED = "OUTCOME configured 1280x48\n"


class TestParametersReadBack(unittest.TestCase):
    """KWin's own request log must carry every mount parameter on the wire (E28-2)."""

    def judge(self, log, code=0, stdout=CONFIGURED):
        return verify_shell.judge_parameters_read_back(code, stdout, log)

    def test_expected_wire_values_of_the_reference(self):
        self.assertEqual(
            verify_shell.expected_layer_requests(verify_shell.REFERENCE_PARAMS),
            {
                "get_layer_surface": ["2, aegis-forum"],
                "set_anchor": ["13"],
                "set_exclusive_zone": ["48"],
                "set_exclusive_edge": ["1"],
                "set_keyboard_interactivity": ["2"],
            },
        )

    def test_the_measured_log_passes(self):
        self.assertEqual(self.judge(SERVER_LOG), (True, ""))

    def test_events_and_other_interfaces_are_not_requests(self):
        requests = verify_shell.layer_requests(SERVER_LOG)
        self.assertNotIn("configure", requests)
        self.assertNotIn("bind", requests)
        self.assertEqual(requests["set_size"], ["0, 48", "1280, 48"])

    def test_a_wrong_layer_fails(self):
        log = SERVER_LOG.replace('nil, 2, "aegis-forum"', 'nil, 3, "aegis-forum"')
        passed, why = self.judge(log)
        self.assertFalse(passed)
        self.assertIn("get_layer_surface", why)

    def test_a_wrong_namespace_fails(self):
        passed, why = self.judge(SERVER_LOG.replace('"aegis-forum"', '"aegis"'))
        self.assertFalse(passed)
        self.assertIn("get_layer_surface", why)

    def test_an_anchor_without_right_fails(self):
        passed, why = self.judge(SERVER_LOG.replace("set_anchor(13)", "set_anchor(5)"))
        self.assertFalse(passed)
        self.assertIn("set_anchor", why)

    def test_a_missing_exclusive_edge_fails(self):
        log = "\n".join(
            line for line in SERVER_LOG.splitlines() if "set_exclusive_edge" not in line
        )
        passed, why = self.judge(log)
        self.assertFalse(passed)
        self.assertIn("set_exclusive_edge", why)

    def test_a_second_contradicting_request_fails(self):
        log = SERVER_LOG + (
            "[09:54:23.200000] zwlr_layer_surface_v1#28.set_keyboard_interactivity(0)\n"
        )
        passed, why = self.judge(log)
        self.assertFalse(passed)
        self.assertIn("set_keyboard_interactivity", why)

    def test_an_exclusive_zone_off_by_one_fails(self):
        log = SERVER_LOG.replace("set_exclusive_zone(48)", "set_exclusive_zone(47)")
        passed, why = self.judge(log)
        self.assertFalse(passed)
        self.assertIn("set_exclusive_zone", why)

    def test_an_empty_log_fails(self):
        passed, why = self.judge("")
        self.assertFalse(passed)
        self.assertIn("get_layer_surface", why)

    def test_a_shell_that_did_not_configure_fails(self):
        passed, why = self.judge(SERVER_LOG, code=5, stdout="OUTCOME deadline: x\n")
        self.assertFalse(passed)
        self.assertIn("exited 5", why)

    def test_the_case_runs_against_a_debug_compositor_and_tears_it_down(self):
        calls = {}

        class Proc:
            def poll(self):
                return None

        def fake_start(runtime_dir, **kwargs):
            calls["start"] = kwargs
            (runtime_dir / kwargs["log_name"]).write_text(SERVER_LOG, encoding="utf-8")
            return Proc(), runtime_dir / kwargs["socket_name"]

        def fake_runner(argv, env):
            calls["env"] = env
            return 0, CONFIGURED

        def fake_teardown(proc):
            calls["teardown"] = True

        import tempfile

        with tempfile.TemporaryDirectory() as tmp:
            result = verify_shell.run_read_back_case(
                "/bin/shell",
                Path(tmp),
                runner=fake_runner,
                start_fn=fake_start,
                teardown_fn=fake_teardown,
            )
            self.assertEqual(result, ("surface/parameters-read-back", True, ""))
            self.assertEqual(calls["start"]["extra_env"], {"WAYLAND_DEBUG": "server"})
            self.assertIsNone(calls["start"]["dbus_addr"])
            self.assertNotEqual(calls["start"]["socket_name"], "kwin-wayland-0")
            self.assertNotEqual(calls["start"]["log_name"], "kwin.log")
            self.assertEqual(calls["env"]["WAYLAND_DISPLAY"], str(Path(tmp) / "kwin-debug-0"))
            self.assertTrue(calls["teardown"])


class TestRefusedParameters(unittest.TestCase):
    """An unknown schema or layer is refused live, for that reason only (E28-2)."""

    # Measured 2026-10-18 from the shell binary.
    SCHEMA_OUT = (
        "OUTCOME refused: the mount parameters were refused: "
        "the mount request names a schema this build does not admit\n"
    )
    LAYER_OUT = (
        "OUTCOME refused: the mount parameters were refused: "
        "the layer is not background, bottom, top or overlay\n"
    )

    def test_schema_refusal_passes(self):
        self.assertEqual(verify_shell.judge_refused_schema(6, self.SCHEMA_OUT), (True, ""))

    def test_layer_refusal_passes(self):
        self.assertEqual(verify_shell.judge_refused_layer(6, self.LAYER_OUT), (True, ""))

    def test_schema_case_refused_for_the_layer_fails(self):
        passed, why = verify_shell.judge_refused_schema(6, self.LAYER_OUT)
        self.assertFalse(passed)
        self.assertIn("schema", why)

    def test_layer_case_refused_for_the_schema_fails(self):
        passed, why = verify_shell.judge_refused_layer(6, self.SCHEMA_OUT)
        self.assertFalse(passed)
        self.assertIn("layer", why)

    def test_a_mapped_surface_fails(self):
        passed, why = verify_shell.judge_refused_layer(0, CONFIGURED)
        self.assertFalse(passed)
        self.assertIn("exited 0", why)

    def test_the_cases_send_exactly_one_changed_member(self):
        sent = []

        def fake_runner(argv, env):
            if argv[0] == "/bin/shell":
                sent.append(json.loads(argv[-1]))
            return 6, ""

        verify_shell.run_surface_cases("/bin/shell", "/k", "/h", "/d", runner=fake_runner)
        params = [r["params"] for r in sent if r["params"] != verify_shell.REFERENCE_PARAMS]
        self.assertEqual(len(params), 2)
        for changed, (key, value) in zip(
            params,
            (("schema", "aegis.p05-p17.mount-shell-canvas.v2"), ("layer", "middle")),
        ):
            self.assertEqual(changed, dict(verify_shell.REFERENCE_PARAMS, **{key: value}))


class TestLibraryFloors(unittest.TestCase):
    """libxkbcommon and libwayland-client floors checked before cases run (M28 criterion 1)."""

    def test_positive_floors_at_floor_pass(self):
        def fake_runner(argv, timeout):
            if argv[2] == "xkbcommon":
                return 0, f"{XKBCOMMON_FLOOR}\n", ""
            if argv[2] == "wayland-client":
                return 0, f"{WAYLAND_CLIENT_FLOOR}\n", ""
            return 1, "", ""

        status, msg, versions = check_library_floors(runner=fake_runner)
        self.assertEqual(status, "ok")
        self.assertIsNone(msg)
        self.assertEqual(versions["xkbcommon"], XKBCOMMON_FLOOR)
        self.assertEqual(versions["wayland-client"], WAYLAND_CLIENT_FLOOR)

    def test_positive_floors_above_floor_pass(self):
        def fake_runner(argv, timeout):
            if argv[2] == "xkbcommon":
                return 0, "1.13.2\n", ""
            if argv[2] == "wayland-client":
                return 0, "1.26.0\n", ""
            return 1, "", ""

        status, msg, versions = check_library_floors(runner=fake_runner)
        self.assertEqual(status, "ok")
        self.assertIsNone(msg)
        self.assertEqual(versions["xkbcommon"], "1.13.2")
        self.assertEqual(versions["wayland-client"], "1.26.0")

    def test_negative_xkbcommon_below_floor_fails(self):
        def fake_runner(argv, timeout):
            if argv[2] == "xkbcommon":
                return 0, "0.9.0\n", ""
            if argv[2] == "wayland-client":
                return 0, f"{WAYLAND_CLIENT_FLOOR}\n", ""
            return 1, "", ""

        status, msg, _ = check_library_floors(runner=fake_runner)
        self.assertEqual(status, "fail")
        self.assertIn("xkbcommon", msg)
        self.assertIn("0.9.0", msg)
        self.assertIn(XKBCOMMON_FLOOR, msg)

    def test_negative_wayland_client_below_floor_fails(self):
        def fake_runner(argv, timeout):
            if argv[2] == "xkbcommon":
                return 0, f"{XKBCOMMON_FLOOR}\n", ""
            if argv[2] == "wayland-client":
                return 0, "1.10.0\n", ""
            return 1, "", ""

        status, msg, _ = check_library_floors(runner=fake_runner)
        self.assertEqual(status, "fail")
        self.assertIn("wayland-client", msg)
        self.assertIn("1.10.0", msg)
        self.assertIn(WAYLAND_CLIENT_FLOOR, msg)

    def test_boundary_equal_to_floor_passes(self):
        def fake_runner(argv, timeout):
            if argv[2] == "xkbcommon":
                return 0, "1.0.0\n", ""
            if argv[2] == "wayland-client":
                return 0, "1.11.0\n", ""
            return 1, "", ""

        status, msg, versions = check_library_floors(runner=fake_runner)
        self.assertEqual(status, "ok")
        self.assertIsNone(msg)
        self.assertEqual(versions["xkbcommon"], "1.0.0")
        self.assertEqual(versions["wayland-client"], "1.11.0")

    def test_boundary_unparsable_version_fails(self):
        def fake_runner(argv, timeout):
            if argv[2] == "xkbcommon":
                return 0, "1.x.0\n", ""
            if argv[2] == "wayland-client":
                return 0, f"{WAYLAND_CLIENT_FLOOR}\n", ""
            return 1, "", ""

        status, msg, _ = check_library_floors(runner=fake_runner)
        self.assertEqual(status, "fail")
        self.assertIn("xkbcommon", msg)
        self.assertIn("unparsable", msg)

    def test_boundary_missing_xkbcommon_skips(self):
        def fake_runner(argv, timeout):
            if argv[2] == "xkbcommon":
                return 1, "", "Package xkbcommon was not found"
            return 0, "1.26.0\n", ""

        status, msg, _ = check_library_floors(runner=fake_runner)
        self.assertEqual(status, "skip")
        self.assertIn("xkbcommon", msg)

    def test_boundary_missing_wayland_client_skips(self):
        def fake_runner(argv, timeout):
            if argv[2] == "wayland-client":
                return 1, "", "Package wayland-client was not found"
            return 0, "1.13.2\n", ""

        status, msg, _ = check_library_floors(runner=fake_runner)
        self.assertEqual(status, "skip")
        self.assertIn("wayland-client", msg)

    def test_main_exits_zero_on_missing_module_with_skip(self):
        def fake_runner(argv, timeout):
            return 1, "", "Package not found"

        stdout = io.StringIO()
        with patch("sys.stdout", stdout):
            code = main(argv=["--timing-out", "/tmp/nonexistent"], runner=fake_runner)
        self.assertEqual(code, 0)
        self.assertIn("SKIP:", stdout.getvalue())

    def test_main_exits_nonzero_on_below_floor_with_fail(self):
        def fake_runner(argv, timeout):
            if "xkbcommon" in argv:
                return 0, "0.5.0\n", ""
            if "wayland-client" in argv:
                return 0, "1.26.0\n", ""
            return 0, "1.0.0\n", ""

        stdout = io.StringIO()
        with patch("sys.stdout", stdout), patch("verify_shell.capability_reasons", return_value=[]):
            code = main(argv=["--timing-out", "/tmp/nonexistent"], runner=fake_runner)
        self.assertEqual(code, 1)
        self.assertIn("FAIL:", stdout.getvalue())
        self.assertIn("xkbcommon", stdout.getvalue())


if __name__ == "__main__":
    unittest.main()
