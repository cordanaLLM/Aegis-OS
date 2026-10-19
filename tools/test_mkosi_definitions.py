"""Regressions for the mkosi definition gate: floor, summary parsing, scratch copies."""

import contextlib
import io
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

from host import target as posix_target

import verify_mkosi_definitions as gate
from test_bpf_objects import recipe_lines, suppresses

ROOT = Path(__file__).resolve().parent.parent
MAKEFILE = ROOT / "Makefile"

# The shape `mkosi summary` prints: every resolved image in turn, the implicit
# default initrd first. Trimmed to the stanzas the gate reads.
SUMMARY = """IMAGE: default-initrd

    OUTPUT:
                      Output Format: cpio
                             Output: initrd.cpio.zst
                           Image ID: aegis-os
                 Repart Directories: none

IMAGE: main

    DISTRIBUTION:
                       Distribution: arch
                            Release: rolling
                       Architecture: x86-64
                           Snapshot: 2026/09/13

    OUTPUT:
                      Output Format: disk
                             Output: aegis-os.raw
                           Image ID: aegis-os
                 Repart Directories: /somewhere/build/mkosi.repart
                                     /somewhere/build/repart.d
                    Split Artifacts: uki
                                     kernel
"""


class VersionTests(unittest.TestCase):
    def test_reference_profile_banner_parses(self):
        """Positive: the exact string the reference profile's mkosi prints."""
        self.assertEqual(gate.parse_mkosi_version(gate.REFERENCE_PROFILE_MKOSI), 27)

    def test_a_banner_that_is_not_mkosi_is_rejected(self):
        """Negative: anything that is not an mkosi banner yields no version."""
        for text in ("", "mkosi27", "systemd 261", "  \n", "not mkosi at all"):
            self.assertIsNone(gate.parse_mkosi_version(text))

    def test_the_floor_is_the_recorded_reference_profile(self):
        """Boundary: the floor constant and the recorded banner cannot drift apart."""
        self.assertEqual(gate.parse_mkosi_version(gate.REFERENCE_PROFILE_MKOSI), gate.MKOSI_FLOOR)
        self.assertIn(str(gate.MKOSI_FLOOR), gate.REFERENCE_PROFILE_MKOSI_PACKAGE)


class SummaryTests(unittest.TestCase):
    def test_the_main_image_output_stanza_is_read(self):
        """Positive: the main image's Output stanza parses into its rows."""
        rows = gate.summary_stanza(SUMMARY, gate.OUTPUT_STANZA)
        self.assertEqual(rows["Output Format"], ["disk"])
        self.assertEqual(rows["Output"], ["aegis-os.raw"])

    def test_a_collection_setting_keeps_every_value(self):
        """Positive: continuation lines belong to the key above them.

        mkosi reads `mkosi.repart/` beside the configuration as well, so the
        reviewed set is not always the only entry. A parser that kept the first
        line only would report the wrong directory.
        """
        rows = gate.summary_stanza(SUMMARY, gate.OUTPUT_STANZA)
        self.assertEqual(
            rows["Repart Directories"],
            ["/somewhere/build/mkosi.repart", "/somewhere/build/repart.d"],
        )
        self.assertEqual(rows["Split Artifacts"], ["uki", "kernel"])

    def test_the_subimage_stanza_is_not_read_as_the_main_one(self):
        """Negative: the default initrd would report cpio and no repart directory."""
        rows = gate.summary_stanza(SUMMARY, gate.OUTPUT_STANZA)
        self.assertNotEqual(rows["Output Format"], ["cpio"])
        self.assertNotEqual(rows["Output"], ["initrd.cpio.zst"])

    def test_a_summary_without_the_main_image_reports_nothing(self):
        """Negative: no main image means no stanza, not an empty pass."""
        subimage_only = SUMMARY.split("IMAGE: main")[0]
        self.assertEqual(gate.summary_stanza(subimage_only, gate.OUTPUT_STANZA), {})
        self.assertEqual(gate.summary_stanza("", gate.OUTPUT_STANZA), {})

    def test_each_stanza_is_read_separately(self):
        """Boundary: two stanzas of one image do not bleed into each other."""
        distribution = gate.summary_stanza(SUMMARY, gate.DISTRIBUTION_STANZA)
        self.assertEqual(distribution["Snapshot"], ["2026/09/13"])
        self.assertNotIn("Output Format", distribution)
        output = gate.summary_stanza(SUMMARY, gate.OUTPUT_STANZA)
        self.assertNotIn("Snapshot", output)


