#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative and boundary coverage for the praetor pin auto-bump (HISS-15).

The bump itself fetches, builds, adopts and pushes; none of that runs here. What
runs is every decision it makes on the way -- reading and moving the pin, the
admission row, when there is nothing to do, which files go back to main, which
adopt changes survive, what the commit and pull request say, and that two runs
never overlap -- with every external command faked. The sweeps at the end hold
the module to HISS-02 (every process under a deadline, through one helper) and
HISS-04 (function length and complexity), because the audit measures neither for
Python.
"""

import ast
import datetime
import io
import signal
import subprocess
import sys
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import host
import praetor_bump as bump

ROOT = Path(__file__).resolve().parent.parent
SOURCE = ROOT / "tools" / "praetor_bump.py"
UNITS = ROOT / "tools" / "praetor-bump"
OLD = "732589684b245fec347ae31c62bb59f4c40a2c9d"
NEW = "95ce800ec1f0c0b8a423e30798de9eb36e1492a1"
BASE = "aa851afcb718b9ce4ca54675d31ed8854226f192"
PLAN = bump.Plan(NEW, OLD, BASE)
TODAY = datetime.date(2026, 9, 28)
# Scalar bounds the sweeps below apply (HISS-02, HISS-04).
MAX_FUNCTION_LINES = 60
MAX_COMPLEXITY = 10
BRANCHES = (ast.If, ast.For, ast.While, ast.IfExp, ast.ExceptHandler, ast.With, ast.Assert)

WORKFLOW = f"""env:
  MARKDOWNLINT_VERSION: "0.23.2"
  # Published Praetor commit that builds the praetorctl this gate pins.
  PRAETOR_COMMIT: "{OLD}"
jobs: {{}}
"""


def admission_page(pin=OLD):
    """Return a two-row admission table in the page's real shape."""
    return (
        "# Toolchain admission matrix\n\n"
        "Status: recorded admissions, current as of 2026-09-13\n\n"
        "| Tool | Pinned version | How it is pinned | Replaces | Admitted by |\n"
        "| :--- | :--- | :--- | :--- | :--- |\n"
        f"| praetorctl | source commit `{pin}` | `.github/workflows/ci.yml`, `PRAETOR_COMMIT`, "
        "verified with `git rev-parse` before the build | nothing | M00 |\n"
        "| lefthook | 2.1.12 | `.github/workflows/ci.yml` | nothing | M00 |\n"
    )


def args(**overrides):
    """Return parsed options pointing at a throwaway cache."""
    options = bump.parse_args(["--cache-dir", overrides.pop("cache", "cache")])
    for key, value in overrides.items():
        setattr(options, key, value)
    return options


def complexity(function):
    """Return a McCabe-style count: one plus each branch and each extra boolean operand."""
    count = 1
    for node in ast.walk(function):
        if isinstance(node, BRANCHES):
            count += 1
        elif isinstance(node, ast.BoolOp):
            count += len(node.values) - 1
        elif isinstance(node, ast.comprehension):
            count += 1 + len(node.ifs)
    return count


class PinTests(unittest.TestCase):
    """PRAETOR_COMMIT is read and moved in ci.yml, and nothing else changes."""

    def test_the_pin_is_read_and_moved(self):
        self.assertEqual(bump.read_pin(WORKFLOW), OLD)
        moved = bump.rewrite_pin(WORKFLOW, NEW)
        self.assertEqual(bump.read_pin(moved), NEW)
        self.assertEqual(moved.replace(NEW, OLD), WORKFLOW)

    def test_the_committed_workflow_pins_what_the_admission_page_records(self):
        """Positive, on the real files: the register and the workflow agree."""
        pin = bump.read_pin(bump.read_exact(ROOT / bump.CI_WORKFLOW))
        page = (ROOT / bump.ADMISSION_PAGE).read_text(encoding="utf-8")
        self.assertIn(f"| praetorctl | source commit `{pin}` |", page)
        self.assertTrue(
            bump.ci_env(bump.read_exact(ROOT / bump.CI_WORKFLOW), "MARKDOWNLINT_VERSION")
        )

    def test_a_short_or_upper_case_pin_is_refused(self):
        for bad in (OLD[:7], OLD.upper(), OLD + "0", ""):
            with self.subTest(bad=bad):
                with self.assertRaises(bump.BumpError):
                    bump.read_pin(WORKFLOW.replace(OLD, bad))
                with self.assertRaises(bump.BumpError):
                    bump.rewrite_pin(WORKFLOW, bad)

    def test_a_missing_or_doubled_pin_is_refused(self):
        """Boundary: exactly one PRAETOR_COMMIT line, never zero or two."""
        doubled = WORKFLOW + f'  PRAETOR_COMMIT: "{OLD}"\n'
        for text in (WORKFLOW.replace("PRAETOR_COMMIT", "PRAETOR_REF"), doubled):
            with self.subTest(text=text[-60:]):
                with self.assertRaises(bump.BumpError):
                    bump.rewrite_pin(text, NEW)

    def test_the_comment_naming_the_pin_is_not_the_pin(self):
        """Boundary: only the `PRAETOR_COMMIT:` key line counts."""
        commented = WORKFLOW.replace("# Published", f"# PRAETOR_COMMIT {NEW} was published")
        self.assertEqual(bump.read_pin(commented), OLD)


