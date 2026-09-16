#!/usr/bin/env python3
"""Put the M19 eBPF fixtures through the host kernel's own verifier and keep the log.

Six kinds of case, no simulation and no failure suppression:

* positive loads -- ``action_gate``, ``kepler_power``, ``scx_cake`` and the
  all-stub ``scx_cake_stub`` are compiled warning-free and loaded with CAP_BPF
  and CAP_PERFMON, and every program's verifier log is retained;
* negative loads -- one bound is deleted from each source by marker, and the
  rebuilt object must be REJECTED BY THE VERIFIER LOG, with the recorded
  diagnostic present. A non-zero exit code alone is not accepted as evidence,
  and neither is a diagnostic that the unmutated object also produces, and
  neither is a log this run did not produce: every log is deleted immediately
  before the load that writes it and must come back carrying that load's own
  nonce, because the probe can exit non-zero WITHOUT writing one and a file left
  by an earlier run would otherwise supply the rejection string;
* the P06 attach case -- the LSM program is attached to
  ``lsm/bprm_check_security``, a marker binary is executed, and the event must
  reach the ring-buffer consumer;
* the P07 boundary attach -- a struct_ops link is created and destroyed on the
  host kernel. It DISPLACES the machine's CPU scheduler, so it runs only under
  ``--allow-scheduler-takeover`` and says why it did not run otherwise (D67);
* structural cases -- the stub's declared handler set and the unmeasured TDP
  literal are read back from the tracked sources and compared with pinned values;
* the guard -- a host without the toolchain, without the kernel configuration or
  without CAP_BPF prints why it did not run instead of reporting a pass. PATH is
  checked for every tool BEFORE any of them is invoked, because invoking an
  absent binary is a GateError, which is a FAIL and not the documented SKIP.

A pass is development evidence on the reference profile. It is a NON-QUALIFYING
LOCAL FIXTURE: the host kernel is neither built nor configured by this
repository, so it cannot close M10's Nucleus-kernel verification, and it closes
no image, boot, hardware or release gate.
"""

import argparse
import os
import re
import secrets
import shutil
import subprocess
import sys
import time
from pathlib import Path

from host import kernel_release
from host import gid as host_gid
from host import uid as host_uid

ROOT = Path(__file__).resolve().parent.parent
BPF_DIR = ROOT / "bpf"
LOADER = BPF_DIR / "loader" / "aegis_bpf_probe.c"

# Admitted versions, recorded in docs/roadmap/toolchain-admission.md. Each is a
# floor plus the exact reference-profile reading, the shape M03 set for systemd
# and M18 for mkosi: a distribution toolchain cannot be materialised from a file
# in this repository, so the admission is a floor the gate refuses to run below
# and a recorded value the outcomes below were observed on.
CLANG_FLOOR = (19, 0, 0)
REFERENCE_PROFILE_CLANG = "clang version 22.1.8"
REFERENCE_PROFILE_CLANG_PACKAGE = "extra/clang 22.1.8-2"
BPFTOOL_FLOOR = (7, 4, 0)
REFERENCE_PROFILE_BPFTOOL = "bpftool v7.8.0"
REFERENCE_PROFILE_BPFTOOL_PACKAGE = "core/bpf 7.2.5-1"
# The libbpf the loader LINKS AGAINST, read from pkg-config and again at runtime
# from libbpf_version_string(). It is NOT the "using libbpf v1.8" that
# `bpftool version` prints: that is the libbpf bpftool was statically built
# against, which is a different artefact and a different version here.
LIBBPF_FLOOR = (1, 5)
REFERENCE_PROFILE_LIBBPF = "1.7.0"
REFERENCE_PROFILE_LIBBPF_PACKAGE = "core/libbpf 1.7.0-1.1"
REFERENCE_PROFILE_BPFTOOL_BUILTIN_LIBBPF = "v1.8"
LLVM_STRIP_FLOOR = (19, 0, 0)
REFERENCE_PROFILE_KERNEL = "7.2.4-1-cachyos"

# Kernel configuration the fixtures require, each read back from the running
# kernel rather than assumed. The probe command is recorded beside each.
REQUIRED_CONFIG = ("CONFIG_BPF_SYSCALL", "CONFIG_BPF_LSM", "CONFIG_DEBUG_INFO_BTF")
SCHED_EXT_CONFIG = "CONFIG_SCHED_CLASS_EXT"
LSM_SYSFS = Path("/sys/kernel/security/lsm")
BTF_VMLINUX = Path("/sys/kernel/btf/vmlinux")
SCHED_EXT_DIR = Path("/sys/kernel/sched_ext")
SCHED_EXT_STATE = SCHED_EXT_DIR / "state"
SCHED_EXT_OPS = SCHED_EXT_DIR / "root/ops"
SCHED_EXT_EVENTS = SCHED_EXT_DIR / "root/events"
# The counters the milestone's criterion asks to capture, and where they really
# are: TOP-LEVEL attributes of /sys/kernel/sched_ext/, not members of root/.
# root/ holds exactly `events` and `ops`, which is what was checked when they
# were reported absent; one directory up holds state, switch_all, nr_rejected,
# enable_seq and hotplug_seq, all world-readable.
SCHED_EXT_COUNTERS = ("switch_all", "nr_rejected", "enable_seq")
# What switch_all reads while a SCX_OPS_SWITCH_PARTIAL scheduler holds root/ops.
# It reports the kernel's own scx_switching_all, so this reading -- not the flag
# read out of the fixture's source -- is the measurement that no task was
# switched to the stub.
SWITCH_ALL_PARTIAL = "0"

