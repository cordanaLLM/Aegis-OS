"""Regressions for the display slice gate (M27): the half that needs no GPU.

`make verify-display` needs a GPU with the iHD VA driver, a Wayland session and
the reference profile's compositor. These tests need none of them. They hold
the gate's own decisions -- the fixture's pinned sha256 and recorded
provenance, the environment each child gets, when the gate skips and what it
prints then, how it reads a child's report, which programs it may start at all
and how it names the compositor without executing it -- so a checkout without
the hardware still fails `make verify-all` when one of those drifts.
"""

import ast
import hashlib
import io
import os
import re
import shutil
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import verify_display as gate

ROOT = Path(__file__).resolve().parent.parent
GATE = ROOT / "tools" / "verify_display.py"
SLICE = ROOT / "crates" / "aegis-scaena" / "src" / "slice.rs"
MAKEFILE = ROOT / "Makefile"
PAGE = ROOT / "docs" / "build" / "display.md"

# The committed Motion-JPEG fixture's sha256, pinned here independently of the
# gate so that a change to either is caught (M27 criterion 5).
FIXTURE_SHA256 = "fd45d15a2837113fc2d581f04a9b82ca7992a6d2b8940f20ab9cc95fe6767be5"
SKIP_SUFFIX = "; the display slice gate did not run."
MAX_SOURCE_LINES = 4000

RUN_OUTPUT = """Display slice (M27, D79): development evidence, client half only.
info compositor pid: 3089
PASS display/attach-before-ack-refused
     refused before the first ack_configure
info VA vendor string: Intel iHD driver for Intel(R) Gen Graphics - 26.2.4 ()
PASS display/driver-is-ihd
FAIL display/mpeg2-reference-crc
     the reference MPEG-2 frame decoded to CRC-32 0x00000000
info modifier the driver chose: 0x0100000000000009
RESULT fail: 1 case(s) failed
"""


def passing(cases, replace=None, line=""):
    """A child report in which every case of `cases` passed.

    With `replace`, that case's PASS line is `line` instead, the way the binary
    prints a NOTE or SKIP line in its place and still ends with RESULT pass.
    """
    lines = [line if case == replace else f"PASS {case}" for case in cases]
    return "\n".join(lines + ["RESULT pass"]) + "\n"


PIN = "display/frame-crc-regression-pin"
OTHER_NODE = "display/other-render-node-refused"
PIN_NOTE = (
    f'NOTE {PIN}: pins recorded under "Intel iHD driver for Intel(R) Gen Graphics - 26.2.4 ()"; '
    'this run\'s driver is "Intel iHD driver for Intel(R) Gen Graphics - 26.3.0 ()"; '
    "7 of 60 differ; re-measure and record them with the driver version (D73)"
)
NODE_SKIP = f"SKIP {OTHER_NODE}: this host has one render node"


class FixturePinTests(unittest.TestCase):
    """The committed fixture is the pinned one, and its provenance is recorded."""

    def test_the_committed_fixture_matches_its_pin(self):
        """Positive: the file hashes to the value pinned here and in the gate."""
        data = gate.FIXTURE.read_bytes()
        self.assertEqual(hashlib.sha256(data).hexdigest(), FIXTURE_SHA256)
        self.assertEqual(gate.FIXTURE_SHA256, FIXTURE_SHA256)
        self.assertEqual(gate.fixture_problems(), [])

    def test_a_changed_fixture_is_refused_naming_its_digest(self):
        """Negative: one flipped byte is a different fixture."""
        with tempfile.TemporaryDirectory() as scratch:
            copy = Path(scratch) / gate.FIXTURE.name
            data = bytearray(gate.FIXTURE.read_bytes())
            data[1000] ^= 0x01
            copy.write_bytes(bytes(data))
            problems = gate.fixture_problems(copy)
        self.assertEqual(len(problems), 1)
        self.assertIn("not the pinned", problems[0])
        self.assertIn(hashlib.sha256(bytes(data)).hexdigest(), problems[0])

    def test_a_missing_or_linked_fixture_is_refused(self):
        """Negative: absence and a symlink are both refusals, not passes."""
        with tempfile.TemporaryDirectory() as scratch:
            missing = Path(scratch) / "absent.mjpeg"
            self.assertTrue(gate.fixture_problems(missing))
            link = Path(scratch) / "link.mjpeg"
            try:
                link.symlink_to(gate.FIXTURE)
            except OSError as error:  # Windows without the symlink privilege
                self.skipTest(
                    f"this host cannot create a symlink ({error.strerror}); Linux runs it"
                )
            self.assertIn("symlink", gate.fixture_problems(link)[0])

    def test_one_byte_short_is_refused_by_length(self):
        """Boundary: the length is held before the digest is computed."""
        with tempfile.TemporaryDirectory() as scratch:
            short = Path(scratch) / "short.mjpeg"
            short.write_bytes(gate.FIXTURE.read_bytes()[:-1])
            problems = gate.fixture_problems(short)
        self.assertEqual(
            problems,
            [f"short.mjpeg is {gate.FIXTURE_BYTES - 1} bytes, not the pinned {gate.FIXTURE_BYTES}"],
        )

    def test_the_recorded_command_draws_the_index_row_and_is_never_run(self):
        """Positive: the provenance draws two guards and six bits; the gate never runs ffmpeg."""
        command = gate.FIXTURE_COMMAND
        self.assertEqual(command[0], "ffmpeg")
        self.assertIn("+bitexact", command)
        self.assertEqual(command[command.index("-frames:v") + 1], "60")
        self.assertEqual(gate.FIXTURE_FILTER.count("drawbox="), 8)
        for bit in range(6):
            self.assertIn(f"floor((n+1)/{1 << bit})", gate.FIXTURE_FILTER)
        self.assertNotIn("ffmpeg", gate.PROGRAMS)