class AdmissionRowTests(unittest.TestCase):
    """The praetorctl row moves to the new pin and names the old one in Replaces."""

    def test_the_row_moves_and_the_old_pin_becomes_replaces(self):
        page = bump.rewrite_admission(admission_page(), OLD, NEW, TODAY)
        row = next(line for line in page.splitlines() if line.startswith("| praetorctl |"))
        cells = [cell.strip() for cell in row.split("|")]
        self.assertEqual(cells[2], f"source commit `{NEW}`")
        self.assertIn(f"the previous pin, source commit `{OLD}`", cells[4])
        self.assertTrue(cells[3].startswith("`.github/workflows/ci.yml`"))
        self.assertEqual(cells[5], "M00")
        self.assertIn("current as of 2026-09-28", page)
        self.assertIn("| lefthook | 2.1.12 |", page)

    def test_the_real_page_rewrites(self):
        """Positive, on the committed page: the row the bump edits is where it expects."""
        page = (ROOT / bump.ADMISSION_PAGE).read_text(encoding="utf-8")
        pin = bump.read_pin(bump.read_exact(ROOT / bump.CI_WORKFLOW))
        rewritten = bump.rewrite_admission(page, pin, NEW, TODAY)
        self.assertEqual(len(rewritten.splitlines()), len(page.splitlines()))
        self.assertIn(f"| praetorctl | source commit `{NEW}` |", rewritten)

    def test_a_row_naming_another_pin_is_refused(self):
        """Negative: a register that disagrees with ci.yml is a RED, not a silent fix."""
        with self.assertRaises(bump.BumpError):
            bump.rewrite_admission(admission_page(pin=NEW), OLD, NEW, TODAY)

    def test_zero_or_two_rows_are_refused(self):
        page = admission_page()
        row = next(line for line in page.splitlines() if line.startswith("| praetorctl |"))
        for text in (page.replace(row + "\n", ""), page + row + "\n"):
            with self.subTest(rows=text.count("| praetorctl |")):
                with self.assertRaises(bump.BumpError):
                    bump.rewrite_admission(text, OLD, NEW, TODAY)

    def test_a_page_without_a_status_date_keeps_its_text(self):
        """Boundary: the status date is refreshed when present, never invented."""
        page = admission_page().replace(
            "Status: recorded admissions, current as of 2026-09-13\n", ""
        )
        rewritten = bump.rewrite_admission(page, OLD, NEW, TODAY)
        self.assertNotIn("Status:", rewritten)


class DecisionTests(unittest.TestCase):
    """When a run has work, and what it does with the rolling pull request."""

    def pr(self, number, target, base=BASE):
        body = f"text\n\n{bump.TARGET_MARKER}{target}\n{bump.BASE_MARKER}{base}\n"
        return {"number": number, "body": body, "url": ""}

    def test_an_equal_pin_is_up_to_date(self):
        self.assertEqual(bump.decide(NEW, NEW, BASE, []), ("up-to-date", None))
        self.assertEqual(bump.decide(NEW, NEW, BASE, [self.pr(7, OLD)]), ("up-to-date", None))

    def test_an_open_pull_request_for_the_target_means_nothing_to_do(self):
        """Idempotency: the same head on the same main, already proposed, does nothing."""
        self.assertEqual(bump.decide(OLD, NEW, BASE, [self.pr(9, NEW)]), ("proposed", 9))

    def test_an_older_open_pull_request_is_updated_and_none_is_created(self):
        self.assertEqual(bump.decide(OLD, NEW, BASE, [self.pr(9, BASE)]), ("update", 9))
        self.assertEqual(bump.decide(OLD, NEW, BASE, []), ("create", None))

    def test_a_moved_aegis_main_rebuilds_the_open_pull_request(self):
        """Negative: the same Praetor head built on an older Aegis main is rebuilt."""
        self.assertEqual(bump.decide(OLD, NEW, BASE, [self.pr(9, NEW, base=OLD)]), ("update", 9))

    def test_a_body_without_a_base_line_is_rebuilt(self):
        """Boundary: a pull request that records no Aegis main is never taken as current."""
        body = {"number": 4, "body": f"{bump.TARGET_MARKER}{NEW}\n", "url": ""}
        self.assertEqual(bump.decide(OLD, NEW, BASE, [body]), ("update", 4))

    def test_an_abbreviated_marker_does_not_count_as_proposed(self):
        """Boundary: only the full commit id matches; a prefix is another commit."""
        target = {"number": 4, "body": f"{bump.TARGET_MARKER}{NEW[:7]}", "url": ""}
        self.assertEqual(bump.decide(OLD, NEW, BASE, [target]), ("update", 4))
        short = f"{bump.TARGET_MARKER}{NEW}\n{bump.BASE_MARKER}{BASE[:7]}\n"
        base = {"number": 5, "body": short, "url": ""}
        self.assertEqual(bump.decide(OLD, NEW, BASE, [base]), ("update", 5))

    def test_an_invalid_commit_is_refused(self):
        with self.assertRaises(bump.BumpError):
            bump.decide("main", NEW, BASE, [])
        with self.assertRaises(bump.BumpError):
            bump.decide(OLD, NEW[:12], BASE, [])
        with self.assertRaises(bump.BumpError):
            bump.decide(OLD, NEW, "HEAD", [])

    def test_gh_output_is_parsed_or_refused(self):
        parsed = bump.parse_prs('[{"number": 3, "body": "b", "url": "u"}, {"body": "x"}]')
        self.assertEqual([entry["number"] for entry in parsed], [3])
        self.assertEqual(bump.parse_prs(""), [])
        for text in ("not json", '{"number": 3}'):
            with self.subTest(text=text):
                with self.assertRaises(bump.BumpError):
                    bump.parse_prs(text)

    def test_the_created_pull_request_number_is_read_from_its_url(self):
        self.assertEqual(bump.pr_number("https://github.com/o/r/pull/132\n"), 132)
        with self.assertRaises(bump.BumpError):
            bump.pr_number("https://github.com/o/r/issues/132")


