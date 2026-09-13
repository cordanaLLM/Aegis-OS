#!/usr/bin/env python3
"""Check what the eBPF verifier gate asserts, without a kernel, a loader or sudo.

`make verify-bpf` needs CAP_BPF, a BTF-enabled kernel and, for one case, the
machine's CPU scheduler. None of that exists on a CI runner, so these tests
cover the half that can be checked anywhere: that the gate's pinned constants
and the admission table agree, that each mutation the negative cases rely on is
present exactly once and actually changes its source, and that the fixture the
gate loads is the observe-only one it is documented to be.

They do not load anything. A pass here says the gate would assert the right
things; it says nothing about whether a verifier accepted an object.
"""

import inspect
import re
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import verify_bpf_objects as gate

ROOT = Path(__file__).resolve().parent.parent
BPF_DIR = ROOT / "bpf"
LOADER = BPF_DIR / "loader/aegis_bpf_probe.c"
ADMISSION = ROOT / "docs/roadmap/toolchain-admission.md"
EVIDENCE = ROOT / "docs/build/bpf.md"
MAKEFILE = ROOT / "Makefile"


def source(name):
    return (BPF_DIR / name).read_text(encoding="utf-8")


def recipe_lines(text, target):
    """The recipe lines of one Makefile target, with the leading tab removed.

    Parsed rather than substring-matched: what a suppression check has to see is
    the LINE make will run, not whether one spelling appears somewhere in the
    file.
    """
    recipe = []
    collecting = False
    for line in text.splitlines():
        if line.startswith(f"{target}:"):
            collecting = True
        elif collecting and line.startswith("\t"):
            recipe.append(line[1:].strip())
        elif collecting and line.strip():
            break
    return recipe


def suppresses(line):
    """Whether make would lose this recipe line's exit status.

    A leading `-` tells make to ignore the status; `|`, `;` and `&` introduce a
    shell construct that can supply a different one. Any of them defeats the
    gate, and `|| true` is only the most obvious spelling.
    """
    return line.startswith("-") or line.startswith("@-") or any(t in line for t in "|;&")


class MutationRegions(unittest.TestCase):
    """Every negative case deletes exactly one region, and the deletion bites."""

    def test_each_region_is_present_exactly_once(self):
        for _, source_name, region, _, _ in gate.NEGATIVE_CASES:
            text = source(f"{source_name}.bpf.c")
            self.assertEqual(
                text.count(f"// AEGIS-MUTATE-BEGIN {region}"), 1, f"{source_name}/{region} BEGIN"
            )
            self.assertEqual(
                text.count(f"// AEGIS-MUTATE-END {region}"), 1, f"{source_name}/{region} END"
            )

    def test_mutation_changes_the_source(self):
        """A marker pair that deleted nothing would make the negative case vacuous."""
        import tempfile

        with tempfile.TemporaryDirectory(prefix="aegis-bpf-test-") as base:
            paths = {"scratch": Path(base)}
            for _, source_name, region, _, _ in gate.NEGATIVE_CASES:
                mutated, removed = gate.mutate(source_name, region, paths)
                self.assertGreater(removed, 0, f"{source_name}/{region} removed no lines")
                original = source(f"{source_name}.bpf.c")
                self.assertNotEqual(mutated.read_text(encoding="utf-8"), original)
                self.assertLess(len(mutated.read_text(encoding="utf-8")), len(original))

    def test_an_unknown_region_is_refused(self):
        """Negative: a renamed marker must fail the gate, not silently no-op."""
        import tempfile

        with tempfile.TemporaryDirectory(prefix="aegis-bpf-test-") as base:
            with self.assertRaises(gate.GateError):
                gate.mutate("action_gate", "no-such-region", {"scratch": Path(base)})

    def test_each_negative_expects_a_distinct_diagnostic(self):
        """Three mutations that all matched one string would prove one thing, not three."""
        rejections = [case[4] for case in gate.NEGATIVE_CASES]
        self.assertEqual(len(set(rejections)), len(rejections))
        for rejection in rejections:
            self.assertFalse(gate.BOOKKEEPING.match(rejection))


