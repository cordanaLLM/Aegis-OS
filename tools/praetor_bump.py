#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Move the CI praetorctl pin to the head of Praetor's main and propose it as one pull request.

This is the procedure of pull requests #122 and #130, automated. The maintainer
decided on 2026-09-28 that `PRAETOR_COMMIT` in `.github/workflows/ci.yml` follows
Praetor's main whenever main moves, and that the bump runs locally -- from a
systemd user unit or by hand -- so the pull request comes from the maintainer's
account and CI runs on it. A pull request opened with the Actions token would
start no CI in this repository.

One run:

1. fetches Praetor's main into a cache clone and stops ("up to date") when
   Aegis main already pins its head, or when the open bump pull request already
   proposes it;
2. builds praetorctl at that commit the way CI does;
3. runs `praetorctl adopt --force` in a fresh worktree of Aegis main;
4. puts back every hand-maintained file adopt rewrote (`HAND_MAINTAINED`),
   restores AGENTS.md and recompiles its projections, then reverts each other
   change no Praetor gate needs (`minimise`);
5. moves the pin in ci.yml and in docs/roadmap/toolchain-admission.md;
6. runs the repository's gates with the pinned praetorctl first on PATH;
7. GREEN: commits with sign-off and force-pushes the one rolling branch,
   opening or updating its pull request. RED: pushes nothing, writes the log to
   .workingdir/evidence/praetor-bump-<sha7>.log and keeps the worktree.