class HandMaintainedTests(unittest.TestCase):
    """The named list decides which files always go back to main."""

    def test_listed_files_directories_and_backups_match(self):
        paths = [
            "lefthook.yml",
            ".config/agent/hooks/block_evasion.py",
            ".vscode/settings.json",
            ".idea/inspectionProfiles/standards.xml",
            "lua/standards.lua",
            "tools/markdownlint/verify.mjs.bak",
            "README.md",
            ".standards.lock",
        ]
        self.assertEqual(
            bump.hand_maintained(paths),
            sorted(paths[:6]),
        )

    def test_near_misses_do_not_match(self):
        """Negative: a prefix without its separator, or a longer name, is another file."""
        for path in (".vscode-extra/x.json", "lefthook.yml.orig", "lua", "AGENTS.md", "x.bak/y"):
            with self.subTest(path=path):
                self.assertFalse(bump.is_hand_maintained(path))

    def test_every_registration_agents_md_declares_is_listed(self):
        """Positive, on the real AGENTS.md: the six hook registrations stay hand-maintained."""
        text = (ROOT / "AGENTS.md").read_text(encoding="utf-8")
        start = text.index("| Client | Registration file |")
        table = text[start:].split("\n\n", 1)[0].splitlines()[2:]
        files = [row.split("|")[2].strip().strip("`") for row in table]
        self.assertEqual(len(files), 6)
        for path in files:
            with self.subTest(path=path):
                self.assertTrue(bump.is_hand_maintained(path))

    def test_an_empty_change_set_selects_nothing(self):
        self.assertEqual(bump.hand_maintained([]), [])

    def test_a_stamp_only_baseline_is_recognised(self):
        before = '{"version": 1, "generated_at": "a", "commit_sha": "b", "infractions": []}'
        after = '{"version": 1, "generated_at": "c", "commit_sha": "d", "infractions": []}'
        self.assertTrue(bump.stamp_only(before, after))
        self.assertTrue(bump.stamp_only(before, before))
        self.assertFalse(bump.stamp_only(before, after.replace('"infractions": []', '"x": 1')))
        self.assertFalse(bump.stamp_only(before, "not json"))
        self.assertFalse(bump.stamp_only("[]", "[]"))


class StatusTests(unittest.TestCase):
    """`git status --porcelain=v1 -z` parsing."""

    def test_modified_untracked_and_deleted_entries_parse(self):
        text = " M lefthook.yml\0?? a/b.bak\0 D gone.txt\0"
        self.assertEqual(
            bump.parse_status(text),
            {"lefthook.yml": " M", "a/b.bak": "??", "gone.txt": " D"},
        )

    def test_a_rename_consumes_its_origin_field(self):
        self.assertEqual(
            bump.parse_status("R  new.txt\0old.txt\0 M x\0"), {"new.txt": "R ", "x": " M"}
        )

    def test_empty_output_is_no_change(self):
        self.assertEqual(bump.parse_status(""), {})


class MinimiseTests(unittest.TestCase):
    """Only what a gate fails without survives, including files that pass only together."""

    def model(self, needed=(), coupled=()):
        """Return (try_revert, reverted set, calls) over a fake gate.

        The gate fails when a needed file is reverted, or when exactly one file of
        a coupled pair is.
        """
        reverted, calls = set(), []

        def passes(state):
            if state & set(needed):
                return False
            return all((a in state) == (b in state) for a, b in coupled)

        def try_revert(paths):
            calls.append(tuple(paths))
            if passes(reverted | set(paths)):
                reverted.update(paths)
                return True
            return False

        return try_revert, reverted, calls

    def test_nothing_needed_reverts_everything_in_one_try(self):
        try_revert, reverted, calls = self.model()
        self.assertEqual(bump.minimise(["a", "b", "c"], try_revert), [])
        self.assertEqual(reverted, {"a", "b", "c"})
        self.assertEqual(len(calls), 1)

    def test_a_needed_file_is_kept(self):
        try_revert, reverted, _calls = self.model(needed=["b"])
        self.assertEqual(bump.minimise(["a", "b", "c"], try_revert), ["b"])
        self.assertEqual(reverted, {"a", "c"})

    def test_a_pair_that_passes_only_together_is_reverted_together(self):
        """The harness and its register digest: neither alone, both together."""
        try_revert, reverted, _calls = self.model(needed=["lock"], coupled=[("harness", "yaml")])
        kept = bump.minimise(["harness", "lock", "readme", "yaml"], try_revert)
        self.assertEqual(kept, ["lock"])
        self.assertEqual(reverted, {"harness", "readme", "yaml"})

    def test_a_needed_pair_is_kept(self):
        try_revert, _reverted, _calls = self.model(needed=["a"], coupled=[("a", "b")])
        self.assertEqual(bump.minimise(["a", "b", "c"], try_revert), ["a", "b"])

    def test_no_candidates_runs_no_gate(self):
        try_revert, _reverted, calls = self.model()
        self.assertEqual(bump.minimise([], try_revert), [])
        self.assertEqual(calls, [])

    def test_candidates_past_the_bound_are_kept_untried(self):
        """Boundary: the bound keeps files rather than dropping them."""
        paths = [f"f{index:04d}" for index in range(bump.MAX_CANDIDATES + 2)]
        try_revert, reverted, _calls = self.model()
        self.assertEqual(bump.minimise(paths, try_revert), paths[-2:])
        self.assertEqual(len(reverted), bump.MAX_CANDIDATES)


