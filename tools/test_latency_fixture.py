"""Regressions for the latency fixture: the verdict rule, the pins and the D57 surface.

The gate itself needs a built kernel, an emulator and `cyclictest`. These tests
need none of them. They exercise the gate's own decisions -- which verdict a
figure reaches, which thresholds it reads out of the crates, which programs it
may invoke at all, and whether the recorded admission still matches the page --
so a checkout that cannot boot a guest still fails when one of those drifts.
"""

import ast
import io
import re
import unittest
from contextlib import redirect_stdout
from pathlib import Path

import verify_latency_fixture as gate

ROOT = Path(__file__).resolve().parent.parent
ADMISSION = ROOT / "docs" / "roadmap" / "toolchain-admission.md"
MEASURED = ROOT / "crates" / "aegis-calliope" / "src" / "measured.rs"
GUEST_INIT = ROOT / "tools" / "guest" / "aegis-latency-init.sh"
PROBE = ROOT / "tools" / "guest" / "aegis-preempt-probe.sh"
GATE = ROOT / "tools" / "verify_latency_fixture.py"

# The admission page is prose plus tables; this bounds the scan over it.
MAX_PAGE_LINES = 4000
ADMISSION_HEADING = "## The latency fixture's toolchain"
TABLE_ROW = re.compile(r"^\|([^|]*)\|([^|]*)\|([^|]*)\|[^|]*\|$")

# Every program the gate is allowed to start. This list is the D57 claim in
# mechanical form: nothing here installs a package, writes a bootloader entry,
# loads a module or touches firmware, and `pacman` appears only as `-Qo`/`-Qq`,
# which are queries. A program added to the gate without being added here fails
# `make verify-all`.
ALLOWED_PROGRAMS = {
    "qemu-system-x86_64",
    "cyclictest",
    "cpio",
    "ldd",
    "bash",
    "mount",
    "cat",
    "grep",
    "gzip",
    "pacman",
}

# The thresholds the gate reads out of the crates, pinned to their values here
# rather than to themselves.
EXPECTED_THRESHOLDS = {
    "BURST_CRITICAL_NS": 100_000,
    "BURST_INTERACTIVE_NS": 2_000_000,
    "BURST_FRAME_NS": 8_000_000,
    "TARGET_RTL_LATENCY_NS": 5_000_000,
}


# The four edges in the order the gate reads them out of the crates. The values
# come from EXPECTED_THRESHOLDS above, which pins them to literals, so a case
# exercised here is exercised against the same numbers the gate evaluates.
FIXTURE_EDGES = [
    ("BURST_CRITICAL_NS", EXPECTED_THRESHOLDS["BURST_CRITICAL_NS"]),
    ("BURST_INTERACTIVE_NS", EXPECTED_THRESHOLDS["BURST_INTERACTIVE_NS"]),
    ("BURST_FRAME_NS", EXPECTED_THRESHOLDS["BURST_FRAME_NS"]),
    ("TARGET_RTL_LATENCY_NS", EXPECTED_THRESHOLDS["TARGET_RTL_LATENCY_NS"]),
]


def measured_figures(version, worst, release="7.2.5-aegis-m26"):
    """Return one `parse_report` shape carrying the kernel version under test.

    The figures are the recorded ones only so the rows are recognisable; what
    each case here varies is the version line, because that is the reading the
    verdict rule is fed.
    """
    arguments = "/bin/cyclictest " + " ".join(gate.CYCLICTEST_ARGUMENTS) + " --json=/a.json"
    return {
        "release": release,
        "version": version,
        "cycles": 50_000,
        "min": 1510,
        "avg": 9664.0,
        "max": worst,
        "arguments": arguments,
    }


def printed_rows(text):
    """Return the `name = threshold ns -> placement, verdict` lines of one report."""
    return [line.strip() for line in text.splitlines() if " -> " in line]


def admitted_rows():
    """Return the M23 admission table as ``{name: (reference, floor)}``."""
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


def literal_head(node):
    """Return the first element of an argv literal when it is a plain string."""
    if isinstance(node, (ast.List, ast.Tuple)) and node.elts:
        first = node.elts[0]
        if isinstance(first, ast.Constant) and isinstance(first.value, str):
            return first.value
    return None


