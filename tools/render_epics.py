#!/usr/bin/env python3
"""Render the roadmap epic issues from ``planning/roadmap.json`` and check them.

Each open issue labelled ``roadmap`` carries one epic. Its title and body are
written by hand once and then go stale: on 2026-10-13, 28 of the 30 open epic
issues named a rank or state the roadmap no longer held (issue #220). This tool
writes each epic's title and body from its milestone row, so the issue says
what the committed roadmap says and nothing more.

Usage:

- ``python3 tools/render_epics.py --print E22-1`` prints one rendered epic.
- ``python3 tools/render_epics.py --check`` reads the open epic issues through
  ``gh`` and exits 1 on any issue whose title or body differs from the render,
  or whose title names no epic in the roadmap.
- ``python3 tools/render_epics.py --check --issues FILE`` reads the issues
  from a ``gh issue list --json number,title,body`` dump instead.
- ``python3 tools/render_epics.py --apply`` rewrites each differing issue.

``.github/workflows/roadmap-epics.yml`` runs ``--check`` after the roadmap
changes on main and once a day. ``tools/test_render_epics.py`` covers the
render and each refusal, a planted stale rank among them.
"""

import argparse
import json
import pathlib
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
ROADMAP = "planning/roadmap.json"
REPOSITORY = "cordanaLLM/Aegis-OS"
LABEL = "roadmap"
MAX_BYTES = 1 << 20
MAX_ISSUES = 200
GH_TIMEOUT = 60
FOOTER = (
    "Blocking states are maintained in `planning/roadmap.json`; `make readiness` "
    "lists the ready set. Close this epic only with committed evidence."
)
PROFILE_NOTE = "A pass on the reference profile is development evidence and closes no gate."


class EpicError(Exception):
    """The roadmap or the issue list cannot be read as expected."""


def read_json(path):
    """Return the parsed JSON file, refusing a link or an oversized file."""
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_BYTES:
        raise EpicError(f"invalid or oversized file: {path}")
    try:
        return json.loads(path.read_text(encoding="utf-8"))
    except (OSError, UnicodeDecodeError, json.JSONDecodeError) as error:
        raise EpicError(f"{path}: {error}") from error


def epic_rows(roadmap):
    """Return {epic id: (milestone, epic)} for every epic in the roadmap."""
    rows = {}
    for milestone in roadmap["milestones"]:
        for epic in milestone.get("epics", []):
            if epic["id"] in rows:
                raise EpicError(f"epic {epic['id']} appears twice in {ROADMAP}")
            rows[epic["id"]] = (milestone, epic)
    return rows


def milestone_line(milestone):
    """Return the bold milestone line with rank, state, cost and profile."""
    facts = [
        f"rank {milestone['rank']}",
        f"state {milestone['state']}",
        f"cost {milestone['cost']}",
    ]
    profile = milestone.get("reference_profile")
    if profile:
        facts.append(f"reference profile {profile['support']}")
    return f"**Milestone:** {milestone['id']} {milestone['title']} ({', '.join(facts)})"


def render_title(epic):
    """Return the issue title of one epic."""
    return f"{epic['id']} {epic['title']}"


def render_body(milestone, epic):
    """Return the issue body of one epic."""
    blockers = ", ".join(milestone["blocked_by"]) or "none"
    lines = [
        milestone_line(milestone),
        f"**Blocked by:** {blockers}",
        "",
        "## Acceptance",
        epic["acceptance"],
        "",
    ]
    requirements = epic.get("requirement_ids", [])
    if requirements:
        joined = ", ".join(requirements)
        lines += [f"**Requirements:** {joined} (see docs/roadmap/requirements.md)", ""]
    profile = milestone.get("reference_profile")
    lines.append(f"{FOOTER} {PROFILE_NOTE}" if profile else FOOTER)
    return "\n".join(lines) + "\n"


def first_difference(actual, expected):
    """Return a short description of the first line where two bodies differ."""
    have = actual.replace("\r\n", "\n").rstrip("\n").split("\n")
    want = expected.rstrip("\n").split("\n")
    for index in range(max(len(have), len(want))):
        got = have[index] if index < len(have) else "<missing>"
        wanted = want[index] if index < len(want) else "<missing>"
        if got != wanted:
            return f"line {index + 1}: has {got!r}, roadmap renders {wanted!r}"
    return None