class ChildEnvironmentTests(unittest.TestCase):
    """LIBVA_DRIVER_NAME exists only in the environment handed to one child."""

    def test_the_positive_child_gets_ihd(self):
        """Positive: the copy carries iHD."""
        env = gate.child_environment("iHD", {"PATH": "/usr/bin"})
        self.assertEqual(env["LIBVA_DRIVER_NAME"], "iHD")
        self.assertEqual(env["PATH"], "/usr/bin")

    def test_the_gates_own_environment_is_never_changed(self):
        """Negative: building a child's environment leaves the parent's alone."""
        before = dict(os.environ)
        gate.child_environment("nvidia")
        self.assertEqual(dict(os.environ), before)

    def test_the_sessions_own_value_is_overridden_only_in_the_copy(self):
        """Boundary: a session that exports nvidia keeps it; the child gets iHD."""
        session = {"LIBVA_DRIVER_NAME": "nvidia"}
        env = gate.child_environment("iHD", session)
        self.assertEqual(env["LIBVA_DRIVER_NAME"], "iHD")
        self.assertEqual(session["LIBVA_DRIVER_NAME"], "nvidia")


class CapabilityTests(unittest.TestCase):
    """HISS-21: a host without the capabilities skips with its reason."""

    def setUp(self):
        self.scratch = tempfile.TemporaryDirectory()
        base = Path(self.scratch.name)
        self.drm = base / "drm"
        (self.drm / "renderD129").mkdir(parents=True)
        (self.drm / "renderD130").mkdir()
        (base / "run").mkdir()
        (base / "run" / "wayland-0").write_text("", encoding="utf-8")
        (base / "dri").mkdir()
        (base / "dri" / gate.IHD_DRIVER_FILE).write_text("", encoding="utf-8")
        self.environ = {
            "WAYLAND_DISPLAY": "wayland-0",
            "XDG_RUNTIME_DIR": str(base / "run"),
            "LIBVA_DRIVERS_PATH": str(base / "dri"),
        }

    def tearDown(self):
        self.scratch.cleanup()

    def reasons(self, environ=None, platform="linux", which=lambda name: f"/usr/bin/{name}"):
        return gate.capability_reasons(
            platform=platform,
            environ=self.environ if environ is None else environ,
            which=which,
            drm_root=self.drm,
        )

    def test_a_host_with_every_capability_has_no_reason(self):
        """Positive: socket, two render nodes, iHD driver and cargo present."""
        self.assertEqual(self.reasons(), [])

    def test_macos_and_windows_skip_with_their_platform_named(self):
        """Negative: off Linux there is exactly one reason, and it names the host."""
        for platform in ("darwin", "win32"):
            reasons = self.reasons(platform=platform)
            self.assertEqual(len(reasons), 1)
            self.assertIn(platform, reasons[0])

    def test_each_missing_capability_is_named(self):
        """Negative: no session, no node, no driver, no cargo each give a reason."""
        self.assertIn("Wayland", self.reasons(dict(self.environ, WAYLAND_DISPLAY=""))[0])
        shutil.rmtree(self.drm / "renderD129")
        shutil.rmtree(self.drm / "renderD130")
        self.assertIn("no DRM render node", self.reasons()[0])
        (self.drm / "renderD129").mkdir()
        (self.drm / "renderD130").mkdir()
        no_driver = dict(self.environ, LIBVA_DRIVERS_PATH="/nonexistent")
        with mock.patch.object(gate, "DRIVER_DIRECTORIES", ("/nonexistent",)):
            self.assertIn(gate.IHD_DRIVER_FILE, self.reasons(no_driver)[0])
        missing = self.reasons(which=lambda name: None)
        self.assertEqual(missing, ["cargo is not on PATH", "rustc is not on PATH"])

    def test_one_render_node_skips_and_a_second_one_runs(self):
        """Boundary: one node has none for negative (b) to refuse; two are enough."""
        shutil.rmtree(self.drm / "renderD130")
        reasons = self.reasons()
        self.assertEqual(len(reasons), 1)
        self.assertIn("one DRM render node", reasons[0])
        self.assertIn("renderD129", reasons[0])
        self.assertIn("negative (b)", reasons[0])
        (self.drm / "renderD130").mkdir()
        self.assertEqual(self.reasons(), [])

    def test_an_absolute_socket_path_is_honoured_and_a_missing_runtime_dir_is_not(self):
        """Boundary: WAYLAND_DISPLAY may be absolute; a relative one needs XDG_RUNTIME_DIR."""
        socket = Path(self.environ["XDG_RUNTIME_DIR"]) / "wayland-0"
        absolute = dict(self.environ, WAYLAND_DISPLAY=str(socket), XDG_RUNTIME_DIR="")
        self.assertEqual(self.reasons(absolute), [])
        relative = dict(self.environ, XDG_RUNTIME_DIR="")
        self.assertIn("Wayland", self.reasons(relative)[0])

    def test_the_driver_path_list_is_split_the_way_libva_splits_it(self):
        """Boundary: the driver in the second of two entries is found; an empty entry is skipped."""
        base = Path(self.scratch.name)
        (base / "empty").mkdir()
        search = os.pathsep.join(("", str(base / "empty"), self.environ["LIBVA_DRIVERS_PATH"]))
        with mock.patch.object(gate, "DRIVER_DIRECTORIES", ()):
            found = gate.ihd_driver({"LIBVA_DRIVERS_PATH": search})
            self.assertEqual(found, base / "dri" / gate.IHD_DRIVER_FILE)
            self.assertIsNone(gate.ihd_driver({"LIBVA_DRIVERS_PATH": str(base / "empty")}))

    def test_a_skipped_run_prints_the_convention_and_exits_zero(self):
        """Positive: the SKIP line has the criterion's exact shape, and exit 0."""
        output = io.StringIO()
        with mock.patch.object(gate, "capability_reasons", return_value=["this host is darwin"]):
            with redirect_stdout(output):
                code = gate.main()
        self.assertEqual(code, 0)
        self.assertIn(f"SKIP: this host is darwin{SKIP_SUFFIX}", output.getvalue())
        self.assertNotIn("PASS", output.getvalue())


