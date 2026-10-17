#!/usr/bin/env python3
"""Hold the crates pinned by git revision to their rows in the pin register.

A crate taken from a git revision has no crates.io release to look up, and
Renovate's dashboard reports new commits without a reason. The register in
``docs/roadmap/toolchain-admission.md`` ("Crates pinned by git revision")
gives each such crate its source, full commit, reason and review trigger
(issue #224). This check reads the table, the root ``Cargo.toml``, every
member manifest and ``Cargo.lock``, and fails when any of them disagree, so
a revision moved without its row, a branch or tag pin, a member's private
git dependency or a lock source that names another commit all stop
``make verify-all``. A second, optional table ("Git sources a pinned crate
brings in") lists the git packages a pinned crate pulls into the lock, each
held to the exact lock source its row names.

Usage: ``python3 tools/verify_crate_pins.py``. ``tools/test_verify_crate_pins.py``
covers each refusal with a planted case.
"""

import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
REGISTER = "docs/roadmap/toolchain-admission.md"
HEADING = "## Crates pinned by git revision"
TABLE_HEADER = "| Crate | Source | Commit | Reason | Review trigger |"
ROW = re.compile(
    r"^\| `([A-Za-z0-9_-]+)` \| `(https://[^`\s]+)` \| `([0-9a-f]{40})` "
    r"\| ([^|]*) \| ([^|]*) \|$"
)
CARRIED_HEADING = "## Git sources a pinned crate brings in"
CARRIED_HEADER = "| Source | Commit | Carried by | Packages | Review trigger |"
CARRIED_ROW = re.compile(
    r"^\| `(https://[^`\s]+)` \| `([0-9a-f]{40})` \| `([A-Za-z0-9_-]+)` "
    r"\| ((?:`[A-Za-z0-9_-]+`(?:, )?)+) \| ([^|]*) \|$"
)
PACKAGE = re.compile(r"`([A-Za-z0-9_-]+)`")
FULL_REV = re.compile(r"^[0-9a-f]{40}$")
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")


class PinError(Exception):
    """The register, a manifest or the lock cannot be read as expected."""


def register_section(text, heading=HEADING, required=True):
    """Return the lines between `heading` and the next heading.

    A section that is not `required` may be absent, which reads as empty.
    """
    lines = text.splitlines()
    starts = [i for i, line in enumerate(lines) if line == heading]
    if not starts and not required:
        return []
    if len(starts) != 1:
        raise PinError(f"{REGISTER}: heading {heading!r} appears {len(starts)} times")
    section = []
    for line in lines[starts[0] + 1 :]:
        if line.startswith("## "):
            break
        section.append(line)
    return section


def register_rows(text):
    """Map each registered crate to (source, commit, reason, trigger)."""
    rows = {}
    table = [line for line in register_section(text) if line.startswith("|")]
    if not table or table[0] != TABLE_HEADER:
        raise PinError(f"{REGISTER}: the register table header is not {TABLE_HEADER!r}")
    for line in table[2:]:
        match = ROW.match(line)
        if match is None:
            raise PinError(f"{REGISTER}: unreadable register row: {line}")
        name, source, commit, reason, trigger = match.groups()
        if name in rows:
            raise PinError(f"{REGISTER}: crate {name} has two rows")
        rows[name] = (source, commit, reason.strip(), trigger.strip())
    return rows


def carried_rows(text):
    """Map each package a carried source lists to (source, commit, carrier, trigger)."""
    carried = {}
    section = register_section(text, CARRIED_HEADING, required=False)
    table = [line for line in section if line.startswith("|")]
    if section and (not table or table[0] != CARRIED_HEADER):
        raise PinError(f"{REGISTER}: the carried-source table header is not {CARRIED_HEADER!r}")
    for line in table[2:]:
        match = CARRIED_ROW.match(line)
        if match is None:
            raise PinError(f"{REGISTER}: unreadable carried-source row: {line}")
        source, commit, carrier, packages, trigger = match.groups()
        for name in PACKAGE.findall(packages):
            if name in carried:
                raise PinError(f"{REGISTER}: package {name} is listed twice")
            carried[name] = (source, commit, carrier, trigger.strip())
    return carried


def git_pin(where, name, spec):
    """Return (source, rev) of one git dependency, refusing a moving pin."""
    for moving in ("branch", "tag"):
        if moving in spec:
            raise PinError(f"{where}: {name} is pinned by {moving}, not by a full rev")
    rev = spec.get("rev", "")
    if not FULL_REV.match(rev):
        raise PinError(f"{where}: {name} rev {rev!r} is not a full 40-character commit")
    return spec["git"], rev


