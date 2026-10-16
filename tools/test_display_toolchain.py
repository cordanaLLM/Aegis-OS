"""Regressions for the display slice's build-toolchain read-back (M27, D80).

`tools/display_toolchain.py` reads pkgconf, libva, VA-API and clang back from
the host and bindgen and cros-libva from `Cargo.lock` before the crate gate
builds. These tests need none of those tools: they drive the read-back through
fake runners, and they hold the admission page, the tool's own lists and the
Verification gate's libva pins to one another, so a floor edited in one place
alone fails `make verify-all` on every platform.
"""

import io
import re
import sys
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import display_toolchain as tool

ROOT = Path(__file__).resolve().parent.parent
ADMISSION = ROOT / "docs" / "roadmap" / "toolchain-admission.md"
WORKFLOW = ROOT / ".github" / "workflows" / "ci.yml"
MAKEFILE = ROOT / "Makefile"

# The admission page is prose plus tables; this bounds the scan over it.
MAX_PAGE_LINES = 4000
ADMISSION_HEADING = "## The display slice's build toolchain (M27, D80)"
TABLE_ROW = re.compile(r"^\|([^|]*)\|([^|]*)\|([^|]*)\|[^|]*\|$")
ENV_PIN = re.compile(r'^  ([A-Z][A-Z0-9_]*): "([^"]*)"$', re.M)
LEADING_VERSION = re.compile(r"^\d+(?:\.\d+)*")


def fake_runner(outputs):
    """A runner that answers each argv with a recorded (code, output) pair."""

    def runner(argv):
        return outputs[tuple(argv)]

    return runner


def admitted_rows():
    """Return the M27 admission table as ``{name: (reference, floor)}``."""
    found = {}
    inside = False
    for line in ADMISSION.read_text(encoding="utf-8").splitlines()[:MAX_PAGE_LINES]:
        if line.startswith("## ") or line.startswith("### "):
            inside = line == ADMISSION_HEADING
            continue
        match = TABLE_ROW.match(line) if inside else None
        if match is None:
            continue
        name, reference, floor = (cell.strip() for cell in match.groups())
        if name == "Tool or crate" or set(name) <= set(": -"):
            continue
        version = LEADING_VERSION.match(reference)
        found[name] = (version.group(0) if version else reference, floor)
    return found


def workflow_pins():
    """Return the `env:` pins of the Verification gate workflow."""
    return dict(ENV_PIN.findall(WORKFLOW.read_text(encoding="utf-8")))


CACHE_KEY = re.compile(r'^          key: "(libva-[^"]*)"$', re.M)
RUNNER_LITERAL = re.compile(r"(?:ubuntu|macos|windows)-\d")
IMAGE_STEP = "name: Name the runner image for the libva cache key"
IMAGE_OUTPUT = "${{ steps.runner-image.outputs.image }}"


def libva_cache_key_reasons(text):
    """Return why the libva cache key could restore another image's build.

    A prefix built on one runner image links against that image's libraries,
    so the key must name the image the job runs on, read at run time, never a
    literal that outlives a runner move.
    """
    keys = CACHE_KEY.findall(text)
    if len(keys) != 1:
        return [f"expected one libva cache key, found {len(keys)}"]
    key = keys[0]
    reasons = []
    if RUNNER_LITERAL.search(key):
        reasons.append("the libva cache key names a literal runner image")
    if IMAGE_OUTPUT not in key:
        reasons.append("the libva cache key does not name the runner image step's output")
    elif IMAGE_STEP not in text or text.index(IMAGE_STEP) > text.index(key):
        reasons.append("the runner image step does not run before the libva cache")
    return reasons


