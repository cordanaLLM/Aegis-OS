#!/usr/bin/env python3
"""Positive, negative and boundary cases for tools/render_epics.py (issue #220)."""

import contextlib
import copy
import io
import json
import pathlib
import sys
import tempfile
import unittest
from unittest import mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import render_epics as re_  # noqa: E402

REPO = pathlib.Path(__file__).resolve().parent.parent

ROADMAP = {
    "milestones": [
        {
            "id": "M07",
            "title": "Planted milestone",
            "rank": 4,
            "state": "ready",
            "cost": "small",
            "blocked_by": ["M01", "M02"],
            "reference_profile": {"support": "full"},
            "epics": [
                {
                    "id": "E07-1",
                    "title": "Planted epic",
                    "requirement_ids": ["REQ-A-01", "REQ-B-02"],
                    "acceptance": "Positive: it works. Negative: it refuses.",
                }
            ],
        },
        {
            "id": "M08",
            "title": "Bare milestone",
            "rank": 5,
            "state": "blocked",
            "cost": "large",
            "blocked_by": [],
            "epics": [{"id": "E08-1", "title": "Bare epic", "acceptance": "Boundary only."}],
        },
    ]
}

PLANTED_BODY = (
    "**Milestone:** M07 Planted milestone (rank 4, state ready, cost small, "
    "reference profile full)\n"
    "**Blocked by:** M01, M02\n"
    "\n"
    "## Acceptance\n"
    "Positive: it works. Negative: it refuses.\n"
    "\n"
    "**Requirements:** REQ-A-01, REQ-B-02 (see docs/roadmap/requirements.md)\n"
    "\n"
    "Blocking states are maintained in `planning/roadmap.json`; `make readiness` "
    "lists the ready set. Close this epic only with committed evidence. "
    "A pass on the reference profile is development evidence and closes no gate.\n"
)


def rendered_issues(roadmap=ROADMAP):
    """Return the open issues exactly as the renderer writes them."""
    issues = []
    for number, (milestone, epic) in enumerate(re_.epic_rows(roadmap).values(), start=100):
        body = re_.render_body(milestone, epic)
        issues.append({"number": number, "title": re_.render_title(epic), "body": body})
    return issues


class Render(unittest.TestCase):
    """The render follows the committed epic shape."""

    def test_planted_epic_renders_exactly(self):
        milestone, epic = re_.epic_rows(ROADMAP)["E07-1"]
        self.assertEqual(re_.render_title(epic), "E07-1 Planted epic")
        self.assertEqual(re_.render_body(milestone, epic), PLANTED_BODY)

    def test_bare_milestone_has_no_profile_requirements_or_blockers(self):
        milestone, epic = re_.epic_rows(ROADMAP)["E08-1"]
        body = re_.render_body(milestone, epic)
        self.assertIn("(rank 5, state blocked, cost large)\n", body)
        self.assertIn("**Blocked by:** none\n", body)
        self.assertNotIn("**Requirements:**", body)
        self.assertNotIn(re_.PROFILE_NOTE, body)
        self.assertTrue(body.endswith(re_.FOOTER + "\n"))

    def test_every_committed_epic_renders(self):
        roadmap = json.loads((REPO / re_.ROADMAP).read_text(encoding="utf-8"))
        rows = re_.epic_rows(roadmap)
        self.assertGreater(len(rows), 0)
        for milestone, epic in rows.values():
            self.assertTrue(re_.render_body(milestone, epic).startswith("**Milestone:** M"))

    def test_duplicate_epic_id_is_refused(self):
        roadmap = copy.deepcopy(ROADMAP)
        roadmap["milestones"][1]["epics"][0]["id"] = "E07-1"
        with self.assertRaisesRegex(re_.EpicError, "E07-1 appears twice"):
            re_.epic_rows(roadmap)


