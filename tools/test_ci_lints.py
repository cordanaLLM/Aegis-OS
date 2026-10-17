#!/usr/bin/env python3
"""Positive, negative and boundary coverage for the CI lint pins.

The reader is fed workflow text of known shape and the launcher check takes a
stand-in `which`, so no case downloads a linter and every case runs on each
platform the Platform Neutrality matrix covers. One case reads the committed
workflow and one reads the Makefile, so a renamed or duplicated pin, or a
recipe that stops asking for one, fails here first.
"""

import contextlib
import io
import unittest

import ci_lints

WORKFLOW = "\n".join(
    (
        "env:",
        '  LEFTHOOK_VERSION: "2.1.12"',
        '  REUSE_VERSION: "6.2.0"',
        '  MARKDOWNLINT_VERSION: "0.23.2"',
        '  YAMLLINT_VERSION: "1.38.0"',
        '  FLAKE8_VERSION: "7.3.0"',
        '  BLACK_VERSION: "26.5.1"',
        "",
    )
)


def present(*names):
    """A `which` that finds only `names`."""
    return lambda name: name if name in names else None


class VersionTest(unittest.TestCase):
    def test_every_pin_is_read_from_the_env_block(self):
        self.assertEqual(
            ci_lints.versions(WORKFLOW),
            {
                "REUSE_VERSION": "6.2.0",
                "MARKDOWNLINT_VERSION": "0.23.2",
                "YAMLLINT_VERSION": "1.38.0",
                "FLAKE8_VERSION": "7.3.0",
                "BLACK_VERSION": "26.5.1",
            },
        )

    def test_the_committed_workflow_pins_every_lint_once(self):
        self.assertEqual(sorted(ci_lints.versions(ci_lints.workflow_text())), sorted(ci_lints.KEYS))

    def test_the_makefile_asks_for_every_pin(self):
        makefile = (ci_lints.ROOT / "Makefile").read_text(encoding="utf-8")
        for key in ci_lints.KEYS:
            self.assertIn(f"tools/ci_lints.py pin {key})", makefile)
        self.assertIn("python3 tools/ci_lints.py preflight", makefile)

    def test_a_missing_pin_is_refused(self):
        text = WORKFLOW.replace('  BLACK_VERSION: "26.5.1"\n', "")
        with self.assertRaisesRegex(ci_lints.LintError, "BLACK_VERSION 0 times"):
            ci_lints.versions(text)

    def test_a_pin_set_twice_is_refused(self):
        text = WORKFLOW + '      YAMLLINT_VERSION: "1.0.0"\n'
        with self.assertRaisesRegex(ci_lints.LintError, "YAMLLINT_VERSION 2 times"):
            ci_lints.versions(text)

    def test_an_empty_pin_is_refused(self):
        text = WORKFLOW.replace('"7.3.0"', '""')
        with self.assertRaisesRegex(ci_lints.LintError, "FLAKE8_VERSION to an empty value"):
            ci_lints.versions(text)

    def test_a_prefixed_key_is_not_read_as_the_pin(self):
        text = WORKFLOW.replace("  REUSE_VERSION:", "  OLD_REUSE_VERSION:")
        with self.assertRaisesRegex(ci_lints.LintError, "REUSE_VERSION 0 times"):
            ci_lints.versions(text)


class PinTest(unittest.TestCase):
    def test_a_lint_key_returns_its_version(self):
        self.assertEqual(ci_lints.pin(WORKFLOW, "MARKDOWNLINT_VERSION"), "0.23.2")

    def test_a_key_outside_the_lints_is_refused(self):
        with self.assertRaisesRegex(ci_lints.LintError, "LEFTHOOK_VERSION is not a CI lint pin"):
            ci_lints.pin(WORKFLOW, "LEFTHOOK_VERSION")


class PreflightTest(unittest.TestCase):
    def test_every_pin_and_launcher_present_passes(self):
        line = ci_lints.preflight(WORKFLOW, which=present(*ci_lints.LAUNCHERS))
        self.assertIn("BLACK_VERSION=26.5.1", line)
        self.assertTrue(line.startswith("PASS: "))

    def test_a_missing_launcher_is_named(self):
        with self.assertRaisesRegex(ci_lints.LintError, "not on PATH: npx, shellcheck;"):
            ci_lints.preflight(WORKFLOW, which=present("git", "pipx"))

    def test_no_launcher_at_all_names_every_one(self):
        self.assertEqual(ci_lints.missing_launchers(present()), list(ci_lints.LAUNCHERS))

    def test_a_broken_pin_fails_before_the_launchers_are_checked(self):
        text = WORKFLOW.replace("  FLAKE8_VERSION:", "  FLAKE9_VERSION:")
        with self.assertRaisesRegex(ci_lints.LintError, "FLAKE8_VERSION 0 times"):
            ci_lints.preflight(text, which=present(*ci_lints.LAUNCHERS))


class MainTest(unittest.TestCase):
    def run_main(self, argv):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = ci_lints.main(argv)
        return code, out.getvalue(), err.getvalue()

    def test_pin_prints_the_committed_version(self):
        code, out, _err = self.run_main(["pin", "YAMLLINT_VERSION"])
        expected = ci_lints.ci_env(ci_lints.workflow_text(), "YAMLLINT_VERSION")
        self.assertEqual((code, out), (0, expected + "\n"))

    def test_an_unknown_key_exits_one_and_prints_nothing_to_stdout(self):
        code, out, err = self.run_main(["pin", "NOPE"])
        self.assertEqual((code, out), (1, ""))
        self.assertIn("FAIL: NOPE is not a CI lint pin", err)

    def test_no_arguments_print_the_usage(self):
        self.assertEqual(self.run_main([])[0], 2)

    def test_extra_arguments_print_the_usage(self):
        self.assertEqual(self.run_main(["pin", "REUSE_VERSION", "x"])[0], 2)


if __name__ == "__main__":
    unittest.main()
