#!/usr/bin/env python3
"""Hold each component's cut class to the code it has in the tree (issue #221).

The cut class says what a build without the operating system would keep of a
component: ``standalone`` (every part is free of the operating system),
``core-and-adapter`` (operating-system-free parts plus parts bound to it) or
``os-only`` (every part is bound to it). ``planning/components.json`` records
it on each component as ``cut``: the class and the parts, each a tracked path
with its ``ties``, ``os-free`` or ``os-bound``.

The ties are measured, not judged:

- a workspace crate is ``os-bound`` when one of its normal, build or target
  dependencies is neither in ``OS_FREE_DEPENDENCIES`` nor an ``os-free``
  workspace crate, and ``os-free`` otherwise; dev-dependencies do not ship;
- a ``bpf/*.bpf.c`` program is ``os-bound``: the kernel loads it;
- a ``ui/`` directory is ``os-free``: it holds design tokens a browser engine
  reads, and no code that calls the operating system.

The class covers the code in the tree only. A daemon a component still plans
(the P01 image builder, the P02 slot daemon) is no part until its code lands.

The check fails when a recorded tie or class differs from the measured one,
when a part path is of no known kind or does not exist, and when a workspace
crate, a BPF program or a ``ui/`` directory is the part of no component or of
more than one.

Usage: ``python3 tools/verify_cut_classes.py [--print] [ROOT]``.
``--print`` adds one line per component for ``make readiness``.
``tools/test_verify_cut_classes.py`` covers each refusal with a planted case.
"""

import json
import pathlib
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
COMPONENTS = "planning/components.json"
FIELD = "cut"
CLASSES = ("standalone", "core-and-adapter", "os-only")
TIES = ("os-free", "os-bound")
PART_KEYS = {"path", "ties"}
# External crates a part may depend on and stay os-free, each read at its pin:
# none of them opens a file, socket, device or process.
OS_FREE_DEPENDENCIES = {
    "accesskit",  # accessibility tree types; the platform adapters are other crates
    "base16ct",  # constant-time hex encoding
    "saeptum-capsule",  # capsule descriptors only; saeptum-microvm holds the sockets
    "schemars",  # JSON schema derivation
    "serde",
    "serde_json",
    "sha2",
    "thiserror",
    "tornus-geometry",  # geometry arithmetic
}
SHIPPED_TABLES = ("dependencies", "build-dependencies")


class CutError(Exception):
    """A manifest or planning file cannot be read as expected."""


def shipped_dependencies(manifest):
    """Return the package names a member ships with, renames resolved."""
    tables = [manifest.get(key, {}) for key in SHIPPED_TABLES]
    for target in manifest.get("target", {}).values():
        tables += [target.get(key, {}) for key in SHIPPED_TABLES]
    names = set()
    for table in tables:
        for key, spec in table.items():
            names.add(spec.get("package", key) if isinstance(spec, dict) else key)
    return names


def workspace_crates(root):
    """Map each member path to its package name and shipped dependencies."""
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    crates = {}
    for member in manifest.get("workspace", {}).get("members", []):
        data = tomllib.loads((root / member / "Cargo.toml").read_text(encoding="utf-8"))
        name = data.get("package", {}).get("name")
        if not name:
            raise CutError(f"{member}/Cargo.toml: no package name")
        crates[member] = (name, shipped_dependencies(data))
    return crates


def crate_ties(crates):
    """Measure each member's ties; os-bound spreads to the members depending on it."""
    names = {name: member for member, (name, _) in crates.items()}
    bound = {
        member
        for member, (_, deps) in crates.items()
        if any(dep not in names and dep not in OS_FREE_DEPENDENCIES for dep in deps)
    }
    for _ in range(len(crates)):
        spread = {
            member
            for member, (_, deps) in crates.items()
            if any(names.get(dep) in bound for dep in deps)
        }
        if spread <= bound:
            break
        bound |= spread
    return {member: TIES[1] if member in bound else TIES[0] for member in crates}


def tree_parts(root, crates):
    """Return every path that must be a part, with its measured ties."""
    parts = crate_ties(crates)
    for program in sorted((root / "bpf").glob("*.bpf.c")):
        parts[f"bpf/{program.name}"] = TIES[1]
    ui = root / "ui"
    if ui.is_dir():
        for entry in sorted(ui.iterdir()):
            if entry.is_dir():
                parts[f"ui/{entry.name}"] = TIES[0]
    return parts


def derived_class(ties):
    """Return the class a list of part ties gives."""
    if all(tie == TIES[0] for tie in ties):
        return CLASSES[0]
    if all(tie == TIES[1] for tie in ties):
        return CLASSES[2]
    return CLASSES[1]


def record_problems(row, measured):
    """Report what is wrong with one component's cut record; return its paths."""
    where = f"{row['id']} {FIELD}"
    record = row.get(FIELD)
    if not isinstance(record, dict) or set(record) != {"class", "parts"}:
        return [f"{where}: must carry exactly ['class', 'parts']"], []
    parts = record["parts"]
    if not isinstance(parts, list) or not parts:
        return [f"{where}: parts must be a non-empty list"], []
    problems, paths, ties = [], [], []
    for index, part in enumerate(parts):
        if not isinstance(part, dict) or set(part) != PART_KEYS:
            problems.append(f"{where}.parts[{index}]: must carry exactly {sorted(PART_KEYS)}")
            continue
        path, tie = part["path"], part["ties"]
        paths.append(path)
        ties.append(tie)
        if path not in measured:
            problems.append(f"{where}: {path} is no workspace crate, BPF program or ui directory")
        elif tie != measured[path]:
            problems.append(f"{where}: {path} records {tie}, measured {measured[path]}")
    if record["class"] not in CLASSES:
        problems.append(f"{where}: class {record['class']!r} is not one of {list(CLASSES)}")
    elif not problems and record["class"] != derived_class(ties):
        problems.append(
            f"{where}: class {record['class']} recorded, its parts give {derived_class(ties)}"
        )
    return problems, paths


def check(root):
    """Return every problem and the component rows."""
    rows = json.loads((root / COMPONENTS).read_text(encoding="utf-8"))["components"]
    measured = tree_parts(root, workspace_crates(root))
    problems, owners = [], {}
    for row in rows:
        found, paths = record_problems(row, measured)
        problems += found
        for path in paths:
            owners.setdefault(path, []).append(row["id"])
    for path in sorted(measured):
        if len(owners.get(path, [])) != 1:
            problems.append(
                f"{path}: must be a part of exactly one component, found {owners.get(path, [])}"
            )
    return problems, rows


def readiness_lines(rows):
    """Return one line per component: class and parts."""
    lines = []
    for row in rows:
        record = row[FIELD]
        parts = ", ".join(f"{part['path']} ({part['ties']})" for part in record["parts"])
        lines.append(f"{row['id']} {row['name']} cut: {record['class']}; {parts}")
    return lines


def main(argv):
    """Run the check and report PASS, each FAIL line, and with --print the classes."""
    args = argv[1:]
    show = "--print" in args
    rest = [arg for arg in args if arg != "--print"]
    root = pathlib.Path(rest[0]) if rest else ROOT
    try:
        problems, rows = check(root)
    except (OSError, KeyError, CutError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"FAIL: {error!r}", file=sys.stderr)
        return 1
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    if show:
        print("\n".join(readiness_lines(rows)))
    counts = {name: sum(row[FIELD]["class"] == name for row in rows) for name in CLASSES}
    summary = ", ".join(f"{count} {name}" for name, count in counts.items())
    print(f"PASS: {len(rows)} components carry the cut class their code gives: {summary}.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
