#!/usr/bin/env python3
"""Run the P17 display slice on the reference profile: VA-API frames to a layer surface.

Milestone M27, decisions D79, D80 and D83. The Rust binary
``aegis-scaena-display`` (``crates/aegis-scaena``) does the work; this gate
reads the toolchain back, checks the committed fixture against its pinned
sha256, builds the binary with ``cargo build --locked`` and starts it twice,
each time with ``LIBVA_DRIVER_NAME`` set in that child's environment only:

* ``run`` with ``LIBVA_DRIVER_NAME=iHD`` is the positive run and the cases that
  live in it: the decode node is the render node whose device number is the
  compositor's ``zwp_linux_dmabuf_v1`` main device, read through
  ``/sys/class/drm``, and every other node is refused; the VA vendor string
  names iHD; the reference MPEG-2 intra frame decodes to cros-libva's CRC-32
  ``0xa5713e52``; a planted zero-filled surface fails the content check; each
  of the 60 fixture frames reads its index row back, is exported with
  ``export_prime``, crosses a ``SOCK_SEQPACKET`` pair with its fd in
  ``SCM_RIGHTS`` and is attached to a ``zwlr_layer_shell_v1`` surface, with 60
  ``created`` events, no ``failed`` event and a ``presented`` event per commit;
  NV12 with ``INTEL_4_TILED_DG2_RC_CCS`` is refused client-side; an attach
  before the first ``ack_configure`` and a presentation that never comes fail
  as they must;
* ``driver-probe`` with the session's ``LIBVA_DRIVER_NAME=nvidia`` must be
  refused before anything is decoded or attached.

The gate prints rustc, libva, the VA vendor string, the compositor it ran
against and the modifier the driver chose, and labels every pass development
evidence for the client half only (D79): the compositor is the host session's,
not P04, and nothing here is evidence that P04 serves either protocol.

Without the reference profile's capabilities -- not Linux, no Wayland session,
fewer than two DRM render nodes (negative (b) needs a node that is not the
compositor's main device), no iHD driver, no cargo, or a locked session, whose
compositor presents no client surface -- it prints
``SKIP: <reason>; the display slice gate did not run.`` and exits 0, so an exit
0 is evidence only with the case lines above it. It is not part of
``make verify-all``: the Verification gate's runner has no GPU and no Wayland
session, and a gate that always skips is not a gate. Its hardware-free half,
``tools/test_display_slice.py`` and the crate's own tests, runs there.

Host safety: the layer surface is 256 x 64, takes no keyboard focus, lives for
about sixty frames and is destroyed by the binary on every path it can catch;
the binary is killed at its deadline. Nothing is installed, no driver is bound
or changed, and the compositor binary is never executed: its version is read
from the package database, never from the program.
"""

import hashlib
import json
import os
import re
import secrets
import shutil
import subprocess
import sys
import time
from pathlib import Path

import display_toolchain

ROOT = Path(__file__).resolve().parent.parent
CRATE = "aegis-scaena"
BINARY = "aegis-scaena-display"
FIXTURE = ROOT / "crates" / "aegis-scaena" / "fixtures" / "index-blocks-60.mjpeg"
# The committed Motion-JPEG fixture, pinned (M27 criterion 5). It was generated
# once with ffmpeg n9.0.2 by FIXTURE_COMMAND; no gate runs ffmpeg.
FIXTURE_SHA256 = "fd45d15a2837113fc2d581f04a9b82ca7992a6d2b8940f20ab9cc95fe6767be5"
FIXTURE_BYTES = 48477
FIXTURE_BITS = tuple(
    f"drawbox=x={32 + 16 * (5 - bit)}:y=16:w=16:h=16:c=white:t=fill"
    f":enable='eq(mod(floor((n+1)/{1 << bit}),2),1)'"
    for bit in range(5, -1, -1)
)
FIXTURE_FILTER = ",".join(
    (
        "color=c=0x808080:s=256x64:r=30",
        "format=yuvj420p",
        "drawbox=x=16:y=16:w=16:h=16:c=white:t=fill",
        "drawbox=x=32:y=16:w=112:h=16:c=black:t=fill",
    )
    + FIXTURE_BITS
)
FIXTURE_COMMAND = (
    "ffmpeg",
    "-nostdin",
    "-hide_banner",
    "-loglevel",
    "error",
    "-f",
    "lavfi",
    "-i",
    FIXTURE_FILTER,
    "-frames:v",
    "60",
    "-c:v",
    "mjpeg",
    "-q:v",
    "2",
    "-huffman",
    "default",
    "-fflags",
    "+bitexact",
    "-flags",
    "+bitexact",
    "-f",
    "mjpeg",
    "index-blocks-60.mjpeg",
)

