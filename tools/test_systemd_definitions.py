"""Regressions for the systemd definition gate: version floor, retargeting, outcomes."""

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import verify_systemd_definitions as gate
from testkit import setup_temp_directory

REVIEWED = """[Transfer]
ProtectVersion=%A

[Target]
Type=partition
Path=auto
InstancesMax=2
"""


class VersionTests(unittest.TestCase):
    def test_reference_profile_banner_parses(self):
        """Positive: the exact string the reference profile's systemctl prints."""
        self.assertEqual(gate.parse_systemd_version(gate.REFERENCE_PROFILE_SYSTEMD), 261)

    def test_non_systemd_banner_is_rejected(self):
        """Negative: anything that is not a systemd banner yields no version."""
        for text in ("", "not systemd at all", "systemd261", "  \n", "sysupdate 261"):
            self.assertIsNone(gate.parse_systemd_version(text))

    def test_bare_and_wide_versions_are_read(self):
        """Boundary: a banner with no parenthesis, and the floor itself."""
        self.assertEqual(gate.parse_systemd_version("systemd 261"), gate.SYSTEMD_FLOOR)
        self.assertEqual(gate.parse_systemd_version("systemd 255 (255.4-1ubuntu8)"), 255)
        self.assertEqual(gate.parse_systemd_version("  systemd 12345 (x)  "), 12345)

    def test_the_floor_is_the_recorded_reference_profile(self):
        """Boundary: the floor constant and the recorded banner cannot drift apart."""
        self.assertEqual(
            gate.parse_systemd_version(gate.REFERENCE_PROFILE_SYSTEMD), gate.SYSTEMD_FLOOR
        )


class RetargetTests(unittest.TestCase):
    def test_the_single_auto_line_is_rewritten(self):
        """Positive: only the Path=auto line changes; every other line survives."""
        out = gate.retarget_transfer(REVIEWED, "/scratch/image.raw")
        self.assertIn("Path=/scratch/image.raw", out)
        self.assertNotIn("Path=auto", out)
        self.assertIn("ProtectVersion=%A", out)
        self.assertIn("InstancesMax=2", out)

    def test_a_transfer_without_an_auto_target_is_refused(self):
        """Negative: a silent no-op would point the gate at the host root device."""
        with self.assertRaises(gate.GateError):
            gate.retarget_transfer("[Target]\nType=partition\nPath=/dev/sda\n", "/scratch.raw")

    def test_two_auto_targets_are_refused(self):
        """Boundary: exactly one line qualifies; two is ambiguous, not a choice."""
        with self.assertRaises(gate.GateError):
            gate.retarget_transfer("Path=auto\nPath=auto\n", "/scratch.raw")

    def test_surrounding_whitespace_still_matches(self):
        """Boundary: an indented drop-in line is the same declaration."""
        out = gate.retarget_transfer("[Target]\n  Path=auto  \n", "/scratch.raw")
        self.assertIn("Path=/scratch.raw", out)


class OutcomeTests(unittest.TestCase):
    def test_a_matching_run_is_accepted(self):
        """Positive: the recorded exit code plus the recorded diagnostic."""
        accepted = gate.outcome(1, requires=["Type= not defined, refusing."])
        self.assertTrue(gate.matches(1, "x.conf:1: Type= not defined, refusing.", accepted))

    def test_a_different_exit_code_is_rejected(self):
        """Negative: the same diagnostic under another exit code is not the case."""
        accepted = gate.outcome(1, requires=["Type= not defined, refusing."])
        self.assertFalse(gate.matches(0, "Type= not defined, refusing.", accepted))

    def test_a_forbidden_diagnostic_fails_an_otherwise_clean_run(self):
        """Boundary: exit 0 with an ignored key is the REQ-CI-01 case and must fail."""
        accepted = gate.outcome(0, forbids=[gate.UNKNOWN_KEY])
        self.assertTrue(gate.matches(0, "nothing to report", accepted))
        self.assertFalse(
            gate.matches(0, "00-esp.conf:4: Unknown key 'Subsystem' in section", accepted)
        )

    def test_an_outcome_renders_its_constraints(self):
        """Positive: a failure message names what was expected."""
        rendered = gate.explain(gate.outcome(1, requires=["a"], forbids=["b"]))
        self.assertIn("exit 1", rendered)
        self.assertIn("with 'a'", rendered)
        self.assertIn("without 'b'", rendered)


