#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Run Aegis's two M18 payloads through the pinned cordanaLLM/imago, offline.

Milestone M09, scoped by decision D92 to the consumption contract and placed by
D93. ``build/contract/producers.pin.json`` names the imago commit, its Go floor,
the bounds its product-input decoder enforces and the three fixtures imago
vendors from this repository, and names the nucleus commit for identity only:
nucleus reads no Aegis payload.

``--fetch`` (``make contract-fetch``) is the only networked step. It runs
``git ls-remote --heads`` against both producers and keeps the output, clones
imago at the pinned commit into a fresh directory in a cache outside the
repository, and builds it with
``GOTOOLCHAIN=local CGO_ENABLED=0 go build -trimpath -buildvcs=true``, so the
binary carries the commit it was built from; the identity record it writes last
keeps the binary's sha256. The gate never touches the network: without go or git
on PATH, or without the cached checkout, binary or identity record, it prints
why it did not run and exits 0 (HISS-21). A cache that is present but wrong is a
FAIL naming ``make contract-fetch``: an identity record fetched for another pin,
a checkout at another commit or with anything in it the commit lacks, a binary
whose digest is not the one the fetch recorded or whose build information is
not the pinned build.

Every git command, go's own included, runs with the system and user git
configuration shut out and without an inherited ``GIT_*`` variable, so neither a
workstation setting nor a hook's ``GIT_DIR`` decides what the checks see.

Cases, every one asserting an exit code and the text beside it:

* ``contract/pin``: the pin and this repository's payloads hash as recorded;
* ``contract/identity``: the retained ``git ls-remote`` record for both
  producers, fetched for this pin, with the binary's recorded digest;
* ``contract/checkout``: the cached checkout is the pinned commit with no
  tracked change, untracked or ignored file or hidden index entry, and the Go
  floor and the bounds the pin records;
* ``contract/binary-provenance``: the binary hashes to the digest the fetch
  recorded, and ``go version -m`` reads the pinned ``vcs.revision``, its commit
  time, the matching module version and ``vcs.modified=false`` out of it;
* ``contract/simulated-output-refused``: a stand-in binary rebuilt from other
  sources that prints what imago printed, byte for byte, is refused for want of
  that provenance (E09-3);
* ``contract/fixtures-identical``: imago's vendored fixtures are byte-identical
  to ``build/product-input.json`` and the kernel requirement pair;
* ``product-input/accepted``, ``product-input/tampered-refused``,
  ``product-input/retry-bound``, ``product-input/packages-bound`` and
  ``product-input/aegis-bounds-inside-imago`` (E09-1);
* ``kernel-requirement/accepted``, ``kernel-requirement/invalid-feature-refused``
  and ``kernel-requirement/empty-features-refused`` (E09-2 as D92 re-scopes it).

Every invocation's argv, exit code, stdout and stderr, every payload fed to
imago and the identity record are retained under ``AEGIS_CONTRACT_DIR``
(default ``${XDG_CACHE_HOME:-$HOME/.cache}/aegis-contract``) in a directory named
by the printed run id. Nothing is written into the repository and nothing is
sent to either producer. A pass is consumption evidence; no product result, no
image and no kernel came back (D92 moved those to M11 and M10).
"""

import argparse
import hashlib
import json
import os
import re
import secrets
import shutil
import signal
import subprocess
import sys
import time
from pathlib import Path

from host import end_session

ROOT = Path(__file__).resolve().parent.parent
PIN = ROOT / "build" / "contract" / "producers.pin.json"
PIN_SCHEMA = "aegis.m09.contract-pin.v1"
PRODUCT_INPUT = ROOT / "build" / "product-input.json"
KERNEL_REQUIREMENT = ROOT / "build" / "kernel-requirement.json"
KERNEL_REFERENCE = ROOT / "build" / "kernel-requirement.reference.json"
# The Rust constants the M18 schema enforces on the Aegis side.
AEGIS_MANIFEST_SOURCE = ROOT / "crates" / "aegis-fabrica-defs" / "src" / "manifest.rs"
AEGIS_BOUNDS = ("MAX_PACKAGES", "MAX_RETRY_ATTEMPTS", "MAX_BACKOFF_SECONDS")
CACHE_VARIABLE = "AEGIS_CONTRACT_DIR"
PRODUCERS = ("imago", "nucleus")
IMAGO_MODULE = "github.com/cordanaLLM/imago"
PRODUCT_PREFIX = "aegis product-input"
KERNEL_PREFIX = "kernel requirement"
# imago's main() ends a rejected command with os.Exit(1); anything else is a crash.
REFUSAL_EXIT = 1
# go build writes the name -o gives it, and Windows starts only a named .exe.
EXE = ".exe" if os.name == "nt" else ""
# `git status` as the checkout check runs it: every untracked and every ignored
# file counts, and a repository-local fsmonitor does not answer for the tree.
STRICT_STATUS = (
    "-c",
    "core.fsmonitor=false",
    "status",
    "--porcelain",
    "--untracked-files=all",
    "--ignored",
)

# Deadlines, in seconds. Every command the gate or the fetch starts carries one.
VERSION_TIMEOUT = 30
GIT_TIMEOUT = 60
NETWORK_TIMEOUT = 300
DOWNLOAD_TIMEOUT = 600
BUILD_TIMEOUT = 600
VALIDATE_TIMEOUT = 60
STOP_TIMEOUT = 15
TERMINATING_SIGNALS = ("SIGTERM", "SIGHUP", "SIGQUIT")

# Scalar bounds (HISS-02).
MAX_OUTPUT_CHARS = 1 << 20
MAX_PROBLEM_LINES = 24
MAX_FIXTURES = 8
MAX_LISTED_LINES = 4096
MAX_RETAINED_RUNS = 16
MAX_PRUNED = 4096

REPOSITORIES = {name: f"https://github.com/cordanaLLM/{name}.git" for name in PRODUCERS}
COMMIT = re.compile(r"[0-9a-f]{40}")
SHA256 = re.compile(r"[0-9a-f]{64}")
FLOOR = re.compile(r"(\d+)\.(\d+)\.(\d+)")
GO_RELEASE = re.compile(r"go(\d+)\.(\d+)(?:\.(\d+))?")
RELATIVE = re.compile(r"[A-Za-z0-9_+-][A-Za-z0-9_.+-]*(?:/[A-Za-z0-9_+-][A-Za-z0-9_.+-]*)*")
GO_DIRECTIVE = re.compile(r"^go (\S+)$", re.M)
RUST_CONST = re.compile(r"^pub const (?P<name>[A-Z_]+): \w+ = (?P<value>\d+);$", re.M)


class GateError(Exception):
    """The gate could not be run at all, as opposed to a case that failed."""


def native(path):
    """Return `path` as this host spells it, for git, go and imago running on this host.

    Deliberately not host.target(): that renders a path for a Linux reader and
    drops a Windows drive letter, which a program on the Windows host itself
    would resolve against the wrong drive. Nothing this gate emits is read by
    Linux on another machine.
    """
    return os.fspath(path)


def read_bytes(path):
    """Return a file's bytes, or raise GateError naming it."""
    try:
        return Path(path).read_bytes()
    except OSError as error:
        raise GateError(f"{native(path)} could not be read: {error}") from error


def load_json(path):
    """Return the parsed JSON document at `path`."""
    try:
        return json.loads(read_bytes(path).decode("utf-8"))
    except ValueError as error:
        raise GateError(f"{native(path)} is not JSON: {error}") from error


def sha256_of(data):
    """Return the lowercase hex sha256 of `data`."""
    return hashlib.sha256(data).hexdigest()


