#!/usr/bin/env python3
"""Run the P01/P02 definitions through the host's own systemd and fail on drift.

Two gates, no simulation and no failure suppression:

* the repart gate builds a scratch image from ``build/repart.d`` and requires
  exit 0 with no ``Unknown key ... ignoring`` diagnostic, because
  systemd-repart exits 0 on an unknown key and would otherwise accept silent
  drift (REQ-CI-01);
* the transfer gate parses ``build/sysupdate.d`` offline and unprivileged and
  requires that both root slots are listed.

Each gate also runs its recorded negative and boundary cases from
``tests/systemd``. A case passes only when the observed exit code and
diagnostics match one of its recorded outcomes, so a systemd release that
changes an outcome is a failed gate rather than an inherited assumption.

A pass is development evidence on the reference profile. It closes no image,
boot, hardware or release gate.
"""

import argparse
import json
import os
import re
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

from host import target as posix_target

ROOT = Path(__file__).resolve().parent.parent
# Admitted floor, recorded in docs/roadmap/toolchain-admission.md. The gate
# refuses to report a pass from an older systemd, whose diagnostics and exit
# codes are not the ones the cases below were observed on. The floor is applied
# to each tool the gate runs, read from that binary's own --version, because a
# copy on PATH may be a different release from the systemd running as PID 1.
SYSTEMD_FLOOR = 261
REFERENCE_PROFILE_SYSTEMD = "systemd 261 (261.3-1-arch)"
# Fixed so two runs of the gate produce the same partition UUIDs and roothash.
SEED = "1d8b8b28-0b1f-4c1a-9a5f-2a0c3e4d5f60"
IMAGE_SIZE = "32G"
# /var is encrypted with Encrypt=tpm2, which binds to the PCR state of the
# machine that seals it. Enrolment therefore belongs on the target machine at
# first boot rather than in the build, so the partition is added to the table
# and left unpopulated. The un-deferred run in docs/build/definitions.md also
# failed to open /dev/tpmrm0; that was observed on 2026-09-13, before the
# developer account joined tss, and the deferral does not rest on it.
DEFER_TYPES = "var"
UNKNOWN_KEY = "Unknown key"
COMMAND_TIMEOUT = 600
MAX_CASES = 32
MAX_DIAGNOSTIC_LINES = 4096
# The two tools both gates run. Each is looked up on PATH first and then in
# SYSTEMD_LIBEXEC, and the gate prints which copy it used.
GATE_TOOLS = ("systemd-repart", "systemd-sysupdate")
# systemd's libexec directory, `systemdutildir` in systemd.pc. systemd 262 moved
# systemd-sysupdate "back from bindir to libexecdir" and names this path "the
# most compatible way to call it" (NEWS, CHANGES WITH 262); 261 had installed it
# in /usr/bin, and the systemd 255 on ubuntu-24.04 ships it only here. A copy on
# PATH still wins, so an operator's own build is the one that runs.
SYSTEMD_LIBEXEC = "/usr/lib/systemd"
# The --root= case reads a self-contained tree: the repart gate writes its
# scratch image into it under IMAGE_NAME, beside the staged transfers and the
# image identity. See stage_root_tree().
SYSROOT = "sysroot"
IMAGE_NAME = "positive.raw"
# systemd reports a specifier it cannot expand and then drops the setting, the
# same exit-0 drift as an unknown key, so a passing listing must not print it.
UNEXPANDED = "Failed to expand specifiers"
# The cases whose exit 0 must list both root slots rather than merely parse.
LISTING_CASES = ("sysupdate/positive", "sysupdate/root-tree")


class GateError(Exception):
    """A gate could not be run at all, as opposed to a case that failed."""


def parse_systemd_version(text):
    """Return the major systemd version in `text`, or None if there is none.

    Accepts what `systemctl --version`, `systemd-repart --version` and
    `systemd-sysupdate --version` print first, for example
    `systemd 261 (261.3-1-arch)` or a bare `systemd 255`.
    """
    match = re.match(r"^systemd\s+(\d{1,5})\b", text.strip())
    if match is None:
        return None
    return int(match.group(1))


