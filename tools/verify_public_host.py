#!/usr/bin/env python3
"""Refuse host identity and host state in public text: tracked files and pull request text.

Aegis is developed against a reference workstation, but the public tree and the
forge text describe a hardware class, never that machine. This gate refuses:

- host-name: the running host's name and every name in the private deny-hosts
  file passed with ``--deny-hosts`` or ``AEGIS_DENY_HOSTS`` (ADR-0005; the file
  is never committed here, and a refusal never prints its entry);
- pci-address: a PCI bus/device/function address outside the declared fixture
  range;
- iommu-group: a VFIO or IOMMU group number outside the declared fixture range;
- process-id: a process id attached to a pid label, a holder or a compositor;
- epoch-time: a ten-digit Unix time between 2020 and 2033, such as a process
  start or boot time;
- home-path: a home folder that names a user;
- host-hardware: CPU model names, firmware vendor strings and device counts of
  one machine (a GPU model next to a measurement stays allowed);
- session-state: a desktop session hanging, rebooting or waiting for a login.

The last four rules bind prose (Markdown, ``planning/``, ``docs/`` and pull
request text); the others bind every tracked text file.

The allow-list has the ADR-0005 shape: synthetic fixture values are declared
below by shape, there is no per-file opt-out, and a legitimate value that a
rule refuses is fixed by a reviewed change to that shape, never by a bypass.

Usage::

    python3 tools/verify_public_host.py               # every tracked text file
    python3 tools/verify_public_host.py --text-file F  # one text, e.g. a PR body

``tools/test_verify_public_host.py`` plants one defect per rule; it builds every
planted value from fragments so that this gate never refuses its own tests.
"""

import argparse
import os
import pathlib
import re
import socket
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
MAX_FILE_BYTES = 4 << 20
MAX_FILES = 20000
GIT_TIMEOUT_SECONDS = 60
MIN_HOST_NAME = 4

# Fixture shapes. Tests and docs that need an address or group number use these,
# so that no example can be read as the reference workstation's topology.
FIXTURE_PCI_BUS = re.compile(r"^(?:0000:)?f[0-9a-f]:", re.IGNORECASE)
FIXTURE_GROUP_MIN = 900
GENERIC_HOME_USERS = frozenset({"user", "someone", "runner", "runneradmin", "builder"})
# Prose is what a reader takes as a statement about a machine: Markdown, the
# planning records and docs/, and every pull request text. Code fixtures keep
# synthetic pids, times and group numbers; the identity rules bind every file.
PROSE_PREFIXES = ("planning/", "docs/")
PROSE_SUFFIXES = (".md",)

PCI_RE = re.compile(r"\b(?:[0-9a-f]{4}:)?[0-9a-f]{2}:[0-1][0-9a-f]\.[0-7]\b", re.IGNORECASE)
GROUP_RE = re.compile(
    r"(?:/dev/vfio/|iommu_groups/|\bIOMMU group |\bvfio group )(\d+)", re.IGNORECASE
)
PID_RE = re.compile(
    r"\bpid[ =:]+\d{3,}"
    r"|\bheld by \d{2,}"
    r"|\b(?:kwin_wayland|kwin|Xwayland|Xorg|plasmashell|gnome-shell|sway) \(?(?:pid )?\d{3,}",
    re.IGNORECASE,
)
EPOCH_RE = re.compile(r"(?<![\w.:-])1[5-9]\d{8}(?![\w.:-])")
EPOCH_MIN, EPOCH_MAX = 1577836800, 2000000000
HOME_RE = re.compile(r"/home/([a-z_][a-z0-9_.-]*)|/Users/([A-Za-z0-9_.-]+)|C:\\+Users\\+(\w+)")
HARDWARE_RE = re.compile(
    r"\bRyzen \d|\b\d{4}X3D\b|\bThreadripper\b|\bEPYC \d|\bCore i[3579]-\d"
    r"|\bXeon\b|\bAmerican Megatrends\b|\b\w+ NVMe (?:controllers|drives)\b|\b\d+ cores and\b",
    re.IGNORECASE,
)
SESSION_RE = re.compile(
    r"\bhung the (?:session|desktop)|\buntil a reboot\b|\b(?:session|desktop) (?:hung|froze)\b"
    r"|\bnext login\b|\blog out and (?:back )?in\b",
    re.IGNORECASE,
)


def _host_name_rules(deny_hosts):
    names = {name.strip().lower() for name in deny_hosts if name.strip()}
    own = socket.gethostname().split(".")[0].strip().lower()
    if own and own != "localhost":
        names.add(own)
    names = {name for name in names if len(name) >= MIN_HOST_NAME}
    # A substring match: a host name inside an identifier (a ledger id, a branch,
    # a repository name) names the host as plainly as the bare word does.
    return [re.compile(re.escape(name), re.IGNORECASE) for name in names]


