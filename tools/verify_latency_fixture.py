#!/usr/bin/env python3
"""Measure wakeup latency on the M26 realtime guest and on the reference host.

Milestone M23. Decision D70 keeps the guest kernel an interim source for this
fixture: it is the kernel `make verify-kernel` builds while Nucleus is a
scaffold, and the kernel the product ships is built by Nucleus against the M18
schema once Nucleus is real. Decision D57 asks for the realtime kernel to be
obtained without modifying the reference host; M26's kernel satisfies that by
construction, because it is a file in a build directory that nothing installs,
packages or boots. No distribution realtime package is downloaded here, and
none is installed.

Six cases, no simulation and no failure suppression:

* ``latency/guest-preempt-rt`` boots M26's image and requires the guest to
  report the recorded release and ``CONFIG_PREEMPT_RT=y`` from its own
  ``/proc/config.gz``;
* ``latency/host-not-preempt-rt`` runs the same probe script on the reference
  host and requires ``# CONFIG_PREEMPT_RT is not set``;
* ``latency/host-unmodified`` records what D57 can be checked against without
  root on this profile, and says what it does not check;
* ``latency/guest-measured`` runs ``cyclictest`` inside the guest and reports
  the worst case against every P07 tier edge and the P08 target;
* ``latency/host-not-satisfying`` runs the identical argument vector on the
  host and requires every verdict to be ``kernel-not-realtime``, whatever the
  figure -- including when the host figure is the smaller of the two;
* ``latency/boundary-edge`` reports a worst case one nanosecond below a
  threshold, exactly on it, and one nanosecond past it as three different
  outcomes.

Nothing is built inside the repository. The guest tree and the retained JSON
live under ``AEGIS_LATENCY_BUILD_DIR`` (default
``${XDG_CACHE_HOME:-$HOME/.cache}/aegis-latency``); the kernel image comes from
``AEGIS_KERNEL_BUILD_DIR`` (default ``.../aegis-kernel``), which M26 produced.
"""

import argparse
import json
import os
import re
import secrets
import shutil
import subprocess
import sys
from pathlib import Path

from host import kernel_release

ROOT = Path(__file__).resolve().parent.parent
PROBE_SCRIPT = ROOT / "tools" / "guest" / "aegis-preempt-probe.sh"
GUEST_INIT = ROOT / "tools" / "guest" / "aegis-latency-init.sh"
TIER_SOURCE = ROOT / "crates" / "aegis-lictor" / "src" / "tier.rs"
MEASURED_SOURCE = ROOT / "crates" / "aegis-calliope" / "src" / "measured.rs"
HOST_CONFIG = Path("/proc/config.gz")
HOST_MODULES = Path("/lib/modules")

# Deadlines. Every external command carries one, so a wedged emulator, a
# cyclictest that never returns or a guest that never powers off fails the gate
# instead of hanging it.
VERSION_TIMEOUT = 30
ARCHIVE_TIMEOUT = 300
PROBE_TIMEOUT = 60
PACMAN_TIMEOUT = 120
# The measurement is 50000 loops at a 200 microsecond interval, which is ten
# seconds of wall clock. The deadlines are the run plus boot and drain, wide
# enough that a slow machine does not fail and narrow enough that a wedge does.
MEASURE_TIMEOUT = 300
BOOT_TIMEOUT = 600

# Scalar bounds.
MAX_GUEST_LINES = 20000
MAX_GUEST_LIBRARIES = 64
MAX_PROBLEM_LINES = 24
MAX_MODULE_DIRECTORIES = 64
MAX_DIAGNOSTIC_LINES = 20

GUEST_MARK = "AEGIS-M23"
GUEST_MEMORY = "2048"
GUEST_CPUS = "2"
NONCE_BYTES = 16
# The programs the guest calls, plus the shell that runs its init. Their
# shared-library closure is resolved with ldd and copied beside them: the
# userspace in the guest is this host's own, and only the kernel under test is
# the one being measured.
GUEST_PROGRAMS = ("bash", "mount", "uname", "gzip", "grep", "cat", "sleep", "cyclictest")

