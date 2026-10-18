#!/usr/bin/env python3
"""Run shfmt over every tracked shell script, honouring dated ignores.

shfmt reads ``.editorconfig`` for its style, but applies ``ignore = true`` only
while it walks directories, never to the file names it is handed. ``make
verify-lints`` hands it ``git ls-files`` output, so this driver applies the
ignores itself, and treats each one as an exception of the fleet rule set: a
section that names exactly one tracked script, with the ``aegis_exception_*``
keys below. An ignore fails the gate when a key is missing, when its expiry
is past or does not match its category's interval, and when the script it
excuses is already clean under that section's style, so a stale ignore cannot
outlive its cause (ledger id aegis-os-shfmt-managed-hooks).

Usage: ``python3 tools/shfmt_scope.py check -- <shfmt command>``. The command
gets ``-d`` and the scripts outside the ignores; each ignored script is piped
to it on its own with ``--filename`` so the section's style applies.
``tools/test_shfmt_scope.py`` covers the reader, the judgement and the run.
"""

import datetime
import pathlib
import re
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parent.parent
EDITORCONFIG = ".editorconfig"
# Rectification interval in days per exception category; A states its own.
INTERVALS = {"B": 3, "C": 10, "D": 120}
KEYS = ("category", "entered", "expires", "reason")
PREFIX = "aegis_exception_"
# One shfmt run over every script, or a stdin run per ignored script.
TIMEOUT_SECONDS = 600
USAGE = "usage: shfmt_scope.py check -- <shfmt command>"
SECTION = re.compile(r"^\[(.+)\]$")
PAIR = re.compile(r"^([A-Za-z0-9_]+)\s*=\s*(.*)$")


class ScopeError(Exception):
    """An ignore is malformed, expired or stale, or a run fails."""


def sections(text):
    """Return ``{pattern: {key: value}}`` for every section of an .editorconfig."""
    found, current = {}, None
    for raw in text.splitlines():
        line = raw.strip()
        if not line or line[0] in "#;":
            continue
        head = SECTION.match(line)
        if head:
            current = found.setdefault(head.group(1), {})
            continue
        pair = PAIR.match(line)
        if pair and current is not None:
            current[pair.group(1).lower()] = pair.group(2).strip()
    return found


def ignores(text):
    """Return ``{path: section}`` for every section that sets ``ignore = true``."""
    return {
        name: keys
        for name, keys in sections(text).items()
        if keys.get("ignore", "").lower() == "true"
    }


def parse_day(path, key, value):
    """Return the date in `value`, or raise naming the path and key."""
    try:
        return datetime.date.fromisoformat(value)
    except ValueError:
        raise ScopeError(f"{path}: {PREFIX}{key} '{value}' is no YYYY-MM-DD date") from None


def judge(path, keys, tracked, today):
    """Raise unless the ignore of `path` is a complete, current exception."""
    if path not in tracked:
        raise ScopeError(f"{path}: an ignore must name exactly one tracked shell script")
    missing = [PREFIX + key for key in KEYS if not keys.get(PREFIX + key)]
    if missing:
        raise ScopeError(f"{path}: the ignore lacks {', '.join(missing)}")
    category = keys[PREFIX + "category"]
    if category not in INTERVALS and category != "A":
        raise ScopeError(f"{path}: category '{category}' is not one of A, B, C, D")
    entered = parse_day(path, "entered", keys[PREFIX + "entered"])
    expires = parse_day(path, "expires", keys[PREFIX + "expires"])
    interval = INTERVALS.get(category)
    if interval is not None and expires != entered + datetime.timedelta(days=interval):
        raise ScopeError(
            f"{path}: category {category} expires {interval} days after {entered}, not on {expires}"
        )
    if today > expires:
        raise ScopeError(f"{path}: the ignore expired on {expires}")


def tracked_scripts():
    """Return the tracked ``*.sh`` paths, POSIX-spelled as git prints them."""
    out = subprocess.run(
        ["git", "-C", str(ROOT), "ls-files", "-z", "*.sh"],
        capture_output=True,
        check=True,
        timeout=TIMEOUT_SECONDS,
    ).stdout
    return [name for name in out.decode("utf-8").split("\0") if name]


def run(argv, stdin=None):
    """Run `argv` from the repository root and return its exit code.

    A run fed on stdin judges an ignored script, whose known diff is not shown.
    """
    quiet = subprocess.DEVNULL if stdin is not None else None
    done = subprocess.run(
        argv, cwd=ROOT, input=stdin, stdout=quiet, timeout=TIMEOUT_SECONDS, check=False
    )
    return done.returncode


def read_script(path):
    """Return the bytes of the tracked script `path`."""
    return (ROOT / path).read_bytes()


def still_dirty(command, path, runner, read):
    """Raise when `path`, formatted with its own section's style, shows no diff."""
    data = read(path)
    if runner([*command, "-d", "--filename", path], data) == 0:
        raise ScopeError(f"{path}: shfmt reports no diff, so the ignore is stale; remove it")


def check(command, text, tracked, today, runner=run, read=read_script):
    """Judge every ignore, then run `command` over the rest; return the PASS line."""
    excused = ignores(text)
    for path, keys in excused.items():
        judge(path, keys, tracked, today)
    for path in excused:
        still_dirty(command, path, runner, read)
    scope = [path for path in tracked if path not in excused]
    if scope and runner([*command, "-d", *scope], None) != 0:
        raise ScopeError(f"shfmt reports drift in the {len(scope)} scripts it checked")
    return (
        f"PASS: shfmt clean on {len(scope)} scripts; "
        f"{len(excused)} dated ignore(s) current and still needed"
    )


def main(argv):
    """Run ``check -- <command>`` and return the exit code."""
    if len(argv) < 3 or argv[:2] != ["check", "--"]:
        print(USAGE, file=sys.stderr)
        return 2
    try:
        text = (ROOT / EDITORCONFIG).read_text(encoding="utf-8")
        line = check(argv[2:], text, tracked_scripts(), datetime.date.today())
    except (ScopeError, OSError, subprocess.SubprocessError) as error:
        print(f"FAIL: {error}", file=sys.stderr)
        return 1
    print(line, flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
