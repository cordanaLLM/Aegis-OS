#!/usr/bin/env python3
"""Validate planning metadata; never execute imported code or claim OS readiness."""

import argparse
import hashlib
import json
import subprocess
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent


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
    if args.sources:
        verify_sources()
    if args.readiness:
        for row in rows:
            print(f"{row['id']} {row['name']}: {row['status']}; " + "; ".join(row["activation_blockers"]))
    print("PASS: planning structure and privacy; native build/boot/release unverified")


if __name__ == "__main__":
    main()