# The one measurement argument vector. The guest runs it from a script the gate
# writes, the host runs it directly, and both runs record their own arguments in
# their JSON output so the gate can compare what actually ran rather than assert
# that it matched.
#
# --default-system is load-bearing rather than tuning: without it cyclictest
# writes a power-management latency target into /dev/cpu_dma_latency, which
# would be a change to the reference host (D57). With it the tool prints
# "WARN: not setting cpu_dma_latency from cyclictest" and changes nothing on
# either machine.
#
# --priority=95 is REQ-P08-02's RLIMIT_RTPRIO, the priority the P08 report calls
# non-negotiable for the audio threads, rather than cyclictest's customary 99.
CYCLICTEST_ARGUMENTS = (
    "--default-system",
    "--mlockall",
    "--priority=95",
    "--interval=200",
    "--distance=0",
    "--threads=1",
    "--affinity=1",
    "--loops=50000",
    "--nsecs",
    "--quiet",
)
GUEST_JSON = "/aegis-latency.json"

# Toolchain admission. Every row is a tool this gate runs, read back from the
# tool before anything is measured. A floor of None is a tool for which no
# source declares one; its reference value is recorded so the admission is a pin
# rather than whatever the workstation ships.
TOOLCHAIN = (
    (
        "qemu-system-x86_64",
        ["qemu-system-x86_64", "--version"],
        r"QEMU emulator version (\d+(?:\.\d+)*)",
        None,
        "11.1.1",
    ),
    ("cyclictest", ["cyclictest", "--help"], r"cyclictest V (\d+(?:\.\d+)*)", "2.10", "2.10"),
    ("cpio", ["cpio", "--version"], r"cpio \(GNU cpio\) (\d+(?:\.\d+)*)", None, "2.15"),
    ("ldd", ["ldd", "--version"], r"ldd \(GNU libc\) (\d+(?:\.\d+)*)", None, "2.44"),
    ("bash", ["bash", "--version"], r"version (\d+(?:\.\d+)*)", "4.2", "5.3.15"),
    ("mount", ["mount", "--version"], r"util-linux (\d+(?:\.\d+)*)", "2.10", "2.42.3"),
    ("coreutils", ["cat", "--version"], r"cat \(GNU coreutils\) (\d+(?:\.\d+)*)", None, "9.11"),
    ("grep", ["grep", "--version"], r"grep \(GNU grep\) (\d+(?:\.\d+)*)", None, "3.12"),
    ("gzip", ["gzip", "--version"], r"gzip (\d+(?:\.\d+)*)", None, "1.14"),
    ("pacman", ["pacman", "--version"], r"Pacman v(\d+(?:\.\d+)*)", None, "7.1.0"),
)


class GateError(Exception):
    """The gate could not be run at all, as opposed to a case that failed."""


def run(argv, timeout, cwd=None, stdin=None):
    """Run `argv` under a hard deadline and return (exit code, stdout, stderr)."""
    env = dict(os.environ)
    env["LC_ALL"] = "C"
    env["LANG"] = "C"
    env["NO_COLOR"] = "1"
    try:
        done = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            env=env,
            cwd=cwd,
            stdin=stdin,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise GateError(f"{argv[0]} exceeded its {timeout}s deadline") from error
    except OSError as error:
        raise GateError(f"{argv[0]} could not be run: {error}") from error
    return done.returncode, done.stdout, done.stderr


def version_tuple(text):
    """Return a dotted version as a tuple of integers, for ordering."""
    return tuple(int(part) for part in text.split("."))


def read_version(row):
    """Return (banner, version string) for one toolchain row, or (reason, None)."""
    name, argv, pattern, _floor, _reference = row
    if shutil.which(argv[0]) is None:
        return f"{name} is not on PATH", None
    code, stdout, stderr = run(argv, VERSION_TIMEOUT)
    if code != 0:
        return f"{' '.join(argv)} exited {code}", None
    match = re.search(pattern, stdout + stderr)
    if match is None:
        return f"no version matched {pattern!r} in the output of {' '.join(argv)}", None
    return match.group(0).strip(), match.group(1)


