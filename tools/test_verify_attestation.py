#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Unit tests for tools/verify_attestation.py.

Positive, negative, and boundary tests for every pure function:
- parse_digest
- parse_pcr_selection
- judge_selection
- case judges (quote_verifies, other_record_fails, pcr_state_changed_fails,
  exact_selection_accepted, missing_pcr11_rejected)
- summary
- capability_reasons
- argv builders
"""

import sys
import unittest
from pathlib import Path

TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import verify_attestation as va  # noqa: E402

FIXTURE_TPMS_ATTEST_0_4_7_11 = """magic: ff544347
type: 8018
qualifiedSigner: 000be990c78eb34dc9d931e3a4a4d252e34a9e861edd2eb68f1149a14579f5a8a73b
extraData: 95d9367ea5a97b4400b195d499116a24433c9f21c4f6dec188e0aa767741b914
clockInfo:
  clock: 159
  resetCount: 1
  restartCount: 0
  safe: 1
firmwareVersion: 0000120025012420
attested:
  quote:
    pcrSelect:
      count: 1
      pcrSelections:
        0:
          hash: 11 (sha256)
          sizeofSelect: 3
          pcrSelect: 910800
    pcrDigest: 55b9d74b208fd5de80534abe07a55554c69eaa8c0d1f2a5883df958fc9f2c045
"""

FIXTURE_TPMS_ATTEST_0_4_7 = """magic: ff544347
type: 8018
qualifiedSigner: 000be990c78eb34dc9d931e3a4a4d252e34a9e861edd2eb68f1149a14579f5a8a73b
extraData: 95d9367ea5a97b4400b195d499116a24433c9f21c4f6dec188e0aa767741b914
clockInfo:
  clock: 159
  resetCount: 1
  restartCount: 0
  safe: 1
firmwareVersion: 0000120025012420
attested:
  quote:
    pcrSelect:
      count: 1
      pcrSelections:
        0:
          hash: 11 (sha256)
          sizeofSelect: 3
          pcrSelect: 910000
    pcrDigest: 55b9d74b208fd5de80534abe07a55554c69eaa8c0d1f2a5883df958fc9f2c045
"""

FIXTURE_TPMS_ATTEST_910008 = """magic: ff544347
type: 8018
qualifiedSigner: 000be990c78eb34dc9d931e3a4a4d252e34a9e861edd2eb68f1149a14579f5a8a73b
extraData: 95d9367ea5a97b4400b195d499116a24433c9f21c4f6dec188e0aa767741b914
clockInfo:
  clock: 159
  resetCount: 1
  restartCount: 0
  safe: 1
firmwareVersion: 0000120025012420
attested:
  quote:
    pcrSelect:
      count: 1
      pcrSelections:
        0:
          hash: 11 (sha256)
          sizeofSelect: 3
          pcrSelect: 910008
    pcrDigest: 55b9d74b208fd5de80534abe07a55554c69eaa8c0d1f2a5883df958fc9f2c045