def bound_heads(scope):
    """Return ``{variable: {possible argv[0]}}`` for argv lists bound inside `scope`.

    The scope is one function, not the module. Resolving across the whole
    module was the first form of this sweep and it was wrong in a way that
    hid call sites rather than reporting them: two functions each bind a local
    named ``argv``, so a call site whose vector came from somewhere else
    entirely resolved to the other function's literal and looked decided.
    """
    bound = {}
    for node in ast.walk(scope):
        if not isinstance(node, ast.Assign):
            continue
        head = literal_head(node.value)
        if head is None:
            continue
        for target in node.targets:
            if isinstance(target, ast.Name):
                bound.setdefault(target.id, set()).add(head)
    return bound


def call_sites(path):
    """Return (programs the gate can start, undecidable call sites by function).

    A call site whose program this sweep cannot name is returned rather than
    dropped: an allowlist with a silent hole states nothing. The gate has
    exactly one, and the caller resolves it from the admitted toolchain.
    """
    tree = ast.parse(path.read_text(encoding="utf-8"))
    resolved = set()
    undecided = []
    for scope in ast.walk(tree):
        if not isinstance(scope, ast.FunctionDef):
            continue
        bound = bound_heads(scope)
        for node in ast.walk(scope):
            if not isinstance(node, ast.Call) or not node.args:
                continue
            name = getattr(node.func, "id", None) or getattr(node.func, "attr", None)
            if name not in {"run", "Popen"}:
                continue
            argv = node.args[0]
            head = literal_head(argv)
            if head is not None:
                resolved.add(head)
            elif isinstance(argv, ast.Name) and argv.id in bound:
                resolved |= bound[argv.id]
            else:
                undecided.append(scope.name)
    return resolved, undecided


def deadlined_call_sites(path):
    """Return (total, with a deadline) over the gate's own external call sites."""
    tree = ast.parse(path.read_text(encoding="utf-8"))
    total = 0
    deadlined = 0
    for node in ast.walk(tree):
        if not isinstance(node, ast.Call):
            continue
        name = getattr(node.func, "id", None) or getattr(node.func, "attr", None)
        if name not in {"run", "Popen", "check_output", "call"}:
            continue
        if name == "run" and isinstance(node.func, ast.Name):
            total += 1
            deadlined += 1 if len(node.args) >= 2 else 0
            continue
        if isinstance(node.func, ast.Attribute) and name in {"run", "Popen"}:
            total += 1
            keywords = {keyword.arg for keyword in node.keywords}
            deadlined += 1 if "timeout" in keywords else 0
    return total, deadlined


class AdmissionTests(unittest.TestCase):
    """The page and the gate must state the same admission."""

    def test_every_admitted_tool_appears_on_both_sides(self):
        page = admitted_rows()
        code = {row[0]: (row[4], row[3]) for row in gate.TOOLCHAIN}
        self.assertEqual(set(page), set(code), "the page and the gate admit different tools")
        for name, (reference, floor) in code.items():
            self.assertEqual(page[name], (reference, floor), f"{name} differs between the two")

    def test_the_page_records_the_reused_kernel_pin_and_the_path_not_taken(self):
        text = ADMISSION.read_text()
        self.assertIn("rt-tests 2.10-1.1", text)
        self.assertIn("linux-7.2.5", text)
        self.assertIn("pacman -Qq linux-rt", text)
        self.assertIn("by construction", text)

    def test_a_floor_is_never_above_its_recorded_reference(self):
        for name, _argv, _pattern, floor, reference in gate.TOOLCHAIN:
            if floor is None:
                continue
            self.assertLessEqual(
                gate.version_tuple(floor),
                gate.version_tuple(reference),
                f"{name} records a reference below its own floor",
            )