def check_toolchain():
    """Print the admitted toolchain read back from the host; return the reasons it cannot run."""
    reasons = []
    for row in TOOLCHAIN:
        name, _argv, _pattern, floor, reference = row
        banner, found = read_version(row)
        if found is None:
            reasons.append(banner)
            continue
        limit = "" if floor is None else f", floor {floor}"
        note = "" if found == reference else f" [reference profile recorded {reference}]"
        print(f"     {name}: {found} (read back from {banner!r}{limit}){note}")
        if floor is not None and version_tuple(found) < version_tuple(floor):
            reasons.append(f"{name} {found} is below the admitted floor {floor}")
    return reasons


def cache_dir(variable, name):
    """Return an out-of-repository directory, overridable by `variable`."""
    override = os.environ.get(variable)
    if override:
        return Path(override).expanduser()
    cache = os.environ.get("XDG_CACHE_HOME")
    root = Path(cache).expanduser() if cache else Path.home() / ".cache"
    return root / name


def kernel_artifacts():
    """Return (bzImage, release) for the kernel M26 built, or a reason it is absent."""
    base = cache_dir("AEGIS_KERNEL_BUILD_DIR", "aegis-kernel") / "out" / "positive"
    image = base / "arch" / "x86" / "boot" / "bzImage"
    release_file = base / "include" / "config" / "kernel.release"
    if not image.is_file() or not release_file.is_file():
        return None, f"{image} does not exist; run `make verify-kernel` first"
    return (image, release_file.read_text(encoding="utf-8").strip()), None


def rust_u64(source, pattern, label):
    """Return one integer literal read out of a Rust source file."""
    match = re.search(pattern, source.read_text(encoding="utf-8"))
    if match is None:
        raise GateError(f"{label} was not found in {source.relative_to(ROOT)}")
    return int(match.group(1).replace("_", ""))


def thresholds():
    """Return the tier edges, read out of the crates rather than restated here.

    The P07 edges are `aegis-lictor`'s own constants and the P08 target is
    `aegis-calliope`'s, so the gate and the crates cannot drift apart: an edge
    changed in one place fails here rather than being evaluated against a stale
    copy. `tools/test_latency_fixture.py` pins each value to a literal.
    """
    rows = []
    for name in ("BURST_CRITICAL_NS", "BURST_INTERACTIVE_NS", "BURST_FRAME_NS"):
        pattern = rf"pub const {name}: u64 = ([0-9_]+);"
        rows.append((name, rust_u64(TIER_SOURCE, pattern, name)))
    target = rust_u64(
        MEASURED_SOURCE,
        r"pub const TARGET_RTL_LATENCY_NS: Declared<u64> = Declared::new\(([0-9_]+)\);",
        "TARGET_RTL_LATENCY_NS",
    )
    rows.append(("TARGET_RTL_LATENCY_NS", target))
    return rows


def recorded_release(constant):
    """Return the release string one recorded KernelIdentity carries."""
    pattern = rf'pub const {constant}: KernelIdentity =\s*KernelIdentity::new\("([^"]+)"'
    match = re.search(pattern, MEASURED_SOURCE.read_text(encoding="utf-8"))
    if match is None:
        raise GateError(f"{constant} was not found in {MEASURED_SOURCE.relative_to(ROOT)}")
    return match.group(1)


def attainment(threshold_ns, worst_ns):
    """Place a worst case against a threshold under the crate's strict rule."""
    if worst_ns < threshold_ns:
        return "attained"
    if worst_ns == threshold_ns:
        return "at-edge"
    return "exceeded"


def reports_realtime(version):
    """Read PREEMPT_RT out of one kernel's own version string.

    The argument is the `uname -v` line the measuring kernel wrote into its own
    cyclictest JSON, so the answer is that machine's reading of itself rather
    than a flag asserted at a call site. Every `placement_rows` call is fed the
    value this returns, which is the shape `Measured::verdict` has in
    `aegis-calliope`: the `KernelIdentity` travels with the figure, so the rows
    a case prints and the outcome it returns cannot rest on different readings.
    """
    return "PREEMPT_RT" in version


def verdict(is_realtime, placed):
    """Decide a determinism verdict, kernel first.

    The order is the rule, and it is the same order
    `DeterminismVerdict::of` uses in `aegis-calliope`: a figure from a kernel
    without CONFIG_PREEMPT_RT never reaches the numeric comparison, so it can
    never be reported as a better or worse number than a realtime one.
    """
    if not is_realtime:
        return "kernel-not-realtime"
    return {
        "attained": "satisfied",
        "at-edge": "worst-case-at-edge",
        "exceeded": "worst-case-exceeds",
    }[placed]


