#!/usr/bin/env python3
"""Run M21's two workstation slices on the reference profile: RAPL energy and KVM sandboxing.

Milestone M21, epics E21-1 and E21-2, decisions D58, D60 and D71. Two halves,
each of which runs or prints why it did not:

* the RAPL half enumerates ``/sys/class/powercap/*`` and records every zone's
  name, range and ``energy_uj`` mode as measured; reads ``energy_uj`` once
  without privilege, which must fail; and reads it twice, a recorded interval
  apart, through the one privileged command this gate may run --
  ``sudo -n cat /sys/class/powercap/intel-rapl:0/energy_uj`` (the maintainer's
  decision of 2026-09-29), each under a deadline. ``aegis-tellus-rapl``
  (``crates/aegis-tellus-rapl``) turns the reads into a rollover-safe delta, a
  Measured wattage behind the M05 seam and an SCI rate from the unchanged
  engine, and refuses the unprivileged read and the dram and psys zones. With
  ``--observe-wrap`` it keeps reading, ten seconds apart, until the counter is
  seen to wrap;
* the sandbox half builds ``aegis-vesta-guest`` statically, packs it as the
  initramfs's ``/init``, and runs ``aegis-vesta-sandbox``: Firecracker 1.17.0,
  fetched and pinned by ``build/sandbox/firecracker.pin.json``, boots microVMs
  the ``aegis-vesta`` controller admitted, one candidate evaluation round-trips
  over ``AF_VSOCK``, the 64th microVM is accepted with all 64 running, the 65th
  and an over-limit memory request are refused, and everything is torn down.
  The gate then looks for any process, socket or network device left behind.

A host without a half's capabilities prints
``SKIP: <reason>; the <half> did not run.`` and exits 0, so an exit 0 is
evidence only with the case lines above it: not Linux, no powercap zone, no
``sudo`` or no passwordless ``sudo -n`` (classic sudo's "a password is required"
or sudo-rs's "interactive authentication is required"), no cargo, running as
root (the unprivileged read would not be unprivileged), another CPU than the
reference profile's; not x86_64 (the pinned Firecracker and guest are), no
read-write ``/dev/kvm``, no fetched Firecracker or kernel, too little free
memory for 64 guests, or no cargo. A cache that is present but wrong is a FAIL.

Every run keeps, under ``<cache>/runs/<run id>``, ``host.json`` (the CPU model,
the kernel and the rustc read back, criterion 5) beside ``rapl-readings.json``,
and ``gate.log``, everything the gate printed, gate-side cases included.
It is not part of ``make verify-all``: the Verification gate's runner has no
powercap counter, no passwordless sudo and no ``/dev/kvm``, and a gate that
always skips is not a gate. Its hardware-free half,
``tools/test_workstation_slices.py`` and the crates' own tests, runs there.

``python3 tools/verify_workstation.py --fetch`` (``make workstation-fetch``) is
the one networked step.

Host safety: no GPU is touched, no driver, module or sysctl is changed, nothing
is installed, and ``sudo`` runs exactly one read-only command. Every microVM
runs as the invoking user with no drive and no network interface, so no tap
device exists; every process, socket and directory it creates is removed or
kept only as a log under the run directory. A pass is development evidence on
the reference profile: it qualifies no hardware and closes no hardware,
isolation or release gate, and M22, not this gate, measures boot time and
footprint (D71).
"""

import argparse
import hashlib
import json
import os
import re
import secrets
import shutil
import signal
import stat
import subprocess
import sys
import tarfile
import time
from contextlib import redirect_stdout
from pathlib import Path, PurePosixPath
from platform import machine as platform_machine

import host

ROOT = Path(__file__).resolve().parent.parent
PIN_FILE = ROOT / "build" / "sandbox" / "firecracker.pin.json"

POWERCAP = Path("/sys/class/powercap")
RAPL_ZONE = "intel-rapl:0"
ENERGY_ATTRIBUTE = "energy_uj"
# The only privileged command this gate may run (maintainer, 2026-09-29).
SUDO_READ = ("sudo", "-n", "cat", "/sys/class/powercap/intel-rapl:0/energy_uj")
# What `ls -d /sys/class/powercap/*` and each zone's `name` read on the reference
# profile (M21 criterion 7); intel-rapl is the control type and has no name.
RECORDED_ZONES = {"intel-rapl": None, "intel-rapl:0": "package-0", "intel-rapl:0:0": "core"}
RECORDED_RANGE_UJ = 65532610987
ENERGY_MODE = 0o400
REFERENCE_CPU = "AMD Ryzen 9 9950X3D 16-Core Processor"
READINGS_SCHEMA = "aegis.m21.rapl-readings.v1"
HOST_SCHEMA = "aegis.m21.host.v1"
# What sudo prints when `sudo -n` would have to ask: classic sudo, then sudo-rs
# (src/common/error.rs, Error::InteractionRequired). Read under LC_ALL=C.
SUDO_UNAVAILABLE = ("a password is required", "interactive authentication is required")
PAIR_SPACING = 5.0
WATCH_SPACING = 10.0
WATCH_BOUND = 1500.0