class MessageTests(unittest.TestCase):
    """The commit message and pull request body the bump writes."""

    def test_the_commit_message_is_conventional_and_names_the_target(self):
        message = bump.commit_message(
            PLAN, (2, ["abc1234 fix: one", "def5678 feat: two"]), ["a"], [("b", "hand-maintained")]
        )
        lines = message.splitlines()
        self.assertEqual(lines[0], f"chore(governance): bump praetor to {NEW[:7]}")
        self.assertEqual(lines[1], "")
        self.assertIn("- abc1234 fix: one", lines)
        self.assertIn("- a", lines)
        self.assertIn("- b (hand-maintained)", lines)
        self.assertEqual(bump.proposed_target(message), NEW)
        self.assertEqual(bump.proposed_base(message), BASE)
        self.assertNotIn("Signed-off-by", message)

    def test_the_commit_prose_is_wrapped(self):
        """Boundary: every line but a listed item fits the conventional width."""
        message = bump.commit_message(PLAN, (123, ["abc1234 " + "x" * 90]), ["a"], [])
        prose = [line for line in message.splitlines() if not line.startswith("- ")]
        self.assertTrue(all(len(line) <= bump.COMMIT_WIDTH for line in prose), prose)

    def test_nothing_kept_is_said_rather_than_left_blank(self):
        message = bump.commit_message(PLAN, (0, []), [], [])
        self.assertIn("- nothing beyond the pin", message)
        self.assertIn("Reverted to the content on main:\n- nothing", message)

    def test_the_commit_list_is_bounded_with_the_remainder_counted(self):
        """Boundary: exactly the bound lists everything; one more adds a count line."""
        lines = [f"{index:07x} subject" for index in range(bump.MAX_LISTED_COMMITS + 1)]
        self.assertEqual(bump.listed_commits(bump.MAX_LISTED_COMMITS, lines[:-1]), lines[:-1])
        listed = bump.listed_commits(bump.MAX_LISTED_COMMITS + 1, lines)
        self.assertEqual(listed[-1], "... and 1 more")
        self.assertEqual(len(listed), bump.MAX_LISTED_COMMITS + 1)

    def test_the_compare_output_parses_or_is_refused(self):
        self.assertEqual(
            bump.parse_compare("2\nabc1234 one\ndef5678 two\n"), (2, ["abc1234 one", "def5678 two"])
        )
        for text in ("", "abc1234 one\n"):
            with self.subTest(text=text):
                with self.assertRaises(bump.BumpError):
                    bump.parse_compare(text)

    def test_the_pull_request_body_carries_the_marker_and_the_checklist(self):
        report = bump.Report(["x"], [], (1, ["abc1234 one"]), ["SKIP: no tool"], "")
        body = bump.pr_body(PLAN, report)
        self.assertEqual(bump.proposed_target(body), NEW)
        self.assertEqual(bump.proposed_base(body), BASE)
        self.assertIn("## Pre-Merge Verification Checklist", body)
        self.assertIn("- SKIP: no tool", body)
        self.assertIn(bump.ROLLING_BRANCH, body)

    def test_skip_lines_are_collected_and_bounded(self):
        text = "\n".join(["PASS: a", "SKIP: b"] + ["SKIP: c"] * (bump.MAX_SKIP_LINES + 5))
        skips = bump.skip_lines(text)
        self.assertEqual(skips[0], "SKIP: b")
        self.assertEqual(len(skips), bump.MAX_SKIP_LINES)


class HookSnapshotTests(unittest.TestCase):
    """adopt's `lefthook install` must leave the shared hooks as it found them."""

    def test_a_rewritten_hook_is_restored_and_a_new_one_removed(self):
        with tempfile.TemporaryDirectory() as base:
            hooks = Path(base)
            (hooks / "pre-commit").write_text("original\n", encoding="utf-8")
            saved = bump.snapshot(hooks)
            (hooks / "pre-commit").write_text("adopt's\n", encoding="utf-8")
            (hooks / "commit-msg").write_text("new\n", encoding="utf-8")
            bump.restore_snapshot(hooks, saved)
            self.assertEqual((hooks / "pre-commit").read_text(encoding="utf-8"), "original\n")
            self.assertFalse((hooks / "commit-msg").exists())

    def test_a_missing_directory_snapshots_as_empty(self):
        self.assertEqual(bump.snapshot(Path(tempfile.gettempdir()) / "aegis-no-such-hooks"), {})

    def test_too_many_files_are_refused(self):
        """Boundary: past the bound the snapshot refuses rather than truncating."""
        with tempfile.TemporaryDirectory() as base:
            for index in range(bump.MAX_HOOK_FILES + 1):
                (Path(base) / f"hook{index}").write_text("x", encoding="utf-8")
            with self.assertRaises(bump.BumpError):
                bump.snapshot(Path(base))

    def test_a_captured_file_is_put_back_or_removed(self):
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "sub" / "file"
            bump.put_back(path, (b"content", 0o644))
            self.assertEqual(bump.capture(path)[0], b"content")
            bump.put_back(path, None)
            self.assertIsNone(bump.capture(path))


