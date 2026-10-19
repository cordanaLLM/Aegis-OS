# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Shared verification helpers and primitives across gate tools."""

import hashlib
import json
import os
from pathlib import Path
import secrets
import signal
import subprocess
import time

from host import end_session, gid, uid

TERMINATING_SIGNALS = ("SIGTERM", "SIGHUP", "SIGQUIT")
HASH_CHUNK = 1 << 20
MAX_HASH_CHUNKS = 1 << 12
MAX_PROBLEM_LINES = 24
MAX_BUILD_LINES = 20000
STOP_TIMEOUT = 15


class GateError(Exception):
    """Failure inside a gate runner."""


def cache_dir(variable, name, environ=None):
    """Return an out-of-repository directory, overridable by `variable`."""
    env = os.environ if environ is None else environ
    override = env.get(variable)
    if override:
        return Path(override).expanduser()
    cache = env.get("XDG_CACHE_HOME")
    root = Path(cache).expanduser() if cache else Path.home() / ".cache"
    return root / name


def file_sha256(path, chunk_size=HASH_CHUNK, max_chunks=MAX_HASH_CHUNKS):
    """Return the sha256 of the file at `path`, read in bounded chunks."""
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for _ in range(max_chunks):
            chunk = handle.read(chunk_size)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def c_locale(environ=None):
    """Return this environment with a fixed locale, so tool output parses the same everywhere."""
    base = os.environ if environ is None else environ
    env = dict(base)
    env.update({"LC_ALL": "C", "LANG": "C", "NO_COLOR": "1"})
    return env


def terminated(signum, _frame=None):
    """Turn a termination signal into SystemExit."""
    raise SystemExit(128 + signum)


def install_signal_handlers(handler=terminated, signals=TERMINATING_SIGNALS):
    """Route SIGTERM, SIGHUP and SIGQUIT through `terminated`; return signals handled."""
    handled = []
    for name in signals:
        number = getattr(signal, name, None)
        if number is not None:
            signal.signal(number, handler)
            handled.append(name)
    return handled


def report(name, problems, notes=(), max_problem_lines=MAX_PROBLEM_LINES):
    """Print one case's outcome and, when it failed, why."""
    print(f"{'PASS' if not problems else 'FAIL'} {name}")
    for note in notes:
        print(f"     {note}")
    for line in problems[:max_problem_lines]:
        print(f"     {line}")


def tail(text, lines=6):
    """Return the last non-empty lines of a step's output, for a failure note."""
    kept = [line for line in text.splitlines() if line.strip()]
    return kept[-lines:]


def executable_from(messages, name, max_build_lines=MAX_BUILD_LINES):
    """Return the executable cargo reported for `name` in its JSON messages."""
    for line in messages.splitlines()[:max_build_lines]:
        if not line.startswith("{"):
            continue
        try:
            message = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = message.get("target") or {}
        if message.get("reason") == "compiler-artifact" and target.get("name") == name:
            if message.get("executable"):
                return Path(message["executable"])
    return None


def version_tuple(text):
    """Return a dotted version as a tuple of integers, for ordering."""
    return tuple(int(part) for part in text.split("."))


def check_toolchain(toolchain, read_version_fn, version_tuple_fn=version_tuple):
    """Print the admitted toolchain read back from the host; return the reasons it cannot run."""
    reasons = []
    for row in toolchain:
        name, _argv, _pattern, floor, reference = row
        banner, found = read_version_fn(row)
        if found is None:
            reasons.append(banner)
            continue
        limit = "" if floor is None else f", floor {floor}"
        note = "" if found == reference else f" [reference profile recorded {reference}]"
        print(f"     {name}: {found} (read back from {banner!r}{limit}){note}")
        if floor is not None and version_tuple_fn(found) < version_tuple_fn(floor):
            reasons.append(f"{name} {found} is below the admitted floor {floor}")
    return reasons


def run_command(argv, timeout=120, environ=None, cwd=None, gate_error_cls=GateError):
    """Run a program under timeout and return (returncode, stdout, stderr)."""
    env = dict(os.environ if environ is None else environ)
    env["LC_ALL"] = "C"
    env["LANG"] = "C"
    try:
        proc = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            env=env,
            cwd=cwd,
            stdin=subprocess.DEVNULL,
            check=False,
        )
        return proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired as error:
        raise gate_error_cls(f"{Path(argv[0]).name} exceeded {timeout}s timeout") from error
    except OSError as error:
        raise gate_error_cls(f"{argv[0]} could not be run: {error}") from error


def summary(case_results, case_count):
    """Return the RESULT line; every one of the case_count cases must have run and passed."""
    if len(case_results) != case_count:
        return f"RESULT fail: {len(case_results)} of {case_count} cases ran"
    failures = sum(1 for _name, passed, _why in case_results if not passed)
    if failures:
        return f"RESULT fail: {failures} case(s) failed"
    return "RESULT pass"


def stop(process, timeout=STOP_TIMEOUT, end_session_fn=end_session):
    """End `process` and everything in its session, waiting a bounded time for it."""
    if process.poll() is None:
        ended, _reason = end_session_fn(process.pid)
        if not ended:
            process.kill()
    try:
        process.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=timeout)


def user_arguments(engine, uid_fn=uid, gid_fn=gid):
    """Run docker as the invoking user; other container engines need no override."""
    if engine != "docker":
        return []
    user, _reason = uid_fn()
    group, _reason = gid_fn()
    return [] if user is None or group is None else ["--user", f"{user}:{group}"]


def report_definition(case, code, problems, diagnostics, max_diagnostic_lines=12):
    """Print one definition case's outcome, and its diagnostics when it failed."""
    status = "PASS" if not problems else "FAIL"
    print(f"{status} {case['name']}: exit {code}")
    if not problems:
        return
    for line in problems:
        print(f"     {line}")
    for line in diagnostics.splitlines()[:max_diagnostic_lines]:
        print(f"     | {line}")


def run_id():
    """Return a run identifier: the UTC start time and a random suffix."""
    return time.strftime("r%Y%m%dT%H%M%S", time.gmtime()) + "-" + secrets.token_hex(2)