def retarget_transfer(text, device):
    """Return `text` with the single `Path=auto` target line pointed at `device`.

    The reviewed transfer targets `auto`, the booted system's own block device.
    The gate must not open the workstation's root device, so exactly this one
    line is rewritten; anything else in the reviewed file reaches systemd
    unchanged. A file with no such line, or with more than one, is a refusal
    rather than a silent no-op.
    """
    lines = text.splitlines()
    hits = [index for index, line in enumerate(lines) if line.strip() == "Path=auto"]
    if len(hits) != 1:
        raise GateError(f"expected exactly one 'Path=auto' line, found {len(hits)}")
    lines[hits[0]] = f"Path={posix_target(device)}"
    return "\n".join(lines) + "\n"


def outcome(code, requires=(), forbids=()):
    """Build one accepted outcome: an exit code plus diagnostic constraints."""
    return {"code": code, "requires": tuple(requires), "forbids": tuple(forbids)}


def matches(observed_code, diagnostics, accepted):
    """Return True when an observed run matches one accepted outcome."""
    if observed_code != accepted["code"]:
        return False
    if any(needle not in diagnostics for needle in accepted["requires"]):
        return False
    return all(needle not in diagnostics for needle in accepted["forbids"])


def explain(accepted):
    """Render one accepted outcome for the summary table."""
    parts = [f"exit {accepted['code']}"]
    for needle in accepted["requires"]:
        parts.append(f"with {needle!r}")
    for needle in accepted["forbids"]:
        parts.append(f"without {needle!r}")
    return ", ".join(parts)


def run(argv, extra_env=None):
    """Run `argv` and return (exit code, stdout, stderr) with a hard deadline."""
    env = dict(os.environ)
    env["SYSTEMD_COLORS"] = "0"
    if extra_env:
        env.update(extra_env)
    try:
        done = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=COMMAND_TIMEOUT,
            env=env,
            check=False,
        )
    except (subprocess.TimeoutExpired, OSError) as error:
        raise GateError(f"{argv[0]} could not be run: {error}") from error
    return done.returncode, done.stdout, done.stderr


def resolve_tool(name):
    """Return (path, where) for the copy of `name` the gate runs, or (None, reason).

    Positive: a tool that is only in the systemd libexec directory is found
    there. Negative: a tool in neither place yields no path and a reason naming
    both places searched. Boundary: a copy on PATH wins over the libexec one.
    """
    on_path = shutil.which(name)
    if on_path is not None:
        return on_path, "PATH"
    in_libexec = shutil.which(name, path=SYSTEMD_LIBEXEC)
    if in_libexec is not None:
        return in_libexec, f"the systemd libexec directory {SYSTEMD_LIBEXEC}, not PATH"
    return None, f"{name} is neither on PATH nor in {SYSTEMD_LIBEXEC}"


def tool_version(binary):
    """Return (major, banner) that `binary --version` reports, or (None, reason).

    Positive: a systemd tool's banner yields its major. Negative: a binary that
    cannot be run, exits non-zero or prints no systemd banner yields None and a
    reason. Boundary: only the first line is read, as systemctl's is.
    """
    try:
        code, out, _ = run([binary, "--version"])
    except GateError as error:
        return None, str(error)
    first = out.splitlines()[0].strip() if out else ""
    version = parse_systemd_version(first) if code == 0 else None
    if version is None:
        return None, f"{binary} --version exited {code} and printed no systemd banner"
    return version, first


def floor_refusal(name, binary):
    """Return (banner, None) when the `binary` for `name` meets the floor, else (None, why)."""
    version, banner = tool_version(binary)
    if version is None:
        return None, f"{name} reports no systemd version ({banner})"
    if version < SYSTEMD_FLOOR:
        return None, (
            f"{name} at {binary} reports {banner!r}, below the admitted floor "
            f"systemd {SYSTEMD_FLOOR}"
        )
    return banner, None


def systemctl_banner():
    """Return what `systemctl --version` prints first, or '' when it cannot be read.

    Printed beside the tools' own banners as a cross-check only: the floor is
    applied to the binaries the gate runs, not to the systemd running as PID 1.
    """
    if shutil.which("systemctl") is None:
        return ""
    try:
        code, out, _ = run(["systemctl", "--version"])
    except GateError:
        return ""
    return out.splitlines()[0].strip() if code == 0 and out else ""