It never skips a hook and never merges unless `--merge` is given and the
repository allows auto-merge. See docs/build/praetor-bump.md.
"""

import argparse
import datetime
import fnmatch
import hashlib
import itertools
import json
import os
import re
import shutil
import signal
import subprocess
import sys
import textwrap
from collections import namedtuple
from pathlib import Path

import host

CI_WORKFLOW = ".github/workflows/ci.yml"
ADMISSION_PAGE = "docs/roadmap/toolchain-admission.md"
AGENTS = "AGENTS.md"
BASELINE = ".standards-baseline.json"
# The keys adopt rewrites in .standards-baseline.json on every run. A change
# confined to them records no debt, so the committed baseline is kept.
BASELINE_STAMP_KEYS = ("generated_at", "commit_sha")

AEGIS_SLUG = "cordanaLLM/Aegis-OS"
AEGIS_REMOTE = "origin"
AEGIS_BASE = "main"
PRAETOR_SLUG = "cordanaLLM/praetor"
PRAETOR_URL = f"https://github.com/{PRAETOR_SLUG}.git"
PRAETOR_BRANCH = "main"
ADOPT_PROFILE = "os-image"
# Every bump is force-pushed to this one branch, so at most one bump pull
# request is ever open, and a newer bump replaces an older one in place.
ROLLING_BRANCH = "chore/praetor-pin-auto"
# The lines naming the full target commit and the Aegis main commit the bump
# was built on, in the commit message and the pull request body; the
# idempotency check reads both back from the open pull request.
TARGET_MARKER = "Target: praetor "
BASE_MARKER = "Base: aegis "

# The gitignored state ledgers `praetorctl flavor audit` reads. They are copied
# from the primary checkout, never symlinked: audit rejects a symlinked
# .workingdir.
LEDGERS = (
    "STATE.md",
    "BUGS.md",
    "QUESTIONS.md",
    "OPEN.md",
    "BACKLOG.md",
    "bugs.meta.json",
    "questions.meta.json",
)

# Files this repository maintains by hand and `praetorctl adopt --force`
# overwrites with Praetor's templates. They always go back to the content on
# main; when a Praetor gate then fails, the bump is RED and a person decides.
# An entry ending in "/" covers a directory, an entry with "*" is a glob matched
# against the path and its file name, and any other entry is one exact path.
HAND_MAINTAINED = (
    # Aegis's lefthook layering (`extends:` .config/lefthook/preparation.yml);
    # adopt writes Praetor's Go template, which drops it.
    "lefthook.yml",
    # The HISS-16 evasion interceptor and its six client registrations, which
    # AGENTS.md ("Evasion interception in agent clients") declares hand-maintained.
    ".config/agent/hooks/block_evasion.py",
    ".claude/settings.json",
    ".codex/hooks.json",
    ".gemini/settings.json",
    ".github/hooks/hiss-16-block-evasion.json",
    ".cursor/hooks.json",
    ".windsurf/hooks.json",
    # Editor configuration. .vscode/ is tuned to this stack (CONTRIBUTING.md);
    # the others stay as committed rather than regenerated on every bump.
    ".vscode/",
    ".zed/",
    ".idea/",
    ".fleet/",
    ".helix/",
    "lua/",
    "standards.sublime-project",
    ".nvim.lua",
    ".dir-locals.el",
    ".clang-tidy",
    ".editorconfig",
    # Backups adopt leaves next to a file it replaced.
    "*.bak",
)

# The file, relative to the git common directory, where `lefthook install`
# records which configuration the shared hooks were written from.
LEFTHOOK_CHECKSUM = "info/lefthook.checksum"
# The documentation portal's pinned requirements; pages.yml builds the portal
# with them and `mkdocs build --strict` on every pull request touching docs/.
DOCS_REQUIREMENTS = "docs/requirements.txt"

# The gates that decide whether adopt's version of a file is needed (#130).
PRAETOR_GATES = (
    ("compile-context --verify", ("compile-context", "--verify")),
    ("audit", ("audit",)),
    ("flavor audit", ("flavor", "audit", ".")),
)

# Deadlines in seconds (HISS-02). Every external command gets one.
DEADLINE_QUICK = 120
DEADLINE_NETWORK = 900
DEADLINE_BUILD = 1800
DEADLINE_ADOPT = 900
DEADLINE_LINT = 900
DEADLINE_VERIFY_ALL = 5400
# git commit and git push run the repository's lefthook hooks, which run the
# Python suite, the audits and the pre-push gate.
DEADLINE_HOOKED = 3600
DEADLINE_DRAIN = 10

# Scalar bounds (HISS-02).
MAX_CANDIDATES = 256
MAX_PAIR_FILES = 24
MAX_PAIR_ROUNDS = 12
MAX_STATUS_ENTRIES = 4096
MAX_HOOK_FILES = 64
MAX_STALE_WORKTREES = 64
MAX_LISTED_COMMITS = 60
MAX_CAPTURE_CHARS = 4 * 1024 * 1024
MAX_FAILURE_LINES = 40
MAX_TAIL_LINES = 15
MAX_SKIP_LINES = 20
# Commit body prose is wrapped at the conventional width.
COMMIT_WIDTH = 72

SHA = re.compile(r"[0-9a-f]{40}")
PIN_LINE = re.compile(r'^(\s*PRAETOR_COMMIT:\s*")([^"\n]*)(")', re.MULTILINE)
STATUS_DATE = re.compile(r"^(Status: .*current as of )\d{4}-\d{2}-\d{2}$", re.MULTILINE)
MARKER_LINE = re.compile(r"^" + re.escape(TARGET_MARKER) + r"([0-9a-f]{40})\s*$", re.MULTILINE)
BASE_LINE = re.compile(r"^" + re.escape(BASE_MARKER) + r"([0-9a-f]{40})\s*$", re.MULTILINE)
FAILURE_LINE = re.compile(r"FAIL|ERROR|[Ee]rror[:\[]|would reformat|^\S+:\d+:\d+: [A-Z]\d+")
PULL_URL = re.compile(r"/pull/(\d+)\s*$")
ADMISSION_ROW = "| praetorctl |"
COMPARE_JQ = '.total_commits, (.commits[] | .sha[0:7] + " " + (.commit.message | split("\\n")[0]))'
# Stable, prompt-free output from every tool this script starts.
QUIET_ENV = {
    "LC_ALL": "C",
    "LANG": "C",
    "NO_COLOR": "1",
    "GIT_TERMINAL_PROMPT": "0",
    "GH_PROMPT_DISABLED": "1",
    "GH_NO_UPDATE_NOTIFIER": "1",
}
# Inherited git variables would point every git call at the wrong repository.
GIT_CONTEXT = ("GIT_DIR", "GIT_WORK_TREE", "GIT_INDEX_FILE", "GIT_COMMON_DIR")

Result = namedtuple("Result", "argv code out err")
Plan = namedtuple("Plan", "target pin base")
Report = namedtuple("Report", "kept reverted commits skips message")


class BumpError(Exception):
    """A step that cannot continue. The message is the one-line reason."""

    def __init__(self, reason, detail=""):
        super().__init__(reason)
        self.detail = detail


# --- running commands -------------------------------------------------------


def clip(data):
    """Decode captured output, keeping at most the last `MAX_CAPTURE_CHARS`."""
    text = data.decode("utf-8", errors="replace") if data else ""
    return text[-MAX_CAPTURE_CHARS:]


def run(argv, deadline, cwd=None, env=None):
    """Run `argv` under a hard deadline; the only place this module starts a process.

    Returns a Result whose code is None when the command could not start or did
    not finish in time; the reason is then in `err`. When this process is
    interrupted instead (Ctrl-C, or SIGTERM through `terminated`), the child's
    whole session is ended before the interruption goes on: the child leads its
    own session, so the terminal's SIGINT never reaches it, and it would
    otherwise outlive the run and its lock without a deadline.
    """
    argv = tuple(str(part) for part in argv)
    try:
        process = subprocess.Popen(
            argv,
            cwd=cwd,
            env=env,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            start_new_session=True,
        )
    except OSError as error:
        return Result(argv, None, "", f"{argv[0]} could not be started: {error}")
    try:
        out, err = process.communicate(timeout=deadline)
    except subprocess.TimeoutExpired:
        return Result(argv, None, "", stop(process, deadline))
    except BaseException:
        stop(process, deadline)
        raise
    return Result(argv, process.returncode, clip(out), clip(err))


def terminated(signum, _frame):
    """Turn SIGTERM into SystemExit, so `run()` ends its child and every `finally` runs."""
    raise SystemExit(128 + signum)


def stop(process, deadline):
    """End a process that outlived its deadline, with everything it started."""
    ended, reason = host.end_session(process.pid)
    if not ended:
        process.kill()
    try:
        out, err = process.communicate(timeout=DEADLINE_DRAIN)
    except subprocess.TimeoutExpired:
        process.kill()
        out, err = b"", b""
    suffix = f"; {reason}" if reason else ""
    return f"{clip(out)}{clip(err)}\n[stopped: exceeded its {deadline}s deadline{suffix}]"


def describe(result):
    """Return how a command ended, in a few words."""
    if result.code is None:
        return "did not start or finish"
    return f"exit {result.code}"


def last_line(result):
    """Return the last non-empty output line of a command, for a one-line reason."""
    lines = [line for line in (result.err + "\n" + result.out).splitlines() if line.strip()]
    return lines[-1].strip()[:200] if lines else "no output"


def failure_lines(name, result):
    """Return the lines of a failed command worth reading first, then its tail."""
    lines = (result.out + "\n" + result.err).splitlines()
    flagged = [line for line in lines if FAILURE_LINE.search(line)][:MAX_FAILURE_LINES]
    tail = [line for line in lines if line.strip()][-MAX_TAIL_LINES:]
    return "\n".join([f"--- {name}: {describe(result)}", *flagged, "--- tail:", *tail])


def skip_lines(text):
    """Return the lines where a gate said it did not run (a SKIP is not a pass)."""
    return [line.strip() for line in text.splitlines() if line.startswith("SKIP")][:MAX_SKIP_LINES]


class Session:
    """What one run shares: its options, its journal and the praetorctl it pinned."""

    def __init__(self, args, stream=None):
        self.args = args
        self.cache = args.cache_dir
        self.stream = stream or sys.stdout
        self.entries = []
        self.binary = None
        self.pushed = False

    def note(self, text):
        """Print a progress line and keep it for the evidence log."""
        self.entries.append(text)
        print(text, file=self.stream, flush=True)

    def environment(self, extra=None):
        """Return the environment every command runs with."""
        env = dict(os.environ)
        for key in GIT_CONTEXT:
            env.pop(key, None)
        env.update(QUIET_ENV)
        env["CARGO_TARGET_DIR"] = str(self.cache / "target")
        if self.binary is not None:
            env["PATH"] = os.pathsep.join([str(self.binary.parent), env.get("PATH", "")])
        env.update(extra or {})
        return env

    def run(self, argv, deadline, cwd=None, extra=None, quiet=False):
        """Run a command and journal it; `quiet` keeps only the command and its exit."""
        result = run(argv, deadline, cwd=cwd, env=self.environment(extra))
        command = " ".join(str(part) for part in result.argv)
        heading = f"$ {command}  [in {cwd or os.getcwd()}] -> {describe(result)}"
        body = "" if quiet else f"\n{result.out}{result.err}".rstrip()
        self.entries.append(heading + body)
        return result

    def must(self, argv, deadline, cwd=None, extra=None, what=None):
        """Run a command that has to succeed; return its standard output."""
        result = self.run(argv, deadline, cwd=cwd, extra=extra)
        if result.code != 0:
            name = what or Path(str(argv[0])).name
            reason = f"{name} failed ({describe(result)}): {last_line(result)}"
            raise BumpError(reason, failure_lines(name, result))
        return result.out

    def log_text(self):
        """Return the whole journal."""
        return "\n".join(self.entries)


# --- the register the bump rewrites (pure) ------------------------------------


def require_sha(text, what):
    """Return `text` when it is a full lower-case commit id, else refuse."""
    value = text.strip()
    if not SHA.fullmatch(value):
        raise BumpError(f"{what} is not a full commit id: {value[:60]!r}")
    return value


def ci_env(text, key):
    """Return the one quoted value of `key:` in the workflow's env block."""
    pattern = re.compile(r"^\s*" + re.escape(key) + r':\s*"([^"\n]*)"\s*$', re.MULTILINE)
    values = pattern.findall(text)
    if len(values) != 1:
        raise BumpError(f"{CI_WORKFLOW} sets {key} {len(values)} times; expected once")
    return values[0]