def library_closure(program):
    """Return the shared objects `program` loads, read back from ldd."""
    code, stdout, _stderr = run(["ldd", program], VERSION_TIMEOUT)
    if code != 0:
        raise GateError(f"ldd could not resolve {program}")
    return sorted(set(re.findall(r"(/[^\s]+\.so[^\s]*)", stdout)))[:MAX_GUEST_LIBRARIES]


def populate_guest(tree, run_script):
    """Fill an initramfs tree with the host userspace the guest calls."""
    for name in ("usr/bin", "usr/lib", "proc", "dev", "sys"):
        (tree / name).mkdir(parents=True, exist_ok=True)
    for name in ("bin", "lib", "lib64"):
        (tree / name).symlink_to(f"usr/{'bin' if name == 'bin' else 'lib'}")
    libraries = set()
    for name in GUEST_PROGRAMS:
        program = shutil.which(name)
        if program is None:
            raise GateError(f"the guest needs {name}, which is not on PATH")
        copied = tree / "usr" / "bin" / name
        shutil.copy2(program, copied)
        # The host ships mount set-uid. Copied into an archive owned by the
        # developer account that bit would drop PID 1 from root to the
        # developer uid, and mount(8) would refuse its own mount.
        copied.chmod(0o755)
        libraries.update(library_closure(program))
    (tree / "usr" / "bin" / "sh").symlink_to("bash")
    for library in sorted(libraries)[:MAX_GUEST_LIBRARIES]:
        shutil.copy2(library, tree / "usr" / "lib" / Path(library).name)
    for source, name in ((GUEST_INIT, "init"), (PROBE_SCRIPT, PROBE_SCRIPT.name)):
        target = tree / name
        shutil.copy2(source, target)
        target.chmod(0o755)
    script = tree / "aegis-latency-run.sh"
    script.write_text(run_script, encoding="utf-8")
    script.chmod(0o755)


def measurement_script():
    """Return the guest's runner, holding the one shared argument vector."""
    arguments = " ".join(CYCLICTEST_ARGUMENTS)
    return (
        "#!/bin/sh\n"
        "# Written by tools/verify_latency_fixture.py from CYCLICTEST_ARGUMENTS.\n"
        "set -eu\n"
        f"exec /bin/cyclictest {arguments} --json={GUEST_JSON}\n"
    )


def build_initramfs(base):
    """Assemble the latency guest's initramfs and return the archive path."""
    tree = base / "guest" / "root"
    if tree.exists():
        shutil.rmtree(tree)
    tree.mkdir(parents=True)
    populate_guest(tree, measurement_script())
    archive = base / "guest" / "initramfs.cpio"
    names = "\n".join(
        sorted(str(path.relative_to(tree)) for path in tree.rglob("*"))[:MAX_GUEST_LINES]
    )
    with archive.open("wb") as handle:
        done = subprocess.run(
            ["cpio", "--quiet", "--create", "--format=newc", "--owner=0:0"],
            input=names.encode(),
            stdout=handle,
            stderr=subprocess.PIPE,
            cwd=tree,
            timeout=ARCHIVE_TIMEOUT,
            check=False,
        )
    if done.returncode != 0:
        raise GateError(f"the initramfs did not archive: {done.stderr.decode(errors='replace')}")
    return archive