# The capability set the loads run under. CAP_BPF alone was observed to be
# insufficient on the reference kernel: BPF_PROG_TYPE_LSM, _TRACEPOINT and
# _STRUCT_OPS all returned -EPERM without CAP_PERFMON. The set is recorded here
# as the one that was actually used, not as the one the register assumed.
CAPABILITIES = "-all,+bpf,+perfmon"

# Every external tool the gate runs. Presence is checked before any of them is
# invoked, so a host without one of them SKIPs as documented instead of failing.
REQUIRED_TOOLS = ("clang", "bpftool", "llvm-strip", "pkg-config", "setpriv", "sudo")
# The probe's exit codes, pinned to their values in bpf/loader/aegis_bpf_probe.c
# rather than to the names that hold them. The distinction is the point: 6 says
# the probe could not WRITE its log, which is not a verifier rejection.
PROBE_EXIT_LOAD_FAILED = 1
PROBE_EXIT_LOG_FAILED = 6
# The header line the probe stamps into every log it writes, with the nonce this
# run generated. A log without it is not this run's evidence.
NONCE_HEADER = "# run-nonce"
NONCE_BYTES = 16
MAX_STALE_LOGS = 64
MAX_NOTE_LINES = 20
# The points the probe reads inside the attach case, in the order it reads them.
PROBE_POINTS = ("before", "during", "still_attached", "after")

COMMAND_TIMEOUT = 300
LOAD_TIMEOUT = 120
MAX_DIAGNOSTIC_LINES = 40
MAX_LOG_BYTES = 8 << 20
MAX_SOURCE_BYTES = 1 << 20
MARKER_NAME = "aegis_exec_prob"
MARKER_SOURCE = Path("/usr/bin/true")
ATTACH_HOLD_MS = 1000
# The machine's own scheduler supervisor. The boundary case releases and restores
# root/ops through this interface rather than by signalling a process, because
# the supervisor would otherwise restart the scheduler underneath the case.
SCX_LOADER_BUS = "org.scx.Loader"
SCX_LOADER_PATH = "/org/scx/Loader"
SCHED_CALL_TIMEOUT = 60
SCHED_POLL_TRIES = 20
SCHED_POLL_SECONDS = 1.0

CLANG_BPF_FLAGS = (
    "-target",
    "bpf",
    "-mcpu=v3",
    "-O2",
    "-g",
    "-D__TARGET_ARCH_x86",
    "-Wall",
    "-Wextra",
    "-Werror",
    # The one warning that is switched off, and why. libbpf's own BPF_PROG macro
    # (/usr/include/bpf/bpf_tracing.h) expands to a wrapper with a `ctx`
    # parameter its expansion does not use, so -Wunused-parameter fires on
    # library-generated code in every struct_ops and LSM program. It was
    # confirmed to be the macro and not the fixtures: kepler_power.bpf.c, the one
    # program written without BPF_PROG, compiles clean with the warning on.
    "-Wno-unused-parameter",
)
LOADER_FLAGS = ("-O2", "-g", "-std=gnu17", "-Wall", "-Wextra", "-Werror", "-D_GNU_SOURCE")

OBJECTS = ("action_gate", "kepler_power", "scx_cake", "scx_cake_stub")

POSITIVE_CASES = (
    ("action_gate/positive-load", "action_gate", "load"),
    ("kepler_power/positive-tracepoint-load", "kepler_power", "load"),
    ("scx_cake/positive-struct-ops-load", "scx_cake", "sops-load"),
    ("scx_cake_stub/boundary-all-stub-load", "scx_cake_stub", "sops-load"),
)

# Each negative deletes exactly one marked region and requires the verifier to
# name a rejection. The recorded diagnostics are the ones observed on the
# reference profile; this is a check over THESE THREE mutations and the strings
# they produced, not a proof that the verifier rejects unbounded code in general.
NEGATIVE_CASES = (
    (
        "action_gate/negative-unchecked-ringbuf-pointer",
        "action_gate",
        "ringbuf-null-check",
        "load",
        "invalid mem access 'ringbuf_mem_or_null'",
    ),
    (
        "scx_cake/negative-unbounded-loop",
        "scx_cake",
        "dispatch-loop-bound",
        "sops-load",
        "jumps is too complex",
    ),
    (
        "kepler_power/negative-out-of-bounds-map-access",
        "kepler_power",
        "map-value-index-bound",
        "load",
        "unbounded memory access, make sure to bounds check any such access",
    ),
)

# The handlers scx_cake_stub.bpf.c declares. "All handlers stubbed" is this set
# and nothing wider: the running kernel's sched_ext_ops carries far more members,
# and claiming the category when an enumeration was checked is exactly the defect
# M05 shipped twice.
STUB_HANDLERS = (
    "aegis_stub_select_cpu",
    "aegis_stub_enqueue",
    "aegis_stub_dequeue",
    "aegis_stub_dispatch",
    "aegis_stub_running",
    "aegis_stub_stopping",
    "aegis_stub_init_task",
    "aegis_stub_init",
    "aegis_stub_exit",
)
# Pinned to the value, never to the constant that holds it: a boundary test
# written as `literal == AEGIS_KEPLER_TDP_MILLIWATT_UNMEASURED` passes at any
# value, which is the defect M08 shipped five times.
TDP_LITERAL_MILLIWATT = 15000
TDP_MACRO = "AEGIS_KEPLER_TDP_MILLIWATT_UNMEASURED"
# A line in a verifier log that is bookkeeping rather than a diagnostic.
BOOKKEEPING = re.compile(r"^(processed \d+ insns|#|===|$)")


class GateError(Exception):
    """The gate could not be run at all, as opposed to a case that failed."""