def read_pin(text):
    """Return the Praetor commit ci.yml pins."""
    return require_sha(ci_env(text, "PRAETOR_COMMIT"), f"PRAETOR_COMMIT in {CI_WORKFLOW}")


def rewrite_pin(text, target):
    """Return ci.yml with `PRAETOR_COMMIT` set to `target`, and nothing else changed."""
    require_sha(target, "the new pin")
    read_pin(text)
    return PIN_LINE.sub(lambda match: match.group(1) + target + match.group(3), text, count=1)


def rewrite_admission(text, pin, target, today):
    """Return the admission page with the praetorctl row moved from `pin` to `target`.

    The row keeps its "how it is pinned" and "admitted by" cells; the previous pin
    moves to "Replaces". The page's status date becomes `today`.
    """
    lines = text.split("\n")
    rows = [index for index, line in enumerate(lines) if line.startswith(ADMISSION_ROW)]
    if len(rows) != 1:
        raise BumpError(f"{ADMISSION_PAGE} has {len(rows)} praetorctl rows; expected one")
    cells = lines[rows[0]].split("|")
    if len(cells) != 7:
        raise BumpError(f"the praetorctl row in {ADMISSION_PAGE} does not have five cells")
    if cells[2].strip() != f"source commit `{pin}`":
        raise BumpError(
            f"the praetorctl row in {ADMISSION_PAGE} names {cells[2].strip()!r}, "
            f"but {CI_WORKFLOW} pins {pin}"
        )
    cells[2] = f" source commit `{require_sha(target, 'the new pin')}` "
    cells[4] = (
        f" the previous pin, source commit `{pin}`; the pin follows Praetor's `main` (see below) "
    )
    lines[rows[0]] = "|".join(cells)
    page = "\n".join(lines)
    return STATUS_DATE.sub(lambda match: match.group(1) + today.isoformat(), page, count=1)


# --- decisions (pure) -----------------------------------------------------------


def parse_prs(text):
    """Return the open pull requests `gh pr list --json number,body,url` printed."""
    try:
        entries = json.loads(text or "[]")
    except json.JSONDecodeError as error:
        raise BumpError(f"gh pr list printed no JSON: {error}") from error
    if not isinstance(entries, list):
        raise BumpError("gh pr list printed JSON that is not a list")
    return [entry for entry in entries if isinstance(entry.get("number"), int)]


def proposed_target(body):
    """Return the full Praetor commit a bump pull request body names, or None."""
    match = MARKER_LINE.search(body or "")
    return match.group(1) if match else None


def proposed_base(body):
    """Return the Aegis main commit a bump pull request body was built on, or None."""
    match = BASE_LINE.search(body or "")
    return match.group(1) if match else None


def decide(pin, target, base, prs):
    """Return (action, pull request number) for this run.

    "up-to-date": main already pins `target`. "proposed": an open bump pull
    request already names `target` built on Aegis `base`, so there is nothing to
    do. "update": the open bump pull request names an older target or an older
    Aegis main, and is rebuilt on `base`; a pull request built on an older main
    can no longer merge under the up-to-date rule, and a rebase made on the forge
    is an unsigned commit the signature rule refuses. "create": no bump pull
    request is open.
    """
    require_sha(pin, "the current pin")
    require_sha(target, "the Praetor head")
    require_sha(base, "Aegis main")
    if pin == target:
        return "up-to-date", None
    for pr in prs:
        body = pr.get("body")
        if proposed_target(body) == target and proposed_base(body) == base:
            return "proposed", pr["number"]
    if prs:
        return "update", prs[0]["number"]
    return "create", None


def pr_number(url):
    """Return the number at the end of the pull request URL `gh pr create` printed."""
    match = PULL_URL.search(url or "")
    if match is None:
        raise BumpError(f"gh pr create printed no pull request URL: {(url or '')[:200]!r}")
    return int(match.group(1))


def is_hand_maintained(path):
    """Return True when `path` is one of `HAND_MAINTAINED`."""
    for entry in HAND_MAINTAINED:
        if entry.endswith("/") and path.startswith(entry):
            return True
        if "*" in entry and fnmatch.fnmatch(path.rsplit("/", 1)[-1], entry):
            return True
        if path == entry:
            return True
    return False


def hand_maintained(paths):
    """Return the paths among `paths` that always go back to main's content."""
    return sorted(path for path in paths if is_hand_maintained(path))


def stamp_only(before, after):
    """Return True when two baselines differ in `BASELINE_STAMP_KEYS` at most."""
    try:
        old, new = json.loads(before), json.loads(after)
    except json.JSONDecodeError:
        return False
    if not isinstance(old, dict) or not isinstance(new, dict):
        return False
    for key in BASELINE_STAMP_KEYS:
        old.pop(key, None)
        new.pop(key, None)
    return old == new


def parse_status(text):
    """Return {path: two-letter status} from `git status --porcelain=v1 -z`."""
    fields = text.split("\0")
    changes = {}
    index = 0
    for _ in range(MAX_STATUS_ENTRIES):
        if index >= len(fields) or len(fields[index]) < 4:
            break
        code, path = fields[index][:2], fields[index][3:]
        changes[path] = code
        index += 2 if code[0] in "RC" else 1
    return changes


