#!/usr/bin/env python3
"""Validate planning metadata; never execute imported code or claim OS readiness."""

import argparse
import hashlib
import json
import os
import re
import string
import sys
import subprocess
import tomllib
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
HEX = string.hexdigits.lower()
LICENSE_TEXTS = {
    "LICENSE": "57fb42fbcd0b037ce528ed8f72f1ec095d67bc6825ecf1448ff39be1fe68a4b4",
    "LICENSES/EUPL-1.2.txt": "57fb42fbcd0b037ce528ed8f72f1ec095d67bc6825ecf1448ff39be1fe68a4b4",
    "LICENSES/CC-BY-SA-4.0.txt": "28a9529c7d0bb4dc51f4bf5c116a3d16ef247a052f7591466768ddf563fd1cf5",
    "LICENSES/MIT.txt": "b05785f9f18e6716bab63424b11454513b9943a222595b70411009202fc592b5",
    # The reference MPEG-2 frame's upstream terms (M27): it carries file-level
    # SPDX headers, never a REUSE.toml table, so LICENSE_IDS stays exact.
    "LICENSES/BSD-3-Clause.txt": "5a93d5831e1297ab10fe643e1a631e83be392896da14ee2951285a79012df69d",
    # The second licence of the code the Linux kernel loads (LICENSING.md,
    # "Exception: code the Linux kernel loads"). Those files carry file-level
    # SPDX headers too, never a REUSE.toml table; tools/test_bpf_objects.py
    # holds the expression to that set. Digest = the SPDX License List text.
    "LICENSES/GPL-2.0-only.txt": "aaf135472f81c5b4a0dca9367e5bb5e9750032b5bebe5442b36e4c0a47430df3",
}
LICENSE_IDS = {"EUPL-1.2", "CC-BY-SA-4.0"}
# Vendored third-party trees and the only identifier REUSE.toml may give each.
# Each is an override table after the whole-tree table; the split-licence
# identifiers above stay exact for everything else.
VENDORED_LICENSES = {
    "tools/figures/third_party/interfig/upstream/**": "MIT",
    "tools/figures/dist/**": "EUPL-1.2 AND MIT",
}
ROADMAP_STATES = {"done", "ready", "blocked"}
# A component is a proposal until it carries the evidence ACTIVATION_EVIDENCE
# names. "activated" is the only advanced status this tool admits: it means the
# component has its own recorded directory, a tracked manifest inside that
# directory, a tracked dependency lock and a test command that names the
# component. It is deliberately NOT a runtime claim - native build, image, boot,
# hardware and release remain separate blocked gates, so a status such as
# "boot-verified" still fails closed.
#
# The binding matters as much as the existence: without it a component could
# certify itself with another component's manifest and another component's test
# command, and every path would still be tracked. So the recorded component path
# must name the component, the manifest must sit under that path, and the test
# command must name the component too.
COMPONENT_STATUSES = {"proposal", "activated"}
ACTIVATED_STATUS = "activated"
# The repository's own machine-readable self-description, required as an exact
# literal so it cannot drift. "activation" names the process the inventory is
# in: components leave "proposal" one at a time under the evidence rule above,
# and at least one already carries that evidence, which is what stopped
# "planning" from being true. It is NOT a runtime or product claim - native
# build, image, boot, hardware, accessibility and release stay separate blocked
# gates, so a token such as "shipping" or "released" still fails closed - and it
# stays true while components are still being activated, so landing the next
# milestone does not require editing it. `make readiness` prints which
# components and milestones are where; this field records no count.
DECLARED_STAGE = "activation"
# The inventory is exact, not a floor: the concept's sixteen subsystems P01-P16
# plus P17 aegis-scaena, the display runtime decision D75 added and ADR-0003
# records as the one addition to the concept's subsystem set. A seventeenth id
# is therefore a recorded decision, and an eighteenth, a gap or a missing row
# still fails.
COMPONENT_COUNT = 17
ACTIVATION_EVIDENCE = (
    "component_path",
    "manifest_path",
    "lockfile_path",
    "test_command",
)
ACTIVATION_TRACKED_PATHS = ("manifest_path", "lockfile_path")
ROADMAP_COSTS = {"trivial", "small", "medium", "large"}
# Library code may not abort. Clippy denies panic, todo, unimplemented, unwrap_used,
# expect_used, unreachable and exit across the workspace, but no clippy lint covers the
# assert family, so this sweep is the only mechanism. Two crates carried their own text
# sweep and the other three were covered by nothing; this one reads the workspace member
# list, so a crate cannot be added and miss it.
ABORT_CONSTRUCTS = (
    "assert!(",
    "assert_eq!(",
    "assert_ne!(",
    "debug_assert!(",
    "debug_assert_eq!(",
    "debug_assert_ne!(",
)
# docs/roadmap/README.md names planning/roadmap.json as its source but is written
# by hand, and it repeats every rank and state twice: once in the ranked table and
# once in each section preamble. Both drifted unnoticed until this check existed.
TABLE_ROW = re.compile(r"^\|\s*(\d+)\s*\|\s*(M\d\d)\s*\|[^|]*\|\s*(done|ready|blocked)\s*\|", re.M)
SECTION_LINE = re.compile(r"^###\s+(M\d\d)\b[^\n]*\n\n?Rank\s+(\d+)\.\s+State:\s+(\w+)\.", re.M)
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
    "resizable_bar",
    "gpu_dma_buf_and_peer_memory",
    "realtime_kernel",
    "secure_boot_enrolment",
    # Recorded 2026-09-28 for M27 (D75, D79): hardware decode on the render node
    # the host compositor imports from, and what that compositor advertises.
    "vaapi_decode",
    "wayland_layer_shell",
    "compositor_dmabuf_import",
}
PROFILE_IDENTIFIERS = ("hostname", "serial", "uuid", "macaddress", "ip_address")
QUOTE_LIMIT = 200
# planning/candidates.json cites a tracked repository file as "public:<path>" with
# that file's full sha256, and verify_public_citation() rehashes it. The same
# convention is written by hand outside the register - in a crate's own source and
# in the roadmap documents - with a 12-character prefix instead of a full digest,
# and nothing read those. A superseded prefix for docs/integration/stack.md
# survived review in crates/aegis-vesta/src/decision.rs because the only check on
# it was a Rust test comparing the constant with a copy of itself. This sweep
# finds the citations it can recognise and runs the same comparison over the
# prefix. It is a check over the citations written in this shape, not a proof
# that every claim in a crate or a document is sourced.
PUBLIC_CITATION = re.compile(r"public:(?P<path>[A-Za-z0-9_][A-Za-z0-9_./-]*\.[A-Za-z0-9]+)")
CITATION_PREFIX = re.compile(r"[\"`](?P<prefix>[0-9a-f]{12})[\"`]")
CITATION_PREFIX_LEN = 12
# The digest follows the path within one struct field or one table cell. A wider
# window would start matching an unrelated hexadecimal literal further down.
CITATION_WINDOW = 200
CITATION_DOCUMENTS = ("docs/roadmap/README.md", "docs/roadmap/inventory.md")


