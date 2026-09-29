#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Run the M04 accessibility suite in the pinned Playwright container, offline.

Milestone M04, and the D100 lint milestone M16 added. The P12 Concordia token
file and its one Svelte 5 component (``ui/concordia-tokens``) are installed
from their lockfile, linted for HISS with ESLint, built and scanned inside the
official Playwright image, named by digest in
``ui/concordia-tokens/toolchain.pin.json``, with networking disabled and the
repository bind-mounted read-only (REQ-P12-04). Node and pnpm are the pinned
releases, not the image's own Node: the Node tarball is pinned by sha256 and the
pnpm binary by the integrity pnpm-lock.yaml records for it.

The gate never pulls and never downloads. ``--fetch`` (``make a11y-fetch``) pulls
the image by digest, downloads the two pinned archives and fills an offline pnpm
store; it is the only networked step. Without an engine, the image or the cache,
or with an offline store filled for another pnpm-lock.yaml, the gate prints
why it did not run and exits 0 (HISS-21).

Cases, with no simulation and no failure suppression:

* ``a11y/pin`` checks the pin, package.json and pnpm-lock.yaml against each
  other: exact versions, the Playwright version in the image tag, the pnpm
  version in three places;
* ``a11y/truncated-digest-refused`` feeds the pin a 12-digit and a 63-digit
  digest and the six-digit one REQ-P12-04's source gives, requires all three
  to be refused, and the full digest to be accepted;