class RepartDirectoryTests(unittest.TestCase):
    @staticmethod
    def _reviewed(base):
        """Create and return the reviewed set inside `base`."""
        reviewed = Path(base) / "build" / gate.REPART_DIRECTORY
        reviewed.mkdir(parents=True)
        (reviewed / "00-esp.conf").write_text("[Partition]\nType=esp\n")
        return reviewed

    @staticmethod
    def _stowaway(base, name):
        """Create a second, unreviewed definition set at `base/name`."""
        stow = Path(base) / name
        stow.mkdir(parents=True)
        (stow / "50-stowaway.conf").write_text("[Partition]\nType=swap\n")
        return stow

    def test_the_reviewed_set_among_others_is_accepted(self):
        """Positive: an empty extra directory beside the reviewed set is tolerated."""
        with tempfile.TemporaryDirectory() as base:
            reviewed = self._reviewed(base)
            extra = Path(base) / "mkosi.repart"
            extra.mkdir()
            listed = [str(extra), str(reviewed)]
            self.assertEqual(gate.check_repart_directories(listed, reviewed), [])

    def test_a_missing_reviewed_set_is_reported(self):
        """Negative: the reviewed definitions must actually be in the list."""
        with tempfile.TemporaryDirectory() as base:
            reviewed = self._reviewed(base)
            for listed in (["/elsewhere/other.d"], []):
                problems = gate.check_repart_directories(listed, reviewed)
                self.assertEqual(len(problems), 1, listed)
                self.assertIn("expected exactly one repart directory", problems[0])

    def test_a_populated_extra_directory_is_reported(self):
        """Boundary: an extra directory holding a definition joins the image silently."""
        with tempfile.TemporaryDirectory() as base:
            reviewed = self._reviewed(base)
            extra = Path(base) / "mkosi.repart"
            extra.mkdir()
            (extra / "99-stowaway.conf").write_text("[Partition]\nType=swap\n")
            listed = [str(extra), str(reviewed)]
            problems = gate.check_repart_directories(listed, reviewed)
            self.assertEqual(len(problems), 1)
            self.assertIn("99-stowaway.conf", problems[0])

    def test_a_second_set_below_the_same_suffix_is_reported(self):
        """Negative: identity is the resolved path, not the tail of the path.

        This is the `mkosi.conf.d/99-stowaway.conf` drop-in that adds a second
        `RepartDirectories=` row pointing at another `build/repart.d`. A suffix
        test exempted it and the gate reported a pass; the definitions it
        carries must be named instead.
        """
        with tempfile.TemporaryDirectory() as base:
            reviewed = self._reviewed(base)
            stow = self._stowaway(base, "stow/build/repart.d")
            # The premise of this case: the stowaway really does share the
            # reviewed tail. Spelled as the gate reports it, so the premise
            # holds on any host.
            self.assertTrue(posix_target(stow).endswith("build/repart.d"))
            problems = gate.check_repart_directories([str(reviewed), str(stow)], reviewed)
            self.assertEqual(len(problems), 1)
            self.assertIn("50-stowaway.conf", problems[0])
            self.assertIn(posix_target(stow), problems[0])

    def test_a_directory_merely_named_repart_d_is_reported(self):
        """Negative: the weaker `/repart.d` tail the floor cases used to assert."""
        with tempfile.TemporaryDirectory() as base:
            reviewed = self._reviewed(base)
            stow = self._stowaway(base, "elsewhere/repart.d")
            problems = gate.check_repart_directories([str(reviewed), str(stow)], reviewed)
            self.assertEqual(len(problems), 1)
            self.assertIn("50-stowaway.conf", problems[0])

    def test_the_reviewed_set_listed_twice_is_reported(self):
        """Negative: the drop-in above makes mkosi resolve the reviewed row twice.

        Exactly one row may be the reviewed set. Two means a second
        `RepartDirectories=` declaration reached the image, which is the thing
        being ruled out even before its contents are read.
        """
        with tempfile.TemporaryDirectory() as base:
            reviewed = self._reviewed(base)
            stow = self._stowaway(base, "stow/build/repart.d")
            listed = [str(reviewed), str(reviewed), str(stow)]
            problems = gate.check_repart_directories(listed, reviewed)
            self.assertEqual(len(problems), 1)
            self.assertIn("found 2", problems[0])

    def test_a_list_past_the_bound_is_reported_not_truncated(self):
        """Boundary: at the bound the list is read; one past it is refused."""
        with tempfile.TemporaryDirectory() as base:
            reviewed = self._reviewed(base)
            stow = self._stowaway(base, "stow/build/repart.d")
            filler = [f"{base}/empty-{index}" for index in range(gate.MAX_REPART_DIRECTORIES - 2)]
            at_bound = [str(reviewed), str(stow), *filler]
            self.assertEqual(len(at_bound), gate.MAX_REPART_DIRECTORIES)
            problems = gate.check_repart_directories(at_bound, reviewed)
            self.assertEqual(len(problems), 1)
            self.assertIn("50-stowaway.conf", problems[0])
            past_bound = [*at_bound, f"{base}/one-too-many"]
            problems = gate.check_repart_directories(past_bound, reviewed)
            self.assertEqual(len(problems), 1)
            self.assertIn("past the bound", problems[0])

    def test_an_unreadable_row_is_not_the_reviewed_set(self):
        """Boundary: a path the resolver refuses is an extra row, not the reviewed one."""
        with tempfile.TemporaryDirectory() as base:
            reviewed = self._reviewed(base)
            self.assertFalse(gate.same_directory("\x00", reviewed.resolve()))
            self.assertTrue(gate.same_directory(str(reviewed), reviewed.resolve()))