def minimise(candidates, try_revert):
    """Return the candidates to keep: those some Praetor gate fails without.

    `try_revert(paths)` puts `paths` back to main's content and runs the gates;
    it returns True when they pass (the revert stands) and otherwise restores
    adopt's content and returns False. All candidates are tried together first,
    then one at a time, then in pairs, because some files only pass together
    (.paperclip/harness.json with the register digest in .standards.yaml).
    Candidates past `MAX_CANDIDATES` are kept untried.
    """
    examined = list(candidates)[:MAX_CANDIDATES]
    untried = list(candidates)[MAX_CANDIDATES:]
    if not examined or try_revert(examined):
        return untried
    kept = [path for path in examined if not try_revert([path])]
    return reduce_pairs(kept, try_revert) + untried


def first_pair(kept, try_revert):
    """Revert the first pair of `kept` whose revert passes; return it, or None."""
    return next((pair for pair in itertools.combinations(kept, 2) if try_revert(list(pair))), None)


def reduce_pairs(kept, try_revert):
    """Revert pairs of kept files that pass only together; return what is left.

    After a pair goes, each remaining file is retried alone, since a file may
    have been needed only by the pair. Past `MAX_PAIR_FILES` the quadratic
    search is not attempted and every kept file stays.
    """
    for _ in range(MAX_PAIR_ROUNDS):
        if not 2 <= len(kept) <= MAX_PAIR_FILES:
            return kept
        pair = first_pair(kept, try_revert)
        if pair is None:
            return kept
        kept = [path for path in kept if path not in pair and not try_revert([path])]
    return kept


# --- messages (pure) ------------------------------------------------------------


def parse_compare(text):
    """Return (total, ["sha7 subject", ...]) from the compare query's output."""
    lines = [line for line in (text or "").splitlines() if line.strip()]
    if not lines or not lines[0].strip().isdigit():
        raise BumpError("the Praetor compare query printed no commit count")
    return int(lines[0]), lines[1:]


def listed_commits(total, commits):
    """Return at most `MAX_LISTED_COMMITS` commit lines, and how many more there are."""
    shown = commits[:MAX_LISTED_COMMITS]
    more = total - len(shown)
    return shown + ([f"... and {more} more"] if more > 0 else [])


def change_lines(kept, reverted):
    """Return the commit message's account of adopt's rewrite."""
    lines = ["Kept from praetorctl adopt --force (a Praetor gate fails without it):"]
    lines += [f"- {path}" for path in kept] or ["- nothing beyond the pin"]
    lines += ["", "Reverted to the content on main:"]
    lines += [f"- {path} ({why})" for path, why in reverted] or ["- nothing"]
    return lines


def subject(target):
    """Return the conventional commit subject and pull request title."""
    return f"chore(governance): bump praetor to {target[:7]}"


def commit_message(plan, commits, kept, reverted):
    """Return the commit message; `git commit --signoff` adds the sign-off."""
    total, lines = commits
    summary = (
        f"Moves PRAETOR_COMMIT from {plan.pin[:7]} to {plan.target[:7]}, {total} commits of "
        "Praetor main, under the maintainer's policy of bumping whenever Praetor main moves. "
        "Generated by tools/praetor_bump.py."
    )
    body = [
        subject(plan.target),
        "",
        textwrap.fill(summary, width=COMMIT_WIDTH),
        "",
        "Praetor commits taken:",
        *[f"- {line}" for line in listed_commits(total, lines)],
        "",
        *change_lines(kept, reverted),
        "",
        f"{TARGET_MARKER}{plan.target}",
        f"{BASE_MARKER}{plan.base}",
    ]
    return "\n".join(body) + "\n"


def checklist(target):
    """Return the repository's pull request checklist, as this bump satisfies it."""
    return [
        "- [x] Local verification passed: `make verify-all` (with its `docs-lint` "
        "Documentation Governance step) exits 0 with `praetorctl` built from "
        f"`{target[:7]}` with CI's build flags; `praetorctl compile-context --verify`, "
        "`praetorctl audit`, `praetorctl flavor audit .`, markdownlint-cli2, yamllint, flake8, "
        "black, `reuse lint` and `mkdocs build --strict` pass on the workstation's linters and "
        "Go (CI re-runs its pinned ones), except any gate listed above as a skip",
        "- [x] No new HISS-16 / NASA Power-of-10 infractions (`praetorctl audit`)",
        "- [x] 3D Tests included: no public interface changed",
        "- [x] Agent contexts regenerated from `AGENTS.md`, not hand-edited: "
        "`praetorctl compile-context`",
        "- [x] No new claim of image, boot, hardware, accessibility or release evidence",
        "- [x] Register surfaces consistent: `docs/roadmap/toolchain-admission.md` records the pin",
        "- [x] Commit messages adhere to Conventional Commits format",
    ]


def pr_body(plan, report):
    """Return the pull request body for this bump."""
    total, lines = report.commits
    return "\n".join(
        [
            "## Description",
            "",
            f"Moves `PRAETOR_COMMIT` from `{plan.pin[:7]}` to `{plan.target[:7]}`: {total} "
            "commits of Praetor main, under the maintainer's policy (2026-09-28) of bumping "
            "whenever Praetor main moves. `tools/praetor_bump.py` wrote this pull request and "
            f"force-pushes `{ROLLING_BRANCH}` on every bump, so it always proposes the newest "
            "head. See `docs/build/praetor-bump.md`.",
            "",
            "**Praetor commits taken**",
            *[f"- {line}" for line in listed_commits(total, lines)],
            "",
            *change_lines(report.kept, report.reverted),
            "",
            "**Gates that reported a skip** (a skip did not run)",
            *([f"- {line}" for line in report.skips] or ["- none"]),
            "",
            "## Pre-Merge Verification Checklist",
            "",
            *checklist(plan.target),
            "",
            f"{TARGET_MARKER}{plan.target}",
            f"{BASE_MARKER}{plan.base}",
            "",
        ]
    )


def pushed_state(pushed):
    """Return what a failed run left on the forge."""
    if not pushed:
        return "nothing pushed"
    return (
        f"{ROLLING_BRANCH} was pushed before the failure, so the open pull request may still "
        "name the previous target; check its title against the branch, or rerun with --retry"
    )


def evidence_text(plan, tree, error, log, pushed=False):
    """Return the RED evidence: the reason, the failing lines, then the whole log."""
    return "\n".join(
        [
            f"praetor bump to {plan.target} on Aegis {plan.base}: RED",
            f"reason: {error}",
            f"forge: {pushed_state(pushed)}",
            f"worktree: {tree if tree.exists() else 'not created'}",
            "",
            "== failing lines ==",
            error.detail or "(none captured)",
            "",
            "== full log ==",
            log,
            "",
        ]
    )


# --- files ----------------------------------------------------------------------


def read_exact(path):
    """Return a text file's content with its line endings untouched."""
    with open(path, encoding="utf-8", newline="") as handle:
        return handle.read()