class ListingTests(unittest.TestCase):
    def test_both_slots_are_read_from_the_json_listing(self):
        """Positive: the slots the repart definitions labelled are reported."""
        line = json.dumps({"current": "b", "all": ["b", "a"]})
        self.assertEqual(gate.listed_versions(f"Determining…\n{line}\n"), {"a", "b"})

    def test_a_listing_without_json_reports_nothing(self):
        """Negative: prose output must not be mistaken for a successful listing."""
        self.assertEqual(gate.listed_versions("Determining installed update sets…\n"), set())

    def test_malformed_json_reports_nothing(self):
        """Boundary: a truncated line is no listing, not a crash."""
        self.assertEqual(gate.listed_versions('{"all": ["a"'), set())

    def test_the_positive_case_requires_both_slots(self):
        """Positive, negative and boundary for the listing assertion itself."""
        case = {"name": "sysupdate/positive"}
        complete = json.dumps({"all": ["a", "b"]})
        self.assertEqual(gate.positive_listing(case, complete), [])
        self.assertTrue(gate.positive_listing(case, json.dumps({"all": ["a"]})))
        self.assertEqual(gate.positive_listing({"name": "sysupdate/boundary-single-slot"}, ""), [])

    def test_the_root_tree_case_must_list_both_slots_when_it_passes(self):
        """Negative: an exit-0 --root= run that lists nothing is not a parse proof."""
        self.assertTrue(gate.positive_listing({"name": "sysupdate/root-tree"}, ""))
        complete = json.dumps({"all": ["b", "a"]})
        self.assertEqual(gate.positive_listing({"name": "sysupdate/root-tree"}, complete), [])


class InvocationTests(unittest.TestCase):
    def test_repart_flags_are_the_spellings_the_host_accepts(self):
        """Positive: the flags were read from systemd-repart --help, not from memory."""
        argv = gate.repart_argv(Path("/defs"), Path("/img.raw"), Path("/root"))
        self.assertEqual(argv[0], "systemd-repart")
        for flag in ("--empty=create", "--dry-run=yes", "--offline=yes", "--definitions=/defs"):
            self.assertIn(flag, argv)
        self.assertEqual(argv[-1], "/img.raw")
        self.assertFalse(any(flag.startswith("--defer-partitions") for flag in argv))
        first_boot_argv = gate.repart_argv(
            Path("/defs"), Path("/img.raw"), Path("/root"), defer=gate.DEFER_TYPES
        )
        self.assertIn(f"--defer-partitions={gate.DEFER_TYPES}", first_boot_argv)

    def test_the_transfer_invocation_selects_one_source_of_definitions(self):
        """Negative: --root and --definitions are alternatives, never both."""
        by_dir = gate.sysupdate_argv(definitions=Path("/defs"))
        by_root = gate.sysupdate_argv(root=Path("/tree"))
        self.assertIn("--definitions=/defs", by_dir)
        self.assertNotIn("--root=/defs", by_dir)
        self.assertIn("--root=/tree", by_root)
        self.assertFalse([flag for flag in by_root if flag.startswith("--definitions=")])

    def test_the_resolved_binary_is_the_one_invoked(self):
        """Positive: a libexec copy found by the guard is argv[0], not the bare name."""
        libexec = f"{gate.SYSTEMD_LIBEXEC}/systemd-sysupdate"
        self.assertEqual(gate.sysupdate_argv(definitions=Path("/d"), binary=libexec)[0], libexec)
        repart = gate.repart_argv(Path("/d"), Path("/i"), Path("/r"), "/opt/bin/systemd-repart")
        self.assertEqual(repart[0], "/opt/bin/systemd-repart")

    def test_both_invocations_stay_offline_and_machine_readable(self):
        """Boundary: the gate never reaches the network and never parses prose."""
        for argv in (gate.sysupdate_argv(definitions=Path("/d")), gate.sysupdate_argv(root="/t")):
            self.assertIn("--offline", argv)
            self.assertIn("--json=short", argv)
            self.assertEqual(argv[-1], "list")