def boot(image, initramfs, base, nonce):
    """Boot the kernel under test and return what the guest reported.

    The report file is removed before the emulator starts and the guest is
    handed a per-run nonce on its own command line, so a file left by an
    earlier run, or one written by hand, is not accepted as evidence for this
    one. ``-cpu max`` is not decoration: the guest userspace is this host's own
    glibc, compiled for a recent x86-64 feature level that the emulator's
    default model does not have.
    """
    report_file = base / "guest" / "report.txt"
    console = base / "guest" / "console.log"
    report_file.unlink(missing_ok=True)
    argv = [
        "qemu-system-x86_64",
        "-display",
        "none",
        "-no-reboot",
        "-cpu",
        "max",
        "-m",
        GUEST_MEMORY,
        "-smp",
        GUEST_CPUS,
        "-serial",
        "stdio",
        "-serial",
        f"file:{report_file}",
        "-kernel",
        str(image),
        "-initrd",
        str(initramfs),
        "-append",
        f"console=ttyS0 panic=5 rdinit=/init aegis.nonce={nonce}",
    ]
    if os.access("/dev/kvm", os.R_OK | os.W_OK):
        argv.insert(1, "-enable-kvm")
    code, stdout, stderr = run(argv, BOOT_TIMEOUT, stdin=subprocess.DEVNULL)
    console.write_text((stdout + stderr).replace("\r\n", "\n").replace("\r", "\n"))
    if code != 0:
        raise GateError(f"the guest exited {code}; its console is in {console}")
    if not report_file.exists():
        raise GateError(f"the guest wrote no report; its console is in {console}")
    return report_file.read_text(errors="replace").replace("\r\n", "\n").replace("\r", "\n")


def guest_field(report, name):
    """Return one `AEGIS-M23-<name> <value>` line's value, as the guest printed it."""
    prefix = f"{GUEST_MARK}-{name} "
    for line in report.splitlines()[:MAX_GUEST_LINES]:
        if line.startswith(prefix):
            return line[len(prefix) :].strip()
    raise GateError(f"the guest printed no {name} line")


def guest_section(report, name):
    """Return the text the guest printed between one pair of markers."""
    start = f"{GUEST_MARK}-{name}-BEGIN"
    end = f"{GUEST_MARK}-{name}-END"
    lines = report.splitlines()[:MAX_GUEST_LINES]
    if start not in lines or end not in lines:
        raise GateError(f"the guest printed no {name} section; it did not reach that stage")
    return "\n".join(lines[lines.index(start) + 1 : lines.index(end)])


def parse_report(text):
    """Return the figures and the argument vector one cyclictest JSON carries.

    The machine name cyclictest records in ``sysinfo.nodename`` is dropped
    here and never reaches the repository: `planning/hardware-profile.json`
    forbids recording machine identifiers, and this gate writes its findings
    into evidence that is read back by tests.
    """
    try:
        payload = json.loads(text)
    except json.JSONDecodeError as error:
        raise GateError(f"the measurement produced no readable JSON: {error}") from error
    thread = payload.get("thread", {}).get("0")
    info = payload.get("sysinfo", {})
    if thread is None or not info:
        raise GateError("the measurement JSON carries no thread 0 or no sysinfo")
    if payload.get("resolution_in_ns") != 1:
        raise GateError("the measurement JSON is not in nanoseconds; --nsecs did not apply")
    return {
        "release": info.get("release", ""),
        "version": info.get("version", ""),
        "cycles": thread.get("cycles", 0),
        "min": int(thread.get("min", 0)),
        "avg": float(thread.get("avg", 0.0)),
        "max": int(thread.get("max", 0)),
        "arguments": payload.get("cmdline:", ""),
    }


def comparable_arguments(recorded):
    """Return one recorded argument vector without its program path or JSON path."""
    parts = recorded.split()[1:]
    return [part for part in parts if not part.startswith("--json=")]


def host_measurement(base):
    """Run the identical argument vector on the reference host."""
    if shutil.which("cyclictest") is None:
        raise GateError("cyclictest is not on PATH")
    target = base / "host-latency.json"
    target.unlink(missing_ok=True)
    argv = ["cyclictest", *CYCLICTEST_ARGUMENTS, f"--json={target}"]
    code, _stdout, stderr = run(argv, MEASURE_TIMEOUT)
    if code != 0 or not target.is_file():
        tail = "\n".join(stderr.splitlines()[-MAX_DIAGNOSTIC_LINES:])
        raise GateError(f"the host measurement exited {code}:\n{tail}")
    return parse_report(target.read_text(encoding="utf-8"))


def host_probe():
    """Run the guest's own probe script against the host's running kernel.

    The script is the same file the guest executed and the interpreter is the
    same program: the guest's ``/bin/sh`` is a copy of this host's ``bash``,
    placed there by ``populate_guest``, so naming ``bash`` here rather than
    ``sh`` makes both halves of the probe identical instead of only the script.
    The resolved path is reported beside the reading.
    """
    shell = shutil.which("bash")
    if shell is None:
        raise GateError("bash is not on PATH")
    code, stdout, stderr = run(["bash", str(PROBE_SCRIPT)], PROBE_TIMEOUT)
    return code, (stdout + stderr).strip(), os.path.realpath(shell)


