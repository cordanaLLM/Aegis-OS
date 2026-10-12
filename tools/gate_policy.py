"""Decide whether a self-guarding gate that cannot run skips or fails.

The definition gates guard themselves: a host without the tool, or with one
below the admitted floor, prints why the gate did not run and exits 0, so
`make verify-all` stays usable on a development machine. On a runner whose
status check is required, that same exit 0 reports green while the gate never
ran.

`AEGIS_REQUIRED_GATES` lists, separated by commas, the gates a run requires. A
listed gate that cannot run prints FAIL and exits 1; an unlisted one prints a
named SKIP and exits 0, as before. A name that is no gate fails every gate that
reads the list, because a misspelt name would otherwise leave the gate it meant
unrequired: the same green-while-not-run this module exists to stop.
"""

import os

VARIABLE = "AEGIS_REQUIRED_GATES"
# Every gate that reads the variable. A name outside this set is a typo.
GATES = ("systemd", "mkosi")


def named(environ=None):
    """Return the names listed in VARIABLE, blanks dropped and whitespace stripped."""
    environ = os.environ if environ is None else environ
    names = (name.strip() for name in environ.get(VARIABLE, "").split(","))
    return [name for name in names if name]


def unknown(environ=None):
    """Return the listed names that are no gate, in the order given."""
    return [name for name in named(environ) if name not in GATES]


def is_required(gate, environ=None):
    """Return whether `gate` is listed in VARIABLE."""
    return gate in named(environ)


def refuse_unknown(environ=None):
    """Print FAIL and return 1 when VARIABLE names no gate; return 0 otherwise."""
    strangers = unknown(environ)
    if not strangers:
        return 0
    print(
        f"FAIL: {VARIABLE} names {', '.join(repr(name) for name in strangers)}, which "
        f"is no gate; the known gates are {', '.join(GATES)}."
    )
    return 1


def did_not_run(gate, reason, note="", environ=None):
    """Print why `gate` did not run and return its exit code: 1 when required, else 0."""
    tail = f" {note}" if note else ""
    if is_required(gate, environ):
        print(
            f"FAIL: {reason}; the {gate} definition gate is required in this run "
            f"({VARIABLE}) and did not run.{tail}"
        )
        return 1
    print(f"SKIP: {reason}; the {gate} definition gate did not run.{tail}")
    return 0