SYS_CLASS_DRM = Path("/sys/class/drm")
IHD_DRIVER_FILE = "iHD_drv_video.so"
DRIVER_DIRECTORIES = (
    "/usr/lib/dri",
    "/usr/lib64/dri",
    "/usr/lib/x86_64-linux-gnu/dri",
    "/usr/local/lib/dri",
)
POSITIVE_DRIVER = "iHD"
NEGATIVE_DRIVER = "nvidia"

# Deadlines (HISS-02): every child carries one.
VERSION_TIMEOUT = 30
BUILD_TIMEOUT = 1800
RUN_TIMEOUT = 120
# Scalar bounds on what is read.
MAX_OUTPUT_LINES = 400
MAX_BUILD_LINES = 20000
MAX_DRM_ENTRIES = 256
MAX_PROBLEM_LINES = 24

# Every program this gate may start, by the name it is started under. The
# compositor is not among them: its version comes from the package database.
PROGRAMS = ("cargo", "rustc", "pkg-config", "clang", "loginctl", "pacman", BINARY)

# The modifiers the reference profile's driver and compositor use, by name.
MODIFIER_NAMES = {
    0x0: "DRM_FORMAT_MOD_LINEAR",
    0x0100000000000001: "I915_FORMAT_MOD_X_TILED",
    0x0100000000000009: "I915_FORMAT_MOD_4_TILED",
    0x010000000000000A: "I915_FORMAT_MOD_4_TILED_DG2_RC_CCS",
    0x010000000000000B: "I915_FORMAT_MOD_4_TILED_DG2_MC_CCS",
    0x010000000000000C: "I915_FORMAT_MOD_4_TILED_DG2_RC_CCS_CC",
}

# The cases each child must report as passed.
RUN_CASES = (
    "display/attach-before-ack-refused",
    "display/other-render-node-refused",
    "display/driver-is-ihd",
    "display/mpeg2-reference-crc",
    "display/planted-zero-surface-refused",
    "display/fixture-frames-present",
    "display/received-fd-is-the-exported-one",
    "display/descriptor-carries-the-exported-modifier",
    "display/unadvertised-pair-refused",
    "display/frame-crc-regression-pin",
    "display/presented-deadline",
)
PROBE_CASES = ("display/driver-not-ihd-refused",)
# The case a child may report with a NOTE line instead of PASS or FAIL: the
# per-frame regression pins, which a run under another driver than the one
# they were recorded under reports and does not hold (M27 criterion 5, D73).
NOTED_CASES = ("display/frame-crc-regression-pin",)
EXIT_SKIP = 2


class GateError(Exception):
    """The gate could not be run at all, as opposed to a case that failed."""