class ReadVersionTests(unittest.TestCase):
    """One row read back from a program on PATH."""

    ROW = ("pkgconf", ["pkg-config", "--version"], r"^\s*(\d+(?:\.\d+)*)\s*$", "1.8.1", "3.0.7")

    def test_a_version_is_read_from_the_output(self):
        """Positive: the version is the first group of the row's pattern."""
        runner = fake_runner({("pkg-config", "--version"): (0, "3.0.7\n")})
        found = tool.read_version(self.ROW, runner, lambda _name: "/usr/bin/pkg-config")
        self.assertEqual(found, "3.0.7")

    def test_a_missing_failing_or_silent_tool_is_refused(self):
        """Negative: absent from PATH, a non-zero exit and no match each raise."""
        with self.assertRaisesRegex(tool.ToolchainError, "not on PATH"):
            tool.read_version(self.ROW, fake_runner({}), lambda _name: None)
        runner = fake_runner({("pkg-config", "--version"): (1, "boom")})
        with self.assertRaisesRegex(tool.ToolchainError, "exited 1"):
            tool.read_version(self.ROW, runner, lambda _name: "/x")
        runner = fake_runner({("pkg-config", "--version"): (0, "pkgconf\n")})
        with self.assertRaisesRegex(tool.ToolchainError, "no version matched"):
            tool.read_version(self.ROW, runner, lambda _name: "/x")

    def test_a_program_that_outlives_its_deadline_is_refused(self):
        """Negative: `run` enforces its deadline instead of hanging (HISS-02)."""
        argv = [sys.executable, "-c", "import time; time.sleep(10)"]
        with self.assertRaisesRegex(tool.ToolchainError, "deadline"):
            tool.run(argv, timeout=0.2)

    def test_the_floor_comparison_is_inclusive_and_pads_components(self):
        """Boundary: the floor itself passes, one below fails, `6` equals `6.0`."""
        self.assertTrue(tool.at_least("2.24.1", "2.24.1"))
        self.assertFalse(tool.at_least("2.24.0", "2.24.1"))
        self.assertTrue(tool.at_least("6", "6.0"))
        self.assertTrue(tool.at_least("18.1.3", "6.0"))
        self.assertFalse(tool.at_least("5.9.9", "6.0"))


class CheckToolchainTests(unittest.TestCase):
    """Every row read back, printed, and held to its floor."""

    def outputs(self, libva):
        return {
            ("pkg-config", "--version"): (0, "1.8.1\n"),
            ("pkg-config", "--variable=libva_version", "libva"): (0, f"{libva}\n"),
            ("pkg-config", "--modversion", "libva"): (0, "1.24.0\n"),
            ("clang", "--version"): (0, "Ubuntu clang version 18.1.3 (1ubuntu1)\n"),
        }

    def check(self, libva):
        printed = io.StringIO()
        with redirect_stdout(printed):
            reasons = tool.check_toolchain(fake_runner(self.outputs(libva)), lambda _: "/x")
        return reasons, printed.getvalue()

    def test_the_runner_values_pass_and_name_the_reference(self):
        """Positive: the Verification gate's values pass and are printed."""
        reasons, printed = self.check("2.24.1")
        self.assertEqual(reasons, [])
        self.assertIn("pkgconf: 1.8.1", printed)
        self.assertIn("[reference profile recorded 3.0.7]", printed)
        self.assertIn("clang: 18.1.3", printed)

    def test_apt_libva_below_the_floor_is_refused(self):
        """Negative: ubuntu-24.04's own libva 2.20.0 is below the floor."""
        reasons, _ = self.check("2.20.0")
        self.assertEqual(reasons, ["libva 2.20.0 is below the admitted floor 2.24.1"])

    def test_the_admitted_floor_itself_passes(self):
        """Boundary: libva exactly at its floor passes."""
        reasons, _ = self.check(tool.TOOLCHAIN[1][3])
        self.assertEqual(reasons, [])


class LockTests(unittest.TestCase):
    """bindgen and cros-libva read back from the lock, exactly."""

    def test_the_committed_lock_carries_the_admitted_entries(self):
        """Positive: the real lock carries each LOCKED row exactly once."""
        with redirect_stdout(io.StringIO()):
            self.assertEqual(tool.check_locked(tool.read_lock()), [])

    def test_a_registry_cros_libva_or_a_second_bindgen_is_refused(self):
        """Negative: cros-libva from crates.io, or two bindgen entries, fail."""
        text = tool.read_lock()
        registry = text.replace(
            tool.LOCKED[1][2], "registry+https://github.com/rust-lang/crates.io-index"
        )
        doubled = text + '\n[[package]]\nname = "bindgen"\nversion = "0.72.1"\n'
        with redirect_stdout(io.StringIO()):
            self.assertEqual(len(tool.check_locked(registry)), 1)
            self.assertEqual(len(tool.check_locked(doubled)), 1)

    def test_a_lock_that_is_not_toml_is_refused(self):
        """Negative: a truncated lock fails to parse instead of reading as empty."""
        with self.assertRaises(tool.ToolchainError):
            tool.lock_entries('[[package]]\nname = "bindgen\n')

    def test_an_absent_or_oversized_lock_is_refused(self):
        """Boundary: a lock that is missing is refused, not read as empty."""
        with self.assertRaises(tool.ToolchainError):
            tool.read_lock(ROOT / "no-such-Cargo.lock")