def tracked_paths():
    """Return every path git has in the index, so evidence cannot cite a stray file."""
    try:
        result = subprocess.run(
            ["git", "ls-files", "-z"],
            cwd=ROOT,
            capture_output=True,
            check=True,
            timeout=30,
            env=git_env(),
        )
    except (subprocess.CalledProcessError, subprocess.TimeoutExpired, OSError) as error:
        raise ValueError(
            f"Cannot read the git index at {ROOT}: activation evidence is unverifiable"
        ) from error
    return {name for name in result.stdout.decode().split("\0") if name}


def verify_evidence_binding(row, evidence):
    """Evidence must certify this component, not merely exist somewhere in the tree."""
    name = row["name"]
    component_path = evidence["component_path"].strip("/")
    if name not in PurePosixPath(component_path).parts:
        raise ValueError(
            f"Unqualified readiness claim: {row['id']} records component path "
            f"{component_path!r}, which does not name {name}"
        )
    directory = ROOT / component_path
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError(
            f"Unqualified readiness claim: {row['id']} component path {component_path!r} "
            "is not a directory"
        )
    manifest = evidence["manifest_path"]
    if not manifest.startswith(f"{component_path}/"):
        raise ValueError(
            f"Unqualified readiness claim: {row['id']} cites manifest {manifest!r}, "
            f"which is outside its own path {component_path!r}"
        )
    if not re.search(rf"(?<![\w-]){re.escape(name)}(?![\w-])", evidence["test_command"]):
        raise ValueError(
            f"Unqualified readiness claim: {row['id']} test command does not name {name}"
        )
    declared = tomllib.loads((ROOT / manifest).read_text()).get("package", {}).get("name")
    if declared != name:
        raise ValueError(
            f"Unqualified readiness claim: {row['id']} cites a manifest declaring "
            f"{declared!r}, not {name!r}"
        )