class StagingTests(unittest.TestCase):
    def setUp(self):
        setup_temp_directory(self)

    def source(self, name, text):
        directory = self.base / "src"
        directory.mkdir(exist_ok=True)
        (directory / name).write_text(text)
        return directory

    def test_a_staged_transfer_is_retargeted(self):
        """Positive: the staged copy points at the scratch image, the source does not."""
        source = self.source("10-root.transfer", REVIEWED)
        _, staged = gate.stage_transfers(self.base, "reviewed", source, Path("/img.raw"))
        self.assertIn("Path=/img.raw", (staged / "10-root.transfer").read_text())
        self.assertIn("Path=auto", (source / "10-root.transfer").read_text())

    def test_a_transfer_without_auto_is_copied_verbatim(self):
        """Negative: a fixture that names no device must not be rewritten."""
        body = "[Target]\nType=partition\nPartitions=root-a:root-b\n"
        source = self.source("10-root.transfer", body)
        _, staged = gate.stage_transfers(self.base, "fixture", source, Path("/img.raw"))
        self.assertEqual((staged / "10-root.transfer").read_text(), body)

    def test_only_transfer_files_are_staged(self):
        """Boundary: systemd 257+ reads *.transfer only, so a .conf is not staged."""
        source = self.source("10-root.transfer", REVIEWED)
        (source / "10-root.conf").write_text(REVIEWED)
        _, staged = gate.stage_transfers(self.base, "suffix", source, Path("/img.raw"))
        self.assertEqual([p.name for p in staged.iterdir()], ["10-root.transfer"])


def fake_which(on_path=None, in_libexec=None):
    """Return a shutil.which stand-in: `on_path` answers a PATH lookup and
    `in_libexec` a lookup confined to the systemd libexec directory."""
    on_path = on_path or {}
    in_libexec = in_libexec or {}

    def which(name, mode=None, path=None):
        if path is None:
            return on_path.get(name)
        return in_libexec.get(name) if path == gate.SYSTEMD_LIBEXEC else None

    return which


class ResolutionTests(unittest.TestCase):
    """PATH first, then the systemd libexec directory, and say which was used."""

    SYSUPDATE = f"{gate.SYSTEMD_LIBEXEC}/systemd-sysupdate"

    def test_a_tool_only_in_libexec_is_found_there(self):
        """Positive: systemd 262 installs systemd-sysupdate in libexec, not on PATH."""
        which = fake_which(in_libexec={"systemd-sysupdate": self.SYSUPDATE})
        with patch.object(gate.shutil, "which", side_effect=which):
            path, where = gate.resolve_tool("systemd-sysupdate")
        self.assertEqual(path, self.SYSUPDATE)
        self.assertIn(gate.SYSTEMD_LIBEXEC, where)
        self.assertIn("not PATH", where)

    def test_a_tool_absent_everywhere_is_a_reason_not_a_path(self):
        """Negative: the reason names both places searched."""
        with patch.object(gate.shutil, "which", side_effect=fake_which()):
            path, reason = gate.resolve_tool("systemd-sysupdate")
        self.assertIsNone(path)
        self.assertIn("systemd-sysupdate", reason)
        self.assertIn("PATH", reason)
        self.assertIn(gate.SYSTEMD_LIBEXEC, reason)

    def test_a_copy_on_path_wins_over_the_libexec_one(self):
        """Boundary: both exist; the PATH copy runs and libexec is never consulted."""
        mine = "/opt/systemd/bin/systemd-sysupdate"
        which = fake_which({"systemd-sysupdate": mine}, {"systemd-sysupdate": self.SYSUPDATE})
        with patch.object(gate.shutil, "which", side_effect=which) as probe:
            path, where = gate.resolve_tool("systemd-sysupdate")
        self.assertEqual((path, where), (mine, "PATH"))
        self.assertEqual(probe.call_count, 1)

    def test_every_gate_tool_is_resolved_and_reported(self):
        """Positive: the gate prints where each tool it runs came from."""
        which = fake_which(
            {"systemd-repart": "/usr/bin/systemd-repart"},
            {"systemd-sysupdate": self.SYSUPDATE},
        )
        out = io.StringIO()
        with patch.object(gate.shutil, "which", side_effect=which):
            with contextlib.redirect_stdout(out):
                tools, missing = gate.resolve_tools()
        self.assertEqual(missing, [])
        self.assertEqual(tools["systemd-sysupdate"], self.SYSUPDATE)
        self.assertIn(f"systemd-sysupdate: {self.SYSUPDATE}", out.getvalue())
        self.assertIn("systemd-repart: /usr/bin/systemd-repart (from PATH)", out.getvalue())


def fake_run(banners):
    """Return a gate.run stand-in answering `<binary> --version` from `banners`.

    A binary missing from `banners` exits 1 with no output, as an unrunnable or
    non-systemd tool would.
    """

    def run(argv, extra_env=None):
        banner = banners.get(argv[0])
        if banner is None:
            return 1, "", ""
        return 0, f"{banner}\n+PAM +AUDIT\n", ""

    return run