def build_root():
    """Where objects and logs are written. Never inside the repository."""
    override = os.environ.get("AEGIS_BPF_BUILD_DIR")
    if override:
        return Path(override)
    cache = os.environ.get("XDG_CACHE_HOME") or str(Path.home() / ".cache")
    return Path(cache) / "aegis-bpf"


def run(argv, timeout=COMMAND_TIMEOUT):
    """Run `argv` and return (exit code, stdout, stderr) with a hard deadline."""
    env = dict(os.environ)
    env["LC_ALL"] = "C"
    env["NO_COLOR"] = "1"
    try:
        done = subprocess.run(
            argv, capture_output=True, text=True, timeout=timeout, env=env, check=False
        )
    except (subprocess.TimeoutExpired, OSError) as error:
        raise GateError(f"{argv[0]} could not be run: {error}") from error
    return done.returncode, done.stdout, done.stderr


def parse_version(text, pattern):
    """Return the dotted version `pattern` captures in `text`, or None."""
    match = re.search(pattern, text)
    if match is None:
        return None
    return tuple(int(part) for part in match.group(1).split("."))


def read_tool_versions():
    """Read every admitted tool version back from the tool itself."""
    versions = {}
    code, stdout, _ = run(["clang", "--version"], timeout=60)
    versions["clang"] = (parse_version(stdout, r"clang version (\d+\.\d+\.\d+)"), stdout.strip())
    code, targets, _ = run(["clang", "-print-targets"], timeout=60)
    versions["clang_bpf_targets"] = sorted(
        name for name in ("bpf", "bpfeb", "bpfel") if re.search(rf"^\s+{name}\s", targets, re.M)
    )
    _, stdout, _ = run(["bpftool", "version"], timeout=60)
    versions["bpftool"] = (parse_version(stdout, r"bpftool v(\d+\.\d+\.\d+)"), stdout.strip())
    builtin = re.search(r"using libbpf (v\S+)", stdout)
    versions["bpftool_builtin_libbpf"] = builtin.group(1) if builtin else None
    _, stdout, _ = run(["pkg-config", "--modversion", "libbpf"], timeout=60)
    versions["libbpf"] = (parse_version(stdout, r"^(\d+\.\d+)"), stdout.strip())
    _, stdout, _ = run(["llvm-strip", "--version"], timeout=60)
    strip_version = parse_version(stdout, r"LLVM version (\d+\.\d+\.\d+)")
    versions["llvm_strip"] = (strip_version, stdout.strip())
    return versions


def kernel_config():
    """Return the running kernel's configuration, or None when it is unreadable."""
    release, _absent = kernel_release()
    candidates = [["zcat", "/proc/config.gz"]]
    if release is not None:
        candidates.append(["cat", f"/boot/config-{release}"])
    for argv in candidates:
        if shutil.which(argv[0]) is None:
            continue
        code, stdout, _ = run(argv, timeout=60)
        if code == 0 and "CONFIG_BPF_SYSCALL" in stdout:
            return stdout
    return None


def read_text(path, limit=MAX_LOG_BYTES):
    """Read a bounded amount of text from `path`, or return None."""
    if path.is_symlink() or not path.exists():
        return None
    try:
        with path.open("r", encoding="utf-8", errors="replace") as handle:
            return handle.read(limit)
    except OSError:
        return None


def missing_tool():
    """Return the first tool the gate runs that is not on PATH, or None.

    This has to precede read_tool_versions(), which INVOKES clang, bpftool,
    pkg-config and llvm-strip: run() turns the OSError from an absent binary
    into a GateError, and a GateError is a FAIL. With the order the other way
    round the documented SKIP was unreachable for those four.
    """
    for tool in REQUIRED_TOOLS:
        if shutil.which(tool) is None:
            return f"{tool} is not on PATH"
    return None


def guard_tools(versions):
    """Return the reason the toolchain cannot run the gate, or None."""
    floors = (
        ("clang", CLANG_FLOOR, REFERENCE_PROFILE_CLANG),
        ("bpftool", BPFTOOL_FLOOR, REFERENCE_PROFILE_BPFTOOL),
        ("libbpf", LIBBPF_FLOOR, REFERENCE_PROFILE_LIBBPF),
        ("llvm_strip", LLVM_STRIP_FLOOR, REFERENCE_PROFILE_CLANG),
    )
    for name, floor, reference in floors:
        found = versions[name][0]
        if found is None:
            return f"no {name} version could be read (reference profile {reference!r})"
        if found < floor:
            return f"{name} {found} is below the admitted floor {floor} ({reference!r})"
    missing = sorted({"bpf", "bpfeb", "bpfel"} - set(versions["clang_bpf_targets"]))
    if missing:
        return f"clang -print-targets does not list {missing}"
    return None


def guard_kernel(config):
    """Return the reason the running kernel cannot run the gate, or None."""
    if config is None:
        return "the running kernel's configuration could not be read"
    for symbol in REQUIRED_CONFIG:
        if f"{symbol}=y" not in config:
            return f"the running kernel does not set {symbol}=y"
    if not BTF_VMLINUX.is_file():
        return f"{BTF_VMLINUX} is absent, so no CO-RE object can be built"
    code, _, _ = run(["sudo", "-n", "true"], timeout=30)
    if code != 0:
        return "passwordless sudo is unavailable, so no load can be run with CAP_BPF"
    return None


def guard():
    """Every reason the gate cannot run, in the order that keeps the SKIP reachable.

    Returns (reason, versions, config); versions and config are None when the
    run stopped before they were read.
    """
    reason = missing_tool()
    if reason is not None:
        return reason, None, None
    versions = read_tool_versions()
    reason = guard_tools(versions)
    if reason is not None:
        return reason, versions, None
    config = kernel_config()
    return guard_kernel(config), versions, config