def producer_problems(name, row):
    """Return what is wrong with one producer's identity in the pin."""
    if not isinstance(row, dict):
        return [f"the pin has no {name} section"]
    problems = []
    if row.get("repository") != REPOSITORIES[name]:
        problems.append(f"{name} repository {row.get('repository')!r} is not {REPOSITORIES[name]}")
    if not COMMIT.fullmatch(str(row.get("commit", ""))):
        problems.append(f"{name} commit {row.get('commit')!r} is not 40 lowercase hex digits")
    return problems


def relative_problem(value):
    """Return why `value` is not a repository-relative POSIX path, or None."""
    text = str(value)
    if not RELATIVE.fullmatch(text) or ".." in text.split("/"):
        return f"{text!r} is not a relative path without '..'"
    return None


def fixture_problems(rows):
    """Return what is wrong with the pin's fixture list."""
    if not isinstance(rows, list) or not 1 <= len(rows) <= MAX_FIXTURES:
        return [f"imago fixtures must list 1..{MAX_FIXTURES} entries"]
    problems = []
    for row in rows:
        row = row if isinstance(row, dict) else {}
        problems += [relative_problem(row.get(key)) for key in ("aegis", "imago")]
        if not SHA256.fullmatch(str(row.get("sha256", ""))):
            problems.append(f"fixture sha256 {row.get('sha256')!r} is not 64 hex digits")
    return [problem for problem in problems if problem]


def imago_problems(row):
    """Return what is wrong with the imago section beyond its identity."""
    problems = []
    if not FLOOR.fullmatch(str(row.get("go", ""))):
        problems.append(f"imago go floor {row.get('go')!r} is not x.y.z")
    if not str(row.get("main-package", "")).startswith(f"{IMAGO_MODULE}/"):
        problems.append(f"imago main-package {row.get('main-package')!r} is not in {IMAGO_MODULE}")
    bounds = row.get("bounds") if isinstance(row.get("bounds"), dict) else {}
    problems.append(relative_problem(bounds.get("source")))
    for key in ("MaxRetryAttempts", "MaxPackages"):
        value = bounds.get(key)
        if not isinstance(value, int) or isinstance(value, bool) or value < 1:
            problems.append(f"imago bound {key} {value!r} is not a positive integer")
    return [problem for problem in problems if problem] + fixture_problems(row.get("fixtures"))


def pin_problems(pin):
    """Return every problem in a parsed pin; an empty list admits it."""
    if not isinstance(pin, dict) or pin.get("schema") != PIN_SCHEMA:
        return [f"the pin does not declare schema {PIN_SCHEMA}"]
    problems = producer_problems("imago", pin.get("imago"))
    problems += producer_problems("nucleus", pin.get("nucleus"))
    if not problems:
        problems += imago_problems(pin["imago"])
    return problems


def load_pin(path=PIN):
    """Return the admitted pin, or raise GateError with every problem found."""
    pin = load_json(path)
    problems = pin_problems(pin)
    if problems:
        raise GateError("; ".join(problems[:MAX_PROBLEM_LINES]))
    return pin


def payload_problems(pin, root=ROOT):
    """Return where this repository's payloads no longer hash to what the pin records.

    imago vendors these bytes; a payload changed here without a re-pin would
    leave the producer's copy, and so everything this gate proves, behind.
    """
    problems = []
    for row in pin["imago"]["fixtures"]:
        found = sha256_of(read_bytes(Path(root) / row["aegis"]))
        if found != row["sha256"]:
            problems.append(
                f"{row['aegis']} hashes {found}, the pin records {row['sha256']}; "
                "re-pin once imago vendors the new bytes"
            )
    return problems


def floor_tuple(text):
    """Return the pin's x.y.z Go floor as a tuple of three integers."""
    return tuple(int(part) for part in FLOOR.fullmatch(text).groups())


def go_release(text):
    """Return (major, minor, patch) from a Go version such as go1.27.1-X:nodwarf5, or None.

    A release named without a patch, go1.27, is read as patch 0.
    """
    match = GO_RELEASE.match(text.strip())
    if match is None:
        return None
    major, minor, patch = match.groups()
    return int(major), int(minor), int(patch or 0)


def parse_buildinfo(text):
    """Return the build information `go version -m` printed, or None when it printed none.

    The first line is '<file>: <go version>'; each later line is tab-separated:
    path, mod, dep and build rows. The path and mod rows keep their version
    column as '<row> version'; dep rows are ignored.
    """
    lines = text.splitlines()[:MAX_LISTED_LINES]
    if not lines or ": " not in lines[0]:
        return None
    info = {"go": lines[0].rsplit(": ", 1)[1].strip(), "build": {}}
    for line in lines[1:]:
        fields = line.split("\t") + ["", ""]
        row, value = fields[1], fields[2]
        if fields[0] != "" or not value:
            continue
        if row in ("path", "mod"):
            info[row], info[f"{row} version"] = value, fields[3]
        elif row == "build" and "=" in value:
            key, _, setting = value.partition("=")
            info["build"][key] = setting
    return info if "path" in info else None


def pseudo_version(committed, commit):
    """Return the version go stamps on a main module built at an untagged commit, or None.

    The fetch brings no tags, so go writes v0.0.0-<commit time>-<12 hex of the
    commit>, with '+dirty' appended for a modified tree.
    """
    if committed is None:
        return None
    return f"v0.0.0-{re.sub(r'[^0-9]', '', committed)}-{commit[:12]}"


def provenance_problems(info, pin, committed):
    """Return why a binary's build information is not the pinned imago build, or [].

    `committed` is the pinned commit's committer time as go writes vcs.time,
    read from the checkout; None, when it could not be read, fails both rows
    that need it. go writes the revision, that time and the module version from
    the checkout it built in, so a binary rebuilt from other sources carries
    other values; byte-patching all three into a copy is what the digest the
    fetch recorded is there to catch.
    """
    if info is None:
        return ["not a Go executable with build information"]
    imago = pin["imago"]
    build = info["build"]
    rows = (
        ("path", info.get("path"), imago["main-package"]),
        ("mod", info.get("mod"), IMAGO_MODULE),
        ("mod version", info.get("mod version"), pseudo_version(committed, imago["commit"])),
        ("vcs", build.get("vcs"), "git"),
        ("vcs.revision", build.get("vcs.revision"), imago["commit"]),
        ("vcs.time", build.get("vcs.time"), committed),
        ("vcs.modified", build.get("vcs.modified"), "false"),
        ("CGO_ENABLED", build.get("CGO_ENABLED"), "0"),
        ("-trimpath", build.get("-trimpath"), "true"),
    )
    problems = [
        f"{key} is {found!r}, the pinned build has {wanted!r}" for key, found, wanted in rows
    ]
    problems = [line for line, (_, found, wanted) in zip(problems, rows) if found != wanted]
    release = go_release(info.get("go", ""))
    if release is None or release < floor_tuple(imago["go"]):
        problems.append(f"built by {info.get('go')!r}, below the go {imago['go']} imago declares")
    return problems


def rust_bounds(text):
    """Return the Aegis schema's upper bounds from the M18 manifest source."""
    found = {match["name"]: int(match["value"]) for match in RUST_CONST.finditer(text)}
    missing = [name for name in AEGIS_BOUNDS if name not in found]
    if missing:
        raise GateError(f"the M18 manifest source declares no {', '.join(missing)}")
    return {name: found[name] for name in AEGIS_BOUNDS}


def go_constants(text, names):
    """Return {name: int} for Go constants declared as '<name> = <integer>' in `text`."""
    found = {}
    for name in names:
        match = re.search(rf"^\s*{re.escape(name)}\s*=\s*(\d+)\b", text, re.M)
        found[name] = int(match.group(1)) if match else None
    return found