# The directory the recorded summary above resolves its reviewed set to.
SUMMARY_REPART = Path("/somewhere/build/repart.d")


class ExpectationTests(unittest.TestCase):
    def test_a_conforming_summary_has_no_problems(self):
        """Positive: the recorded expectations match the recorded summary."""
        self.assertEqual(gate.check_summary(SUMMARY, SUMMARY_REPART), [])

    def test_a_changed_output_format_is_reported(self):
        """Negative: a format change is drift, not a detail."""
        drifted = SUMMARY.replace("Output Format: disk", "Output Format: tar")
        problems = gate.check_summary(drifted, SUMMARY_REPART)
        self.assertTrue(any("Output Format" in problem for problem in problems))

    def test_an_unpinned_snapshot_is_reported(self):
        """Negative: decision D18 is checked against the resolved configuration."""
        unpinned = SUMMARY.replace("Snapshot: 2026/09/13", "Snapshot: none")
        problems = gate.check_summary(unpinned, SUMMARY_REPART)
        self.assertTrue(any("Snapshot" in problem for problem in problems))

    def test_a_definition_without_the_reviewed_repart_directory_is_reported(self):
        """Boundary: the resolved path is what ties the definition to the M03 files."""
        elsewhere = SUMMARY.replace("/somewhere/build/repart.d", "/elsewhere/other.d")
        problems = gate.check_summary(elsewhere, SUMMARY_REPART)
        self.assertTrue(
            any("expected exactly one repart directory" in problem for problem in problems)
        )
        near_miss = SUMMARY.replace("/somewhere/build/repart.d", "/nearly/build/repart.d")
        problems = gate.check_summary(near_miss, SUMMARY_REPART)
        self.assertTrue(
            any("expected exactly one repart directory" in problem for problem in problems)
        )


