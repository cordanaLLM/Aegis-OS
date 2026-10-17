#!/usr/bin/env python3
"""Parse the P01 image definition with the host's own mkosi and fail on drift.

Static section check plus four cases, no simulation and no failure suppression:

* a static check scans ``build`` and drop-ins for unknown section headers that
  mkosi skips without a diagnostic;
* the positive case runs ``mkosi summary`` over ``build`` and requires exit 0
  with a parsable Output stanza that names the reviewed repart directory, the
  pinned distribution snapshot and the declared output format;
* the negative floor case raises ``MinimumVersion=`` above the admitted floor in a
  scratch copy and requires mkosi to refuse the configuration;
* the negative ignored-setting case plants an unknown setting in a scratch copy
  and requires mkosi to report it as ignored;
* the boundary case sets ``MinimumVersion=`` to the floor itself and requires
  mkosi to accept it.

``mkosi summary`` resolves configuration and prints it. It downloads nothing,
builds nothing and writes nothing into the repository: decision D56 keeps
image-definition validation in Aegis while Imago is a scaffold, and validation
is not construction. A pass is development evidence on the reference profile and
closes no image, boot, hardware or release gate.
"""

import argparse
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import definitions_container
import gate_policy
from host import target as posix_target

ROOT = Path(__file__).resolve().parent.parent
# Admitted floor, recorded in docs/roadmap/toolchain-admission.md. build/mkosi.conf
# carries the same number as MinimumVersion=, so mkosi itself refuses an older
# mkosi; this constant is what keeps the gate's own cases honest about which
# version the recorded outcomes were observed on.
MKOSI_FLOOR = 27
REFERENCE_PROFILE_MKOSI = "mkosi 27"
REFERENCE_PROFILE_MKOSI_PACKAGE = "extra/mkosi 27-1"
# This gate's name in AEGIS_REQUIRED_GATES: a run that lists it fails, rather
# than skips, when mkosi is absent or below the floor.
GATE = "mkosi"
DEFINITION_DIRECTORY = "build"
MAIN_IMAGE = "IMAGE: main"
OUTPUT_STANZA = "OUTPUT:"
DISTRIBUTION_STANZA = "DISTRIBUTION:"
COMMAND_TIMEOUT = 300
MAX_SUMMARY_LINES = 4096
MAX_DIAGNOSTIC_LINES = 64
# What the Output and Distribution stanzas must say about the reviewed
# definition. Each value is read back from the stanza rather than assumed.
EXPECTED_OUTPUT = {
    "Output Format": "disk",
    "Output": "aegis-os.raw",
    "Image ID": "aegis-os",
}
EXPECTED_DISTRIBUTION = {
    "Distribution": "arch",
    "Architecture": "x86-64",
    "Snapshot": "2026/09/13",
}
# The reviewed partition-definition set, named relative to whichever definition
# directory a case runs from: the repository's own for the positive case, and
# the scratch copy's for each floor case. It is compared as a resolved path, so
# a directory that merely ends the same way is a different directory.
REPART_DIRECTORY = "repart.d"
# Scalar bound on the resolved repart directory list. A summary listing more is
# reported rather than truncated, because truncating is how an unreviewed set
# would pass unseen.
MAX_REPART_DIRECTORIES = 32
MAX_DROPIN_FILES = 32
REFUSAL = "or newer is required by this configuration"
UNKNOWN_SETTING = "Unknown setting"
IGNORED_DIAGNOSTICS = (
    "should be configured in [",
    "is deprecated, please use",
    "specifier is deprecated",
    UNKNOWN_SETTING,
)
PLANTED_SETTING = "AegisPlantedUnknownSetting"

# The sections mkosi 27.1 config.py parses (SETTINGS sections plus Host, Match,
# TriggerMatch, Assert, TriggerAssert).
KNOWN_SECTIONS = frozenset(
    {
        "Assert",
        "Build",
        "Config",
        "Content",
        "Distribution",
        "Host",
        "Include",
        "Match",
        "Output",
        "Runtime",
        "TriggerAssert",
        "TriggerMatch",
        "Validation",
    }
)


class GateError(Exception):
    """The gate could not be run at all, as opposed to a case that failed."""


def parse_mkosi_version(text):
    """Return the major mkosi version in `text`, or None if there is none.

    Accepts what `mkosi --version` prints, for example `mkosi 27`.
    """
    match = re.match(r"^mkosi\s+(\d{1,5})\b", text.strip())
    if match is None:
        return None
    return int(match.group(1))