def go_duration(seconds):
    """Render whole seconds the way Go's time.Duration.String() does: 30s, 1m30s, 1h0m0s."""
    hours, rest = divmod(int(seconds), 3600)
    minutes, secs = divmod(rest, 60)
    if hours:
        return f"{hours}h{minutes}m{secs}s"
    if minutes:
        return f"{minutes}m{secs}s"
    return f"{secs}s"


def build_request(payload):
    """Return the build request imago's --json output must print for an accepted payload.

    It mirrors imago's ProductInput.BuildRequest() at the pinned commit: every
    field of the payload, with the snake_case keys and the Go duration imago
    writes, so an accepted run is compared field by field rather than by exit
    code.
    """
    definitions = payload["definitions"]
    return {
        "correlation_id": payload["correlation-id"],
        "revision": payload["revision"],
        "distribution": dict(payload["distribution"]),
        "definitions": {
            "repart": definitions["repart"],
            "sysupdate": definitions["sysupdate"],
            "mkosi": definitions["mkosi"],
            "kernel_requirement": definitions["kernel-requirement"],
        },
        "packages": list(payload["packages"]),
        "kernel_source": payload["kernel"]["source"],
        "kernel_package": payload["kernel"]["default-package"],
        "retry": {
            "attempts": payload["retries"]["max-attempts"],
            "backoff": go_duration(payload["retries"]["backoff-seconds"]),
        },
    }


def accepted_problems(result, payload):
    """Return why an `imago aegis validate --json` run did not accept `payload`, or []."""
    if result["exit"] != 0:
        return [f"expected exit 0, observed {result['exit']}"] + tail(result["stderr"])
    try:
        printed = json.loads(result["stdout"])
    except ValueError:
        return ["exit 0 but stdout is not the build request JSON"] + tail(result["stdout"])
    if printed != build_request(payload):
        return [f"the printed build request differs from the payload: {printed!r}"[:400]]
    if "Error:" in result["stderr"]:
        return ["exit 0 but stderr carries an error"] + tail(result["stderr"])
    return []


def refused_problems(result, prefix, correlation_id, reason):
    """Return why a run was not refused with the payload's correlated error, or [].

    A refusal is exit 1 with 'Error: <prefix> <correlation-id>: <field>: <reason>'
    on stderr; a crash, an acceptance or the right code with the wrong text all
    fail.
    """
    needle = f"Error: {prefix} {correlation_id}: {reason}"
    problems = []
    if result["exit"] != REFUSAL_EXIT:
        problems.append(f"expected exit {REFUSAL_EXIT}, observed {result['exit']}")
    if needle not in result["stderr"]:
        problems += [f"expected {needle!r} on stderr"] + tail(result["stderr"], 2)
    return problems


def feature_line(feature):
    """Return the four columns imago prints for one accepted feature."""
    return [feature["symbol"], feature["state"], feature["probe"], feature["required-by"]]


def kernel_accepted_problems(result, requirement):
    """Return why `imago kernel requirement validate` did not accept `requirement`, or []."""
    if result["exit"] != 0:
        return [f"expected exit 0, observed {result['exit']}"] + tail(result["stderr"])
    lines = result["stdout"].splitlines()
    header = (
        f"✓ Kernel requirement {requirement['correlation-id']} is valid "
        f"({requirement['schema']})."
    )
    listed = [line.split() for line in lines]
    wanted = [header, f"  Features: {len(requirement['features'])}"]
    problems = [f"stdout lacks {line!r}" for line in wanted if line not in lines]
    for feature in requirement["features"]:
        if feature_line(feature) not in listed:
            problems.append(f"stdout does not list {' '.join(feature_line(feature))}")
    return problems


def tail(text, lines=4):
    """Return the last non-empty lines of `text`, for a failure note."""
    kept = [line for line in text.splitlines() if line.strip()]
    return kept[-lines:]


def ls_remote_main(text):
    """Return the sha `git ls-remote --heads` printed for refs/heads/main, or None."""
    for line in text.splitlines()[:MAX_LISTED_LINES]:
        fields = line.split("\t")
        if len(fields) == 2 and fields[1] == "refs/heads/main" and COMMIT.fullmatch(fields[0]):
            return fields[0]
    return None


def recorded_rows(record):
    """Return {producer: its row} from a retained identity record, {} for each row it lacks."""
    producers = record.get("producers") if isinstance(record, dict) else None
    producers = producers if isinstance(producers, dict) else {}
    rows = {name: producers.get(name) for name in PRODUCERS}
    return {name: row if isinstance(row, dict) else {} for name, row in rows.items()}


def recorded_binary(record):
    """Return the sha256 the fetch recorded for the binary it built, or None."""
    row = record.get("binary") if isinstance(record, dict) else None
    digest = row.get("sha256") if isinstance(row, dict) else None
    return digest if SHA256.fullmatch(str(digest)) else None


def producer_record_problems(name, row, pin):
    """Return where one producer's retained row disagrees with the pin, or []."""
    problems = []
    if row.get("repository") != pin[name]["repository"]:
        problems.append(f"{name}: the record names {row.get('repository')!r}")
    if row.get("exit") != 0 or not COMMIT.fullmatch(str(row.get("main", ""))):
        problems.append(f"{name}: git ls-remote exit {row.get('exit')}, main {row.get('main')}")
    if row.get("pinned") != pin[name]["commit"]:
        problems.append(
            f"{name}: the cache was fetched for {row.get('pinned')}, the pin names "
            f"{pin[name]['commit']}; run `make contract-fetch`"
        )
    return problems


def identity_problems(record, pin):
    """Return where a retained identity record disagrees with the pin, or [].

    A record fetched for another pin is a cache that is present but wrong, and
    fails (D93); so does one without the binary digest the gate compares.
    """
    if not isinstance(record, dict) or not isinstance(record.get("producers"), dict):
        return ["the identity record lists no producers; run `make contract-fetch`"]
    problems = []
    for name, row in recorded_rows(record).items():
        problems += producer_record_problems(name, row, pin)
    if recorded_binary(record) is None:
        problems.append("the identity record carries no binary sha256; run `make contract-fetch`")
    return problems


def child_environment(overrides=None):
    """Return this environment for a child: no inherited GIT_* variable, C locale, `overrides`.

    A git hook exports GIT_DIR, GIT_INDEX_FILE and GIT_CONFIG_PARAMETERS;
    inherited, they would point `git -C <checkout>`, the fetch's `git init` and
    go's own git calls at the repository that ran the hook instead.
    """
    env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    env.update({"LC_ALL": "C", "LANG": "C", "GIT_TERMINAL_PROMPT": "0"})
    env.update(overrides or {})
    return env


def git_isolation(context):
    """Return the settings that shut the system and the user git configuration out.

    GIT_CONFIG_GLOBAL names an empty file under the run directory, written on
    first use; the repository's own configuration is the one the fetch created.
    """
    if not context.gitconfig.is_file():
        write_bytes(context.gitconfig, b"")
    return {"GIT_CONFIG_NOSYSTEM": "1", "GIT_CONFIG_GLOBAL": native(context.gitconfig)}


def go_environment(offline):
    """Return the Go settings every go command runs with; `offline` also forbids the proxy.

    GOTOOLCHAIN=local keeps go from downloading another toolchain when a go.mod
    asks for a newer one, which is what makes the Go floor a refusal instead of
    a silent download.
    """
    env = {"GOTOOLCHAIN": "local", "CGO_ENABLED": "0", "GOFLAGS": "-mod=readonly", "GOWORK": "off"}
    if offline:
        env["GOPROXY"] = "off"
    return env


