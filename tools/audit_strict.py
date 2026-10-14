#!/usr/bin/env python3
"""Run the engine audit and fail on a warning, as HISS-10 requires.

`praetorctl audit` exits 0 while it prints ``[WARN]`` lines: HISS-18, for
one, reports a draft pull request that runs its gate and still passes. HISS-10
tolerates zero warnings, so `make verify-all` runs the audit through this file
instead of on its own.

The reader is an allow-list. A line that opens with a bracketed marker passes
only when the marker is one of ``ACCEPTED``; ``[WARN]``, ``[FAIL]`` and any
marker a later engine adds fail the gate until this file names them. A run
that never prints the engine's summary line fails too, so an audit that
stopped early, or a command that is not the audit, cannot pass silently.

A workflow that ``.standards.yaml`` declares failing under
``workflow_runs.expected`` prints an ``[INFO]`` line instead of a warning, and
the engine's declaration has no expiry. Such a line passes only in one fixed
shape: its reason names an exception category, its entry date, its expiry, the
one run it covers and the engine order that removes the cause, and the run the
engine reports as the latest failure is that run. A declaration past its
expiry, one that covers another run, or one of any other shape fails.

Usage: ``python3 tools/audit_strict.py <praetorctl> audit [arguments]``; the
command after the file name runs as given.
``tools/test_audit_strict.py`` covers the verdict.
"""

import datetime
import re
import subprocess
import sys

# The markers an audit line may open with and still pass. UNSCANNED is
# accepted: it names source no HISS rule reads, an engine gap tracked in the
# engine's repository, not a finding about this one.
ACCEPTED = ("PASS", "INFO", "REPORT", "SKIP", "UNSCANNED")
MARKER = re.compile(r"^(?:Error:\s*)?\[([A-Z]+)\]")
SUMMARY = "Audit Summary: configured governance gates passed"
# A line the engine prints for a declared workflow (praetor
# internal/forge/workflow_run_health.go, WorkflowRunFinding.line).
DECLARED_PREFIX = re.compile(r"^\[INFO\] Workflow \S+: \S+ as declared \(")
# The one shape such a line may take: a failing workflow, a reason naming the
# category, entry date, expiry, run and engine order, and the engine's own
# reference to the latest failed run.
DECLARED = re.compile(
    r"^\[INFO\] Workflow (?P<workflow>[A-Za-z0-9_.-]+): failing as declared "
    r"\(category (?P<category>[BCD]), entry (?P<entry>\d{4}-\d{2}-\d{2}), "
    r"expiry (?P<expiry>\d{4}-\d{2}-\d{2}), run (?P<run>\d+), "
    r"(?P<order>[a-z0-9]+(?:-[a-z0-9]+)+)\): "
    r"failing on \S+, failed runs in a row: (?:at least )?\d+ "
    r"\(latest: run #\d+, [a-z_]+, failure, \S+, "
    r"https://github\.com/[A-Za-z0-9_.-]+/[A-Za-z0-9_.-]+/actions/runs/(?P<latest>\d+)\)$"
)
# The fleet's exception categories and their fixed rectification intervals in
# days; category A states its own interval and has no place in a fixed shape.
CATEGORY_DAYS = {"B": 3, "C": 10, "D": 120}
# The audit reads the whole tree and, with a token, the forge; this bounds it
# so a wedged audit fails the gate instead of hanging it (HISS-02).
AUDIT_TIMEOUT = 900
# Scalar bound on the lines judged (HISS-02).
MAX_LINES = 100_000


def parse_date(text):
    """Return the date text names, or None when it names none."""
    try:
        return datetime.date.fromisoformat(text)
    except ValueError:
        return None


def declared_problem(line, today):
    """Return why a declared-workflow line fails, or None when it passes."""
    if not DECLARED_PREFIX.match(line):
        return None
    found = DECLARED.match(line)
    if not found:
        return f"a workflow declaration outside the one accepted shape: {line}"
    entry, expiry = parse_date(found["entry"]), parse_date(found["expiry"])
    if entry is None or expiry is None:
        return f"a workflow declaration with an invalid date: {line}"
    if expiry - entry != datetime.timedelta(days=CATEGORY_DAYS[found["category"]]):
        return f"category {found['category']} does not allow expiry {expiry} after entry {entry}"
    if today > expiry:
        return f"the {found['workflow']} declaration expired on {expiry}: {line}"
    if found["latest"] != found["run"]:
        return (
            f"the {found['workflow']} declaration covers run {found['run']}, "
            f"but the latest failure is run {found['latest']}"
        )
    return None


def judge(code, text, today=None):
    """Return the reasons the audit output fails, or an empty list."""
    if today is None:
        today = datetime.datetime.now(datetime.timezone.utc).date()
    problems = []
    if code != 0:
        problems.append(f"the audit exited {code}")
    lines = text.splitlines()[:MAX_LINES]
    for number, line in enumerate(lines, start=1):
        found = MARKER.match(line.strip())
        if found and found.group(1) not in ACCEPTED:
            problems.append(f"line {number} carries [{found.group(1)}]: {line.strip()}")
        declared = declared_problem(line.strip(), today)
        if declared:
            problems.append(f"line {number}: {declared}")
    if not any(line.startswith(SUMMARY) for line in lines):
        problems.append(f"the audit never printed its summary line ({SUMMARY!r})")
    return problems


def run_audit(command, timeout=AUDIT_TIMEOUT):
    """Run command, echo its combined output, and return (exit code, text)."""
    try:
        done = subprocess.run(
            command,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
            text=True,
            timeout=timeout,
            check=False,
        )
    except subprocess.TimeoutExpired:
        return 124, f"[FAIL] the audit did not finish within {timeout} s\n"
    except OSError as error:
        return 127, f"[FAIL] the audit could not start: {error}\n"
    sys.stdout.write(done.stdout)
    return done.returncode, done.stdout


def main(argv):
    if len(argv) < 2:
        print("usage: audit_strict.py <praetorctl> audit [arguments]", file=sys.stderr)
        return 2
    code, text = run_audit(argv[1:])
    problems = judge(code, text)
    for problem in problems:
        print(f"FAIL: {problem}")
    if problems:
        print("FAIL: the audit is not clean; HISS-10 tolerates zero warnings.")
        return 1
    print("PASS: the audit printed no warning and reached its summary.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