class AdoptRestoreTests(unittest.TestCase):
    """adopt leaves the hooks and lefthook's checksum in the shared git directory as found."""

    def run_adopt(self, base, code=0, checksum=b"cdf5be07 1790610223\n"):
        """Run `adopt()` against a fake git directory whose adopt rewrites both files."""
        hooks, info = base / "hooks", base / "info"
        hooks.mkdir()
        info.mkdir()
        (hooks / "pre-commit").write_bytes(b"mine\n")
        if checksum is not None:
            (info / "lefthook.checksum").write_bytes(checksum)

        def fake(argv, deadline, cwd=None, env=None):
            if "rev-parse" in argv:
                return bump.Result(tuple(argv), 0, f"{base / argv[-1]}\n", "")
            (hooks / "pre-commit").write_bytes(b"adopt's\n")
            (info / "lefthook.checksum").write_bytes(b"821d7dfa 1790610258\n")
            return bump.Result(tuple(argv), code, "", "adopt failed\n" if code else "")

        session = bump.Session(args(cache=str(base / "cache")), stream=io.StringIO())
        session.binary = Path("praetorctl")
        with mock.patch.object(bump, "run", side_effect=fake):
            bump.adopt(session, base, base / "praetor")
        return (hooks / "pre-commit").read_bytes(), bump.capture(info / "lefthook.checksum")

    def test_the_hooks_and_the_checksum_are_put_back(self):
        with tempfile.TemporaryDirectory() as base:
            hook, checksum = self.run_adopt(Path(base))
        self.assertEqual(hook, b"mine\n")
        self.assertEqual(checksum[0], b"cdf5be07 1790610223\n")

    def test_a_failed_adopt_still_puts_them_back(self):
        """Negative: the restore runs in `finally`, so a RED leaves nothing rewritten."""
        with tempfile.TemporaryDirectory() as base:
            with self.assertRaises(bump.BumpError):
                self.run_adopt(Path(base), code=1)
            self.assertEqual((Path(base) / "hooks" / "pre-commit").read_bytes(), b"mine\n")
            checksum = (Path(base) / "info" / "lefthook.checksum").read_bytes()
        self.assertEqual(checksum, b"cdf5be07 1790610223\n")

    def test_a_checksum_adopt_created_is_removed(self):
        """Boundary: a checkout without a checksum is left without one."""
        with tempfile.TemporaryDirectory() as base:
            _hook, checksum = self.run_adopt(Path(base), checksum=None)
        self.assertIsNone(checksum)


class LockAndRootTests(unittest.TestCase):
    """Two runs never overlap, and the bump never runs as root."""

    def test_a_held_lock_makes_the_second_run_do_nothing(self):
        with tempfile.TemporaryDirectory() as base:
            options = args(cache=base)
            with open(Path(options.cache_dir) / "lock", "a+", encoding="utf-8") as holder:
                held, reason = host.exclusive_lock(holder)
                with mock.patch.object(bump, "parse_args", return_value=options), mock.patch.object(
                    bump, "locked"
                ) as work, mock.patch.object(host, "uid", return_value=(1000, None)), mock.patch(
                    "sys.stdout", new_callable=io.StringIO
                ) as out:
                    code = bump.main([])
                self.assertEqual(code, 0)
                work.assert_not_called()
                self.assertIn("SKIP:", out.getvalue())
                if not held:
                    self.assertIn("fcntl", reason)

    def test_a_free_lock_lets_the_run_proceed(self):
        with tempfile.TemporaryDirectory() as base:
            options = args(cache=base)
            with mock.patch.object(bump, "parse_args", return_value=options), mock.patch.object(
                bump, "locked", return_value=0
            ) as work, mock.patch.object(host, "uid", return_value=(1000, None)), mock.patch(
                "sys.stdout", new_callable=io.StringIO
            ):
                code = bump.main([])
            self.assertEqual(code, 0)
            if host.fcntl is not None:
                work.assert_called_once()
            else:
                work.assert_not_called()

    def test_root_is_refused_before_anything_runs(self):
        with tempfile.TemporaryDirectory() as base:
            options = args(cache=base)
            with mock.patch.object(bump, "parse_args", return_value=options), mock.patch.object(
                host, "uid", return_value=(0, None)
            ), mock.patch.object(bump, "locked") as work, mock.patch(
                "sys.stdout", new_callable=io.StringIO
            ) as out:
                self.assertEqual(bump.main([]), 2)
            work.assert_not_called()
            self.assertIn("never as root", out.getvalue())


