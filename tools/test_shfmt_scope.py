#!/usr/bin/env python3
"""Positive, negative and boundary coverage for the shfmt driver.

Every case feeds .editorconfig text of known shape and a stand-in runner, so no
case starts shfmt and every case runs on each platform the Platform Neutrality
matrix covers. One case reads the committed .editorconfig and the Makefile, so
an ignore the driver refuses, or a recipe that stops calling the driver with
the pinned shfmt, fails here first.
"""

import datetime
import unittest

import shfmt_scope

TODAY = datetime.date(2026, 10, 18)
TRACKED = ["a.sh", "hooks/managed.sh"]
COMMAND = ["shfmt"]
IGNORE = "\n".join(
    (
        "root = true",
        "[*]",
        "indent_size = 4",
        "",
        "# the engine ships this one",
        "[hooks/managed.sh]",
        "indent_size = 2",
        "ignore = true",
        "aegis_exception_category = C",
        "aegis_exception_entered = 2026-10-18",
        "aegis_exception_expires = 2026-10-28",
        "aegis_exception_reason = engine order engine-shfmt-clean-hook-scripts-20261018",
        "",
    )
)


class Runner:
    """Record each call; answer with the exit code its first matching rule names."""

    def __init__(self, ignored_rc=1, scope_rc=0):
        self.calls = []
        self.ignored_rc = ignored_rc
        self.scope_rc = scope_rc

    def __call__(self, argv, stdin):
        self.calls.append((argv, stdin))
        return self.ignored_rc if "--filename" in argv else self.scope_rc


def check(text=IGNORE, today=TODAY, runner=None):
    return shfmt_scope.check(
        COMMAND, text, TRACKED, today, runner or Runner(), read=lambda path: b"x=1\n"
    )


class ReaderTest(unittest.TestCase):
    def test_an_ignore_section_is_read_with_its_keys(self):
        found = shfmt_scope.ignores(IGNORE)
        self.assertEqual(list(found), ["hooks/managed.sh"])
        self.assertEqual(found["hooks/managed.sh"]["aegis_exception_category"], "C")

    def test_a_section_without_ignore_true_is_no_ignore(self):
        self.assertEqual(shfmt_scope.ignores(IGNORE.replace("ignore = true", "ignore = false")), {})

    def test_comments_and_keys_before_any_section_are_skipped(self):
        text = "; head\nstray = 1\n# note\n[x.sh]\nIgnore = TRUE\n"
        self.assertEqual(shfmt_scope.ignores(text), {"x.sh": {"ignore": "TRUE"}})


class CheckTest(unittest.TestCase):
    def test_a_current_needed_ignore_passes_and_the_rest_is_checked(self):
        runner = Runner()
        line = check(runner=runner)
        self.assertEqual(
            line, "PASS: shfmt clean on 1 scripts; 1 dated ignore(s) current and still needed"
        )
        self.assertEqual(runner.calls[-1], (["shfmt", "-d", "a.sh"], None))

    def test_the_ignored_script_is_piped_with_its_own_name(self):
        runner = Runner()
        check(runner=runner)
        argv, stdin = runner.calls[0]
        self.assertEqual(argv, ["shfmt", "-d", "--filename", "hooks/managed.sh"])
        self.assertEqual(stdin, b"x=1\n")

    def test_drift_outside_the_ignores_fails(self):
        with self.assertRaisesRegex(shfmt_scope.ScopeError, "drift in the 1 scripts"):
            check(runner=Runner(scope_rc=1))

    def test_an_ignored_script_that_is_clean_fails_as_stale(self):
        with self.assertRaisesRegex(
            shfmt_scope.ScopeError, "hooks/managed.sh: shfmt reports no diff"
        ):
            check(runner=Runner(ignored_rc=0))

    def test_the_expiry_day_itself_still_passes(self):
        self.assertTrue(check(today=datetime.date(2026, 10, 28)).startswith("PASS"))

    def test_the_day_after_expiry_fails(self):
        with self.assertRaisesRegex(shfmt_scope.ScopeError, "expired on 2026-10-28"):
            check(today=datetime.date(2026, 10, 29))

    def test_an_expiry_off_the_category_interval_fails(self):
        text = IGNORE.replace("expires = 2026-10-28", "expires = 2026-10-29")
        with self.assertRaisesRegex(shfmt_scope.ScopeError, "category C expires 10 days after"):
            check(text)

    def test_category_a_states_its_own_interval(self):
        text = IGNORE.replace("category = C", "category = A").replace("2026-10-28", "2027-01-01")
        self.assertTrue(check(text).startswith("PASS"))

    def test_an_unknown_category_fails(self):
        with self.assertRaisesRegex(shfmt_scope.ScopeError, "category 'E'"):
            check(IGNORE.replace("category = C", "category = E"))

    def test_a_missing_key_is_named(self):
        text = IGNORE.replace(
            "aegis_exception_reason = engine order engine-shfmt-clean-hook-scripts-20261018\n", ""
        )
        with self.assertRaisesRegex(shfmt_scope.ScopeError, "lacks aegis_exception_reason"):
            check(text)

    def test_a_malformed_date_fails(self):
        with self.assertRaisesRegex(shfmt_scope.ScopeError, "'2026-10-1x' is no YYYY-MM-DD date"):
            check(IGNORE.replace("entered = 2026-10-18", "entered = 2026-10-1x"))

    def test_a_glob_or_untracked_ignore_fails(self):
        with self.assertRaisesRegex(shfmt_scope.ScopeError, "exactly one tracked shell script"):
            check(IGNORE.replace("[hooks/managed.sh]", "[hooks/*.sh]"))

    def test_no_ignore_checks_every_script(self):
        runner = Runner()
        line = check("root = true\n", runner=runner)
        self.assertEqual(
            line, "PASS: shfmt clean on 2 scripts; 0 dated ignore(s) current and still needed"
        )
        self.assertEqual(runner.calls, [(["shfmt", "-d", *TRACKED], None)])


class MainTest(unittest.TestCase):
    def test_a_missing_command_is_a_usage_error(self):
        self.assertEqual(shfmt_scope.main(["check", "--"]), 2)

    def test_another_mode_is_a_usage_error(self):
        self.assertEqual(shfmt_scope.main(["run", "--", "shfmt"]), 2)


class CommittedTest(unittest.TestCase):
    def test_the_committed_ignores_are_well_formed(self):
        text = (shfmt_scope.ROOT / shfmt_scope.EDITORCONFIG).read_text(encoding="utf-8")
        for path, keys in shfmt_scope.ignores(text).items():
            shfmt_scope.judge(
                path, keys, [path], datetime.date.fromisoformat(keys["aegis_exception_entered"])
            )

    def test_the_recipe_runs_the_driver_with_the_pinned_shfmt(self):
        makefile = (shfmt_scope.ROOT / "Makefile").read_text(encoding="utf-8")
        self.assertIn(
            "python3 tools/shfmt_scope.py check -- \\\n"
            '\t\tpipx run --spec "shfmt-py==$$(python3 tools/ci_lints.py pin SHFMT_VERSION)"'
            " shfmt",
            makefile,
        )


if __name__ == "__main__":
    unittest.main()