def repart_argv(definitions, image, root, binary="systemd-repart"):
    """Build the repart invocation every repart case runs, with `binary` as argv[0].

    `--dry-run=yes` is kept because M03's exit criteria name it, but it is
    inert here and this is not a dry run: under `--empty=create` on systemd
    261 the run formats the filesystems and writes a real GPT into the
    scratch image, the same table `--dry-run=no` produces, while
    systemd-repart(8) says a dry run does not touch the partition table.
    docs/build/definitions.md records the observation. The image is a sparse
    file inside a TemporaryDirectory that is removed when the gate exits, so
    the divergence costs a throwaway image rather than a real device.
    """
    return [
        binary,
        "--empty=create",
        f"--size={IMAGE_SIZE}",
        # Inert under --empty=create on systemd 261; see this function's docstring.
        "--dry-run=yes",
        "--offline=yes",
        f"--defer-partitions={DEFER_TYPES}",
        f"--seed={SEED}",
        f"--root={posix_target(root)}",
        f"--definitions={posix_target(definitions)}",
        posix_target(image),
    ]


def scratch_root(base):
    """Create the scratch root tree repart copies from and reads identity from."""
    root = base / "root"
    (root / "boot").mkdir(parents=True, exist_ok=True)
    (root / "etc").mkdir(parents=True, exist_ok=True)
    (root / "etc" / "machine-id").write_text("ffffffffffffffffffffffffffffffff\n")
    return root


def os_release(base):
    """Write the image identity the transfer's ProtectVersion=%A expands from."""
    path = base / "os-release"
    path.write_text(
        'ID=aegis\nNAME="Aegis OS"\nVERSION_ID=1.0.0\n' "IMAGE_ID=aegis-os\nIMAGE_VERSION=a\n"
    )
    return path


def repart_cases(base, root, positive_image, binary="systemd-repart"):
    """Return the repart gate's positive, negative and boundary cases.

    The positive case keeps its scratch image: the transfer gate points the
    reviewed transfer at it, so sysupdate reads the partition table these
    definitions actually produced rather than a second, hand-written one.
    """
    fixtures = ROOT / "tests" / "systemd"
    rows = [
        ("repart/positive", ROOT / "build" / "repart.d", [outcome(0, forbids=[UNKNOWN_KEY])]),
        (
            "repart/negative-missing-type",
            fixtures / "repart-missing-type",
            [outcome(1, requires=["Type= not defined, refusing."])],
        ),
        (
            "repart/negative-inverted-size",
            fixtures / "repart-inverted-size",
            [outcome(1, requires=["SizeMinBytes= larger than SizeMaxBytes=, refusing."])],
        ),
        (
            "repart/negative-unknown-key",
            fixtures / "repart-unknown-key",
            [outcome(0, requires=[UNKNOWN_KEY, "'Subsystem'"])],
        ),
        (
            "repart/boundary-equal-size",
            fixtures / "repart-equal-size",
            [outcome(0, forbids=[UNKNOWN_KEY])],
        ),
    ]
    cases = []
    for index, (name, definitions, accepted) in enumerate(rows[:MAX_CASES]):
        keep = name == "repart/positive"
        image = positive_image if keep else base / f"scratch-{index}.raw"
        cases.append(
            {
                "name": name,
                "argv": repart_argv(definitions, image, root, binary),
                "accepted": accepted,
                "env": None,
                "image": None if keep else image,
            }
        )
    return cases


def sysupdate_argv(definitions=None, root=None, binary="systemd-sysupdate"):
    """Build the offline, unprivileged transfer listing invocation, `binary` first."""
    argv = [binary]
    argv.append(
        f"--root={posix_target(root)}"
        if definitions is None
        else f"--definitions={posix_target(definitions)}"
    )
    argv.extend(["--offline", "--no-pager", "--json=short", "list"])
    return argv


def write_transfers(directory, source, target):
    """Copy `source`'s transfers into `directory`, each `Path=auto` pointed at `target`.

    Every `Path=auto` line is retargeted at the scratch image the repart gate
    built, because an unprivileged gate must not open the workstation's own
    root block device. Nothing else in a staged file is rewritten.
    """
    directory.mkdir(parents=True, exist_ok=True)
    names = sorted(entry.name for entry in source.iterdir() if entry.suffix == ".transfer")
    for name in names[:MAX_CASES]:
        text = (source / name).read_text()
        if "Path=auto" in text:
            text = retarget_transfer(text, target)
        (directory / name).write_text(text)
    return directory