class VerdictRuleTests(unittest.TestCase):
    """The kernel decides before the figure does."""

    def test_a_realtime_kernel_reaches_every_outcome(self):
        self.assertEqual(gate.verdict(True, "attained"), "satisfied")
        self.assertEqual(gate.verdict(True, "at-edge"), "worst-case-at-edge")
        self.assertEqual(gate.verdict(True, "exceeded"), "worst-case-exceeds")

    def test_a_non_realtime_kernel_reaches_one_outcome_whatever_the_figure(self):
        for placed in ("attained", "at-edge", "exceeded"):
            self.assertEqual(gate.verdict(False, placed), "kernel-not-realtime")

    def test_a_perfect_figure_on_a_non_realtime_kernel_is_still_refused(self):
        placed = gate.attainment(EXPECTED_THRESHOLDS["BURST_CRITICAL_NS"], 0)
        self.assertEqual(placed, "attained")
        self.assertEqual(gate.verdict(False, placed), "kernel-not-realtime")

    def test_the_three_points_around_a_threshold_are_three_outcomes(self):
        edge = EXPECTED_THRESHOLDS["BURST_CRITICAL_NS"]
        self.assertEqual(edge, 100_000)
        placed = [
            gate.attainment(edge, edge - 1),
            gate.attainment(edge, edge),
            gate.attainment(edge, edge + 1),
        ]
        self.assertEqual(placed, ["attained", "at-edge", "exceeded"])
        decided = [gate.verdict(True, one) for one in placed]
        self.assertEqual(len(set(decided)), 3)

    def test_the_boundary_case_reports_three_outcomes(self):
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.boundary_case([("BURST_CRITICAL_NS", 100_000)])
        self.assertEqual(problems, [])
        self.assertIn("PASS latency/boundary-edge", printed.getvalue())
        self.assertIn("not measurements", printed.getvalue())


class MeasuringKernelReadingTests(unittest.TestCase):
    """The rows a case prints and the outcome it returns come from one reading.

    `placement_rows` was handed a literal at both of its call sites, so the
    printed verdicts stated what the caller asserted while the decision came
    from a separate `if` over the figures: a measurement taken on a kernel
    without PREEMPT_RT printed `satisfied` rows above the reason they did not
    count, and the host case could not have noticed a host that had become
    realtime at all. Both now place their rows with the version line the
    measuring kernel wrote into its own JSON, which is the shape
    `Measured::verdict` has in `aegis-calliope`.
    """

    def test_the_reading_is_the_kernels_own_version_line(self):
        self.assertTrue(gate.reports_realtime("#1 SMP PREEMPT_RT Sun Sep 13 18:55:36 CEST 2026"))
        self.assertFalse(gate.reports_realtime("#1 SMP PREEMPT_DYNAMIC"))
        self.assertFalse(gate.reports_realtime(""))

    def test_neither_case_hands_the_rule_a_literal(self):
        """Read out of the gate's syntax tree, so a literal cannot come back."""
        calls = [
            node
            for node in ast.walk(ast.parse(GATE.read_text()))
            if isinstance(node, ast.Call)
            and isinstance(node.func, ast.Name)
            and node.func.id == "placement_rows"
        ]
        self.assertEqual(len(calls), 2, "the sweep did not find both call sites")
        for call in calls:
            flag = call.args[1]
            self.assertIsInstance(flag, ast.Name, f"a literal reached the rule: {ast.dump(flag)}")
            self.assertEqual(flag.id, "realtime")

    def test_a_guest_that_is_not_realtime_prints_no_satisfied_row(self):
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.guest_measured_case(
                measured_figures("#1 SMP PREEMPT_DYNAMIC", 273_969),
                FIXTURE_EDGES,
                "7.2.5-aegis-m26",
            )
        rows = printed_rows(printed.getvalue())
        self.assertEqual(len(rows), len(FIXTURE_EDGES))
        for row in rows:
            self.assertTrue(row.endswith("kernel-not-realtime"), row)
        self.assertTrue(any("not PREEMPT_RT" in line for line in problems))
        self.assertIn("FAIL latency/guest-measured", printed.getvalue())

    def test_a_realtime_guest_still_reaches_the_recorded_outcomes(self):
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.guest_measured_case(
                measured_figures("#1 SMP PREEMPT_RT", 273_969),
                FIXTURE_EDGES,
                "7.2.5-aegis-m26",
            )
        self.assertEqual(problems, [])
        rows = printed_rows(printed.getvalue())
        self.assertTrue(rows[0].endswith("worst-case-exceeds"), rows[0])
        for row in rows[1:]:
            self.assertTrue(row.endswith("satisfied"), row)

    def test_the_recorded_host_shape_still_passes(self):
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.host_not_satisfying_case(
                measured_figures("#1 SMP PREEMPT_RT", 273_969),
                measured_figures("#1 SMP PREEMPT_DYNAMIC", 267_461, "7.2.4-1-cachyos"),
                FIXTURE_EDGES,
            )
        self.assertEqual(problems, [])
        for row in printed_rows(printed.getvalue()):
            self.assertTrue(row.endswith("kernel-not-realtime"), row)
        self.assertIn("4 verdicts read kernel-not-realtime", printed.getvalue())

    def test_a_host_that_reported_preempt_rt_fails_the_case_it_used_to_pass(self):
        """The literal `False` made this case unable to notice its own premise."""
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.host_not_satisfying_case(
                measured_figures("#1 SMP PREEMPT_RT", 273_969),
                measured_figures("#1 SMP PREEMPT_RT", 267_461, "7.2.4-1-cachyos"),
                FIXTURE_EDGES,
            )
        self.assertTrue(any("which is PREEMPT_RT" in line for line in problems))
        self.assertIn("FAIL latency/host-not-satisfying", printed.getvalue())

    def test_a_host_measurement_with_no_version_line_is_refused(self):
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.host_not_satisfying_case(
                measured_figures("#1 SMP PREEMPT_RT", 273_969),
                measured_figures("", 267_461, "7.2.4-1-cachyos"),
                FIXTURE_EDGES,
            )
        self.assertTrue(any("no kernel version line" in line for line in problems))

    def test_the_rule_is_still_unflippable_by_the_figure(self):
        """A host reading of zero, the best conceivable figure, is still refused."""
        for worst in (0, EXPECTED_THRESHOLDS["BURST_CRITICAL_NS"] - 1):
            placed = gate.attainment(EXPECTED_THRESHOLDS["BURST_CRITICAL_NS"], worst)
            self.assertEqual(placed, "attained")
            self.assertEqual(gate.verdict(False, placed), "kernel-not-realtime")