def run(argv, cwd=None):
    """Run `argv` and return (exit code, stdout, stderr) with a hard deadline."""
    env = dict(os.environ)
    env["SYSTEMD_COLORS"] = "0"
    env["NO_COLOR"] = "1"
    try:
        done = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=COMMAND_TIMEOUT,
            env=env,
            cwd=cwd,
            check=False,
        )
    except (subprocess.TimeoutExpired, OSError) as error:
        raise GateError(f"{argv[0]} could not be run: {error}") from error
    return done.returncode, done.stdout, done.stderr


def summary_stanza(text, stanza):
    """Return one stanza of the main image's summary as a key to values map.

    Two properties of the format matter and are handled explicitly:

    * the summary lists every image mkosi resolved, the implicit default initrd
      first. Only the ``IMAGE: main`` section is read, because the initrd is
      mkosi's own subimage and says nothing about the reviewed definition;
    * a setting that takes a collection prints its first value on the key's own
      line and each further value on an indented line of its own. Those
      continuation lines are appended to the key they belong to, so every value
      is read rather than only the first.
    """
    rows = {}
    in_main = False
    in_stanza = False
    last = None
    for line in text.splitlines()[:MAX_SUMMARY_LINES]:
        if line.startswith("IMAGE: "):
            in_main = line.strip() == MAIN_IMAGE
            in_stanza = False
            last = None
            continue
        stripped = line.strip()
        if not stripped:
            last = None
            continue
        if stripped.endswith(":") and ":" not in stripped[:-1]:
            in_stanza = in_main and stripped == stanza
            last = None
            continue
        if not in_stanza:
            continue
        if ":" in stripped:
            key, _, value = stripped.partition(":")
            last = key.strip()
            rows.setdefault(last, []).append(value.strip())
            continue
        if last is not None:
            rows[last].append(stripped)
    return rows


def check_stanza(rows, expected, label):
    """Return the problems in one parsed stanza."""
    if not rows:
        return [f"the summary carries no {label} stanza for the main image"]
    problems = []
    for key, value in expected.items():
        observed = rows.get(key)
        if observed != [value]:
            problems.append(f"{label} {key} is {observed!r}, expected {[value]!r}")
    return problems


def same_directory(row, resolved):
    """Return True when `row` names the directory `resolved` already names.

    Identity is the resolved path and nothing else. A suffix test would exempt
    every directory whose path happens to end the same way, which is exactly
    the row an unreviewed definition set would arrive on.
    """
    try:
        return Path(row).resolve() == resolved
    except (OSError, ValueError):
        return False


def check_repart_directories(listed, expected):
    """Return the problems in the resolved repart directory list.

    mkosi reads a `mkosi.repart/` directory beside the configuration, every
    `RepartDirectories=` row of `mkosi.conf`, and every row a `mkosi.conf.d/`
    drop-in adds, so the resolved list is not necessarily the reviewed set
    alone. Two things are required, and the second is what keeps the first
    honest: exactly one listed directory must be the reviewed one, compared as
    a resolved path, and no other listed directory may hold a partition
    definition. An empty extra directory is tolerated and a populated one fails
    the gate, so a second definition set cannot join the image without being
    seen.
    """
    resolved = Path(expected).resolve()
    if len(listed) > MAX_REPART_DIRECTORIES:
        return [
            f"the summary lists {len(listed)} repart directories, past the "
            f"bound of {MAX_REPART_DIRECTORIES}"
        ]
    matched = [row for row in listed if same_directory(row, resolved)]
    if len(matched) != 1:
        return [
            f"expected exactly one repart directory at {posix_target(resolved)!r}, "
            f"found {len(matched)}; the summary lists {listed!r}"
        ]
    problems = []
    for row in listed:
        if same_directory(row, resolved):
            continue
        extra = sorted(path.name for path in Path(row).glob("*.conf"))
        if extra:
            # A repart directory names a location mkosi reads on Linux, so it is
            # reported in the target's spelling and not the running host's.
            problems.append(f"repart directory {posix_target(row)!r} adds the definitions {extra}")
    return problems


