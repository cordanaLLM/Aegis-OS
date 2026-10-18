#!/usr/bin/env python3
"""Hold each scoped deviation to a live, categorised exception at its pin (issue #223).

A scoped deviation is a roadmap decision that lets third-party code break an
invariant Aegis code keeps (D83: cros-libva heap-allocates for every decoded
picture on P17's frame path, HISS-03; D115: gpui rebuilds its AccessKit tree
every frame on the native shell's path). ``DEVIATIONS`` lists them; each must be
declared in the ``exceptions`` list of ``.standards.yaml`` as the entry for its
rule and path, and the check fails when:

- the entry is missing, or names no category or entered date;
- the entry has expired: the engine excuses nothing after the expiry, but no
  engine gate reads HISS-03, so without this check an expired entry would
  fail nothing;
- the reason does not name the decision, or does not name the source revision
  ``Cargo.lock`` pins for the package: a re-pin of the package fails here until
  the deviation is reviewed and the entry renewed with the new revision.

The engine validates the entry itself (rule, path, category interval and the
90-day bound); this check reads only what it needs, through a reader that
accepts the entry keys the engine declares and refuses any other.

Usage: ``python3 tools/verify_deviations.py [ROOT]``.
``tools/test_verify_deviations.py`` covers each refusal with a planted case.
"""

import datetime
import pathlib
import re
import sys
import tomllib

ROOT = pathlib.Path(__file__).resolve().parent.parent
MANIFEST = ".standards.yaml"
LOCK = "Cargo.lock"
DEVIATIONS = (
    {
        "decision": "D83",
        "rule": "HISS-03",
        "path": "crates/aegis-scaena/src/decode.rs",
        "package": "cros-libva",
    },
    {
        "decision": "D115",
        "rule": "HISS-03",
        "path": "crates/aegis-forum-shell-gpui/Cargo.toml",
        "package": "gpui",
    },
)
# The keys of one exceptions entry in the engine's schema (internal/config/exceptions.go).
ENTRY_KEYS = {
    "rule",
    "path",
    "glob",
    "function",
    "tool",
    "reason",
    "expires",
    "category",
    "entered",
}
MAX_LINES = 100000
ITEM = re.compile(r"  - ([a-z]+): (.*)")
FIELD = re.compile(r"    ([a-z]+): (.*)")
TOP_LEVEL = re.compile(r"[a-z_]+:.*")


class DeviationError(Exception):
    """A manifest or lock file cannot be read as expected."""


def scalar(text):
    """Return a YAML plain or quoted scalar; refuse what this reader does not model."""
    if len(text) >= 2 and text[0] == text[-1] == "'":
        return text[1:-1].replace("''", "'")
    if len(text) >= 2 and text[0] == text[-1] == '"' and "\\" not in text:
        return text[1:-1]
    if text and text[0] not in "'\"&*!|>{[#" and " #" not in text:
        return text
    raise DeviationError(f"exceptions value {text!r} is not a scalar this reader models")


def exception_entries(manifest_text):
    """Return the exceptions list of .standards.yaml as dicts of strings."""
    lines = manifest_text.splitlines()[:MAX_LINES]
    if "exceptions:" not in lines:
        return []
    entries = []
    for line in lines[lines.index("exceptions:") + 1 :]:
        if TOP_LEVEL.fullmatch(line):
            break
        match = ITEM.fullmatch(line) or FIELD.fullmatch(line)
        if match is None:
            raise DeviationError(f"exceptions line {line!r} is not one this reader models")
        key, value = match.groups()
        if key not in ENTRY_KEYS:
            raise DeviationError(f"exceptions key {key!r} is not one the engine declares")
        if line.startswith("  - "):
            entries.append({})
        if not entries or key in entries[-1]:
            raise DeviationError(f"exceptions key {key!r} outside an entry or repeated")
        entries[-1][key] = scalar(value)
    return entries


def locked_revision(lock_text, package):
    """Return the source revision Cargo.lock pins for package, or None."""
    lock = tomllib.loads(lock_text)
    for row in lock.get("package", []):
        if row.get("name") == package and "#" in row.get("source", ""):
            return row["source"].rsplit("#", 1)[1]
    return None


def deviation_problems(deviation, entries, revision, today):
    """Return why the exceptions entry of one deviation does not hold it."""
    name = f"{deviation['decision']} ({deviation['rule']}, {deviation['path']})"
    matching = [
        entry
        for entry in entries
        if entry.get("rule") == deviation["rule"] and entry.get("path") == deviation["path"]
    ]
    if len(matching) != 1:
        return [f"{name}: {len(matching)} exceptions entries, expected exactly one"]
    entry = matching[0]
    problems = []
    if not entry.get("category") or not entry.get("entered"):
        problems.append(f"{name}: the entry names no category or no entered date")
    try:
        expires = datetime.date.fromisoformat(entry.get("expires", ""))
    except ValueError:
        expires = None
    if expires is None or expires < today:
        problems.append(f"{name}: the entry expired or carries no date ({entry.get('expires')})")
    reason = entry.get("reason", "")
    if deviation["decision"] not in reason:
        problems.append(f"{name}: the reason does not name the decision")
    if revision is None or revision not in reason:
        problems.append(
            f"{name}: the reason does not name {deviation['package']} at the revision "
            f"Cargo.lock pins ({revision}); review the deviation and renew the entry"
        )
    return problems


def check(root, today):
    """Return every problem with the scoped deviations under root."""
    entries = exception_entries((root / MANIFEST).read_text(encoding="utf-8"))
    lock_text = (root / LOCK).read_text(encoding="utf-8")
    problems = []
    for deviation in DEVIATIONS:
        revision = locked_revision(lock_text, deviation["package"])
        problems += deviation_problems(deviation, entries, revision, today)
    return problems


def main(argv):
    """Run the check and report PASS or each FAIL line."""
    root = pathlib.Path(argv[1]) if len(argv) > 1 else ROOT
    try:
        problems = check(root, datetime.date.today())
    except (OSError, DeviationError, tomllib.TOMLDecodeError) as error:
        print(f"FAIL: {error!r}", file=sys.stderr)
        return 1
    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1
    names = ", ".join(deviation["decision"] for deviation in DEVIATIONS)
    print(f"PASS: {len(DEVIATIONS)} scoped deviation(s) live at their pinned revision: {names}.")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