class ThresholdTests(unittest.TestCase):
    """The gate reads the edges out of the crates and does not restate them."""

    def test_the_thresholds_are_read_from_the_crates_with_the_recorded_values(self):
        found = dict(gate.thresholds())
        self.assertEqual(found, EXPECTED_THRESHOLDS)

    def test_the_recorded_identities_are_the_two_kernels(self):
        self.assertEqual(gate.recorded_release("AEGIS_M26_GUEST"), "7.2.5-aegis-m26")
        self.assertEqual(gate.recorded_release("REFERENCE_HOST"), "7.2.4-1-cachyos")

    def test_a_missing_constant_is_a_gate_error_rather_than_a_default(self):
        with self.assertRaises(gate.GateError):
            gate.recorded_release("NO_SUCH_IDENTITY")

    def test_every_recorded_figure_names_one_of_the_two_identities(self):
        text = MEASURED.read_text()
        for constant, identity in (
            ("GUEST_WORST_WAKEUP_NS", "AEGIS_M26_GUEST"),
            ("HOST_WORST_WAKEUP_NS", "REFERENCE_HOST"),
        ):
            pattern = (
                rf"pub const {constant}: Measured<u64> =\s*Measured::new\([0-9_]+, {identity},"
            )
            self.assertRegex(text, pattern, f"{constant} does not name {identity}")


class ArgumentTests(unittest.TestCase):
    """One argument vector, and the two arguments that are load-bearing."""

    def test_the_fixture_changes_neither_machine(self):
        self.assertIn("--default-system", gate.CYCLICTEST_ARGUMENTS)

    def test_the_measuring_thread_runs_at_the_priority_p08_requires(self):
        self.assertIn("--priority=95", gate.CYCLICTEST_ARGUMENTS)

    def test_the_guest_script_carries_the_same_vector(self):
        script = gate.measurement_script()
        for argument in gate.CYCLICTEST_ARGUMENTS:
            self.assertIn(argument, script)
        self.assertIn(gate.GUEST_JSON, script)

    def test_the_comparison_drops_the_program_and_the_output_path(self):
        guest = "/bin/cyclictest --nsecs --json=/aegis-latency.json"
        host = "/usr/bin/cyclictest --nsecs --json=/tmp/host.json"
        self.assertEqual(
            gate.comparable_arguments(guest),
            gate.comparable_arguments(host),
        )

    def test_a_differing_vector_is_not_comparable(self):
        guest = "/bin/cyclictest --nsecs --loops=50000"
        host = "/usr/bin/cyclictest --nsecs --loops=10"
        self.assertNotEqual(gate.comparable_arguments(guest), gate.comparable_arguments(host))