def scan_sections(path):
    """Return problems found in one configuration file."""
    problems = []
    try:
        text = path.read_text(encoding="utf-8")
    except (UnicodeDecodeError, OSError) as error:
        return [f"{posix_target(path)} could not be read: {error}"]
    for line in text.splitlines()[:MAX_SUMMARY_LINES]:
        stripped = line.strip()
        if not stripped or stripped.startswith(("#", ";")):
            continue
        match = re.match(r"^\[([^\]]+)\]$", stripped)
        if match is None:
            continue
        name = match.group(1)
        if name not in KNOWN_SECTIONS:
            problems.append(
                f"{posix_target(path)}: section [{name}] is not one "
                f"mkosi {MKOSI_FLOOR} parses; mkosi skips it without a diagnostic"
            )
    return problems


def unknown_sections(directory):
    """Return problems in directory/mkosi.conf and drop-ins."""
    base = Path(directory)
    problems = []
    config = base / "mkosi.conf"
    if not config.is_file():
        problems.append(f"{posix_target(config)} does not exist")
    paths = [config] if config.is_file() else []
    dropin_dir = base / "mkosi.conf.d"
    if dropin_dir.is_dir():
        dropins = sorted(dropin_dir.glob("*.conf"))
        if len(dropins) > MAX_DROPIN_FILES:
            problems.append(
                f"{posix_target(dropin_dir)} lists {len(dropins)} drop-ins, "
                f"past the bound of {MAX_DROPIN_FILES}"
            )
        else:
            paths.extend(dropins)
    for path in paths[: MAX_DROPIN_FILES + 1]:
        problems.extend(scan_sections(path))
    return problems


def check_summary(stdout, repart_directory):
    """Return the problems in a `mkosi summary` run that was expected to pass."""
    output = summary_stanza(stdout, OUTPUT_STANZA)
    problems = check_stanza(output, EXPECTED_OUTPUT, "Output")
    problems.extend(
        check_stanza(
            summary_stanza(stdout, DISTRIBUTION_STANZA),
            EXPECTED_DISTRIBUTION,
            "Distribution",
        )
    )
    problems.extend(
        check_repart_directories(output.get("Repart Directories", []), repart_directory)
    )
    return problems


def ignored_diagnostics(text):
    """Return stripped lines from `text` containing any ignored-setting needle."""
    found = []
    for line in text.splitlines()[:MAX_SUMMARY_LINES]:
        stripped = line.strip()
        for needle in IGNORED_DIAGNOSTICS:
            if needle in stripped:
                found.append(stripped)
                break
    return found


def ignored_problems(found, code):
    """Return the problems a run that exited `code` with `found` warnings carries.

    This is the judgement the positive and boundary cases receive. The planted
    case is judged by the same function and must be rejected by it, so a
    detector that stopped rejecting anything fails the planted case on a real
    mkosi run, not only in the unit tests.
    """
    if code != 0:
        return []
    return [f"mkosi ignored part of the input: {line}" for line in found]


def check_ignored_diagnostics(found, expects_ignored, code):
    """Return problems from ignored-setting diagnostics for one case."""
    rejected = ignored_problems(found, code)
    if not expects_ignored:
        return rejected
    if not rejected:
        return ["expected the gate to reject mkosi's report of the planted setting"]
    return []


def scratch_definition(base, minimum):
    """Copy the reviewed definition into `base` with a rewritten MinimumVersion.

    Exactly the one line is rewritten; everything else reaches mkosi unchanged.
    A file with no such line, or with more than one, is a refusal rather than a
    silent no-op.
    """
    target = base / f"minimum-{minimum}"
    shutil.copytree(ROOT / DEFINITION_DIRECTORY, target)
    config = target / "mkosi.conf"
    lines = config.read_text().splitlines()
    hits = [index for index, line in enumerate(lines) if line.startswith("MinimumVersion=")]
    if len(hits) != 1:
        raise GateError(f"expected exactly one MinimumVersion= line, found {len(hits)}")
    lines[hits[0]] = f"MinimumVersion={minimum}"
    config.write_text("\n".join(lines) + "\n")
    return target


def scratch_ignored_setting(base):
    """Copy the reviewed definition into `base` with one planted unknown setting."""
    target = base / "ignored-setting"
    shutil.copytree(ROOT / DEFINITION_DIRECTORY, target)
    config = target / "mkosi.conf"
    with config.open("a", encoding="utf-8") as handle:
        handle.write(f"\n[Output]\n{PLANTED_SETTING}=1\n")
    return target