RAPL_CRATE = "aegis-tellus-rapl"
RAPL_BINARY = "aegis-tellus-rapl"
SANDBOX_CRATE = "aegis-vesta-sandbox"
HOST_BINARY = "aegis-vesta-sandbox"
GUEST_BINARY = "aegis-vesta-guest"
GUEST_TARGET = "x86_64-unknown-linux-gnu"
# The architecture of the pinned Firecracker binary, guest kernel and guest init.
PINNED_ARCH = "x86_64"
GUEST_RUSTFLAGS = "-C target-feature=+crt-static"
KVM = Path("/dev/kvm")
MEMINFO = Path("/proc/meminfo")
CPUINFO = Path("/proc/cpuinfo")
NET = Path("/sys/class/net")
PROC = Path("/proc")
# 64 guests at 64 MiB with Firecracker's own share, doubled for headroom.
MIN_AVAILABLE_MIB = 9216

# Deadlines (HISS-02): every child carries one.
SUDO_TIMEOUT = 10
VERSION_TIMEOUT = 30
BUILD_TIMEOUT = 1800
RAPL_TIMEOUT = 60
SANDBOX_TIMEOUT = 600
DOWNLOAD_TIMEOUT = 600
# Scalar bounds on what is read.
MAX_OUTPUT_LINES = 400
MAX_BUILD_LINES = 20000
MAX_PROBLEM_LINES = 24
MAX_POWERCAP_ENTRIES = 64
MAX_PROC_ENTRIES = 65536
MAX_NET_ENTRIES = 1024
MAX_WATCH_READS = 200
MAX_ELF_HEADERS = 64
MAX_CPUINFO_LINES = 20000
MAX_WALK_ENTRIES = 4096

FIRECRACKER_NAME = "firecracker-v1.17.0-x86_64"
PROGRAMS = ("cargo", "rustc", "sudo", "curl", FIRECRACKER_NAME, RAPL_BINARY, HOST_BINARY)

RAPL_CASES = (
    "rapl/recorded-range",
    "rapl/measured-sci",
    "rapl/unprivileged-read-fails-closed",
    "rapl/dram-zone-refused",
    "rapl/psys-zone-refused",
    "rapl/wrap-at-live-range",
)
WRAP_CASE = "rapl/observed-wrap"
SANDBOX_CASES = (
    "sandbox/over-limit-memory-refused",
    "sandbox/boot-and-round-trip",
    "sandbox/sixty-fourth-accepted",
    "sandbox/sixty-fifth-refused",
    "sandbox/teardown",
)
SKIP, PASS, FAIL = "skip", "pass", "fail"


class GateError(Exception):
    """The gate could not run a step, as opposed to a case that failed."""


class Unavailable(Exception):
    """The host lacks a capability; the half prints the reason as a SKIP."""


def check_argv(argv):
    """Refuse a program this gate may not start, and any sudo but the one read.

    The sudo check is on the program's name, not on argv[0]: a path-qualified
    sudo is still sudo, and SUDO_READ names it bare, so it is refused whole.
    """
    name = Path(argv[0]).name
    if name not in PROGRAMS:
        raise GateError(f"{argv[0]} is not a program this gate may start")
    if name == "sudo" and tuple(argv) != SUDO_READ:
        raise GateError(f"sudo may run only {' '.join(SUDO_READ)}; refused {' '.join(argv)}")


def child_env(environ=None, extra=None):
    """Return a copy of the environment with a C locale and `extra` set."""
    env = dict(os.environ if environ is None else environ)
    env["LC_ALL"] = "C"
    env["LANG"] = "C"
    env.update(extra or {})
    return env


def run(argv, timeout, extra_env=None, cwd=None):
    """Run `argv` under a hard deadline and return (exit code, stdout, stderr)."""
    check_argv(argv)
    try:
        done = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            env=child_env(extra=extra_env),
            cwd=cwd,
            stdin=subprocess.DEVNULL,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise GateError(f"{Path(argv[0]).name} exceeded its {timeout}s deadline") from error
    except OSError as error:
        raise GateError(f"{argv[0]} could not be run: {error}") from error
    return done.returncode, done.stdout, done.stderr


def load_pin(path=PIN_FILE):
    """Return the pin file, refusing one that lacks a digest where one is needed."""
    pin = json.loads(path.read_text(encoding="utf-8"))
    firecracker, kernel = pin["firecracker"], pin["guest_kernel"]
    for label, digest in (
        ("archive", firecracker["archive"]["sha256"]),
        ("binary", firecracker["binary"]["sha256"]),
        ("licence", firecracker["licence_file"]["sha256"]),
        ("kernel", kernel["sha256"]),
        ("kernel config", kernel["config"]["sha256"]),
    ):
        if not re.fullmatch(r"[0-9a-f]{64}", digest):
            raise GateError(f"the pin's {label} digest is not 64 lowercase hex digits")
    return pin


def file_sha256(path):
    """Return the sha256 of `path`, read in bounded chunks."""
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def cpu_model(path=CPUINFO):
    """Return the first `model name` in /proc/cpuinfo, or None."""
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
    except OSError:
        return None
    for line in lines[:MAX_CPUINFO_LINES]:
        if line.startswith("model name") and ":" in line:
            return line.split(":", 1)[1].strip()
    return None


