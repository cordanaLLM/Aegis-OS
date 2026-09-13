"""Regressions for the kernel build gate: state matching, read-back parsing, pinned inputs.

The gate itself needs a kernel source tree, a compiler and an emulator. These
tests need none of them: they exercise the gate's own decisions -- which
configurations satisfy the M18 payload, which read-back is refused, and whether
the tracked pin, the fragments and the recorded toolchain still agree -- so a
checkout that cannot run the build still fails when one of those drifts.
"""

import gzip
import io
import json
import os
import re
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import verify_kernel_build as gate

ROOT = Path(__file__).resolve().parent.parent
ADMISSION = ROOT / "docs" / "roadmap" / "toolchain-admission.md"

# The admission page is prose plus tables; this bounds the scan over it.
MAX_PAGE_LINES = 4000
ADMISSION_HEADING = "## The kernel build toolchain"
TABLE_ROW = re.compile(r"^\|([^|]*)\|([^|]*)\|([^|]*)\|[^|]*\|$")

# A produced configuration, trimmed to the lines a requirement row reads.
PRODUCED = """#
# Automatically generated file; DO NOT EDIT.
#
CONFIG_LOCALVERSION="-aegis-m26"
CONFIG_BPF_SYSCALL=y
CONFIG_BPF_LSM=y
CONFIG_PREEMPT_RT=y
# CONFIG_PREEMPT_DYNAMIC is not set
CONFIG_HZ_1000=y
CONFIG_HZ=1000
CONFIG_VFIO=m
"""

READBACK = """AEGIS-M26-BEGIN
AEGIS-M26-UNAME-R 7.2.5-aegis-m26
AEGIS-M26-UNAME-V #1 SMP PREEMPT_RT Sun Sep 13 18:24:27 CEST 2026
AEGIS-M26-CONFIG-BEGIN
CONFIG_PREEMPT_RT=y
AEGIS-M26-CONFIG-END
AEGIS-M26-END
"""


def row(symbol, state, required_by="REQ-P07-01"):
    """One requirement row in the shape build/kernel-requirement.json uses."""
    return {"symbol": symbol, "state": state, "probe": "kernel-config", "required-by": required_by}


def admitted_rows():
    """Return the M26 admission table as ``{name: (reference, floor)}``.

    Both value columns carry more than the value the gate enforces: the Tool
    cell may name the package (``ld (binutils)``) and the Floor cell the script
    that declares the number (``8.1.0 (...)``), so only the first token of each
    is compared. ``none declared`` is read as ``None``, which is what a row with
    no floor carries in the gate's own ``TOOLCHAIN`` list.
    """
    found = {}
    inside = False
    for line in ADMISSION.read_text().splitlines()[:MAX_PAGE_LINES]:
        if line.startswith("## "):
            inside = line.startswith(ADMISSION_HEADING)
            continue
        match = TABLE_ROW.match(line) if inside else None
        if match is None:
            continue
        name, reference, floor = (cell.strip() for cell in match.groups())
        if name == "Tool" or set(name) <= set(": -"):
            continue
        found[name.split()[0]] = (reference, None if floor.startswith("none") else floor.split()[0])
    return found


def host_readback(rows, release, configuration):
    """Run the gate's host read-back cases against a stand-in /proc/config.gz.

    ``configuration`` of None stands for a kernel that publishes none, which is
    the case the skip has to handle without dropping the boundary case with it.
    """
    with tempfile.TemporaryDirectory() as directory:
        stand_in = Path(directory) / "config.gz"
        if configuration is not None:
            stand_in.write_bytes(gzip.compress(configuration.encode()))
        printed = io.StringIO()
        with mock.patch.object(gate, "HOST_CONFIG", stand_in):
            with redirect_stdout(printed):
                outcomes = gate.host_readback_cases(rows, release)
    return outcomes, printed.getvalue()


class RecordingChild:
    """A stand-in decompressor that records whether it was killed, and when.

    ``subprocess.Popen.__exit__`` waits on the child with no bound at all, so a
    kill that happens after it is no deadline. ``kills_before_wait`` is what a
    caller has to assert on.
    """

    def __init__(self):
        self.stdout = None
        self.kills = 0
        self.kills_before_wait = None

    def __enter__(self):
        return self

    def __exit__(self, *_details):
        self.kills_before_wait = self.kills
        return False

    def kill(self):
        self.kills += 1


