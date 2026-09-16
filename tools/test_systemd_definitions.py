"""Regressions for the systemd definition gate: version floor, retargeting, outcomes."""

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import verify_systemd_definitions as gate

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
        self.assertEqual(gate.positive_listing({"name": "sysupdate/root-tree"}, ""), [])


class InvocationTests(unittest.TestCase):
    def test_repart_flags_are_the_spellings_the_host_accepts(self):
        """Positive: the flags were read from systemd-repart --help, not from memory."""
        argv = gate.repart_argv(Path("/defs"), Path("/img.raw"), Path("/root"))
        self.assertEqual(argv[0], "systemd-repart")
        for flag in ("--empty=create", "--dry-run=yes", "--offline=yes", "--definitions=/defs"):
            self.assertIn(flag, argv)
        self.assertEqual(argv[-1], "/img.raw")
        self.assertIn(f"--defer-partitions={gate.DEFER_TYPES}", argv)

    def test_the_transfer_invocation_selects_one_source_of_definitions(self):
        """Negative: --root and --definitions are alternatives, never both."""
        by_dir = gate.sysupdate_argv(definitions=Path("/defs"))
        by_root = gate.sysupdate_argv(root=Path("/tree"))
        self.assertIn("--definitions=/defs", by_dir)
        self.assertNotIn("--root=/defs", by_dir)
        self.assertIn("--root=/tree", by_root)
        self.assertFalse([flag for flag in by_root if flag.startswith("--definitions=")])

    def test_both_invocations_stay_offline_and_machine_readable(self):
        """Boundary: the gate never reaches the network and never parses prose."""
        for argv in (gate.sysupdate_argv(definitions=Path("/d")), gate.sysupdate_argv(root="/t")):
            self.assertIn("--offline", argv)
            self.assertIn("--json=short", argv)
            self.assertEqual(argv[-1], "list")


class StagingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.base = Path(self.temp.name)
        self.addCleanup(self.temp.cleanup)

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


class GuardTests(unittest.TestCase):
    """The guard prints its reason; the tests capture it so the suite stays quiet."""

    def guard(self):
        """Run the guard with stdout captured and return its verdict."""
        with contextlib.redirect_stdout(io.StringIO()):
            return gate.guard()

    def test_a_host_at_the_floor_is_admitted(self):
        """Positive: the reference profile runs the gate."""
        with patch.object(gate.shutil, "which", return_value="/usr/bin/x"):
            with patch.object(
                gate, "host_systemd_version", return_value=(261, gate.REFERENCE_PROFILE_SYSTEMD)
            ):
                self.assertEqual(self.guard()[0], 261)

    def test_a_host_without_systemd_says_so(self):
        """Negative: an absent tool is an explicit skip, never a quiet pass."""
        with patch.object(gate.shutil, "which", return_value=None):
            self.assertIsNone(self.guard())

    def test_a_host_below_the_floor_is_not_admitted(self):
        """Boundary: one version below the floor skips; the floor itself runs."""
        with patch.object(gate.shutil, "which", return_value="/usr/bin/x"):
            with patch.object(gate, "host_systemd_version", return_value=(260, "systemd 260")):
                self.assertIsNone(self.guard())
            with patch.object(gate, "host_systemd_version", return_value=(None, "")):
                self.assertIsNone(self.guard())


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
            ("repart.d/20-var.conf", "BtrfsSubvolumes="),
            ("sysupdate.d/10-root.transfer", "Partitions="),
        ):
            body = (gate.ROOT / "build" / relative).read_text()
            declarations = [line for line in body.splitlines() if not line.startswith("#")]
            self.assertFalse([line for line in declarations if line.startswith(rejected)], relative)

    def test_the_definition_set_is_exactly_the_five_partitions(self):
        """Boundary: the milestone ships five repart drop-ins and one transfer."""
        repart = sorted(p.name for p in (gate.ROOT / "build" / "repart.d").iterdir())
        self.assertEqual(
            repart,
            [
                "00-esp.conf",
                "10-root-a.conf",
                "10-root-b.conf",
                "11-root-verity.conf",
                "20-var.conf",
            ],
        )
        transfers = sorted(p.name for p in (gate.ROOT / "build" / "sysupdate.d").iterdir())
        self.assertEqual(transfers, ["10-root.transfer"])


if __name__ == "__main__":
    unittest.main()