def new_nonce():
    """A value for one load's log header that no earlier run can carry."""
    return f"aegis-{os.getpid()}-{secrets.token_hex(NONCE_BYTES)}"


def remove_log(log_path):
    """Delete the log a load is about to write; return why it survived, or None.

    Deleting it first is half the proof that the file read back afterwards came
    from this load. The other half is the nonce: deletion can only be attempted,
    and a directory the probe cannot write is exactly the case that produced a
    stale file in the first place.
    """
    try:
        log_path.unlink(missing_ok=True)
    except OSError as error:
        return f"the log at {log_path} could not be removed before the load: {error}"
    if log_path.exists():
        return f"the log at {log_path} survived removal before the load"
    return None


def clear_logs(paths):
    """Drop every log an earlier run left in the evidence directory.

    Hygiene, not proof: a case that does not run this time must not leave a file
    behind that reads like evidence. What decides whether a log belongs to this
    run is remove_log() plus the nonce, per case.
    """
    stale = sorted(paths["logs"].glob("*.log"))[:MAX_STALE_LOGS]
    for path in stale:
        try:
            path.unlink()
        except OSError:
            continue


def stamped_text(paths, log_path):
    """The text at `log_path` only if THIS run stamped it, else None."""
    nonce = paths["nonces"].get(str(log_path))
    text = read_text(log_path)
    if nonce is None or text is None:
        return None
    return text if f"{NONCE_HEADER} {nonce}" in text else None


def log_problems(stamp, code):
    """Return (text, problems) for the log one load was supposed to write.

    The text comes back only when the retained file carries that load's nonce.
    An exit code says nothing on its own here: the probe exits
    PROBE_EXIT_LOG_FAILED when it could not write the log at all, and before
    that code existed a non-verifier failure plus a pre-existing file read as a
    verifier rejection.
    """
    problems = []
    if stamp["removal"] is not None:
        problems.append(stamp["removal"])
    if code == PROBE_EXIT_LOG_FAILED:
        problems.append(
            f"the probe exited {PROBE_EXIT_LOG_FAILED}: it could not WRITE its verifier log, "
            f"which is not a verifier rejection"
        )
    text = read_text(stamp["path"])
    if text is None:
        problems.append(f"no verifier log was retained at {stamp['path']}")
        return None, problems
    if f"{NONCE_HEADER} {stamp['nonce']}" not in text:
        problems.append(
            f"the log at {stamp['path']} does not carry this load's nonce "
            f"{stamp['nonce']}; it was not written by this load and proves nothing about it"
        )
        return None, problems
    return text, problems


def capability_argv():
    """The exact command prefix every load runs under: one unprivileged uid and
    the recorded capability set, never the caller's own privileges."""
    return [
        "sudo",
        "-n",
        "setpriv",
        f"--reuid={host_uid()[0]}",
        f"--regid={host_gid()[0]}",
        "--clear-groups",
        f"--bounding-set={CAPABILITIES}",
        f"--inh-caps={CAPABILITIES}",
        f"--ambient-caps={CAPABILITIES}",
        "--",
    ]


def generate_vmlinux(paths):
    """Dump the running kernel's BTF as a C header. The objects are CO-RE against
    this machine's own kernel, which is the whole reason the result is a local
    fixture and not a portable claim."""
    header = paths["include"] / "vmlinux.h"
    code, stdout, stderr = run(
        ["bpftool", "btf", "dump", "file", str(BTF_VMLINUX), "format", "c"], timeout=COMMAND_TIMEOUT
    )
    if code != 0 or len(stdout) < 1000:
        raise GateError(f"bpftool btf dump failed: exit {code}: {stderr.strip()[:200]}")
    header.write_text(stdout, encoding="utf-8")
    return header


def compile_object(source, target, include_dirs, paths):
    """Compile one BPF source, then strip DWARF while keeping BTF."""
    argv = ["clang", *CLANG_BPF_FLAGS]
    for directory in include_dirs:
        argv.extend(["-I", str(directory)])
    argv.extend(["-isystem", str(paths["include"]), "-c", str(source), "-o", str(target)])
    code, stdout, stderr = run(argv)
    if code != 0:
        raise GateError(f"{source.name} did not compile warning-free: {(stderr or stdout)[:800]}")
    code, _, stderr = run(["llvm-strip", "-g", str(target)], timeout=60)
    if code != 0:
        raise GateError(f"llvm-strip failed on {target.name}: {stderr[:200]}")
    return target


def compile_loader(paths):
    """Build the verifier probe against the host libbpf."""
    code, cflags, _ = run(["pkg-config", "--cflags", "--libs", "libbpf"], timeout=60)
    if code != 0:
        raise GateError("pkg-config could not describe libbpf")
    target = paths["build"] / "aegis_bpf_probe"
    argv = ["clang", *LOADER_FLAGS, "-I", str(BPF_DIR), str(LOADER), "-o", str(target)]
    argv.extend(cflags.split())
    code, stdout, stderr = run(argv)
    if code != 0:
        raise GateError(f"the loader did not compile warning-free: {(stderr or stdout)[:800]}")
    return target


