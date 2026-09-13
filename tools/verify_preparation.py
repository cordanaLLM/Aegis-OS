#!/usr/bin/env python3
"""Validate planning metadata; never execute imported code or claim OS readiness."""

import argparse
import hashlib
import json
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LICENSE_TEXTS = {
    "LICENSE": "57fb42fbcd0b037ce528ed8f72f1ec095d67bc6825ecf1448ff39be1fe68a4b4",
    "LICENSES/EUPL-1.2.txt": "57fb42fbcd0b037ce528ed8f72f1ec095d67bc6825ecf1448ff39be1fe68a4b4",
    "LICENSES/CC-BY-SA-4.0.txt": "28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5",
}
LICENSE_IDS = {"EUPL-1.2", "CC-BY-SA-4.0"}
ROADMAP_STATES = {"done", "ready", "blocked"}
ROADMAP_COSTS = {"trivial", "small", "medium", "large"}


def read_json(path):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 1 << 20:
        raise ValueError(f"Invalid or oversized metadata: {path.name}")
    return json.loads(path.read_text())


def verify_components():
    data = read_json(ROOT / "planning/components.json")
    components = data["components"]
    if data["stage"] != "planning" or len(components) != 16:
        raise ValueError("Expected explicitly declared planning stage and 16 components")
    expected = {f"P{i:02}" for i in range(1, 17)}
    if {row["id"] for row in components} != expected:
        raise ValueError("Missing or duplicate subsystem IDs")
    for row in components:
        if row["status"] != "proposal" or not row["activation_blockers"]:
            raise ValueError(f"Unqualified readiness claim: {row['id']}")
    return components


def verify_privacy():
    result = subprocess.run(
        ["git", "ls-files", "-z", "--", ".workingdir", ".workingdir2"],
        cwd=ROOT, capture_output=True, check=True, timeout=10,
    )
    if result.stdout:
        raise ValueError("Private working data is tracked or staged")
    subprocess.run(
        ["git", "check-ignore", "-q", "--no-index", ".workingdir/privacy-probe"],
        cwd=ROOT, check=True, timeout=10,
    )


def verify_licensing():
    for name, digest in LICENSE_TEXTS.items():
        path = ROOT / name
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 1 << 20:
            raise ValueError(f"Missing or invalid licence text: {name}")
        if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
            raise ValueError(f"Licence text differs from the pinned canonical text: {name}")
    reuse = tomllib.loads((ROOT / "REUSE.toml").read_text())
    declared = {row.get("SPDX-License-Identifier") for row in reuse.get("annotations", [])}
    if reuse.get("version") != 1 or declared != LICENSE_IDS:
        raise ValueError("REUSE.toml must declare version 1 with exactly the split-licence identifiers")
    if not (ROOT / "LICENSING.md").is_file():
        raise ValueError("LICENSING.md explains the split licence and must exist")


MILESTONE_FIELDS = {"id", "title", "rank", "state", "blocked_by", "cost", "exit_criteria", "epics"}


def verify_milestone(row, by_id):
    missing = MILESTONE_FIELDS - row.keys()
    if missing:
        raise ValueError(f"Milestone {row.get('id', '?')} lacks fields {sorted(missing)}")
    if row["state"] not in ROADMAP_STATES or row["cost"] not in ROADMAP_COSTS:
        raise ValueError(f"Milestone {row['id']} has an invalid state or cost")
    if not row["exit_criteria"] or not row["epics"]:
        raise ValueError(f"Milestone {row['id']} needs exit criteria and epics")
    unknown = set(row["blocked_by"]) - by_id.keys()
    if unknown or row["id"] in row["blocked_by"]:
        raise ValueError(f"Milestone {row['id']} blocks on unknown milestones {sorted(unknown)}")
    if row["state"] == "done" and not row.get("evidence"):
        raise ValueError(f"Milestone {row['id']} is done without recorded evidence")
    blockers_done = all(by_id[b]["state"] == "done" for b in row["blocked_by"])
    expected = "ready" if blockers_done else "blocked"
    if row["state"] != "done" and row["state"] != expected:
        raise ValueError(f"Milestone {row['id']} must be {expected} given its blockers")
    for blocker in row["blocked_by"]:
        if by_id[blocker]["rank"] >= row["rank"]:
            raise ValueError(f"Milestone {row['id']} is ranked before its blocker {blocker}")


def verify_roadmap():
    rows = read_json(ROOT / "planning/roadmap.json")["milestones"]
    ids = [row["id"] for row in rows]
    if not 1 <= len(rows) <= 64 or len(set(ids)) != len(ids):
        raise ValueError("Roadmap must list 1..64 uniquely identified milestones")
    if sorted(row["rank"] for row in rows) != list(range(len(rows))):
        raise ValueError("Milestone ranks must be 0..N-1 without gaps")
    by_id = {row["id"]: row for row in rows}
    for row in rows:
        verify_milestone(row, by_id)
    indegree = {row["id"]: len(row["blocked_by"]) for row in rows}
    ready = [key for key, degree in indegree.items() if degree == 0]
    visited = 0
    for _ in range(len(rows)):
        if not ready:
            break
        current = ready.pop()
        visited += 1
        for row in rows:
            if current in row["blocked_by"]:
                indegree[row["id"]] -= 1
                if indegree[row["id"]] == 0:
                    ready.append(row["id"])
    if visited != len(rows):
        raise ValueError("Milestone dependencies contain a cycle")
    return rows


def verify_sources():
    data = read_json(ROOT / ".workingdir/source-inventory.json")
    source = ROOT / ".workingdir/notebookllmprep"
    rows = data["files"]
    if not 1 <= len(rows) <= 128 or data["count"] != len(rows):
        raise ValueError("Source inventory count invalid")
    names = {row["path"] for row in rows}
    if len(names) != len(rows) or names != {p.name for p in source.iterdir()}:
        raise ValueError("Source inventory changed; review additions and removals")
    for row in rows:
        if Path(row["path"]).name != row["path"]:
            raise ValueError("Source path must be a direct child")
        path = source / row["path"]
        if path.is_symlink() or not path.is_file() or path.stat().st_size > 4 << 20:
            raise ValueError(f"Invalid source: {row['path']}")
        raw = path.read_bytes()
        if len(raw) != row["bytes"] or hashlib.sha256(raw).hexdigest() != row["sha256"]:
            raise ValueError(f"Source changed: {row['path']}")
    print(f"PASS: {len(rows)} original source hashes; no source execution")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--sources", action="store_true")
    parser.add_argument("--readiness", action="store_true")
    args = parser.parse_args()
    rows = verify_components()
    verify_privacy()
    verify_licensing()
    milestones = verify_roadmap()
    if args.sources:
        verify_sources()
    if args.readiness:
        for row in rows:
            print(f"{row['id']} {row['name']}: {row['status']}; " + "; ".join(row["activation_blockers"]))
        for row in milestones:
            marker = "READY" if row["state"] == "ready" else row["state"]
            print(f"{row['id']} [{marker}] rank {row['rank']} cost {row['cost']}: {row['title']}; blocked by " + (", ".join(row["blocked_by"]) or "nothing"))
    print("PASS: planning structure, privacy, licence texts and roadmap states; native build/boot/release unverified")


if __name__ == "__main__":
    main()