def stage_transfers(base, label, source, image):
    """Copy `source`'s transfers into a scratch usr/lib/sysupdate.d tree."""
    tree = base / label
    return tree, write_transfers(tree / "usr" / "lib" / "sysupdate.d", source, image)


def stage_root_tree(tree, source, image_name, identity):
    """Stage `source`'s transfers as a self-contained root tree for `--root=`.

    systemd 262 reads a transfer through `--root=` relative to the tree and
    resolves its `Path=` inside the tree as well: a target outside it ends on
    `Failed to resolve ...: No such file or directory`. So the scratch image
    lives in this tree, `Path=auto` names it as the tree itself does
    (`/<image_name>`), and the identity `ProtectVersion=%A` expands from is the
    tree's own usr/lib/os-release.
    """
    write_transfers(tree / "usr" / "lib" / "sysupdate.d", source, f"/{image_name}")
    (tree / "usr" / "lib" / "os-release").write_text(identity.read_text())
    return tree


def transfer_case(name, argv, accepted, env):
    """Build one transfer case row."""
    return {"name": name, "argv": argv, "accepted": accepted, "env": env, "image": None}


def sysupdate_cases(base, image, identity, binary="systemd-sysupdate"):
    """Return the transfer gate's positive, negative and boundary cases."""
    fixtures = ROOT / "tests" / "systemd"
    _, reviewed = stage_transfers(base, "reviewed", ROOT / "build" / "sysupdate.d", image)
    _, unsupported = stage_transfers(
        base, "unsupported", fixtures / "sysupdate-unsupported-keys", image
    )
    _, single = stage_transfers(base, "single-slot", fixtures / "sysupdate-single-slot", image)
    env = {"SYSTEMD_OS_RELEASE": str(identity)}
    return [
        transfer_case(
            "sysupdate/positive",
            sysupdate_argv(definitions=reviewed, binary=binary),
            [outcome(0, forbids=[UNKNOWN_KEY, UNEXPANDED])],
            env,
        ),
        transfer_case(
            "sysupdate/negative-unsupported-keys",
            sysupdate_argv(definitions=unsupported, binary=binary),
            [outcome(1, requires=[UNKNOWN_KEY, "Source specification lacks MatchPattern=."])],
            env,
        ),
        transfer_case(
            "sysupdate/boundary-single-slot",
            sysupdate_argv(definitions=single, binary=binary),
            [outcome(0, requires=["InstancesMax= value must be at least 2"])],
            env,
        ),
        root_tree_case(image, identity, binary),
    ]


def root_tree_case(image, identity, binary="systemd-sysupdate"):
    """The invocation M03 names in its exit criteria, over a self-contained tree.

    Two outcomes are accepted and any third fails the gate. systemd 261 reads
    no transfer at all through --root=: it finds the file under the tree and
    then opens the root-prefixed path a second time, so every key is unset and
    the run ends on the recorded Source Type= refusal. Proof rather than
    inference: a tree whose outer file is not a config at all still exits 0
    when a valid copy sits at <tree><tree>/usr/lib/sysupdate.d. systemd 262
    reads the tree as a root and lists both slots, which is why the image and
    the identity live inside it (stage_root_tree()). SYSTEMD_OS_RELEASE is not
    passed: under --root= on 262 it leaves ProtectVersion=%A unexpanded, and
    systemd reports that and ignores the setting.
    """
    tree = stage_root_tree(image.parent, ROOT / "build" / "sysupdate.d", image.name, identity)
    return transfer_case(
        "sysupdate/root-tree",
        sysupdate_argv(root=tree, binary=binary),
        [
            outcome(0, forbids=[UNKNOWN_KEY, UNEXPANDED]),
            outcome(1, requires=["Source Type= must be one of"]),
        ],
        None,
    )


def listed_versions(stdout):
    """Return the versions a --json=short listing reports, from its last JSON line."""
    for line in reversed(stdout.splitlines()[:MAX_DIAGNOSTIC_LINES]):
        text = line.strip()
        if not text.startswith("{"):
            continue
        try:
            return set(json.loads(text).get("all", []))
        except (ValueError, AttributeError):
            return set()
    return set()