def stop(process):
    """End `process` and everything in its session, waiting a bounded time for it."""
    if process.poll() is None:
        ended, _reason = end_session(process.pid)
        if not ended:
            process.kill()
    try:
        process.wait(timeout=STOP_TIMEOUT)
    except subprocess.TimeoutExpired:
        process.kill()
        process.wait(timeout=STOP_TIMEOUT)


def run(argv, timeout, env=None, cwd=None):
    """Run `argv` in its own session under a hard deadline; return (code, stdout, stderr)."""
    try:
        process = subprocess.Popen(
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=child_environment(env),
            cwd=cwd,
            start_new_session=True,
        )
    except OSError as error:
        raise GateError(f"{argv[0]} could not be run: {error}") from error
    try:
        stdout, stderr = process.communicate(timeout=timeout)
    except subprocess.TimeoutExpired as error:
        stop(process)
        raise GateError(f"{argv[0]} exceeded its {timeout}s deadline") from error
    except BaseException:
        stop(process)
        raise
    return process.returncode, decode(stdout), decode(stderr)


def decode(data):
    """Return captured output as text, keeping at most MAX_OUTPUT_CHARS of its tail."""
    return data.decode("utf-8", errors="replace")[-MAX_OUTPUT_CHARS:]


def cache_dir():
    """Return the directory the checkout, the binary and the run logs live in."""
    override = os.environ.get(CACHE_VARIABLE)
    if override:
        return Path(override).expanduser()
    base = os.environ.get("XDG_CACHE_HOME") or str(Path.home() / ".cache")
    return Path(base) / "aegis-contract"


class Context:
    """One run: the pin, the cache it reads and the run directory it writes."""

    def __init__(self, pin, store, run_dir):
        self.pin, self.store, self.run_dir = pin, store, run_dir
        self.checkout = store / "imago"
        self.binary = store / "bin" / f"imago{EXE}"
        self.identity = store / "identity.json"
        self.gitconfig = run_dir / "empty.gitconfig"
        self.outcomes = []


def write_bytes(path, data):
    """Write `data` to `path` through a temporary name, creating its directory."""
    path.parent.mkdir(parents=True, exist_ok=True)
    partial = path.with_name(path.name + ".partial")
    partial.write_bytes(data)
    partial.replace(path)


def retain(context, label, argv, code, stdout, stderr):
    """Keep one invocation's argv, exit code, stdout and stderr under the run directory."""
    record = {"argv": list(argv), "exit": code, "stdout": stdout, "stderr": stderr}
    write_bytes(context.run_dir / "logs" / f"{label}.json", json_bytes(record))


def json_bytes(value):
    """Return `value` as indented JSON bytes with a final newline."""
    return (json.dumps(value, indent=2, ensure_ascii=False) + "\n").encode("utf-8")


def invoke(context, label, argv, timeout, env=None, cwd=None):
    """Run one command under its deadline, retain it, and return {exit, stdout, stderr}.

    Every command runs with the workstation's git configuration shut out, so
    the git calls go makes for -buildvcs see what the gate's own calls see.
    """
    isolated = dict(git_isolation(context), **(env or {}))
    code, stdout, stderr = run(argv, timeout, env=isolated, cwd=cwd)
    retain(context, label, argv, code, stdout, stderr)
    return {"exit": code, "stdout": stdout, "stderr": stderr}


def report(context, name, problems, notes=()):
    """Print one case's outcome and, when it failed, why; keep it for the run summary."""
    print(f"{'PASS' if not problems else 'FAIL'} {name}")
    for note in notes:
        print(f"     {note}")
    for line in problems[:MAX_PROBLEM_LINES]:
        print(f"     {line}")
    context.outcomes.append({"case": name, "problems": list(problems), "notes": list(notes)})
    return problems


def buildinfo(context, label, binary):
    """Return the parsed `go version -m` of `binary` and the raw result."""
    result = invoke(
        context,
        label,
        ["go", "version", "-m", native(binary)],
        VERSION_TIMEOUT,
        env=go_environment(offline=True),
    )
    info = parse_buildinfo(result["stdout"]) if result["exit"] == 0 else None
    return info, result


def hidden_entries(listing):
    """Return the `git ls-files -v` lines whose entry git status would not report.

    'H' is a plain cached entry; a lowercase tag marks assume-unchanged and 'S'
    skip-worktree, and either hides a changed file from `git status` and from
    go's vcs.modified alike.
    """
    lines = listing.splitlines()[:MAX_LISTED_LINES]
    return [line for line in lines if not line.startswith("H ")]


def checkout_problems(context, label):
    """Return why the cached checkout is not exactly the pinned commit, or [].

    Nothing may differ from the commit: no tracked change, no untracked or
    ignored file (go would compile an extra .go file and still stamp
    vcs.modified=false for an ignored one), and no hidden index entry.
    """
    checkout = context.checkout
    head = invoke(context, f"{label}-head", git(checkout, "rev-parse", "HEAD"), GIT_TIMEOUT)
    status = invoke(context, f"{label}-status", git(checkout, *STRICT_STATUS), GIT_TIMEOUT)
    index = invoke(context, f"{label}-index", git(checkout, "ls-files", "-v"), GIT_TIMEOUT)
    problems = []
    if head["exit"] != 0 or head["stdout"].strip() != context.pin["imago"]["commit"]:
        problems.append(f"HEAD is {head['stdout'].strip() or head['stderr'].strip()!r}")
    if status["exit"] != 0 or status["stdout"].strip():
        problems.append(f"the checkout is modified: {tail(status['stdout'] + status['stderr'])}")
    hidden = hidden_entries(index["stdout"])
    if index["exit"] != 0 or hidden:
        problems.append(
            f"the index hides entries from git status: {(hidden or [index['stderr']])[:4]}"
        )
    return problems


def commit_time(context, label):
    """Return the checkout's HEAD committer time as go writes vcs.time, or None."""
    argv = git(context.checkout, "log", "-1", "--format=%ct", "HEAD")
    shown = invoke(context, label, argv, GIT_TIMEOUT)
    seconds = shown["stdout"].strip()
    if shown["exit"] != 0 or not seconds.isdigit():
        return None
    return time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime(int(seconds)))


def git(checkout, *arguments):
    """Return a git argv that runs in `checkout`."""
    return ["git", "-C", native(checkout), *arguments]


def source_problems(context):
    """Return where the checkout's go.mod and decoder bounds differ from the pin, or []."""
    imago = context.pin["imago"]
    go_mod = read_bytes(context.checkout / "go.mod").decode("utf-8", errors="replace")
    directive = GO_DIRECTIVE.search(go_mod)
    problems = []
    if directive is None or directive.group(1) != imago["go"]:
        problems.append(
            f"go.mod declares {directive and directive.group(1)!r}, pinned {imago['go']}"
        )
    bounds = imago["bounds"]
    source = read_bytes(context.checkout / bounds["source"]).decode("utf-8", errors="replace")
    names = ("MaxRetryAttempts", "MaxPackages")
    for name, value in go_constants(source, names).items():
        if value != bounds[name]:
            problems.append(f"{bounds['source']} declares {name} = {value}, pinned {bounds[name]}")
    return problems


def pin_case(context):
    """The pin, and this repository's payloads against the digests it records."""
    pin = context.pin
    head = invoke(context, "aegis-head", git(ROOT, "rev-parse", "HEAD"), GIT_TIMEOUT)
    dirty = invoke(context, "aegis-status", git(ROOT, "status", "--porcelain"), GIT_TIMEOUT)
    notes = [
        f"imago {pin['imago']['repository']} at {pin['imago']['commit']}, go {pin['imago']['go']}",
        f"nucleus {pin['nucleus']['repository']} at {pin['nucleus']['commit']} (identity only)",
        f"Aegis revision {head['stdout'].strip()}, "
        f"{len(dirty['stdout'].splitlines())} uncommitted path(s)",
    ]
    notes += [f"{row['aegis']}: sha256 {row['sha256']}" for row in pin["imago"]["fixtures"]]
    return report(context, "contract/pin", payload_problems(pin), notes)