class SurveyTests(unittest.TestCase):
    """The survey stops early when there is nothing to do."""

    def session(self, base, pin=OLD, prs="[]"):
        session = bump.Session(args(cache=base), stream=io.StringIO())
        outputs = {"rev-parse": NEW, "show": WORKFLOW.replace(OLD, pin), "list": prs}

        def fake(argv, deadline, cwd=None, env=None):
            key = next((word for word in ("rev-parse", "show", "list") if word in argv), "")
            text = outputs.get(key, "")
            if key == "rev-parse" and any("refs/remotes" in str(part) for part in argv):
                text = BASE
            return bump.Result(tuple(argv), 0, text + "\n", "")

        return session, fake

    def test_an_up_to_date_pin_stops_with_the_words_up_to_date(self):
        with tempfile.TemporaryDirectory() as base:
            session, fake = self.session(base, pin=NEW)
            with mock.patch.object(bump, "run", side_effect=fake):
                self.assertEqual(bump.survey(session), (None, 0))
            self.assertIn("up to date", session.stream.getvalue())

    def proposal(self, base):
        body = f"{bump.TARGET_MARKER}{NEW}\\n{bump.BASE_MARKER}{base}"
        return f'[{{"number": 5, "body": "{body}", "url": ""}}]'

    def test_an_already_proposed_head_stops(self):
        with tempfile.TemporaryDirectory() as base:
            session, fake = self.session(base, prs=self.proposal(BASE))
            with mock.patch.object(bump, "run", side_effect=fake):
                self.assertEqual(bump.survey(session), (None, 0))
            self.assertIn("#5 already proposes", session.stream.getvalue())

    def test_a_proposal_on_an_older_main_plans_a_rebuild(self):
        """Negative: Aegis main moved under the open pull request, so the bump reruns."""
        with tempfile.TemporaryDirectory() as base:
            session, fake = self.session(base, prs=self.proposal(OLD))
            with mock.patch.object(bump, "run", side_effect=fake):
                self.assertEqual(bump.survey(session), (PLAN, 0))

    def test_a_pair_that_failed_before_is_not_rerun_without_retry(self):
        with tempfile.TemporaryDirectory() as base:
            session, fake = self.session(base)
            marker = bump.failure_marker(session.cache, PLAN)
            marker.parent.mkdir(parents=True)
            marker.write_text("/evidence.log\n", encoding="utf-8")
            with mock.patch.object(bump, "run", side_effect=fake):
                self.assertEqual(bump.survey(session), (None, 1))
                session.args.retry = True
                self.assertEqual(bump.survey(session), (PLAN, 0))

    def test_a_moved_head_plans_a_bump(self):
        with tempfile.TemporaryDirectory() as base:
            session, fake = self.session(base)
            with mock.patch.object(bump, "run", side_effect=fake):
                self.assertEqual(bump.survey(session), (PLAN, 0))


class RunHelperTests(unittest.TestCase):
    """The one helper that starts processes always ends with a result, never a hang."""

    def test_a_command_that_cannot_start_is_a_result(self):
        with mock.patch("subprocess.Popen", side_effect=OSError("no such file")):
            result = bump.run(["missing-tool"], 5)
        self.assertIsNone(result.code)
        self.assertIn("could not be started", result.err)

    def test_a_deadline_ends_the_whole_session(self):
        process = mock.Mock(pid=4242)
        process.communicate.side_effect = [subprocess.TimeoutExpired("x", 5), (b"out", b"")]
        with mock.patch("subprocess.Popen", return_value=process), mock.patch.object(
            host, "end_session", return_value=(True, None)
        ) as end:
            result = bump.run(["slow"], 5)
        end.assert_called_once_with(4242)
        self.assertIsNone(result.code)
        self.assertIn("exceeded its 5s deadline", result.err)

    def test_a_finished_command_reports_its_exit_and_output(self):
        process = mock.Mock(pid=1, returncode=3)
        process.communicate.return_value = (b"out\n", b"err\n")
        with mock.patch("subprocess.Popen", return_value=process):
            result = bump.run(["tool", Path("arg")], 5)
        self.assertEqual((result.code, result.out, result.err), (3, "out\n", "err\n"))
        self.assertEqual(result.argv, ("tool", "arg"))

    def test_an_interrupt_ends_the_session_and_goes_on(self):
        """Negative: Ctrl-C or SIGTERM must not leave the child running without a deadline."""
        process = mock.Mock(pid=4242)
        process.communicate.side_effect = [KeyboardInterrupt(), (b"", b"")]
        with mock.patch("subprocess.Popen", return_value=process), mock.patch.object(
            host, "end_session", return_value=(True, None)
        ) as end:
            with self.assertRaises(KeyboardInterrupt):
                bump.run(["slow"], 5)
        end.assert_called_once_with(4242)

    def test_an_interrupted_real_child_is_gone(self):
        """Positive, with a real child: after the interrupt nothing of it is left running."""
        original = subprocess.Popen.communicate
        started = []

        def interrupted(process, *arguments, **options):
            started.append(process)
            if len(started) == 1:
                raise KeyboardInterrupt
            return original(process, *arguments, **options)

        with mock.patch.object(
            subprocess.Popen, "communicate", autospec=True, side_effect=interrupted
        ):
            with self.assertRaises(KeyboardInterrupt):
                bump.run([sys.executable, "-c", "import time; time.sleep(60)"], 600)
        self.assertIsNotNone(started[0].poll())

    def test_sigterm_becomes_a_system_exit(self):
        """Boundary: the handler exits with the shell's code for the signal."""
        with self.assertRaises(SystemExit) as caught:
            bump.terminated(signal.SIGTERM, None)
        self.assertEqual(caught.exception.code, 128 + signal.SIGTERM)

    def test_must_turns_a_failure_into_a_reason(self):
        session = bump.Session(args(), stream=io.StringIO())
        failed = bump.Result(("git",), 1, "", "fatal: bad thing\n")
        with mock.patch.object(bump, "run", return_value=failed):
            with self.assertRaises(bump.BumpError) as caught:
                session.must(["git", "status"], 5, what="git status")
        self.assertIn("git status failed (exit 1): fatal: bad thing", str(caught.exception))