"""


class TestParseDigest(unittest.TestCase):
    """Tests for parse_digest."""

    def test_positive_canonical_digest(self):
        text = "DIGEST 95d9367ea5a97b4400b195d499116a24433c9f21c4f6dec188e0aa767741b914\n"
        digest = va.parse_digest(text)
        self.assertEqual(digest, "95d9367ea5a97b4400b195d499116a24433c9f21c4f6dec188e0aa767741b914")

    def test_positive_with_surrounding_output(self):
        text = (
            "   Compiling aegis-justitia v0.1.0\n"
            "    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.12s\n"
            "     Running `target/debug/examples/record_digest`\n"
            "DIGEST 5daf828f8dead22049fc279f9c984257ba8964adeca6401431fa472821aa06a1\n"
        )
        self.assertEqual(
            va.parse_digest(text),
            "5daf828f8dead22049fc279f9c984257ba8964adeca6401431fa472821aa06a1",
        )

    def test_negative_no_digest_line(self):
        with self.assertRaises(va.GateError):
            va.parse_digest("error: compilation failed\n")

    def test_boundary_short_or_invalid_hex(self):
        with self.assertRaises(va.GateError):
            va.parse_digest("DIGEST 1234abcd\n")
        with self.assertRaises(va.GateError):
            va.parse_digest("DIGEST " + "g" * 64 + "\n")


class TestParsePcrSelection(unittest.TestCase):
    """Tests for parse_pcr_selection."""

    def test_positive_0_4_7_11(self):
        raw_hex, pcrs = va.parse_pcr_selection(FIXTURE_TPMS_ATTEST_0_4_7_11)
        self.assertEqual(raw_hex, "910800")
        self.assertEqual(pcrs, {0, 4, 7, 11})

    def test_boundary_0_4_7(self):
        raw_hex, pcrs = va.parse_pcr_selection(FIXTURE_TPMS_ATTEST_0_4_7)
        self.assertEqual(raw_hex, "910000")
        self.assertEqual(pcrs, {0, 4, 7})

    def test_boundary_910008(self):
        raw_hex, pcrs = va.parse_pcr_selection(FIXTURE_TPMS_ATTEST_910008)
        self.assertEqual(raw_hex, "910008")
        self.assertEqual(pcrs, {0, 4, 7, 19})

    def test_negative_missing_pcrselect(self):
        with self.assertRaises(va.GateError):
            va.parse_pcr_selection("extraData: 1234\nclockInfo:\n  safe: 1\n")


class TestJudgeSelection(unittest.TestCase):
    """Tests for judge_selection."""

    def test_positive_accepts_910800(self):
        ok, why = va.judge_selection(FIXTURE_TPMS_ATTEST_0_4_7_11)
        self.assertTrue(ok)
        self.assertEqual(why, "")

    def test_negative_rejects_910008_pcr19_for_11(self):
        # 910008 selects PCR 19 in place of PCR 11; swtpm 0.10.2 prints 910800
        # for 0, 4, 7, 11 (measured 2026-10-18), so no bitmap literal may pass.
        ok, why = va.judge_selection(FIXTURE_TPMS_ATTEST_910008)
        self.assertFalse(ok)
        self.assertIn("missing PCR 11", why)
        self.assertIn("910008", why)

    def test_negative_rejects_missing_pcr11(self):
        ok, why = va.judge_selection(FIXTURE_TPMS_ATTEST_0_4_7)
        self.assertFalse(ok)
        self.assertIn("11", why)
        self.assertIn("910000", why)

    def test_negative_missing_selection_line(self):
        ok, why = va.judge_selection("invalid text")
        self.assertFalse(ok)
        self.assertIn("no valid pcrSelect line", why)

    def test_boundary_unexpected_pcr(self):
        text = "pcrSelect: 930800\n"  # PCR 1 also selected (0x93 = 0x91 | 0x02)
        ok, why = va.judge_selection(text)
        self.assertFalse(ok)
        self.assertIn("unexpected PCR(s)", why)


class TestJudges(unittest.TestCase):
    """Tests for case-specific verdict judges."""

    def test_judge_quote_verifies_positive(self):
        self.assertEqual(va.judge_quote_verifies(0, True, ""), (True, ""))

    def test_judge_quote_verifies_negative_rc(self):
        ok, why = va.judge_quote_verifies(1, True, "")
        self.assertFalse(ok)
        self.assertIn("exited 1", why)

    def test_judge_quote_verifies_negative_selection(self):
        ok, why = va.judge_quote_verifies(0, False, "bad selection")
        self.assertFalse(ok)
        self.assertIn("selection rejected", why)

    def test_judge_other_record_fails_positive(self):
        self.assertEqual(va.judge_other_record_fails(1), (True, ""))

    def test_judge_other_record_fails_negative(self):
        ok, why = va.judge_other_record_fails(0)
        self.assertFalse(ok)
        self.assertIn("unexpectedly succeeded", why)

    def test_judge_other_record_fails_boundary_rc2(self):
        self.assertEqual(va.judge_other_record_fails(2), (True, ""))

    def test_judge_pcr_state_changed_fails_positive(self):
        self.assertEqual(va.judge_pcr_state_changed_fails(1), (True, ""))

    def test_judge_pcr_state_changed_fails_negative(self):
        ok, why = va.judge_pcr_state_changed_fails(0)
        self.assertFalse(ok)
        self.assertIn("unexpectedly succeeded", why)

    def test_judge_exact_selection_accepted_positive(self):
        self.assertEqual(va.judge_exact_selection_accepted(True, ""), (True, ""))

    def test_judge_exact_selection_accepted_negative(self):
        ok, why = va.judge_exact_selection_accepted(False, "missing PCR 11")
        self.assertFalse(ok)
        self.assertIn("missing PCR 11", why)

    def test_judge_missing_pcr11_rejected_positive(self):
        ok, why = va.judge_missing_pcr11_rejected(0, False, "missing PCR 11 (bitmap: 910000)")
        self.assertTrue(ok)
        self.assertEqual(why, "")

    def test_judge_missing_pcr11_rejected_negative_checkquote_fail(self):
        ok, why = va.judge_missing_pcr11_rejected(1, False, "missing PCR 11")
        self.assertFalse(ok)
        self.assertIn("exited 1", why)

    def test_judge_missing_pcr11_rejected_negative_selection_accepted(self):
        ok, why = va.judge_missing_pcr11_rejected(0, True, "")
        self.assertFalse(ok)
        self.assertIn("unexpectedly accepted", why)

    def test_judge_missing_pcr11_rejected_negative_unnamed_pcr11(self):
        ok, why = va.judge_missing_pcr11_rejected(0, False, "unrelated failure")
        self.assertFalse(ok)
        self.assertIn("did not name PCR 11", why)


class TestSummary(unittest.TestCase):
    """Tests for summary()."""

    def test_positive_all_five_pass(self):
        cases = [
            ("attest/quote-verifies", True, ""),
            ("attest/other-record-fails", True, ""),
            ("attest/pcr-state-changed-fails", True, ""),
            ("attest/exact-selection-accepted", True, ""),
            ("attest/missing-pcr11-rejected", True, ""),
        ]
        self.assertEqual(va.summary(cases), "RESULT pass")

    def test_negative_one_failure(self):
        cases = [
            ("attest/quote-verifies", True, ""),
            ("attest/other-record-fails", False, "failed"),
            ("attest/pcr-state-changed-fails", True, ""),
            ("attest/exact-selection-accepted", True, ""),
            ("attest/missing-pcr11-rejected", True, ""),
        ]
        self.assertEqual(va.summary(cases), "RESULT fail: 1 case(s) failed")

    def test_boundary_wrong_case_count(self):
        cases = [
            ("attest/quote-verifies", True, ""),
            ("attest/other-record-fails", True, ""),
        ]
        self.assertEqual(va.summary(cases), "RESULT fail: 2 of 5 cases ran")


class TestCapabilityReasons(unittest.TestCase):
    """Tests for capability_reasons()."""

    def test_positive_all_present(self):
        reasons = va.capability_reasons(
            platform="linux",
            which=lambda prog: f"/usr/bin/{prog}",
            tcti_check=lambda: True,
        )
        self.assertEqual(reasons, [])

    def test_negative_non_linux(self):
        reasons = va.capability_reasons(platform="darwin")
        self.assertEqual(len(reasons), 1)
        self.assertIn("Linux", reasons[0])

    def test_negative_missing_swtpm(self):
        def fake_which(prog):
            return None if prog == "swtpm" else f"/usr/bin/{prog}"

        reasons = va.capability_reasons(
            platform="linux",
            which=fake_which,
            tcti_check=lambda: True,
        )
        self.assertIn("swtpm is not installed", reasons)

    def test_negative_missing_tcti(self):
        reasons = va.capability_reasons(
            platform="linux",
            which=lambda prog: f"/usr/bin/{prog}",
            tcti_check=lambda: False,
        )
        self.assertIn("libtss2-tcti-swtpm.so is missing", reasons)


class TestArgvBuilders(unittest.TestCase):
    """Tests for argv builder functions."""

    def test_swtpm_command(self):
        cmd = va.swtpm_command("/tmp/state", "/tmp/sock", "/tmp/ctrl")
        self.assertEqual(cmd[0], "swtpm")
        self.assertEqual(cmd[1], "socket")
        self.assertIn("--flags=not-need-init,startup-clear", cmd)

    def test_quote_command(self):
        cmd = va.quote_command("ak.ctx", "0,4,7,11", "95d9", "q.msg", "q.sig", "q.pcrs")
        self.assertEqual(cmd[0], "tpm2_quote")
        self.assertIn("-l", cmd)
        self.assertIn("sha256:0,4,7,11", cmd)
        self.assertIn("-q", cmd)
        self.assertIn("95d9", cmd)

    def test_checkquote_command(self):
        cmd = va.checkquote_command("ak.pub", "q.msg", "q.sig", "q.pcrs", "95d9")
        self.assertEqual(cmd[0], "tpm2_checkquote")
        self.assertIn("-q", cmd)
        self.assertIn("95d9", cmd)

    def test_cargo_digest_command(self):
        self.assertEqual(
            va.cargo_digest_command(other=False),
            [
                "cargo",
                "run",
                "--locked",
                "--offline",
                "-q",
                "-p",
                "aegis-justitia",
                "--example",
                "record_digest",
            ],
        )
        self.assertEqual(
            va.cargo_digest_command(other=True),
            [
                "cargo",
                "run",
                "--locked",
                "--offline",
                "-q",
                "-p",
                "aegis-justitia",
                "--example",
                "record_digest",
                "other",
            ],
        )


if __name__ == "__main__":
    unittest.main()
