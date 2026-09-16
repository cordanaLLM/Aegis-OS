#!/usr/bin/env python3
"""Positive, negative and boundary coverage for the host-difference helpers.

HISS-15 requires all three for a public interface. HISS-21 requires that a gate
either runs or states why it did not, so the tests that matter most here are the
ones asserting a *reason* comes back rather than an exception or a bare False.

`test_no_gate_calls_a_posix_only_api_directly` is the sweep that keeps the class
closed: guarding the call sites that existed is worth little if the next one
is written next week, so the rule is enforced over the tree rather than over a
list of known offenders.
"""

import pathlib
import re
import unittest

import host

ROOT = pathlib.Path(__file__).resolve().parent
# The gates that emit or read Linux-target artifacts, which are what run on a
# contributor's own machine. host.py is exempt because it is where the guarded
# call lives, and the test modules are exempt because they name these APIs as
# mock targets and as the text of the very skip reasons being asserted.
GATES = sorted(
    p for p in ROOT.glob("*.py") if p.name != "host.py" and not p.name.startswith("test_")
)
# Scalar bound on the sweep (HISS-02); the directory holds well under this.
MAX_SWEPT_FILES = 128
POSIX_ONLY = re.compile(r"\bos\.(uname|getuid|getgid|geteuid|getegid|killpg|getpgid)\s*\(")


def unguarded_call(line):
    """Return True where `line` is a call site rather than prose about one.

    The sweep and this predicate are the same decision, so the boundary case
    below exercises what the sweep actually applies, not a weaker regex.
    """
    stripped = line.strip()
    if stripped.startswith("#") or "``" in line:
        return False
    return POSIX_ONLY.search(line) is not None


class TargetPathTests(unittest.TestCase):
    """`target()` renders a path the way its Linux consumer reads it."""

    def test_a_posix_path_is_unchanged(self):
        """Positive: the common case must not be perturbed."""
        self.assertEqual(host.target("/sys/kernel/sched_ext/switch_all"), "/sys/kernel/sched_ext/switch_all")
        self.assertEqual(host.target("/defs"), "/defs")
        self.assertEqual(host.target(pathlib.PurePosixPath("/img.raw")), "/img.raw")

    def test_a_path_past_the_bound_is_refused(self):
        """Negative: an unbounded path is a caller defect, not a value to emit."""
        with self.assertRaises(ValueError):
            host.target("/" + "a" * host.MAX_TARGET_PATH)

    def test_a_windows_path_renders_posix_without_its_drive(self):
        """Boundary: the host's own spelling never reaches the Linux consumer.

        This is the defect the helper exists for: `str(Path("/defs"))` produced
        `\\defs` on Windows, which reached a systemd-repart command line and a
        `.transfer` unit as a literal backslash path.
        """
        rendered = host.target(pathlib.PureWindowsPath(r"C:\tmp\scratch\img.raw"))
        self.assertEqual(rendered, "/tmp/scratch/img.raw")
        self.assertNotIn("\\", rendered)
        self.assertNotIn(":", rendered)

    def test_a_relative_path_keeps_its_shape(self):
        """Boundary: a relative path stays relative; the helper adds no root."""
        self.assertEqual(host.target(pathlib.PureWindowsPath(r"build\repart.d")), "build/repart.d")


class HostProbeTests(unittest.TestCase):
    """Each probe answers with a value or a reason, and never raises."""

    def test_a_probe_returns_exactly_one_of_a_value_and_a_reason(self):
        """Positive: the two-state contract HISS-21 requires, on this host."""
        for probe in (host.kernel_release, host.uid, host.gid):
            with self.subTest(probe=probe.__name__):
                value, reason = probe()
                self.assertTrue(
                    (value is None) != (reason is None),
                    f"{probe.__name__} returned value={value!r} reason={reason!r}",
                )

    def test_a_reason_names_what_is_absent(self):
        """Negative: an empty reason would be the silent skip the rule forbids."""
        for probe in (host.kernel_release, host.uid, host.gid):
            with self.subTest(probe=probe.__name__):
                value, reason = probe()
                if value is None:
                    self.assertTrue(reason.strip(), "a skip must state its reason")
                    self.assertIn("os.", reason)

    def test_the_chmod_probe_is_measured_not_inferred(self):
        """Boundary: the answer is a real attempt, so it holds for a root container too."""
        enforced, reason = host.readonly_directory_blocks_removal()
        self.assertIsInstance(enforced, bool)
        self.assertTrue((reason is None) == enforced)
        if not enforced:
            self.assertIn("read-only directory", reason)


class PosixOnlyApiSweep(unittest.TestCase):
    """The class stays closed: no gate may call a POSIX-only API unguarded."""

    def test_no_gate_calls_a_posix_only_api_directly(self):
        """Negative: a new unguarded call site fails here, not on a user's Windows host."""
        offenders = []
        for path in GATES[:MAX_SWEPT_FILES]:
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                if unguarded_call(line):
                    offenders.append(f"{path.name}:{number}: {line.strip()}")
        self.assertEqual(
            offenders,
            [],
            "call host.kernel_release()/host.uid() instead, so the gate skips with a reason "
            "(HISS-21) rather than raising AttributeError off POSIX:\n" + "\n".join(offenders),
        )

    def test_the_sweep_would_catch_a_new_call_site(self):
        """Positive: the sweep's own pattern is proven against a known-bad line."""
        self.assertTrue(unguarded_call("    host = os.uname().release"))
        self.assertTrue(unguarded_call('    f"--reuid={os.getuid()}",'))
        self.assertTrue(unguarded_call('    f"--regid={os.getgid()}",'))

    def test_the_sweep_does_not_fire_on_prose(self):
        """Boundary: a docstring naming the API is documentation, not a call site."""
        self.assertFalse(unguarded_call("    the boundary case reads ``os.uname()`` alone"))
        self.assertFalse(unguarded_call("    # os.uname() is guarded in host.py"))


if __name__ == "__main__":
    unittest.main()