def identity_case(context):
    """The retained `git ls-remote --heads` record for both producers, fetched for this pin."""
    record = load_json(context.identity)
    write_bytes(context.run_dir / "identity.json", json_bytes(record))
    problems = identity_problems(record, context.pin)
    record = record if isinstance(record, dict) else {}
    notes = [f"fetched by run {record.get('run')} at {record.get('fetched')}"]
    for name, row in recorded_rows(record).items():
        pinned = context.pin[name]["commit"]
        moved = "equals the pin" if row.get("main") == pinned else "has moved past the pin"
        notes.append(
            f"{row.get('command')}: exit {row.get('exit')}; main {row.get('main')} {moved}"
        )
    return report(context, "contract/identity", problems, notes)


def checkout_case(context):
    """The cached checkout is exactly the pinned commit, with the pinned floor and bounds."""
    problems = checkout_problems(context, "checkout") + source_problems(context)
    imago = context.pin["imago"]
    notes = [
        f"{native(context.checkout)} at {imago['commit']}: no tracked change, "
        "untracked or ignored file, or hidden index entry",
        f"go.mod: go {imago['go']}; {imago['bounds']['source']}: MaxRetryAttempts "
        f"{imago['bounds']['MaxRetryAttempts']}, MaxPackages {imago['bounds']['MaxPackages']}",
    ]
    return report(context, "contract/checkout", problems, notes if not problems else [])


def digest_problems(context):
    """Return the binary's sha256 and why it is not the one the fetch recorded, or []."""
    digest = sha256_of(read_bytes(context.binary))
    recorded = recorded_binary(load_json(context.identity))
    if digest == recorded:
        return digest, []
    return digest, [
        f"binary sha256 {digest}, the fetch recorded {recorded}; run `make contract-fetch`"
    ]


def provenance_case(context):
    """The binary is the one the fetch built: its recorded digest and the pinned build information.

    Recorded digest: a binary replaced after the fetch fails however it was
    made. Build information: the pinned revision, its commit time and module
    version, unmodified, CGO-free, with -trimpath.
    """
    committed = commit_time(context, "provenance-commit-time")
    info, result = buildinfo(context, "provenance", context.binary)
    problems = provenance_problems(info, context.pin, committed)
    if result["exit"] != 0:
        problems += tail(result["stderr"], 2)
    digest, mismatch = digest_problems(context)
    problems += mismatch
    version = invoke(
        context, "go-version", ["go", "version"], VERSION_TIMEOUT, env=go_environment(offline=True)
    )
    notes = [f"{version['stdout'].strip()} reads the build information"]
    if info is not None:
        build = info["build"]
        notes.append(
            f"{info['path']} {info.get('mod version')} built by {info['go']}: vcs.revision "
            f"{build.get('vcs.revision')}, vcs.modified {build.get('vcs.modified')}, "
            f"vcs.time {build.get('vcs.time')} (commit time {committed})"
        )
    notes.append(f"binary sha256 {digest}, {'as' if not mismatch else 'NOT as'} the fetch recorded")
    return report(context, "contract/binary-provenance", problems, notes)


def fixtures_case(context):
    """imago's vendored fixtures are byte-identical to this repository's payloads."""
    problems, notes = [], []
    for row in context.pin["imago"]["fixtures"][:MAX_FIXTURES]:
        ours = read_bytes(ROOT / row["aegis"])
        theirs = read_bytes(context.checkout / row["imago"])
        if ours != theirs:
            problems.append(f"imago {row['imago']} differs from {row['aegis']}")
        if sha256_of(theirs) != row["sha256"]:
            problems.append(f"imago {row['imago']} hashes {sha256_of(theirs)}")
        notes.append(f"{row['imago']} == {row['aegis']} ({len(theirs)} bytes)")
    return report(context, "contract/fixtures-identical", problems, notes)


def payload_path(context, label, document):
    """Write one payload fed to imago under the run directory and return its path."""
    path = context.run_dir / "payloads" / f"{label}.json"
    write_bytes(path, json_bytes(document))
    return path


def validate(context, label, kind, path, binary=None):
    """Run imago's validator for `kind` on `path` and return the retained result."""
    argv = [native(binary or context.binary)]
    if kind == "product-input":
        argv += ["aegis", "validate", native(path), "--json"]
    else:
        argv += ["kernel", "requirement", "validate", native(path)]
    return invoke(context, label, argv, VALIDATE_TIMEOUT, cwd=context.run_dir)


def product_accepted_case(context):
    """Positive: imago accepts build/product-input.json and echoes every field."""
    payload = load_json(PRODUCT_INPUT)
    result = validate(context, "product-input-accepted", "product-input", PRODUCT_INPUT)
    problems = accepted_problems(result, payload)
    notes = [
        f"imago aegis validate build/product-input.json --json: exit {result['exit']}",
        f"correlation_id {payload['correlation-id']}, revision {payload['revision']}, "
        f"{len(payload['packages'])} packages, retry {payload['retries']['max-attempts']} "
        f"x {go_duration(payload['retries']['backoff-seconds'])}: printed as sent",
    ]
    return report(context, "product-input/accepted", problems, notes), result


def with_field(document, path, value):
    """Return a copy of `document` with the dotted `path` set to `value`."""
    copy = json.loads(json.dumps(document))
    keys = path.split(".")
    target = copy
    for key in keys[:-1]:
        target = target[key]
    target[keys[-1]] = value
    return copy


def with_packages(payload, count):
    """Return `payload` with exactly `count` unique packages, its own listed first."""
    own = list(payload["packages"])
    extra = [f"aegis-bound-{index:03d}" for index in range(max(count - len(own), 0))]
    return with_field(payload, "packages", (own + extra)[:count])


def standin_sources(stdout):
    """Return {relative path: bytes} for a stand-in that prints `stdout` and exits 0.

    It claims imago's module path and main package, so only what the Go
    toolchain stamps from the commit it was built at -- the revision, the
    commit time and the module version -- tells it apart. The reply is embedded
    rather than written as a Go literal, so every byte the real run printed
    survives.
    """
    main = (
        'package main\n\nimport (\n\t_ "embed"\n\t"os"\n)\n\n//go:embed reply.txt\n'
        "var reply string\n\nfunc main() { os.Stdout.WriteString(reply) }\n"
    )
    return {
        "go.mod": f"module {IMAGO_MODULE}\n\ngo 1.21\n".encode("utf-8"),
        "cmd/imago/main.go": main.encode("utf-8"),
        "cmd/imago/reply.txt": stdout.encode("utf-8"),
    }


# The stand-in commits in a throwaway repository under the run directory. invoke()
# shuts the user's and the system's git configuration out, as for every command,
# so no signing key, template or hook of the workstation takes part; these name
# the commit's author, which that configuration would otherwise have supplied.
STANDIN_GIT = {
    "GIT_AUTHOR_NAME": "aegis-contract-standin",
    "GIT_AUTHOR_EMAIL": "standin@invalid",
    "GIT_COMMITTER_NAME": "aegis-contract-standin",
    "GIT_COMMITTER_EMAIL": "standin@invalid",
}