class Check(unittest.TestCase):
    """The check passes the rendered issues and refuses each kind of drift."""

    def test_rendered_issues_pass(self):
        self.assertEqual(re_.check(rendered_issues(), ROADMAP), [])

    def test_planted_stale_rank_is_refused(self):
        issues = rendered_issues()
        issues[0]["body"] = issues[0]["body"].replace("rank 4,", "rank 3,")
        findings = re_.check(issues, ROADMAP)
        self.assertEqual(len(findings), 1)
        self.assertIn("#100 E07-1: body line 1: has", findings[0])
        self.assertIn("rank 3,", findings[0])

    def test_stale_state_and_blockers_are_refused(self):
        issues = rendered_issues()
        issues[1]["body"] = issues[1]["body"].replace("state blocked", "state ready")
        issues[0]["body"] = issues[0]["body"].replace("M01, M02", "M01")
        findings = re_.check(issues, ROADMAP)
        self.assertEqual(len(findings), 2)
        self.assertIn("#100 E07-1: body line 2:", findings[0])
        self.assertIn("#101 E08-1: body line 1:", findings[1])

    def test_stale_title_is_refused(self):
        issues = rendered_issues()
        issues[0]["title"] = "E07-1 Old name"
        findings = re_.check(issues, ROADMAP)
        self.assertEqual(
            findings, ["#100: title 'E07-1 Old name', roadmap renders 'E07-1 Planted epic'"]
        )

    def test_unknown_epic_is_refused(self):
        issues = rendered_issues() + [{"number": 7, "title": "E99-9 Gone", "body": ""}]
        findings = re_.check(issues, ROADMAP)
        self.assertEqual(findings, [f"#7: title 'E99-9 Gone' names no epic in {re_.ROADMAP}"])

    def test_second_open_issue_for_one_epic_is_refused(self):
        issues = rendered_issues()
        issues.append(dict(issues[0], number=150))
        findings = re_.check(issues, ROADMAP)
        self.assertEqual(findings, ["#150: epic E07-1 is also open as #100"])

    def test_missing_and_extra_lines_are_refused(self):
        issues = rendered_issues()
        issues[0]["body"] = issues[0]["body"].rsplit("\n\n", 1)[0] + "\n"
        issues[1]["body"] += "Hand-written note.\n"
        findings = re_.check(issues, ROADMAP)
        self.assertIn("line 8: has '<missing>', roadmap renders ''", findings[0])
        self.assertIn("roadmap renders '<missing>'", findings[1])

    def test_crlf_and_trailing_newlines_pass(self):
        issues = rendered_issues()
        issues[0]["body"] = issues[0]["body"].replace("\n", "\r\n") + "\r\n"
        self.assertEqual(re_.check(issues, ROADMAP), [])


class Apply(unittest.TestCase):
    """Apply rewrites only the issues that differ."""

    def test_only_stale_issues_are_edited(self):
        issues = rendered_issues()
        issues[1]["body"] = issues[1]["body"].replace("rank 5", "rank 9")
        issues.append({"number": 7, "title": "E99-9 Gone", "body": ""})
        with mock.patch.object(re_, "gh", return_value="") as gh:
            changed = re_.apply(issues, ROADMAP)
        self.assertEqual(changed, [101])
        arguments = gh.call_args.args[0]
        self.assertEqual(arguments[:5], ["issue", "edit", "101", "-R", re_.REPOSITORY])
        self.assertEqual(arguments[-1], re_.render_body(*re_.epic_rows(ROADMAP)["E08-1"]))