class ToolVersionTests(unittest.TestCase):
    """Each tool's own --version decides whether it may run."""

    def test_a_systemd_tool_banner_yields_its_major(self):
        """Positive: the first line the reference profile's tools print."""
        run = fake_run({"/usr/bin/systemd-repart": "systemd 262 (262-1-arch)"})
        with patch.object(gate, "run", side_effect=run):
            self.assertEqual(
                gate.tool_version("/usr/bin/systemd-repart"), (262, "systemd 262 (262-1-arch)")
            )

    def test_an_unreadable_tool_is_a_reason_not_a_version(self):
        """Negative: a non-zero exit, and a binary that cannot be run, both give a reason."""
        with patch.object(gate, "run", side_effect=fake_run({})):
            version, reason = gate.tool_version("/x/systemd-sysupdate")
        self.assertIsNone(version)
        self.assertIn("/x/systemd-sysupdate --version exited 1", reason)
        with patch.object(gate, "run", side_effect=gate.GateError("no such file")):
            self.assertEqual(gate.tool_version("/x/y"), (None, "no such file"))

    def test_the_floor_itself_is_admitted_and_one_below_is_not(self):
        """Boundary: 261 runs, 260 is refused with the tool, its path and banner named."""
        at = gate.REFERENCE_PROFILE_SYSTEMD
        with patch.object(gate, "run", side_effect=fake_run({"/a": at})):
            self.assertEqual(gate.floor_refusal("systemd-repart", "/a"), (at, None))
        with patch.object(gate, "run", side_effect=fake_run({"/b": "systemd 260"})):
            banner, why = gate.floor_refusal("systemd-sysupdate", "/b")
        self.assertIsNone(banner)
        self.assertIn("systemd-sysupdate at /b reports 'systemd 260'", why)
        self.assertIn(f"floor systemd {gate.SYSTEMD_FLOOR}", why)


class GuardTests(unittest.TestCase):
    """The guard prints its reason; the tests capture it so the suite stays quiet."""

    REPART = "/usr/bin/systemd-repart"
    SYSUPDATE = f"{gate.SYSTEMD_LIBEXEC}/systemd-sysupdate"

    def guard(self, banners, which=None, out=None):
        """Run the guard against fake tools reporting `banners`, stdout captured."""
        which = which or fake_which(
            {"systemd-repart": self.REPART}, {"systemd-sysupdate": self.SYSUPDATE}
        )
        # A run that requires this gate prints FAIL instead of SKIP; these cases
        # read the unrequired wording, whatever the surrounding run requires.
        with patch.dict(gate.os.environ, {"AEGIS_REQUIRED_GATES": ""}):
            with patch.object(gate.shutil, "which", side_effect=which):
                with patch.object(gate, "run", side_effect=fake_run(banners)):
                    with contextlib.redirect_stdout(out or io.StringIO()):
                        return gate.guard()

    def test_a_host_at_the_floor_is_admitted(self):
        """Positive: tools at the recorded reference profile run the gate."""
        at = gate.REFERENCE_PROFILE_SYSTEMD
        banners, tools = self.guard({self.REPART: at, self.SYSUPDATE: at})
        self.assertEqual(banners, {name: at for name in gate.GATE_TOOLS})
        self.assertEqual(tools["systemd-repart"], self.REPART)

    def test_a_host_without_systemd_says_so(self):
        """Negative: an absent tool is an explicit skip, never a quiet pass."""
        self.assertIsNone(self.guard({}, which=fake_which()))

    def test_the_skip_names_where_the_tool_was_looked_for(self):
        """Negative: the printed reason names PATH and the libexec directory."""
        out = io.StringIO()
        which = fake_which({"systemd-repart": self.REPART})
        self.assertIsNone(self.guard({self.REPART: "systemd 262"}, which=which, out=out))
        text = out.getvalue()
        self.assertIn("SKIP: systemd-sysupdate is neither on PATH nor in", text)
        self.assertIn(gate.SYSTEMD_LIBEXEC, text)
        self.assertIn("did not run", text)

    def test_a_host_with_sysupdate_only_in_libexec_is_admitted(self):
        """Positive: the systemd 262 layout runs the gate with the libexec copy."""
        at = "systemd 262 (262-1-arch)"
        banners, tools = self.guard({self.REPART: at, self.SYSUPDATE: at})
        self.assertEqual(banners["systemd-sysupdate"], at)
        self.assertEqual(tools["systemd-sysupdate"], self.SYSUPDATE)

    def test_a_libexec_copy_below_the_floor_still_skips(self):
        """Boundary: finding the tool does not bypass the floor (ubuntu-24.04, 255)."""
        old = "systemd 255 (255.4-1ubuntu8.17)"
        out = io.StringIO()
        self.assertIsNone(self.guard({self.REPART: old, self.SYSUPDATE: old}, out=out))
        self.assertIn(f"systemd-sysupdate at {self.SYSUPDATE} reports {old!r}", out.getvalue())

    def test_an_old_path_copy_skips_although_systemctl_is_newer(self):
        """Boundary: a PATH copy reporting 255 is refused next to a systemctl reporting 262."""
        mine = "/opt/old/systemd-sysupdate"
        which = fake_which(
            {"systemd-repart": self.REPART, "systemd-sysupdate": mine, "systemctl": "/usr/bin/x"},
            {"systemd-sysupdate": self.SYSUPDATE},
        )
        new, old = "systemd 262 (262-1-arch)", "systemd 255 (255.4-1ubuntu8.17)"
        banners = {"systemctl": new, self.REPART: new, self.SYSUPDATE: new, mine: old}
        out = io.StringIO()
        self.assertIsNone(self.guard(banners, which=which, out=out))
        self.assertIn(f"systemd-sysupdate at {mine} reports {old!r}", out.getvalue())
        self.assertNotIn(f"at {self.REPART}", out.getvalue())

    def test_a_host_below_the_floor_is_not_admitted(self):
        """Boundary: one version below the floor skips, and so does an unreadable banner."""
        self.assertIsNone(self.guard({self.REPART: "systemd 260", self.SYSUPDATE: "systemd 261"}))
        self.assertIsNone(self.guard({self.REPART: "systemd 261"}))

    def test_the_banner_names_each_tool_that_runs(self):
        """Positive: the run line carries the tools' banners, not systemctl's."""
        text = gate.describe({"systemd-repart": "systemd 261", "systemd-sysupdate": "systemd 262"})
        self.assertEqual(text, "systemd-repart 'systemd 261' and systemd-sysupdate 'systemd 262'")