class SessionLockTests(unittest.TestCase):
    """A locked session presents no client surface, so the gate skips."""

    def test_an_unlocked_session_runs(self):
        """Positive: LockedHint=no is no reason."""
        runner = mock.Mock(return_value=(0, "no\n", ""))
        self.assertIsNone(
            gate.session_locked({"XDG_SESSION_ID": "3"}, runner, lambda _: "/usr/bin/loginctl")
        )
        self.assertEqual(runner.call_args[0][0][:3], ["loginctl", "show-session", "3"])

    def test_a_locked_session_skips_with_the_session_named(self):
        """Negative: LockedHint=yes is a reason naming the session."""
        runner = mock.Mock(return_value=(0, "yes\n", ""))
        reason = gate.session_locked({"XDG_SESSION_ID": "3"}, runner, lambda _: "/usr/bin/loginctl")
        self.assertIn("session 3 locked", reason)

    def test_without_loginctl_or_an_answer_the_run_proceeds(self):
        """Boundary: no loginctl and a failed query are not reasons to skip."""
        runner = mock.Mock(return_value=(1, "", "no session"))
        self.assertIsNone(gate.session_locked({}, runner, lambda _: "/usr/bin/loginctl"))
        self.assertEqual(runner.call_args[0][0][2], "auto")
        self.assertIsNone(gate.session_locked({}, runner, lambda _: None))