def run(argv, timeout, environ=None, cwd=None):
    """Run `argv` under a hard deadline and return (exit code, stdout, stderr)."""
    if Path(argv[0]).name not in PROGRAMS:
        raise GateError(f"{argv[0]} is not a program this gate may start")
    env = dict(os.environ if environ is None else environ)
    env["LC_ALL"] = "C"
    env["LANG"] = "C"
    try:
        done = subprocess.run(
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
    except subprocess.TimeoutExpired as error:
        raise GateError(f"{Path(argv[0]).name} exceeded its {timeout}s deadline") from error
    except OSError as error:
        raise GateError(f"{argv[0]} could not be run: {error}") from error
    return done.returncode, done.stdout, done.stderr


def child_environment(driver, environ=None):
    """Return a copy of the environment with LIBVA_DRIVER_NAME set for one child.

    The gate's own environment is never changed: the variable exists only in
    the copy handed to the child process.
    """
    env = dict(os.environ if environ is None else environ)
    env["LIBVA_DRIVER_NAME"] = driver
    return env


def wayland_socket(environ):
    """Return the Wayland socket path the session names, or None.

    libwayland takes an absolute WAYLAND_DISPLAY as the socket path itself and
    a relative one under XDG_RUNTIME_DIR. On Linux `is_absolute()` is exactly
    its leading-slash test; unlike a literal "/" test it also holds for the
    scratch paths the Windows leg of the portability matrix builds (HISS-21).
    """
    display = environ.get("WAYLAND_DISPLAY", "")
    if not display:
        return None
    if Path(display).is_absolute():
        return Path(display)
    runtime = environ.get("XDG_RUNTIME_DIR", "")
    return Path(runtime) / display if runtime else None


def render_nodes(root=SYS_CLASS_DRM):
    """Return the sorted renderD* entries under `root`, bounded."""
    if not root.is_dir():
        return []
    names = []
    for entry in root.iterdir():
        if len(names) >= MAX_DRM_ENTRIES:
            break
        if entry.name.startswith("renderD"):
            names.append(entry.name)
    return sorted(names)


def ihd_driver(environ):
    """Return the path of the iHD VA driver libva would load, or None.

    libva splits LIBVA_DRIVERS_PATH on ENV_VAR_SEPARATOR (va/va.c): ":" on
    Linux, ";" on Windows, which is `os.pathsep` on each. Splitting on a
    literal ":" would cut a Windows drive letter off its path (HISS-21).
    """
    search = environ.get("LIBVA_DRIVERS_PATH", "")
    extra = [part for part in search.split(os.pathsep) if part]
    for directory in extra + list(DRIVER_DIRECTORIES):
        candidate = Path(directory) / IHD_DRIVER_FILE
        if candidate.is_file():
            return candidate
    return None


def node_reasons(nodes, drm_root):
    """Return why `nodes` cannot carry the run: none, or only one.

    Negative (b) refuses a render node that is not the compositor's main
    device; a host with one render node has none to refuse, so it lacks a
    capability of the reference profile rather than failing the case.
    """
    if not nodes:
        return [f"no DRM render node under {drm_root}"]
    if len(nodes) < 2:
        return [
            f"one DRM render node under {drm_root} ({nodes[0]}); negative (b) needs a "
            "second one that is not the compositor's main device"
        ]
    return []


def capability_reasons(platform=None, environ=None, which=shutil.which, drm_root=SYS_CLASS_DRM):
    """Return one reason per reference-profile capability this host lacks."""
    platform = sys.platform if platform is None else platform
    environ = os.environ if environ is None else environ
    if not platform.startswith("linux"):
        return [
            f"the display slice needs DRM render nodes, VA-API and a Wayland session; "
            f"this host is {platform}"
        ]
    reasons = []
    socket = wayland_socket(environ)
    if socket is None or not socket.exists():
        reasons.append("no Wayland session socket (WAYLAND_DISPLAY, XDG_RUNTIME_DIR)")
    reasons += node_reasons(render_nodes(drm_root), drm_root)
    if ihd_driver(environ) is None:
        reasons.append(f"no {IHD_DRIVER_FILE} in LIBVA_DRIVERS_PATH or {DRIVER_DIRECTORIES}")
    for program in ("cargo", "rustc"):
        if which(program) is None:
            reasons.append(f"{program} is not on PATH")
    return reasons


def session_locked(environ=None, runner=run, which=shutil.which):
    """Return a reason when logind reports the session locked, else None.

    A locked session's compositor shows its lock screen and presents no client
    surface, so no presentation event can arrive; that is a host state, not a
    result. Without loginctl the question cannot be asked and the run proceeds.
    """
    environ = os.environ if environ is None else environ
    if which("loginctl") is None:
        return None
    session = environ.get("XDG_SESSION_ID") or "auto"
    argv = ["loginctl", "show-session", session, "-p", "LockedHint", "--value"]
    code, stdout, _stderr = runner(argv, VERSION_TIMEOUT)
    if code == 0 and stdout.strip() == "yes":
        return (
            f"logind reports session {session} locked; its compositor presents no "
            "client surface while the lock screen is up"
        )
    return None


def fixture_problems(path=FIXTURE):
    """Return why the committed fixture is not the pinned one, or an empty list."""
    if path.is_symlink() or not path.is_file():
        return [f"{path} is missing or a symlink"]
    data = path.read_bytes()
    if len(data) != FIXTURE_BYTES:
        return [f"{path.name} is {len(data)} bytes, not the pinned {FIXTURE_BYTES}"]
    digest = hashlib.sha256(data).hexdigest()
    if digest != FIXTURE_SHA256:
        return [f"{path.name} hashes to {digest}, not the pinned {FIXTURE_SHA256}"]
    return []


def rustc_version(runner=run):
    """Return the rustc that builds the binary, read back."""
    code, stdout, stderr = runner(["rustc", "--version"], VERSION_TIMEOUT, cwd=ROOT)
    if code != 0:
        raise GateError(f"rustc --version exited {code}: {stderr.strip()[:200]}")
    return stdout.strip()


def libva_version(runner=run):
    """Return (libva, VA-API) as pkg-config reads them back."""
    versions = []
    for argv in (
        ["pkg-config", "--variable=libva_version", "libva"],
        ["pkg-config", "--modversion", "libva"],
    ):
        code, stdout, _stderr = runner(argv, VERSION_TIMEOUT)
        versions.append(stdout.strip() if code == 0 else "unread")
    return tuple(versions)


def executable_from(messages, name=BINARY):
    """Return the executable cargo reported for `name` in its JSON messages."""
    for line in messages.splitlines()[:MAX_BUILD_LINES]:
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


def build(runner=run):
    """Build the binary with the locked graph and return its path."""
    argv = [
        "cargo",
        "build",
        "--locked",
        "-p",
        CRATE,
        "--bin",
        BINARY,
        "--message-format=json-render-diagnostics",
    ]
    code, stdout, stderr = runner(argv, BUILD_TIMEOUT, cwd=ROOT)
    if code != 0:
        tail = "\n".join(stderr.strip().splitlines()[-MAX_PROBLEM_LINES:])
        raise GateError(f"cargo build exited {code}:\n{tail}")
    executable = executable_from(stdout)
    if executable is None or not executable.is_file():
        raise GateError(f"cargo reported no executable for {BINARY}")
    return executable


def parse_child(stdout):
    """Return (info, cases, result, notes) from a child's report lines."""
    info, cases, notes, result = {}, {}, [], None
    current = None
    for line in stdout.splitlines()[:MAX_OUTPUT_LINES]:
        match = re.match(r"^(PASS|FAIL) (\S+)$", line)
        if match:
            current = match.group(2)
            cases[current] = match.group(1) == "PASS"
        elif line.startswith("info ") and ": " in line:
            key, value = line[5:].split(": ", 1)
            info[key] = value
        elif line.startswith("RESULT "):
            result = line[7:]
        elif line.startswith(("SKIP ", "NOTE ")):
            notes.append(line)
    return info, cases, result, notes


def case_problems(cases, required, notes=()):
    """Return one problem per required case that did not pass.

    A required case with no PASS or FAIL line is looked up in the child's
    NOTE and SKIP lines (`unreported`).
    """
    problems = []
    for name in required:
        if name not in cases:
            problems += unreported(name, notes)
        elif not cases[name]:
            problems.append(f"{name} failed")
    return problems


def unreported(name, notes):
    """Return the problem for a required case that printed no PASS or FAIL line.

    A NOTE line for a case in NOTED_CASES reports it without holding it -- the
    regression pins under another driver (D73) -- and is no problem. Any other
    NOTE or SKIP line naming the case says why it did not run, which fails the
    gate with that reason; a case no line names did not report.
    """
    prefix = f"{name}: "
    for line in notes:
        kind, _, rest = line.partition(" ")
        if not rest.startswith(prefix):
            continue
        if kind == "NOTE" and name in NOTED_CASES:
            return []
        return [f"{name} did not run: {rest[len(prefix) :]}"]
    return [f"{name} did not report"]


def not_held(cases, notes):
    """Return the NOTED_CASES a child reported by a NOTE line and did not hold."""
    return [
        name
        for name in NOTED_CASES
        if name not in cases and any(line.startswith(f"NOTE {name}: ") for line in notes)
    ]


def run_id():
    """Return a run identifier: the UTC start time and a random suffix."""
    return time.strftime("r%Y%m%dT%H%M%S", time.gmtime()) + "-" + secrets.token_hex(2)


def retain_dir(environ=None):
    """Return the out-of-repository directory the runs' logs are kept in."""
    environ = os.environ if environ is None else environ
    override = environ.get("AEGIS_DISPLAY_DIR")
    if override:
        return Path(override).expanduser()
    cache = environ.get("XDG_CACHE_HOME")
    return (Path(cache).expanduser() if cache else Path.home() / ".cache") / "aegis-display"


def run_child(binary, mode, driver, retain=None):
    """Start the binary in `mode` with LIBVA_DRIVER_NAME=`driver` for it alone."""
    print(f"     starting {binary.name} {mode}; LIBVA_DRIVER_NAME={driver} for that child only")
    code, stdout, stderr = run(
        [str(binary), mode], RUN_TIMEOUT, environ=child_environment(driver), cwd=ROOT
    )
    for line in stdout.splitlines()[:MAX_OUTPUT_LINES]:
        print(line)
    for line in stderr.strip().splitlines()[:MAX_PROBLEM_LINES]:
        print(f"     stderr: {line}")
    if retain is not None:
        log = f"exit {code}\n{stdout}\n--- stderr\n{stderr}"
        (retain / f"{mode}.log").write_text(log, encoding="utf-8")
    return code, stdout


def compositor(pid, runner=run, which=shutil.which):
    """Name the compositor process `pid` without executing it."""
    if not pid or not pid.isdigit():
        return "unidentified (the kernel reported no peer process)"
    try:
        executable = os.readlink(f"/proc/{pid}/exe")
    except OSError as error:
        return f"pid {pid}, executable unreadable ({error.strerror})"
    package = "package not read (no pacman)"
    if which("pacman") is not None:
        code, stdout, _stderr = runner(["pacman", "-Qo", executable], VERSION_TIMEOUT)
        owned = re.search(r" is owned by (\S+) (\S+)", stdout)
        if code == 0 and owned:
            package = f"package {owned.group(1)} {owned.group(2)}"
    return f"{executable} (pid {pid}, {package})"


def modifier_name(text):
    """Return a modifier's hexadecimal value with its DRM name, when known."""
    try:
        value = int(text, 16)
    except ValueError:
        return text
    return f"{text} ({MODIFIER_NAMES.get(value, 'unnamed here')})"


def summary(run_info, toolchain):
    """Print the facts every recorded pass carries (M27 criterion 5, D79)."""
    rustc, (libva, va_api) = toolchain
    print(f"     rustc: {rustc}")
    print(f"     libva: {libva} (VA-API {va_api}), read back with pkg-config")
    print(f"     VA vendor string: {run_info.get('VA vendor string', 'unread')}")
    print(f"     compositor: {compositor(run_info.get('compositor pid'))}")
    print(f"     decode node: {run_info.get('decode node', 'unread')}")
    chosen = run_info.get("modifier the driver chose", "unread")
    print(f"     modifier the driver chose: {modifier_name(chosen)}")
    for name in run_info.get("not held", []):
        print(
            f"     {name}: reported, not held; the pins were recorded under another "
            "driver and are re-measured and recorded with this one (D73)"
        )
    print("     label: client half only (D79); the compositor is the host session's, not P04")


def prepare():
    """Read the toolchain back, check the fixture and build; return what ran."""
    reasons = display_toolchain.check_toolchain() + display_toolchain.check_locked(
        display_toolchain.read_lock()
    )
    reasons += fixture_problems()
    if reasons:
        raise GateError("; ".join(reasons[:MAX_PROBLEM_LINES]))
    toolchain = (rustc_version(), libva_version())
    print(f"     fixture: {FIXTURE.name}, sha256 {FIXTURE_SHA256} (pinned)")
    return toolchain, build()


def run_cases(binary, retain=None):
    """Start both children; return (skip reason or None, failures, run info)."""
    code, stdout = run_child(binary, "run", POSITIVE_DRIVER, retain)
    info, cases, result, notes = parse_child(stdout)
    if code == EXIT_SKIP and result and result.startswith("skip: "):
        return result[len("skip: ") :], [], info
    failures = case_problems(cases, RUN_CASES, notes)
    info["not held"] = not_held(cases, notes)
    if code != 0 and not failures:
        failures.append(f"{BINARY} run exited {code}: {result}")
    probe_code, probe_stdout = run_child(binary, "driver-probe", NEGATIVE_DRIVER, retain)
    _probe_info, probe_cases, probe_result, probe_notes = parse_child(probe_stdout)
    probe_failures = case_problems(probe_cases, PROBE_CASES, probe_notes)
    if probe_code != 0 and not probe_failures:
        probe_failures.append(f"{BINARY} driver-probe exited {probe_code}: {probe_result}")
    return None, failures + probe_failures, info


def skip(reasons):
    """Print one SKIP line per reason and return the gate's exit status."""
    for reason in reasons[:MAX_PROBLEM_LINES]:
        print(f"SKIP: {reason}; the display slice gate did not run.")
    return 0


def main():
    """Run the gate; exit 0 on a pass or a skip, 1 on a failure."""
    print("Display slice gate (M27, D79, D80, D83; development evidence, client half only).")
    reasons = capability_reasons()
    if reasons:
        return skip(reasons)
    locked = session_locked()
    if locked:
        return skip([locked])
    identifier = run_id()
    retain = retain_dir() / identifier
    print(f"     run {identifier}; logs kept in {retain}")
    try:
        retain.mkdir(parents=True, exist_ok=True)
        toolchain, binary = prepare()
        skipped, failures, info = run_cases(binary, retain)
    except (GateError, display_toolchain.ToolchainError, OSError) as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    if skipped:
        return skip([skipped])
    summary(info, toolchain)
    for failure in failures[:MAX_PROBLEM_LINES]:
        print(f"     {failure}")
    if failures:
        print(f"FAIL: {len(failures)} display slice case(s) did not pass.")
        return 1
    print(
        "PASS: display slice on the reference profile (M27): VA-API frames decoded on the "
        "compositor's main device under iHD, checked against content, exported and passed "
        "over SCM_RIGHTS, and presented on a zwlr_layer_shell_v1 surface. Client half only "
        "(D79): the compositor is the host session's, not P04; development evidence that "
        "closes no hardware, accessibility or release gate."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