def mutate(source_name, region, paths):
    """Copy one source with exactly the marked region deleted.

    The region is delimited by AEGIS-MUTATE-BEGIN/END comments carrying the
    region name. Anything other than exactly one match is a refusal, so a
    renamed marker fails the gate instead of quietly producing an object
    identical to the positive one.
    """
    source = BPF_DIR / f"{source_name}.bpf.c"
    text = source.read_text(encoding="utf-8")[:MAX_SOURCE_BYTES]
    pattern = re.compile(
        rf"^[ \t]*// AEGIS-MUTATE-BEGIN {re.escape(region)}\n"
        rf".*?^[ \t]*// AEGIS-MUTATE-END {re.escape(region)}\n",
        re.S | re.M,
    )
    hits = pattern.findall(text)
    if len(hits) != 1:
        raise GateError(f"{source.name} carries {len(hits)} regions named {region!r}, expected 1")
    mutated = paths["scratch"] / f"{source_name}_negative.bpf.c"
    mutated.write_text(pattern.sub("", text), encoding="utf-8")
    return mutated, len(hits[0].splitlines())


def probe(paths, obj, mode, log_path, extra=()):
    """Run one load through the probe under the recorded capability set.

    Returns (exit code, stdout, stderr, stamp). The log is removed before the
    load runs and the load stamps its own nonce into the header, so a file an
    earlier run left at the same path can never be read back as this one's.
    """
    stamp = {"path": log_path, "nonce": new_nonce(), "removal": remove_log(log_path)}
    paths["nonces"][str(log_path)] = stamp["nonce"]
    argv = [
        *capability_argv(),
        str(paths["probe"]),
        "--object",
        str(obj),
        "--log",
        str(log_path),
        "--mode",
        mode,
        "--nonce",
        stamp["nonce"],
        "--deadline",
        "45",
        *extra,
    ]
    code, stdout, stderr = run(argv, timeout=LOAD_TIMEOUT)
    return code, stdout, stderr, stamp


def diagnostics(log_text):
    """Every line of a verifier log that is a diagnostic rather than bookkeeping."""
    if log_text is None:
        return []
    return [line for line in log_text.splitlines() if not BOOKKEEPING.match(line.strip())]


def check_positive(paths, name, obj_name, mode):
    """A positive load: the object must load and its log must be retained."""
    log_path = paths["logs"] / f"{obj_name}.positive.log"
    code, stdout, stderr, stamp = probe(paths, paths["objects"][obj_name], mode, log_path)
    log_text, problems = log_problems(stamp, code)
    if code != 0:
        problems.append(f"expected the object to load, probe exited {code}")
    if log_text is not None and "# programs:" not in log_text:
        problems.append("the retained log carries no program section")
    if "load_rc=0" not in stdout:
        problems.append("the probe did not report load_rc=0")
    note = f"{len(re.findall(r'^program=', stdout, re.M))} program(s); nonce {stamp['nonce']}"
    return name, code, problems, stderr + stdout, note, log_path


def rejection_problems(log_text, rejection):
    """Whether a log THIS load produced names the recorded rejection.

    An unstamped or absent log is reported by log_problems() instead; there is
    nothing to read here, and reading a file anyway is the defect this closes.
    """
    if log_text is None:
        return []
    if rejection not in log_text:
        return [f"the verifier log does not carry the rejection {rejection!r}"]
    if not any(rejection in line for line in diagnostics(log_text)):
        return [f"{rejection!r} appears only in bookkeeping lines, not as a diagnostic"]
    return []


def check_negative(paths, case):
    """A negative load, proven by the verifier log rather than by the exit code.

    Four things must all hold, and the first two are what the criterion is
    about: the log must have been written BY THIS LOAD, it must carry the
    recorded verifier rejection, the corresponding POSITIVE log -- also this
    run's -- must not carry it, and the load must fail. A non-zero exit over a
    log some earlier run left is what this case used to accept.
    """
    name, source_name, region, mode, rejection = case
    mutated, removed = mutate(source_name, region, paths)
    target = paths["build"] / f"{source_name}_negative.bpf.o"
    compile_object(mutated, target, (paths["scratch"], BPF_DIR), paths)
    log_path = paths["logs"] / f"{source_name}.negative.log"
    code, stdout, stderr, stamp = probe(paths, target, mode, log_path)
    log_text, problems = log_problems(stamp, code)
    control_path = paths["logs"] / f"{source_name}.positive.log"
    control = stamped_text(paths, control_path)
    if control is None:
        problems.append(
            f"no log from this run at {control_path}, so the rejection below cannot be "
            f"attributed to the deleted bound rather than to the unmutated object"
        )
    problems.extend(rejection_problems(log_text, rejection))
    if control is not None and rejection in control:
        problems.append(
            f"the unmutated object produces {rejection!r} too; the mutation proves nothing"
        )
    if code == 0:
        problems.append("the mutated object loaded; the deleted bound was not load-bearing")
    note = f"removed {removed} lines from {region}; nonce {stamp['nonce']}"
    return name, code, problems, stderr + stdout, note, log_path


def check_lsm_attach(paths, lsm_active):
    """P06: attach the LSM program and require a real exec to reach the consumer."""
    name = "action_gate/positive-exec-reaches-ringbuf"
    if not lsm_active:
        return name, None, [], "", "SKIP: bpf is not in the active LSM list", None
    marker = paths["scratch"] / MARKER_NAME
    shutil.copy(MARKER_SOURCE, marker)
    marker.chmod(0o755)
    log_path = paths["logs"] / "action_gate.attach.log"
    code, stdout, stderr, stamp = probe(
        paths, paths["objects"]["action_gate"], "lsm-probe", log_path, ("--marker", str(marker))
    )
    _, problems = log_problems(stamp, code)
    if code != 0:
        problems.append(f"the attach probe exited {code}")
    if "attached=lsm" not in stdout:
        problems.append("the probe did not report an LSM attach")
    if f"filename={marker}" not in stdout:
        problems.append("no event naming the marker binary reached the ring-buffer consumer")
    if "detached=lsm marker_seen=1" not in stdout:
        problems.append("the probe did not report a clean detach after seeing the marker")
    return name, code, problems, stderr + stdout, stdout, log_path