def host_facts():
    """Return the host model and kernel every recorded run carries (criterion 5)."""
    release, reason = host.kernel_release()
    return {"cpu": cpu_model() or "unread", "kernel": release or f"unread ({reason})"}


def rustc_version(runner=run, which=shutil.which):
    """Return the rustc that builds the binaries, read back, or why it was not."""
    if which("rustc") is None:
        return "unread (rustc is not on PATH)"
    try:
        code, stdout, stderr = runner(["rustc", "--version"], VERSION_TIMEOUT, cwd=ROOT)
    except GateError as error:
        return f"unread ({error})"
    return stdout.strip() if code == 0 else f"unread (exit {code}: {stderr.strip()[:120]})"


def run_id():
    """Return a run identifier: the UTC start time and a random suffix."""
    return time.strftime("r%Y%m%dT%H%M%S", time.gmtime()) + "-" + secrets.token_hex(2)


def cache_dir(environ=None):
    """Return the out-of-repository directory fetched bytes and runs are kept in."""
    environ = os.environ if environ is None else environ
    override = environ.get("AEGIS_WORKSTATION_DIR")
    if override:
        return Path(override).expanduser()
    cache = environ.get("XDG_CACHE_HOME")
    return (Path(cache).expanduser() if cache else Path.home() / ".cache") / "aegis-workstation"


def tail(text, lines=MAX_PROBLEM_LINES):
    """Return the last `lines` lines of `text`."""
    return "\n".join(text.strip().splitlines()[-lines:])


# --- The RAPL half ---------------------------------------------------------


def rapl_reasons(platform=None, which=shutil.which, powercap=POWERCAP, cpu=None, uid=host.uid):
    """Return one reason per capability the RAPL half needs and this host lacks."""
    platform = sys.platform if platform is None else platform
    if not platform.startswith("linux"):
        return [f"the RAPL half reads the Linux powercap class; this host is {platform}"]
    reasons = []
    if not (powercap / RAPL_ZONE / ENERGY_ATTRIBUTE).exists():
        reasons.append(f"no {host.target(powercap / RAPL_ZONE / ENERGY_ATTRIBUTE)}")
    if which("sudo") is None:
        reasons.append("sudo is not on PATH, so the root-only counter cannot be read")
    if which("cargo") is None:
        reasons.append("cargo is not on PATH, so the reader cannot be built")
    value, _reason = uid()
    if value == 0:
        reasons.append("the gate runs as root, so its unprivileged read would not be unprivileged")
    model = cpu_model() if cpu is None else cpu
    if model != REFERENCE_CPU:
        reasons.append(f"the CPU is {model!r}, not the reference profile's {REFERENCE_CPU!r}")
    return reasons


def read_text(path):
    """Return a world-readable attribute's text, or None when it cannot be read."""
    try:
        return path.read_text(encoding="ascii", errors="replace")
    except OSError:
        return None


def enumerate_powercap(root=POWERCAP):
    """Return every /sys/class/powercap entry with its name, range and counter mode."""
    entries = []
    names = sorted(entry.name for entry in root.iterdir())[:MAX_POWERCAP_ENTRIES]
    for name in names:
        zone = root / name
        counter = zone / ENERGY_ATTRIBUTE
        info = counter.stat() if counter.exists() else None
        entries.append(
            {
                "entry": name,
                "name": (read_text(zone / "name") or "").strip() or None,
                "range": (read_text(zone / "max_energy_range_uj") or "").strip() or None,
                "mode": stat.S_IMODE(info.st_mode) if info else None,
                "owner": info.st_uid if info else None,
            }
        )
    return entries


def enumeration_problems(entries):
    """Return how the measured enumeration differs from the recorded one."""
    found = {entry["entry"]: entry["name"] for entry in entries}
    if found != RECORDED_ZONES:
        return [f"/sys/class/powercap/* reads {found}, not the recorded {RECORDED_ZONES}"]
    problems = []
    for entry in entries:
        if entry["name"] is None:
            continue
        if entry["range"] != str(RECORDED_RANGE_UJ):
            problems.append(f"{entry['entry']} max_energy_range_uj reads {entry['range']}")
        if entry["mode"] != ENERGY_MODE or entry["owner"] != 0:
            problems.append(
                f"{entry['entry']}/energy_uj is mode {entry['mode']!r} owner {entry['owner']!r}, "
                f"not 0400 root"
            )
    return problems


def unprivileged_read(path=POWERCAP / RAPL_ZONE / ENERGY_ATTRIBUTE, clock=time.monotonic_ns):
    """Read the counter without privilege; on the reference profile this must fail."""
    stamp = clock()
    try:
        return {"text": path.read_text(encoding="ascii"), "monotonic-ns": stamp}
    except OSError as error:
        return {"error": f"{type(error).__name__}: {error}", "monotonic-ns": stamp}