def write_exact(path, text):
    """Write a text file without translating line endings."""
    with open(path, "w", encoding="utf-8", newline="") as handle:
        handle.write(text)


def capture(path):
    """Return (bytes, mode) of a file, or None when it does not exist."""
    if not path.is_file():
        return None
    return path.read_bytes(), path.stat().st_mode & 0o777


def put_back(path, state):
    """Restore a file captured by `capture`, removing it when it did not exist."""
    if state is None:
        path.unlink(missing_ok=True)
        return
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(state[0])
    path.chmod(state[1])


def snapshot(directory):
    """Return {name: (bytes, mode)} for the regular files in `directory`."""
    if not directory.is_dir():
        return {}
    files = [entry for entry in sorted(directory.iterdir()) if entry.is_file()]
    if len(files) > MAX_HOOK_FILES:
        raise BumpError(f"{directory} holds more than {MAX_HOOK_FILES} files; not snapshotting")
    return {entry.name: capture(entry) for entry in files}


def restore_snapshot(directory, saved):
    """Put `directory` back to a `snapshot`: rewrite changed files, drop new ones."""
    for name, state in saved.items():
        if capture(directory / name) != state:
            put_back(directory / name, state)
    for entry in snapshot(directory):
        if entry not in saved:
            (directory / entry).unlink(missing_ok=True)


# --- steps ----------------------------------------------------------------------


def clone_argv(args, mirror):
    """Return the first clone of Praetor into the cache.

    The shared Praetor checkout only lends objects (`--reference-if-able` with
    `--dissociate` copies them), so the clone never depends on it and nothing is
    ever written to it.
    """
    argv = ["git", "clone", "--bare", "--quiet"]
    donor = args.praetor_checkout
    if donor is not None and (donor / ".git").exists():
        argv += ["--reference-if-able", str(donor), "--dissociate"]
    return argv + [args.praetor_url, str(mirror)]


def praetor_head(session):
    """Fetch Praetor's main into the cache clone and return its head commit."""
    mirror = session.cache / "praetor.git"
    if not (mirror / "HEAD").is_file():
        session.must(clone_argv(session.args, mirror), DEADLINE_NETWORK, what="cloning Praetor")
    refspec = f"+refs/heads/{PRAETOR_BRANCH}:refs/heads/{PRAETOR_BRANCH}"
    fetch = ["git", "-C", mirror, "fetch", "--quiet", session.args.praetor_url, refspec]
    session.must(fetch, DEADLINE_NETWORK, what="fetching Praetor main")
    ref = f"refs/heads/{PRAETOR_BRANCH}^{{commit}}"
    head = session.must(["git", "-C", mirror, "rev-parse", "--verify", ref], DEADLINE_QUICK)
    return require_sha(head, "Praetor main")


def aegis_pin(session):
    """Fetch Aegis main and return (its commit, the Praetor commit its ci.yml pins)."""
    repo = session.args.repo
    tracking = f"refs/remotes/{AEGIS_REMOTE}/{AEGIS_BASE}"
    refspec = f"+refs/heads/{AEGIS_BASE}:{tracking}"
    fetch = ["git", "-C", repo, "fetch", "--quiet", AEGIS_REMOTE, refspec]
    session.must(fetch, DEADLINE_NETWORK, what="fetching Aegis main")
    verify = ["git", "-C", repo, "rev-parse", "--verify", f"{tracking}^{{commit}}"]
    base = require_sha(session.must(verify, DEADLINE_QUICK), "Aegis main")
    workflow = session.must(["git", "-C", repo, "show", f"{base}:{CI_WORKFLOW}"], DEADLINE_QUICK)
    return base, read_pin(workflow)


def open_bump_prs(session):
    """Return the open pull requests from the rolling branch."""
    argv = ["gh", "pr", "list", "--repo", AEGIS_SLUG, "--head", ROLLING_BRANCH]
    argv += ["--state", "open", "--json", "number,body,url"]
    return parse_prs(session.must(argv, DEADLINE_QUICK, what="gh pr list"))


def failure_marker(cache, plan):
    """Return the file that records a RED run for this Praetor and Aegis pair."""
    return cache / "failed" / f"{plan.target}-{plan.base}"


def survey(session):
    """Decide whether this run has work; return (plan or None, exit code)."""
    target = praetor_head(session)
    base, pin = aegis_pin(session)
    prs = open_bump_prs(session)
    action, number = decide(pin, target, base, prs)
    if action == "up-to-date":
        session.note(f"up to date: Aegis main already pins praetor {target[:7]}")
        if prs:
            session.note(f"note: pull request #{prs[0]['number']} from {ROLLING_BRANCH} is open")
        return None, 0
    if action == "proposed":
        session.note(
            f"up to date: pull request #{number} already proposes praetor {target[:7]} "
            f"on Aegis {base[:7]}"
        )
        return None, 0
    plan = Plan(target, pin, base)
    marker = failure_marker(session.cache, plan)
    if marker.is_file() and not session.args.retry:
        session.note(
            f"RED (unchanged): the bump to {target[:7]} on Aegis {base[:7]} already failed; "
            f"evidence: {read_exact(marker).strip()}; pass --retry to run it again"
        )
        return None, 1
    return plan, 0


def fresh_worktree(session, repository, path, commit):
    """Create a detached worktree of `repository` at `commit`, replacing a stale one."""
    if path.exists():
        remove = ["git", "-C", repository, "worktree", "remove", "--force", path]
        session.run(remove, DEADLINE_QUICK)
        shutil.rmtree(path, ignore_errors=True)
    path.parent.mkdir(parents=True, exist_ok=True)
    add = ["git", "-C", repository, "worktree", "add", "--force", "--detach", "--quiet"]
    session.must(add + [path, commit], DEADLINE_QUICK, what=f"creating worktree {path.name}")