class ChildReportTests(unittest.TestCase):
    """The gate reads each child's cases and requires every one to pass."""

    def test_facts_cases_and_result_are_read(self):
        """Positive: info lines, case lines and the result line."""
        info, cases, result, _notes = gate.parse_child(RUN_OUTPUT)
        self.assertEqual(info["compositor pid"], "3089")
        self.assertEqual(info["modifier the driver chose"], "0x0100000000000009")
        self.assertTrue(cases["display/driver-is-ihd"])
        self.assertFalse(cases["display/mpeg2-reference-crc"])
        self.assertEqual(result, "fail: 1 case(s) failed")

    def test_a_failed_or_missing_case_is_a_problem(self):
        """Negative: a FAIL and a case that never reported both fail the gate."""
        _info, cases, _result, _notes = gate.parse_child(RUN_OUTPUT)
        problems = gate.case_problems(cases, gate.RUN_CASES)
        self.assertIn("display/mpeg2-reference-crc failed", problems)
        self.assertIn("display/fixture-frames-present did not report", problems)

    def test_every_required_case_passing_is_no_problem(self):
        """Boundary: exactly the required cases, all passed, is a pass."""
        _info, cases, _result, _notes = gate.parse_child(passing(gate.RUN_CASES))
        self.assertEqual(gate.case_problems(cases, gate.RUN_CASES), [])

    def test_a_child_that_skips_is_a_skip_not_a_pass(self):
        """Negative: exit 2 with a skip result stops the gate before the probe."""
        with mock.patch.object(
            gate, "run_child", return_value=(2, "RESULT skip: no layer shell\n")
        ) as child:
            skipped, failures, _info = gate.run_cases(Path("/bin/true"))
        self.assertEqual(skipped, "no layer shell")
        self.assertEqual(failures, [])
        self.assertEqual(child.call_count, 1)

    def test_both_children_run_with_their_drivers(self):
        """Positive: iHD for the run, nvidia for the probe, and both must pass."""
        outputs = [(0, passing(gate.RUN_CASES)), (0, passing(gate.PROBE_CASES))]
        with mock.patch.object(gate, "run_child", side_effect=outputs) as child:
            skipped, failures, _info = gate.run_cases(Path("/bin/true"))
        self.assertIsNone(skipped)
        self.assertEqual(failures, [])
        drivers = [call.args[2] for call in child.call_args_list]
        self.assertEqual(drivers, ["iHD", "nvidia"])

    def test_a_probe_child_that_fails_its_case_fails_the_gate(self):
        """Negative (a): a FAIL of driver-not-ihd-refused is a failure of the gate."""
        probe = "FAIL display/driver-not-ihd-refused\nRESULT fail: 1 case(s) failed\n"
        outputs = [(0, passing(gate.RUN_CASES)), (1, probe)]
        with mock.patch.object(gate, "run_child", side_effect=outputs):
            _skipped, failures, _info = gate.run_cases(Path("/bin/true"))
        self.assertEqual(failures, ["display/driver-not-ihd-refused failed"])

    def test_a_child_that_exits_non_zero_after_passing_every_case_fails(self):
        """Negative: every case PASS is not enough when either child exits non-zero."""
        for codes, expected in (((3, 0), "run exited 3"), ((0, 4), "driver-probe exited 4")):
            outputs = [(codes[0], passing(gate.RUN_CASES)), (codes[1], passing(gate.PROBE_CASES))]
            with mock.patch.object(gate, "run_child", side_effect=outputs):
                _skipped, failures, _info = gate.run_cases(Path("/bin/true"))
            self.assertEqual(len(failures), 1, failures)
            self.assertIn(expected, failures[0])

    def test_pins_under_another_driver_are_reported_not_held(self):
        """Positive (D73): a NOTE for the regression pin passes and is named not held."""
        outputs = [
            (0, passing(gate.RUN_CASES, PIN, PIN_NOTE)),
            (0, passing(gate.PROBE_CASES)),
        ]
        with mock.patch.object(gate, "run_child", side_effect=outputs):
            skipped, failures, info = gate.run_cases(Path("/bin/true"))
        self.assertIsNone(skipped)
        self.assertEqual(failures, [])
        self.assertEqual(info["not held"], [PIN])
        output = io.StringIO()
        with redirect_stdout(output):
            gate.summary(info, ("rustc 1.98.1", ("2.24.1", "1.24.0")))
        self.assertIn(f"{PIN}: reported, not held", output.getvalue())

    def test_a_case_the_binary_skipped_fails_naming_its_reason(self):
        """Negative: a SKIP line in a case's place fails with its reason, not 'did not report'."""
        _info, cases, _result, notes = gate.parse_child(
            passing(gate.RUN_CASES, OTHER_NODE, NODE_SKIP)
        )
        self.assertEqual(
            gate.case_problems(cases, gate.RUN_CASES, notes),
            [f"{OTHER_NODE} did not run: this host has one render node"],
        )
        self.assertEqual(gate.not_held(cases, notes), [])

    def test_only_the_pin_may_be_noted_and_a_note_never_overrides_a_fail(self):
        """Boundary: a NOTE in another case's place fails; a FAILed pin with a NOTE still fails."""
        other = "display/driver-is-ihd"
        _info, cases, _result, notes = gate.parse_child(
            passing(gate.RUN_CASES, other, f"NOTE {other}: something else")
        )
        self.assertEqual(
            gate.case_problems(cases, gate.RUN_CASES, notes),
            [f"{other} did not run: something else"],
        )
        _info, cases, _result, notes = gate.parse_child(
            passing(gate.RUN_CASES, PIN, f"FAIL {PIN}\n{PIN_NOTE}")
        )
        self.assertEqual(gate.case_problems(cases, gate.RUN_CASES, notes), [f"{PIN} failed"])
        self.assertEqual(gate.not_held(cases, notes), [])

    def test_the_note_and_skip_lines_are_the_ones_the_binary_prints(self):
        """Boundary: the binary notes only the pin and skips only cases the gate requires."""
        source = SLICE.read_text(encoding="utf-8")
        noted = set(re.findall(r'"NOTE (display/[a-z0-9-]+): ', source))
        skipped = set(re.findall(r'"SKIP (display/[a-z0-9-]+): ', source))
        self.assertEqual(noted, set(gate.NOTED_CASES))
        self.assertEqual(skipped, {OTHER_NODE})
        self.assertTrue(set(gate.NOTED_CASES) <= set(gate.RUN_CASES))

    def test_the_required_cases_are_the_ones_the_binary_reports(self):
        """Boundary: the gate requires exactly the case names the Rust slice prints."""
        source = SLICE.read_text(encoding="utf-8")
        printed = set(re.findall(r'"(display/[a-z0-9-]+)"', source))
        required = set(gate.RUN_CASES) | set(gate.PROBE_CASES)
        self.assertEqual(printed, required)