def workspace_pins(manifest):
    """Map each git dependency of [workspace.dependencies] to (source, rev)."""
    deps = manifest.get("workspace", {}).get("dependencies", {})
    pins = {}
    for name, spec in deps.items():
        if isinstance(spec, dict) and "git" in spec:
            pins[name] = git_pin("Cargo.toml [workspace.dependencies]", name, spec)
    return pins


def patch_git_entries(manifest):
    """Name every git source a [patch] table substitutes, which no row covers."""
    found = []
    for registry, entries in manifest.get("patch", {}).items():
        for name, spec in entries.items():
            if isinstance(spec, dict) and "git" in spec:
                found.append(f"Cargo.toml [patch.{registry}] {name}")
    return found


def dependency_tables(member):
    """Yield (label, table) for every dependency table of a member manifest."""
    for key in DEPENDENCY_TABLES:
        yield key, member.get(key, {})
    for target, tables in member.get("target", {}).items():
        for key in DEPENDENCY_TABLES:
            yield f"target.{target}.{key}", tables.get(key, {})


def member_git_deps(path, member):
    """Name every dependency a member manifest takes from git itself."""
    found = []
    for label, table in dependency_tables(member):
        for name, spec in table.items():
            if isinstance(spec, dict) and "git" in spec:
                found.append(f"{path} [{label}] {name}")
    return found


def lock_git_sources(lock):
    """Map each locked package with a git source to its source string."""
    sources = {}
    for package in lock.get("package", []):
        source = package.get("source", "")
        if source.startswith("git+"):
            sources[package["name"]] = source
    return sources


def compare_rows(pins, rows):
    """Report crates missing from the register, stale rows and drifted pins."""
    problems = []
    for name in sorted(set(pins) - set(rows)):
        problems.append(f"{name}: pinned by git in Cargo.toml but has no register row")
    for name in sorted(set(rows) - set(pins)):
        problems.append(f"{name}: has a register row but is not a git dependency")
    for name in sorted(set(pins) & set(rows)):
        source, rev = pins[name]
        row_source, row_commit, reason, trigger = rows[name]
        if source != row_source:
            problems.append(f"{name}: Cargo.toml source {source} != register {row_source}")
        if rev != row_commit:
            problems.append(f"{name}: Cargo.toml rev {rev} != register {row_commit}")
        if not reason or not trigger:
            problems.append(f"{name}: register row has an empty reason or review trigger")
    return problems


def compare_carried(pins, carried, sources):
    """Report carried packages whose carrier, trigger or lock source disagrees."""
    problems = []
    for name in sorted(carried):
        source, commit, carrier, trigger = carried[name]
        if carrier not in pins:
            problems.append(f"{name}: carried by {carrier}, which is not a git dependency")
        if name in pins:
            problems.append(f"{name}: pinned in Cargo.toml and also listed as carried")
        if not trigger:
            problems.append(f"{name}: carried-source row has an empty review trigger")
        expected = f"git+{source}?rev={commit}#{commit}"
        actual = sources.get(name)
        if actual != expected:
            problems.append(f"{name}: Cargo.lock source {actual} != {expected}")
    return problems


def compare_lock(pins, sources, carried=()):
    """Report lock sources that are not exactly the pinned source and rev."""
    problems = []
    for name in sorted(set(sources) - set(pins) - set(carried)):
        problems.append(f"{name}: Cargo.lock takes it from git but no row covers it")
    for name in sorted(pins):
        source, rev = pins[name]
        expected = f"git+{source}?rev={rev}#{rev}"
        actual = sources.get(name)
        if actual != expected:
            problems.append(f"{name}: Cargo.lock source {actual} != {expected}")
    return problems


def check(root):
    """Return every disagreement between register, manifests and lock."""
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    text = (root / REGISTER).read_text(encoding="utf-8")
    rows = register_rows(text)
    carried = carried_rows(text)
    pins = workspace_pins(manifest)
    problems = compare_rows(pins, rows)
    problems += [
        f"{entry}: git source outside the register" for entry in patch_git_entries(manifest)
    ]
    for member in manifest.get("workspace", {}).get("members", []):
        path = f"{member}/Cargo.toml"
        data = tomllib.loads((root / path).read_text(encoding="utf-8"))
        for entry in member_git_deps(path, data):
            problems.append(f"{entry}: git dependency outside [workspace.dependencies]")
    lock = tomllib.loads((root / "Cargo.lock").read_text(encoding="utf-8"))
    sources = lock_git_sources(lock)
    problems += compare_lock(pins, sources, carried)
    problems += compare_carried(pins, carried, sources)
    return problems, len(pins)


def main(argv):
    """Run the check on the repository and report PASS or each FAIL line."""
    root = pathlib.Path(argv[1]) if len(argv) > 1 else ROOT
    try:
        problems, count = check(root)
    except (OSError, PinError, tomllib.TOMLDecodeError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(f"PASS: {count} crates pinned by git revision match their register rows and Cargo.lock.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