class ScratchTests(unittest.TestCase):
    def test_only_the_minimum_version_line_is_rewritten(self):
        """Positive: the scratch copy differs from the reviewed file in one line."""
        with tempfile.TemporaryDirectory() as base:
            target = gate.scratch_definition(Path(base), 99)
            copied = (target / "mkosi.conf").read_text()
            reviewed = (ROOT / "build" / "mkosi.conf").read_text()
            # Declarations only: the reviewed header explains the floor in
            # prose, and a comment naming it is not a second declaration.
            declarations = [line for line in copied.splitlines() if not line.startswith("#")]
            self.assertIn("MinimumVersion=99", declarations)
            self.assertNotIn("MinimumVersion=27", declarations)
            differing = [
                (left, right)
                for left, right in zip(reviewed.splitlines(), copied.splitlines())
                if left != right
            ]
            self.assertEqual(len(differing), 1)
            self.assertTrue((target / "repart.d").is_dir())

    def test_a_definition_without_a_minimum_version_is_refused(self):
        """Negative: a silent no-op would run both floor cases at the same version."""
        with tempfile.TemporaryDirectory() as base:
            target = Path(base) / "empty"
            (target / "repart.d").mkdir(parents=True)
            (target / "mkosi.conf").write_text("[Config]\n")
            with patch.object(gate, "ROOT", Path(base)), patch.object(
                gate, "DEFINITION_DIRECTORY", "empty"
            ):
                with self.assertRaises(gate.GateError):
                    gate.scratch_definition(Path(base) / "out", 99)

    def test_the_reviewed_definition_pins_the_floor_itself(self):
        """Boundary: build/mkosi.conf carries the admitted floor, not a comment about it."""
        reviewed = (ROOT / "build" / "mkosi.conf").read_text()
        declarations = [line for line in reviewed.splitlines() if not line.startswith("#")]
        self.assertIn(f"MinimumVersion={gate.MKOSI_FLOOR}", declarations)


# Each warning mkosi 27.1 prints while it still exits 0, in its own wording
# (mkosi/config.py). Every one means part of the reviewed file was not applied.
IGNORED_LINES = (
    "\u2023 /w/build/mkosi.conf: Unknown setting BogusSetting",
    "\u2023 /w/build/mkosi.conf: Setting Format should be configured in [Output], not [Content].",
    "\u2023 /w/build/mkosi.conf: Setting OldName is deprecated, please use NewName instead.",
    "\u2023 /w/build/mkosi.conf: The '@' specifier is deprecated, please use Output instead of "
    "@Output",
)


class IgnoredDiagnosticTests(unittest.TestCase):
    """E11-3: mkosi exits 0 while ignoring part of the image input; the gate does not."""

    def test_every_ignored_spelling_is_caught(self):
        """Negative: each warning mkosi 27.1 prints on exit 0 is reported, not only one."""
        for line in IGNORED_LINES:
            with self.subTest(line=line):
                self.assertEqual(gate.ignored_diagnostics("noise\n" + line + "\n"), [line])

    def test_clean_output_has_none(self):
        """Positive: a summary run with no warnings reports nothing."""
        self.assertEqual(gate.ignored_diagnostics(SUMMARY), [])
        self.assertEqual(gate.ignored_diagnostics(""), [])

    def _execute(self, case, stderr):
        with patch.object(gate, "run", return_value=(0, SUMMARY, stderr)):
            return gate.execute(case)

    def test_a_passing_case_with_an_ignored_setting_fails(self):
        """Negative: exit 0 plus an unknown-setting warning is a failed case."""
        case = {
            "name": "probe",
            "directory": ROOT / "build",
            "code": 0,
            "requires": (),
            "repart_directory": None,
            "expects_ignored": False,
        }
        _, problems, _ = self._execute(case, IGNORED_LINES[0])
        self.assertEqual(len(problems), 1)
        self.assertIn("BogusSetting", problems[0])
        _, problems, _ = self._execute(case, "")
        self.assertEqual(problems, [])

    def test_the_planted_case_requires_the_warning(self):
        """Boundary: the planted case passes only when mkosi really printed the warning."""
        case = {
            "name": "probe",
            "directory": ROOT / "build",
            "code": 0,
            "requires": (gate.UNKNOWN_SETTING,),
            "repart_directory": None,
            "expects_ignored": True,
        }
        _, problems, _ = self._execute(case, IGNORED_LINES[0])
        self.assertEqual(problems, [])
        _, problems, _ = self._execute(case, "")
        self.assertTrue(problems)

    def test_the_planted_scratch_copy_appends_one_unknown_setting(self):
        """Positive: the planted copy is the reviewed file plus the unknown setting only."""
        with tempfile.TemporaryDirectory() as base:
            target = gate.scratch_ignored_setting(Path(base))
            copied = (target / "mkosi.conf").read_text().splitlines()
            reviewed = (ROOT / "build" / "mkosi.conf").read_text().splitlines()
            self.assertEqual(copied[: len(reviewed)], reviewed)
            added = [line for line in copied[len(reviewed) :] if line.strip()]
            self.assertEqual(added, ["[Output]", f"{gate.PLANTED_SETTING}=1"])

    def test_the_cases_carry_the_planted_ignored_setting(self):
        with tempfile.TemporaryDirectory() as base:
            listed = {case["name"]: case for case in gate.cases(Path(base))}
        planted = listed["mkosi/negative-ignored-setting"]
        self.assertTrue(planted["expects_ignored"])
        self.assertEqual(planted["code"], 0)
        self.assertEqual(
            [name for name, case in listed.items() if case["expects_ignored"]],
            ["mkosi/negative-ignored-setting"],
        )


