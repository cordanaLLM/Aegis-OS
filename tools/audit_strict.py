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

Usage: ``python3 tools/audit_strict.py <praetorctl> audit [arguments]``; the
command after the file name runs as given.
``tools/test_audit_strict.py`` covers the verdict.
"""

import re
import subprocess
import sys

# The markers an audit line may open with and still pass. UNSCANNED is
# accepted: it names source no HISS rule reads, an engine gap tracked in the
# engine's repository, not a finding about this one.
ACCEPTED = ("PASS", "INFO", "REPORT", "SKIP", "UNSCANNED")
MARKER = re.compile(r"^(?:Error:\s*)?\[([A-Z]+)\]")
SUMMARY = "Audit Summary: configured governance gates passed"
# The audit reads the whole tree and, with a token, the forge; this bounds it
# so a wedged audit fails the gate instead of hanging it (HISS-02).
AUDIT_TIMEOUT = 900
# Scalar bound on the lines judged (HISS-02).
MAX_LINES = 100_000


def judge(code, text):
    """Return the reasons the audit output fails, or an empty list."""
    problems = []
    if code != 0:
        problems.append(f"the audit exited {code}")
    lines = text.splitlines()[:MAX_LINES]
    for number, line in enumerate(lines, start=1):
        found = MARKER.match(line.strip())
        if found and found.group(1) not in ACCEPTED:
            problems.append(f"line {number} carries [{found.group(1)}]: {line.strip()}")
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