def build_standin(context, stdout):
    """Build the stand-in from its own throwaway git commit, offline; return its path."""
    source = context.run_dir / "standin"
    for relative, data in standin_sources(stdout).items():
        write_bytes(source / relative, data)
    steps = (
        ("standin-init", git(source, "init", "-q")),
        ("standin-add", git(source, "add", "-A")),
        ("standin-commit", git(source, "commit", "-q", "-m", "stand-in")),
    )
    for label, argv in steps:
        result = invoke(context, label, argv, GIT_TIMEOUT, env=STANDIN_GIT)
        if result["exit"] != 0:
            raise GateError(f"{label} exited {result['exit']}: {tail(result['stderr'], 2)}")
    binary = context.run_dir / "standin-bin" / f"imago{EXE}"
    argv = ["go", "build", "-trimpath", "-buildvcs=true", "-o", native(binary), "./cmd/imago"]
    env = go_environment(offline=True)
    built = invoke(context, "standin-build", argv, BUILD_TIMEOUT, env=env, cwd=source)
    if built["exit"] != 0:
        raise GateError(f"the stand-in did not build: {tail(built['stderr'], 3)}")
    return binary


# What the stand-in must be refused by: the two rows go derives from the commit
# a binary was built at, whatever the binary prints.
STANDIN_REFUSALS = ("vcs.revision is", "mod version is")


def simulated_case(context, real):
    """Negative: output identical to imago's is refused when the binary is not the pinned build.

    The stand-in replays what the pinned binary printed for the positive case,
    exits 0 and claims imago's module path; the gate refuses it by what the Go
    toolchain stamped from the stand-in's own commit -- its revision and module
    version -- and it does not hash to the digest the fetch recorded (E09-3).
    This proves a rebuilt stand-in is refused; a forged binary written over the
    cached one is the digest check's to catch, in `contract/binary-provenance`.
    """
    binary = build_standin(context, real["stdout"])
    replay = validate(context, "standin-product-input", "product-input", PRODUCT_INPUT, binary)
    info, _ = buildinfo(context, "standin-provenance", binary)
    committed = commit_time(context, "standin-commit-time")
    refusal = provenance_problems(info, context.pin, committed)
    problems = []
    if replay["exit"] != 0 or replay["stdout"] != real["stdout"]:
        problems.append("the stand-in did not reproduce the pinned binary's output")
    for needle in STANDIN_REFUSALS:
        if not any(line.startswith(needle) for line in refusal):
            problems.append(f"the stand-in was not refused on {needle.split()[0]}: {refusal}")
    if sha256_of(read_bytes(binary)) == recorded_binary(load_json(context.identity)):
        problems.append("the stand-in hashes to the digest the fetch recorded")
    notes = [
        f"stand-in {(info or {}).get('path')}: exit {replay['exit']}, stdout identical "
        f"to the pinned binary's ({len(replay['stdout'])} bytes), sha256 not the recorded one",
        f"refused: {'; '.join(refusal)}",
    ]
    return report(context, "contract/simulated-output-refused", problems, notes)


# The negative rows for the product input: the field changed, its new value and
# the correlated reason imago must print. Every other field stays valid.
PRODUCT_TAMPERS = (
    (
        "schema-v2",
        "schema",
        "aegis.p01.product-input.v2",
        'schema: want "aegis.p01.product-input.v1"',
    ),
    (
        "unknown-field",
        "image-digest",
        "sha256:" + "0" * 64,
        'input: strict decode: json: unknown field "image-digest"',
    ),
    (
        "kernel-not-listed",
        "kernel.default-package",
        "linux-zen",
        "kernel.default-package: must be one of packages",
    ),
    (
        "revision-uppercase",
        "revision",
        "0123456789ABCDEF" * 2 + "01234567",
        "revision: must be exactly 40 lowercase hex characters",
    ),
)


def row_problems(kind, result, document, reason):
    """Return why one row's run did not end the way the row expects, or [].

    `reason` None means the row must be accepted; otherwise it is the text
    after the correlation id that the refusal must carry.
    """
    if kind == "product-input":
        if reason is None:
            return accepted_problems(result, document)
        return refused_problems(result, PRODUCT_PREFIX, document["correlation-id"], reason)
    if reason is None:
        return kernel_accepted_problems(result, document)
    return refused_problems(result, KERNEL_PREFIX, document["correlation-id"], reason)


def rows_case(context, name, kind, rows, extra=((), ())):
    """Run each (label, document, reason) row through imago and report them as one case.

    `extra` carries problems and notes the caller established before the rows.
    """
    problems, notes = list(extra[0]), list(extra[1])
    prefix = name.split("/")[0]
    for label, document, reason in rows:
        path = payload_path(context, f"{prefix}-{label}", document)
        result = validate(context, f"{prefix}-{label}", kind, path)
        row = row_problems(kind, result, document, reason)
        problems += [f"{label}: {line}" for line in row]
        notes.append(f"{label}: exit {result['exit']}, {row_outcome(kind, document, reason, row)}")
    return report(context, name, problems, notes)


def row_outcome(kind, document, reason, row):
    """Return one row's outcome as the note prints it: the correlated error in full."""
    if row:
        return "NOT as the row expects"
    if reason is None:
        return f"accepted {document['correlation-id']}"
    prefix = PRODUCT_PREFIX if kind == "product-input" else KERNEL_PREFIX
    return f"refused: {prefix} {document['correlation-id']}: {reason}"


def product_tamper_rows(payload):
    """Return the negative rows: one changed field each, refused with the payload's id."""
    return [
        (label, with_field(payload, path, value), reason)
        for label, path, value, reason in PRODUCT_TAMPERS
    ]


def retry_rows(payload, bound):
    """Return the retry-budget boundary: exactly imago's bound accepted, one above refused."""
    field = "retries.max-attempts"
    return [
        (f"max-attempts-{bound}", with_field(payload, field, bound), None),
        (
            f"max-attempts-{bound + 1}",
            with_field(payload, field, bound + 1),
            f"{field}: {bound + 1} outside 1..{bound}",
        ),
    ]


def packages_rows(payload, bound):
    """Return the packages boundary: exactly imago's bound accepted, one above refused."""
    return [
        (f"packages-{bound}", with_packages(payload, bound), None),
        (
            f"packages-{bound + 1}",
            with_packages(payload, bound + 1),
            f"packages: count {bound + 1} outside 1..{bound}",
        ),
    ]


def aegis_maxima(payload, bounds):
    """Return `payload` at every upper bound the Aegis schema itself allows."""
    at_bound = with_packages(payload, bounds["MAX_PACKAGES"])
    at_bound = with_field(at_bound, "retries.max-attempts", bounds["MAX_RETRY_ATTEMPTS"])
    return with_field(at_bound, "retries.backoff-seconds", bounds["MAX_BACKOFF_SECONDS"])


def bounds_problems(aegis, imago):
    """Return where an Aegis upper bound exceeds the one imago enforces, or []."""
    pairs = (
        ("MAX_PACKAGES", "MaxPackages"),
        ("MAX_RETRY_ATTEMPTS", "MaxRetryAttempts"),
    )
    return [
        f"Aegis {ours} is {aegis[ours]}, above imago's {theirs} {imago[theirs]}"
        for ours, theirs in pairs
        if aegis[ours] > imago[theirs]
    ]