def verify_with(child, gpg):
    """Call verify_signature with a recorded child and a stubbed gpg run."""
    with mock.patch.object(gate.subprocess, "Popen", return_value=child):
        with mock.patch.object(gate, "run", gpg):
            return gate.verify_signature(
                Path("keyring"), Path("linux.tar.sign"), Path("linux.tar.xz"), "AABB CCDD"
            )


class ConfigParsingTests(unittest.TestCase):
    def test_assignments_and_unset_lines_are_both_read(self):
        """Positive: `=y`, `=m`, a quoted string and an `is not set` line all parse."""
        states = gate.parse_config(PRODUCED)
        self.assertEqual(states["CONFIG_BPF_SYSCALL"], "y")
        self.assertEqual(states["CONFIG_VFIO"], "m")
        self.assertEqual(states["CONFIG_PREEMPT_DYNAMIC"], "n")
        self.assertEqual(states["CONFIG_LOCALVERSION"], '"-aegis-m26"')
        self.assertEqual(states["CONFIG_HZ"], "1000")

    def test_comments_and_prose_are_not_read_as_state(self):
        """Negative: a comment that merely mentions a symbol sets no state."""
        states = gate.parse_config("# CONFIG_PREEMPT_RT is required by REQ-P07-01\n#\n")
        self.assertEqual(states, {})

    def test_an_empty_configuration_records_nothing(self):
        """Boundary: no lines at all is an empty map, not a default."""
        self.assertEqual(gate.parse_config(""), {})


class SatisfactionTests(unittest.TestCase):
    def test_each_required_state_is_satisfied_by_its_own_observation(self):
        """Positive: the four required states accept exactly what they name."""
        self.assertTrue(gate.satisfied("built-in", "y"))
        self.assertTrue(gate.satisfied("module", "m"))
        self.assertTrue(gate.satisfied("present", "m"))
        self.assertTrue(gate.satisfied("absent", "n"))

    def test_a_module_never_satisfies_a_built_in_row(self):
        """Negative: the state the schema demands is the state it demands."""
        self.assertFalse(gate.satisfied("built-in", "m"))
        self.assertFalse(gate.satisfied("module", "y"))
        self.assertFalse(gate.satisfied("present", "n"))
        self.assertFalse(gate.satisfied("absent", "y"))

    def test_an_unrecorded_symbol_satisfies_nothing_including_absent(self):
        """Boundary: fail closed. An unanswered question is not a measured absence."""
        for required in ("built-in", "module", "present", "absent"):
            self.assertFalse(gate.satisfied(required, None))

    def test_an_unknown_required_state_is_refused(self):
        """Boundary: a state spelling the gate does not know never passes."""
        self.assertFalse(gate.satisfied("built-in-ish", "y"))


class UnsatisfiedTests(unittest.TestCase):
    def test_a_conforming_configuration_reports_nothing(self):
        """Positive: every row met means an empty problem list."""
        rows = [row("CONFIG_PREEMPT_RT", "built-in"), row("CONFIG_VFIO", "module")]
        self.assertEqual(gate.unsatisfied(rows, gate.parse_config(PRODUCED)), [])

    def test_a_missing_option_is_named_with_its_requirement(self):
        """Negative: the refusal names the symbol, the requirement and what was seen."""
        rows = [row("CONFIG_SCHED_CLASS_EXT", "built-in", "REQ-P07-06")]
        problems = gate.unsatisfied(rows, gate.parse_config(PRODUCED))
        self.assertEqual(len(problems), 1)
        self.assertIn("CONFIG_SCHED_CLASS_EXT", problems[0])
        self.assertIn("REQ-P07-06", problems[0])
        self.assertIn("unrecorded", problems[0])

    def test_a_module_where_built_in_is_demanded_is_reported(self):
        """Boundary: the wrong state is reported as loudly as a missing symbol."""
        problems = gate.unsatisfied([row("CONFIG_VFIO", "built-in")], gate.parse_config(PRODUCED))
        self.assertEqual(len(problems), 1)
        self.assertIn("requires built-in, observed 'm'", problems[0])


