#!/usr/bin/env python3
"""Positive, negative and boundary coverage for the portability floor.

The floor in portability_selftest.py is the only thing that turns a platform
which quietly stopped running cases into a red leg. These tests run real
`unittest` suites of known shape through `verdict`, so the figure under test is
the one `unittest` itself reports, skips included.
"""

import contextlib
import io
import unittest

import portability_selftest


class _Fixture:
    """Cases of known outcome, held off module scope so discovery skips them."""

    class Passing(unittest.TestCase):
        def test_runs(self):
            self.assertTrue(True)

    class Skipped(unittest.TestCase):
        def test_skips(self):
            self.skipTest("absent on this host")

    class Failing(unittest.TestCase):
        def test_fails(self):
            self.fail("planted failure")


def run_suite(passing, skipped=0, failing=0):
    """Run a suite of the given shape quietly and return its result."""
    suite = unittest.TestSuite()
    suite.addTests(_Fixture.Passing("test_runs") for _ in range(passing))
    suite.addTests(_Fixture.Skipped("test_skips") for _ in range(skipped))
    suite.addTests(_Fixture.Failing("test_fails") for _ in range(failing))
    return unittest.TextTestRunner(stream=io.StringIO(), verbosity=0).run(suite)


def quiet_verdict(result, floor):
    """Return (exit code, printed text) of `verdict` for `result` against `floor`."""
    printed = io.StringIO()
    with contextlib.redirect_stdout(printed):
        code = portability_selftest.verdict(result, floor)
    return code, printed.getvalue()


class ExecutedCount(unittest.TestCase):
    def test_skips_are_not_counted_as_executed(self):
        result = run_suite(passing=3, skipped=2)
        self.assertEqual(result.testsRun, 5)
        self.assertEqual(portability_selftest.executed_count(result), 3)

    def test_empty_suite_executes_nothing(self):
        self.assertEqual(portability_selftest.executed_count(run_suite(passing=0)), 0)


class Verdict(unittest.TestCase):
    def test_passes_above_the_floor(self):
        code, printed = quiet_verdict(run_suite(passing=5), floor=3)
        self.assertEqual(code, 0)
        self.assertIn("PASS: 5 case(s)", printed)

    def test_passes_exactly_at_the_floor(self):
        code, _ = quiet_verdict(run_suite(passing=4), floor=4)
        self.assertEqual(code, 0)

    def test_fails_one_below_the_floor(self):
        code, printed = quiet_verdict(run_suite(passing=3), floor=4)
        self.assertEqual(code, 1)
        self.assertIn("below the floor of 4", printed)

    def test_a_platform_that_skips_everything_fails_the_floor(self):
        code, printed = quiet_verdict(run_suite(passing=0, skipped=6), floor=1)
        self.assertEqual(code, 1)
        self.assertIn("only 0 case(s) ran", printed)

    def test_skips_cannot_lift_a_short_run_over_the_floor(self):
        code, _ = quiet_verdict(run_suite(passing=3, skipped=1), floor=4)
        self.assertEqual(code, 1)

    def test_a_failing_suite_fails_whatever_the_count(self):
        code, printed = quiet_verdict(run_suite(passing=9, failing=1), floor=1)
        self.assertEqual(code, 1)
        self.assertIn("did not pass", printed)


if __name__ == "__main__":
    unittest.main()