def _pci_findings(text):
    return [m.group(0) for m in PCI_RE.finditer(text) if not FIXTURE_PCI_BUS.match(m.group(0))]


def _group_findings(text):
    return [m.group(0) for m in GROUP_RE.finditer(text) if int(m.group(1)) < FIXTURE_GROUP_MIN]


def _epoch_findings(text):
    return [m.group(0) for m in EPOCH_RE.finditer(text) if EPOCH_MIN <= int(m.group(0)) < EPOCH_MAX]


def _home_findings(text):
    found = []
    for m in HOME_RE.finditer(text):
        user = next(group for group in m.groups() if group is not None)
        if user.lower() not in GENERIC_HOME_USERS:
            found.append(m.group(0))
    return found


def scan_text(text, host_rules, prose=True):
    """Return (rule, matched text) pairs; host-name matches never echo the entry."""
    findings = []
    for rule in host_rules:
        findings += [("host-name", "<deny-hosts entry>") for _ in rule.finditer(text)]
    findings += [("pci-address", v) for v in _pci_findings(text)]
    findings += [("home-path", v) for v in _home_findings(text)]
    findings += [("host-hardware", m.group(0)) for m in HARDWARE_RE.finditer(text)]
    if prose:
        findings += [("iommu-group", v) for v in _group_findings(text)]
        findings += [("process-id", m.group(0)) for m in PID_RE.finditer(text)]
        findings += [("epoch-time", v) for v in _epoch_findings(text)]
        findings += [("session-state", m.group(0)) for m in SESSION_RE.finditer(text)]
    return findings


def is_prose(name):
    return name.startswith(PROSE_PREFIXES) or name.endswith(PROSE_SUFFIXES)


# The variables `git rev-parse --local-env-vars` lists. A commit hook exports some
# of them for its own repository; git run with them would read that repository's
# index instead of the one at --root.
GIT_LOCAL_ENV = frozenset(
    {
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_CONFIG",
        "GIT_CONFIG_PARAMETERS",
        "GIT_CONFIG_COUNT",
        "GIT_OBJECT_DIRECTORY",
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_IMPLICIT_WORK_TREE",
        "GIT_GRAFT_FILE",
        "GIT_INDEX_FILE",
        "GIT_NO_REPLACE_OBJECTS",
        "GIT_REPLACE_REF_BASE",
        "GIT_PREFIX",
        "GIT_SHALLOW_FILE",
        "GIT_COMMON_DIR",
    }
)


def git_env():
    """The environment without a hook's repository-local git variables."""
    return {key: value for key, value in os.environ.items() if key not in GIT_LOCAL_ENV}


def tracked_files(root):
    out = subprocess.run(
        ["git", "-C", str(root), "ls-files", "-z"],
        capture_output=True,
        timeout=GIT_TIMEOUT_SECONDS,
        check=True,
        env=git_env(),
    ).stdout
    return [name for name in out.decode().split("\0") if name][:MAX_FILES]


def read_text(path):
    if not path.is_file() or path.is_symlink() or path.stat().st_size > MAX_FILE_BYTES:
        return None
    try:
        return path.read_text(encoding="utf-8")
    except UnicodeDecodeError:
        return None


def scan_tree(root, host_rules):
    report = []
    for name in tracked_files(root):
        text = read_text(root / name)
        if text is None:
            continue
        prose = is_prose(name)
        for line_no, line in enumerate(text.splitlines(), 1):
            for rule, value in scan_text(line, host_rules, prose):
                report.append(f"{name}:{line_no}: {rule}: {value}")
    return report


def load_deny_hosts(path):
    if path is None:
        return []
    lines = pathlib.Path(path).read_text(encoding="utf-8").splitlines()
    return [line for line in lines if not line.startswith("#")]


def main(argv=None):
    parser = argparse.ArgumentParser(description="Refuse host identity and state in public text.")
    parser.add_argument("--root", type=pathlib.Path, default=ROOT)
    parser.add_argument("--text-file", type=pathlib.Path)
    parser.add_argument("--deny-hosts", default=os.environ.get("AEGIS_DENY_HOSTS"))
    args = parser.parse_args(argv)
    host_rules = _host_name_rules(load_deny_hosts(args.deny_hosts))
    if args.text_file is not None:
        text = args.text_file.read_text(encoding="utf-8")
        findings = scan_text(text, host_rules)
        report = [f"{args.text_file.name}: {rule}: {value}" for rule, value in findings]
        scope = "text"
    else:
        report = scan_tree(args.root, host_rules)
        scope = "tracked files"
    for line in report:
        print(f"FAIL: {line}")
    if report:
        print(f"FAIL: {len(report)} host identifier(s) in {scope}; name the hardware class instead")
        return 1
    print(f"PASS: no host identifier in {scope} ({len(host_rules)} host-name rule(s))")
    return 0


if __name__ == "__main__":
    sys.exit(main())