def aegis_bounds_case(context, payload):
    """A payload at the Aegis schema's own upper bounds lies inside imago's and is accepted."""
    aegis = rust_bounds(read_bytes(AEGIS_MANIFEST_SOURCE).decode("utf-8").replace("\r\n", "\n"))
    imago = context.pin["imago"]["bounds"]
    note = (
        f"Aegis MAX_PACKAGES {aegis['MAX_PACKAGES']}, MAX_RETRY_ATTEMPTS "
        f"{aegis['MAX_RETRY_ATTEMPTS']}, MAX_BACKOFF_SECONDS {aegis['MAX_BACKOFF_SECONDS']}; "
        f"imago MaxPackages {imago['MaxPackages']}, MaxRetryAttempts {imago['MaxRetryAttempts']}"
    )
    rows = [("aegis-maxima", aegis_maxima(payload, aegis), None)]
    extra = (bounds_problems(aegis, imago), [note])
    return rows_case(
        context, "product-input/aegis-bounds-inside-imago", "product-input", rows, extra
    )


def product_cases(context):
    """The negative and boundary rows for the product input (E09-1)."""
    payload = load_json(PRODUCT_INPUT)
    bounds = context.pin["imago"]["bounds"]
    kind = "product-input"
    return [
        rows_case(context, "product-input/tampered-refused", kind, product_tamper_rows(payload)),
        rows_case(
            context,
            "product-input/retry-bound",
            kind,
            retry_rows(payload, bounds["MaxRetryAttempts"]),
        ),
        rows_case(
            context,
            "product-input/packages-bound",
            kind,
            packages_rows(payload, bounds["MaxPackages"]),
        ),
        aegis_bounds_case(context, payload),
    ]


def with_feature(requirement, index, **changes):
    """Return `requirement` with feature `index` changed by `changes`."""
    features = [dict(feature) for feature in requirement["features"]]
    features[index].update(changes)
    return with_field(requirement, "features", features)


def kernel_invalid_rows(requirement):
    """Return features imago cannot accept, each to be refused with the payload's id."""
    first = requirement["features"][0]["symbol"]
    return [
        (
            "state-yes",
            with_feature(requirement, 0, state="yes"),
            "features[0].state: must be built-in or module",
        ),
        (
            "probe-dmesg",
            with_feature(requirement, 0, probe="dmesg"),
            'features[0].probe: unknown probe "dmesg"',
        ),
        (
            "duplicate-symbol",
            with_feature(requirement, 1, symbol=first),
            f"features[1].symbol: duplicate symbol {first}",
        ),
    ]


def empty_rows(requirement):
    """Return the boundary: no feature is refused explicitly, one feature is accepted."""
    return [
        (
            "features-0",
            with_field(requirement, "features", []),
            "features: feature list is empty",
        ),
        ("features-1", with_field(requirement, "features", requirement["features"][:1]), None),
    ]


def kernel_cases(context):
    """E09-2 as D92 re-scopes it: imago's pkg/kernel consumes the kernel requirement."""
    requirement = load_json(KERNEL_REQUIREMENT)
    accepted = [
        ("requirement", requirement, None),
        ("reference", load_json(KERNEL_REFERENCE), None),
    ]
    kind = "kernel-requirement"
    return [
        rows_case(context, "kernel-requirement/accepted", kind, accepted),
        rows_case(
            context,
            "kernel-requirement/invalid-feature-refused",
            kind,
            kernel_invalid_rows(requirement),
        ),
        rows_case(
            context, "kernel-requirement/empty-features-refused", kind, empty_rows(requirement)
        ),
    ]


def run_cases(context):
    """Run every case in order and return the number that failed.

    Nothing a binary prints counts until its provenance is the pinned build,
    so the payload cases run only after the four identity cases pass.
    """
    outcomes = [pin_case(context), identity_case(context), checkout_case(context)]
    outcomes.append(provenance_case(context))
    if any(outcomes):
        failed = [row["case"] for row in context.outcomes if row["problems"]]
        print(f"     the payload cases did not run, because {', '.join(failed)} failed")
        return sum(1 for problems in outcomes if problems)
    outcomes.append(fixtures_case(context))
    accepted, real = product_accepted_case(context)
    outcomes.append(accepted)
    outcomes.append(simulated_case(context, real))
    outcomes += product_cases(context)
    outcomes += kernel_cases(context)
    return sum(1 for problems in outcomes if problems)


def tool_reasons():
    """Return why go or git cannot be used here, one reason each."""
    return [f"{tool} is not on PATH" for tool in ("go", "git") if shutil.which(tool) is None]


def cache_reasons(context):
    """Return why the cache cannot serve a run: each piece of it that is absent.

    Only absence is a skip (D93). A cache that is present but wrong -- fetched
    for another pin, a checkout or a binary that is not the pinned build -- is
    a failure, found by the identity, checkout and provenance cases, each of
    which names `make contract-fetch`.
    """
    pieces = (
        ("imago checkout", context.checkout / ".git"),
        ("imago binary", context.binary),
        ("identity record", context.identity),
    )
    where = native(context.store)
    return [
        f"no {label} under {where}; run `make contract-fetch`"
        for label, path in pieces
        if not path.exists()
    ]


def write_summary(context, failed):
    """Keep the run's verdict, the pinned revisions and every case beside its logs."""
    summary = {
        "run": context.run_dir.name,
        "gate": "M09 contract pair (D92, D93)",
        "imago": context.pin["imago"]["commit"],
        "nucleus": context.pin["nucleus"]["commit"],
        "failed": failed,
        "cases": context.outcomes,
    }
    write_bytes(context.run_dir / "summary.json", json_bytes(summary))


def gate_main(context):
    """Run the gate: SKIP with a reason, FAIL with a reason, or PASS with every case above it."""
    reasons = tool_reasons() or cache_reasons(context)
    if reasons:
        for reason in reasons[:MAX_PROBLEM_LINES]:
            print(f"SKIP: {reason}; the contract pair gate did not run.")
        return 0
    context.run_dir.mkdir(parents=True, exist_ok=True)
    print(f"     run id {context.run_dir.name}; retained in {native(context.run_dir)}")
    try:
        failed = run_cases(context)
    finally:
        shutil.rmtree(context.run_dir / "standin-bin", ignore_errors=True)
    write_summary(context, failed)
    if failed:
        print(f"FAIL: {failed} contract pair case(s) did not match their recorded outcome.")
        return 1
    print(
        f"PASS: contract pair gate (M09, D92) on run {context.run_dir.name}: imago "
        f"{context.pin['imago']['commit'][:12]} consumed both Aegis payloads and refused every "
        "tampered, out-of-bound and empty one with the payload's correlation id, and a stand-in "
        "without the pinned provenance was refused. Consumption evidence only: no product "
        "result, image or kernel came back (M11, M10)."
    )
    return 0


def fetch_toolchain(context):
    """Return why the fetch cannot build imago here: go or git absent, or go below the floor."""
    missing = tool_reasons()
    if missing:
        return report(context, "contract-fetch/toolchain", missing)
    env = go_environment(offline=True)
    go = invoke(context, "go-version", ["go", "version"], VERSION_TIMEOUT, env=env)
    version = invoke(context, "git-version", ["git", "version"], VERSION_TIMEOUT)
    words = go["stdout"].split()
    release = go_release(words[2]) if len(words) > 2 else None
    floor = context.pin["imago"]["go"]
    problems = []
    if release is None or release < floor_tuple(floor):
        problems.append(
            f"{go['stdout'].strip()!r} is below go {floor}, which imago's go.mod declares; "
            "GOTOOLCHAIN=local does not download another toolchain"
        )
    notes = [go["stdout"].strip(), version["stdout"].strip()]
    return report(context, "contract-fetch/toolchain", problems, notes)


def identity_row(context, name, argv, result):
    """Return the retained record of one producer's `git ls-remote --heads`."""
    return {
        "repository": context.pin[name]["repository"],
        "command": " ".join(argv),
        "exit": result["exit"],
        "main": ls_remote_main(result["stdout"]),
        "pinned": context.pin[name]["commit"],
        "output": result["stdout"],
    }


