#!/usr/bin/env python3
"""Validate planning metadata; never execute imported code or claim OS readiness."""

import argparse
import hashlib
import json
import string
import subprocess
import tomllib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
HEX = string.hexdigits.lower()
LICENSE_TEXTS = {
    "LICENSE": "57fb42fbcd0b037ce528ed8f72f1ec095d67bc6825ecf1448ff39be1fe68a4b4",
    "LICENSES/EUPL-1.2.txt": "57fb42fbcd0b037ce528ed8f72f1ec095d67bc6825ecf1448ff39be1fe68a4b4",
    "LICENSES/CC-BY-SA-4.0.txt": "28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5",
}
LICENSE_IDS = {"EUPL-1.2", "CC-BY-SA-4.0"}
ROADMAP_STATES = {"done", "ready", "blocked"}
ROADMAP_COSTS = {"trivial", "small", "medium", "large"}
REGISTER_BUNDLE = "8186bf0336e16764216396a147c536d96b3933901f5b2b81a4d0d3b74ffa25c6"
REGISTER_LIMITS = {
    "candidates": 64,
    "contradictions": 256,
    "toolchain_drift": 256,
    "quarantined_artifacts": 64,
}
DISPUTE_STATES = {"resolved", "open"}
PROFILE_CAPABILITIES = {
    "kvm",
    "tpm2",
    "iommu",
    "rapl_energy_counters",
    "sched_ext",
    "bpf_lsm",
    "btf",
    "pci_p2pdma",
    "erofs_dm_verity",
    "realtime_kernel",
    "secure_boot_enrolment",
}
PROFILE_IDENTIFIERS = ("hostname", "serial", "uuid", "macaddress", "ip_address")
QUOTE_LIMIT = 200


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
        cwd=ROOT,
        capture_output=True,
        check=True,
        timeout=10,
    )
    if result.stdout:
        raise ValueError("Private working data is tracked or staged")
    subprocess.run(
        ["git", "check-ignore", "-q", "--no-index", ".workingdir/privacy-probe"],
        cwd=ROOT,
        check=True,
        timeout=10,
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
        raise ValueError(
            "REUSE.toml must declare version 1 with exactly the split-licence identifiers"
        )
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


def verify_digest(value, label):
    """Accept only a full lowercase hexadecimal sha256 digest."""
    if not isinstance(value, str) or len(value) != 64 or value.strip(HEX) != "":
        raise ValueError(f"{label} needs a 64-character lowercase sha256 digest")


def verify_public_citation(source_id, digest):
    """A public citation must hash to the tracked file it names."""
    path = ROOT / source_id.split("public:", 1)[1].split("@")[0]
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"Cited repository file is missing: {source_id}")
    if hashlib.sha256(path.read_bytes()).hexdigest() != digest:
        raise ValueError(
            f"Cited repository file changed since the register was written: {source_id}"
        )


def verify_side(side, label):
    source_id = side["source_id"]
    verify_digest(side["sha256"], f"{label} {source_id}")
    if not side["summary"] or len(side["summary"]) > QUOTE_LIMIT:
        raise ValueError(f"{label} needs a summary of at most {QUOTE_LIMIT} characters")
    if source_id.startswith("public:"):
        verify_public_citation(source_id, side["sha256"])


def verify_candidate(row, proposals):
    if not row["proposed_paths"] or not row["sources"]:
        raise ValueError(f"Candidate {row['candidate_id']} needs a path and a source")
    for source in row["sources"]:
        verify_digest(source["sha256"], f"Candidate {row['candidate_id']} {source['id']}")
    claims = row["manifest_present"] or row["lockfile_present"]
    if claims and row["component"] in proposals:
        raise ValueError(
            f"Candidate {row['candidate_id']} claims a manifest while {row['component']} "
            "is still a proposal"
        )


def verify_dispute(row):
    if row["status"] not in DISPUTE_STATES:
        raise ValueError(f"Contradiction {row['id']} needs a status in {sorted(DISPUTE_STATES)}")
    for side in ("side_a", "side_b"):
        verify_side(row[side], f"Contradiction {row['id']} {side}")
    resolved = row["status"] == "resolved"
    if resolved != bool(row.get("resolution")):
        raise ValueError(f"Contradiction {row['id']} records a resolution only when resolved")