def build_praetorctl(session, target):
    """Build praetorctl at `target` exactly as CI does; return its source worktree.

    The binary becomes `session.binary`, which every later command finds first
    on PATH; the source worktree is what adopt reads as `--lock-source-root`.

    CI checks the commit out, runs `go mod download` and `go mod verify`, and
    builds with CGO_ENABLED=0 GOFLAGS=-mod=readonly and -trimpath -buildvcs=false.
    CI installs the Go version go.mod declares and sets GOTOOLCHAIN=local; here
    GOTOOLCHAIN stays at the workstation's setting, whose default `auto` builds
    with the local Go when it satisfies go.mod and fetches the one go.mod names
    when it does not.
    """
    source = session.cache / "work" / f"praetor-{target[:7]}"
    fresh_worktree(session, session.cache / "praetor.git", source, target)
    head = session.must(["git", "-C", source, "rev-parse", "HEAD"], DEADLINE_QUICK)
    if head.strip() != target:
        raise BumpError(f"the Praetor worktree is at {head.strip()[:12]}, not {target[:12]}")
    binary = session.cache / "bin" / target[:7] / "praetorctl"
    binary.parent.mkdir(parents=True, exist_ok=True)
    build = {"CGO_ENABLED": "0", "GOFLAGS": "-mod=readonly"}
    steps = (
        ["go", "mod", "download"],
        ["go", "mod", "verify"],
        ["go", "build", "-trimpath", "-buildvcs=false", "-o", binary, "./cmd/standardsctl"],
    )
    for step in steps:
        session.must(step, DEADLINE_BUILD, cwd=source, extra=build, what=" ".join(step[:3]))
    session.binary = binary
    session.note(f"built praetorctl at {target[:7]}: {binary}")
    return source


def ledger_origin(name):
    """Return where a missing ledger `name` of the primary checkout comes from."""
    if name == "bugs.meta.json":
        return (
            "the first `praetorctl state bug add` writes it, and `praetorctl state "
            "migrate-bugs` moves BUGS.md's inline metadata into it"
        )
    if name == "questions.meta.json":
        return "the first `praetorctl state question add` writes it"
    return (
        "`praetorctl state init . --if-absent` creates it only where .workingdir/ holds "
        "no ledger yet; it never repairs a partial set, so restore the file explicitly"
    )


def copy_ledgers(repo, tree):
    """Copy the flavor-audit ledgers from the primary checkout into the worktree."""
    source, target = repo / ".workingdir", tree / ".workingdir"
    if target.is_symlink():
        raise BumpError(f"{target} is a symlink; praetorctl flavor audit rejects that")
    target.mkdir(exist_ok=True)
    for name in LEDGERS:
        if not (source / name).is_file():
            raise BumpError(f"{source / name} is missing: {ledger_origin(name)}")
        shutil.copyfile(source / name, target / name)


def git_path(session, tree, name):
    """Return the absolute path of `name` in the git directory the worktree uses."""
    where = ["git", "-C", tree, "rev-parse", "--path-format=absolute", "--git-path", name]
    return Path(session.must(where, DEADLINE_QUICK).strip())


def adopt(session, tree, source):
    """Run `praetorctl adopt --force` in the worktree, leaving the shared git files as found.

    adopt runs `lefthook install`, which writes two things every worktree of the
    primary checkout shares: the hooks directory, and `LEFTHOOK_CHECKSUM`, which
    lefthook compares with the configuration before each hook run and, on a
    mismatch, re-syncs the hooks. Both are restored byte for byte.
    """
    hooks = git_path(session, tree, "hooks")
    checksum = git_path(session, tree, LEFTHOOK_CHECKSUM)
    saved, saved_checksum = snapshot(hooks), capture(checksum)
    argv = [session.binary, "adopt", "--force", "--path", ".", "--profile", ADOPT_PROFILE]
    try:
        session.must(argv + ["--lock-source-root", source], DEADLINE_ADOPT, cwd=tree)
    finally:
        restore_snapshot(hooks, saved)
        if capture(checksum) != saved_checksum:
            put_back(checksum, saved_checksum)


def status(session, tree):
    """Return the worktree's changes as {path: status}."""
    argv = ["git", "-C", tree, "status", "--porcelain=v1", "-z", "--untracked-files=all"]
    return parse_status(session.must(argv, DEADLINE_QUICK))


def revert(session, tree, path, code):
    """Put one path back to main's content: delete it when adopt created it."""
    if code == "??":
        (tree / path).unlink(missing_ok=True)
        return
    session.must(["git", "-C", tree, "checkout", "HEAD", "--", path], DEADLINE_QUICK)


def praetor_gates(session, tree, quiet=False):
    """Run the Praetor gates; return (all passed, the failing lines)."""
    failures = []
    for name, arguments in PRAETOR_GATES:
        result = session.run([session.binary, *arguments], DEADLINE_QUICK, cwd=tree, quiet=quiet)
        if result.code != 0:
            failures.append(failure_lines(name, result))
    return not failures, "\n".join(failures)


def try_revert(session, tree, changes, paths):
    """Revert `paths`; keep the revert when the Praetor gates pass, else undo it."""
    saved = {path: capture(tree / path) for path in paths}
    for path in paths:
        revert(session, tree, path, changes[path])
    passed, _ = praetor_gates(session, tree, quiet=True)
    if not passed:
        for path, state in saved.items():
            put_back(tree / path, state)
    return passed


def restore_fixed(session, tree, changes):
    """Revert the hand-maintained files, a stamp-only baseline and AGENTS.md."""
    reverted = [(path, "hand-maintained") for path in hand_maintained(changes)]
    if changes.get(BASELINE, "??") != "??" and (tree / BASELINE).is_file():
        committed = session.must(["git", "-C", tree, "show", f"HEAD:{BASELINE}"], DEADLINE_QUICK)
        if stamp_only(committed, read_exact(tree / BASELINE)):
            reverted.append((BASELINE, "only its timestamp and commit changed"))
    for path, _why in reverted:
        revert(session, tree, path, changes[path])
    session.must(["git", "-C", tree, "checkout", "HEAD", "--", AGENTS], DEADLINE_QUICK)
    session.must([session.binary, "compile-context"], DEADLINE_QUICK, cwd=tree)
    return reverted


def reconcile(session, tree):
    """Keep of adopt's rewrite only what a Praetor gate needs; return (kept, reverted)."""
    adopted = status(session, tree)
    reverted = restore_fixed(session, tree, adopted)
    passed, failing = praetor_gates(session, tree)
    if not passed:
        raise BumpError(
            "the Praetor gates fail on adopt's output with the hand-maintained files restored",
            failing,
        )
    changes = status(session, tree)
    candidates = sorted(changes)
    kept = minimise(candidates, lambda paths: try_revert(session, tree, changes, paths))
    reverted += [(path, "no gate needs it") for path in candidates if path not in kept]
    if AGENTS in adopted and AGENTS not in changes:
        reverted.append((AGENTS, "restored from main; its projections recompiled"))
    session.note(f"adopt reconciled: {len(kept)} kept, {len(reverted)} reverted")
    return kept, sorted(reverted)


