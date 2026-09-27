#!/usr/bin/env python3
"""Run `praetorctl gate run` for the pre-push hook, choosing the mode by repository.

This lived as an inline `run:` block in `lefthook.yml`. It was a multi-statement
shell fragment carrying a quoted string with a comma in it, and lefthook did not
hand it to `sh` intact on Windows: the push failed with

    go.mod,: -c: line 1: unexpected EOF while looking for matching `"'

while the identical fragment runs correctly under `sh -c` on the same machine.
A hook that cannot run on a platform is not a passing hook (HISS-21), so the
logic moved here, where lefthook invokes one program and no shell re-parses it.

The mode is not cosmetic: praetor's gate mints an Exit-0 receipt and runs a
race-detector stage, neither of which applies to a repository with no Go module.
This one has none -- it is Rust and Python -- so the gate runs `--dry-run` and
says so, rather than minting a receipt for a stage that never ran.
"""

import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[3]
# Scalar upper bound on the gate invocation (HISS-02).
GATE_TIMEOUT_SECONDS = 900


def gate_argv():
    """Return the gate command for this repository, or None when praetorctl is absent."""
    executable = shutil.which("praetorctl") or shutil.which("standardsctl")
    if executable is None:
        return None, None
    argv = [executable, "gate", "run", f"--path={ROOT.as_posix()}"]
    if (ROOT / "go.mod").is_file():
        return argv, None
    argv.append("--dry-run")
    return argv, "no go.mod: race-detector stage skipped and no receipt minted"


def main():
    """Run the gate, reporting why a stage was skipped rather than skipping silently."""
    argv, skip_reason = gate_argv()
    if argv is None:
        print(
            "HISS-16 governance hook cannot run: neither praetorctl nor standardsctl "
            "is on PATH. Build it from the pinned Praetor commit and retry.",
            file=sys.stderr,
        )
        return 1
    if skip_reason is not None:
        print(f"SKIP: {skip_reason}", file=sys.stderr)
    try:
        completed = subprocess.run(argv, cwd=ROOT, timeout=GATE_TIMEOUT_SECONDS, check=False)
    except subprocess.TimeoutExpired:
        print(f"gate run exceeded {GATE_TIMEOUT_SECONDS}s and was stopped.", file=sys.stderr)
        return 1
    except OSError as error:
        print(f"gate run could not be started: {error}", file=sys.stderr)
        return 1
    return completed.returncode


if __name__ == "__main__":
    sys.exit(main())