def issue_findings(issue, rows, seen):
    """Return the findings for one open epic issue."""
    number = issue["number"]
    epic_id = issue["title"].split(" ", 1)[0]
    if epic_id not in rows:
        return [f"#{number}: title {issue['title']!r} names no epic in {ROADMAP}"]
    if epic_id in seen:
        return [f"#{number}: epic {epic_id} is also open as #{seen[epic_id]}"]
    seen[epic_id] = number
    milestone, epic = rows[epic_id]
    findings = []
    title = render_title(epic)
    if issue["title"] != title:
        findings.append(f"#{number}: title {issue['title']!r}, roadmap renders {title!r}")
    difference = first_difference(issue["body"], render_body(milestone, epic))
    if difference:
        findings.append(f"#{number} {epic_id}: body {difference}")
    return findings


def check(issues, roadmap):
    """Return every finding over the open epic issues, in issue order."""
    rows = epic_rows(roadmap)
    seen = {}
    findings = []
    for issue in sorted(issues, key=lambda row: row["number"]):
        findings += issue_findings(issue, rows, seen)
    return findings


def gh(arguments):
    """Run gh with a timeout and return its standard output."""
    try:
        result = subprocess.run(
            ["gh", *arguments],
            capture_output=True,
            check=True,
            text=True,
            timeout=GH_TIMEOUT,
        )
    except (OSError, subprocess.SubprocessError) as error:
        detail = getattr(error, "stderr", "") or error
        raise EpicError(f"gh {arguments[0]} {arguments[1]} failed: {detail}") from error
    return result.stdout


def fetch_issues():
    """Return the open roadmap issues, refusing a list that reached the limit."""
    output = gh(
        [
            "issue",
            "list",
            "-R",
            REPOSITORY,
            "--label",
            LABEL,
            "--state",
            "open",
            "--limit",
            str(MAX_ISSUES),
            "--json",
            "number,title,body",
        ]
    )
    issues = json.loads(output)
    if len(issues) >= MAX_ISSUES:
        raise EpicError(f"{len(issues)} open epics reach the list limit {MAX_ISSUES}")
    return issues


def apply(issues, roadmap):
    """Rewrite each issue whose title or body differs; return the numbers changed."""
    rows = epic_rows(roadmap)
    changed = []
    for issue in sorted(issues, key=lambda row: row["number"]):
        epic_id = issue["title"].split(" ", 1)[0]
        if epic_id not in rows:
            continue
        milestone, epic = rows[epic_id]
        title, body = render_title(epic), render_body(milestone, epic)
        if issue["title"] == title and not first_difference(issue["body"], body):
            continue
        number = str(issue["number"])
        gh(["issue", "edit", number, "-R", REPOSITORY, "--title", title, "--body", body])
        changed.append(issue["number"])
    return changed


def parse(argv):
    """Return the parsed command line."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    mode = parser.add_mutually_exclusive_group(required=True)
    mode.add_argument("--print", metavar="EPIC", help="print one rendered epic")
    mode.add_argument("--check", action="store_true", help="fail on a stale open epic")
    mode.add_argument("--apply", action="store_true", help="rewrite each stale open epic")
    parser.add_argument("--issues", type=pathlib.Path, help="read issues from a gh JSON dump")
    parser.add_argument("--roadmap", type=pathlib.Path, default=ROOT / ROADMAP)
    return parser.parse_args(argv)


def run(args):
    """Carry out the selected mode and return the exit status."""
    roadmap = read_json(args.roadmap)
    if args.print:
        rows = epic_rows(roadmap)
        if args.print not in rows:
            raise EpicError(f"no epic {args.print} in {ROADMAP}")
        milestone, epic = rows[args.print]
        print(render_title(epic))
        print(render_body(milestone, epic), end="")
        return 0
    issues = read_json(args.issues) if args.issues else fetch_issues()
    if args.apply:
        changed = apply(issues, roadmap)
        print(f"rewrote {len(changed)} of {len(issues)} open epics: {changed}")
        issues = fetch_issues()
    findings = check(issues, roadmap)
    for finding in findings:
        print(f"FAIL: {finding}", file=sys.stderr)
    if findings:
        return 1
    print(f"PASS: {len(issues)} open epics match {ROADMAP}.")
    return 0


def main(argv):
    """Run the command line and report a failure to read as exit 1."""
    args = parse(argv[1:])
    try:
        return run(args)
    except (EpicError, KeyError, TypeError) as error:
        print(f"FAIL: {error!r}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main(sys.argv))