def verify_register_rows(data, proposals):
    for name, limit in REGISTER_LIMITS.items():
        rows = data[name]
        if not 1 <= len(rows) <= limit:
            raise ValueError(f"Register section {name} must hold 1..{limit} rows")
    ids = [row["candidate_id"] for row in data["candidates"]]
    if len(set(ids)) != len(ids):
        raise ValueError("Duplicate candidate identifiers")
    for row in data["candidates"]:
        verify_candidate(row, proposals)
    for row in data["contradictions"]:
        verify_dispute(row)
    for row in data["toolchain_drift"]:
        verify_digest(row["sha256"], f"Drift row {row['item']}")
    for row in data["quarantined_artifacts"]:
        verify_digest(row["sha256"], f"Quarantined {row['artifact']}")
        if row["tracked_in_repository"] or not row["defects"]:
            raise ValueError(f"Quarantined {row['artifact']} must be untracked and list defects")


def verify_candidates(components):
    """Validate the M01 register against the pinned bundle and the component inventory."""
    data = read_json(ROOT / "planning/candidates.json")
    if data["schema_version"] != 1 or data["source_bundle_sha256"] != REGISTER_BUNDLE:
        raise ValueError("Register must declare schema 1 and the pinned source bundle")
    proposals = {row["id"] for row in components if row["status"] == "proposal"}
    covered = {row["component"] for row in data["candidates"]}
    if covered != {row["id"] for row in components}:
        raise ValueError("Register must cover every component exactly once or more")
    verify_register_rows(data, proposals)
    return data


def verify_profile_capability(name, row):
    if not isinstance(row.get("present"), bool) or not row.get("evidence_command"):
        raise ValueError(f"Capability {name} needs a boolean and an evidence command")


def verify_hardware_profile(milestones):
    """Validate the reference profile and every milestone claim made against it."""
    data = read_json(ROOT / "planning/hardware-profile.json")
    if data["schema_version"] != 1 or not data["evidence_class"]:
        raise ValueError("Profile must declare schema 1 and its evidence class")
    if "development evidence only" not in data["evidence_class"].lower():
        raise ValueError("Profile must state that it is development evidence only")
    capabilities = data["capabilities"]
    if set(capabilities) != PROFILE_CAPABILITIES:
        raise ValueError("Profile must record exactly the declared capability set")
    for name, row in capabilities.items():
        verify_profile_capability(name, row)
    blob = json.dumps(data).lower()
    leaked = [key for key in PROFILE_IDENTIFIERS if f'"{key}"' in blob]
    if leaked:
        raise ValueError(f"Profile must not record machine identifiers: {leaked}")
    available = {name for name, row in capabilities.items() if row["present"]}
    for row in milestones:
        claimed = row.get("reference_profile", {}).get("requires", [])
        unknown = set(claimed) - PROFILE_CAPABILITIES
        if unknown:
            raise ValueError(f"Milestone {row['id']} names unknown capabilities {sorted(unknown)}")
        support = row.get("reference_profile", {}).get("support")
        if support == "full" and not set(claimed) <= available:
            raise ValueError(
                f"Milestone {row['id']} claims full local support without the capability"
            )
    return data


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
    register = verify_candidates(rows)
    milestones = verify_roadmap()
    profile = verify_hardware_profile(milestones)
    if args.sources:
        verify_sources()
    if args.readiness:
        open_disputes = sum(1 for row in register["contradictions"] if row["status"] == "open")
        print(
            f"M01 register: {len(register['candidates'])} candidates; "
            f"{len(register['contradictions'])} contradictions ({open_disputes} open); "
            f"{len(register['toolchain_drift'])} drift rows; "
            f"{len(register['quarantined_artifacts'])} quarantined artefacts"
        )
        absent = [name for name, row in profile["capabilities"].items() if not row["present"]]
        print(
            f"Reference profile: {profile['cpu']['model']}; "
            f"absent capabilities: {', '.join(absent) or 'none'}"
        )
        for row in rows:
            print(
                f"{row['id']} {row['name']}: {row['status']}; "
                + "; ".join(row["activation_blockers"])
            )
        for row in milestones:
            marker = "READY" if row["state"] == "ready" else row["state"]
            blockers = ", ".join(row["blocked_by"]) or "nothing"
            print(
                f"{row['id']} [{marker}] rank {row['rank']} cost {row['cost']}: "
                f"{row['title']}; blocked by {blockers}"
            )
    print(
        "PASS: planning structure, privacy, licence texts and roadmap states; "
        "native build/boot/release unverified"
    )


if __name__ == "__main__":
    main()