def fetch_identity(context):
    """Run `git ls-remote --heads` against both producers; return (record, problems)."""
    producers, problems, notes = {}, [], []
    for name in PRODUCERS:
        argv = ["git", "ls-remote", "--heads", context.pin[name]["repository"]]
        result = invoke(context, f"ls-remote-{name}", argv, NETWORK_TIMEOUT)
        row = identity_row(context, name, argv, result)
        producers[name] = row
        if result["exit"] != 0 or row["main"] is None:
            problems += [f"{name}: exit {result['exit']}, no refs/heads/main"]
            problems += tail(result["stderr"], 2)
        moved = "equals the pin" if row["main"] == row["pinned"] else "has moved past the pin"
        notes.append(f"{row['command']}: exit {row['exit']}; main {row['main']} {moved}")
    record = {
        "schema": "aegis.m09.contract-identity.v1",
        "run": context.run_dir.name,
        "fetched": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "producers": producers,
    }
    return record, report(context, "contract-fetch/identity", problems, notes)


def fetch_checkout(context):
    """Clone imago at the pinned commit into a fresh directory, never reusing the cached one.

    A reused checkout could carry a file git ignores, an index flag or a
    repository setting that no status check sees and go would build from.
    """
    imago = context.pin["imago"]
    shutil.rmtree(context.checkout, ignore_errors=True)
    if context.checkout.exists():
        problems = [f"the cached checkout {native(context.checkout)} could not be removed"]
        return report(context, "contract-fetch/checkout", problems)
    fetch = ["fetch", "-q", "--depth", "1", "--no-tags", imago["repository"], imago["commit"]]
    steps = (
        ("clone-init", ["git", "init", "-q", native(context.checkout)], GIT_TIMEOUT),
        ("clone-fetch", git(context.checkout, *fetch), NETWORK_TIMEOUT),
        (
            "clone-checkout",
            git(context.checkout, "checkout", "-q", "--detach", imago["commit"]),
            GIT_TIMEOUT,
        ),
    )
    for label, argv, timeout in steps:
        result = invoke(context, label, argv, timeout)
        if result["exit"] != 0:
            problems = [f"{label} exited {result['exit']}"] + tail(result["stderr"], 3)
            return report(context, "contract-fetch/checkout", problems)
    problems = checkout_problems(context, "fetched")
    notes = [f"{imago['repository']} at {imago['commit']}, depth 1, into a fresh directory"]
    return report(context, "contract-fetch/checkout", problems, notes)


def fetch_build(context):
    """Download and verify imago's modules, build it with its provenance, and keep it."""
    env = go_environment(offline=False)
    partial = context.binary.with_name(f"partial-{context.binary.name}")
    partial.parent.mkdir(parents=True, exist_ok=True)
    build = [
        "go",
        "build",
        "-trimpath",
        "-buildvcs=true",
        "-o",
        native(partial),
        "./cmd/imago",
    ]
    steps = (
        ("mod-download", ["go", "mod", "download"], DOWNLOAD_TIMEOUT),
        ("mod-verify", ["go", "mod", "verify"], DOWNLOAD_TIMEOUT),
        ("build", build, BUILD_TIMEOUT),
    )
    for label, argv, timeout in steps:
        result = invoke(context, label, argv, timeout, env=env, cwd=context.checkout)
        if result["exit"] != 0:
            partial.unlink(missing_ok=True)
            problems = [f"go {' '.join(argv[1:3])} exited {result['exit']}"]
            return report(context, "contract-fetch/build", problems + tail(result["stderr"], 3))
    info, _ = buildinfo(context, "build-provenance", partial)
    committed = commit_time(context, "build-commit-time")
    problems = provenance_problems(info, context.pin, committed)
    if problems:
        partial.unlink(missing_ok=True)
    else:
        partial.replace(context.binary)
    info = info or {"build": {}}
    notes = [
        "GOTOOLCHAIN=local CGO_ENABLED=0 GOFLAGS=-mod=readonly go build -trimpath -buildvcs=true",
        f"{native(context.binary)}: vcs.revision {info['build'].get('vcs.revision')}, "
        f"{info.get('mod version')}, built by {info.get('go')}",
    ]
    return report(context, "contract-fetch/build", problems, notes)


def fetch_main(context):
    """`make contract-fetch`: identity for both producers, then clone and build imago.

    The identity record is written last, with the built binary's sha256, so it
    always names a complete fetch and the binary that fetch produced.
    """
    context.run_dir.mkdir(parents=True, exist_ok=True)
    print(f"     run id {context.run_dir.name}; retained in {native(context.run_dir)}")
    problems = fetch_toolchain(context)
    record = None
    if not problems:
        record, problems = fetch_identity(context)
    if not problems:
        problems = fetch_checkout(context) or fetch_build(context)
    write_summary(context, 1 if problems else 0)
    if problems:
        print("FAIL: the contract pair's pinned producer could not be fetched and built.")
        return 1
    digest = sha256_of(read_bytes(context.binary))
    record["binary"] = {"file": context.binary.name, "sha256": digest}
    print(f"     binary sha256 {digest}, recorded in {native(context.identity)}")
    write_bytes(context.identity, json_bytes(record))
    print(
        "PASS: identity retained for both producers and imago "
        f"{context.pin['imago']['commit'][:12]} built with its provenance; "
        "`make verify-all` now runs the contract gate."
    )
    return 0


def recorded_fetch(store):
    """Return the run name the identity record was written by, or None."""
    try:
        record = json.loads((store / "identity.json").read_bytes().decode("utf-8"))
    except (OSError, ValueError):
        return None
    return str(record.get("run")) if isinstance(record, dict) else None


def prune_runs(store):
    """Keep the newest MAX_RETAINED_RUNS gate runs and fetch runs, and the recorded fetch."""
    runs = store / "runs"
    if not runs.is_dir():
        return
    keep = {recorded_fetch(store)}
    names = sorted(path for path in runs.iterdir() if path.is_dir())[:MAX_PRUNED]
    for prefix in ("f", "r"):
        group = [path for path in names if path.name.startswith(prefix)]
        for old in group[:-MAX_RETAINED_RUNS]:
            if old.name not in keep:
                shutil.rmtree(old, ignore_errors=True)


def terminated(signum, _frame):
    """Turn a termination signal into SystemExit, so a running child is ended on the way out."""
    raise SystemExit(128 + signum)


def install_signal_handlers():
    """Route SIGTERM, SIGHUP and SIGQUIT through `terminated`; each is looked up (HISS-21)."""
    handled = []
    for name in TERMINATING_SIGNALS:
        number = getattr(signal, name, None)
        if number is not None:
            signal.signal(number, terminated)
            handled.append(name)
    return handled


def parse_arguments(argv):
    """Return the parsed options."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument(
        "--fetch",
        action="store_true",
        help="run git ls-remote on both producers, clone imago at the pin and build it",
    )
    return parser.parse_args(argv)


def run_name(fetch):
    """Return a new run directory name: f... for a fetch, r... for a gate run."""
    return f"{'f' if fetch else 'r'}{time.strftime('%Y%m%dT%H%M%S')}-{secrets.token_hex(2)}"


def main(argv=None):
    """Load the pin, then fetch or run the gate."""
    options = parse_arguments(argv)
    install_signal_handlers()
    print("Contract pair gate (M09, D92, D93; consumption evidence only).")
    try:
        pin = load_pin()
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    store = cache_dir()
    context = Context(pin, store, store / "runs" / run_name(options.fetch))
    try:
        return fetch_main(context) if options.fetch else gate_main(context)
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    finally:
        prune_runs(store)


if __name__ == "__main__":
    sys.exit(main())