class PinnedValues(unittest.TestCase):
    """Pinned literals are compared with their values, never with themselves."""

    def test_tdp_literal_is_the_recorded_value(self):
        self.assertEqual(gate.TDP_LITERAL_MILLIWATT, 15000)
        match = re.search(rf"#define {gate.TDP_MACRO} (\d+)ULL", source("aegis_bpf_abi.h"))
        self.assertIsNotNone(match)
        self.assertEqual(int(match.group(1)), 15000)

    def test_the_tdp_macro_and_its_field_say_they_are_unmeasured(self):
        self.assertIn("_UNMEASURED", gate.TDP_MACRO)
        self.assertIn("pseudo_energy_uj_unmeasured", source("aegis_bpf_abi.h"))
        self.assertIn("pseudo_energy_uj_unmeasured", source("kepler_power.bpf.c"))

    def test_kepler_slot_bound_is_a_power_of_two_at_its_recorded_value(self):
        match = re.search(r"#define AEGIS_KEPLER_SLOTS (\d+)u", source("aegis_bpf_abi.h"))
        self.assertIsNotNone(match)
        slots = int(match.group(1))
        self.assertEqual(slots, 8)
        self.assertEqual(slots & (slots - 1), 0, "the mask bound needs a power of two")

    def test_stub_watchdog_bound_is_the_recorded_value(self):
        match = re.search(r"#define AEGIS_CAKE_STUB_TIMEOUT_MS (\d+)u", source("aegis_bpf_abi.h"))
        self.assertIsNotNone(match)
        self.assertEqual(int(match.group(1)), 5000)

    def test_reference_versions_are_at_or_above_their_floors(self):
        pairs = (
            (gate.REFERENCE_PROFILE_CLANG, r"(\d+\.\d+\.\d+)", gate.CLANG_FLOOR),
            (gate.REFERENCE_PROFILE_BPFTOOL, r"v(\d+\.\d+\.\d+)", gate.BPFTOOL_FLOOR),
            (gate.REFERENCE_PROFILE_LIBBPF, r"^(\d+\.\d+)", gate.LIBBPF_FLOOR),
        )
        for reference, pattern, floor in pairs:
            found = gate.parse_version(reference, pattern)
            self.assertIsNotNone(found, reference)
            self.assertGreaterEqual(found, floor, reference)


class StubHandlerSet(unittest.TestCase):
    """ "All handlers stubbed" is an enumeration, and it is checked as one."""

    def test_the_recorded_set_is_exactly_what_the_source_declares(self):
        text = source("scx_cake_stub.bpf.c")
        declared = set(re.findall(r'SEC\("struct_ops/(\w+)"\)', text))
        self.assertEqual(declared, set(gate.STUB_HANDLERS))

    def test_every_recorded_handler_is_assigned_in_the_ops_struct(self):
        text = source("scx_cake_stub.bpf.c")
        assigned = set(re.findall(r"\.\w+ = \(void \*\)(\w+),", text))
        self.assertEqual(assigned, set(gate.STUB_HANDLERS))

    def test_the_stub_declares_switch_partial_and_a_watchdog_bound(self):
        """Boundary: the stub is safe to attach only because of these two fields."""
        text = source("scx_cake_stub.bpf.c")
        self.assertIn(".flags = SCX_OPS_SWITCH_PARTIAL,", text)
        self.assertIn(".timeout_ms = AEGIS_CAKE_STUB_TIMEOUT_MS,", text)

    def test_the_stub_set_is_narrower_than_the_kernel_ops_struct(self):
        """Negative: the claim is nine declared handlers, not every sched_ext hook."""
        self.assertEqual(len(gate.STUB_HANDLERS), 9)


class ObserveOnlyFixture(unittest.TestCase):
    """The LSM fixture must not be able to deny an exec."""

    def test_the_lsm_program_returns_only_allow_or_the_incoming_verdict(self):
        text = source("action_gate.bpf.c")
        returns = set(re.findall(r"^\s*return (.+);$", text, re.M))
        self.assertEqual(returns, {"ret", "AEGIS_ACTION_ALLOW"})

    def test_allow_is_zero(self):
        match = re.search(r"#define AEGIS_ACTION_ALLOW (\S+)", source("aegis_bpf_abi.h"))
        self.assertIsNotNone(match)
        self.assertEqual(match.group(1), "0")

    def test_no_fixture_calls_a_denying_helper(self):
        """A check over these four sources, not a proof about BPF objects at large."""
        for name in ("action_gate", "kepler_power", "scx_cake", "scx_cake_stub"):
            text = source(f"{name}.bpf.c")
            for banned in ("bpf_send_signal", "bpf_override_return", "bpf_probe_write_user"):
                self.assertNotIn(banned, text, f"{name} calls {banned}")