def sched_ext_snapshot():
    """The scheduler state and counters, read from the kernel's own files.

    /sys/kernel/sched_ext/ holds state, switch_all, nr_rejected, enable_seq and
    hotplug_seq; root/ holds exactly events and ops. Reading only root/ is what
    made switch_all and nr_rejected look absent.
    """
    snapshot = {
        "state": (read_text(SCHED_EXT_STATE) or "<absent>").strip(),
        "ops": (read_text(SCHED_EXT_OPS) or "<absent>").strip(),
        "events": (read_text(SCHED_EXT_EVENTS) or "<absent>").strip(),
    }
    for name in SCHED_EXT_COUNTERS:
        snapshot[name] = (read_text(SCHED_EXT_DIR / name) or "<absent>").strip()
    return snapshot


def probe_counters(stdout, label):
    """The counters the probe read at one labelled point, from its own output."""
    found = {}
    for name in SCHED_EXT_COUNTERS:
        match = re.search(rf"^sched_ext_{name}_{label}=(\S*)$", stdout, re.M)
        found[name] = match.group(1) if match else None
    return found


def during_counters(stdout):
    """The counters the probe read while it HELD the link, from its own output.

    They cannot be read from here: by the time this process reads sysfs the hold
    window is over. The probe prints them from inside it.
    """
    return probe_counters(stdout, "during")


def probe_counter_lines(stdout):
    """One line per point the probe read, so every step of the sequence is recorded.

    The probe's `before` and `after` are taken with the slot already released and
    released again, which is why they are separate rows from the gate's own
    before and after: those bracket the whole case, these bracket the link.
    """
    lines = []
    for label in PROBE_POINTS:
        found = probe_counters(stdout, label)
        ops = re.search(rf"^sched_ext_ops_{label}=(.*)$", stdout, re.M)
        values = " ".join(f"{name}={found[name] or '<absent>'}" for name in SCHED_EXT_COUNTERS)
        lines.append(f"probe {label}: ops={ops.group(1) if ops else '<absent>'} {values}")
    return lines


def enable_seq_delta(before, after):
    """How many scheduler enables separate two snapshots."""
    try:
        count = int(after["enable_seq"]) - int(before["enable_seq"])
    except ValueError:
        return f"unreadable: before {before['enable_seq']!r}, after {after['enable_seq']!r}"
    return f"{count} scheduler enable(s) between the two readings"


def counter_note(before, during, after):
    """The counters at each point, and which pair of readings is a difference.

    Split by what the kernel actually does with each, not by one blanket claim:
    enable_seq is incremented on every enable and never reset, so before/after
    IS a difference; nr_rejected is set to 0 by the enable path itself, so a
    pair spanning an attach is NOT; switch_all is not a counter at all but the
    live scx_switching_all of whatever holds root/ops when it is read; and the
    SCX_EV_* lines hang off the running scheduler's own kobject and go with it.
    """
    lines = []
    for name in SCHED_EXT_COUNTERS:
        held = during[name] if during[name] is not None else "<not captured>"
        lines.append(f"{name}: before={before[name]} during={held} after={after[name]}")
    lines.append(f"enable_seq is monotonic since boot: {enable_seq_delta(before, after)}")
    lines.append(
        "nr_rejected is reset to 0 by every scheduler enable, so its readings belong to "
        "the instance that was running and are NOT subtracted across the attach"
    )
    lines.append(
        f"the SCX_EV_* lines of root/events belong to the scheduler instance too: before "
        f"{len(before['events'].splitlines())} lines, after "
        f"{len(after['events'].splitlines())} lines, not comparable as totals"
    )
    return lines


def loader_call(method):
    """Call one method on the machine's own scheduler supervisor over DBUS.

    Through `sudo -n`, which is not decoration: these methods are polkit-gated,
    and an unprivileged caller with no interactive agent to answer the prompt
    gets `Not allowed!` after the polkit timeout, not an authorisation. A
    recorded command that cannot reproduce its own evidence unattended is not a
    recorded command, and the gate already requires passwordless sudo.
    """
    return run(
        [
            "sudo",
            "-n",
            "busctl",
            "--system",
            "call",
            SCX_LOADER_BUS,
            SCX_LOADER_PATH,
            SCX_LOADER_BUS,
            method,
        ],
        timeout=SCHED_CALL_TIMEOUT,
    )[0]


def await_ops(predicate):
    """Poll root/ops until `predicate` holds, or give up after a scalar bound."""
    for _ in range(SCHED_POLL_TRIES):
        value = (read_text(SCHED_EXT_OPS) or "<absent>").strip()
        if predicate(value):
            return True, value
        time.sleep(SCHED_POLL_SECONDS)
    return False, (read_text(SCHED_EXT_OPS) or "<absent>").strip()


def free_scheduler(before):
    """Ask the supervisor to release root/ops. Only one scheduler may hold it."""
    if before == "<absent>":
        return True, "root/ops was already free"
    if shutil.which("busctl") is None:
        return False, "busctl is absent, so the running scheduler cannot be released"
    if loader_call("StopScheduler") != 0:
        return False, f"{SCX_LOADER_BUS}.StopScheduler failed; {before!r} still holds root/ops"
    freed, value = await_ops(lambda value: value == "<absent>")
    return freed, (
        f"released root/ops from {before!r}" if freed else f"root/ops still reads {value!r}"
    )