def positive_listing(case, stdout):
    """A listing case that exits 0 must list both root slots, not merely parse.

    Parsing alone would pass on a transfer that matches no partition at all.
    The slots come from the scratch image the repart gate just built, so this
    also ties the two halves of the milestone together: the labels
    ``build/repart.d`` writes are the instances ``build/sysupdate.d`` finds.
    The caller applies it to an exit-0 run only, because the --root= case may
    also end on its recorded refusal, which lists nothing.
    """
    if case["name"] not in LISTING_CASES:
        return []
    missing = sorted({"a", "b"} - listed_versions(stdout))
    if missing:
        return [f"listing does not report root slot(s) {missing}"]
    return []


def execute(case):
    """Run one case and return (passed, exit code, report lines)."""
    code, stdout, stderr = run(case["argv"], case["env"])
    diagnostics = "\n".join((stderr + "\n" + stdout).splitlines()[:MAX_DIAGNOSTIC_LINES])
    passed = any(matches(code, diagnostics, accepted) for accepted in case["accepted"])
    problems = [] if passed else [f"expected {' | '.join(explain(a) for a in case['accepted'])}"]
    if code == 0:
        problems.extend(positive_listing(case, stdout))
    if case["image"] is not None and case["image"].exists():
        case["image"].unlink()
    return (not problems), code, problems, diagnostics


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


def resolve_tools():
    """Return ({name: path}, reasons): every gate tool found, and why any is absent.

    Each copy found is printed with where it came from, so the evidence names
    the binary that actually ran rather than the one PATH would have offered.
    """
    tools, missing = {}, []
    for name in GATE_TOOLS:
        path, where = resolve_tool(name)
        if path is None:
            missing.append(where)
            continue
        tools[name] = path
        print(f"{name}: {path} (from {where})")
    return tools, missing


def guard():
    """Return (banners, tools) when the gate may run, or None after saying why.

    Every resolved tool is held to the floor by its own --version, so a PATH
    copy older than the floor is refused even when systemctl reports a newer
    systemd, and the banners returned are those of the binaries that run.
    """
    tools, missing = resolve_tools()
    if missing:
        print(f"SKIP: {'; '.join(missing)}; the systemd definition gate did not run.")
        return None
    banners, refusals = {}, []
    for name in GATE_TOOLS:
        banner, refusal = floor_refusal(name, tools[name])
        if refusal is not None:
            refusals.append(refusal)
        banners[name] = banner
    if refusals:
        print(
            f"SKIP: {'; '.join(refusals)}; the systemd definition gate did not run. "
            f"The recorded reference profile is {REFERENCE_PROFILE_SYSTEMD!r}."
        )
        return None
    return banners, tools


def describe(banners):
    """Render the banners of the tools that run, one clause per tool."""
    return " and ".join(f"{name} {banners[name]!r}" for name in GATE_TOOLS)


def run_cases(base, tools):
    """Run every case with the resolved `tools` and return the number that failed."""
    root = scratch_root(base)
    identity = os_release(base)
    positive_image = base / SYSROOT / IMAGE_NAME
    positive_image.parent.mkdir(parents=True, exist_ok=True)
    cases = repart_cases(base, root, positive_image, tools["systemd-repart"])
    cases.extend(sysupdate_cases(base, positive_image, identity, tools["systemd-sysupdate"]))
    failed = 0
    for case in cases[:MAX_CASES]:
        passed, code, problems, diagnostics = execute(case)
        report(case, code, problems, diagnostics)
        failed += 0 if passed else 1
    positive_image.unlink(missing_ok=True)
    return failed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    admitted = guard()
    if admitted is None:
        return 0
    banners, tools = admitted
    print(f"systemd definition gate on {describe(banners)} (floor systemd {SYSTEMD_FLOOR}).")
    cross_check = systemctl_banner() or "no banner"
    print(f"systemctl reports {cross_check!r}: a cross-check only; the floor applies to the tools.")
    try:
        with tempfile.TemporaryDirectory(prefix="aegis-defs-") as base:
            failed = run_cases(Path(base), tools)
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    if failed:
        print(f"FAIL: {failed} systemd definition case(s) did not match their recorded outcome.")
        return 1
    print(
        "PASS: repart and sysupdate definition gates on the reference profile; "
        "development evidence only, no image, boot, hardware or release gate is closed."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