class Provenance(unittest.TestCase):
    """D66: bpf/scx_cake.bpf.c records what it is and is not."""

    def test_scx_cake_records_its_provenance_against_the_distribution_package(self):
        text = source("scx_cake.bpf.c")
        self.assertIn("PROVENANCE", text)
        self.assertIn("scx-scheds 1.1.3-2", text)
        self.assertIn("/usr/bin/scx_cake", text)
        self.assertIn("not a fork", text)

    def test_every_tracked_bpf_source_carries_an_spdx_identifier(self):
        # REUSE-IgnoreStart -- the expected tag below is test data, not this
        # file's own licence declaration; without these markers `reuse lint`
        # reads it as one and reports an invalid expression.
        expected = "SPDX-License-Identifier: EUPL-1.2"
        # REUSE-IgnoreEnd
        for path in sorted(BPF_DIR.rglob("*.c")) + sorted(BPF_DIR.rglob("*.h")):
            head = path.read_text(encoding="utf-8")[:400]
            self.assertIn(expected, head, str(path))


class GateWiring(unittest.TestCase):
    """Where the gate lives, and the fact that verify-all deliberately omits it."""

    def test_the_makefile_declares_a_verify_bpf_target(self):
        text = MAKEFILE.read_text(encoding="utf-8")
        self.assertIn("verify-bpf:", text)
        self.assertIn("python3 tools/verify_bpf_objects.py", text)

    def test_verify_all_does_not_invoke_the_bpf_gate(self):
        """Negative: a gate that can only skip on the runner is not a gate."""
        text = MAKEFILE.read_text(encoding="utf-8")
        recipe = text.split("verify-all:", 1)[1].split("\n\n", 1)[0]
        self.assertNotIn("verify-bpf", recipe)

    def test_the_gate_never_suppresses_a_failure(self):
        """Positive: the shipped recipe line hands make its own exit status."""
        recipe = recipe_lines(MAKEFILE.read_text(encoding="utf-8"), "verify-bpf")
        self.assertEqual(len(recipe), 1, recipe)
        self.assertIn("tools/verify_bpf_objects.py", recipe[0])
        self.assertFalse(suppresses(recipe[0]), recipe[0])

    def test_every_suppression_spelling_is_caught_not_just_one(self):
        """Negative: asserting the absence of `... || true` passes for four other
        spellings that suppress the failure exactly as well."""
        for line in (
            "-python3 tools/verify_bpf_objects.py",
            "@-python3 tools/verify_bpf_objects.py",
            "python3 tools/verify_bpf_objects.py || true",
            "python3 tools/verify_bpf_objects.py || :",
            "python3 tools/verify_bpf_objects.py ; true",
            "python3 tools/verify_bpf_objects.py & ",
        ):
            self.assertTrue(suppresses(line), line)

    def test_the_recipe_parser_reads_the_right_target(self):
        """Boundary: the parser stops at the next target rather than running on."""
        text = "alpha:\n\tfirst\n\tsecond\n\nbeta:\n\tthird\n"
        self.assertEqual(recipe_lines(text, "alpha"), ["first", "second"])
        self.assertEqual(recipe_lines(text, "beta"), ["third"])
        self.assertEqual(recipe_lines(text, "gamma"), [])

    def test_the_admission_table_names_every_tool_the_gate_runs(self):
        text = ADMISSION.read_text(encoding="utf-8")
        for value in (
            gate.REFERENCE_PROFILE_CLANG_PACKAGE,
            gate.REFERENCE_PROFILE_BPFTOOL_PACKAGE,
            gate.REFERENCE_PROFILE_LIBBPF_PACKAGE,
            gate.REFERENCE_PROFILE_LIBBPF,
            gate.REFERENCE_PROFILE_BPFTOOL_BUILTIN_LIBBPF,
        ):
            self.assertIn(value, text, f"the admission table omits {value!r}")

    def test_the_admission_table_separates_the_two_libbpf_readings(self):
        """The register recorded libbpf v1.8 from bpftool's banner; the linked
        library is 1.7.0, and both must be on the page, distinctly."""
        text = ADMISSION.read_text(encoding="utf-8")
        self.assertIn("pkg-config --modversion libbpf", text)
        self.assertIn("bpftool version", text)

    def test_the_evidence_page_states_the_non_qualifying_scope(self):
        text = EVIDENCE.read_text(encoding="utf-8").lower()
        self.assertIn("non-qualifying", text)
        self.assertIn("m10", text)

    def test_the_capability_set_records_perfmon_as_well_as_bpf(self):
        """CAP_BPF alone returned -EPERM; the recorded set is the one that ran."""
        self.assertIn("+bpf", gate.CAPABILITIES)
        self.assertIn("+perfmon", gate.CAPABILITIES)
        self.assertTrue(gate.CAPABILITIES.startswith("-all,"))