def restore_scheduler(before):
    """Put the recorded scheduler back, and require it to be the SAME one.

    A different scheduler reading from root/ops afterwards is a failure, not a
    restore: on a host where a second supervisor is also enabled, whichever one
    retries first claims the free slot, and accepting that would record a restore
    that did not happen.
    """
    if before == "<absent>":
        return True, "nothing to restore; root/ops was free before the case"
    for method in ("RestoreDefault", "RestartScheduler"):
        if loader_call(method) != 0:
            continue
        restored, value = await_ops(lambda value: value == before)
        if restored:
            return True, f"{method} restored root/ops to {before!r}"
    value = (read_text(SCHED_EXT_OPS) or "<absent>").strip()
    return False, f"root/ops was {before!r} before and reads {value!r} now; it was NOT restored"


def check_struct_ops_attach(paths, allowed, sched_ext):
    """P07 boundary: attach and detach the all-stub scheduler on the host kernel.

    This displaces whatever scheduler holds /sys/kernel/sched_ext/root/ops, so it
    is opt-in (D67). The sequence is record, release, attach, detach, restore,
    and the restore is verified against the recorded value rather than against
    "something is attached".
    """
    name = "scx_cake_stub/boundary-struct-ops-attach-and-detach"
    if not sched_ext:
        return name, None, [], "", "SKIP: the host does not expose sched_ext", None
    if not allowed:
        return (
            name,
            None,
            [],
            "",
            "SKIP: --allow-scheduler-takeover was not given; this case displaces the "
            "machine's CPU scheduler and is a deliberate step, not a background one",
            None,
        )
    return run_struct_ops_attach(paths, name)


def attach_problems(stdout, during):
    """What the probe's own output must say about the window it held the link.

    The switch_all reading is the substantive one: SCX_OPS_SWITCH_PARTIAL read
    out of the fixture's source says what the stub ASKED for, while switch_all
    is the kernel answering whether it switched every task to it.
    """
    problems = []
    if "sched_ext_ops_during=aegis_cake_stub" not in stdout:
        problems.append("root/ops never read the stub scheduler while the link was held")
    if "detach_rc=0" not in stdout:
        problems.append("the struct_ops link was not destroyed cleanly")
    for name in SCHED_EXT_COUNTERS:
        if during[name] is None:
            problems.append(f"{name} was not captured while the link was held")
    if during["switch_all"] not in (None, SWITCH_ALL_PARTIAL):
        problems.append(
            f"switch_all read {during['switch_all']!r} while the stub held root/ops; a "
            f"SCX_OPS_SWITCH_PARTIAL scheduler reads {SWITCH_ALL_PARTIAL!r}, and that "
            f"reading is the kernel's own measurement that no task was switched to it"
        )
    return problems


def run_struct_ops_attach(paths, name):
    """The disruptive sequence itself, with the restore run on every path out."""
    before = sched_ext_snapshot()
    problems = []
    freed, free_note = free_scheduler(before["ops"])
    code, stdout, stderr = (None, "", "")
    log_path = None
    if not freed:
        problems.append(free_note)
    else:
        log_path = paths["logs"] / "scx_cake_stub.attach.log"
        code, stdout, stderr, stamp = probe(
            paths,
            paths["objects"]["scx_cake_stub"],
            "sops-attach",
            log_path,
            ("--hold-ms", str(ATTACH_HOLD_MS)),
        )
        if code != 0:
            problems.append(f"the struct_ops attach probe exited {code}")
        problems.extend(log_problems(stamp, code)[1])
        problems.extend(attach_problems(stdout, during_counters(stdout)))
    restored, restore_note = restore_scheduler(before["ops"])
    if not restored:
        problems.append(restore_note)
    after = sched_ext_snapshot()
    note = "\n".join(
        [
            f"{free_note}; {restore_note}",
            f"ops before={before['ops']} during=aegis_cake_stub after={after['ops']}",
            f"state before={before['state']} after={after['state']}",
        ]
        + probe_counter_lines(stdout)
        + counter_note(before, during_counters(stdout), after)
    )
    return name, code, problems, stderr + stdout, note, log_path


def check_stub_handlers(paths):
    """The stub's declared handler set, read back from the built object."""
    name = "scx_cake_stub/boundary-declared-handler-set"
    log_path = paths["logs"] / "scx_cake_stub.positive.log"
    text = stamped_text(paths, log_path)
    problems = []
    if text is None:
        return name, 0, [f"no log from this run at {log_path}"], "", "no handler set read", None
    found = sorted(re.findall(r"^=== program (\S+) \(section struct_ops", text, re.M))
    if found != sorted(STUB_HANDLERS):
        problems.append(f"the object declares {found}, the recorded set is {sorted(STUB_HANDLERS)}")
    return name, 0, problems, "", f"{len(found)} declared handlers, each an empty body", None


def check_tdp_literal():
    """The P13 TDP literal is recorded as unmeasured, and pinned to its value."""
    name = "kepler_power/boundary-tdp-literal-recorded-unmeasured"
    header = (BPF_DIR / "aegis_bpf_abi.h").read_text(encoding="utf-8")[:MAX_SOURCE_BYTES]
    match = re.search(rf"#define {TDP_MACRO} (\d+)ULL", header)
    problems = []
    if match is None:
        problems.append(f"{TDP_MACRO} is not defined in bpf/aegis_bpf_abi.h")
    elif int(match.group(1)) != TDP_LITERAL_MILLIWATT:
        problems.append(
            f"the literal is {match.group(1)}, the recorded value is {TDP_LITERAL_MILLIWATT}"
        )
    if "_UNMEASURED" not in TDP_MACRO:
        problems.append("the macro name does not mark the value as unmeasured")
    source = (BPF_DIR / "kepler_power.bpf.c").read_text(encoding="utf-8")[:MAX_SOURCE_BYTES]
    if "pseudo_energy_uj_unmeasured" not in source:
        problems.append("the accumulating field does not name itself unmeasured")
    note = f"{TDP_MACRO} = {TDP_LITERAL_MILLIWATT} mW, declared, never measured"
    return name, 0, problems, "", note, None