def apply_pin(tree, plan, today):
    """Move the pin in ci.yml and in the toolchain admission page."""
    workflow = tree / CI_WORKFLOW
    text = read_exact(workflow)
    if read_pin(text) != plan.pin:
        raise BumpError(f"{CI_WORKFLOW} in the worktree no longer pins {plan.pin[:7]}")
    write_exact(workflow, rewrite_pin(text, plan.target))
    page = tree / ADMISSION_PAGE
    write_exact(page, rewrite_admission(read_exact(page), plan.pin, plan.target, today))


def gate_commands(binary, markdownlint, mkdocs=None, site=None):
    """Return (name, argv, deadline) for every gate the bump has to pass.

    `make verify-all` also runs `node tools/markdownlint/verify.mjs`, the
    Documentation Governance check, through its `docs-lint` prerequisite.
    `mkdocs` is the Python of the documentation venv; without one the portal
    build is not a gate here and the caller reports it as a skip.
    """
    black = ["black", "--check", "--line-length", "100", "tools", ".config/agent/hooks"]
    gates = (
        (
            "make verify-all",
            ["make", "verify-all", f"PRAETORCTL={host.target(binary)}"],
            DEADLINE_VERIFY_ALL,
        ),
        ("compile-context --verify", [binary, "compile-context", "--verify"], DEADLINE_QUICK),
        ("audit", [binary, "audit"], DEADLINE_QUICK),
        ("flavor audit", [binary, "flavor", "audit", "."], DEADLINE_QUICK),
        ("markdownlint-cli2", ["npx", "--yes", f"markdownlint-cli2@{markdownlint}"], DEADLINE_LINT),
        ("yamllint", ["yamllint", "."], DEADLINE_LINT),
        ("flake8", ["flake8", "."], DEADLINE_LINT),
        ("black", black, DEADLINE_LINT),
        ("reuse lint", ["reuse", "lint"], DEADLINE_LINT),
    )
    if mkdocs is None:
        return gates
    portal = [mkdocs, "-m", "mkdocs", "build", "--strict", "-d", site]
    return gates + (("mkdocs build --strict", portal, DEADLINE_LINT),)


# Where `python -m venv` puts the interpreter: bin/python on POSIX, Scripts\python.exe on
# Windows. The bump runs on the Linux workstation; the Windows spelling keeps the unit
# tests honest on the Platform Neutrality legs (HISS-21).
VENV_PYTHON = ("Scripts", "python.exe") if sys.platform == "win32" else ("bin", "python")


def mkdocs_python(session, tree):
    """Return (the Python of a venv holding `DOCS_REQUIREMENTS`, None), or (None, reason).

    The venv lives in the cache, is made with the Python running this script
    (the workstation's, not the 3.12 pages.yml sets up), and is rebuilt only
    when the requirements' digest changes. A venv that cannot be made or filled
    is a reason to report, not a pass.
    """
    requirements = tree / DOCS_REQUIREMENTS
    if not requirements.is_file():
        return None, f"{DOCS_REQUIREMENTS} is absent"
    digest = hashlib.sha256(requirements.read_bytes()).hexdigest()
    venv = session.cache / "mkdocs-venv"
    python, stamp = venv.joinpath(*VENV_PYTHON), venv / "requirements.sha256"
    if python.is_file() and stamp.is_file() and read_exact(stamp).strip() == digest:
        return python, None
    shutil.rmtree(venv, ignore_errors=True)
    pip = [python, "-m", "pip", "install", "--disable-pip-version-check", "--quiet"]
    for step in ([sys.executable, "-m", "venv", venv], pip + ["-r", requirements]):
        result = session.run(step, DEADLINE_NETWORK)
        if result.code != 0:
            return (
                None,
                f"the pinned mkdocs was not installed ({describe(result)}): {last_line(result)}",
            )
    write_exact(stamp, digest + "\n")
    return python, None


def run_gates(session, tree):
    """Run every gate; return the SKIP lines, or raise with the failing lines."""
    markdownlint = ci_env(read_exact(tree / CI_WORKFLOW), "MARKDOWNLINT_VERSION")
    mkdocs, reason = mkdocs_python(session, tree)
    skips = [] if mkdocs else [f"SKIP: mkdocs build --strict did not run: {reason}"]
    for line in skips:
        session.note(f"gate mkdocs build --strict: {line}")
    failures = []
    gates = gate_commands(session.binary, markdownlint, mkdocs, session.cache / "site")
    for name, argv, deadline in gates:
        result = session.run(argv, deadline, cwd=tree)
        skips += skip_lines(result.out + "\n" + result.err)
        session.note(f"gate {name}: {'pass' if result.code == 0 else describe(result)}")
        if result.code != 0:
            failures.append((name, failure_lines(name, result)))
    if failures:
        names = ", ".join(name for name, _lines in failures)
        raise BumpError(f"gates failed: {names}", "\n\n".join(lines for _name, lines in failures))
    return skips[:MAX_SKIP_LINES]


def praetor_commits(session, plan):
    """Return (total, lines) for the Praetor commits between the old and the new pin."""
    endpoint = f"repos/{PRAETOR_SLUG}/compare/{plan.pin}...{plan.target}"
    argv = ["gh", "api", endpoint, "--jq", COMPARE_JQ]
    return parse_compare(session.must(argv, DEADLINE_QUICK, what="gh api compare"))


def commit(session, tree, message):
    """Commit everything with sign-off; the repository's hooks run and must pass."""
    path = session.cache / "commit-message.txt"
    write_exact(path, message)
    session.must(["git", "-C", tree, "add", "--all"], DEADLINE_QUICK)
    argv = ["git", "-C", tree, "commit", "--signoff", "--quiet", "--file", path]
    session.must(argv, DEADLINE_HOOKED, what="git commit (with the repository's hooks)")


def prepare(session, plan, tree):
    """Build, adopt, reconcile, move the pin, gate and commit; return the Report."""
    fresh_worktree(session, session.args.repo, tree, plan.base)
    copy_ledgers(session.args.repo, tree)
    source = build_praetorctl(session, plan.target)
    adopt(session, tree, source)
    kept, reverted = reconcile(session, tree)
    apply_pin(tree, plan, datetime.date.today())
    skips = run_gates(session, tree)
    commits = praetor_commits(session, plan)
    message = commit_message(plan, commits, kept, reverted)
    commit(session, tree, message)
    return Report(kept, reverted, commits, skips, message)