def report(name, problems, notes=()):
    """Print one case's outcome and, when it failed, why."""
    print(f"{'PASS' if not problems else 'FAIL'} {name}")
    for note in notes:
        print(f"     {note}")
    for line in problems[:MAX_PROBLEM_LINES]:
        print(f"     {line}")


def guest_identity_case(report_text, nonce, release):
    """Case 1: the guest is the kernel M26 built, and it says so about itself."""
    problems = []
    reported_nonce = guest_field(report_text, "NONCE")
    if reported_nonce != nonce:
        problems.append(
            f"the report carries nonce {reported_nonce!r}, not this run's {nonce!r}; "
            "it was not written by this boot and proves nothing about it"
        )
    reported = guest_field(report_text, "UNAME-R")
    recorded = recorded_release("AEGIS_M26_GUEST")
    host, absent = kernel_release()
    if host is None:
        print(f"SKIP latency/guest-preempt-rt: {absent}")
        return []
    if reported != release:
        problems.append(f"the guest reports {reported!r}, not the built release {release!r}")
    if reported != recorded:
        problems.append(f"the guest reports {reported!r}; the crate records {recorded!r}")
    if reported == host:
        problems.append(f"the guest reports the host's own release {host!r}")
    line = guest_section(report_text, "PROBE").strip()
    if line != "CONFIG_PREEMPT_RT=y":
        problems.append(f"the guest's own configuration reports {line!r}, not CONFIG_PREEMPT_RT=y")
    report(
        "latency/guest-preempt-rt",
        problems,
        [
            f"nonce {reported_nonce}",
            f"guest uname -r: {reported}   host uname -r: {host}",
            f"guest uname -v: {guest_field(report_text, 'UNAME-V')}",
            f"guest probe: {line}",
        ],
    )
    return problems


def host_identity_case():
    """Case 2: the same probe on the reference host reports the option not set."""
    problems = []
    code, output, shell = host_probe()
    recorded = recorded_release("REFERENCE_HOST")
    running, absent = kernel_release()
    if running is None:
        print(f"SKIP latency/host-identity: {absent}")
        return []
    if code != 0:
        problems.append(f"the probe exited {code} on the host: {output}")
    elif output != "# CONFIG_PREEMPT_RT is not set":
        problems.append(f"the host's own configuration reports {output!r}")
    if running != recorded:
        problems.append(
            f"the host now runs {running!r}; the crate records {recorded!r}. "
            "The recorded host reading describes a kernel that is no longer running"
        )
    report(
        "latency/host-not-preempt-rt",
        problems,
        [
            f"host uname -r: {running}",
            f"host probe ({PROBE_SCRIPT.name}, the file the guest ran, under {shell}): "
            f"{output}",
        ],
    )
    return problems


def installed_modules():
    """Return the kernel release directories under /lib/modules."""
    if not HOST_MODULES.is_dir():
        return []
    return sorted(p.name for p in list(HOST_MODULES.iterdir())[:MAX_MODULE_DIRECTORIES])


def host_unmodified_case(image, release):
    """Case 3: D57, at the scope this profile actually admits a check.

    What is checked: the image is not owned by any package, no modules for the
    guest release are installed, and no distribution realtime package is
    installed. What is NOT checked, and is said rather than implied: `/boot` is
    mode 0700 root on this profile, so the gate cannot enumerate bootloader
    entries as the developer account and does not claim to have. What stands in
    for that is narrower and stronger than an enumeration -- the recorded set of
    programs this gate may invoke, which `tools/test_latency_fixture.py` holds,
    contains no installer, no bootloader tool and nothing that writes outside
    the build directory.
    """
    problems = []
    code, stdout, stderr = run(["pacman", "-Qo", str(image)], PACMAN_TIMEOUT)
    owner = (stdout + stderr).strip()
    if code == 0:
        problems.append(f"the kernel image is owned by a package: {owner}")
    modules = installed_modules()
    if release in modules:
        problems.append(f"/lib/modules/{release} exists; the guest kernel's modules are installed")
    realtime_code, realtime_out, realtime_err = run(["pacman", "-Qq", "linux-rt"], PACMAN_TIMEOUT)
    if realtime_code == 0:
        problems.append(f"a distribution realtime kernel is installed: {realtime_out.strip()}")
    report(
        "latency/host-unmodified",
        problems,
        [
            f"kernel image: {image}",
            f"pacman -Qo: {owner}",
            f"pacman -Qq linux-rt: {realtime_err.strip() or realtime_out.strip()}",
            f"/lib/modules holds {modules}; {release} is not among them",
            "not checked here: /boot is mode 0700 root on this profile, so bootloader "
            "entries are not enumerated and no claim is made about them",
        ],
    )
    return problems