class Main(unittest.TestCase):
    """The command line exits 0 on a match and 1 on drift or a read failure."""

    def run_main(self, *argv):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = re_.main(["render_epics.py", *argv])
        return code, out.getvalue(), err.getvalue()

    def write(self, root, name, data):
        path = root / name
        path.write_text(json.dumps(data), encoding="utf-8")
        return str(path)

    def test_check_from_file_passes_and_fails(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            roadmap = self.write(root, "roadmap.json", ROADMAP)
            issues = rendered_issues()
            good = self.write(root, "good.json", issues)
            issues[0]["body"] = issues[0]["body"].replace("rank 4,", "rank 3,")
            stale = self.write(root, "stale.json", issues)
            passed = self.run_main("--check", "--issues", good, "--roadmap", roadmap)
            failed = self.run_main("--check", "--issues", stale, "--roadmap", roadmap)
        self.assertEqual(passed[0], 0)
        self.assertIn("PASS: 2 open epics match", passed[1])
        self.assertEqual(failed[0], 1)
        self.assertIn("FAIL: #100 E07-1: body line 1:", failed[2])

    def test_print_renders_one_epic(self):
        with tempfile.TemporaryDirectory() as tmp:
            roadmap = self.write(pathlib.Path(tmp), "roadmap.json", ROADMAP)
            code, out, _ = self.run_main("--print", "E07-1", "--roadmap", roadmap)
            missing = self.run_main("--print", "E99-9", "--roadmap", roadmap)
        self.assertEqual(code, 0)
        self.assertEqual(out, "E07-1 Planted epic\n" + PLANTED_BODY)
        self.assertEqual(missing[0], 1)
        self.assertIn("no epic E99-9", missing[2])

    def test_unreadable_inputs_fail(self):
        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            roadmap = self.write(root, "roadmap.json", ROADMAP)
            broken = root / "broken.json"
            broken.write_text("{", encoding="utf-8")
            missing = self.run_main("--check", "--issues", str(root / "none.json"))
            garbled = self.run_main("--check", "--issues", str(broken), "--roadmap", roadmap)
        self.assertEqual(missing[0], 1)
        self.assertIn("invalid or oversized file", missing[2])
        self.assertEqual(garbled[0], 1)
        self.assertIn("broken.json", garbled[2])

    def test_apply_rewrites_then_checks_the_reread_issues(self):
        stale = rendered_issues()
        stale[0]["body"] = stale[0]["body"].replace("rank 4,", "rank 3,")
        listed = []

        def gh(arguments):
            if arguments[1] == "list":
                return json.dumps(listed.pop(0))
            return ""

        with tempfile.TemporaryDirectory() as tmp:
            root = pathlib.Path(tmp)
            roadmap = self.write(root, "roadmap.json", ROADMAP)
            issues = self.write(root, "issues.json", stale)
            with mock.patch.object(re_, "gh", side_effect=gh) as called:
                listed.extend([rendered_issues(), stale])
                fixed = self.run_main("--apply", "--issues", issues, "--roadmap", roadmap)
                still = self.run_main("--apply", "--issues", issues, "--roadmap", roadmap)
        edits = [c.args[0][:3] for c in called.call_args_list if c.args[0][1] == "edit"]
        self.assertEqual(edits, [["issue", "edit", "100"], ["issue", "edit", "100"]])
        self.assertEqual(fixed[0], 0)
        self.assertIn("rewrote 1 of 2 open epics: [100]", fixed[1])
        self.assertEqual(still[0], 1)
        self.assertIn("FAIL: #100 E07-1: body line 1:", still[2])

    def test_gh_failure_fails_loudly(self):
        with mock.patch.object(re_.subprocess, "run", side_effect=OSError("no gh")):
            code, _, err = self.run_main("--check")
        self.assertEqual(code, 1)
        self.assertIn("gh issue list failed", err)


WORKFLOW = REPO / ".github" / "workflows" / "roadmap-epics.yml"


def job_block(text, job):
    """Return the lines of one job in the workflow, up to the next job."""
    lines = text.splitlines()
    start = lines.index(f"  {job}:")
    end = next(
        (n for n in range(start + 1, len(lines)) if lines[n][:3].strip() and lines[n][2] != " "),
        len(lines),
    )
    return lines[start:end]


def run_lines(block):
    """Return the render_epics.py command of each run step in a job, in order."""
    return [line.split("run:", 1)[1].strip() for line in block if "run: python3" in line]


class Workflow(unittest.TestCase):
    """The push job rewrites the epics before it checks them; only it may write."""

    def test_push_job_applies_then_checks_with_job_scoped_write(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        block = job_block(text, "apply")
        self.assertIn("    if: github.event_name == 'push'", block)
        self.assertIn("      issues: write", block)
        self.assertEqual(
            run_lines(block),
            ["python3 tools/render_epics.py --apply", "python3 tools/render_epics.py --check"],
        )

    def test_no_other_scope_may_write_issues(self):
        text = WORKFLOW.read_text(encoding="utf-8")
        head = [line for line in text.split("\njobs:\n", 1)[0].splitlines() if line[:1] != "#"]
        check = job_block(text, "check")
        self.assertNotIn(": write", "\n".join(head))
        self.assertNotIn(": write", "\n".join(check))
        self.assertIn("    if: github.event_name != 'push'", check)
        self.assertEqual(run_lines(check), ["python3 tools/render_epics.py --check"])

    def test_job_block_stops_at_the_next_job(self):
        text = "jobs:\n  apply:\n    a: 1\n  check:\n    b: 2\n"
        self.assertEqual(job_block(text, "apply"), ["  apply:", "    a: 1"])
        self.assertEqual(job_block(text, "check"), ["  check:", "    b: 2"])


if __name__ == "__main__":
    unittest.main()