def verify_activation_evidence(row, tracked):
    """An advanced status needs tracked evidence; anything less fails closed."""
    evidence = row.get("activation_evidence")
    if not isinstance(evidence, dict):
        raise ValueError(f"Unqualified readiness claim: {row['id']} records no activation evidence")
    for field in ACTIVATION_EVIDENCE:
        value = evidence.get(field)
        if not isinstance(value, str) or not value.strip():
            raise ValueError(f"Unqualified readiness claim: {row['id']} lacks {field}")
    for field in ACTIVATION_TRACKED_PATHS:
        path = evidence[field]
        if path not in tracked:
            raise ValueError(
                f"Unqualified readiness claim: {row['id']} cites {field} {path!r}, "
                "which git does not track"
            )
        if (ROOT / path).is_symlink() or not (ROOT / path).is_file():
            raise ValueError(f"Unqualified readiness claim: {row['id']} {field} is not a file")
    verify_evidence_binding(row, evidence)
    if not row.get("milestone"):
        raise ValueError(f"Unqualified readiness claim: {row['id']} names no milestone")


def git_env():
    """Environment for git calls: a hook exports GIT_DIR and GIT_INDEX_FILE, and
    inheriting them would read another repository's index instead of ROOT's."""
    return {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}


def read_json(path):
    if path.is_symlink() or not path.is_file() or path.stat().st_size > 1 << 20:
        raise ValueError(f"Invalid or oversized metadata: {path.name}")
    return json.loads(path.read_text())


def verify_components():
    data = read_json(ROOT / "planning/components.json")
    components = data["components"]
    if data["stage"] != DECLARED_STAGE or len(components) != COMPONENT_COUNT:
        raise ValueError(
            f"Expected stage declared exactly as {DECLARED_STAGE!r} "
            f"and {COMPONENT_COUNT} components"
        )
    expected = {f"P{i:02}" for i in range(1, COMPONENT_COUNT + 1)}
    if {row["id"] for row in components} != expected:
        raise ValueError("Missing or duplicate subsystem IDs")
    tracked = None
    for row in components:
        if row["status"] not in COMPONENT_STATUSES or not row["activation_blockers"]:
            raise ValueError(f"Unqualified readiness claim: {row['id']}")
        if row["status"] == ACTIVATED_STATUS:
            if tracked is None:
                tracked = tracked_paths()
            verify_activation_evidence(row, tracked)
    return components