def placement_rows(figures, is_realtime, edges):
    """Return one (name, threshold, attainment, verdict) row per threshold."""
    rows = []
    for name, threshold in edges:
        placed = attainment(threshold, figures["max"])
        rows.append((name, threshold, placed, verdict(is_realtime, placed)))
    return rows


def figure_notes(label, figures, rows):
    """Return the printable lines for one measured run."""
    notes = [
        f"{label}: kernel {figures['release']} ({figures['version']})",
        f"{label}: {figures['cycles']} cycles, min {figures['min']} ns, "
        f"avg {figures['avg']:.0f} ns, max {figures['max']} ns",
    ]
    for name, threshold, placed, decided in rows:
        notes.append(f"{label}: {name} = {threshold} ns -> {placed}, {decided}")
    return notes


def guest_measured_case(figures, edges, release):
    """Case 4: measured figures for the tier thresholds, on the realtime guest.

    The kernel flag the rows are placed with is read out of the figures the
    guest produced, not asserted here, and the check below is then an assertion
    about that reading rather than a separate one: a guest that did not report
    PREEMPT_RT prints `kernel-not-realtime` at every edge instead of four
    `satisfied` lines followed by the reason they did not count.
    """
    realtime = reports_realtime(figures["version"])
    rows = placement_rows(figures, realtime, edges)
    problems = []
    if figures["release"] != release:
        problems.append(f"the measurement ran on {figures['release']!r}, not on {release!r}")
    if not realtime:
        problems.append(f"the measuring kernel reports {figures['version']!r}, not PREEMPT_RT")
    if figures["cycles"] <= 0 or figures["max"] <= 0:
        problems.append(f"the measurement produced no samples: {figures}")
    if not any(decided == "satisfied" for _n, _t, _p, decided in rows):
        problems.append(
            "no tier edge was satisfied; no threshold was attained on an admitted kernel"
        )
    report("latency/guest-measured", problems, figure_notes("guest", figures, rows))
    return problems


def host_not_satisfying_case(guest, figures, edges):
    """Case 5: the same fixture on the host is not-satisfying, not a smaller number.

    The flag is read out of the host's own figures rather than written here, so
    the loop below checks that reading instead of restating it: with a literal
    `False` a reference host that had become realtime would still have printed
    `kernel-not-realtime` at every edge and passed this case. The falsifier
    further down keeps its literal on purpose -- it evaluates the rule against
    the best conceivable figure and is not a reading of any machine.
    """
    realtime = reports_realtime(figures["version"])
    rows = placement_rows(figures, realtime, edges)
    problems = []
    if not figures["version"]:
        problems.append("the host measurement carries no kernel version line to read")
    if realtime:
        problems.append(
            f"the measuring kernel reports {figures['version']!r}, which is PREEMPT_RT; "
            "this case requires the non-realtime reference host"
        )
    for name, _threshold, _placed, decided in rows:
        if decided != "kernel-not-realtime":
            problems.append(f"{name} on the host decided {decided}")
    if verdict(False, attainment(edges[0][1], 0)) != "kernel-not-realtime":
        problems.append("a worst case of zero on the host would not be kernel-not-realtime")
    if comparable_arguments(guest["arguments"]) != comparable_arguments(figures["arguments"]):
        problems.append(
            f"the two runs did not share an argument vector: "
            f"{comparable_arguments(guest['arguments'])} against "
            f"{comparable_arguments(figures['arguments'])}"
        )
    relation = "below" if figures["max"] < guest["max"] else "above"
    reached = ", ".join(sorted({decided for _n, _t, _p, decided in rows}))
    notes = figure_notes("host", figures, rows)
    notes.append(
        f"host max {figures['max']} ns is {relation} guest max {guest['max']} ns, and its "
        f"{len(rows)} verdicts read {reached}: the verdict is decided by the kernel, "
        "not by the figure"
    )
    notes.append(f"shared arguments: {' '.join(comparable_arguments(figures['arguments']))}")
    report("latency/host-not-satisfying", problems, notes)
    return problems