class SectionTests(unittest.TestCase):
    """mkosi 27.1 skips an unknown [Section] without any diagnostic; the gate reads them."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.base = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)

    def test_the_reviewed_definition_has_only_known_sections(self):
        """Positive: build/ names no section mkosi would drop."""
        self.assertEqual(gate.unknown_sections(ROOT / "build"), [])

    def test_an_unknown_section_is_reported(self):
        """Negative: a section mkosi skips silently, in mkosi.conf and in a drop-in."""
        (self.base / "mkosi.conf").write_text("[Output]\nFormat=disk\n[BogusSection]\nFoo=1\n")
        problems = gate.unknown_sections(self.base)
        self.assertEqual(len(problems), 1)
        self.assertIn("BogusSection", problems[0])
        self.assertIn("mkosi.conf", problems[0])
        (self.base / "mkosi.conf").write_text("[Output]\n")
        (self.base / "mkosi.conf.d").mkdir()
        (self.base / "mkosi.conf.d" / "10-x.conf").write_text("[OutputTypo]\nFormat=disk\n")
        problems = gate.unknown_sections(self.base)
        self.assertEqual(len(problems), 1)
        self.assertIn("10-x.conf", problems[0])

    def test_every_known_section_and_match_header_is_accepted(self):
        """Boundary: each section mkosi parses passes, including [Match] and [TriggerMatch]."""
        body = "".join(f"[{name}]\n" for name in sorted(gate.KNOWN_SECTIONS))
        (self.base / "mkosi.conf").write_text("# [NotASection]\n" + body)
        self.assertEqual(gate.unknown_sections(self.base), [])
        for name in ("Match", "TriggerMatch", "Assert", "TriggerAssert", "Host"):
            self.assertIn(name, gate.KNOWN_SECTIONS)

    def test_a_missing_or_undecodable_file_is_reported(self):
        """Boundary: no mkosi.conf, or bytes that are not UTF-8, is a problem, not a pass."""
        self.assertTrue(gate.unknown_sections(self.base))
        (self.base / "mkosi.conf").write_bytes(b"[Output]\n\xff\xfe\n")
        self.assertTrue(gate.unknown_sections(self.base))

    def test_the_known_sections_match_the_installed_mkosi(self):
        """The allow-list is the set mkosi 27.1 parses, read back from its own source."""
        try:
            import mkosi.config as installed
        except ImportError:
            self.skipTest("mkosi is not importable on this host; the list is pinned to 27.1")
        parsed = {setting.section for setting in installed.SETTINGS} | {"Host"}
        parsed |= {"Match", "TriggerMatch", "Assert", "TriggerAssert"}
        self.assertEqual(set(gate.KNOWN_SECTIONS), parsed)

    def test_main_refuses_an_unknown_section_before_running_mkosi(self):
        """Negative: the static check runs on every host, before tool discovery."""
        build = self.base / "build"
        build.mkdir()
        (build / "mkosi.conf").write_text("[BogusSection]\n")
        stdout = io.StringIO()
        with patch.object(gate, "ROOT", self.base), patch.object(
            gate.definitions_container, "dispatch"
        ) as dispatch, patch.object(
            gate.sys, "argv", ["verify_mkosi_definitions.py"]
        ), contextlib.redirect_stdout(
            stdout
        ):
            code = gate.main()
        self.assertEqual(code, 1)
        self.assertIn("BogusSection", stdout.getvalue())
        dispatch.assert_not_called()


class SuppressionTests(unittest.TestCase):
    """E11-3 boundary: no step on the image-input path can turn a failure into a pass."""

    def test_the_image_input_gates_run_unsuppressed(self):
        makefile = MAKEFILE.read_text()
        for target, script in (
            ("verify-systemd", "python3 tools/verify_systemd_definitions.py"),
            ("verify-mkosi", "python3 tools/verify_mkosi_definitions.py"),
        ):
            with self.subTest(target=target):
                self.assertEqual(recipe_lines(makefile, target), [script])
                wired = [
                    line
                    for line in recipe_lines(makefile, "verify-all")
                    if line.endswith(f" {target}")
                ]
                self.assertEqual(wired, [f"$(MAKE) --no-print-directory {target}"])
                for line in wired + [script]:
                    self.assertFalse(suppresses(line), line)


class GuardTests(unittest.TestCase):
    def _guard(self, which, code, banner):
        # A run that requires this gate prints FAIL instead of SKIP; these cases
        # read the unrequired wording, whatever the surrounding run requires.
        with patch.dict(gate.os.environ, {"AEGIS_REQUIRED_GATES": ""}), patch.object(
            gate.shutil, "which", return_value=which
        ), patch.object(gate, "run", return_value=(code, banner, "")):
            stdout = io.StringIO()
            with contextlib.redirect_stdout(stdout):
                return gate.guard(), stdout.getvalue()

    def test_a_host_at_the_floor_is_admitted(self):
        """Positive: the reference profile runs the gate."""
        banner, printed = self._guard("/usr/bin/mkosi", 0, "mkosi 27\n")
        self.assertEqual(banner, "mkosi 27")
        self.assertEqual(printed, "")

    def test_a_host_without_mkosi_says_so(self):
        """Negative: a missing tool prints why and reports no pass."""
        banner, printed = self._guard(None, 0, "")
        self.assertIsNone(banner)
        self.assertIn("SKIP", printed)

    def test_a_host_below_the_floor_is_not_admitted(self):
        """Boundary: one major below the floor still does not run."""
        banner, printed = self._guard("/usr/bin/mkosi", 0, "mkosi 26\n")
        self.assertIsNone(banner)
        self.assertIn("below the admitted floor", printed)
        self.assertIn(gate.REFERENCE_PROFILE_MKOSI_PACKAGE, printed)


class RequiredGateTests(unittest.TestCase):
    """A gate named in AEGIS_REQUIRED_GATES fails where it would have skipped."""

    def _main(self, required, which=None, banner=""):
        out = io.StringIO()
        with patch.dict(
            gate.os.environ, {"AEGIS_REQUIRED_GATES": required, "AEGIS_DEFINITIONS_CONTAINER": ""}
        ), patch.object(gate.shutil, "which", return_value=which), patch.object(
            gate, "run", return_value=(0, banner, "")
        ), patch.object(
            gate.sys, "argv", ["verify_mkosi_definitions.py"]
        ), contextlib.redirect_stdout(
            out
        ):
            return gate.main(), out.getvalue()

    def test_a_required_gate_without_mkosi_fails(self):
        """Negative: the planted CI defect, a required gate on a host without mkosi."""
        code, printed = self._main("mkosi")
        self.assertEqual(code, 1)
        self.assertIn("FAIL", printed)
        self.assertIn("mkosi not on PATH", printed)
        self.assertNotIn("SKIP", printed)

    def test_a_required_gate_below_the_floor_fails(self):
        """Boundary: mkosi one major below the floor still fails when required."""
        code, printed = self._main(
            "systemd,mkosi", "/usr/bin/mkosi", f"mkosi {gate.MKOSI_FLOOR - 1}\n"
        )
        self.assertEqual(code, 1)
        self.assertIn("below the admitted floor", printed)
        self.assertIn(gate.REFERENCE_PROFILE_MKOSI_PACKAGE, printed)

    def test_an_unrequired_gate_still_skips_by_name(self):
        """Positive: a development host without mkosi reports a named skip."""
        code, printed = self._main("")
        self.assertEqual(code, 0)
        self.assertIn("SKIP", printed)
        self.assertIn("the mkosi definition gate did not run", printed)

    def test_requiring_another_gate_leaves_this_one_skipping(self):
        """Boundary: only the gates named are required."""
        code, printed = self._main("systemd")
        self.assertEqual(code, 0)
        self.assertIn("SKIP", printed)


if __name__ == "__main__":
    unittest.main()
