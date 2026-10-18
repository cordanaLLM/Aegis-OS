#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative and boundary tests for the image-side boot-report unit (HISS-15)."""

import io
import unittest
from contextlib import redirect_stdout
from unittest.mock import patch

import boot_report_unit as unit
import verify_boot_harness as harness

UNIT_TEXT = unit.UNIT_PATH.read_text(encoding="utf-8")
PRESET_TEXT = unit.PRESET_PATH.read_text(encoding="utf-8")
IMAGE_SCRIPT = unit.IMAGE_SCRIPT_PATH.read_text(encoding="utf-8")
GUEST_SCRIPT = unit.GUEST_SCRIPT_PATH.read_text(encoding="utf-8")
CREDENTIAL = harness.NONCE_CREDENTIAL
TIMEOUT = harness.REPORT_TIMEOUT


class PositiveTests(unittest.TestCase):
    """Positive tests over the real committed files."""

    def test_real_tracked_files_pass_all_checks(self):
        issues = unit.problems(
            UNIT_TEXT, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertEqual(issues, [])

    def test_main_prints_ok_and_returns_zero(self):
        buf = io.StringIO()
        with redirect_stdout(buf):
            ret = unit.main()
        self.assertEqual(ret, 0)
        self.assertIn("ok: boot-report unit", buf.getvalue())


class NegativeTests(unittest.TestCase):
    """Negative tests planted on copies of real text."""

    def test_missing_condition_credential_refused(self):
        planted = UNIT_TEXT.replace(f"ConditionCredential={CREDENTIAL}\n", "")
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertTrue(any("ConditionCredential" in i for i in issues))

    def test_wrong_credential_name_in_import_credential(self):
        planted = UNIT_TEXT.replace(
            f"ImportCredential={CREDENTIAL}", "ImportCredential=wrong.nonce"
        )
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertTrue(any("ImportCredential" in i for i in issues))

    def test_type_simple_refused(self):
        planted = UNIT_TEXT.replace("Type=oneshot", "Type=simple")
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertTrue(any("Type" in i for i in issues))

    def test_exec_start_drift_refused(self):
        planted = UNIT_TEXT.replace(
            "ExecStart=/bin/sh /usr/libexec/aegis/aegis-boot-report.sh",
            "ExecStart=/bin/sh /usr/libexec/aegis/other.sh",
        )
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertTrue(any("ExecStart" in i for i in issues))

    def test_timeout_start_sec_missing_refused(self):
        planted = "\n".join(
            line for line in UNIT_TEXT.splitlines() if not line.startswith("TimeoutStartSec")
        )
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertTrue(any("TimeoutStartSec" in i for i in issues))

    def test_timeout_start_sec_non_integer_refused(self):
        planted = UNIT_TEXT.replace("TimeoutStartSec=60", "TimeoutStartSec=abc")
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertTrue(any("TimeoutStartSec" in i for i in issues))

    def test_wanted_by_drift_refused(self):
        planted = UNIT_TEXT.replace("WantedBy=multi-user.target", "WantedBy=rescue.target")
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertTrue(any("WantedBy" in i for i in issues))

    def test_preset_without_enable_line_refused(self):
        planted = PRESET_TEXT.replace(
            "enable aegis-boot-report.service", "disable aegis-boot-report.service"
        )
        issues = unit.problems(UNIT_TEXT, planted, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT)
        self.assertTrue(any("preset" in i for i in issues))

    def test_report_block_drift_refused(self):
        planted = IMAGE_SCRIPT.replace(
            'echo "${MARK}-END"', 'echo "${MARK}-MUTATED"\n\techo "${MARK}-END"'
        )
        issues = unit.problems(UNIT_TEXT, PRESET_TEXT, planted, GUEST_SCRIPT, CREDENTIAL, TIMEOUT)
        self.assertTrue(any("report block" in i for i in issues))

    def test_image_script_not_reading_credential_refused(self):
        planted = IMAGE_SCRIPT.replace(f"${{CREDENTIALS_DIRECTORY:?}}/{CREDENTIAL}", "/tmp/nonce")
        issues = unit.problems(UNIT_TEXT, PRESET_TEXT, planted, GUEST_SCRIPT, CREDENTIAL, TIMEOUT)
        self.assertTrue(any("image script" in i for i in issues))

    def test_line_outside_section_raises_value_error(self):
        planted = "Key=Value\n[Unit]\nDescription=Test\n"
        with self.assertRaises(ValueError):
            unit.unit_settings(planted)

    def test_line_without_equals_raises_value_error(self):
        planted = "[Unit]\nInvalidLineWithoutEquals\n"
        with self.assertRaises(ValueError):
            unit.unit_settings(planted)

    def test_unit_exceeding_max_lines_raises_value_error(self):
        with self.assertRaises(ValueError):
            unit.unit_settings("\n" * (unit.MAX_LINES + 1))

    def test_missing_report_block_raises_value_error(self):
        with self.assertRaises(ValueError):
            unit.report_block("echo no block here\n")

    def test_main_failure_path_prints_fail_and_returns_one(self):
        with patch.object(unit, "problems", return_value=["planted problem"]):
            buf = io.StringIO()
            with redirect_stdout(buf):
                ret = unit.main()
            self.assertEqual(ret, 1)
            self.assertIn("FAIL: planted problem", buf.getvalue())


class BoundaryTests(unittest.TestCase):
    """Boundary conditions on TimeoutStartSec."""

    def test_timeout_boundary_exact_report_timeout_ok(self):
        planted = UNIT_TEXT.replace("TimeoutStartSec=60", f"TimeoutStartSec={TIMEOUT}")
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertEqual(issues, [])

    def test_timeout_boundary_report_timeout_plus_one_refused(self):
        planted = UNIT_TEXT.replace("TimeoutStartSec=60", f"TimeoutStartSec={TIMEOUT + 1}")
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertTrue(any("TimeoutStartSec" in i for i in issues))

    def test_timeout_boundary_zero_refused(self):
        planted = UNIT_TEXT.replace("TimeoutStartSec=60", "TimeoutStartSec=0")
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertTrue(any("TimeoutStartSec" in i for i in issues))

    def test_timeout_boundary_one_ok(self):
        planted = UNIT_TEXT.replace("TimeoutStartSec=60", "TimeoutStartSec=1")
        issues = unit.problems(
            planted, PRESET_TEXT, IMAGE_SCRIPT, GUEST_SCRIPT, CREDENTIAL, TIMEOUT
        )
        self.assertEqual(issues, [])


if __name__ == "__main__":
    unittest.main()