class ReadBackTests(unittest.TestCase):
    def test_the_guest_sections_and_fields_are_read(self):
        """Positive: the markers delimit the release line and the configuration."""
        self.assertEqual(gate.guest_field(READBACK, "UNAME-R"), "7.2.5-aegis-m26")
        self.assertEqual(gate.guest_section(READBACK, "CONFIG"), "CONFIG_PREEMPT_RT=y")

    def test_a_truncated_read_back_is_refused_rather_than_half_read(self):
        """Negative: a guest that stopped mid-dump fails instead of reporting less."""
        truncated = READBACK.replace("AEGIS-M26-CONFIG-END\n", "")
        with self.assertRaises(gate.GateError):
            gate.guest_section(truncated, "CONFIG")
        with self.assertRaises(gate.GateError):
            gate.guest_field("", "UNAME-R")

    def test_the_identity_check_refuses_anything_but_the_built_release(self):
        """Boundary: the read-back runs on the built artifact or it does not run."""
        self.assertEqual(gate.identity_problems("7.2.5-aegis-m26", "7.2.5-aegis-m26", "6.0.0"), [])
        host = gate.identity_problems("6.0.0-host", "7.2.5-aegis-m26", "6.0.0-host")
        self.assertEqual(len(host), 2)
        self.assertIn("not the built release", host[0])
        self.assertIn("the host's own release", host[1])


class VersionTests(unittest.TestCase):
    def test_versions_order_by_component_and_not_by_text(self):
        """Positive: 1.31 is above the 1.26 floor, and 2.10 below 2.42."""
        self.assertGreater(gate.version_tuple("1.31"), gate.version_tuple("1.26"))
        self.assertLess(gate.version_tuple("2.10"), gate.version_tuple("2.42.3"))

    def test_every_recorded_reference_version_parses(self):
        """Negative: a reference value that is not a version would skip the gate silently."""
        for _name, _argv, _pattern, floor, reference in gate.TOOLCHAIN:
            self.assertTrue(gate.version_tuple(reference))
            if floor is not None:
                self.assertGreaterEqual(
                    gate.version_tuple(reference), gate.version_tuple(floor), reference
                )

    def test_a_version_equal_to_the_floor_is_admitted(self):
        """Boundary: the floor itself is inside the admission, not outside it."""
        self.assertFalse(gate.version_tuple("4.0") < gate.version_tuple("4.0"))


class PinnedInputTests(unittest.TestCase):
    def test_the_tracked_fragment_assigns_exactly_the_payload_symbols(self):
        """Positive: the fragment and the M18 payload name the same symbols."""
        payload = json.loads(gate.REQUIREMENT.read_text())
        expected = sorted(feature["symbol"] for feature in payload["features"])
        found = sorted(gate.parse_config(gate.REQUIREMENT_FRAGMENT.read_text()))
        self.assertEqual(found, expected)

    def test_the_support_fragment_states_no_product_requirement(self):
        """Negative: a hand-written file must not be able to satisfy a schema row."""
        payload = json.loads(gate.REQUIREMENT.read_text())
        required = {feature["symbol"] for feature in payload["features"]}
        support = set(gate.parse_config(gate.SUPPORT_FRAGMENT.read_text()))
        self.assertEqual(support & required, set())

    def test_the_pin_names_the_fragments_the_gate_applies(self):
        """Boundary: the pin is the only tracked statement of what is built."""
        pin = json.loads(gate.SOURCE_PIN.read_text())
        self.assertEqual(
            pin["fragments"],
            [
                str(gate.SUPPORT_FRAGMENT.relative_to(ROOT)),
                str(gate.REQUIREMENT_FRAGMENT.relative_to(ROOT)),
            ],
        )
        self.assertEqual(len(pin["tarball-sha256"]), 64)
        self.assertIn(pin["version"], pin["expected-kernel-release"])


class AdmissionTests(unittest.TestCase):
    def test_the_admission_table_and_the_gate_admit_exactly_the_same_tools(self):
        """Positive: set equality, so neither side can carry a row the other does not.

        An `assertIn` in one direction only would admit a page row for a tool
        the gate never runs, which is an admission nothing enforces.
        """
        self.assertEqual(set(admitted_rows()), {row[0] for row in gate.TOOLCHAIN})

    def test_every_admitted_floor_and_reference_is_the_one_the_gate_enforces(self):
        """Negative: a Floor column the gate does not enforce is a false admission."""
        found = admitted_rows()
        for name, _argv, _pattern, floor, reference in gate.TOOLCHAIN:
            self.assertIn(name, found, name)
            self.assertEqual(found[name][0], reference, f"{name} reference")
            self.assertEqual(found[name][1], floor, f"{name} floor")

    def test_both_floor_cell_shapes_are_read_as_the_value_they_state(self):
        """Boundary: a declared floor with its source, and a row with no floor at all."""
        found = admitted_rows()
        self.assertEqual(found["gcc"], ("16.2.1", "8.1.0"))
        self.assertEqual(found["perl"], ("5.42.2", None))

    def test_the_pinned_source_is_recorded_on_the_admission_page(self):
        """Negative: a pin the page does not carry is not an admission."""
        pin = json.loads(gate.SOURCE_PIN.read_text())
        page = ADMISSION.read_text()
        self.assertIn(pin["tarball-sha256"], page)
        self.assertIn(pin["base-configuration"], page)

    def test_the_admission_page_records_the_signing_key(self):
        """Boundary: the digest alone is not how the source was verified."""
        pin = json.loads(gate.SOURCE_PIN.read_text())
        page = ADMISSION.read_text()
        self.assertIn(pin["signing-keys"]["tarball"]["fingerprint"], page)