class ProgramTests(unittest.TestCase):
    """Only listed programs start, every one under a deadline, and never the compositor."""

    def test_an_unlisted_program_is_refused_before_it_starts(self):
        """Negative: ffmpeg, the compositor and a shell are not programs of the gate."""
        for argv in (
            ["ffmpeg", "-version"],
            ["/usr/bin/kwin_wayland_wrapper", "--version"],
            ["sh", "-c", "true"],
        ):
            with mock.patch.object(gate.subprocess, "run") as started:
                with self.assertRaises(gate.GateError):
                    gate.run(argv, 1)
                started.assert_not_called()

    def test_pacman_is_only_ever_asked_who_owns_a_file(self):
        """Positive: every pacman argument vector in the gate is a -Qo query."""
        tree = ast.parse(GATE.read_text(encoding="utf-8"))
        vectors = [
            node
            for node in ast.walk(tree)
            if isinstance(node, ast.List)
            and node.elts
            and isinstance(node.elts[0], ast.Constant)
            and node.elts[0].value == "pacman"
        ]
        self.assertTrue(vectors)
        for vector in vectors:
            self.assertEqual(vector.elts[1].value, "-Qo")

    def test_only_run_starts_a_process_and_it_carries_a_deadline(self):
        """Boundary: one subprocess call site, inside run(), with timeout=."""
        tree = ast.parse(GATE.read_text(encoding="utf-8"))
        sites = []
        for function in ast.walk(tree):
            if not isinstance(function, ast.FunctionDef):
                continue
            for node in ast.walk(function):
                if (
                    isinstance(node, ast.Call)
                    and isinstance(node.func, ast.Attribute)
                    and isinstance(node.func.value, ast.Name)
                    and node.func.value.id in {"subprocess", "os"}
                    and node.func.attr in {"run", "Popen", "system", "popen", "execv", "execvp"}
                ):
                    sites.append((function.name, {keyword.arg for keyword in node.keywords}))
        self.assertEqual(len(sites), 1)
        self.assertEqual(sites[0][0], "run")
        self.assertIn("timeout", sites[0][1])

    def test_the_compositor_is_named_without_being_executed(self):
        """Positive: its package comes from pacman -Qo; the compositor never runs."""
        runner = mock.Mock(
            return_value=(0, "/usr/bin/kwin_wayland_wrapper is owned by kwin 6.7.5-1.1\n", "")
        )
        with mock.patch.object(gate.os, "readlink", return_value="/usr/bin/kwin_wayland_wrapper"):
            named = gate.compositor("3089", runner, lambda _: "/usr/bin/pacman")
        self.assertEqual(named, "/usr/bin/kwin_wayland_wrapper (pid 3089, package kwin 6.7.5-1.1)")
        self.assertEqual(runner.call_args[0][0], ["pacman", "-Qo", "/usr/bin/kwin_wayland_wrapper"])

    def test_an_unreported_peer_is_unidentified(self):
        """Boundary: no pid, or not a number, names nothing and runs nothing."""
        runner = mock.Mock()
        self.assertIn("unidentified", gate.compositor(None, runner))
        self.assertIn("unidentified", gate.compositor("unreported", runner))
        runner.assert_not_called()

    def test_modifiers_are_named(self):
        """Positive: the chosen modifier prints with its DRM name; an unknown one says so."""
        self.assertEqual(
            gate.modifier_name("0x0100000000000009"), "0x0100000000000009 (I915_FORMAT_MOD_4_TILED)"
        )
        self.assertIn("unnamed here", gate.modifier_name("0x0100000000000007"))
        self.assertEqual(gate.modifier_name("unread"), "unread")


