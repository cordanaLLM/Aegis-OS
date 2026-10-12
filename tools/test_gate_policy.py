"""Regressions for the required-gate policy: a required gate that cannot run fails."""

import contextlib
import io
import unittest

import gate_policy as policy


def environ(value):
    return {policy.VARIABLE: value}


def capture(function, *args, **kwargs):
    """Return (result, stdout) of `function` called with `args` and `kwargs`."""
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        result = function(*args, **kwargs)
    return result, out.getvalue()


class NamedTests(unittest.TestCase):
    def test_listed_names_are_read_in_order(self):
        """Positive: a comma list yields each gate it names."""
        self.assertEqual(policy.named(environ("systemd,mkosi")), ["systemd", "mkosi"])

    def test_an_unset_variable_requires_nothing(self):
        """Negative: without the variable no gate is required."""
        self.assertEqual(policy.named({}), [])
        self.assertFalse(policy.is_required("systemd", {}))

    def test_blanks_and_whitespace_are_dropped(self):
        """Boundary: empty entries and padding name no gate and hide none."""
        self.assertEqual(policy.named(environ(" , systemd ,, ")), ["systemd"])
        self.assertEqual(policy.named(environ("")), [])

    def test_a_name_is_matched_whole(self):
        """Boundary: a gate name inside another word does not require that gate."""
        self.assertFalse(policy.is_required("mkosi", environ("mkosi2")))
        self.assertFalse(policy.is_required("systemd", environ("SYSTEMD")))


class UnknownNameTests(unittest.TestCase):
    def test_known_names_pass(self):
        """Positive: every recorded gate is accepted."""
        code, printed = capture(policy.refuse_unknown, environ(",".join(policy.GATES)))
        self.assertEqual((code, printed), (0, ""))

    def test_a_misspelt_name_fails(self):
        """Negative: a typo would leave the intended gate unrequired, so it fails."""
        code, printed = capture(policy.refuse_unknown, environ("systemd,mksoi"))
        self.assertEqual(code, 1)
        self.assertIn("'mksoi'", printed)
        self.assertNotIn("'systemd'", printed)

    def test_an_empty_list_has_no_unknown_name(self):
        """Boundary: an empty value is a run that requires nothing, not an error."""
        self.assertEqual(capture(policy.refuse_unknown, environ(" , "))[0], 0)


class DidNotRunTests(unittest.TestCase):
    def test_an_unrequired_gate_skips_by_name(self):
        """Positive: the development-host wording and exit 0 are unchanged."""
        code, printed = capture(
            policy.did_not_run, "mkosi", "mkosi not on PATH", environ=environ("")
        )
        self.assertEqual(code, 0)
        self.assertEqual(
            printed, "SKIP: mkosi not on PATH; the mkosi definition gate did not run.\n"
        )

    def test_a_required_gate_fails(self):
        """Negative: a required gate that did not run exits 1 and names the variable."""
        code, printed = capture(
            policy.did_not_run, "mkosi", "mkosi not on PATH", environ=environ("mkosi")
        )
        self.assertEqual(code, 1)
        self.assertTrue(printed.startswith("FAIL: mkosi not on PATH;"))
        self.assertIn(policy.VARIABLE, printed)

    def test_the_note_follows_either_verdict(self):
        """Boundary: the reference-profile note is printed for SKIP and FAIL alike."""
        for value, verdict in (("", "SKIP"), ("systemd", "FAIL")):
            _, printed = capture(
                policy.did_not_run, "systemd", "too old", "Profile X.", environ=environ(value)
            )
            self.assertTrue(printed.startswith(verdict))
            self.assertTrue(printed.rstrip().endswith("Profile X."))


if __name__ == "__main__":
    unittest.main()