class SignatureDeadlineTests(unittest.TestCase):
    """HISS-02: the decompressor is killed before the unbounded wait, on every path."""

    def test_a_good_signature_returns_its_lines_and_kills_the_decompressor(self):
        """Positive: the normal path still reports the signature, and leaves no child."""
        child = RecordingChild()
        good = 'gpg: Good signature from "Greg Kroah-Hartman"\ngpg: key AABBCCDD\n'
        lines = verify_with(child, lambda *_a, **_k: (0, "", good))
        self.assertEqual(len(lines), 1)
        self.assertIn("Good signature", lines[0])
        self.assertEqual(child.kills_before_wait, 1)

    def test_a_bad_signature_is_refused_and_still_kills_the_decompressor(self):
        """Negative: a verification failure is a GateError, not a leaked child."""
        child = RecordingChild()
        with self.assertRaises(gate.GateError):
            verify_with(child, lambda *_a, **_k: (1, "", "gpg: BAD signature\n"))
        self.assertEqual(child.kills_before_wait, 1)

    def test_a_gpg_deadline_fails_closed_instead_of_waiting_on_the_decompressor(self):
        """Boundary: the deadline path. Killing after `run` would never be reached.

        This is the case the gate's own header promises: a wedged external
        command fails the gate rather than hanging it. Without the `finally`
        the GateError raises past the kill and `Popen.__exit__` blocks forever.
        """
        child = RecordingChild()

        def wedged(*_arguments, **_keywords):
            raise gate.GateError("gpg exceeded its 1800s deadline")

        with self.assertRaises(gate.GateError):
            verify_with(child, wedged)
        self.assertEqual(child.kills_before_wait, 1)


class HostReadBackCaseTests(unittest.TestCase):
    """Both host cases are accounted for, separately, however the host is configured."""

    def test_a_host_that_publishes_its_configuration_reports_both_cases(self):
        """Positive: two outcomes, two reports, both of them a pass."""
        outcomes, printed = host_readback(
            [row("CONFIG_SCHED_CLASS_EXT", "built-in")], "7.2.5-aegis-m26", PRODUCED
        )
        self.assertEqual(outcomes, [[], []])
        self.assertIn("PASS kernel/readback-negative-missing-option", printed)
        self.assertIn("PASS kernel/readback-boundary-host-kernel", printed)

    def test_a_failing_negative_case_stays_its_own_failure_unit(self):
        """Negative: one outcome list per case, so two failures cannot count as one."""
        outcomes, printed = host_readback(
            [row("CONFIG_PREEMPT_RT", "built-in")], "7.2.5-aegis-m26", PRODUCED
        )
        self.assertEqual(len(outcomes), 2)
        self.assertTrue(outcomes[0])
        self.assertEqual(outcomes[1], [])
        self.assertIn("FAIL kernel/readback-negative-missing-option", printed)
        self.assertIn("PASS kernel/readback-boundary-host-kernel", printed)

    def test_a_host_without_a_published_configuration_skips_only_the_negative_case(self):
        """Boundary: the boundary case needs no host configuration and must still run."""
        outcomes, printed = host_readback(
            [row("CONFIG_PREEMPT_RT", "built-in")], "7.2.5-aegis-m26", None
        )
        self.assertEqual(outcomes, [[]])
        self.assertIn("SKIP kernel/readback-negative-missing-option", printed)
        self.assertIn("PASS kernel/readback-boundary-host-kernel", printed)
        self.assertIn(os.uname().release, printed)


if __name__ == "__main__":
    unittest.main()
