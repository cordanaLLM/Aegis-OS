#!/usr/bin/env python3
"""Supply the pinned versions of the CI lints to `make verify-lints`.

The versions live once, in the ``env`` block of ``.github/workflows/ci.yml``,
where Renovate and ``tools/praetor_bump.py`` already read and write them; the
``verify-lints`` recipes take them from here and nowhere else, so the local
gate and CI run the same linter at the same version (issue #226, ledger id
aegis-os-ci-lints-in-local-gate). The recipes name each linter themselves,
because the engine's declared-linters audit reads Make recipes, not scripts.

``preflight`` reads every pin and checks every launcher the recipes call; a
missing ``pipx``, ``npx``, ``shellcheck`` or ``git`` fails the gate instead of
skipping a lint. ``pin KEY`` prints one version and exits 1 when the key is
not set exactly once, so a broken pin also fails the recipe that uses it.

Usage: ``python3 tools/ci_lints.py preflight`` or
``python3 tools/ci_lints.py pin REUSE_VERSION``. ``tools/test_ci_lints.py``
covers the reader, the launcher check and both commands.
"""

import pathlib
import re
import shutil
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
CI_WORKFLOW = ".github/workflows/ci.yml"
# The env keys that pin the lint versions, in the order the recipes run them.
KEYS = (
    "REUSE_VERSION",
    "MARKDOWNLINT_VERSION",
    "YAMLLINT_VERSION",
    "FLAKE8_VERSION",
    "BLACK_VERSION",
    "SHFMT_VERSION",
    "CODESPELL_VERSION",
)
# Every program the verify-lints recipes start; shellcheck has no pin and
# comes from the host (docs/roadmap/toolchain-admission.md, not yet admitted).
LAUNCHERS = ("git", "npx", "pipx", "shellcheck")
USAGE = "usage: ci_lints.py preflight | ci_lints.py pin KEY"


class LintError(Exception):
    """A pin cannot be read, or a launcher is missing."""


def ci_env(text, key):
    """Return the one quoted value of `key:` in the workflow's env block."""
    pattern = re.compile(r"^\s*" + re.escape(key) + r':\s*"([^"\n]*)"\s*$', re.MULTILINE)
    values = pattern.findall(text)
    if len(values) != 1:
        raise LintError(f"{CI_WORKFLOW} sets {key} {len(values)} times; expected once")
    if not values[0]:
        raise LintError(f"{CI_WORKFLOW} sets {key} to an empty value")
    return values[0]


def versions(text):
    """Return every pinned lint version, by env key."""
    return {key: ci_env(text, key) for key in KEYS}


def missing_launchers(which=shutil.which):
    """Return the launchers the recipes call that are not on PATH."""
    return [name for name in LAUNCHERS if which(name) is None]


def workflow_text():
    """Return the committed CI workflow."""
    return (ROOT / CI_WORKFLOW).read_text(encoding="utf-8")


def preflight(text, which=shutil.which):
    """Return the PASS line, or raise naming every unreadable pin or missing launcher."""
    pinned = versions(text)
    missing = missing_launchers(which)
    if missing:
        raise LintError(f"not on PATH: {', '.join(missing)}; the CI lints cannot run")
    pins = ", ".join(f"{key}={value}" for key, value in pinned.items())
    return f"PASS: CI lint pins read from {CI_WORKFLOW} ({pins}); launchers present"


def pin(text, key):
    """Return the version `key` pins; only the lint keys are served."""
    if key not in KEYS:
        raise LintError(f"{key} is not a CI lint pin; expected one of {', '.join(KEYS)}")
    return ci_env(text, key)


def main(argv):
    """Run `preflight` or `pin KEY` and return the exit code."""
    try:
        if argv == ["preflight"]:
            print(preflight(workflow_text()), flush=True)
        elif len(argv) == 2 and argv[0] == "pin":
            print(pin(workflow_text(), argv[1]), flush=True)
        else:
            print(USAGE, file=sys.stderr)
            return 2
    except (LintError, OSError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
