#!/usr/bin/env python3
"""Coverage for the dogfood finding allow-list export and deny scan.

Exercises positive, negative and boundary cases for dogfood export invariants,
including barrier-1 allow-list rules, barrier-2 deny patterns, and CLI mechanics.
"""

import json
import pathlib
import subprocess
import sys
import tempfile
import unittest
from unittest import mock

try:
    import dogfood_export
except ModuleNotFoundError:
    from tools import dogfood_export


class DogfoodExportTests(unittest.TestCase):
    """Test suite for dogfood_export allow-list validation and deny scan."""

    def setUp(self) -> None:
        self.hostname_mock = mock.patch("socket.gethostname", return_value="testbox.lan")
        self.hostname_mock.start()
        self.addCleanup(self.hostname_mock.stop)

        self.temp_dir = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp_dir.cleanup)
        self.root = pathlib.Path(self.temp_dir.name)

        (self.root / "crates" / "aegis-vulcan-gpu").mkdir(parents=True)
        (self.root / "tools").mkdir(parents=True)
        (self.root / "tools" / "verify_display.py").write_text("# tool\n", encoding="utf-8")
        (self.root / "planning").mkdir(parents=True)
        (self.root / "planning" / "roadmap.json").write_text(
            json.dumps({"milestones": [{"id": "M25"}]}), encoding="utf-8"
        )
        (self.root / "docs" / "roadmap").mkdir(parents=True)
        (self.root / "docs" / "roadmap" / "requirements.md").write_text(
            "Specification contains REQ-P03-01 active\n", encoding="utf-8"
        )

        self.valid_full_record = {
            "schema": "aegis-dogfood-export/1",
            "date": "2026-10-17",
            "module": "aegis-vulcan-gpu",
            "milestone": "M25",
            "verdict": "pass",
            "requirement": "REQ-P03-01",
            "kernel_release": "7.2.10-1-cachyos",
            "driver": "amdgpu",
            "pci_id": "1002:73bf",
            "evidence_sha256": "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855",
            "measurements": {
                "latency_us_p50": 12.5,
                "boot_seconds": 3,
            },
        }

    def test_valid_full_record_exports(self) -> None:
        """Positive: a complete allow-listed record exports without errors."""
        out = dogfood_export.export(self.valid_full_record, self.root, frozenset())
        self.assertEqual(out["schema"], "aegis-dogfood-export/1")
        self.assertEqual(out["module"], "aegis-vulcan-gpu")
        self.assertEqual(out["driver"], "amdgpu")

    def test_output_keys_sorted(self) -> None:
        """Positive: emitted dictionary keys are strictly sorted."""
        out = dogfood_export.export(self.valid_full_record, self.root, frozenset())
        self.assertEqual(list(out.keys()), sorted(out.keys()))
        self.assertEqual(list(out["measurements"].keys()), sorted(out["measurements"].keys()))

    def test_to_json_deterministic_twice(self) -> None:
        """Positive: to_json serializes identically on repeated invocations."""
        out = dogfood_export.export(self.valid_full_record, self.root, frozenset())
        first = dogfood_export.to_json(out)
        second = dogfood_export.to_json(out)
        self.assertEqual(first, second)
        self.assertTrue(first.endswith("\n"))

    def test_minimal_record_ok(self) -> None:
        """Positive: record with only the 5 required keys is accepted."""
        minimal = {
            "schema": "aegis-dogfood-export/1",
            "date": "2026-10-17",
            "module": "verify_display.py",
            "milestone": "M25",
            "verdict": "pass",
        }
        out = dogfood_export.export(minimal, self.root, frozenset())
        self.assertEqual(len(out), 5)
        self.assertEqual(out["module"], "verify_display.py")

    def test_kernel_release_cachyos_ok(self) -> None:
        """Positive: distribution kernel suffix passes allow-list and deny scan."""
        rec = dict(self.valid_full_record, kernel_release="7.2.10-1-cachyos")
        out = dogfood_export.export(rec, self.root, frozenset())
        self.assertEqual(out["kernel_release"], "7.2.10-1-cachyos")

    def test_evidence_sha256_no_mac_false_positive(self) -> None:
        """Positive: 64 hex characters does not trip MAC address detection."""
        rec = dict(
            self.valid_full_record,
            evidence_sha256="0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )
        out = dogfood_export.export(rec, self.root, frozenset())
        self.assertEqual(
            out["evidence_sha256"],
            "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef",
        )

    def test_planted_hostname_extra_key(self) -> None:
        """Negative: planted hostname in extra top-level key is refused."""
        bad = dict(self.valid_full_record, hostalpha="planted")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset({"hostalpha"}))
        self.assertTrue(any("hostalpha" in r for r in ctx.exception.reasons))

    def test_planted_hostname_measurements_key(self) -> None:
        """Negative: planted hostname as measurements key is refused."""
        bad = dict(self.valid_full_record, measurements={"hostalpha": 42.0})
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset({"hostalpha"}))
        self.assertTrue(any("hostalpha" in r for r in ctx.exception.reasons))

    def test_gethostname_as_kernel_suffix(self) -> None:
        """Negative: local hostname in kernel_release trips deny scan and allow-list."""
        bad = dict(self.valid_full_record, kernel_release="7.2-testbox")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("kernel_release" in r or "testbox" in r for r in ctx.exception.reasons))

    def test_planted_ipv4_in_driver(self) -> None:
        """Negative: IPv4 address planted in driver field is refused."""
        bad = dict(self.valid_full_record, driver="192.168.1.20")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("driver" in r or "IPv4" in r for r in ctx.exception.reasons))

    def test_ipv6_refused(self) -> None:
        """Negative: IPv6 address is refused by deny scan and allow-list."""
        bad = dict(self.valid_full_record, driver="fe80::1")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("driver" in r or "IPv6" in r for r in ctx.exception.reasons))

    def test_email_in_module_refused(self) -> None:
        """Negative: email address in module field is refused."""
        bad = dict(self.valid_full_record, module="a@b.example")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("module" in r or "email" in r for r in ctx.exception.reasons))

    def test_ssh_key_in_requirement_refused(self) -> None:
        """Negative: SSH key in requirement field is refused."""
        bad = dict(self.valid_full_record, requirement="ssh-ed25519 AAAAC3Nza")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("requirement" in r or "SSH" in r for r in ctx.exception.reasons))

    def test_private_key_marker_refused(self) -> None:
        """Negative: private key header marker is refused."""
        bad = dict(self.valid_full_record, requirement="-----BEGIN OPENSSH PRIVATE KEY-----")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(
            any("requirement" in r or "private key" in r for r in ctx.exception.reasons)
        )

    def test_token_prefix_refused(self) -> None:
        """Negative: sensitive token prefix is refused."""
        bad = dict(self.valid_full_record, requirement="ghp_" + "x" * 36)
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("requirement" in r or "token" in r for r in ctx.exception.reasons))

    def test_home_path_in_module_refused(self) -> None:
        """Negative: /home/ path in module field is refused."""
        bad = dict(self.valid_full_record, module="/home/someone/x")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("module" in r or "forbidden path" in r for r in ctx.exception.reasons))

    def test_mac_address_in_pci_id_refused(self) -> None:
        """Negative: MAC address in pci_id field is refused."""
        bad = dict(self.valid_full_record, pci_id="aa:bb:cc:dd:ee:ff")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("pci_id" in r or "MAC" in r for r in ctx.exception.reasons))

    def test_unknown_top_level_key_refused(self) -> None:
        """Negative: unrecognized top-level key is refused."""
        bad = dict(self.valid_full_record, unexpected_key="val")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("unexpected_key" in r for r in ctx.exception.reasons))

    def test_missing_verdict_refused(self) -> None:
        """Negative: missing required verdict field is refused."""
        bad = dict(self.valid_full_record)
        del bad["verdict"]
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("verdict" in r for r in ctx.exception.reasons))

    def test_milestone_m99_not_in_roadmap_refused(self) -> None:
        """Negative: milestone not recorded in roadmap.json is refused."""
        bad = dict(self.valid_full_record, milestone="M99")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("milestone" in r for r in ctx.exception.reasons))

    def test_module_not_existing_refused(self) -> None:
        """Negative: non-existent module name is refused."""
        bad = dict(self.valid_full_record, module="verify_nonexistent.py")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("module" in r for r in ctx.exception.reasons))

    def test_bool_measurement_refused(self) -> None:
        """Negative: boolean value in measurements is refused."""
        bad = dict(self.valid_full_record, measurements={"latency_us_p50": True})
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("measurements.latency_us_p50" in r for r in ctx.exception.reasons))

    def test_nan_measurement_refused(self) -> None:
        """Negative: NaN value in measurements is refused."""
        bad = dict(self.valid_full_record, measurements={"latency_us_p50": float("nan")})
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertTrue(any("measurements.latency_us_p50" in r for r in ctx.exception.reasons))

    def test_multiple_faults_all_reasons_collected(self) -> None:
        """Negative: multiple faults collect all reasons rather than failing fast."""
        bad = dict(self.valid_full_record, milestone="M99", driver="invalid_driver")
        del bad["verdict"]
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad, self.root, frozenset())
        self.assertGreaterEqual(len(ctx.exception.reasons), 2)

    def test_barrier_2_independence(self) -> None:
        """Barrier 2: deny scan directly refuses an allow-listed record with denied host."""
        valid = {
            "schema": "aegis-dogfood-export/1",
            "date": "2026-10-17",
            "module": "aegis-vulcan-gpu",
            "milestone": "M25",
            "verdict": "pass",
        }
        out = dogfood_export.export(valid, self.root, frozenset())
        self.assertEqual(out["module"], "aegis-vulcan-gpu")

        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.deny_scan(valid, frozenset({"aegis-vulcan-gpu"}))
        self.assertTrue(any("denied host" in r for r in ctx.exception.reasons))

        with self.assertRaises(dogfood_export.Refused) as ctx_exp:
            dogfood_export.export(valid, self.root, frozenset({"aegis-vulcan-gpu"}))
        self.assertTrue(any("denied host" in r for r in ctx_exp.exception.reasons))

    def test_denied_host_name_not_echoed(self) -> None:
        """Negative: a refusal names the rule, never the private deny-hosts entry."""
        record = dict(self.valid_full_record, hostalpha="x")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(record, self.root, frozenset({"hostalpha"}))
        self.assertTrue(any("denied host" in r for r in ctx.exception.reasons))
        deny_reasons = [r for r in ctx.exception.reasons if r.startswith("deny_scan")]
        self.assertFalse(any("hostalpha" in r.lower() for r in deny_reasons))

    def test_unreadable_host_name_refuses(self) -> None:
        """Negative: without the host name the scan is incomplete, so the record is refused."""
        with mock.patch("socket.gethostname", side_effect=OSError("no name")):
            with self.assertRaises(dogfood_export.Refused) as ctx:
                dogfood_export.export(self.valid_full_record, self.root, frozenset())
        self.assertTrue(any("host name" in r for r in ctx.exception.reasons))

    def test_boundary_input_file_size(self) -> None:
        """Boundary: exactly 1 MiB file is refused; 1 MiB - 1 bytes parses cleanly."""
        file_1mib = self.root / "file_1mib.json"
        file_1mib.write_bytes(b" " * (1024 * 1024))
        data, err = dogfood_export._read_record(file_1mib)
        self.assertIsNone(data)
        self.assertIn("1 MiB limit", err or "")

        valid_bytes = dogfood_export.to_json(self.valid_full_record).encode("utf-8")
        pad_size = 1024 * 1024 - 1 - len(valid_bytes)
        file_under_1mib = self.root / "file_under_1mib.json"
        file_under_1mib.write_bytes(valid_bytes + b" " * pad_size)
        self.assertEqual(file_under_1mib.stat().st_size, 1024 * 1024 - 1)
        parsed, parse_err = dogfood_export._read_record(file_under_1mib)
        self.assertIsNone(parse_err)
        self.assertIsNotNone(parsed)
        self.assertEqual(parsed["schema"], "aegis-dogfood-export/1")

    def test_boundary_deny_hosts_comments_and_empty_lines(self) -> None:
        """Boundary: deny-hosts loader ignores comments, inline comments and blanks."""
        deny_file = self.root / "deny.txt"
        deny_file.write_text(
            "# Header comment\n\nhostalpha # a planted entry\n   \n   apollo   \n# Trailer\n",
            encoding="utf-8",
        )
        hosts = dogfood_export.load_deny_hosts(deny_file)
        self.assertEqual(hosts, frozenset({"hostalpha", "apollo"}))

    def test_boundary_leap_year_dates(self) -> None:
        """Boundary: non-leap 2026-02-29 is refused; leap 2028-02-29 is accepted."""
        bad_date = dict(self.valid_full_record, date="2026-02-29")
        with self.assertRaises(dogfood_export.Refused) as ctx:
            dogfood_export.export(bad_date, self.root, frozenset())
        self.assertTrue(any("date" in r for r in ctx.exception.reasons))

        good_date = dict(self.valid_full_record, date="2028-02-29")
        out = dogfood_export.export(good_date, self.root, frozenset())
        self.assertEqual(out["date"], "2028-02-29")

    def test_cli_subprocess(self) -> None:
        """CLI: executes via subprocess; verifies return codes and output file creation."""
        script = pathlib.Path(__file__).resolve().parent / "dogfood_export.py"

        in_valid = self.root / "cli_valid.json"
        in_valid.write_text(dogfood_export.to_json(self.valid_full_record), encoding="utf-8")
        out_valid = self.root / "cli_out_valid.json"

        proc = subprocess.run(
            [
                sys.executable,
                str(script),
                str(in_valid),
                "--out",
                str(out_valid),
                "--root",
                str(self.root),
            ],
            capture_output=True,
            text=True,
        )
        self.assertEqual(proc.returncode, 0, f"CLI stderr: {proc.stderr}")
        self.assertTrue(out_valid.exists())

        in_bad = self.root / "cli_bad.json"
        in_bad.write_text(json.dumps({"invalid_key": True}), encoding="utf-8")
        out_bad = self.root / "cli_out_bad.json"

        proc_bad = subprocess.run(
            [
                sys.executable,
                str(script),
                str(in_bad),
                "--out",
                str(out_bad),
                "--root",
                str(self.root),
            ],
            capture_output=True,
            text=True,
        )
        self.assertEqual(proc_bad.returncode, 1)
        self.assertIn("REFUSED", proc_bad.stderr)
        self.assertFalse(out_bad.exists())


if __name__ == "__main__":
    unittest.main()
