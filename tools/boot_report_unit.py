#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Check the boot-report unit the Aegis image ships for the M24 boot harness (D114).

An Imago guest has no cloud-init, so the harness passes its nonce as the system
credential verify_boot_harness.NONCE_CREDENTIAL, and the image's own
aegis-boot-report.service writes the report. This check holds the image side to
what the harness expects: the unit runs only when that credential is present
and imports it, it is a oneshot under the harness's report deadline, a preset
enables it, and its script writes the same report block as
tools/guest/aegis-boot-report.sh, which the upstream path hands to cloud-init.
"""

import sys
from pathlib import Path

import verify_boot_harness

ROOT = Path(__file__).resolve().parent.parent
EXTRA = ROOT / "build" / "mkosi.extra" / "usr"
UNIT_PATH = EXTRA / "lib" / "systemd" / "system" / "aegis-boot-report.service"
PRESET_PATH = EXTRA / "lib" / "systemd" / "system-preset" / "50-aegis-boot-report.preset"
IMAGE_SCRIPT_PATH = EXTRA / "libexec" / "aegis" / "aegis-boot-report.sh"
GUEST_SCRIPT_PATH = ROOT / "tools" / "guest" / "aegis-boot-report.sh"

MAX_LINES = 200
EXPECTED_EXEC_START = ["/bin/sh /usr/libexec/aegis/aegis-boot-report.sh"]
EXPECTED_TYPE = ["oneshot"]
EXPECTED_WANTED_BY = ["multi-user.target"]
PRESET_ENABLE_LINE = "enable aegis-boot-report.service"


def is_ignorable(line):
    """Return whether a line is empty or a systemd comment."""
    return not line or line.startswith(("#", ";"))


def unit_settings(text):
    """Parse systemd INI unit text into section -> {key: [values]}."""
    lines = text.splitlines()
    if len(lines) > MAX_LINES:
        raise ValueError(f"unit file exceeds {MAX_LINES} lines: {len(lines)}")
    sections = {}
    current_section = None
    for lineno, raw_line in enumerate(lines, start=1):
        line = raw_line.strip()
        if is_ignorable(line):
            continue
        if line.startswith("[") and line.endswith("]"):
            current_section = line[1:-1].strip()
            sections.setdefault(current_section, {})
        elif current_section is None:
            raise ValueError(f"line outside section at line {lineno}: {raw_line}")
        elif "=" not in line:
            raise ValueError(f"line without '=' at line {lineno}: {raw_line}")
        else:
            key, value = line.split("=", 1)
            sections[current_section].setdefault(key.strip(), []).append(value.strip())
    return sections


def report_block(text):
    """Return lines from '{' up to and including '} >\"${REPORT}\"'.

    Raises ValueError if either start or end delimiter is absent.
    """
    lines = text.splitlines()
    start = None
    end = None
    for idx, line in enumerate(lines):
        if start is None:
            if line.startswith("{"):
                start = idx
        elif line.startswith('} >"${REPORT}"'):
            end = idx
            break
    if start is None or end is None:
        raise ValueError("report block delimiters not found")
    return lines[start : end + 1]


def check_timeout(service, report_timeout):
    """Verify TimeoutStartSec is an integer between 1 and report_timeout."""
    timeouts = service.get("TimeoutStartSec", [])
    if len(timeouts) != 1:
        return ["Service.TimeoutStartSec must be specified exactly once"]
    try:
        sec = int(timeouts[0])
    except ValueError:
        return [f"Service.TimeoutStartSec is not an integer: {timeouts[0]}"]
    if sec < 1 or sec > report_timeout:
        return [f"Service.TimeoutStartSec {sec} outside range 1..{report_timeout}"]
    return []


def check_unit_settings(settings, credential, report_timeout):
    """Check required systemd unit directives and timeouts."""
    issues = []
    unit = settings.get("Unit", {})
    service = settings.get("Service", {})
    install = settings.get("Install", {})

    if unit.get("ConditionCredential") != [credential]:
        issues.append(f"Unit.ConditionCredential must be [{credential}]")
    if service.get("ImportCredential") != [credential]:
        issues.append(f"Service.ImportCredential must be [{credential}]")
    if service.get("Type") != EXPECTED_TYPE:
        issues.append("Service.Type must be ['oneshot']")
    if service.get("ExecStart") != EXPECTED_EXEC_START:
        issues.append(f"Service.ExecStart must be {EXPECTED_EXEC_START}")

    issues.extend(check_timeout(service, report_timeout))

    if install.get("WantedBy") != EXPECTED_WANTED_BY:
        issues.append("Install.WantedBy must be ['multi-user.target']")
    return issues


def problems(unit_text, preset_text, image_script, guest_script, credential, report_timeout):
    """Verify systemd unit, preset, and image/guest scripts match all requirements."""
    settings = unit_settings(unit_text)
    issues = check_unit_settings(settings, credential, report_timeout)

    preset_lines = [line.strip() for line in preset_text.splitlines()]
    if PRESET_ENABLE_LINE not in preset_lines:
        issues.append(f"preset missing '{PRESET_ENABLE_LINE}'")

    expected_cred = f"${{CREDENTIALS_DIRECTORY:?}}/{credential}"
    if expected_cred not in image_script:
        issues.append(f"image script does not mention {expected_cred}")

    if report_block(image_script) != report_block(guest_script):
        issues.append("report block drift between image script and guest script")

    return issues


def main():
    """Read tracked files, verify boot-report settings, and report issues."""
    unit_text = UNIT_PATH.read_text(encoding="utf-8")
    preset_text = PRESET_PATH.read_text(encoding="utf-8")
    image_script = IMAGE_SCRIPT_PATH.read_text(encoding="utf-8")
    guest_script = GUEST_SCRIPT_PATH.read_text(encoding="utf-8")

    issues = problems(
        unit_text=unit_text,
        preset_text=preset_text,
        image_script=image_script,
        guest_script=guest_script,
        credential=verify_boot_harness.NONCE_CREDENTIAL,
        report_timeout=verify_boot_harness.REPORT_TIMEOUT,
    )
    if issues:
        for issue in issues:
            print(f"FAIL: {issue}")
        return 1
    print("ok: boot-report unit")
    return 0


if __name__ == "__main__":
    sys.exit(main())