def report(outcome):
    """Print one case's outcome, and its diagnostics when it failed."""
    name, code, problems, diag, note, log_path = outcome
    if code is None and not problems:
        print(f"SKIP {name}: {note}")
        return 0
    status = "PASS" if not problems else "FAIL"
    print(f"{status} {name}: probe exit {code}")
    for line in note.splitlines()[:MAX_NOTE_LINES]:
        print(f"     {line}")
    if log_path is not None:
        print(f"     verifier log retained at {log_path}")
    for line in problems:
        print(f"     {line}")
    if problems:
        for line in diag.splitlines()[:MAX_DIAGNOSTIC_LINES]:
            print(f"     | {line}")
    return 1 if problems else 0


def prepare(paths):
    """Generate the kernel header, compile every object and the loader."""
    for key in ("build", "logs", "scratch", "include"):
        paths[key].mkdir(parents=True, exist_ok=True)
    paths["nonces"] = {}
    clear_logs(paths)
    generate_vmlinux(paths)
    paths["probe"] = compile_loader(paths)
    paths["objects"] = {}
    for name in OBJECTS:
        paths["objects"][name] = compile_object(
            BPF_DIR / f"{name}.bpf.c", paths["build"] / f"{name}.bpf.o", (BPF_DIR,), paths
        )
    return paths


def print_facts(versions, config, lsm_active, sched_ext):
    """Print every observed fact with the command that produced it."""
    release, absent = kernel_release()
    print(
        f"kernel: {release or absent} (uname -r); "
        f"reference profile {REFERENCE_PROFILE_KERNEL}"
    )
    for symbol in (*REQUIRED_CONFIG, SCHED_EXT_CONFIG):
        state = "y" if f"{symbol}=y" in (config or "") else "not set"
        print(f"  {symbol}={state} (zcat /proc/config.gz)")
    lsm_list = (read_text(LSM_SYSFS) or "").strip()
    print(f"  LSM list: {lsm_list} (cat {LSM_SYSFS}); bpf active: {lsm_active}")
    state = (read_text(SCHED_EXT_STATE) or "<absent>").strip()
    print(f"  sched_ext state: {state} (cat {SCHED_EXT_STATE})")
    ops = (read_text(SCHED_EXT_OPS) or "<absent>").strip()
    print(f"  sched_ext root/ops: {ops} (cat {SCHED_EXT_OPS})")
    for counter in SCHED_EXT_COUNTERS:
        value = (read_text(SCHED_EXT_DIR / counter) or "<absent>").strip()
        print(f"  sched_ext {counter}: {value} (cat {SCHED_EXT_DIR / counter})")
    print(f"  BTF: {BTF_VMLINUX} present: {BTF_VMLINUX.is_file()}")
    print(f"clang: {versions['clang'][1].splitlines()[0]} (clang --version)")
    print(f"  BPF targets: {versions['clang_bpf_targets']} (clang -print-targets)")
    print(f"bpftool: {versions['bpftool'][1].splitlines()[0]} (bpftool version)")
    print(
        f"  bpftool was built against libbpf {versions['bpftool_builtin_libbpf']}, which is NOT "
        f"the libbpf the loader links: that is {versions['libbpf'][1]} "
        f"(pkg-config --modversion libbpf)"
    )
    strip_version = ".".join(str(part) for part in versions["llvm_strip"][0] or ())
    print(f"llvm-strip: LLVM version {strip_version} (llvm-strip --version)")
    running_uid, no_uid = host_uid()
    print(f"loads run as uid {running_uid or no_uid} with capabilities {CAPABILITIES} (setpriv)")
    print(f"sched_ext present: {sched_ext}")


def run_cases(paths, lsm_active, sched_ext, allowed):
    """Run every case in order and return the number that failed."""
    failed = 0
    for name, obj_name, mode in POSITIVE_CASES:
        failed += report(check_positive(paths, name, obj_name, mode))
    failed += report(check_stub_handlers(paths))
    failed += report(check_tdp_literal())
    for case in NEGATIVE_CASES:
        failed += report(check_negative(paths, case))
    failed += report(check_lsm_attach(paths, lsm_active))
    failed += report(check_struct_ops_attach(paths, allowed, sched_ext))
    return failed


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--allow-scheduler-takeover",
        action="store_true",
        help="run the struct_ops boundary case, which displaces the machine's CPU scheduler",
    )
    args = parser.parse_args()
    try:
        reason, versions, config = guard()
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    if reason is not None:
        print(f"SKIP: {reason}; the eBPF verifier gate did not run.")
        return 0
    lsm_active = "bpf" in (read_text(LSM_SYSFS) or "").strip().split(",")
    sched_ext = f"{SCHED_EXT_CONFIG}=y" in config and SCHED_EXT_STATE.is_file()
    print_facts(versions, config, lsm_active, sched_ext)
    root = build_root()
    paths = {
        "build": root / "build",
        "logs": root / "logs",
        "scratch": root / "scratch",
        "include": root / "include",
    }
    try:
        failed = run_cases(prepare(paths), lsm_active, sched_ext, args.allow_scheduler_takeover)
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    if failed:
        print(f"FAIL: {failed} eBPF case(s) did not match their recorded outcome.")
        return 1
    print(
        "PASS: eBPF verifier gate on the reference profile. This is a NON-QUALIFYING "
        "LOCAL FIXTURE: the objects were verified by a host kernel this repository "
        "neither built nor configured, so it does not close M10's Nucleus-kernel "
        "verification, and no image, boot, hardware or release gate is closed."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