* ``a11y/image-present`` finds the image locally by its pinned digest;
* ``a11y/lockfile-installs`` installs offline with ``--frozen-lockfile``;
* ``a11y/hiss-lint`` runs ESLint over the package with zero warnings allowed
  and requires every JavaScript and Svelte source to be linted clean (D100:
  HISS-01, HISS-04 and HISS-08 until cordanaLLM/praetor#589 ships a scanner);
  ``a11y/hiss-lint-<family>-refused`` lints one planted violation per rule
  family and requires exactly its findings, and ``a11y/hiss-lint-at-the-limits``
  requires a file sitting on every limit, a 60-line function among them, to
  lint clean;
* ``a11y/lockfile-mismatch-refused`` installs a copy whose package.json no
  longer matches the lockfile and requires pnpm to refuse it;
* ``a11y/image-node-refused`` installs on the image's own Node and requires
  the exact engines pin to refuse it;
* ``a11y/toolchain-readback`` reads every admitted version back from inside the
  container, the lint's included, and launches the pinned Chromium;
* ``a11y/browser-revision-absent-refused`` points Playwright at a browser
  directory without the pinned revision and requires the launch to fail;
* ``a11y/build`` builds the static page;
* ``a11y/suite`` runs the Playwright and axe-core suite and prints its report:
  the D81 tag set, the executed rules, the EN 301 549 V4.1.1 and V3.2.1 clause
  per criterion, and the measured focus, boundary and text-scaling checks. A
  test counts only when it was meant to pass and passed, so an expected
  failure, a skip or a flaky retry fails the case, and the gate checks each
  measured report's values itself;
* ``a11y/planted-violation-fails``, ``a11y/stripped-outline-fails`` and
  ``a11y/d76-stub-theme-fails`` plant one defect each and require the suite
  to fail on it.

Nothing is written into the repository. The cache and the retained logs live
under ``AEGIS_A11Y_DIR`` (default ``${XDG_CACHE_HOME:-$HOME/.cache}/aegis-a11y``).
A pass is development evidence for the P12 token component and the lint; it
closes no accessibility gate for the shell or the image.
"""

import argparse
import base64
import hashlib
import json
import os
import re
import secrets
import shutil
import signal
import subprocess
import sys
import tarfile
import time
from pathlib import Path

from host import end_session, gid, uid

ROOT = Path(__file__).resolve().parent.parent
PACKAGE = ROOT / "ui" / "concordia-tokens"
PIN = PACKAGE / "toolchain.pin.json"
MANIFEST = PACKAGE / "package.json"
LOCKFILE = PACKAGE / "pnpm-lock.yaml"
CLAUSE_MAP = PACKAGE / "tests" / "en301549-clauses.json"
# The criteria the measured checks bear on, which no axe-core rule evaluates:
# the focus indicator's presence, width and contrast, and text resized to 200%.
MEASURED_CRITERIA = (("focus indicator", ("2.4.7", "1.4.11")), ("200% text", ("1.4.4",)))
PIN_SCHEMA = "aegis.m04.a11y-toolchain-pin.v1"
PNPM_PACKAGE = "@pnpm/exe.linux-x64"
ENGINES = ("podman", "docker")
# The record `make a11y-fetch` leaves beside the offline store: the sha256 of
# the pnpm-lock.yaml the store was filled from.
STORE_MARKER = "pnpm-lock.sha256"
# The reference REQ-P12-04's source gives for its validation container
# (export-020, "[Report P12] ...", section 7.3): an image M04 does not adopt,
# named by six hex digits and an ellipsis. E04-2 records it as non-pinnable.
SOURCE_REFERENCE = "ghcr.io/lusoris/concordia-validate@sha256:8f47c3..."
ENGINE_VARIABLE = "AEGIS_A11Y_ENGINE"
CACHE_VARIABLE = "AEGIS_A11Y_DIR"

# Paths inside the container. The container is Linux whatever host builds the
# command line, so these are POSIX literals (HISS-21).
IN_REPO = "/repo"
IN_PACKAGE = "/repo/ui/concordia-tokens"
IN_WORK = "/work"
IN_PKG = "/work/pkg"
IN_NODE = "/opt/node"
IN_PNPM = "/opt/pnpm"
IN_OFFLINE = "/offline"
IN_STORE = "/offline/store"
IN_METADATA = "/offline/cache"
IMAGE_PATH = "/usr/local/sbin:/usr/local/bin:/usr/sbin:/usr/bin:/sbin:/bin"
PINNED_PATH = f"{IN_NODE}/bin:{IN_PNPM}:{IMAGE_PATH}"
IMAGE_NODE_PATH = f"{IN_PNPM}:{IMAGE_PATH}"
# What the gate copies out of the read-only repository mount. Anything else in
# the package directory -- node_modules, dist, test output -- stays behind.
PACKAGE_ENTRIES = (
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    "concordia-tokens.css",
    "index.html",
    "vite.config.js",
    "playwright.config.js",
    "eslint.config.js",
    "hiss-lint",
    "src",
    "tests",
)
LOCK_ENTRIES = ("package.json", "pnpm-lock.yaml", "pnpm-workspace.yaml")

# Deadlines, in seconds. Every command the gate starts carries one.
VERSION_TIMEOUT = 30
INSPECT_TIMEOUT = 60
REMOVE_TIMEOUT = 60
PULL_TIMEOUT = 1800
DOWNLOAD_TIMEOUT = 600
STEP_TIMEOUT = 300
SUITE_TIMEOUT = 600
STOP_TIMEOUT = 15
TERMINATING_SIGNALS = ("SIGTERM", "SIGHUP", "SIGQUIT")

# Scalar bounds (HISS-02).
MAX_OUTPUT_CHARS = 1 << 20
MAX_PROBLEM_LINES = 24
MAX_DIGEST_CHUNKS = 4096
DIGEST_CHUNK = 1 << 20
MAX_ARCHIVE_MEMBERS = 20000
MAX_SPECS = 256
MAX_RETAINED_RUNS = 8
MAX_PRUNED = 4096
MAX_LINTED_FILES = 4096
EXPECTED_TESTS = 11

D81_TAGS = ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"]
DIGEST = re.compile(r"sha256:[0-9a-f]{64}")
VERSION = re.compile(r"\d+\.\d+\.\d+")
IMAGE_REFERENCE = re.compile(
    r"(?P<repository>[a-z0-9][a-z0-9./-]*/playwright):"
    r"(?P<tag>v(?P<version>\d+\.\d+\.\d+)-[a-z]+)@(?P<digest>\S+)"
)
LOCK_PNPM = re.compile(
    r"^    packageManagerDependencies:\n      pnpm:\n"
    r"        specifier: (?P<specifier>\S+)\n        version: (?P<version>\S+)$",
    re.M,
)
SUPPRESSION = ("||", "|", ";", "&")

# The three planted defects: plant name, the test it must fail, and a string
# the failure has to carry, so the right test failed for the right reason.
PLANTS = (
    ("a11y/planted-violation-fails", "missing-alt", "default state", "image-alt"),
    (
        "a11y/stripped-outline-fails",
        "strip-outline",
        "focus indicator: every tab stop",
        "indicator 0px is below the D16 floor",
    ),
    (
        "a11y/d76-stub-theme-fails",
        "stub-theme",
        "focus indicator: every tab stop",
        "indicator 1px is below the D16 floor",
    ),
)

# D100: the HISS lint's planted violations under hiss-lint/plants/, one per
# rule family, each with the exact findings it must produce, sorted. A finding
# ESLint reports without a rule -- an inline directive the configuration
# ignores -- reads as DIRECTIVE.
DIRECTIVE = "<ignored inline directive>"
LINT_PLANTS = (
    ("function-length", "function-length.js", ["max-lines-per-function"]),
    ("complexity", "complexity.js", ["complexity"]),
    ("statements", "statements.js", ["max-statements"]),
    ("dynamic-execution", "dynamic-execution.js", ["no-eval", "no-implied-eval", "no-new-func"]),
    ("self-recursion", "self-recursion.js", ["aegis-hiss/no-self-recursion"] * 4),
    ("svelte-component", "component.svelte", ["aegis-hiss/no-self-recursion", "no-eval"]),
    ("inline-disable", "inline-disable.js", [DIRECTIVE, "no-eval"]),
)
# The boundary: a file whose functions sit exactly on each limit lints clean.
LINT_BOUNDARY = "at-the-limits.js"
LINT_PLANT_DIR = "hiss-lint/plants"
# The lint's packages, as tests/readback.mjs reports them.
LINT_READBACK = (
    ("eslint", "eslint"),
    ("eslintPluginSvelte", "eslint-plugin-svelte"),
    ("svelteEslintParser", "svelte-eslint-parser"),
)


class GateError(Exception):
    """The gate could not be run at all, as opposed to a case that failed."""


def read_text(path):
    """Return a tracked file's text with LF line ends, whatever the checkout wrote."""
    try:
        return path.read_bytes().decode("utf-8").replace("\r\n", "\n")
    except (OSError, UnicodeDecodeError) as error:
        raise GateError(f"{path.name} could not be read: {error}") from error


def load_json(path):
    """Return the parsed JSON document at `path`."""
    try:
        return json.loads(read_text(path))
    except ValueError as error:
        raise GateError(f"{path.name} is not JSON: {error}") from error


def digest_problem(text):
    """Return why `text` cannot pin an image, or None when it can.

    A digest is the whole sha256: 'sha256:' and 64 lowercase hex digits. A
    truncated digest, the form REQ-P12-04's source names, matches more than one
    manifest in principle and names no single image, so it is refused (E04-2).
    """
    if DIGEST.fullmatch(text):
        return None
    return (
        f"image digest {text!r} is not 'sha256:' and 64 hex digits; a truncated "
        "digest names no single image and cannot pin one"
    )


def image_problems(image):
    """Return the problems in the pin's image section."""
    match = IMAGE_REFERENCE.fullmatch(str(image.get("reference", "")))
    if match is None:
        return [
            f"image reference {image.get('reference')!r} is not <repo>/playwright:v<x.y.z>-<os>@"
        ]
    problems = [digest_problem(match.group("digest"))]
    for platform, digest in dict(image.get("platform-digests", {})).items():
        problems.append(None if DIGEST.fullmatch(digest) else f"{platform} digest {digest!r}")
    browser = image.get("browser", {})
    if not str(browser.get("revision", "")).isdigit():
        problems.append(f"browser revision {browser.get('revision')!r} is not a number")
    if not re.fullmatch(r"\d+(?:\.\d+){3}", str(browser.get("version", ""))):
        problems.append(f"browser version {browser.get('version')!r} is not a.b.c.d")
    return [problem for problem in problems if problem]


def node_problems(node):
    """Return the problems in the pin's node section: an exact version and its sha256."""
    problems = []
    if not VERSION.fullmatch(str(node.get("version", ""))):
        problems.append(f"node version {node.get('version')!r} is not x.y.z")
    if not re.fullmatch(r"[0-9a-f]{64}", str(node.get("sha256", ""))):
        problems.append("node sha256 is not 64 hex digits")
    return problems


def pin_problems(pin):
    """Return every problem in a parsed pin; an empty list admits it."""
    if not isinstance(pin, dict) or pin.get("schema") != PIN_SCHEMA:
        return [f"the pin does not declare schema {PIN_SCHEMA}"]
    missing = [key for key in ("image", "node") if not isinstance(pin.get(key), dict)]
    if missing:
        return [f"the pin has no {key} section" for key in missing]
    return image_problems(pin["image"]) + node_problems(pin["node"])


def pnpm_version(manifest):
    """Return the pnpm version package.json's packageManager field pins, or None."""
    match = re.fullmatch(r"pnpm@(\d+\.\d+\.\d+)", str(manifest.get("packageManager", "")))
    return match.group(1) if match else None


def lock_integrity(lock, version):
    """Return the integrity pnpm-lock.yaml records for the pinned pnpm binary."""
    pattern = re.compile(
        rf"^  '{re.escape(PNPM_PACKAGE)}@{re.escape(version)}':\n"
        r"    resolution: \{integrity: (?P<integrity>sha512-[A-Za-z0-9+/]+={0,2})\}$",
        re.M,
    )
    match = pattern.search(lock)
    if match is None:
        raise GateError(f"pnpm-lock.yaml records no integrity for {PNPM_PACKAGE}@{version}")
    return match.group("integrity")


def script_problems(scripts):
    """Return the package.json scripts that could lose an exit status (REQ-CI-02)."""
    problems = []
    for name, command in dict(scripts).items():
        if any(token in command for token in SUPPRESSION) or command.startswith("-"):
            problems.append(f"script {name!r} can suppress a failure: {command!r}")
    return problems


def manifest_problems(pin, manifest, lock):
    """Return where package.json and the lockfile disagree with the pin or each other.

    pnpm is pinned once, by packageManager; engines.pnpm and the lockfile's
    packageManagerDependencies must name the same version.
    """
    version = IMAGE_REFERENCE.fullmatch(pin["image"]["reference"]).group("version")
    pnpm, node = pnpm_version(manifest), pin["node"]["version"]
    engines = manifest.get("engines", {})
    problems = script_problems(manifest.get("scripts", {}))
    if pnpm is None:
        problems.append(f"packageManager {manifest.get('packageManager')!r} is not pnpm@x.y.z")
    expected = {
        "engines.node": (engines.get("node"), node),
        "engines.pnpm": (engines.get("pnpm"), pnpm),
        "@playwright/test": (manifest.get("devDependencies", {}).get("@playwright/test"), version),
    }
    for field, (found, wanted) in expected.items():
        if found != wanted:
            problems.append(f"package.json {field} is {found!r}, the pin needs {wanted!r}")
    for name, spec in manifest.get("devDependencies", {}).items():
        if not VERSION.fullmatch(str(spec)):
            problems.append(f"devDependency {name} is {spec!r}, not an exact version")
    locked = LOCK_PNPM.search(lock)
    if locked is None or locked.group("version") != pnpm:
        problems.append(f"pnpm-lock.yaml does not record packageManager pnpm {pnpm}")
    return problems


def derive_archives(pin, pnpm, lock):
    """Add the two archives' file names, URLs and digests the gate fetches and checks.

    Each is derived from its one pin rather than written down twice: the Node
    release from the pin's version and sha256, the pnpm binary from
    packageManager and the integrity pnpm-lock.yaml records for it.
    """
    node = pin["node"]
    node["file"] = f"node-v{node['version']}-linux-x64.tar.xz"
    node["url"] = f"https://nodejs.org/dist/v{node['version']}/{node['file']}"
    file = f"exe.linux-x64-{pnpm}.tgz"
    pin["pnpm"] = {
        "version": pnpm,
        "package": PNPM_PACKAGE,
        "file": file,
        "url": f"https://registry.npmjs.org/{PNPM_PACKAGE}/-/{file}",
        "integrity": lock_integrity(lock, pnpm),
    }


def load_pin(path=PIN, manifest_path=MANIFEST, lock_path=LOCKFILE):
    """Return the admitted pin, with the lockfile's pnpm integrity, or raise GateError."""
    pin = load_json(path)
    problems = pin_problems(pin)
    if not problems:
        lock = read_text(lock_path)
        problems = manifest_problems(pin, load_json(manifest_path), lock)
    if problems:
        raise GateError("; ".join(problems[:MAX_PROBLEM_LINES]))
    derive_archives(pin, pnpm_version(load_json(manifest_path)), lock)
    pin["lock-sha256"] = hashlib.sha256(lock.encode("utf-8")).hexdigest()
    match = IMAGE_REFERENCE.fullmatch(pin["image"]["reference"])
    pin["image"]["repository"] = match.group("repository")
    pin["image"]["digest"] = match.group("digest")
    pin["image"]["playwright"] = match.group("version")
    pin["image"]["by-digest"] = f"{match.group('repository')}@{match.group('digest')}"
    return pin


def c_locale():
    """Return this environment with a fixed locale, so tool output parses the same everywhere."""
    env = dict(os.environ)
    env.update({"LC_ALL": "C", "LANG": "C", "NO_COLOR": "1"})
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


def run(argv, timeout):
    """Run `argv` in its own session under a hard deadline; return (code, stdout, stderr)."""
    try:
        process = subprocess.Popen(
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=c_locale(),
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


def pick_engine():
    """Return (engine, None), or (None, reason) when no engine is usable here.

    Rootless podman is preferred: it needs no daemon and no group membership,
    and what the container writes into the scratch mount belongs to the user
    who ran the gate. docker is the fallback and then runs as that user's uid.
    """
    forced = os.environ.get(ENGINE_VARIABLE, "")
    if forced and forced not in ENGINES:
        return None, f"{ENGINE_VARIABLE}={forced!r} names neither podman nor docker"
    for engine in (forced,) if forced else ENGINES:
        if shutil.which(engine) is not None:
            return engine, None
    return None, "neither podman nor docker is on PATH"


def image_present(engine, image):
    """Return (True, digests) when the image is stored locally under its pinned digest."""
    code, stdout, _ = run(
        [engine, "image", "inspect", "--format", "{{json .RepoDigests}}", image["by-digest"]],
        INSPECT_TIMEOUT,
    )
    if code != 0:
        return False, []
    try:
        digests = json.loads(stdout.strip() or "[]")
    except ValueError:
        return False, []
    return image["by-digest"] in digests, digests


def cache_dir():
    """Return the directory the fetched image inputs and the run logs live in."""
    override = os.environ.get(CACHE_VARIABLE)
    if override:
        return Path(override)
    base = os.environ.get("XDG_CACHE_HOME") or str(Path.home() / ".cache")
    return Path(base) / "aegis-a11y"


def file_digests(path):
    """Return (sha256 hex, sha512 SRI) of one file, read in bounded chunks."""
    sha256, sha512 = hashlib.sha256(), hashlib.sha512()
    with open(path, "rb") as handle:
        for _ in range(MAX_DIGEST_CHUNKS):
            chunk = handle.read(DIGEST_CHUNK)
            if not chunk:
                break
            sha256.update(chunk)
            sha512.update(chunk)
        else:
            raise GateError(f"{path.name} is larger than the digest bound")
    return sha256.hexdigest(), "sha512-" + base64.b64encode(sha512.digest()).decode("ascii")


def archive_state(pin, store):
    """Return (reasons to skip, problems): an absent archive skips, a wrong one fails."""
    reasons, problems = [], []
    rows = (
        (store / "downloads" / pin["node"]["file"], 0, pin["node"]["sha256"]),
        (store / "downloads" / pin["pnpm"]["file"], 1, pin["pnpm"]["integrity"]),
    )
    for path, index, wanted in rows:
        if not path.is_file():
            reasons.append(f"{path.name} is not in {path.parent}; run `make a11y-fetch`")
            continue
        found = file_digests(path)[index]
        if found != wanted:
            problems.append(f"{path.name} hashes {found}, the pin records {wanted}")
    reason = store_reason(store, pin["lock-sha256"])
    return reasons + ([reason] if reason else []), problems


def store_reason(store, wanted):
    """Return why the offline store cannot serve this pnpm-lock.yaml, or None when it can.

    `make a11y-fetch` records the sha256 of the lockfile it filled the store
    from. A store filled for another lockfile lacks what that lockfile added,
    so for this checkout the store is absent: a skip that names the cure, as
    for the image and the archives, which a bump makes absent the same way.
    """
    offline = store / "offline"
    if not (offline / "store").is_dir():
        return f"no offline pnpm store under {store}; run `make a11y-fetch`"
    try:
        found = (offline / STORE_MARKER).read_bytes().decode("ascii").strip()
    except (OSError, UnicodeDecodeError):
        found = None
    if found is None:
        return (
            f"the offline pnpm store under {store} records no pnpm-lock.yaml digest; "
            "run `make a11y-fetch`"
        )
    if found != wanted:
        return (
            f"the offline pnpm store under {store} was filled for another pnpm-lock.yaml "
            f"(sha256 {found[:12]}, this checkout's is {wanted[:12]}); run `make a11y-fetch`"
        )
    return None


def safe_members(archive):
    """Return an archive's members, bounded, with no absolute or escaping path."""
    members = archive.getmembers()
    if len(members) > MAX_ARCHIVE_MEMBERS:
        raise GateError(f"{len(members)} archive members exceed {MAX_ARCHIVE_MEMBERS}")
    for member in members:
        if member.name.startswith("/") or ".." in member.name.split("/"):
            raise GateError(f"archive member {member.name!r} leaves the target")
    return members


def unpack(pin, store, target):
    """Unpack the verified Node and pnpm archives into `target`; return their directories."""
    node_dir, pnpm_dir = target / "node", target / "pnpm"
    with tarfile.open(store / "downloads" / pin["node"]["file"], "r:xz") as archive:
        archive.extractall(node_dir, members=safe_members(archive), filter="data")
    with tarfile.open(store / "downloads" / pin["pnpm"]["file"], "r:gz") as archive:
        member = archive.getmember("package/pnpm")
        pnpm_dir.mkdir(parents=True, exist_ok=True)
        with archive.extractfile(member) as source, open(pnpm_dir / "pnpm", "wb") as sink:
            shutil.copyfileobj(source, sink)
    (pnpm_dir / "pnpm").chmod(0o755)
    return node_dir / pin["node"]["file"].removesuffix(".tar.xz"), pnpm_dir


class Context:
    """One run: the engine, the pin, and the directories the containers mount."""

    def __init__(self, engine, pin, store, run_dir):
        self.engine, self.pin, self.store, self.run_dir = engine, pin, store, run_dir
        self.work = run_dir / "work"
        self.node = self.pnpm = None
        self.pending = []


def bind(source, target, readonly=False):
    """Return one --mount argument pair; a comma would split the engine's option string."""
    text = str(source)
    if "," in text:
        raise GateError(f"{text!r} contains a comma, which --mount cannot carry")
    suffix = ",readonly" if readonly else ""
    return ["--mount", f"type=bind,source={text},target={target}{suffix}"]


def user_arguments(engine):
    """docker runs as root unless told otherwise; run it as the invoking user."""
    if engine != "docker":
        return []
    user, _reason = uid()
    group, _reason = gid()
    return [] if user is None or group is None else ["--user", f"{user}:{group}"]


def container_argv(context, name, argv, options):
    """Return the engine command line for one container step.

    `options` may set `network` (only the fetch uses it), `path`, `workdir` and
    `env`. Every other property is fixed: the image by digest, no pull, a
    read-only root and the repository read-only. There is no --init: podman
    needs a separately packaged init binary for it (catatonit, only recommended
    by Ubuntu 24.04's podman), and nothing needs one here, because Playwright
    starts the preview server itself and ends it with its process group.
    """
    command = [context.engine, "run", "--rm", "--pull=never", "--name", name]
    command += ["--read-only", "--tmpfs", "/tmp", "--shm-size=1g"]
    command += [] if options.get("network") else ["--network=none"]
    command += user_arguments(context.engine)
    command += bind(ROOT, IN_REPO, readonly=True) + bind(context.work, IN_WORK)
    command += bind(context.node, IN_NODE, readonly=True)
    command += bind(context.pnpm, IN_PNPM, readonly=True)
    command += bind(context.store / "offline", IN_OFFLINE, readonly=not options.get("network"))
    environment = {
        "HOME": f"{IN_WORK}/home",
        "XDG_CACHE_HOME": IN_METADATA,
        "PATH": options.get("path", PINNED_PATH),
        "CI": "1",
        "PLAYWRIGHT_SKIP_BROWSER_DOWNLOAD": "1",
    }
    environment.update(options.get("env", {}))
    for key, value in environment.items():
        command += ["--env", f"{key}={value}"]
    command += ["--workdir", options.get("workdir", f"{IN_WORK}/pkg")]
    return command + [context.pin["image"]["by-digest"]] + list(argv)


def remove_container(context, name):
    """Remove a container a step left behind: ending the engine client does not end it."""
    run([context.engine, "rm", "--force", name], REMOVE_TIMEOUT)


def step(context, label, argv, timeout, **options):
    """Run one container step and keep its log; return (exit code, combined output)."""
    name = f"aegis-a11y-{context.run_dir.name}-{label}"
    context.pending.append(name)
    code, stdout, stderr = run(container_argv(context, name, argv, options), timeout)
    context.pending.remove(name)
    text = stdout + stderr
    logs = context.run_dir / "logs"
    logs.mkdir(parents=True, exist_ok=True)
    (logs / f"{label}.log").write_bytes(text.encode("utf-8"))
    return code, text


def report(name, problems, notes=()):
    """Print one case's outcome and, when it failed, why."""
    print(f"{'PASS' if not problems else 'FAIL'} {name}")
    for note in notes:
        print(f"     {note}")
    for line in problems[:MAX_PROBLEM_LINES]:
        print(f"     {line}")


def tail(text, lines=6):
    """Return the last non-empty lines of a step's output, for a failure note."""
    kept = [line for line in text.splitlines() if line.strip()]
    return kept[-lines:]


ESCAPES = re.compile(r"\x1b\[[0-?]*[ -/]*[@-~]")


def strictly_passed(test):
    """Return True only for a test that was meant to pass, ran, and passed.

    Playwright's own `ok` is also true for an expected failure (`test.fail()`),
    a skip and a flaky retry, each of which would turn a real failure into a
    pass here; so the gate reads the test's expected status, its outcome and
    its last result instead (REQ-CI-02, E04-1).
    """
    results = test.get("results") or [{}]
    return (
        test.get("expectedStatus") == "passed"
        and test.get("status") == "expected"
        and results[-1].get("status") == "passed"
    )


def spec_outcome(spec):
    """Return {title, passed, status, message} for one spec of a Playwright JSON report."""
    tests = spec.get("tests", [])
    errors = [
        ESCAPES.sub("", error.get("message", ""))
        for test in tests
        for result in test.get("results", [])
        for error in result.get("errors", [])
    ]
    return {
        "title": spec.get("title", ""),
        "passed": bool(tests) and all(strictly_passed(test) for test in tests),
        "status": [test.get("status") for test in tests],
        "message": errors,
    }


def spec_outcomes(results):
    """Return one spec_outcome per spec in a Playwright JSON report.

    Suites nest, so they are walked with an explicit stack and a fixed bound
    rather than by recursion (HISS-01, HISS-02).
    """
    outcomes, stack = [], list(results.get("suites", []))
    for _ in range(MAX_SPECS):
        if not stack:
            return outcomes
        suite = stack.pop()
        stack.extend(suite.get("suites", []))
        outcomes += [spec_outcome(spec) for spec in suite.get("specs", [])]
    raise GateError(f"the Playwright report nests more than {MAX_SPECS} suites")


def suite_results(context, out):
    """Return the parsed results.json one suite run wrote, or raise GateError."""
    path = context.work / out / "results.json"
    if not path.is_file():
        raise GateError(f"the suite wrote no {out}/results.json; see {context.run_dir}/logs")
    return spec_outcomes(load_json(path))


def copy_package(context):
    """Copy the package out of the read-only repository mount into the scratch mount."""
    sources = [f"{IN_PACKAGE}/{entry}" for entry in PACKAGE_ENTRIES]
    code, text = step(context, "copy", ["cp", "-R", *sources, f"{IN_WORK}/pkg/"], STEP_TIMEOUT)
    if code != 0:
        raise GateError(f"copying the package out of {IN_REPO} failed: {tail(text)}")


def install_argv():
    """Return the one install command every lockfile case runs."""
    return [
        "pnpm",
        "install",
        "--offline",
        "--frozen-lockfile",
        "--frozen-store",
        "--store-dir",
        IN_STORE,
    ]


def lock_copy(context, name, manifest=None):
    """Copy the three lockfile inputs into /work/<name>, optionally with another package.json."""
    target = context.work / name
    target.mkdir(parents=True, exist_ok=True)
    for entry in LOCK_ENTRIES:
        shutil.copyfile(context.work / "pkg" / entry, target / entry)
    if manifest is not None:
        (target / "package.json").write_bytes(json.dumps(manifest, indent=2).encode("utf-8"))
    return f"{IN_WORK}/{name}"


def refusal_case(name, code, text, needle, note):
    """Report a case that passes only when the command failed with `needle` in its output."""
    problems = []
    if code == 0:
        problems.append(f"expected a refusal, observed exit 0: {note}")
    if needle not in text:
        problems.append(f"expected {needle!r} in the output")
    notes = [f"{note}: exit {code}"] + [line for line in tail(text, 40) if needle in line][:2]
    report(name, problems, notes)
    return problems


def install_case(context):
    """Positive: the pinned Node and pnpm install the package from its lockfile, offline."""
    code, text = step(context, "install", install_argv(), STEP_TIMEOUT)
    problems = [] if code == 0 else [f"pnpm install exited {code}"] + tail(text)
    notes = [f"pnpm install --offline --frozen-lockfile --frozen-store: exit {code}"]
    notes += tail(text, 2)
    report("a11y/lockfile-installs", problems, notes)
    return problems


def mismatch_case(context):
    """Negative: a package.json that no longer matches the lockfile is refused."""
    manifest = load_json(context.work / "pkg" / "package.json")
    pinned = manifest["devDependencies"]["svelte"]
    manifest["devDependencies"]["svelte"] = f"^{pinned}"
    workdir = lock_copy(context, "mismatch", manifest)
    code, text = step(context, "mismatch", install_argv(), STEP_TIMEOUT, workdir=workdir)
    note = f"svelte {pinned} loosened to ^{pinned}, pnpm install --frozen-lockfile"
    return refusal_case(
        "a11y/lockfile-mismatch-refused", code, text, "ERR_PNPM_OUTDATED_LOCKFILE", note
    )


def image_node_case(context):
    """Negative: the image's own Node is refused by the exact engines pin."""
    workdir = lock_copy(context, "image-node")
    code, text = step(
        context, "image-node", install_argv(), STEP_TIMEOUT, workdir=workdir, path=IMAGE_NODE_PATH
    )
    note = "the same install on the image's own Node, without the pinned Node on PATH"
    return refusal_case("a11y/image-node-refused", code, text, "ERR_PNPM_UNSUPPORTED_ENGINE", note)


def last_json(text):
    """Return the last line of `text` that parses as a JSON object, or None."""
    for line in reversed(text.splitlines()[-200:]):
        try:
            value = json.loads(line)
        except ValueError:
            continue
        if isinstance(value, dict):
            return value
    return None


def readback_problems(pin, manifest, found):
    """Return where the versions read inside the container differ from the pins."""
    browser = pin["image"]["browser"]
    playwright = pin["image"]["playwright"]
    wanted = {
        "node": f"v{pin['node']['version']}",
        "playwrightTest": playwright,
        "playwrightCore": playwright,
        "browsersPath": "/ms-playwright",
        "revisionPresent": True,
        "launched": browser["version"],
    }
    for key, name in (("axeCore", "axe-core"), ("axePlaywright", "@axe-core/playwright")):
        wanted[key] = manifest["devDependencies"][name]
    for key in ("svelte", "vite"):
        wanted[key] = manifest["devDependencies"][key]
    for key, name in LINT_READBACK:
        wanted[key] = manifest["devDependencies"][name]
    shell = {"revision": browser["revision"], "browserVersion": browser["version"]}
    problems = [
        f"{key} is {found.get(key)!r}, pinned {value!r}"
        for key, value in wanted.items()
        if found.get(key) != value
    ]
    if found.get("headlessShell") != shell:
        problems.append(f"headless shell is {found.get('headlessShell')!r}, pinned {shell!r}")
    return problems


def readback_case(context):
    """Every admitted version, read back inside the container; the pinned Chromium launches."""
    code, text = step(context, "readback", ["node", "tests/readback.mjs", "--launch"], STEP_TIMEOUT)
    found = last_json(text)
    if code != 0 or found is None:
        problems = [f"the readback exited {code} without a report"] + tail(text)
        report("a11y/toolchain-readback", problems)
        return problems
    manifest = load_json(context.work / "pkg" / "package.json")
    problems = readback_problems(context.pin, manifest, found)
    notes = [
        f"node {found.get('node')}, pnpm {context.pin['pnpm']['version']}, "
        f"@playwright/test {found.get('playwrightTest')}, axe-core {found.get('axeCore')}, "
        f"@axe-core/playwright {found.get('axePlaywright')}, svelte {found.get('svelte')}, "
        f"vite {found.get('vite')}",
        f"chromium-headless-shell revision {found.get('headlessShell', {}).get('revision')} "
        f"in {found.get('browsersPath')}: present {found.get('revisionPresent')}; "
        f"launched {found.get('launched')}",
        f"lint: eslint {found.get('eslint')}, "
        f"eslint-plugin-svelte {found.get('eslintPluginSvelte')}, "
        f"svelte-eslint-parser {found.get('svelteEslintParser')}",
    ]
    report("a11y/toolchain-readback", problems, notes)
    return problems


def browser_absent_case(context):
    """Negative: with the pinned revision absent the launch fails; nothing is downloaded."""
    code, text = step(
        context,
        "browser-absent",
        ["node", "tests/readback.mjs", "--launch"],
        STEP_TIMEOUT,
        env={"PLAYWRIGHT_BROWSERS_PATH": f"{IN_WORK}/no-browsers"},
    )
    note = "PLAYWRIGHT_BROWSERS_PATH without the pinned revision, network disabled"
    return refusal_case(
        "a11y/browser-revision-absent-refused", code, text, "Executable doesn't exist", note
    )


def lint_argv(output, target, no_ignore=False):
    """Return the D100 lint's command line: JSON to `output`, zero warnings allowed."""
    return [
        "pnpm",
        "exec",
        "eslint",
        "--max-warnings",
        "0",
        "--format",
        "json",
        "--output-file",
        output,
        *(["--no-ignore"] if no_ignore else []),
        target,
    ]


def lint_run(context, label, target, no_ignore=False):
    """Lint `target` inside the container; return (exit code, parsed report or None, output)."""
    (context.work / "lint").mkdir(parents=True, exist_ok=True)
    output = f"{IN_WORK}/lint/{label}.json"
    code, text = step(context, label, lint_argv(output, target, no_ignore), STEP_TIMEOUT)
    path = context.work / "lint" / f"{label}.json"
    return code, (load_json(path) if path.is_file() else None), text


def lint_findings(results):
    """Return (linted paths under the package, sorted finding rule ids, suppressed count)."""
    files, findings, suppressed = [], [], 0
    for entry in list(results or [])[:MAX_LINTED_FILES]:
        files.append(str(entry.get("filePath", "")).removeprefix(f"{IN_PKG}/"))
        findings += [message.get("ruleId") or DIRECTIVE for message in entry.get("messages", [])]
        suppressed += len(entry.get("suppressedMessages", []))
    return files, sorted(findings), suppressed


def lintable(root):
    """Return the package-relative JavaScript and Svelte sources under `root`, bounded.

    The same files the configuration lints, minus what it ignores, so a
    source ESLint silently skipped is caught.
    """
    found, stack = [], [root]
    skipped = {"node_modules", "dist", "test-output"}
    for _ in range(MAX_LINTED_FILES):
        if not stack:
            break
        directory = stack.pop()
        for path in sorted(directory.iterdir())[:MAX_LINTED_FILES]:
            relative = path.relative_to(root).as_posix()
            if path.is_dir() and path.name not in skipped and relative != LINT_PLANT_DIR:
                stack.append(path)
            elif path.is_file() and path.suffix in {".js", ".mjs", ".cjs", ".svelte"}:
                found.append(relative)
    return sorted(found)


def lint_problems(code, files, findings, suppressed, expected_files):
    """Return why the package lint is not a clean pass over every source file."""
    problems = [] if code == 0 else [f"ESLint exited {code}"]
    if findings or suppressed:
        problems.append(f"findings {findings[:MAX_PROBLEM_LINES]}, suppressed {suppressed}")
    missed = sorted(set(expected_files) - set(files))
    if missed:
        problems.append(f"not linted: {missed[:MAX_PROBLEM_LINES]}")
    if not any(name.endswith(".svelte") for name in files):
        problems.append("no Svelte file was linted")
    return problems


def lint_case(context):
    """Positive (D100): every JavaScript and Svelte file in the package lints clean."""
    code, results, text = lint_run(context, "lint", ".")
    files, findings, suppressed = lint_findings(results)
    expected = lintable(context.work / "pkg")
    problems = lint_problems(code, files, findings, suppressed, expected)
    problems += [] if results is not None else ["ESLint wrote no report"] + tail(text)
    svelte = sum(1 for name in files if name.endswith(".svelte"))
    notes = [
        f"eslint --max-warnings 0 over ui/concordia-tokens: exit {code}; {len(files)} files "
        f"({len(files) - svelte} JavaScript, {svelte} Svelte), {len(findings)} findings, "
        f"{suppressed} suppressed",
        "rules: max-lines-per-function 60, complexity 10, max-statements 50, no-eval, "
        "no-implied-eval, no-new-func, aegis-hiss/no-self-recursion; inline configuration off",
    ]
    report("a11y/hiss-lint", problems, notes)
    return problems


def lint_plant_case(context, family, file, expected):
    """Negative per rule family, or the boundary when `expected` is empty (D100).

    A plant must make ESLint exit 1 -- a finding, not a crash, which exits 2 --
    with exactly `expected`; the boundary file must exit 0 with no finding.
    """
    code, results, text = lint_run(
        context, f"lint-{family}", f"{LINT_PLANT_DIR}/{file}", no_ignore=True
    )
    files, findings, _suppressed = lint_findings(results)
    wanted_code = 1 if expected else 0
    problems = [] if code == wanted_code else [f"ESLint exited {code}, not {wanted_code}"]
    if results is None or files != [f"{LINT_PLANT_DIR}/{file}"]:
        problems += [f"the report does not cover exactly {file}: {files}"] + tail(text)
    if findings != expected:
        problems.append(f"findings {findings}; the file must produce exactly {expected}")
    name = f"a11y/hiss-lint-{family}" + ("-refused" if expected else "")
    report(name, problems, [f"{LINT_PLANT_DIR}/{file}: exit {code}; findings {findings}"])
    return problems


def lint_cases(context):
    """The D100 lint: the package clean, each planted family refused, the limits admitted."""
    outcomes = [lint_case(context)]
    outcomes += [lint_plant_case(context, *plant) for plant in LINT_PLANTS]
    outcomes.append(lint_plant_case(context, "at-the-limits", LINT_BOUNDARY, []))
    return outcomes


def build_case(context):
    """The static page builds from the installed packages."""
    code, text = step(context, "build", ["pnpm", "run", "build"], STEP_TIMEOUT)
    problems = [] if code == 0 else [f"vite build exited {code}"] + tail(text)
    built = [line.strip() for line in text.splitlines() if line.strip().startswith("dist/")]
    report("a11y/build", problems, [f"pnpm run build: exit {code}"] + built[:4])
    return problems


def reports(context, out):
    """Return the per-test report files one suite run wrote, by name."""
    found = {}
    for path in sorted((context.work / out).glob("*.json"))[:MAX_SPECS]:
        if path.name != "results.json":
            found[path.stem] = load_json(path)
    return found


def rows_of(body, key, fields):
    """Return one list of `fields` per row of `body[key]`."""
    return [[row[field] for field in fields] for row in body[key]]


# What each measured report must record for the suite to count as passed: the
# report, what the gate reads from it, and the value E04-2, D16, D76 and D89
# require. The gate reads these itself rather than relying on the tests'
# verdicts alone, so an assertion lost from the spec still fails here.
MEASURED_REPORTS = (
    (
        "focus-stops",
        lambda body: rows_of(body, "stops", ("kind", "width", "reasons")),
        [["outline", 3, []]] * 3,
    ),
    (
        "falsifier-stripped-outline",
        lambda body: [body["before"]["pass"], body["after"]["kind"], body["after"]["pass"]],
        [True, "none", False],
    ),
    (
        "boundary-width",
        lambda body: rows_of(body, "rows", ("measured", "pass")),
        [[3, True], [2, True], [1, False]],
    ),
    (
        "boundary-contrast",
        lambda body: [[round(row["ratio"], 2), row["pass"]] for row in body["rows"]],
        [[3.0, True], [2.99, False]],
    ),
    (
        "boundary-contrast",
        lambda body: [row["pass"] for row in body["thresholds"]],
        [True, False, False],
    ),
    (
        "d76-stub-theme",
        lambda body: [body["stop"]["width"], body["stop"]["pass"]],
        [1, False],
    ),
    (
        "text-scaling-200",
        lambda body: [body["base"], body["fontSize"], body["scaled"], bool(body["planted"])],
        [[], "32px", [], True],
    ),
    (
        "emulation-reduced-motion",
        lambda body: [body["normal"], body["reduced"]],
        ["0.15s", "0s"],
    ),
    (
        "emulation-forced-colors",
        lambda body: [rows_of(body, "stops", ("width", "reasons")), body["violations"]],
        [[[3, []]] * 3, []],
    ),
)


def measured_problems(found):
    """Return where a measured report differs from what the gate requires of it."""
    problems = []
    for name, read, wanted in MEASURED_REPORTS:
        body = found.get(name)
        if body is None:
            problems.append(f"the suite wrote no {name} report")
            continue
        try:
            seen = read(body)
        except (KeyError, TypeError, IndexError) as error:
            seen = f"unreadable ({type(error).__name__}: {error})"
        if seen != wanted:
            problems.append(f"{name} records {seen!r}; the gate requires {wanted!r}")
    return problems


def default_state_problems(state):
    """Return what the default-state report contradicts in D81 and E04-2."""
    problems = []
    if state is None:
        return ["the suite wrote no default-state report"]
    if state.get("runOnly") != {"type": "tag", "values": D81_TAGS}:
        problems.append(f"axe-core ran {state.get('runOnly')!r}, not the D81 tag set")
    if state.get("violations") or state.get("incomplete"):
        problems.append(
            f"violations {state.get('violations')}, incomplete {state.get('incomplete')}"
        )
    if (state.get("targetSize") or {}).get("bucket") != "passes":
        problems.append(f"target-size did not evaluate the targets: {state.get('targetSize')!r}")
    if state.get("unmapped"):
        problems.append(f"criteria with no clause in the map: {state['unmapped']}")
    return problems


def criterion_notes(state):
    """One line per criterion an executed rule is tagged with, with both EN 301 549 clauses."""
    notes = []
    rows = state.get("criteria", {})
    order = sorted(rows, key=lambda sc: tuple(int(part) for part in sc.split(".")))
    for criterion in order:
        row = rows[criterion]
        rules = ", ".join(row["evaluated"]) or "none with an applicable node"
        idle = f"; ran with no applicable node: {', '.join(row['inapplicable'])}"
        notes.append(
            f"WCAG {criterion} {row['title']} ({row['level']}): EN 301 549 V4.1.1 "
            f"{row['v4.1.1']}, V3.2.1 {row['v3.2.1']}; evaluated by {rules}"
            f"{idle if row['inapplicable'] else ''}"
        )
    for criterion, row in sorted(state.get("notCovered", {}).items()):
        notes.append(
            f"not covered by this gate: V4.1.1 {row['v4.1.1']} (WCAG {criterion} {row['title']}; "
            f"V3.2.1 {row['v3.2.1']}), axe-core {state.get('axe')} has no rule for it"
        )
    return notes


def measured_clause_notes(criteria):
    """Name the clauses the measured checks bear on, from the same clause map."""
    notes = []
    for check, numbers in MEASURED_CRITERIA:
        rows = [(number, criteria[number]) for number in numbers]
        named = [
            f"WCAG {number} (V4.1.1 {row['v4.1.1']}, V3.2.1 {row['v3.2.1']})"
            for number, row in rows
        ]
        notes.append(f"measured, not axe-core: {check} against {' and '.join(named)}")
    return notes


def focus_notes(found):
    """The measured focus, boundary and D76 results, one line each."""
    notes = measured_clause_notes(load_json(CLAUSE_MAP)["criteria"])
    for stop in found.get("focus-stops", {}).get("stops", []):
        notes.append(
            f"focus {stop['element']}: {stop['kind']} {stop['width']}px, "
            f"{stop['ratio']:.2f}:1 against rgb{tuple(stop['background'][:3])}"
        )
    for row in found.get("boundary-width", {}).get("rows", []):
        notes.append(
            f"D16 width boundary: token {row['token']} -> {row['measured']}px, pass {row['pass']}"
        )
    for row in found.get("boundary-contrast", {}).get("rows", []):
        notes.append(
            f"contrast boundary: ring {row['ring']} -> {row['ratio']:.6f}:1, pass {row['pass']}"
        )
    stripped = found.get("falsifier-stripped-outline", {})
    if stripped:
        notes.append(
            f"stripped outline: {stripped['after']['kind']} {stripped['after']['width']}px, "
            f"pass {stripped['after']['pass']}; axe-core violations with it stripped: "
            f"{stripped['axeViolationsWithTheOutlineStripped']}"
        )
    stub = found.get("d76-stub-theme", {}).get("stop")
    if stub:
        notes.append(f"D76 stub theme: {stub['width']}px, pass {stub['pass']}: {stub['reasons']}")
    return notes


def scope_notes(found):
    """Text scaling, and the labelled emulation results that are not portal or AT-SPI2 evidence."""
    notes = []
    scaled = found.get("text-scaling-200", {})
    if scaled:
        notes.append(
            f"200% text: font {scaled['fontSize']}, overflow {scaled['scaled']}; "
            f"planted clip detected: {bool(scaled['planted'])}"
        )
    motion = found.get("emulation-reduced-motion", {})
    if motion:
        notes.append(
            f"[browser emulation, not portal evidence] reduced motion: transition "
            f"{motion['normal']} -> {motion['reduced']}"
        )
    forced = found.get("emulation-forced-colors", {})
    if forced:
        widths = [stop["width"] for stop in forced["stops"]]
        notes.append(
            f"[browser emulation, not portal evidence] forced colours: ring widths {widths}, "
            f"axe-core violations {forced['violations']}"
        )
    if found.get("accessibility-tree"):
        notes.append(
            "[browser accessibility tree, not AT-SPI2] the visually hidden values stay in "
            "Chromium's tree; the toggle reports [pressed]"
        )
    return notes


def suite_case(context):
    """The Playwright and axe-core suite on the default state, with its report."""
    code, text = step(
        context,
        "suite",
        ["pnpm", "exec", "playwright", "test"],
        SUITE_TIMEOUT,
        env={"AEGIS_A11Y_OUT": f"{IN_WORK}/out"},
    )
    outcomes = suite_results(context, "out")
    found = reports(context, "out")
    state = found.get("default-state")
    passed = sum(1 for outcome in outcomes if outcome["passed"])
    problems = [] if code == 0 else [f"playwright test exited {code}"] + tail(text)
    if passed != EXPECTED_TESTS or len(outcomes) != EXPECTED_TESTS:
        problems.append(f"{passed} of {len(outcomes)} tests passed; {EXPECTED_TESTS} are expected")
    problems += [
        f"not a pass: {outcome['title']} (outcome {outcome['status']})"
        for outcome in outcomes
        if not outcome["passed"]
    ]
    problems += default_state_problems(state) + measured_problems(found)
    notes = [f"{passed}/{len(outcomes)} tests passed; exit {code}"]
    if state is not None:
        counts = state["counts"]
        notes.append(
            f"axe-core {state['axe']} on Chromium {state['browser']}, tags "
            f"{', '.join(state['runOnly']['values'])}, no impact filter: "
            f"violations {counts['violations']}, incomplete {counts['incomplete']}, "
            f"passes {counts['passes']}, inapplicable {counts['inapplicable']}"
        )
        target = state["targetSize"] or {}
        notes.append(f"target-size executed: {target.get('bucket')}, {target.get('nodes')} targets")
        notes += criterion_notes(state)
    notes += focus_notes(found) + scope_notes(found)
    report("a11y/suite", problems, notes)
    return problems


def plant_problems(code, outcomes, title, needle):
    """Return why a planted run did not fail the way its case requires.

    It must exit non-zero, run only the selected test, and fail it with a
    message carrying `needle`, so the right test failed for the right reason.
    """
    failed = [outcome for outcome in outcomes if not outcome["passed"]]
    messages = " ".join(message for outcome in failed for message in outcome["message"])
    strays = sorted({o["title"] for o in outcomes if not o["title"].startswith(title)})
    checks = (
        (
            code != 0 and bool(failed),
            f"the suite did not fail with the defect planted: exit {code}",
        ),
        (not strays, f"--grep {title!r} also selected {strays}"),
        (needle in messages, f"no failure carries {needle!r}"),
    )
    return [problem for held, problem in checks if not held]


def plant_case(context, name, plant, title, needle):
    """Negative: one planted defect makes the named test, and so the gate, fail."""
    out = f"out-{plant}"
    code, text = step(
        context,
        f"plant-{plant}",
        ["pnpm", "exec", "playwright", "test", "--grep", title],
        SUITE_TIMEOUT,
        env={"AEGIS_A11Y_OUT": f"{IN_WORK}/{out}", "AEGIS_A11Y_PLANT": plant},
    )
    outcomes = suite_results(context, out)
    failed = [outcome for outcome in outcomes if not outcome["passed"]]
    problems = plant_problems(code, outcomes, title, needle)
    notes = [f"AEGIS_A11Y_PLANT={plant}: exit {code}; failed: {[o['title'] for o in failed]}"]
    notes += [f"failure carries {needle!r}"] if not problems else []
    report(name, problems, notes)
    return problems


def pin_case(pin):
    """The admitted pin, cross-checked against package.json and pnpm-lock.yaml by load_pin."""
    browser = pin["image"]["browser"]
    notes = [
        f"image {pin['image']['reference']}",
        f"linux/amd64 manifest {pin['image']['platform-digests'].get('linux/amd64')}",
        f"browser {browser['name']} revision {browser['revision']} ({browser['version']})",
        f"node {pin['node']['version']}, sha256 {pin['node']['sha256']}",
        f"pnpm {pin['pnpm']['version']}, {pin['pnpm']['integrity']} (pnpm-lock.yaml)",
    ]
    report("a11y/pin", [], notes)
    return []


def truncated_digest_case(pin):
    """Boundary: the full digest pins the image; 63 digits, 12 digits and the source's form fail.

    The last candidate is the reference REQ-P12-04's source itself gives, which
    names another image, so only its digest is put to the same check.
    """
    base, _, digest = pin["image"]["reference"].partition("@")
    candidates = (("full", digest), ("63 digits", digest[:-1]), ("12 digits", digest[:19]))
    notes, verdicts = [], []
    for label, candidate in candidates:
        refused = bool(image_problems(dict(pin["image"], reference=f"{base}@{candidate}")))
        verdicts.append(refused)
        notes.append(f"{label} {candidate}: {'refused' if refused else 'accepted'}")
    refused = digest_problem(SOURCE_REFERENCE.partition("@")[2]) is not None
    verdicts.append(refused)
    notes.append(
        f"REQ-P12-04's source (export-020) {SOURCE_REFERENCE}: "
        f"{'refused' if refused else 'accepted'}"
    )
    problems = [] if verdicts == [False, True, True, True] else [f"verdicts {verdicts}"]
    report("a11y/truncated-digest-refused", problems, notes)
    return problems


def image_case(context):
    """The image is stored locally under its pinned digest."""
    present, digests = image_present(context.engine, context.pin["image"])
    problems = [] if present else [f"{context.pin['image']['by-digest']} is not stored"]
    report("a11y/image-present", problems, [f"{context.engine} image digests: {digests}"])
    return problems


def download(url, target, index, wanted):
    """Download `url` to `target` and keep it only if it hashes to the pin.

    A cached copy that already hashes to the pin is kept rather than fetched
    again; one that does not is replaced.
    """
    if target.is_file() and file_digests(target)[index] == wanted:
        return []
    partial = target.with_name(target.name + ".partial")
    target.parent.mkdir(parents=True, exist_ok=True)
    code, _, stderr = run(
        [
            "curl",
            "-fsSL",
            "--proto",
            "=https",
            "--max-time",
            str(DOWNLOAD_TIMEOUT),
            "-o",
            str(partial),
            url,
        ],
        DOWNLOAD_TIMEOUT + 30,
    )
    if code != 0:
        partial.unlink(missing_ok=True)
        return [f"curl exited {code} for {url}: {stderr.strip()[-200:]}"]
    found = file_digests(partial)[index]
    if found != wanted:
        partial.unlink(missing_ok=True)
        return [f"{url} hashes {found}, the pin records {wanted}; the download was discarded"]
    partial.replace(target)
    return []


def fetch_image(context):
    """Pull the image by digest, never by tag, and confirm the engine stored that digest."""
    image = context.pin["image"]
    code, _, stderr = run([context.engine, "pull", image["by-digest"]], PULL_TIMEOUT)
    present, digests = image_present(context.engine, image)
    problems = [] if code == 0 and present else [f"pull exited {code}: {stderr.strip()[-300:]}"]
    report(
        "a11y-fetch/image", problems, [f"{context.engine} pull {image['by-digest']}", str(digests)]
    )
    return problems


def fetch_archives(context):
    """Download the pinned Node tarball and pnpm binary, each checked against its digest."""
    pin, downloads = context.pin, context.store / "downloads"
    problems = download(
        pin["node"]["url"], downloads / pin["node"]["file"], 0, pin["node"]["sha256"]
    )
    problems += download(
        pin["pnpm"]["url"], downloads / pin["pnpm"]["file"], 1, pin["pnpm"]["integrity"]
    )
    notes = [f"{pin['node']['file']}: sha256 {pin['node']['sha256']}"]
    notes.append(f"{pin['pnpm']['file']}: {pin['pnpm']['integrity']}")
    report("a11y-fetch/archives", problems, notes)
    return problems


def fetch_store(context):
    """Fill the offline pnpm store from the lockfile: the one container step with network.

    The lockfile's sha256 is recorded beside the store only once `pnpm fetch`
    succeeded, and removed before it starts, so a failed fetch leaves no
    record that the gate would take for a filled store.
    """
    marker = context.store / "offline" / STORE_MARKER
    marker.parent.mkdir(parents=True, exist_ok=True)
    marker.unlink(missing_ok=True)
    prepare(context)
    code, text = step(
        context,
        "fetch-store",
        ["pnpm", "fetch", "--store-dir", IN_STORE],
        STEP_TIMEOUT,
        network=True,
    )
    problems = [] if code == 0 else [f"pnpm fetch exited {code}"] + tail(text)
    if not problems:
        marker.write_bytes(f"{context.pin['lock-sha256']}\n".encode("ascii"))
    notes = [f"pnpm fetch into {context.store / 'offline'}: exit {code}"]
    notes += (
        [f"filled for pnpm-lock.yaml sha256 {context.pin['lock-sha256']}"] if not problems else []
    )
    report("a11y-fetch/store", problems, notes)
    return problems


def fetch_main(context):
    """`make a11y-fetch`: pull, download and fill the store; FAIL on any mismatch."""
    problems = fetch_image(context)
    problems += fetch_archives(context)
    if not problems:
        problems += fetch_store(context)
    if problems:
        print("FAIL: the accessibility gate's inputs could not all be fetched and verified.")
        return 1
    print(
        "PASS: image, Node, pnpm and the offline store are cached; "
        "`make verify-all` now runs the gate."
    )
    return 0


def prepare(context):
    """Unpack the verified archives for this run and copy the package into the scratch mount."""
    for directory in (context.work / "pkg", context.work / "home"):
        directory.mkdir(parents=True, exist_ok=True)
    context.node, context.pnpm = unpack(context.pin, context.store, context.run_dir)
    copy_package(context)


def preflight(context):
    """Return why the gate cannot run here; a cached archive that does not verify is a failure."""
    present, _digests = image_present(context.engine, context.pin["image"])
    reasons = []
    if not present:
        reasons.append(
            f"{context.engine} does not store {context.pin['image']['by-digest']}; "
            "run `make a11y-fetch`"
        )
    missing, problems = archive_state(context.pin, context.store)
    if problems:
        raise GateError("; ".join(problems))
    return reasons + missing


def run_cases(context):
    """Run every case in order and return the number that failed."""
    outcomes = [pin_case(context.pin), truncated_digest_case(context.pin), image_case(context)]
    prepare(context)
    outcomes.append(install_case(context))
    if outcomes[-1]:
        raise GateError("the pinned lockfile did not install, so nothing else can run")
    outcomes += lint_cases(context)
    outcomes += [mismatch_case(context), image_node_case(context)]
    outcomes += [readback_case(context), browser_absent_case(context), build_case(context)]
    if outcomes[-1]:
        raise GateError("the static build failed, so the suite has nothing to load")
    outcomes.append(suite_case(context))
    outcomes += [plant_case(context, *plant) for plant in PLANTS]
    return sum(1 for problems in outcomes if problems)


def cleanup(context):
    """Remove any container a step started and did not see end."""
    for name in list(context.pending)[:MAX_PRUNED]:
        remove_container(context, name)


def discard_bulk(run_dir):
    """Remove the unpacked toolchain and installed packages; keep the logs and the reports."""
    for path in (run_dir / "node", run_dir / "pnpm", run_dir / "work" / "home"):
        shutil.rmtree(path, ignore_errors=True)
    work = run_dir / "work"
    if not work.is_dir():
        return
    for directory in sorted(work.iterdir())[:MAX_PRUNED]:
        for bulky in ("node_modules", "dist", "src", "tests"):
            shutil.rmtree(directory / bulky, ignore_errors=True)


def prune_runs(store):
    """Keep the newest MAX_RETAINED_RUNS run directories."""
    runs = store / "runs"
    if not runs.is_dir():
        return
    names = sorted(path for path in runs.iterdir() if path.is_dir())[:MAX_PRUNED]
    for old in names[:-MAX_RETAINED_RUNS]:
        shutil.rmtree(old, ignore_errors=True)


def terminated(signum, _frame):
    """Turn a termination signal into SystemExit, so every container is removed on the way out."""
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
        help="pull the image by digest, download the pinned archives and fill the pnpm store",
    )
    return parser.parse_args(argv)


def gate_main(context):
    """Run the gate: SKIP with a reason, FAIL with a reason, or PASS with every case above it."""
    reasons = preflight(context)
    if reasons:
        for reason in reasons[:MAX_PROBLEM_LINES]:
            print(f"SKIP: {reason}; the accessibility gate did not run.")
        return 0
    print(f"     engine: {context.engine}; run directory: {context.run_dir}")
    failed = run_cases(context)
    if failed:
        print(f"FAIL: {failed} accessibility case(s) did not match their recorded outcome.")
        return 1
    print(
        "PASS: accessibility gate (M04) and HISS lint (M16, D100) in the pinned container "
        "with networking disabled. It covers the P12 token component and its JavaScript and "
        "Svelte; it closes no accessibility gate for the shell or the image, and no portal, "
        "AT-SPI2 or UKI evidence."
    )
    return 0


def main(argv=None):
    """Load the pin, pick an engine, then fetch or run the gate."""
    options = parse_arguments(argv)
    install_signal_handlers()
    print("Accessibility gate (M04, D16, D65, D76, D81; M16, D100; development evidence only).")
    try:
        pin = load_pin()
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    engine, reason = pick_engine()
    if engine is None:
        print(
            f"{'FAIL' if options.fetch else 'SKIP'}: {reason}; the accessibility gate did not run."
        )
        return 1 if options.fetch else 0
    store = cache_dir()
    run_dir = store / "runs" / f"{time.strftime('r%Y%m%dT%H%M%S')}-{secrets.token_hex(2)}"
    context = Context(engine, pin, store, run_dir)
    try:
        return fetch_main(context) if options.fetch else gate_main(context)
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    finally:
        cleanup(context)
        discard_bulk(run_dir)
        prune_runs(store)


if __name__ == "__main__":
    sys.exit(main())
