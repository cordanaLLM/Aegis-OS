#!/usr/bin/env python3
"""Run the harness self-tests on one platform and prove they actually ran.

HISS-21 permits a gate two states on a platform: running, or skipped with the
reason printed. A suite's exit code alone cannot tell those apart from a third,
prohibited state -- a platform that quietly stopped executing a case and still
reported green.

So this driver asserts two things, not one: the suite exited zero, *and* at
least ``--min-executed`` cases actually ran. A platform that loses coverage then
shows up as a number that moved rather than as an unchanged green check.

The floor is a collapse detector, not a coverage assertion. Raise it to a
platform's measured figure once a green run reports one; never lower it to turn
a red run green.

Every skip is printed with its reason, because a skip whose reason is not stated
is the silent non-run the invariant exists to forbid.
"""

import argparse
import sys
import unittest

# Scalar bound on the reported skip list (HISS-02).
MAX_REPORTED_SKIPS = 64
DEFAULT_MIN_EXECUTED = 250


def parse_arguments(argv):
    """Return the parsed driver options."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--min-executed",
        type=int,
        default=DEFAULT_MIN_EXECUTED,
        help="fail when fewer than this many cases ran (collapse detector)",
    )
    parser.add_argument(
        "--start-directory",
        default="tools",
        help="directory the suite is discovered from",
    )
    return parser.parse_args(argv)


def report_skips(result):
    """Print every skip with the reason it carries, and return how many there were."""
    if not result.skipped:
        print("skips: none on this platform.")
        return 0
    print(f"skips: {len(result.skipped)}; each states what is absent and where it is covered.")
    for case, reason in result.skipped[:MAX_REPORTED_SKIPS]:
        print(f"  SKIP {case.id()}: {reason}")
    return len(result.skipped)


def main(argv):
    """Run the suite, report what ran and what was skipped, and enforce the floor."""
    options = parse_arguments(argv)
    suite = unittest.defaultTestLoader.discover(
        start_dir=options.start_directory, pattern="test_*.py"
    )
    result = unittest.TextTestRunner(verbosity=1, stream=sys.stdout).run(suite)
    executed = result.testsRun
    report_skips(result)
    print(f"executed: {executed} case(s) on {sys.platform}; floor is {options.min_executed}.")

    if not result.wasSuccessful():
        print("FAIL: the suite did not pass on this platform.")
        return 1
    if executed < options.min_executed:
        print(
            f"FAIL: only {executed} case(s) ran, below the floor of {options.min_executed}. "
            "A platform that silently stops executing cases is the state HISS-21 forbids; "
            "investigate the collapse rather than lowering the floor."
        )
        return 1
    print(f"PASS: {executed} case(s) ran and passed on {sys.platform}.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