class GateListTests(unittest.TestCase):
    """The bump runs the gates CI runs, with the pinned praetorctl."""

    def test_every_gate_carries_a_deadline_and_the_pinned_binary(self):
        gates = bump.gate_commands(Path("/cache/bin/praetorctl"), "0.23.2")
        names = [name for name, _argv, _deadline in gates]
        for expected in ("make verify-all", "audit", "flavor audit", "reuse lint", "black"):
            self.assertIn(expected, names)
        self.assertIn("PRAETORCTL=/cache/bin/praetorctl", [str(p) for p in gates[0][1]])
        self.assertTrue(all(deadline > 0 for _name, _argv, deadline in gates))
        self.assertIn("markdownlint-cli2@0.23.2", gates[4][1])

    def test_the_portal_build_is_a_gate_only_with_its_venv(self):
        """Positive and boundary: with a venv it runs strict; without, it is not claimed."""
        gates = bump.gate_commands(Path("/b/praetorctl"), "0.23.2", Path("/v/python"), Path("/s"))
        name, argv, deadline = gates[-1]
        self.assertEqual(name, "mkdocs build --strict")
        self.assertEqual(argv[:5], [Path("/v/python"), "-m", "mkdocs", "build", "--strict"])
        self.assertEqual(argv[-2:], ["-d", Path("/s")])
        self.assertGreater(deadline, 0)
        names = [name for name, _argv, _deadline in bump.gate_commands(Path("/b"), "0.23.2")]
        self.assertNotIn("mkdocs build --strict", names)

    def test_a_missing_portal_venv_is_a_reported_skip(self):
        """Negative: when the pinned mkdocs cannot be installed, the body says it did not run."""
        with tempfile.TemporaryDirectory() as base:
            tree = Path(base) / "tree"
            (tree / ".github" / "workflows").mkdir(parents=True)
            (tree / bump.CI_WORKFLOW).write_text(WORKFLOW, encoding="utf-8")
            session = bump.Session(args(cache=str(Path(base) / "cache")), stream=io.StringIO())
            session.binary = Path("praetorctl")
            passed = bump.Result((), 0, "", "")
            with mock.patch.object(bump, "run", return_value=passed), mock.patch.object(
                bump, "mkdocs_python", return_value=(None, "no network")
            ):
                skips = bump.run_gates(session, tree)
        self.assertEqual(skips[0], "SKIP: mkdocs build --strict did not run: no network")

    def test_the_portal_venv_is_reused_rebuilt_or_refused(self):
        with tempfile.TemporaryDirectory() as base:
            tree, cache = Path(base) / "tree", Path(base) / "cache"
            (tree / "docs").mkdir(parents=True)
            (tree / bump.DOCS_REQUIREMENTS).write_bytes(b"mkdocs==1.6.1\n")
            session = bump.Session(args(cache=str(cache)), stream=io.StringIO())
            failed = bump.Result(("pip",), 1, "", "ERROR: no network\n")
            with mock.patch.object(bump, "run", return_value=failed) as run:
                self.assertEqual(
                    bump.mkdocs_python(session, tree)[1],
                    "the pinned mkdocs was not installed (exit 1): ERROR: no network",
                )
                digest = bump.hashlib.sha256(b"mkdocs==1.6.1\n").hexdigest()
                venv = session.cache / "mkdocs-venv"
                python = venv.joinpath(*bump.VENV_PYTHON)
                python.parent.mkdir(parents=True)
                python.write_text("", encoding="utf-8")
                (venv / "requirements.sha256").write_text(digest + "\n", encoding="utf-8")
                run.reset_mock()
                self.assertEqual(bump.mkdocs_python(session, tree), (python, None))
                run.assert_not_called()
            (tree / bump.DOCS_REQUIREMENTS).unlink()
            self.assertIsNone(bump.mkdocs_python(session, tree)[0])

    def test_the_first_clone_only_borrows_from_the_shared_checkout(self):
        with tempfile.TemporaryDirectory() as base:
            (Path(base) / ".git").mkdir()
            options = args(praetor_checkout=Path(base))
            argv = bump.clone_argv(options, Path("/cache/praetor.git"))
            self.assertIn("--dissociate", argv)
            self.assertEqual(argv[argv.index("--reference-if-able") + 1], base)
            options.praetor_checkout = Path(base) / "absent"
            self.assertNotIn("--reference-if-able", bump.clone_argv(options, Path("/m")))