def boundary_case(edges):
    """Case 6: at a threshold, one below it and one beyond it are three outcomes."""
    name, threshold = edges[0]
    below = threshold - 1
    beyond = threshold + 1
    placements = [
        (below, attainment(threshold, below)),
        (threshold, attainment(threshold, threshold)),
        (beyond, attainment(threshold, beyond)),
    ]
    decided = [(value, placed, verdict(True, placed)) for value, placed in placements]
    problems = []
    expected = [("attained", "satisfied"), ("at-edge", "worst-case-at-edge")]
    expected.append(("exceeded", "worst-case-exceeds"))
    for (_value, placed, outcome), (want_placed, want_outcome) in zip(decided, expected):
        if (placed, outcome) != (want_placed, want_outcome):
            problems.append(f"expected {want_placed}/{want_outcome}, got {placed}/{outcome}")
    if len({outcome for _v, _p, outcome in decided}) != 3:
        problems.append("the three boundary values were not reported as three outcomes")
    notes = [f"{name} = {threshold} ns; these are rule evaluations, not measurements"]
    notes.extend(
        f"worst case {value} ns -> {placed}, {outcome}" for value, placed, outcome in decided
    )
    report("latency/boundary-edge", problems, notes)
    return problems


def run_cases(base, image, release):
    """Run every case in order and return the number that failed."""
    edges = thresholds()
    print(f"     {len(edges)} thresholds read from the crates: {edges}")
    nonce = f"aegis-{os.getpid()}-{secrets.token_hex(NONCE_BYTES)}"
    report_text = boot(image, build_initramfs(base), base, nonce)
    (base / "guest" / "retained-report.txt").write_text(report_text, encoding="utf-8")
    failed = 1 if guest_identity_case(report_text, nonce, release) else 0
    failed += 1 if host_identity_case() else 0
    failed += 1 if host_unmodified_case(image, release) else 0
    if guest_field(report_text, "MEASURE-STATUS") != "0":
        raise GateError("the guest's own measurement exited non-zero; see its retained report")
    guest = parse_report(guest_section(report_text, "JSON"))
    failed += 1 if guest_measured_case(guest, edges, release) else 0
    failed += 1 if host_not_satisfying_case(guest, host_measurement(base), edges) else 0
    failed += 1 if boundary_case(edges) else 0
    return failed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.parse_args()
    base = cache_dir("AEGIS_LATENCY_BUILD_DIR", "aegis-latency")
    base.mkdir(parents=True, exist_ok=True)
    (base / "guest").mkdir(parents=True, exist_ok=True)
    print(f"Latency fixture gate (M23, D57, D70, development evidence only). Work tree: {base}")
    reasons = check_toolchain()
    artifacts, absent = kernel_artifacts()
    if absent:
        reasons.append(absent)
    if not HOST_CONFIG.exists():
        reasons.append(f"{HOST_CONFIG} does not exist; the host half of the probe cannot run")
    if reasons:
        for reason in reasons[:MAX_PROBLEM_LINES]:
            print(f"SKIP: {reason}; the latency fixture gate did not run.")
        return 0
    image, release = artifacts
    print(f"     kernel under test: {image} as {release}")
    try:
        failed = run_cases(base, image, release)
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    if failed:
        print(f"FAIL: {failed} latency case(s) did not match their recorded outcome.")
        return 1
    print(
        "PASS: latency fixture on the reference profile (M23, D57, D70). Figures "
        "were measured in a guest running the kernel M26 builds and on the host. "
        "Nothing was installed and no package owns the image; /boot was not read, "
        "so no claim is made about bootloader entries. The boot, hardware and "
        "release gates remain blocked."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