class RunRecordTests(unittest.TestCase):
    """Every run has an identifier, and its logs stay outside the repository."""

    def test_a_run_id_is_the_utc_start_and_a_random_suffix(self):
        """Positive: the shape the other hardware gates' run ids have."""
        self.assertRegex(gate.run_id(), r"^r\d{8}T\d{6}-[0-9a-f]{4}$")

    def test_logs_never_land_in_the_repository(self):
        """Negative: the default and the XDG cache are both outside the checkout."""
        for environ in ({}, {"XDG_CACHE_HOME": "/var/cache/someone"}):
            directory = gate.retain_dir(environ)
            self.assertEqual(directory.name, "aegis-display")
            self.assertNotIn(str(ROOT), str(directory))

    def test_the_override_wins(self):
        """Boundary: AEGIS_DISPLAY_DIR is taken as given, over XDG_CACHE_HOME."""
        environ = {"AEGIS_DISPLAY_DIR": "/srv/display", "XDG_CACHE_HOME": "/var/cache/x"}
        self.assertEqual(gate.retain_dir(environ), Path("/srv/display"))


class TargetTests(unittest.TestCase):
    """verify-display exists, runs this gate and stays out of verify-all."""

    def recipe(self, target):
        lines = MAKEFILE.read_text(encoding="utf-8").splitlines()[:MAX_SOURCE_LINES]
        start = lines.index(f"{target}:")
        body = []
        for line in lines[start + 1 :]:
            if not line.startswith("\t"):
                break
            body.append(line)
        return body

    def test_the_target_runs_the_gate(self):
        """Positive: make verify-display runs tools/verify_display.py."""
        self.assertEqual(self.recipe("verify-display"), ["\tpython3 tools/verify_display.py"])

    def test_verify_all_does_not_run_it(self):
        """Negative: the hardware gate is not part of verify-all."""
        recipe = "\n".join(self.recipe("verify-all"))
        self.assertNotIn("verify-display", recipe)
        self.assertNotIn("verify_display", recipe)

    def test_the_page_records_the_pin_and_the_cases(self):
        """Boundary: docs/build/display.md carries the pin, the command and every case."""
        page = PAGE.read_text(encoding="utf-8")
        self.assertIn(FIXTURE_SHA256, page)
        self.assertIn("-huffman default", page)
        for case in gate.RUN_CASES + gate.PROBE_CASES:
            self.assertIn(case, page)


if __name__ == "__main__":
    unittest.main()