def open_or_update_pr(session, plan, report):
    """Open the rolling branch's pull request, or rewrite the open one; return its number."""
    body = session.cache / "pr-body.md"
    write_exact(body, pr_body(plan, report))
    title = subject(plan.target)
    prs = open_bump_prs(session)
    if prs:
        number = prs[0]["number"]
        edit = ["gh", "pr", "edit", number, "--repo", AEGIS_SLUG, "--title", title]
        session.must(edit + ["--body-file", body], DEADLINE_QUICK, what="gh pr edit")
        session.note(f"GREEN: pull request #{number} now proposes praetor {plan.target[:7]}")
        return number
    create = ["gh", "pr", "create", "--repo", AEGIS_SLUG, "--base", AEGIS_BASE]
    create += ["--head", ROLLING_BRANCH, "--title", title, "--body-file", body]
    url = session.must(create, DEADLINE_QUICK, what="gh pr create").strip()
    session.note(f"GREEN: opened {url}")
    return pr_number(url)


def request_merge(session, number):
    """Queue a squash auto-merge, only where the repository allows auto-merge."""
    query = ["gh", "api", f"repos/{AEGIS_SLUG}", "--jq", ".allow_auto_merge"]
    if session.must(query, DEADLINE_QUICK, what="gh api").strip() != "true":
        session.note(f"not merging: auto-merge is disabled on {AEGIS_SLUG}")
        return
    merge = ["gh", "pr", "merge", number, "--repo", AEGIS_SLUG, "--squash", "--auto"]
    session.must(merge, DEADLINE_QUICK, what="gh pr merge --auto")
    session.note(f"auto-merge queued for pull request #{number}")


def publish(session, plan, tree, report):
    """Force-push the rolling branch and open or update its pull request.

    Returns False when `--no-publish` kept the commit local, so the worktree stays.
    """
    if session.args.no_publish:
        session.note(f"committed in {tree}; nothing pushed (--no-publish)")
        return False
    push = ["git", "-C", tree, "push", "--force", "--quiet", AEGIS_REMOTE]
    session.must(push + [f"HEAD:refs/heads/{ROLLING_BRANCH}"], DEADLINE_HOOKED, what="git push")
    session.pushed = True
    number = open_or_update_pr(session, plan, report)
    if session.args.merge:
        request_merge(session, number)
    return True


def remove_worktrees(session, repository, pattern):
    """Remove the worktrees of `repository` under the cache's work/ matching `pattern`."""
    work = session.cache / "work"
    trees = sorted(work.glob(pattern))[:MAX_STALE_WORKTREES] if work.is_dir() else []
    for tree in trees:
        session.run(
            ["git", "-C", repository, "worktree", "remove", "--force", tree], DEADLINE_QUICK
        )
        shutil.rmtree(tree, ignore_errors=True)


def clean(session):
    """Remove every worktree and binary in the cache, and the failure records.

    This run's worktrees go, and so do those that failed runs kept for earlier
    Praetor heads: the pin this run proposes supersedes them, and each would
    otherwise stay registered in the primary checkout's `git worktree list`.
    Their evidence logs in .workingdir/evidence/ stay.
    """
    remove_worktrees(session, session.args.repo, "aegis-*")
    remove_worktrees(session, session.cache / "praetor.git", "praetor-*")
    shutil.rmtree(session.cache / "bin", ignore_errors=True)
    shutil.rmtree(session.cache / "failed", ignore_errors=True)


def red(session, plan, tree, error):
    """Record a failed bump: evidence in the primary checkout, worktree kept."""
    evidence = session.args.repo / ".workingdir" / "evidence"
    evidence.mkdir(parents=True, exist_ok=True)
    evidence = evidence / f"praetor-bump-{plan.target[:7]}.log"
    text = evidence_text(plan, tree, error, session.log_text(), session.pushed)
    write_exact(evidence, text)
    marker = failure_marker(session.cache, plan)
    marker.parent.mkdir(parents=True, exist_ok=True)
    write_exact(marker, f"{evidence}\n")
    pushed = pushed_state(session.pushed)
    session.note(f"RED: {error}; {pushed}; evidence: {evidence}; worktree kept: {tree}")
    return 1


def execute(session, plan):
    """Carry out a planned bump; return the exit code."""
    tree = session.cache / "work" / f"aegis-{plan.target[:7]}"
    session.note(f"bumping praetor {plan.pin[:7]} -> {plan.target[:7]} on Aegis {plan.base[:7]}")
    try:
        report = prepare(session, plan, tree)
        published = publish(session, plan, tree, report)
    except BumpError as error:
        return red(session, plan, tree, error)
    if published:
        clean(session)
    return 0


def locked(session):
    """Survey and, when there is work, carry it out; the caller holds the lock."""
    try:
        plan, code = survey(session)
    except BumpError as error:
        session.note(f"FAIL: {error}")
        return 1
    if plan is None:
        return code
    return execute(session, plan)


def default_cache():
    """Return $XDG_CACHE_HOME/aegis-praetor-bump, defaulting to ~/.cache."""
    base = os.environ.get("XDG_CACHE_HOME") or str(Path.home() / ".cache")
    return Path(base) / "aegis-praetor-bump"


def parse_args(argv):
    """Return the parsed options."""
    parser = argparse.ArgumentParser(description=__doc__.split("\n\n")[0])
    parser.add_argument("--repo", type=Path, default=Path(__file__).resolve().parents[1])
    parser.add_argument("--cache-dir", type=Path, default=default_cache())
    parser.add_argument("--praetor-url", default=PRAETOR_URL)
    parser.add_argument(
        "--praetor-checkout",
        type=Path,
        default=None,
        help="local Praetor checkout lent as an object source for the first clone only; "
        "never written (default: praetor next to --repo)",
    )
    parser.add_argument("--merge", action="store_true", help="queue auto-merge when allowed")
    parser.add_argument("--no-publish", action="store_true", help="commit locally, push nothing")
    parser.add_argument("--retry", action="store_true", help="rerun a pair that already failed")
    args = parser.parse_args(argv)
    args.repo = args.repo.resolve()
    args.cache_dir = args.cache_dir.expanduser().resolve()
    if args.praetor_checkout is None:
        args.praetor_checkout = args.repo.parent / "praetor"
    return args


def main(argv=None):
    """Run one bump under the cache lock; return the exit code."""
    session = Session(parse_args(argv))
    uid, _reason = host.uid()
    if uid == 0:
        session.note("REFUSED: the praetor bump runs as the maintainer, never as root")
        return 2
    session.cache.mkdir(parents=True, exist_ok=True)
    with open(session.cache / "lock", "a+", encoding="utf-8") as handle:
        held, reason = host.exclusive_lock(handle)
        if not held:
            session.note(f"SKIP: {reason}; this run did nothing")
            return 0
        return locked(session)


if __name__ == "__main__":
    signal.signal(signal.SIGTERM, terminated)
    sys.exit(main())
