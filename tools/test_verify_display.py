# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Unit tests for the M32 subtitle verification cases in verify_display.py (E32-1)."""

from pathlib import Path
import unittest
from unittest import mock

from testkit import passing
import verify_display as gate


class TestSubtitleCasesPositive(unittest.TestCase):
    """Positive tests for subtitle cases in verify_display.py."""

    def test_all_subtitle_cases_pass(self):
        """Positive: parsing a child report where all subtitle cases passed yields no problems."""
        stdout = passing(gate.SUBTITLE_CASES)
        _info, cases, result, notes = gate.parse_child(stdout)
        problems = gate.case_problems(cases, gate.SUBTITLE_CASES, notes)
        self.assertEqual(problems, [])
        self.assertEqual(result, "pass")
        for case in gate.SUBTITLE_CASES:
            self.assertTrue(cases.get(case))

    def test_subtitle_run_three_children_success(self):
        """Positive: run_cases executes run, subtitles, and driver-probe, passing all."""
        outputs = [
            (0, passing(gate.RUN_CASES)),
            (0, passing(gate.SUBTITLE_CASES)),
            (0, passing(gate.PROBE_CASES)),
        ]
        with mock.patch.object(gate, "run_child", side_effect=outputs) as child:
            skipped, failures, _info = gate.run_cases(Path("/bin/true"))
        self.assertIsNone(skipped)
        self.assertEqual(failures, [])
        drivers = [call.args[2] for call in child.call_args_list]
        self.assertEqual(drivers, ["iHD", "iHD", "nvidia"])
        modes = [call.args[1] for call in child.call_args_list]
        self.assertEqual(modes, ["run", "subtitles", "driver-probe"])

    def test_fixture_problems_clean_on_committed_fixtures(self):
        """Positive: fixture_problems checks the committed mjpeg fixture cleanly."""
        self.assertEqual(gate.fixture_problems(), [])

    def test_subtitle_cases_tuple_structure(self):
        """Positive: SUBTITLE_CASES contains exactly the four required cases."""
        expected = (
            "subtitle/cue-1-within-100ms",
            "subtitle/cue-2-within-100ms",
            "subtitle/cue-3-within-100ms",
            "subtitle/planted-150ms-late-refused",
        )
        self.assertEqual(gate.SUBTITLE_CASES, expected)


class TestSubtitleCasesNegative(unittest.TestCase):
    """Negative tests for subtitle cases in verify_display.py."""

    def test_one_subtitle_case_failure_reported_as_problem(self):
        """Negative: a FAIL line for a subtitle case is collected in case_problems."""
        output = (
            "PASS subtitle/cue-1-within-100ms\n"
            "FAIL subtitle/cue-2-within-100ms\n"
            "     offset 1250 ms differs by 150 ms from cue start 1100 ms\n"
            "PASS subtitle/cue-3-within-100ms\n"
            "PASS subtitle/planted-150ms-late-refused\n"
            "RESULT fail: 1 case(s) failed\n"
        )
        _info, cases, result, notes = gate.parse_child(output)
        problems = gate.case_problems(cases, gate.SUBTITLE_CASES, notes)
        self.assertEqual(problems, ["subtitle/cue-2-within-100ms failed"])
        self.assertEqual(result, "fail: 1 case(s) failed")

    def test_subtitle_child_exit_non_zero_fails_gate(self):
        """Negative: non-zero exit from subtitles child reports failure even if cases pass."""
        outputs = [
            (0, passing(gate.RUN_CASES)),
            (3, passing(gate.SUBTITLE_CASES)),
            (0, passing(gate.PROBE_CASES)),
        ]
        with mock.patch.object(gate, "run_child", side_effect=outputs):
            _skipped, failures, _info = gate.run_cases(Path("/bin/true"))
        self.assertEqual(len(failures), 1)
        self.assertIn("subtitles exited 3", failures[0])

    def test_subtitle_fixture_problem_reaches_gate_problems(self):
        """Negative: a subtitle fixture problem reaches the gate's problems."""
        with (
            mock.patch.object(gate.display_toolchain, "check_toolchain", return_value=[]),
            mock.patch.object(gate.display_toolchain, "check_locked", return_value=[]),
            mock.patch.object(gate, "fixture_problems", return_value=[]),
            mock.patch.object(
                gate.subtitle_fixture,
                "fixture_problems",
                return_value=["x"],
            ),
        ):
            with self.assertRaises(gate.GateError) as ctx:
                gate.prepare()
            self.assertIn("x", str(ctx.exception))


class TestSubtitleCasesBoundary(unittest.TestCase):
    """Boundary tests for subtitle cases in verify_display.py."""

    def test_missing_subtitle_case_reports_did_not_report(self):
        """Boundary: an omitted subtitle case reports 'did not report'."""
        output = (
            "PASS subtitle/cue-1-within-100ms\n"
            "PASS subtitle/cue-2-within-100ms\n"
            "PASS subtitle/planted-150ms-late-refused\n"
            "RESULT pass\n"
        )
        _info, cases, _result, notes = gate.parse_child(output)
        problems = gate.case_problems(cases, gate.SUBTITLE_CASES, notes)
        self.assertEqual(problems, ["subtitle/cue-3-within-100ms did not report"])

    def test_empty_output_reports_all_subtitle_cases_did_not_report(self):
        """Boundary: empty stdout reports all required subtitle cases as 'did not report'."""
        _info, cases, _result, notes = gate.parse_child("")
        problems = gate.case_problems(cases, gate.SUBTITLE_CASES, notes)
        expected = [f"{case} did not report" for case in gate.SUBTITLE_CASES]
        self.assertEqual(problems, expected)

    def test_subtitle_skip_propagates_as_gate_skip(self):
        """Boundary: exit code EXIT_SKIP from subtitles child propagates skip reason."""
        outputs = [
            (0, passing(gate.RUN_CASES)),
            (gate.EXIT_SKIP, "RESULT skip: compositor dropped connection\n"),
        ]
        with mock.patch.object(gate, "run_child", side_effect=outputs):
            skipped, failures, _info = gate.run_cases(Path("/bin/true"))
        self.assertEqual(skipped, "compositor dropped connection")
        self.assertEqual(failures, [])


if __name__ == "__main__":
    unittest.main()