class CrossCheckTests(unittest.TestCase):
    """systemctl is read for the record only and never decides admission."""

    def test_the_systemctl_banner_is_read(self):
        """Positive: the first line systemctl prints."""
        with patch.object(gate.shutil, "which", return_value="/usr/bin/systemctl"):
            with patch.object(gate, "run", side_effect=fake_run({"systemctl": "systemd 262"})):
                self.assertEqual(gate.systemctl_banner(), "systemd 262")

    def test_no_systemctl_is_an_empty_banner(self):
        """Negative: absent, failing or unrunnable systemctl yields '' rather than raising."""
        with patch.object(gate.shutil, "which", return_value=None):
            self.assertEqual(gate.systemctl_banner(), "")
        with patch.object(gate.shutil, "which", return_value="/usr/bin/systemctl"):
            with patch.object(gate, "run", side_effect=fake_run({})):
                self.assertEqual(gate.systemctl_banner(), "")
            with patch.object(gate, "run", side_effect=gate.GateError("timeout")):
                self.assertEqual(gate.systemctl_banner(), "")


class RootTreeTests(unittest.TestCase):
    """The --root= case reads a self-contained tree, and judges only what it may."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.base = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)
        self.identity = self.base / "os-release"
        self.identity.write_text("ID=aegis\nIMAGE_VERSION=a\n")

    def source(self, text):
        directory = self.base / "src"
        directory.mkdir(exist_ok=True)
        (directory / "10-root.transfer").write_text(text)
        return directory

    def test_the_tree_carries_its_transfer_image_path_and_identity(self):
        """Positive: Path= names the image as the tree itself does, beside os-release."""
        tree = self.base / gate.SYSROOT
        gate.stage_root_tree(tree, self.source(REVIEWED), gate.IMAGE_NAME, self.identity)
        staged = (tree / "usr" / "lib" / "sysupdate.d" / "10-root.transfer").read_text()
        self.assertIn(f"Path=/{gate.IMAGE_NAME}", staged)
        self.assertNotIn(str(self.base), staged)
        self.assertEqual(
            (tree / "usr" / "lib" / "os-release").read_text(), self.identity.read_text()
        )

    def test_the_case_does_not_pass_the_host_identity_override(self):
        """Negative: SYSTEMD_OS_RELEASE under --root= left %A unexpanded on 262."""
        image = self.base / gate.SYSROOT / gate.IMAGE_NAME
        case = gate.root_tree_case(image, self.identity, "/usr/lib/systemd/systemd-sysupdate")
        self.assertIsNone(case["env"])
        self.assertEqual(case["argv"][0], "/usr/lib/systemd/systemd-sysupdate")
        # The argv carries the Linux spelling on every host (HISS-21).
        self.assertIn(f"--root={gate.posix_target(image.parent)}", case["argv"])

    def test_both_recorded_outcomes_pass_and_a_third_fails(self):
        """Boundary: 261's refusal and 262's listing pass; 262 on an outside image fails."""
        image = self.base / gate.SYSROOT / gate.IMAGE_NAME
        accepted = gate.root_tree_case(image, self.identity)["accepted"]

        def passes(code, text):
            return any(gate.matches(code, text, row) for row in accepted)

        self.assertTrue(passes(1, "10-root.transfer:1: Source Type= must be one of url-file"))
        self.assertTrue(passes(0, '{"current":"b","all":["b","a"]}'))
        self.assertFalse(passes(1, "Failed to resolve '/x/positive.raw': No such file"))
        self.assertFalse(
            passes(0, "10-root.transfer:48: Failed to expand specifiers in ProtectVersion=")
        )

    def test_a_refusal_is_not_also_asked_for_a_listing(self):
        """Boundary: the recorded exit-1 refusal lists nothing and still passes."""
        case = {"name": "sysupdate/root-tree", "argv": ["x"], "env": None, "image": None}
        case["accepted"] = [gate.outcome(1, requires=["Source Type= must be one of"])]
        with patch.object(gate, "run", return_value=(1, "", "Source Type= must be one of")):
            passed, code, problems, _ = gate.execute(case)
        self.assertEqual((passed, code, problems), (True, 1, []))
        case["accepted"] = [gate.outcome(0)]
        with patch.object(gate, "run", return_value=(0, "no listing", "")):
            passed, _, problems, _ = gate.execute(case)
        self.assertFalse(passed)
        self.assertTrue(problems)


class ShippedDefinitionTests(unittest.TestCase):
    """The reviewed files themselves, checked without running systemd."""

    def test_every_reviewed_definition_cites_its_source(self):
        """Positive: each file names an export id and a 64-character digest."""
        root = gate.ROOT / "build"
        # Repo-relative identifiers are POSIX by convention (HISS-21): git spells
        # them with "/", and so does every citation in planning/.
        names = sorted(p.relative_to(root).as_posix() for p in root.rglob("*") if p.is_file())
        self.assertIn("repart.d/00-esp.conf", names)
        for relative in names:
            if not relative.endswith((".conf", ".transfer")):
                continue
            text = (root / relative).read_text()
            self.assertRegex(text, r"export-\d{3}", relative)
            self.assertRegex(text, r"sha256\s*\n?#?\s*[0-9a-f]{64}", relative)

    def test_no_reviewed_definition_carries_a_rejected_key(self):
        """Negative: the keys systemd 261 ignores must not come back."""
        for relative, rejected in (
            ("repart.d/00-esp.conf", "Subsystem="),
            ("mkosi.extra/usr/lib/repart.d/20-var.conf", "BtrfsSubvolumes="),
            ("sysupdate.d/10-root.transfer", "Partitions="),
        ):
            body = (gate.ROOT / "build" / relative).read_text()
            declarations = [line for line in body.splitlines() if not line.startswith("#")]
            self.assertFalse([line for line in declarations if line.startswith(rejected)], relative)

    def test_the_definition_set_is_exactly_the_four_build_time_partitions(self):
        """Boundary: the milestone ships four build-time repart drop-ins and one transfer."""
        repart = sorted(p.name for p in (gate.ROOT / "build" / "repart.d").iterdir())
        self.assertEqual(
            repart,
            [
                "00-esp.conf",
                "10-root-a.conf",
                "10-root-b.conf",
                "11-root-verity.conf",
            ],
        )
        transfers = sorted(p.name for p in (gate.ROOT / "build" / "sysupdate.d").iterdir())
        self.assertEqual(transfers, ["10-root.transfer"])

    def test_the_first_boot_repart_set_holds_tpm2_var(self):
        """Positive: the first-boot dir holds exactly 20-var.conf and it carries Encrypt=tpm2."""
        files = sorted(p.name for p in gate.FIRST_BOOT_REPART.iterdir())
        self.assertEqual(files, ["20-var.conf"])
        text = (gate.FIRST_BOOT_REPART / "20-var.conf").read_text()
        self.assertIn("Encrypt=tpm2", text)


class BuildTimeEncryptTests(unittest.TestCase):
    """build_time_encrypt_problems flags TPM2 encryption declarations in build-time repart sets."""

    def setUp(self):
        setup_temp_directory(self)

    def test_real_build_repart_directory_has_no_encrypt_problems(self):
        """Positive: real build/repart.d -> build_time_encrypt_problems == []."""
        problems = gate.build_time_encrypt_problems(gate.ROOT / "build" / "repart.d")
        self.assertEqual(problems, [])

    def test_planted_tpm2_encryption_is_refused(self):
        """Negative planted: tmp dir conf Encrypt=tpm2, Encrypt = TPM2, Encrypt=key-file+tpm2."""
        conf = self.base / "20-var.conf"
        conf.write_text("[Partition]\nType=var\nEncrypt=tpm2\n")
        problems = gate.build_time_encrypt_problems(self.base)
        self.assertEqual(len(problems), 1)
        self.assertIn("20-var.conf", problems[0])
        self.assertIn("tpm2", problems[0])

        conf.write_text("[Partition]\nType=var\nEncrypt = TPM2\n")
        problems = gate.build_time_encrypt_problems(self.base)
        self.assertEqual(len(problems), 1)
        self.assertIn("20-var.conf", problems[0])
        self.assertIn("TPM2", problems[0])

        conf.write_text("[Partition]\nType=var\nEncrypt=key-file+tpm2\n")
        problems = gate.build_time_encrypt_problems(self.base)
        self.assertEqual(len(problems), 1)
        self.assertIn("20-var.conf", problems[0])
        self.assertIn("key-file+tpm2", problems[0])

    def test_boundary_comments_non_tpm_missing_and_invalid_utf8(self):
        """Boundary: comments and non-tpm are allowed; missing or invalid utf-8 fail."""
        conf = self.base / "test.conf"
        for content in (
            "[Partition]\n#Encrypt=tpm2\n",
            "[Partition]\n;Encrypt=tpm2\n",
            "[Partition]\nEncrypt=off\n",
            "[Partition]\nEncrypt=key-file\n",
        ):
            conf.write_text(content)
            self.assertEqual(gate.build_time_encrypt_problems(self.base), [])

        missing = self.base / "does_not_exist"
        problems = gate.build_time_encrypt_problems(missing)
        self.assertTrue(len(problems) >= 1)

        conf.write_bytes(b"[Partition]\n\xff\xfe\n")
        problems = gate.build_time_encrypt_problems(self.base)
        self.assertTrue(len(problems) >= 1)

    def test_the_first_boot_layout_holds_both_sets(self):
        """Positive: the staged layout is the four build-time drop-ins plus 20-var.conf."""
        layout = gate.stage_first_boot_layout(self.base)
        self.assertEqual(
            sorted(p.name for p in layout.iterdir()),
            [
                "00-esp.conf",
                "10-root-a.conf",
                "10-root-b.conf",
                "11-root-verity.conf",
                "20-var.conf",
            ],
        )

    def test_a_drop_in_in_both_sets_is_refused(self):
        """Negative: a name in both sets would silently shadow one; staging refuses it."""
        first_boot = self.base / "first-boot"
        first_boot.mkdir()
        (first_boot / "00-esp.conf").write_text("[Partition]\nType=esp\n", encoding="utf-8")
        with patch.object(gate, "FIRST_BOOT_REPART", first_boot):
            with self.assertRaises(gate.GateError):
                gate.stage_first_boot_layout(self.base / "stage")

    def test_a_missing_first_boot_set_is_refused(self):
        """Boundary: an absent set is a gate error, not a smaller layout."""
        with patch.object(gate, "FIRST_BOOT_REPART", self.base / "absent"):
            with self.assertRaises(gate.GateError):
                gate.stage_first_boot_layout(self.base / "stage")


class RequiredGateTests(unittest.TestCase):
    """A gate named in AEGIS_REQUIRED_GATES fails where it would have skipped."""

    def _main(self, required, which=None, banners=None):
        out = io.StringIO()
        with patch.dict(
            gate.os.environ, {"AEGIS_REQUIRED_GATES": required, "AEGIS_DEFINITIONS_CONTAINER": ""}
        ), patch.object(gate.shutil, "which", side_effect=which or fake_which()), patch.object(
            gate, "run", side_effect=fake_run(banners or {})
        ), patch.object(
            gate.sys, "argv", ["verify_systemd_definitions.py"]
        ), contextlib.redirect_stdout(
            out
        ):
            return gate.main(), out.getvalue()

    def test_a_required_gate_without_its_tools_fails(self):
        """Negative: the planted CI defect, a required gate on a host without systemd."""
        code, printed = self._main("systemd")
        self.assertEqual(code, 1)
        self.assertIn("FAIL", printed)
        self.assertIn("systemd-repart is neither on PATH", printed)
        self.assertNotIn("SKIP", printed)

    def test_a_required_gate_below_the_floor_fails(self):
        """Boundary: tools present one release below the floor still fail when required."""
        which = fake_which({name: f"/usr/bin/{name}" for name in gate.GATE_TOOLS})
        old = {f"/usr/bin/{name}": f"systemd {gate.SYSTEMD_FLOOR - 1}" for name in gate.GATE_TOOLS}
        code, printed = self._main("mkosi, systemd", which, old)
        self.assertEqual(code, 1)
        self.assertIn("below the admitted floor", printed)
        self.assertIn(gate.REFERENCE_PROFILE_SYSTEMD, printed)

    def test_an_unrequired_gate_still_skips_by_name(self):
        """Positive: a development host without systemd tools reports a named skip."""
        code, printed = self._main("")
        self.assertEqual(code, 0)
        self.assertIn("SKIP", printed)
        self.assertIn("the systemd definition gate did not run", printed)

    def test_requiring_another_gate_leaves_this_one_skipping(self):
        """Boundary: only the gates named are required."""
        code, printed = self._main("mkosi")
        self.assertEqual(code, 0)
        self.assertIn("SKIP", printed)


class LayoutCaseTests(unittest.TestCase):
    """The repart gate reads what its images hold, not only how repart exited (M11)."""

    def setUp(self):
        setup_temp_directory(self)

    def test_the_scratch_root_holds_a_pe_uki_and_a_payload(self):
        """Positive: the ESP and root-a copies have something real to copy."""
        root = gate.scratch_root(self.base)
        uki = root / "efi" / "EFI" / "BOOT" / "BOOTX64.EFI"
        self.assertEqual(uki.read_bytes()[:2], b"MZ")
        self.assertGreater((root / "usr" / "bin" / "payload").stat().st_size, 4096)

    def test_the_planted_layout_drops_both_copies_of_run_39090040772(self):
        """Negative: the staged copy lacks the two CopyFiles= lines the fix added."""
        planted = gate.stage_planted_layout(self.base)
        esp = (planted / "00-esp.conf").read_text(encoding="utf-8")
        root = (planted / "10-root-a.conf").read_text(encoding="utf-8")
        self.assertNotIn("\nCopyFiles=/efi:/\n", esp)
        self.assertIn("\nCopyFiles=/boot:/\n", esp)
        self.assertNotIn("\nCopyFiles=/\n", root)
        self.assertEqual(
            sorted(p.name for p in planted.glob("*.conf")),
            sorted(p.name for p in (gate.ROOT / "build" / "repart.d").glob("*.conf")),
        )

    def test_a_definition_without_the_line_cannot_be_planted(self):
        """Boundary: a source that no longer carries the line is a gate error, not a pass."""
        source = self.base / "src"
        source.mkdir()
        (source / "00-esp.conf").write_text("[Partition]\nType=esp\n")
        (source / "10-root-a.conf").write_text("[Partition]\nCopyFiles=/\n")
        with self.assertRaises(gate.GateError):
            gate.stage_planted_layout(self.base, source)

    def test_both_layout_cases_are_run(self):
        """Positive: the committed layout must be clean, the planted one refused."""
        root = gate.scratch_root(self.base)
        cases = {c["name"]: c for c in gate.repart_cases(self.base, root, self.base / "p.raw")}
        self.assertEqual(cases["repart/positive"]["layout"], [])
        self.assertIn(
            "ESP: EFI/BOOT/BOOTX64.EFI is missing",
            cases["repart/layout-planted-empty-esp"]["layout"],
        )
        self.assertIsNone(cases["repart/negative-missing-type"]["layout"])

    def test_layout_problems_compare_the_image_to_its_expectation(self):
        """Positive, negative, boundary: clean wanted, refusal wanted, nothing asked."""
        image = self.base / "x.raw"
        image.write_bytes(b"")
        clean = {"layout": [], "target": image}
        refused = {"layout": ["ESP: EFI/BOOT/BOOTX64.EFI is missing"], "target": image}
        missing = ["ESP: EFI/BOOT/BOOTX64.EFI is missing", "root-a: near-empty EROFS"]
        with patch.object(gate.image_layout, "problems", return_value=[]):
            self.assertEqual(gate.layout_problems(clean), [])
            self.assertTrue(gate.layout_problems(refused))
        with patch.object(gate.image_layout, "problems", return_value=missing):
            found = gate.layout_problems(clean)
            self.assertTrue(any("EFI/BOOT/BOOTX64.EFI is missing" in line for line in found))
            self.assertEqual(gate.layout_problems(refused), [])
        self.assertEqual(gate.layout_problems({"target": image}), [])


if __name__ == "__main__":
    unittest.main()