class LogStaleness(unittest.TestCase):
    """A verifier log is evidence only when THIS load wrote it.

    The defect: the probe returns non-zero WITHOUT writing when its fopen fails,
    and the gate read the file off disk anyway. A non-verifier failure plus a
    file an earlier run left then read as a verifier rejection.
    """

    def test_the_loader_gives_a_log_write_failure_its_own_exit_code(self):
        text = LOADER.read_text(encoding="utf-8")
        self.assertIn("#define AEGIS_EXIT_LOG_FAILED 6", text)
        self.assertIn("#define AEGIS_EXIT_LOAD_FAILED 1", text)
        self.assertEqual(gate.PROBE_EXIT_LOG_FAILED, 6)
        self.assertEqual(gate.PROBE_EXIT_LOAD_FAILED, 1)
        self.assertNotEqual(gate.PROBE_EXIT_LOG_FAILED, gate.PROBE_EXIT_LOAD_FAILED)

    def test_the_write_failure_path_returns_that_code(self):
        """Negative: the one path that used to collapse into a load failure."""
        text = LOADER.read_text(encoding="utf-8")
        body = text.split("static int load_object", 1)[1]
        write_failure = body.split("write_logs(", 1)[1].split("report_programs", 1)[0]
        self.assertIn("return AEGIS_EXIT_LOG_FAILED;", write_failure)

    def test_the_loader_stamps_the_nonce_the_gate_requires(self):
        self.assertEqual(gate.NONCE_HEADER, "# run-nonce")
        self.assertIn('fprintf(out, "# run-nonce %s\\n", nonce);', LOADER.read_text("utf-8"))
        self.assertIn('"--nonce",', inspect.getsource(gate.probe))

    def test_the_loader_refuses_a_run_without_a_nonce(self):
        """Boundary: --nonce is required, so an unstamped log cannot be produced."""
        text = LOADER.read_text(encoding="utf-8")
        self.assertIn("options->nonce == NULL", text)
        self.assertIn("valid_nonce(options->nonce)", text)

    def test_a_log_carrying_this_runs_nonce_is_read(self):
        with tempfile.TemporaryDirectory(prefix="aegis-bpf-test-") as base:
            log = Path(base) / "one.log"
            log.write_text(f"{gate.NONCE_HEADER} abc-123\nR7 invalid mem access\n", "utf-8")
            paths = {"nonces": {str(log): "abc-123"}}
            self.assertIn("R7 invalid mem access", gate.stamped_text(paths, log))

    def test_a_log_this_run_did_not_write_is_not_read(self):
        """Negative: the hand-typed file that used to pass the headline case."""
        with tempfile.TemporaryDirectory(prefix="aegis-bpf-test-") as base:
            log = Path(base) / "one.log"
            log.write_text("TYPED BY HAND\nR7 invalid mem access\n", encoding="utf-8")
            self.assertIsNone(gate.stamped_text({"nonces": {str(log): "abc-123"}}, log))
            self.assertIsNone(gate.stamped_text({"nonces": {}}, log))

    def test_log_problems_names_the_write_failure_and_the_missing_nonce(self):
        with tempfile.TemporaryDirectory(prefix="aegis-bpf-test-") as base:
            log = Path(base) / "one.log"
            log.write_text("TYPED BY HAND\nR7 invalid mem access\n", encoding="utf-8")
            stamp = {"path": log, "nonce": "abc-123", "removal": None}
            text, problems = gate.log_problems(stamp, gate.PROBE_EXIT_LOG_FAILED)
            self.assertIsNone(text)
            self.assertTrue(any("could not WRITE" in line for line in problems), problems)
            self.assertTrue(any("does not carry this load's nonce" in p for p in problems))

    def test_log_problems_passes_a_log_this_load_wrote(self):
        """Positive: the same shape, stamped, is read back with no problem."""
        with tempfile.TemporaryDirectory(prefix="aegis-bpf-test-") as base:
            log = Path(base) / "one.log"
            log.write_text(f"{gate.NONCE_HEADER} abc-123\n# programs: 1\n", encoding="utf-8")
            text, problems = gate.log_problems(
                {"path": log, "nonce": "abc-123", "removal": None}, 0
            )
            self.assertEqual(problems, [])
            self.assertIn("# programs: 1", text)

    def test_remove_log_deletes_the_previous_file(self):
        with tempfile.TemporaryDirectory(prefix="aegis-bpf-test-") as base:
            log = Path(base) / "one.log"
            log.write_text("stale\n", encoding="utf-8")
            self.assertIsNone(gate.remove_log(log))
            self.assertFalse(log.exists())

    def test_remove_log_accepts_a_path_that_is_not_there(self):
        """Boundary: the first run of a case has nothing to remove."""
        with tempfile.TemporaryDirectory(prefix="aegis-bpf-test-") as base:
            self.assertIsNone(gate.remove_log(Path(base) / "absent.log"))

    def test_remove_log_reports_a_file_it_cannot_remove(self):
        """Negative: exactly the case where a stale log survives the load."""
        with tempfile.TemporaryDirectory(prefix="aegis-bpf-test-") as base:
            directory = Path(base) / "logs"
            directory.mkdir()
            log = directory / "one.log"
            log.write_text("stale\n", encoding="utf-8")
            directory.chmod(0o555)
            try:
                problem = gate.remove_log(log)
            finally:
                directory.chmod(0o755)
            self.assertIsNotNone(problem)
            self.assertIn("could not be removed", problem)

    def test_the_rejection_is_not_read_out_of_an_unstamped_log(self):
        """The rejection check refuses to look at a log log_problems rejected."""
        self.assertEqual(gate.rejection_problems(None, "invalid mem access"), [])
        self.assertEqual(gate.rejection_problems("R7 invalid mem access\n", "invalid mem"), [])
        self.assertEqual(len(gate.rejection_problems("nothing here\n", "invalid mem")), 1)


