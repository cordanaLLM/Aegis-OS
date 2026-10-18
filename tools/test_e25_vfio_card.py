"""Regressions for the E25-1 operator card script (M25): the half that needs no device.

`tools/operator/e25-1-vfio-run.sh` is the operator's one-time bind of the Arc
A380 to vfio-pci, the `map-bar` run and the rebind to xe. Its stub test drives
it against a fake sysfs tree and a fake sudo, so nothing under /sys or /dev is
touched. This wrapper runs that stub test inside `make verify-all` and requires
both a clean exit and the case floor, since an exit code alone cannot tell a
passing run from one that stopped executing cases. The script needs bash and
GNU find, so the test skips with a reason anywhere else (HISS-21).
"""

import shutil
import subprocess
import sys
import unittest
from pathlib import Path

STUB_TEST = Path(__file__).resolve().parent / "operator" / "e25-1-vfio-run-stub-test.sh"

# Cases the stub test reports as "ok"; raise with the test, never lower it.
CASE_FLOOR = 28


def skip_reason():
    """Returns why the stub test cannot run on this host, or None."""
    if not sys.platform.startswith("linux"):
        return f"the card script targets Linux sysfs; host is {sys.platform}"
    for tool in ("bash", "find"):
        if shutil.which(tool) is None:
            return f"{tool} not found"
    return None


class CardStubTests(unittest.TestCase):
    """The card script binds, maps and restores only the Arc, and refuses otherwise."""

    stub_run = None

    def run_stub(self):
        reason = skip_reason()
        if reason:
            self.skipTest(reason)
        if CardStubTests.stub_run is None:
            CardStubTests.stub_run = subprocess.run(
                ["bash", str(STUB_TEST)],
                capture_output=True,
                text=True,
                timeout=120,
                check=False,
            )
        return CardStubTests.stub_run

    def test_every_stub_case_passes(self):
        """Positive: the stub run exits 0 and reports fails=0."""
        run = self.run_stub()
        self.assertEqual(run.returncode, 0, run.stdout + run.stderr)
        self.assertIn("fails=0", run.stdout)

    def test_no_case_reports_a_failure(self):
        """Negative: no line starts with FAIL."""
        run = self.run_stub()
        failing = [line for line in run.stdout.splitlines() if line.startswith("FAIL")]
        self.assertEqual(failing, [])

    def test_the_case_count_reaches_its_floor(self):
        """Boundary: at least CASE_FLOOR cases ran, so a stub that stops early is red."""
        run = self.run_stub()
        passed = [line for line in run.stdout.splitlines() if line.startswith("ok   ")]
        self.assertGreaterEqual(len(passed), CASE_FLOOR, run.stdout)

    def test_the_card_line_sets_no_stub_override(self):
        """Negative: the stub overrides are read with defaults, never set in the script."""
        script = (STUB_TEST.parent / "e25-1-vfio-run.sh").read_text(encoding="utf-8")
        for name in (
            "AEGIS_SYSFS",
            "AEGIS_DEV",
            "AEGIS_SUDO",
            "AEGIS_BIN",
            "AEGIS_EVID",
            "AEGIS_PROC",
            "AEGIS_NVSMI",
            "AEGIS_KWIN_PIN",
        ):
            self.assertIn("${" + name + ":-", script)
            self.assertNotIn(name + "=", script.replace("${" + name + ":-", ""))

    def test_the_nvidia_device_is_never_a_write_target(self):
        """Boundary: lines naming the NVIDIA device only read it; none writes or runs sudo."""
        script = (STUB_TEST.parent / "e25-1-vfio-run.sh").read_text(encoding="utf-8")
        for line in script.splitlines():
            if "$NVIDIA" in line:
                self.assertNotIn("wsys", line)
                self.assertNotIn("$SUDO", line)


if __name__ == "__main__":
    unittest.main()