class AdmissionTests(unittest.TestCase):
    """The page, the tool and the workflow state one admission."""

    def test_every_admitted_row_appears_on_both_sides(self):
        page = admitted_rows()
        code = {row[0]: (row[4], row[3]) for row in tool.TOOLCHAIN}
        code.update({name: (version, version) for name, version, _source in tool.LOCKED})
        self.assertEqual(set(page), set(code), "the page and the tool admit different rows")
        for name, (reference, floor) in code.items():
            self.assertEqual(page[name], (reference, floor), f"{name} differs")

    def test_a_floor_is_never_above_its_recorded_reference(self):
        for name, _argv, _pattern, floor, reference in tool.TOOLCHAIN:
            self.assertTrue(tool.at_least(reference, floor), f"{name}: {reference} < {floor}")

    def test_the_workflow_builds_the_admitted_libva_floor(self):
        """The CI pins name the floor, and the page records every pin."""
        pins = workflow_pins()
        self.assertEqual(pins["LIBVA_VERSION"], tool.TOOLCHAIN[1][3])
        page = ADMISSION.read_text(encoding="utf-8")
        for key in (
            "LIBVA_SHA256",
            "MESON_VERSION",
            "MESON_WHEEL_SHA256",
            "NINJA_VERSION",
            "NINJA_WHEEL_SHA256",
        ):
            self.assertRegex(pins[key], r"^[0-9a-f.]+$", key)
            self.assertIn(pins[key], page, f"{key} is not recorded on the page")
        self.assertEqual(len(pins["LIBVA_SHA256"]), 64)

    def test_the_libva_build_runs_before_the_gate_and_the_gate_reads_it_back(self):
        """The build precedes `make verify-all`, and verify-rust reads back first."""
        text = WORKFLOW.read_text(encoding="utf-8")
        build = text.index("name: Build the pinned libva from its release tarball")
        point = text.index("name: Point the crate build at the pinned libva")
        gate = text.index("run: make verify-all")
        self.assertLess(build, point)
        self.assertLess(point, gate)
        self.assertIn('echo "${LIBVA_SHA256}  $work/libva.tar.bz2" | sha256sum -c -', text)
        makefile = MAKEFILE.read_text(encoding="utf-8")
        recipe = makefile[makefile.index("verify-rust:\n") :]
        self.assertLess(
            recipe.index("python3 tools/display_toolchain.py"),
            recipe.index("cargo build --locked"),
        )


class LibvaCacheKeyTests(unittest.TestCase):
    """The libva prefix is never restored onto another runner image."""

    def test_the_workflow_keys_the_cache_on_the_runner_image(self):
        """Positive: the committed key names the image step's output."""
        self.assertEqual(libva_cache_key_reasons(WORKFLOW.read_text(encoding="utf-8")), [])

    def test_a_literal_runner_image_in_the_key_is_refused(self):
        """Negative: the key that restored a 24.04 build on 26.04 fails."""
        text = WORKFLOW.read_text(encoding="utf-8").replace(IMAGE_OUTPUT, "ubuntu-24.04")
        self.assertEqual(
            libva_cache_key_reasons(text),
            [
                "the libva cache key names a literal runner image",
                "the libva cache key does not name the runner image step's output",
            ],
        )

    def test_an_image_step_after_the_cache_or_a_second_key_is_refused(self):
        """Boundary: the output read before its step runs, or two keys, fails."""
        text = WORKFLOW.read_text(encoding="utf-8")
        moved = text.replace(IMAGE_STEP, "name: Something else", 1) + f"\n# {IMAGE_STEP}\n"
        self.assertEqual(
            libva_cache_key_reasons(moved),
            ["the runner image step does not run before the libva cache"],
        )
        key = CACHE_KEY.search(text).group(0)
        self.assertEqual(
            libva_cache_key_reasons(text + "\n" + key + "\n"),
            ["expected one libva cache key, found 2"],
        )


if __name__ == "__main__":
    unittest.main()
