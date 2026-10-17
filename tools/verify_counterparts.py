#!/usr/bin/env python3
"""Hold every leaf crate to a recorded reason for each counterpart it lacks.

A leaf crate is a workspace member that no other member depends on and that
depends on no other member, under any dependency kind. Such a crate passes its
own positive, negative and boundary tests while no test drives it against the
component on the other side of its interface, so a contract drift between the
two would pass every gate (issue #230).

``planning/components.json`` records, on the component a leaf crate belongs
to, one ``counterpart_gaps`` entry per edge of that component: the crate, the
relation and counterpart as the edge names them, the milestone whose exit
criteria bring the pairing (``null`` when none does) and the reason. This
check reads the root ``Cargo.toml``, every member manifest,
``planning/components.json`` and ``planning/roadmap.json``, and fails when a
leaf crate or one of its edges has no entry, when an entry names a crate that
is not a leaf (it has a counterpart now, so the entry is stale), an edge its
component does not have, a milestone the roadmap does not list or one already
done, or carries an empty reason.

Usage: ``python3 tools/verify_counterparts.py``.
``tools/test_verify_counterparts.py`` covers each refusal with a planted case.
"""

import json
import pathlib
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
COMPONENTS = "planning/components.json"
ROADMAP = "planning/roadmap.json"
FIELD = "counterpart_gaps"
ENTRY_KEYS = {"crate", "relation", "counterpart", "milestone", "reason"}
DEPENDENCY_TABLES = ("dependencies", "dev-dependencies", "build-dependencies")
DONE = "done"


class GapError(Exception):
    """A manifest or planning file cannot be read as expected."""


def dependency_tables(member):
    """Yield every dependency table of a member manifest."""
    for key in DEPENDENCY_TABLES:
        yield member.get(key, {})
    for tables in member.get("target", {}).values():
        for key in DEPENDENCY_TABLES:
            yield tables.get(key, {})


def dependency_names(member):
    """Return the package names a member manifest depends on, renames resolved."""
    names = set()
    for table in dependency_tables(member):
        for key, spec in table.items():
            package = spec.get("package", key) if isinstance(spec, dict) else key
            names.add(package)
    return names


def workspace_graph(root):
    """Map each member's package name to the member names it depends on."""
    manifest = tomllib.loads((root / "Cargo.toml").read_text(encoding="utf-8"))
    members = {}
    for member in manifest.get("workspace", {}).get("members", []):
        data = tomllib.loads((root / member / "Cargo.toml").read_text(encoding="utf-8"))
        name = data.get("package", {}).get("name")
        if not name or name in members:
            raise GapError(f"{member}/Cargo.toml: package name {name!r} missing or repeated")
        members[name] = dependency_names(data)
    return {name: deps & set(members) for name, deps in members.items()}


def leaf_crates(graph):
    """Return the members with no workspace dependency in either direction."""
    depended_on = set().union(*graph.values()) if graph else set()
    return {name for name, deps in graph.items() if not deps and name not in depended_on}


def component_edges(row):
    """Return the (relation, counterpart) pairs of one component row."""
    edges = [(edge["relation"], edge["to"]) for edge in row.get("sends_to", [])]
    edges += [(edge["relation"], edge["from"]) for edge in row.get("receives_from", [])]
    return edges


def entry_problems(where, entry, edges, milestones):
    """Report what is wrong with one counterpart_gaps entry on its own."""
    if not isinstance(entry, dict) or set(entry) != ENTRY_KEYS:
        return [f"{where}: an entry must carry exactly {sorted(ENTRY_KEYS)}"]
    problems = []
    if (entry["relation"], entry["counterpart"]) not in edges:
        problems.append(
            f"{where}: {entry['relation']} with {entry['counterpart']} is not an edge "
            "of this component"
        )
    milestone = entry["milestone"]
    if milestone is not None and milestone not in milestones:
        problems.append(f"{where}: milestone {milestone!r} is not in {ROADMAP}")
    elif milestone is not None and milestones[milestone] == DONE:
        problems.append(f"{where}: milestone {milestone} is done, so it brings nothing more")
    if not isinstance(entry["reason"], str) or not entry["reason"].strip():
        problems.append(f"{where}: the reason is empty")
    return problems


def read_entries(rows, milestones):
    """Collect every entry as (component id, crate, edge) and report bad ones."""
    found, problems = [], []
    for row in rows:
        entries = row.get(FIELD, [])
        if not isinstance(entries, list):
            problems.append(f"{row['id']}: {FIELD} is not a list")
            continue
        edges = component_edges(row)
        for index, entry in enumerate(entries):
            where = f"{row['id']} {FIELD}[{index}]"
            bad = entry_problems(where, entry, edges, milestones)
            problems += bad
            if not bad:
                found.append((row["id"], entry["crate"], (entry["relation"], entry["counterpart"])))
    return found, problems


def compare(graph, rows, found):
    """Report leaf crates or edges without an entry, and stale or repeated ones."""
    problems = []
    leaves = leaf_crates(graph)
    owners = {}
    for component, crate, edge in found:
        owners.setdefault(crate, {}).setdefault(component, []).append(edge)
    for crate in sorted(set(owners) - leaves):
        state = "is not a workspace member" if crate not in graph else "is not a leaf any more"
        problems.append(f"{crate}: has {FIELD} entries but {state}; drop them")
    by_id = {row["id"]: row for row in rows}
    for crate in sorted(leaves):
        components = owners.get(crate, {})
        if len(components) != 1:
            problems.append(
                f"{crate}: leaf crate needs its {FIELD} on exactly one component, "
                f"found {sorted(components)}"
            )
            continue
        component, edges = next(iter(components.items()))
        for edge in sorted(set(edges)):
            if edges.count(edge) > 1:
                problems.append(f"{crate}: {edge[0]} with {edge[1]} is recorded twice")
        for relation, counterpart in component_edges(by_id[component]):
            if (relation, counterpart) not in edges:
                problems.append(
                    f"{crate}: {component} edge {relation} with {counterpart} has no entry"
                )
    return problems, len(leaves)


def check(root):
    """Return every problem and the number of leaf crates."""
    rows = json.loads((root / COMPONENTS).read_text(encoding="utf-8"))["components"]
    roadmap = json.loads((root / ROADMAP).read_text(encoding="utf-8"))
    milestones = {row["id"]: row["state"] for row in roadmap["milestones"]}
    found, problems = read_entries(rows, milestones)
    more, leaves = compare(workspace_graph(root), rows, found)
    return problems + more, leaves, len(found)


def main(argv):
    """Run the check on the repository and report PASS or each FAIL line."""
    root = pathlib.Path(argv[1]) if len(argv) > 1 else ROOT
    try:
        problems, leaves, entries = check(root)
    except (OSError, KeyError, GapError, json.JSONDecodeError, tomllib.TOMLDecodeError) as error:
        print(f"FAIL: {error!r}", file=sys.stderr)
        return 1
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    print(
        f"PASS: {leaves} leaf crates record why each of their {entries} edges has no counterpart."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