class GuardOrder(unittest.TestCase):
    """A host without the toolchain SKIPs, which requires PATH to be checked first."""

    def test_a_missing_tool_is_found_before_any_tool_is_invoked(self):
        """Negative: read_tool_versions() INVOKES four of these, and an absent
        binary raises GateError there -- a FAIL, not the documented SKIP."""

        def explode(*args, **kwargs):
            raise AssertionError("a tool was invoked before PATH was checked")

        with mock.patch.object(gate.shutil, "which", return_value=None):
            with mock.patch.object(gate, "run", explode):
                reason, versions, config = gate.guard()
        self.assertEqual(reason, "clang is not on PATH")
        self.assertIsNone(versions)
        self.assertIsNone(config)

    def test_the_path_check_passes_when_every_tool_resolves(self):
        with mock.patch.object(gate.shutil, "which", return_value="/usr/bin/anything"):
            self.assertIsNone(gate.missing_tool())

    def test_every_tool_the_version_reader_runs_is_checked_for_presence(self):
        """Boundary: the four the version reader executes are the ones that made
        the SKIP unreachable, so they must all be on the PATH list."""
        text = inspect.getsource(gate.read_tool_versions)
        for tool in ("clang", "bpftool", "pkg-config", "llvm-strip"):
            self.assertIn(f'"{tool}"', text)
            self.assertIn(tool, gate.REQUIRED_TOOLS)
        self.assertIn("sudo", gate.REQUIRED_TOOLS)
        self.assertIn("setpriv", gate.REQUIRED_TOOLS)