def sudo_read(runner=run, clock=time.monotonic_ns):
    """Read the counter through the one scoped sudo command, stamped at its midpoint."""
    started = clock()
    code, stdout, stderr = runner(list(SUDO_READ), SUDO_TIMEOUT)
    ended = clock()
    if code != 0 and any(message in stderr for message in SUDO_UNAVAILABLE):
        raise Unavailable(f"sudo -n is unavailable here: {stderr.strip()[:200]}")
    if code != 0:
        raise GateError(f"{' '.join(SUDO_READ)} exited {code}: {stderr.strip()[:200]}")
    return {"text": stdout, "monotonic-ns": (started + ended) // 2, "latency-ns": ended - started}


def sample_pair(reader=sudo_read, sleep=time.sleep, spacing=PAIR_SPACING):
    """Two privileged readings `spacing` seconds apart."""
    first = reader()
    sleep(spacing)
    return [first, reader()]


def wrapped(readings):
    """Return True once the last reading is below the one before it."""
    if len(readings) < 2:
        return False
    return int(readings[-1]["text"]) < int(readings[-2]["text"])


def watch_wrap(reader=sudo_read, sleep=time.sleep, spacing=WATCH_SPACING, bound=WATCH_BOUND):
    """Readings `spacing` apart until one after a wrap, or the bound, whichever first."""
    readings = [reader()]
    seen = False
    limit = min(MAX_WATCH_READS, int(bound // spacing) + 1)
    for _ in range(limit):
        sleep(spacing)
        readings.append(reader())
        if seen:
            break
        seen = wrapped(readings)
    return readings


def readings_document(zone, range_text, mode, readings, unprivileged):
    """Return the document aegis-tellus-rapl judges, without the gate's own fields."""
    keep = ("text", "error", "monotonic-ns")
    return {
        "schema": READINGS_SCHEMA,
        "zone": zone,
        "max-energy-range-uj": range_text,
        "mode": mode,
        "readings": [{k: v for k, v in read.items() if k in keep} for read in readings],
        "unprivileged": {k: v for k, v in unprivileged.items() if k in keep},
    }


def executable_from(messages, name):
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


def cargo_build(crate, binary, extra=(), extra_env=None, runner=run):
    """Build one binary with the locked graph and return its path."""
    argv = ["cargo", "build", "--locked", "-p", crate, "--bin", binary]
    argv += list(extra) + ["--message-format=json-render-diagnostics"]
    code, stdout, stderr = runner(argv, BUILD_TIMEOUT, extra_env=extra_env, cwd=ROOT)
    if code != 0:
        raise GateError(f"cargo build of {binary} exited {code}:\n{tail(stderr)}")
    executable = executable_from(stdout, binary)
    if executable is None or not executable.is_file():
        raise GateError(f"cargo reported no executable for {binary}")
    return executable


def parse_child(stdout):
    """Return (info, cases, result, notes) from a child's report lines."""
    info, cases, result, notes = {}, {}, None, {}
    current = None
    for line in stdout.splitlines()[:MAX_OUTPUT_LINES]:
        match = re.match(r"^(PASS|FAIL) (\S+)$", line)
        if match:
            current = match.group(2)
            cases[current] = match.group(1) == "PASS"
            notes[current] = []
        elif line.startswith("     ") and current is not None:
            notes[current].append(line.strip())
        elif line.startswith("info ") and ": " in line:
            key, value = line[5:].split(": ", 1)
            info[key] = value
        elif line.startswith("RESULT "):
            result = line[7:]
    return info, cases, result, notes


def case_problems(cases, required):
    """Return one problem per required case that did not report a pass."""
    problems = []
    for name in required:
        if name not in cases:
            problems.append(f"{name} did not report")
        elif not cases[name]:
            problems.append(f"{name} failed")
    return problems


def judge_child(code, stdout, required, label):
    """Return the problems of one child run: its cases, then its exit status."""
    _info, cases, result, _notes = parse_child(stdout)
    problems = case_problems(cases, required)
    if code != 0 and not problems:
        problems.append(f"{label} exited {code}: {result}")
    return problems


def gate_case(name, problems, lines):
    """Print one gate-side case and return its problems."""
    print(f"{'FAIL' if problems else 'PASS'} {name}")
    for line in (problems or lines)[:MAX_PROBLEM_LINES]:
        print(f"     {line}")
    return problems


def rapl_enumeration(retain):
    """Record the zone enumeration and the counter's mode; return problems."""
    entries = enumerate_powercap()
    (retain / "powercap.json").write_text(json.dumps(entries, indent=2) + "\n", encoding="utf-8")
    listed = ", ".join(
        f"{entry['entry']} ({entry['name']})" if entry["name"] else entry["entry"]
        for entry in entries
    )
    problems = gate_case(
        "rapl/zone-enumeration",
        enumeration_problems(entries),
        [
            f"ls -d /sys/class/powercap/*: {listed}",
            f"max_energy_range_uj {RECORDED_RANGE_UJ} for both zones; no dram and no psys zone",
            "energy_uj is mode 0400, owner root, for both zones (CVE-2020-8694 mitigation)",
        ],
    )
    return problems


def rapl_reads(observe_wrap):
    """Take the unprivileged read and the privileged ones; return (mode, reads, unprivileged)."""
    unprivileged = unprivileged_read()
    if observe_wrap:
        print(
            f"     watching the counter every {WATCH_SPACING:.0f} s for a wrap, at most "
            f"{WATCH_BOUND:.0f} s"
        )
        return "wrap-watch", watch_wrap(), unprivileged
    return "pair", sample_pair(), unprivileged


def rapl_half(retain, observe_wrap=False):
    """Run the RAPL half; return (status, problems)."""
    reasons = rapl_reasons()
    if reasons:
        return SKIP, reasons
    problems = rapl_enumeration(retain)
    try:
        mode, readings, unprivileged = rapl_reads(observe_wrap)
    except Unavailable as reason:
        return (FAIL, problems) if problems else (SKIP, [str(reason)])
    zone = POWERCAP / RAPL_ZONE
    document = readings_document(
        read_text(zone / "name") or "",
        read_text(zone / "max_energy_range_uj") or "",
        mode,
        readings,
        unprivileged,
    )
    text = json.dumps(document)
    (retain / "rapl-readings.json").write_text(json.dumps(document, indent=2) + "\n", "utf-8")
    print(
        f"     privileged reads: {len(readings)} through `{' '.join(SUDO_READ)}`; the "
        f"slowest took {max(read['latency-ns'] for read in readings)} ns"
    )
    binary = cargo_build(RAPL_CRATE, RAPL_BINARY)
    code, stdout, stderr = run([str(binary), "run", text], RAPL_TIMEOUT, cwd=ROOT)
    (retain / "rapl.log").write_text(f"exit {code}\n{stdout}\n--- stderr\n{stderr}", "utf-8")
    for line in stdout.splitlines()[:MAX_OUTPUT_LINES]:
        print(line)
    required = RAPL_CASES + ((WRAP_CASE,) if observe_wrap else ())
    problems += judge_child(code, stdout, required, RAPL_BINARY)
    return (FAIL if problems else PASS), problems


# --- The sandbox half ------------------------------------------------------


def available_mib(path=MEMINFO):
    """Return MemAvailable in MiB, or None when it cannot be read."""
    text = read_text(path) or ""
    match = re.search(r"^MemAvailable:\s+(\d+) kB$", text, re.M)
    return int(match.group(1)) // 1024 if match else None


def cached_paths(cache, pin):
    """Return where the fetch keeps the firecracker binary, the kernel and its config."""
    firecracker, kernel = pin["firecracker"], pin["guest_kernel"]
    release = cache / "firecracker" / firecracker["tag"]
    return {
        "firecracker": release / firecracker["binary"]["file"],
        "licence": release / firecracker["licence_file"]["file"],
        "kernel": cache / "kernel" / kernel["file"],
        "config": cache / "kernel" / kernel["config"]["file"],
    }


def arch_reasons(arch=None):
    """Return a reason when this host cannot run the pinned x86_64 bytes, else none."""
    arch = platform_machine() if arch is None else arch
    if arch == PINNED_ARCH:
        return []
    return [f"the pinned Firecracker and guest are {PINNED_ARCH}; this host is {arch or 'unknown'}"]


def sandbox_reasons(cache, pin, platform=None, which=shutil.which, kvm=KVM, memory=None, arch=None):
    """Return one reason per capability the sandbox half needs and this host lacks."""
    platform = sys.platform if platform is None else platform
    if not platform.startswith("linux"):
        return [f"the sandbox half needs KVM and Firecracker; this host is {platform}"]
    reasons = arch_reasons(arch)
    if not kvm.exists() or not os.access(kvm, os.R_OK | os.W_OK):
        reasons.append(f"no read-write {host.target(kvm)}")
    for name, path in cached_paths(cache, pin).items():
        if not path.exists():
            reasons.append(f"no fetched {name} at {path}; run `make workstation-fetch`")
    free = available_mib() if memory is None else memory
    if free is None or free < MIN_AVAILABLE_MIB:
        reasons.append(f"{free} MiB available; 64 guests need at least {MIN_AVAILABLE_MIB} MiB")
    if which("cargo") is None:
        reasons.append("cargo is not on PATH")
    return reasons


def cache_problems(cache, pin):
    """Return how the cached bytes differ from the pin: a FAIL, never a SKIP."""
    firecracker, kernel = pin["firecracker"], pin["guest_kernel"]
    paths = cached_paths(cache, pin)
    expected = {
        "firecracker": firecracker["binary"]["sha256"],
        "licence": firecracker["licence_file"]["sha256"],
        "kernel": kernel["sha256"],
        "config": kernel["config"]["sha256"],
    }
    problems = []
    for name, digest in expected.items():
        actual = file_sha256(paths[name])
        if actual != digest:
            problems.append(f"the cached {name} hashes to {actual}, not the pinned {digest}")
    config = read_text(paths["config"]) or ""
    for line in kernel["required_config"]:
        if line not in config.splitlines():
            problems.append(f"the pinned kernel's configuration lacks {line}")
    return problems


def firecracker_version(binary, pin, runner=run):
    """Return the version line the pinned binary prints, refusing any other."""
    code, stdout, stderr = runner([str(binary), "--version"], VERSION_TIMEOUT)
    line = (stdout.strip().splitlines() or [""])[0]
    if code != 0 or line != pin["firecracker"]["binary"]["version_output"]:
        raise GateError(f"{binary.name} --version printed {line!r} (exit {code}): {stderr[:200]}")
    return line


def elf_interpreter(data):
    """Return (static, reason): an ELF64 with no PT_INTERP program header is static."""
    if len(data) < 64 or data[:4] != b"\x7fELF" or data[4] != 2 or data[5] != 1:
        return False, "not a little-endian ELF64 file"
    offset = int.from_bytes(data[32:40], "little")
    size = int.from_bytes(data[54:56], "little")
    count = int.from_bytes(data[56:58], "little")
    if count > MAX_ELF_HEADERS or size < 4:
        return False, f"{count} program headers of {size} bytes is outside the bound"
    for index in range(count):
        start = offset + index * size
        header = data[start : start + 4]
        if len(header) < 4:
            return False, "a program header runs past the end of the file"
        if int.from_bytes(header, "little") == 3:
            return False, "it names a program interpreter (PT_INTERP), so it is dynamic"
    return True, None


def newc_entry(inode, name, mode, data):
    """Return one newc cpio member: header, NUL-terminated name and data, 4-aligned."""
    encoded = name.encode("ascii") + b"\0"
    fields = (inode, mode, 0, 0, 1, 0, len(data), 0, 0, 0, 0, len(encoded), 0)
    header = b"070701" + "".join(f"{value:08x}" for value in fields).encode("ascii")
    member = header + encoded
    member += b"\0" * (-len(member) % 4)
    return member + data + b"\0" * (-len(data) % 4)


def newc_archive(entries):
    """Return a deterministic newc archive of (name, mode, data) entries and its trailer."""
    archive = b""
    for inode, (name, mode, data) in enumerate(entries, start=1):
        if name.startswith("/") or ".." in name.split("/") or not name or len(name) > 255:
            raise GateError(f"refused an initramfs member name {name!r}")
        archive += newc_entry(inode, name, mode, data)
    return archive + newc_entry(len(entries) + 1, "TRAILER!!!", 0, b"")


def build_initramfs(guest, target):
    """Pack the static guest init as /init, with /dev, and return the archive's sha256."""
    data = guest.read_bytes()
    static, reason = elf_interpreter(data)
    if not static:
        raise GateError(f"{guest.name} is not a static executable: {reason}")
    archive = newc_archive([("dev", 0o040755, b""), ("init", 0o100755, data)])
    target.write_bytes(archive)
    return hashlib.sha256(archive).hexdigest()


def net_devices(root=NET):
    """Return the host's network device names, bounded."""
    if not root.is_dir():
        return []
    return sorted(entry.name for entry in root.iterdir())[:MAX_NET_ENTRIES]


def started_by_run(command, marker):
    """Return True when a /proc cmdline is a Firecracker or sandbox host naming `marker`.

    Only what the run starts qualifies: a shell, `tail` or `less` that merely
    names the run directory is not the run's and is never killed.
    """
    program = PurePosixPath(command.split(b"\0", 1)[0].decode("utf-8", "replace")).name
    return program in (FIRECRACKER_NAME, HOST_BINARY) and marker.encode() in command


def leftover_processes(marker, proc=PROC):
    """Return the pids of Firecracker or sandbox-host processes naming `marker`."""
    found = []
    entries = [entry for entry in proc.iterdir() if entry.name.isdigit()][:MAX_PROC_ENTRIES]
    for entry in entries:
        try:
            command = (entry / "cmdline").read_bytes()
        except OSError:
            continue
        if started_by_run(command, marker):
            found.append(int(entry.name))
    return found


def leftover_sockets(root):
    """Return every socket file under `root`, bounded."""
    sockets = []
    for count, path in enumerate(root.rglob("*")):
        if count >= MAX_WALK_ENTRIES:
            break
        if path.is_socket():
            sockets.append(path)
    return sockets


def run_session(argv, timeout, cwd=None):
    """Run `argv` in its own session; past the deadline the whole session is killed."""
    check_argv(argv)
    try:
        child = subprocess.Popen(
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            text=True,
            errors="replace",
            env=child_env(),
            cwd=cwd,
            start_new_session=True,
        )
    except OSError as error:
        raise GateError(f"{argv[0]} could not be run: {error}") from error
    try:
        stdout, stderr = child.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        host.end_session(child.pid)
        child.kill()
        child.communicate()
        raise GateError(f"{Path(argv[0]).name} exceeded its {timeout}s deadline; session killed")
    return child.returncode, stdout, stderr


def kill(pid):
    """Send SIGKILL to one process; a host without SIGKILL, or a gone pid, is no error."""
    sigkill = getattr(signal, "SIGKILL", None)
    if sigkill is None:
        return
    try:
        os.kill(pid, sigkill)
    except ProcessLookupError:
        return


def cleanup_problems(run_dir, before):
    """Return what the run left behind: processes, sockets, network devices."""
    problems = []
    for pid in leftover_processes(str(run_dir)):
        problems.append(
            f"process {pid} ({FIRECRACKER_NAME} or {HOST_BINARY}) outlived the run; killed"
        )
        kill(pid)
    problems += [f"socket {path} was left behind" for path in leftover_sockets(run_dir)]
    after = net_devices()
    if after != before:
        problems.append(f"network devices changed from {before} to {after}")
    return problems


def sandbox_prepare(cache, pin, run_dir):
    """Check the cache, build both binaries and the initramfs; return the launch paths."""
    problems = cache_problems(cache, pin)
    if problems:
        raise GateError("; ".join(problems[:MAX_PROBLEM_LINES]))
    paths = cached_paths(cache, pin)
    print(
        f"     firecracker: {firecracker_version(paths['firecracker'], pin)}, sha256 "
        f"{pin['firecracker']['binary']['sha256']} (pinned; jailer not used)"
    )
    print(
        f"     guest kernel: {pin['guest_kernel']['file']}, sha256 "
        f"{pin['guest_kernel']['sha256']} (pinned, {pin['guest_kernel']['prefix']})"
    )
    host_binary = cargo_build(SANDBOX_CRATE, HOST_BINARY)
    guest = cargo_build(
        SANDBOX_CRATE,
        GUEST_BINARY,
        extra=("--release", "--target", GUEST_TARGET),
        extra_env={"RUSTFLAGS": GUEST_RUSTFLAGS},
    )
    initrd = run_dir / "initramfs.cpio"
    digest = build_initramfs(guest, initrd)
    print(f"     initramfs: /init is {GUEST_BINARY}, static ({GUEST_RUSTFLAGS}); sha256 {digest}")
    return host_binary, paths, initrd


def sandbox_half(cache, run_dir):
    """Run the sandbox half; return (status, problems)."""
    pin = load_pin()
    reasons = sandbox_reasons(cache, pin)
    if reasons:
        return SKIP, reasons
    host_binary, paths, initrd = sandbox_prepare(cache, pin, run_dir)
    vms = run_dir / "vms"
    vms.mkdir()
    before = net_devices()
    argv = [
        str(host_binary),
        "run",
        str(paths["firecracker"]),
        str(paths["kernel"]),
        str(initrd),
        str(vms),
    ]
    code, stdout, stderr = run_session(argv, SANDBOX_TIMEOUT, cwd=ROOT)
    (run_dir / "sandbox.log").write_text(f"exit {code}\n{stdout}\n--- stderr\n{stderr}", "utf-8")
    for line in stdout.splitlines()[:MAX_OUTPUT_LINES]:
        print(line)
    problems = judge_child(code, stdout, SANDBOX_CASES, HOST_BINARY)
    problems += gate_case(
        "sandbox/nothing-left-behind",
        cleanup_problems(run_dir, before),
        [
            f"no Firecracker or {HOST_BINARY} process names {run_dir}; no socket file under it",
            f"network devices unchanged: {len(before)} before and after; no tap device",
        ],
    )
    return (FAIL if problems else PASS), problems


# --- The fetch -------------------------------------------------------------


def download(url, target, size, digest, runner=run):
    """Download `url` to `target` unless it is cached as pinned; refuse any other bytes."""
    if target.exists() and target.stat().st_size == size and file_sha256(target) == digest:
        return f"{target.name}: cached, as pinned"
    partial = target.with_name(f"{target.name}.partial")
    partial.unlink(missing_ok=True)
    argv = [
        "curl",
        "--fail",
        "--location",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--max-time",
        str(DOWNLOAD_TIMEOUT),
        "--output",
        str(partial),
        url,
    ]
    code, _stdout, stderr = runner(argv, DOWNLOAD_TIMEOUT + VERSION_TIMEOUT)
    if code != 0:
        raise GateError(f"curl exited {code} for {url}: {tail(stderr)}")
    actual = file_sha256(partial)
    if partial.stat().st_size != size or actual != digest:
        partial.unlink(missing_ok=True)
        raise GateError(f"refused {url}: sha256 {actual}, not the pinned {digest}")
    partial.replace(target)
    return f"{target.name}: downloaded, {size} bytes, sha256 as pinned"


def published_digest(url, target, runner=run):
    """Return the digest the release's .sha256.txt publishes for the archive."""
    argv = [
        "curl",
        "--fail",
        "--location",
        "--silent",
        "--show-error",
        "--proto",
        "=https",
        "--max-time",
        str(VERSION_TIMEOUT),
        "--output",
        str(target),
        url,
    ]
    code, _stdout, stderr = runner(argv, VERSION_TIMEOUT * 2)
    if code != 0:
        raise GateError(f"curl exited {code} for {url}: {tail(stderr)}")
    words = (read_text(target) or "").split()
    return words[0] if words else ""


def extract_member(archive, member, target, digest):
    """Extract one regular member of the release archive and check its digest."""
    with tarfile.open(archive, "r:gz") as bundle:
        info = bundle.getmember(member)
        if not info.isfile():
            raise GateError(f"{member} is not a regular file in the archive")
        source = bundle.extractfile(info)
        if source is None:
            raise GateError(f"{member} could not be read from the archive")
        target.write_bytes(source.read())
    actual = file_sha256(target)
    if actual != digest:
        target.unlink(missing_ok=True)
        raise GateError(f"{member} hashes to {actual}, not the pinned {digest}")
    return f"{target.name}: extracted, sha256 as pinned"


def fetch(cache):
    """The one networked step: download, check against the pin and the published digest."""
    if shutil.which("curl") is None:
        print("SKIP: curl is not on PATH; the workstation fetch did not run.")
        return 0
    unfit = arch_reasons()
    if unfit:
        print(f"SKIP: {unfit[0]}; the workstation fetch did not run.")
        return 0
    pin = load_pin()
    firecracker, kernel = pin["firecracker"], pin["guest_kernel"]
    paths = cached_paths(cache, pin)
    for path in paths.values():
        path.parent.mkdir(parents=True, exist_ok=True)
    archive = paths["firecracker"].parent / firecracker["archive"]["file"]
    lines = [
        download(
            firecracker["archive"]["url"],
            archive,
            firecracker["archive"]["size"],
            firecracker["archive"]["sha256"],
        )
    ]
    published = published_digest(
        firecracker["archive"]["checksum_url"], archive.with_name(archive.name + ".sha256.txt")
    )
    if published != firecracker["archive"]["sha256"]:
        raise GateError(f"the release publishes {published!r} for the archive, not the pin")
    lines.append(f"{archive.name}: the release's .sha256.txt names the pinned digest")
    for key, row in (
        ("firecracker", firecracker["binary"]),
        ("licence", firecracker["licence_file"]),
    ):
        lines.append(extract_member(archive, row["member"], paths[key], row["sha256"]))
    paths["firecracker"].chmod(0o755)
    lines.append(download(kernel["url"], paths["kernel"], kernel["size"], kernel["sha256"]))
    config = kernel["config"]
    lines.append(download(config["url"], paths["config"], config["size"], config["sha256"]))
    for line in lines:
        print(f"     {line}")
    print(
        f"PASS: Firecracker {firecracker['version']} and guest kernel {kernel['release']} "
        f"fetched into {cache}."
    )
    return 0


# --- The run ---------------------------------------------------------------


def report(label, status, problems):
    """Print one half's outcome; return True when it failed."""
    if status == SKIP:
        for reason in problems[:MAX_PROBLEM_LINES]:
            print(f"SKIP: {reason}; the {label} did not run.")
        return False
    for problem in problems[:MAX_PROBLEM_LINES]:
        print(f"     {problem}")
    counted = f": {len(problems)} problem(s)" if problems else ""
    print(f"{status.upper()}: the {label}{counted}.")
    return status == FAIL


def run_halves(cache, run_dir, observe_wrap):
    """Run both halves; return their (label, status, problems) rows."""
    rows = []
    for label, half in (
        ("RAPL half", lambda: rapl_half(run_dir, observe_wrap)),
        ("sandbox half", lambda: sandbox_half(cache, run_dir)),
    ):
        print(f"--- {label}")
        try:
            status, problems = half()
        except (GateError, OSError, ValueError, KeyError) as error:
            status, problems = FAIL, [f"the gate could not run: {error}"]
        rows.append((label, status, problems))
    return rows


class Tee:
    """A text stream that writes to the terminal and to the run's gate.log at once."""

    def __init__(self, terminal, log):
        self.terminal, self.log = terminal, log

    def write(self, text):
        """Write `text` to both streams."""
        self.terminal.write(text)
        self.log.write(text)
        return len(text)

    def flush(self):
        """Flush both streams."""
        self.terminal.flush()
        self.log.flush()


def record_host(run_dir, identifier):
    """Write host.json beside the readings and print it: criterion 5's record."""
    facts = dict(host_facts(), rustc=rustc_version())
    document = {"schema": HOST_SCHEMA, "run": identifier, **facts}
    (run_dir / "host.json").write_text(json.dumps(document, indent=2) + "\n", "utf-8")
    print(f"     run {identifier}; logs kept in {run_dir}")
    print(f"     host: {facts['cpu']}; kernel {facts['kernel']} (kept in host.json)")
    print(f"     rustc: {facts['rustc']}")


def run_gate(cache, observe_wrap):
    """Run both halves with every printed line kept in the run's gate.log."""
    identifier = run_id()
    run_dir = cache / "runs" / identifier
    run_dir.mkdir(parents=True, exist_ok=True)
    with (run_dir / "gate.log").open("w", encoding="utf-8") as log:
        with redirect_stdout(Tee(sys.stdout, log)):
            return run_recorded(cache, run_dir, identifier, observe_wrap)


def main(argv=None):
    """Run the gate; exit 0 on a pass or a skip, 1 on a failure."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--fetch", action="store_true", help="download the pinned bytes")
    parser.add_argument("--observe-wrap", action="store_true", help="watch for a counter wrap")
    options = parser.parse_args(argv)
    cache = cache_dir()
    if options.fetch:
        try:
            return fetch(cache)
        except (GateError, OSError, KeyError, tarfile.TarError) as error:
            print(f"FAIL: the workstation fetch could not complete: {error}")
            return 1
    return run_gate(cache, options.observe_wrap)


def run_recorded(cache, run_dir, identifier, observe_wrap):
    """Record the host, run both halves and print the verdict; return the exit code."""
    print("Workstation hardware slices gate (M21; development evidence on the reference profile).")
    record_host(run_dir, identifier)
    rows = run_halves(cache, run_dir, observe_wrap)
    failed = [label for label, status, problems in rows if report(label, status, problems)]
    ran = [label for label, status, _problems in rows if status != SKIP]
    if failed:
        print(f"FAIL: {', '.join(failed)} did not pass.")
        return 1
    print(
        f"PASS: {', '.join(ran) or 'nothing'} ran and passed on the reference profile (M21). "
        "Development evidence only: no hardware qualified, no hardware, isolation or release "
        "gate closed; boot time and footprint are M22's (D71)."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