class CleanupAndEvidenceTests(unittest.TestCase):
    """What a success removes, and what a failure tells the maintainer."""

    def test_a_success_removes_every_kept_worktree(self):
        """Positive: worktrees earlier failures kept go too, each through its own repository."""
        with tempfile.TemporaryDirectory() as base:
            session = bump.Session(args(cache=base), stream=io.StringIO())
            names = ("aegis-aaaaaaa", "aegis-bbbbbbb", "praetor-aaaaaaa", "praetor-bbbbbbb")
            for name in names:
                (Path(base) / "work" / name).mkdir(parents=True)
            (Path(base) / "bin" / "aaaaaaa").mkdir(parents=True)
            (Path(base) / "failed").mkdir()
            with mock.patch.object(bump, "run", return_value=bump.Result((), 0, "", "")) as run:
                bump.clean(session)
            removed = {
                (Path(call.args[0][2]).name, Path(call.args[0][-1]).name)
                for call in run.call_args_list
            }
            self.assertEqual(
                removed,
                {
                    (session.args.repo.name, "aegis-aaaaaaa"),
                    (session.args.repo.name, "aegis-bbbbbbb"),
                    ("praetor.git", "praetor-aaaaaaa"),
                    ("praetor.git", "praetor-bbbbbbb"),
                },
            )
            for name in names:
                self.assertFalse((Path(base) / "work" / name).exists())
            self.assertFalse((Path(base) / "bin").exists())
            self.assertFalse((Path(base) / "failed").exists())

    def test_an_empty_cache_runs_nothing(self):
        """Boundary: no work/ directory means no git call."""
        with tempfile.TemporaryDirectory() as base:
            session = bump.Session(args(cache=base), stream=io.StringIO())
            with mock.patch.object(bump, "run") as run:
                bump.clean(session)
            run.assert_not_called()

    def test_the_evidence_says_whether_the_branch_was_pushed(self):
        error = bump.BumpError("gh pr edit failed (exit 1): HTTP 401", "detail")
        tree = Path(tempfile.gettempdir()) / "aegis-no-such-worktree"
        quiet = bump.evidence_text(PLAN, tree, error, "log")
        self.assertIn("forge: nothing pushed", quiet)
        pushed = bump.evidence_text(PLAN, tree, error, "log", pushed=True)
        self.assertIn(f"forge: {bump.ROLLING_BRANCH} was pushed", pushed)
        self.assertIn("--retry", pushed)

    def test_a_missing_ledger_names_its_real_source(self):
        """Negative: `state init --if-absent` is never offered for what it cannot create."""
        self.assertIn("state bug add", bump.ledger_origin("bugs.meta.json"))
        self.assertIn("state question add", bump.ledger_origin("questions.meta.json"))
        for name in ("bugs.meta.json", "questions.meta.json"):
            self.assertNotIn("state init", bump.ledger_origin(name))
        self.assertIn("never repairs a partial set", bump.ledger_origin("BACKLOG.md"))

    def test_a_missing_ledger_stops_the_copy(self):
        with tempfile.TemporaryDirectory() as base:
            repo, tree = Path(base) / "repo", Path(base) / "tree"
            (repo / ".workingdir").mkdir(parents=True)
            tree.mkdir()
            for name in bump.LEDGERS[:-1]:
                (repo / ".workingdir" / name).write_text("x", encoding="utf-8")
            with self.assertRaises(bump.BumpError) as caught:
                bump.copy_ledgers(repo, tree)
        self.assertIn("questions.meta.json is missing: the first", str(caught.exception))


class UnitFileTests(unittest.TestCase):
    """The systemd user units run the script, as the user, on the two triggers."""

    def text(self, name):
        return (UNITS / name).read_text(encoding="utf-8")

    def test_the_service_is_a_oneshot_that_refuses_root(self):
        service = self.text("aegis-praetor-bump.service")
        self.assertIn("Type=oneshot", service)
        self.assertIn("ConditionUser=!root", service)
        self.assertIn("tools/praetor_bump.py", service)
        directives = [line.split("=", 1)[0] for line in service.splitlines() if "=" in line]
        self.assertNotIn("User", directives, "a user unit runs as its user; User= is not set")

    def test_the_triggers_are_a_reinstall_and_a_persistent_daily_timer(self):
        self.assertIn("PathChanged=%h/.local/bin/praetorctl", self.text("aegis-praetor-bump.path"))
        timer = self.text("aegis-praetor-bump.timer")
        self.assertIn("Persistent=true", timer)
        self.assertIn("OnCalendar=daily", timer)

    def test_the_make_target_installs_exactly_these_units(self):
        makefile = (ROOT / "Makefile").read_text(encoding="utf-8")
        shipped = sorted(path.name for path in UNITS.glob("aegis-praetor-bump.*"))
        self.assertEqual(len(shipped), 3)
        for name in shipped:
            self.assertIn(name, makefile)
        self.assertIn("install-praetor-bump:", makefile)


class SourceSweeps(unittest.TestCase):
    """HISS-02 and HISS-04 over the module itself."""

    tree = ast.parse(SOURCE.read_text(encoding="utf-8"))

    def test_only_the_run_helper_starts_a_process_and_it_waits_with_a_deadline(self):
        starters, waits = [], []
        for function in ast.walk(self.tree):
            if not isinstance(function, ast.FunctionDef):
                continue
            for node in ast.walk(function):
                if isinstance(node, ast.Attribute) and isinstance(node.value, ast.Name):
                    if node.value.id == "subprocess" and node.attr in {"Popen", "run", "call"}:
                        starters.append((function.name, node.attr))
                if isinstance(node, ast.Call) and getattr(node.func, "attr", "") == "communicate":
                    waits.append({keyword.arg for keyword in node.keywords})
        self.assertEqual(starters, [("run", "Popen")])
        self.assertTrue(waits)
        self.assertTrue(all("timeout" in keywords for keywords in waits))

    def test_no_function_exceeds_the_length_or_complexity_limit(self):
        offenders = []
        for node in ast.walk(self.tree):
            if isinstance(node, ast.FunctionDef):
                length = node.end_lineno - node.lineno + 1
                if length > MAX_FUNCTION_LINES or complexity(node) > MAX_COMPLEXITY:
                    offenders.append(f"{node.name}: {length} lines, complexity {complexity(node)}")
        self.assertEqual(offenders, [])

    def test_the_complexity_count_sees_branches(self):
        """Positive: the measure is proven against a known-branchy function."""
        planted = ast.parse(
            "def f(a, b):\n    if a and b:\n        return 1\n    for x in a:\n        pass\n"
        )
        self.assertEqual(complexity(planted.body[0]), 4)

    def test_the_module_never_evaluates_code(self):
        names = {node.id for node in ast.walk(self.tree) if isinstance(node, ast.Name)}
        self.assertFalse(names & {"eval", "exec", "compile"})


if __name__ == "__main__":
    unittest.main()