class GuestScriptTests(unittest.TestCase):
    """The guest must prove the current run produced its report."""

    def test_the_guest_reports_the_nonce_it_was_handed(self):
        text = GUEST_INIT.read_text()
        self.assertIn(f"MARK='{gate.GUEST_MARK}'", text)
        self.assertIn("aegis\\.nonce=", text)
        self.assertIn("${MARK}-NONCE", text)

    def test_the_guest_runs_the_shared_probe_and_reports_its_status(self):
        text = GUEST_INIT.read_text()
        self.assertIn(PROBE.name, text)
        self.assertIn("${MARK}-PROBE-STATUS", text)
        self.assertIn("${MARK}-MEASURE-STATUS", text)

    def test_the_probe_reads_the_running_kernel_and_nothing_else(self):
        text = PROBE.read_text()
        self.assertIn("/proc/config.gz", text)
        self.assertIn("CONFIG_PREEMPT_RT", text)
        for forbidden in ("uname", "/boot", "modprobe", "insmod"):
            self.assertNotIn(forbidden, text, f"the probe reads {forbidden}")

    def test_a_report_from_another_run_is_refused(self):
        stale = (
            f"{gate.GUEST_MARK}-BEGIN\n"
            f"{gate.GUEST_MARK}-NONCE aegis-1-deadbeef\n"
            f"{gate.GUEST_MARK}-UNAME-R 7.2.5-aegis-m26\n"
            f"{gate.GUEST_MARK}-UNAME-V #1 SMP PREEMPT_RT\n"
            f"{gate.GUEST_MARK}-PROBE-BEGIN\n"
            "CONFIG_PREEMPT_RT=y\n"
            f"{gate.GUEST_MARK}-PROBE-END\n"
        )
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.guest_identity_case(stale, "aegis-2-cafe", "7.2.5-aegis-m26")
        self.assertTrue(any("not this run's" in line for line in problems))
        self.assertIn("FAIL latency/guest-preempt-rt", printed.getvalue())

    def test_a_report_from_this_run_with_the_wrong_option_is_refused(self):
        text = (
            f"{gate.GUEST_MARK}-BEGIN\n"
            f"{gate.GUEST_MARK}-NONCE aegis-2-cafe\n"
            f"{gate.GUEST_MARK}-UNAME-R 7.2.5-aegis-m26\n"
            f"{gate.GUEST_MARK}-UNAME-V #1 SMP PREEMPT_DYNAMIC\n"
            f"{gate.GUEST_MARK}-PROBE-BEGIN\n"
            "# CONFIG_PREEMPT_RT is not set\n"
            f"{gate.GUEST_MARK}-PROBE-END\n"
        )
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.guest_identity_case(text, "aegis-2-cafe", "7.2.5-aegis-m26")
        self.assertTrue(any("not CONFIG_PREEMPT_RT=y" in line for line in problems))

    def test_a_report_that_never_reached_a_stage_is_a_gate_error(self):
        with self.assertRaises(gate.GateError):
            gate.guest_section(f"{gate.GUEST_MARK}-BEGIN\n", "JSON")
        with self.assertRaises(gate.GateError):
            gate.guest_field(f"{gate.GUEST_MARK}-BEGIN\n", "UNAME-R")