def cases(base):
    """Return the positive, negative and boundary cases, in order.

    Each case that is expected to resolve carries the directory its own
    reviewed set must be, so the floor cases assert their scratch copy rather
    than anything that reads like one.
    """
    reviewed = ROOT / DEFINITION_DIRECTORY
    boundary = scratch_definition(base, MKOSI_FLOOR)
    return [
        {
            "name": "mkosi/positive",
            "directory": reviewed,
            "code": 0,
            "requires": (),
            "repart_directory": reviewed / REPART_DIRECTORY,
            "expects_ignored": False,
        },
        {
            "name": "mkosi/negative-below-floor",
            "directory": scratch_definition(base, MKOSI_FLOOR + 1),
            "code": 1,
            "requires": (REFUSAL,),
            "repart_directory": None,
            "expects_ignored": False,
        },
        {
            "name": "mkosi/negative-ignored-setting",
            "directory": scratch_ignored_setting(base),
            "code": 0,
            "requires": (UNKNOWN_SETTING,),
            "repart_directory": None,
            "expects_ignored": True,
        },
        {
            "name": "mkosi/boundary-at-floor",
            "directory": boundary,
            "code": 0,
            "requires": (),
            "repart_directory": boundary / REPART_DIRECTORY,
            "expects_ignored": False,
        },
    ]


def execute(case):
    """Run one case and return (exit code, problems, diagnostics)."""
    code, stdout, stderr = run(
        ["mkosi", "--no-pager", "--directory", str(case["directory"]), "summary"]
    )
    diagnostics = "\n".join((stderr + "\n" + stdout).splitlines()[:MAX_DIAGNOSTIC_LINES])
    problems = []
    if code != case["code"]:
        problems.append(f"expected exit {case['code']}, observed {code}")
    for needle in case["requires"]:
        if needle not in stderr and needle not in stdout:
            problems.append(f"expected the diagnostic {needle!r}")
    found = ignored_diagnostics(stderr + "\n" + stdout)
    problems.extend(check_ignored_diagnostics(found, case["expects_ignored"], code))
    if case["repart_directory"] is not None and code == 0:
        problems.extend(check_summary(stdout, case["repart_directory"]))
    return code, problems, diagnostics


def report(case, code, problems, diagnostics):
    """Print one case's outcome, and its diagnostics when it failed."""
    status = "PASS" if not problems else "FAIL"
    print(f"{status} {case['name']}: exit {code}")
    if not problems:
        return
    for line in problems:
        print(f"     {line}")
    for line in diagnostics.splitlines()[:12]:
        print(f"     | {line}")


def guard():
    """Return the host banner when the gate may run, or None after saying why."""
    if shutil.which("mkosi") is None:
        gate_policy.did_not_run(GATE, "mkosi not on PATH")
        return None
    code, stdout, _ = run(["mkosi", "--version"])
    banner = stdout.splitlines()[0].strip() if stdout else ""
    version = parse_mkosi_version(banner) if code == 0 else None
    if version is None:
        gate_policy.did_not_run(GATE, "no mkosi version could be read")
        return None
    if version < MKOSI_FLOOR:
        gate_policy.did_not_run(
            GATE,
            f"host reports {banner!r}, below the admitted floor mkosi {MKOSI_FLOOR}",
            f"The recorded reference profile is {REFERENCE_PROFILE_MKOSI!r} "
            f"({REFERENCE_PROFILE_MKOSI_PACKAGE}).",
        )
        return None
    return banner


def run_cases(base):
    """Run every case in order and return the number that failed."""
    failed = 0
    for case in cases(base):
        code, problems, diagnostics = execute(case)
        report(case, code, problems, diagnostics)
        failed += 1 if problems else 0
    return failed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    if gate_policy.refuse_unknown():
        return 1
    section_problems = unknown_sections(ROOT / DEFINITION_DIRECTORY)
    if section_problems:
        for problem in section_problems[:MAX_SUMMARY_LINES]:
            print(f"FAIL: {problem}")
        return 1
    routed = definitions_container.dispatch(GATE, Path(__file__).name)
    if routed is not None:
        return routed
    banner = guard()
    if banner is None:
        return 1 if gate_policy.is_required(GATE) else 0
    print(f"mkosi definition gate on {banner!r} (floor mkosi {MKOSI_FLOOR}).")
    try:
        with tempfile.TemporaryDirectory(prefix="aegis-mkosi-") as base:
            failed = run_cases(Path(base))
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    if failed:
        print(f"FAIL: {failed} mkosi definition case(s) did not match their recorded outcome.")
        return 1
    print(
        "PASS: mkosi definition gate on the reference profile; the configuration "
        "was parsed, no image was constructed, and no image, boot, hardware or "
        "release gate is closed."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