class SchedExtCounters(unittest.TestCase):
    """The counters exist, one directory above where they were looked for."""

    def test_the_counters_are_top_level_attributes_not_members_of_root(self):
        self.assertEqual(gate.SCHED_EXT_COUNTERS, ("switch_all", "nr_rejected", "enable_seq"))
        for name in gate.SCHED_EXT_COUNTERS:
            path = str(gate.SCHED_EXT_DIR / name)
            self.assertEqual(path, f"/sys/kernel/sched_ext/{name}")
            self.assertNotIn("root", path)
        self.assertEqual(str(gate.SCHED_EXT_OPS), "/sys/kernel/sched_ext/root/ops")
        self.assertEqual(str(gate.SCHED_EXT_STATE), "/sys/kernel/sched_ext/state")

    def test_the_probe_reads_them_from_inside_the_hold_window(self):
        """A reading taken after the link is released is not a reading during it."""
        text = LOADER.read_text(encoding="utf-8")
        self.assertIn('print_sched_ext_counters("during");', text)
        self.assertIn('print_sched_ext_counters("before");', text)
        self.assertIn('print_sched_ext_counters("after");', text)
        for name in gate.SCHED_EXT_COUNTERS:
            self.assertIn(f'"{name}",', text)

    def test_the_during_readings_are_parsed_from_the_probes_output(self):
        found = gate.during_counters(
            "attached=struct_ops map=x\nsched_ext_switch_all_during=0\n"
            "sched_ext_nr_rejected_during=0\nsched_ext_enable_seq_during=18\n"
        )
        self.assertEqual(found, {"switch_all": "0", "nr_rejected": "0", "enable_seq": "18"})

    def test_a_counter_the_probe_did_not_print_reads_as_absent(self):
        """Negative: a silently missing reading must not look like a zero."""
        found = gate.during_counters("attached=struct_ops map=x\n")
        self.assertEqual(found, {"switch_all": None, "nr_rejected": None, "enable_seq": None})

    def test_switch_all_must_read_zero_while_the_stub_holds_root_ops(self):
        """The kernel's own measurement of 'no task was switched to it', which is
        a different thing from the SCX_OPS_SWITCH_PARTIAL flag in the source."""
        stdout = "sched_ext_ops_during=aegis_cake_stub\ndetach_rc=0\n"
        self.assertEqual(gate.SWITCH_ALL_PARTIAL, "0")
        clean = {"switch_all": "0", "nr_rejected": "0", "enable_seq": "18"}
        self.assertEqual(gate.attach_problems(stdout, clean), [])
        switched = dict(clean, switch_all="1")
        self.assertTrue(any("switch_all read" in p for p in gate.attach_problems(stdout, switched)))
        absent = dict(clean, switch_all=None)
        problems = gate.attach_problems(stdout, absent)
        self.assertTrue(any("was not captured" in p for p in problems), problems)

    def test_the_note_separates_the_monotonic_counter_from_the_per_instance_ones(self):
        """enable_seq is incremented and never reset; nr_rejected is set to 0 by
        every enable. One blanket 'they all reset' claim is wrong either way."""
        before = {"switch_all": "1", "nr_rejected": "0", "enable_seq": "17", "events": "a\nb"}
        during = {"switch_all": "0", "nr_rejected": "0", "enable_seq": "18"}
        after = {"switch_all": "1", "nr_rejected": "0", "enable_seq": "19", "events": "a\nb"}
        lines = "\n".join(gate.counter_note(before, during, after))
        self.assertIn("switch_all: before=1 during=0 after=1", lines)
        self.assertIn("2 scheduler enable(s)", lines)
        self.assertIn("monotonic since boot", lines)
        self.assertIn("nr_rejected is reset to 0 by every scheduler enable", lines)
        self.assertIn("SCX_EV_* lines", lines)

    def test_an_unreadable_enable_seq_is_not_silently_subtracted(self):
        """Boundary: absent counters must not produce a fabricated delta."""
        absent = {"enable_seq": "<absent>"}
        self.assertIn("unreadable", gate.enable_seq_delta(absent, absent))

    def test_the_supervisor_calls_run_with_the_privilege_they_need(self):
        """The methods are polkit-gated: an unprivileged busctl gets `Not
        allowed!`, so a recorded command without sudo reproduces nothing."""
        text = inspect.getsource(gate.loader_call)
        self.assertIn('"sudo"', text)
        self.assertIn('"-n"', text)
        self.assertIn('"busctl"', text)


class EvidencePage(unittest.TestCase):
    """The prose says where the counters are, and says it correctly."""

    def test_the_evidence_page_places_the_counters_at_the_top_level(self):
        text = EVIDENCE.read_text(encoding="utf-8")
        self.assertIn("/sys/kernel/sched_ext/switch_all", text)
        self.assertIn("/sys/kernel/sched_ext/nr_rejected", text)
        self.assertNotIn("no `nr_rejected` and no `switch_all`", text)

    def test_the_evidence_page_records_the_measured_switch_all_reading(self):
        text = EVIDENCE.read_text(encoding="utf-8")
        self.assertIn("switch_all", text)
        self.assertIn("SCX_OPS_SWITCH_PARTIAL", text)


if __name__ == "__main__":
    unittest.main()