class ParsingTests(unittest.TestCase):
    """What the gate accepts as a measurement, and what it refuses."""

    PAYLOAD = (
        '{"cmdline:": "/bin/cyclictest --nsecs --json=/a.json", "resolution_in_ns": 1,'
        ' "sysinfo": {"release": "7.2.5-aegis-m26", "version": "#1 SMP PREEMPT_RT",'
        ' "nodename": "a-machine-name"},'
        ' "thread": {"0": {"cycles": 50000, "min": 1510, "avg": 9664.0, "max": 273969}}}'
    )

    def test_a_nanosecond_payload_parses(self):
        found = gate.parse_report(self.PAYLOAD)
        self.assertEqual(found["max"], 273_969)
        self.assertEqual(found["release"], "7.2.5-aegis-m26")

    def test_the_machine_name_never_leaves_the_payload(self):
        found = gate.parse_report(self.PAYLOAD)
        self.assertNotIn("nodename", found)
        self.assertNotIn("a-machine-name", str(found))

    def test_a_microsecond_payload_is_refused(self):
        with self.assertRaises(gate.GateError):
            gate.parse_report(
                self.PAYLOAD.replace('"resolution_in_ns": 1', '"resolution_in_ns": 0')
            )

    def test_a_payload_without_a_thread_is_refused(self):
        with self.assertRaises(gate.GateError):
            gate.parse_report('{"resolution_in_ns": 1, "sysinfo": {"release": "x"}}')

    def test_text_that_is_not_json_is_refused(self):
        with self.assertRaises(gate.GateError):
            gate.parse_report("the guest printed nothing useful")


class HostSurfaceTests(unittest.TestCase):
    """D57, as a property of what the gate can do rather than of what it says."""

    def test_the_gate_invokes_only_allowed_programs(self):
        resolved, _undecided = call_sites(GATE)
        self.assertTrue(resolved, "the sweep found no call site; it is reading nothing")
        self.assertLessEqual(resolved, ALLOWED_PROGRAMS, "the gate invokes an unlisted program")

    def test_the_two_undecidable_call_sites_are_the_ones_that_have_to_be(self):
        """The sweep reports what it cannot name, and both are resolved here.

        Two call sites run an argument vector they receive as a parameter, so no
        syntax tree can name their program, and both are named rather than
        dropped. `run` is the gate's own deadline wrapper, which starts whatever
        its caller hands it -- so every real call site is one of the decidable
        ones that go through it. `read_version` runs one row of `TOOLCHAIN`, and
        what names those is the table itself.
        """
        _resolved, undecided = call_sites(GATE)
        self.assertEqual(sorted(undecided), ["read_version", "run"])
        rows = {row[1][0] for row in gate.TOOLCHAIN}
        self.assertLessEqual(rows, ALLOWED_PROGRAMS, "a toolchain row runs an unlisted program")

    def test_the_allowlist_has_no_entry_nothing_runs(self):
        resolved, _undecided = call_sites(GATE)
        rows = {row[1][0] for row in gate.TOOLCHAIN}
        self.assertEqual(resolved | rows, ALLOWED_PROGRAMS)

    def test_the_sweep_would_notice_an_unlisted_program(self):
        planted = ast.parse("def f():\n    argv = ['bootctl', 'install']\n    run(argv, 10)\n")
        function = planted.body[0]
        self.assertEqual(bound_heads(function), {"argv": {"bootctl"}})
        self.assertFalse({"bootctl"} <= ALLOWED_PROGRAMS)

    def test_the_sweep_reports_a_call_it_cannot_name_instead_of_dropping_it(self):
        planted = ast.parse("def f(vector):\n    run(vector, 10)\n")
        function = planted.body[0]
        self.assertEqual(bound_heads(function), {})

    def test_the_allowlist_holds_no_installer_or_bootloader_tool(self):
        for forbidden in ("pacman -S", "bootctl", "grub-mkconfig", "modprobe", "insmod", "dd"):
            self.assertNotIn(forbidden, ALLOWED_PROGRAMS)
        source = GATE.read_text()
        for forbidden in ("bootctl", "modprobe", "insmod", "-S ", "--sysupgrade"):
            self.assertNotIn(forbidden, source, f"the gate names {forbidden}")

    def test_every_external_call_site_carries_a_deadline(self):
        total, deadlined = deadlined_call_sites(GATE)
        self.assertGreater(total, 0, "the sweep found no call site; it is reading nothing")
        self.assertEqual(total, deadlined, "an external command runs without a deadline")

    def test_the_sweep_would_notice_a_call_without_one(self):
        planted = ast.parse("subprocess.run(['ls'])\n")
        calls = [node for node in ast.walk(planted) if isinstance(node, ast.Call)]
        self.assertEqual(len(calls), 1)
        self.assertEqual({keyword.arg for keyword in calls[0].keywords}, set())


if __name__ == "__main__":
    unittest.main()