def verify_privacy():
    result = subprocess.run(
        ["git", "ls-files", "-z", "--", ".workingdir", ".workingdir2"],
        cwd=ROOT,
        capture_output=True,
        check=True,
        timeout=10,
        env=git_env(),
    )
    if result.stdout:
        raise ValueError("Private working data is tracked or staged")
    subprocess.run(
        ["git", "check-ignore", "-q", "--no-index", ".workingdir/privacy-probe"],
        cwd=ROOT,
        check=True,
        env=git_env(),
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
    rows = reuse.get("annotations", [])
    own = [row for row in rows if not vendored_path(row)]
    declared = {row.get("SPDX-License-Identifier") for row in own}
    if reuse.get("version") != 1 or declared != LICENSE_IDS:
        raise ValueError(
            "REUSE.toml must declare version 1 with exactly the split-licence identifiers"
        )
    verify_vendored(rows)
    if not (ROOT / "LICENSING.md").is_file():
        raise ValueError("LICENSING.md explains the split licence and must exist")


def vendored_path(row):
    """Return the vendored tree an annotation table labels alone, or None."""
    paths = row.get("path")
    if isinstance(paths, list) and len(paths) == 1 and paths[0] in VENDORED_LICENSES:
        return paths[0]
    return None


def verify_vendored(rows):
    """Each vendored tree present on disk has its override after the whole-tree table."""
    whole = [index for index, row in enumerate(rows) if row.get("path") == ["**"]]
    for tree, identifier in VENDORED_LICENSES.items():
        found = [index for index, row in enumerate(rows) if vendored_path(row) == tree]
        if not found:
            if (ROOT / tree.removesuffix("/**")).exists():
                raise ValueError(f"REUSE.toml labels no licence on the vendored tree {tree}")
            continue
        row = rows[found[-1]]
        if len(found) > 1 or row.get("SPDX-License-Identifier") != identifier:
            raise ValueError(f"REUSE.toml must label {tree} once, as {identifier}")
        if row.get("precedence") != "override" or any(index > found[-1] for index in whole):
            raise ValueError(f"REUSE.toml must label {tree} by an override after the ** table")


MILESTONE_FIELDS = {"id", "title", "rank", "state", "blocked_by", "cost", "exit_criteria", "epics"}
# external_unblocks (#222): plan nodes outside this repository that wait on a
# milestone, each with the issue or decision record of the consuming repository
# that names the dependency. A milestone of this roadmap belongs in unblocks.
EXTERNAL_UNBLOCK_KEYS = {"node", "source"}
PLAN_NODE = re.compile(r"[A-Z][A-Za-z0-9.-]{0,31}")
MILESTONE_ID = re.compile(r"M[0-9]{2}")


def external_entry_valid(entry):
    """True when an entry holds exactly an outside plan node id and a non-empty source."""
    if not isinstance(entry, dict) or set(entry) != EXTERNAL_UNBLOCK_KEYS:
        return False
    node, source = entry["node"], entry["source"]
    if not isinstance(node, str) or not isinstance(source, str):
        return False
    return bool(PLAN_NODE.fullmatch(node) and not MILESTONE_ID.fullmatch(node) and source.strip())


def verify_external_unblocks(row):
    """external_unblocks is a list of valid entries naming each node once."""
    entries = row.get("external_unblocks", [])
    if not isinstance(entries, list):
        raise ValueError(f"Milestone {row['id']} external_unblocks must be a list")
    nodes = []
    for entry in entries:
        if not external_entry_valid(entry):
            raise ValueError(
                f"Milestone {row['id']} external_unblocks entry {entry!r} must carry exactly "
                "a plan node id outside this roadmap and the source that names the dependency"
            )
        nodes.append(entry["node"])
    if len(set(nodes)) != len(nodes):
        raise ValueError(f"Milestone {row['id']} lists an external_unblocks node twice")


# external_blockers: releases outside this repository a milestone waits on, each
# with the issue or decision record that names the wait. A milestone of this
# roadmap belongs in blocked_by. An open entry holds the milestone blocked, so
# readiness shows the wait rather than a ready milestone with a missing input.
EXTERNAL_BLOCKER_KEYS = {"release", "source"}


def blocker_entry_valid(entry):
    """True when an entry holds exactly a non-empty release and a non-empty source."""
    if not isinstance(entry, dict) or set(entry) != EXTERNAL_BLOCKER_KEYS:
        return False
    return all(isinstance(entry[key], str) and entry[key].strip() for key in EXTERNAL_BLOCKER_KEYS)


def verify_external_blockers(row):
    """external_blockers is a list of valid entries naming each release once.

    Returns the entries. A milestone that waits outside needs an external
    contract, so the ranking controller places it in that tier.
    """
    entries = row.get("external_blockers", [])
    if not isinstance(entries, list) or not all(blocker_entry_valid(e) for e in entries):
        raise ValueError(
            f"Milestone {row['id']} external_blockers must be a list of entries carrying "
            "exactly a release outside this roadmap and the source that names the wait"
        )
    releases = [entry["release"] for entry in entries]
    if len(set(releases)) != len(releases):
        raise ValueError(f"Milestone {row['id']} lists an external_blockers release twice")
    if entries and not row.get("needs_external_contract"):
        raise ValueError(
            f"Milestone {row['id']} waits on {releases[0]} but does not set "
            "needs_external_contract"
        )
    if entries and row["state"] == "done":
        raise ValueError(f"Milestone {row['id']} is done while it still waits on {releases[0]}")
    return entries


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
    verify_external_unblocks(row)
    waits_outside = verify_external_blockers(row)
    blockers_done = not waits_outside and all(
        by_id[b]["state"] == "done" for b in row["blocked_by"]
    )
    expected = "ready" if blockers_done else "blocked"
    if row["state"] != "done" and row["state"] != expected:
        raise ValueError(f"Milestone {row['id']} must be {expected} given its blockers")
    for blocker in row["blocked_by"]:
        if by_id[blocker]["rank"] >= row["rank"]:
            raise ValueError(f"Milestone {row['id']} is ranked before its blocker {blocker}")


def verify_epic_ownership(rows):
    """Every epic belongs to exactly one milestone, and its id names that milestone.

    M23 once carried byte-identical copies of M07's epics, and two milestone
    pairs claimed the same epic until D71 and D72 settled them. A copied epic
    keeps either its id or its title, so both are refused across milestones.
    """
    owner_of_id, owner_of_title = {}, {}
    for row in rows:
        prefix = f"E{row['id'][1:]}-"
        for epic in row["epics"]:
            if not epic.get("id", "").startswith(prefix):
                raise ValueError(
                    f"Milestone {row['id']} holds epic {epic.get('id')!r}; its epics are {prefix}N"
                )
            for key, owners in ((epic["id"], owner_of_id), (epic.get("title"), owner_of_title)):
                if key is None:
                    continue
                if owners.get(key, row["id"]) != row["id"]:
                    raise ValueError(
                        f"Epic {key!r} is claimed by both {owners[key]} and {row['id']}"
                    )
                owners[key] = row["id"]


PROSE_FIELDS = (("reference_profile", "rationale"), (None, "cheapest_exit"))


def verify_prose_endings(rows):
    """Planning prose ends on a sentence terminator, so a sliced string cannot pass.

    Twelve reference_profile.rationale strings were once cut at 400 characters,
    some mid-word, and six more lacked a full stop.
    """
    for row in rows:
        for parent, field in PROSE_FIELDS:
            holder = (row.get(parent) or {}) if parent else row
            text = holder.get(field)
            if text is not None and not text.rstrip().endswith((".", "!", "?")):
                name = f"{parent}.{field}" if parent else field
                raise ValueError(
                    f"Milestone {row['id']} {name} ends {text[-24:]!r}, not on a full stop"
                )


def verify_ranking(milestones):
    """The committed order must match the ranking controller, or record why not."""
    sys.path.insert(0, str(ROOT / "tools"))
    import rank_roadmap

    profile = ROOT / "planning/hardware-profile.json"
    absent = set()
    if profile.is_file():
        capabilities = read_json(profile)["capabilities"]
        absent = {name for name, row in capabilities.items() if not row["present"]}
    _, order, diffs, _ = rank_roadmap.drift(milestones, absent)
    if diffs:
        first = diffs[0]
        raise ValueError(
            f"Roadmap order drifts from the ranking controller at rank {first[0]}: "
            f"committed {first[1]}, computed {first[2]}; record rank_override.reason or re-rank"
        )
    return order


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
    verify_epic_ownership(rows)
    verify_prose_endings(rows)
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


def verify_roadmap_document(rows):
    """Both rank-and-state surfaces of the roadmap document must match the register.

    The document is not generated, so a milestone that changes state in
    planning/roadmap.json leaves two places behind. Ten table rows and three
    section preambles were stale when this check was written.
    """
    text = (ROOT / "docs/roadmap/README.md").read_text(encoding="utf-8")
    surfaces = (
        ("ranked table", {key: (int(r), s) for r, key, s in TABLE_ROW.findall(text)}),
        ("milestone sections", {key: (int(r), s) for key, r, s in SECTION_LINE.findall(text)}),
    )
    for label, found in surfaces:
        if len(found) != len(rows):
            raise ValueError(
                f"docs/roadmap/README.md lists {len(found)} milestones in its "
                f"{label}, planning/roadmap.json has {len(rows)}"
            )
        for row in rows:
            got = found.get(row["id"])
            if got is None:
                raise ValueError(f"docs/roadmap/README.md {label} omits {row['id']}")
            if got != (row["rank"], row["state"]):
                raise ValueError(
                    f"docs/roadmap/README.md {label} for {row['id']} reads rank "
                    f"{got[0]} state {got[1]}; planning/roadmap.json says rank "
                    f"{row['rank']} state {row['state']}"
                )


def workspace_members():
    """The crates the workspace activates, read from the manifest rather than globbed."""
    manifest = ROOT / "Cargo.toml"
    if not manifest.is_file():
        return []
    members = tomllib.loads(manifest.read_text())["workspace"]["members"]
    return [ROOT / member for member in members]


def verify_library_aborts():
    """No activated crate's library sources may abort.

    A doc comment may carry an assertion: a doctest is a test. Code may not, because
    an abort in a library is an unhandled error path by another name (HISS-07).
    """
    for member in workspace_members():
        source = member / "src"
        if not source.is_dir():
            raise ValueError(f"Workspace member {member.name} has no src directory")
        for path in sorted(source.rglob("*.rs")):
            for number, line in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
                if line.lstrip().startswith("//"):
                    continue
                for construct in ABORT_CONSTRUCTS:
                    if construct in line:
                        raise ValueError(
                            f"{path.relative_to(ROOT)}:{number} aborts in library code: "
                            f"{construct} -- return an error instead"
                        )


# The longest a workflow job may run (HISS-02). The slowest job, the
# Verification gate, took at most 397 s over its 40 latest successful runs on
# 2026-10-13, so a 30-minute limit let a sixfold slowdown pass green.
JOB_TIMEOUT_CEILING = 15
# A workflow whose job measurably needs more names its own ceiling here. The
# Verification gate's 40 latest successful runs on main took 494 to 821 s on
# 2026-10-18; the engine audit wants timeout-minutes at 1.25 x the slowest of
# the three latest, and 1.25 x 821 s is 17.1 minutes.
MEASURED_JOB_TIMEOUT_CEILINGS = {"ci.yml": 18}
RUNS_ON = re.compile(r"^(?P<indent> *)runs-on:")
TIMEOUT = re.compile(r"^(?P<indent> *)timeout-minutes: *(?P<value>[^ #]*) *(#.*)?$")


def job_timeouts(text):
    """Return the (indent, value) of each `runs-on` and `timeout-minutes` line."""
    runs = [match["indent"] for match in map(RUNS_ON.match, text.splitlines()) if match]
    limits = [
        (match["indent"], match["value"])
        for match in map(TIMEOUT.match, text.splitlines())
        if match
    ]
    return runs, limits


def verify_job_timeouts():
    """Every workflow job sets its own timeout, at most JOB_TIMEOUT_CEILING minutes, or
    the ceiling MEASURED_JOB_TIMEOUT_CEILINGS names for its workflow."""
    workflows = sorted((ROOT / ".github/workflows").glob("*.yml"))
    if not workflows:
        raise ValueError("no workflow under .github/workflows to check for job timeouts")
    for path in workflows:
        name = path.relative_to(ROOT)
        runs, limits = job_timeouts(path.read_text(encoding="utf-8"))
        job_limits = [value for indent, value in limits if indent in runs]
        if len(job_limits) != len(runs):
            raise ValueError(
                f"{name}: {len(runs)} job(s) but {len(job_limits)} job-level "
                "timeout-minutes; every job sets its own (HISS-02)"
            )
        ceiling = MEASURED_JOB_TIMEOUT_CEILINGS.get(path.name, JOB_TIMEOUT_CEILING)
        for value in job_limits:
            if not value.isdigit() or not 0 < int(value) <= ceiling:
                raise ValueError(
                    f"{name}: timeout-minutes {value!r} is not a whole number from 1 "
                    f"to {ceiling}"
                )


def citation_files():
    """Every file this sweep reads: each workspace member's library sources, then the
    roadmap documents that repeat the register's citations in prose."""
    paths = []
    for member in workspace_members():
        source = member / "src"
        if not source.is_dir():
            raise ValueError(f"Workspace member {member.name} has no src directory")
        paths.extend(sorted(source.rglob("*.rs")))
    for name in CITATION_DOCUMENTS:
        path = ROOT / name
        if path.is_file() and not path.is_symlink():
            paths.append(path)
    return paths


def verify_crate_citations():
    """Check every `public:` citation this sweep can recognise against its file.

    This is a check over an enumeration, not a proof over a category: it reads
    the files citation_files() lists, matches PUBLIC_CITATION, and compares the
    first CITATION_PREFIX_LEN characters of the named file's sha256 with the
    quoted prefix that follows within CITATION_WINDOW characters. A citation
    written in another shape, or in a file outside that list, is invisible here.
    Returns the number of citations actually compared, so a sweep that silently
    stops finding any is visible to its tests.
    """
    checked = 0
    for path in citation_files():
        text = path.read_text(encoding="utf-8")
        for match in PUBLIC_CITATION.finditer(text):
            number = text.count("\n", 0, match.start()) + 1
            label = f"{path.relative_to(ROOT)}:{number}"
            source_id = "public:" + match["path"]
            window = CITATION_PREFIX.search(text, match.end(), match.end() + CITATION_WINDOW)
            if window is None:
                raise ValueError(
                    f"{label} cites {source_id} with no quoted 12-character digest "
                    f"prefix in the following {CITATION_WINDOW} characters"
                )
            actual = public_citation_digest(source_id)[:CITATION_PREFIX_LEN]
            if window["prefix"] != actual:
                raise ValueError(
                    f"{label} cites {source_id} as {window['prefix']}; the tracked "
                    f"file hashes to {actual} -- the citation names a superseded revision"
                )
            checked += 1
    return checked


def verify_digest(value, label):
    """Accept only a full lowercase hexadecimal sha256 digest."""
    if not isinstance(value, str) or len(value) != 64 or value.strip(HEX) != "":
        raise ValueError(f"{label} needs a 64-character lowercase sha256 digest")


def public_citation_digest(source_id):
    """The sha256 of the tracked file a public citation names."""
    path = ROOT / source_id.split("public:", 1)[1].split("@")[0]
    if path.is_symlink() or not path.is_file():
        raise ValueError(f"Cited repository file is missing: {source_id}")
    return hashlib.sha256(path.read_bytes()).hexdigest()


def verify_public_citation(source_id, digest):
    """A public citation must hash to the tracked file it names."""
    if public_citation_digest(source_id) != digest:
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


def verify_candidate(row, proposals, activated):
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
    if row["component"] in activated and not (row["manifest_present"] and row["lockfile_present"]):
        raise ValueError(
            f"Candidate {row['candidate_id']} records no manifest or lock while "
            f"{row['component']} is activated"
        )


def verify_dispute(row):
    if row["status"] not in DISPUTE_STATES:
        raise ValueError(f"Contradiction {row['id']} needs a status in {sorted(DISPUTE_STATES)}")
    for side in ("side_a", "side_b"):
        verify_side(row[side], f"Contradiction {row['id']} {side}")
    resolved = row["status"] == "resolved"
    if resolved != bool(row.get("resolution")):
        raise ValueError(f"Contradiction {row['id']} records a resolution only when resolved")


def verify_register_rows(data, proposals, activated):
    for name, limit in REGISTER_LIMITS.items():
        rows = data[name]
        if not 1 <= len(rows) <= limit:
            raise ValueError(f"Register section {name} must hold 1..{limit} rows")
    ids = [row["candidate_id"] for row in data["candidates"]]
    if len(set(ids)) != len(ids):
        raise ValueError("Duplicate candidate identifiers")
    for row in data["candidates"]:
        verify_candidate(row, proposals, activated)
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
    activated = {row["id"] for row in components if row["status"] == ACTIVATED_STATUS}
    covered = {row["component"] for row in data["candidates"]}
    if covered != {row["id"] for row in components}:
        raise ValueError("Register must cover every component exactly once or more")
    verify_register_rows(data, proposals, activated)
    return data


def verify_profile_capability(name, row):
    if not isinstance(row.get("present"), bool) or not row.get("evidence_command"):
        raise ValueError(f"Capability {name} needs a boolean and an evidence command")


def _verify_profile_metadata(data):
    """Validate operator actions and metadata requirements of the hardware profile."""
    for row in data.get("operator_actions", []):
        if not row.get("action") or not row.get("unblocks"):
            raise ValueError("Every operator action needs an action and what it unblocks")
    if data["schema_version"] != 1 or not data["evidence_class"]:
        raise ValueError("Profile must declare schema 1 and its evidence class")
    if "development evidence only" not in data["evidence_class"].lower():
        raise ValueError("Profile must state that it is development evidence only")


def _verify_milestone_profile_claims(milestones, available):
    """Validate milestone capability requirements and full support claims against profile."""
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


def verify_hardware_profile(milestones):
    """Validate the reference profile and every milestone claim made against it."""
    data = read_json(ROOT / "planning/hardware-profile.json")
    _verify_profile_metadata(data)
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
    _verify_milestone_profile_claims(milestones, available)
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


def milestone_line(row):
    """One readiness line: state, rank, cost, blockers, outside waits and outside nodes."""
    marker = "READY" if row["state"] == "ready" else row["state"]
    blockers = ", ".join(row["blocked_by"]) or "nothing"
    line = (
        f"{row['id']} [{marker}] rank {row['rank']} cost {row['cost']}: "
        f"{row['title']}; blocked by {blockers}"
    )
    waits = [entry["release"] for entry in row.get("external_blockers", [])]
    if waits:
        line += f"; waits outside: {', '.join(waits)}"
    outside = [entry["node"] for entry in row.get("external_unblocks", [])]
    if outside:
        line += f"; unblocks outside: {', '.join(outside)}"
    return line


# The release train runs its action once more after a failed first attempt
# (HISS-02). On 2026-10-18 one GitHub GraphQL server error failed the train on
# main, and the engine's workflow-run check then failed every pull request's
# gate until the run was repeated by hand.
RELEASE_WORKFLOW = ".github/workflows/release-please.yml"
RELEASE_ACTION = "googleapis/release-please-action@"
STEP_ITEM = re.compile(r"^(?P<indent> *)- ")
RETRY_KEYS = ("name", "id", "if", "continue-on-error")


def workflow_steps(text):
    """Split a workflow into its list items, each ending at the first line indented
    less than its dash."""
    steps, current, indent = [], None, 0
    for line in text.splitlines():
        item = STEP_ITEM.match(line)
        if item:
            current, indent = [line], len(item["indent"])
            steps.append(current)
        elif current is not None and line.strip():
            if len(line) - len(line.lstrip(" ")) <= indent:
                current = None
            else:
                current.append(line)
    return steps


def step_field(step, key):
    """The value of one top-level key of a step, or None when the step has none."""
    pattern = re.compile(rf"^ *(- )?{re.escape(key)}: *(?P<value>.*?) *$")
    values = [match["value"] for match in map(pattern.match, step) if match]
    return values[0] if values else None


def step_body(step):
    """A step without the keys that tell its first attempt from its retry."""
    pattern = re.compile(rf"^ *(- )?({'|'.join(RETRY_KEYS)}):")
    return [line.lstrip(" -") for line in step if not pattern.match(line)]


def anchored_block(step):
    """Return (anchor_name, block_lines) for a with: &anchor block, or (None, [])."""
    anchor_name, block = None, []
    recording, base_indent = False, 0
    for line in step:
        if not recording:
            match = re.match(r"^(?P<indent> *)with:\s*&(?P<name>[^\s#]+)\s*(?:#.*)?$", line)
            if match:
                anchor_name = match.group("name")
                base_indent = len(match.group("indent"))
                recording = True
                block.append(line)
        elif line.strip() and len(line) - len(line.lstrip(" ")) <= base_indent:
            break
        else:
            block.append(line)
    return anchor_name, block


def expand_retry_step(first, retry):
    """Expand with: *alias in retry to the matching with: &anchor block from first."""
    if any(re.match(r"^ *with:\s*&", line) for line in retry):
        return None
    anchor_name, block = anchored_block(first)
    expanded = []
    for line in retry:
        match = re.match(r"^ *with:\s*\*(?P<name>[^\s#]+)\s*(?:#.*)?$", line)
        if match and anchor_name and match.group("name") == anchor_name:
            expanded.extend(block)
        else:
            expanded.append(line)
    return expanded


def verify_release_retry():
    """The release train runs its action, and once more only when that failed."""
    path = ROOT / RELEASE_WORKFLOW
    if not path.is_file():
        raise ValueError(f"{RELEASE_WORKFLOW}: missing; the release train is checked there")
    text = path.read_text(encoding="utf-8")
    steps = [step for step in workflow_steps(text) if any(RELEASE_ACTION in line for line in step)]
    if len(steps) != 2:
        raise ValueError(
            f"{RELEASE_WORKFLOW}: {len(steps)} release-please step(s); the train runs "
            "it once and retries it once"
        )
    first, retry = steps
    ident = step_field(first, "id")
    if not ident or step_field(first, "continue-on-error") != "true":
        raise ValueError(
            f"{RELEASE_WORKFLOW}: the first release-please step needs an id and "
            "continue-on-error: true, so its failure reaches the retry"
        )
    if step_field(retry, "if") != f"steps.{ident}.outcome == 'failure'":
        raise ValueError(
            f"{RELEASE_WORKFLOW}: the retry runs only if steps.{ident}.outcome == 'failure'"
        )
    if step_field(retry, "continue-on-error") is not None:
        raise ValueError(f"{RELEASE_WORKFLOW}: a failed retry must fail the run")
    expanded = expand_retry_step(first, retry)
    if expanded is None or step_body(first) != step_body(expanded):
        raise ValueError(f"{RELEASE_WORKFLOW}: the retry runs the same pin with the same inputs")


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
    verify_roadmap_document(milestones)
    verify_library_aborts()
    verify_job_timeouts()
    verify_release_retry()
    verify_crate_citations()
    profile = verify_hardware_profile(milestones)
    verify_ranking(milestones)
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
            print(milestone_line(row))
    print(
        "PASS: the tracked register under planning/ - inventory, privacy, licence "
        "digests, source citations, job timeouts, release retry, hardware profile, roadmap states "
        "and ranking - "
        "and the roadmap document against it. No build, image, boot, hardware, "
        "accessibility or release evidence is checked here."
    )


if __name__ == "__main__":
    main()
