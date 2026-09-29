#!/usr/bin/env python3
"""Boot the Nucleus-built kernel in a guest and repeat the M19 loads on it (M10).

The kernel under test is the one cordanaLLM/nucleus built and published as
release ``v7.2.8-realtime-lusoris1`` with an ``imago.nucleus.kernel-artifact.v1``
manifest. `build/kernel/nucleus-artifact.pin.json` pins every asset by sha256
and size, the tag's commit, and the keyless signer. Nothing here builds,
installs or packages a kernel: the image is a file under
``AEGIS_NUCLEUS_KERNEL_DIR`` (default
``${XDG_CACHE_HOME:-$HOME/.cache}/aegis-nucleus-kernel``), filled by
``--fetch``, the one networked step.

The order is the contract. Nothing boots before the release is verified and
recorded (E10-4), and no M19 object loads before the Aegis kernel requirement
has been checked against the configuration the kernel reported from inside
the guest (D94, E10-5):

1. every cached asset hashes to the pin; cosign verifies SHA256SUMS against
   the pinned identity, and against the pinned commit and tag as the
   certificate's workflow SHA and ref, so the provenance revision is bound
   to the signature; imago, built from the commit M09 pinned, accepts the
   manifest against the assets with ``imago kernel artifact verify``, and the
   kernel release, config digest, artifact digests and provenance revision it
   returns are recorded (positive); a copy whose kernel image differs by one
   byte is refused by the gate's own pre-boot hash and by imago, and is never
   booted (negative); the release floor admits exactly
   ``abi.minimum-release`` and refuses the release below it (boundary); the
   record is written to the run's ``release-record.json`` before any boot;
2. the readback boot, through M23's direct-kernel boot with only the kernel
   image and the initramfs substituted: the guest reports the nonce, its
   release, its own configuration and the sched_ext, BPF LSM and BTF probe;
   the probe is diffed against the same probe on the host and the reference
   profile, and the requirement is decided against the guest's configuration;
3. the load boot, only after every check above passed, negatives and
   boundaries included (each stage runs all of its cases, and the next starts
   only when every one of them passed): the guest re-reads its
   configuration's sha256 and loads nothing unless it is the one checked; the
   M19 objects, compiled against this kernel's own BTF, load through the
   verifier under the M19 capability set, positive and negative, and attach.

A pass is development evidence on the reference profile. The M19 host-kernel
fixture does not close M10, and a pass here closes neither the hardware nor
the release gate.
"""

import argparse
import base64
import binascii
import hashlib
import json
import os
import re
import secrets
import shlex
import shutil
import sys
import time
import zlib
from pathlib import Path

import kernel_requirement_check as requirement_check
import verify_bpf_objects as bpf
import verify_contract_pair as contract
import verify_latency_fixture as harness
from host import kernel_release
from host import target as linux_path

ROOT = Path(__file__).resolve().parent.parent
PIN = ROOT / "build" / "kernel" / "nucleus-artifact.pin.json"
PIN_SCHEMA = "aegis.m10.nucleus-kernel-pin.v1"
MANIFEST_SCHEMA = "imago.nucleus.kernel-artifact.v1"
PROFILE = ROOT / "planning" / "hardware-profile.json"
GUEST_INIT = ROOT / "tools" / "guest" / "aegis-nucleus-init.sh"
GUEST_PROBE = ROOT / "tools" / "guest" / "aegis-nucleus-probe.sh"
CACHE_VARIABLE = "AEGIS_NUCLEUS_KERNEL_DIR"
CACHE_NAME = "aegis-nucleus-kernel"
KVM = Path("/dev/kvm")
HOST_CONFIG = Path("/proc/config.gz")
SKIP_LINE = "the Nucleus kernel gate did not run."
RELEASE_RECORD = "release-record.json"

# Deadlines. Every external command carries one.
VERSION_TIMEOUT = 30
DOWNLOAD_TIMEOUT = 900
COSIGN_TIMEOUT = 120
IMAGO_TIMEOUT = 120
PROBE_TIMEOUT = 60
BTF_TIMEOUT = 300

# Scalar bounds (HISS-02).
MAX_ASSETS = 16
MAX_PROBLEM_LINES = 24
MAX_REPORT_LINES = 1 << 20
MAX_LOG_BYTES = 16 << 20
MAX_IMAGE_BYTES = 64 << 20
MAX_VMLINUX_BYTES = 256 << 20
HASH_CHUNK = 1 << 20
MAX_HASH_CHUNKS = 1 << 12
MAX_POWER_ROWS = 1024
MAX_TAIL_LINES = 4

HEX64 = re.compile(r"^[0-9a-f]{64}$")
COMMIT = re.compile(r"^[0-9a-f]{40}$")
BASENAME = re.compile(r"^[A-Za-z0-9][A-Za-z0-9._+-]{0,127}$")
POWER_ROW = re.compile(
    r"^power_(begin|end) cgroup_id=(\d+) runtime_ns=(\d+) pseudo_energy_uj_unmeasured=(\d+)$"
)

# The guest. Its userspace is this host's own; only the kernel is under test.
GUEST_MARK = "AEGIS-M10"
GUEST_STAGE = "aegis-m10"
GUEST_UID = 65534
GUEST_PROGRAMS = (
    "bash",
    "mount",
    "uname",
    "gzip",
    "grep",
    "cat",
    "sleep",
    "setpriv",
    "sha256sum",
    "mkdir",
    "stat",
    "base64",
    "true",
)
GUEST_LOADER = "aegis_bpf_probe"
MARKER = f"/{GUEST_STAGE}/{bpf.MARKER_NAME}"
MISSING_TRACEPOINT = "sched/aegis_no_such_tracepoint"
KEPLER_TRACEPOINT = "sched/sched_switch"
LOAD_DEADLINE = "45"
ATTACH_HOLD_MS = str(bpf.ATTACH_HOLD_MS)
IDLE_SETTLE_MS = "1500"
IDLE_HOLD_MS = "3000"
PROBE_EXIT_ATTACH_FAILED = 2

POSITIVE_OBJECTS = ("action_gate", "scx_cake", "kepler_power")
# M19's recorded negatives, reused by source name: the deleted region, the
# load mode and the rejection the verifier must name.
NEGATIVE_SOURCES = ("action_gate", "scx_cake")

CASE_LSM = "action_gate-attach"
CASE_LSM_NEGATIVE = "action_gate-unchecked-pointer"
CASE_SOPS = "scx_cake-attach"
CASE_SOPS_NEGATIVE = "scx_cake-unbounded"
CASE_TP = "kepler_power-attach-idle-interval"
CASE_TP_MISSING = "kepler_power-missing-tracepoint"

# The capabilities criterion 6 compares, with the reading the kernel under
# test must give. Each is decided from the guest's own reading; the host and
# the reference profile are printed beside it and never substituted for it.
CAPABILITIES = (
    ("CONFIG_SCHED_CLASS_EXT", "y"),
    ("bpf in the active LSM list", "yes"),
    ("/sys/kernel/btf/vmlinux", "present"),
)
# Read beside criterion 6's three and recorded, never decided here: a BPF LSM
# program attaches through a BPF trampoline, which on x86 patches the 5-byte
# nop that -mfentry leaves at the hook's entry (arch/x86/net/bpf_jit_comp.c,
# __bpf_arch_text_poke, -EBUSY when the bytes differ). A kernel built without
# the function tracer has no such nop, whatever CONFIG_BPF_LSM says. Since D107
# build/kernel-requirement.json requires CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS,
# which the requirement case decides like every row; trampoline_role() labels
# each symbol by whether the requirement names it.
TRAMPOLINE_SYMBOLS = ("CONFIG_FUNCTION_TRACER", "CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS")
SCHEDULER_UNREGISTERED = (
    'sched_ext: BPF scheduler "aegis_cake" disabled (unregistered from user space)'
)
CAKE_BUDGET_MAP = "cake_tier_budget"
CAPABILITY_DEFECTS = {
    "CONFIG_SCHED_CLASS_EXT": "a kernel without sched_ext is a Nucleus requirement defect (E10-2)",
    "bpf in the active LSM list": "BPF LSM absent from the kernel under test fails M10 (E10-1)",
    "/sys/kernel/btf/vmlinux": "without BTF no CO-RE object can load on the kernel under test",
}


def toolchain_rows():
    """Return the admitted toolchain rows: M23's guest tools plus M10's own."""
    reused = ("qemu-system-x86_64", "cpio", "ldd", "bash", "mount", "coreutils", "grep", "gzip")
    rows = [row for row in harness.TOOLCHAIN if row[0] in reused]
    floor = ".".join
    rows += [
        ("setpriv", ["setpriv", "--version"], r"util-linux (\d+(?:\.\d+)*)", None, "2.42.4"),
        ("cosign", ["cosign", "version"], r"GitVersion:\s+v(\d+(?:\.\d+)*)", "2.6.3", "2.6.3"),
        (
            "clang",
            ["clang", "--version"],
            r"clang version (\d+(?:\.\d+)*)",
            floor(str(part) for part in bpf.CLANG_FLOOR),
            "22.1.8",
        ),
        (
            "bpftool",
            ["bpftool", "version"],
            r"bpftool v(\d+(?:\.\d+)*)",
            floor(str(part) for part in bpf.BPFTOOL_FLOOR),
            "7.8.0",
        ),
        (
            "llvm-strip",
            ["llvm-strip", "--version"],
            r"LLVM version (\d+(?:\.\d+)*)",
            floor(str(part) for part in bpf.LLVM_STRIP_FLOOR),
            "22.1.8",
        ),
        (
            "libbpf",
            ["pkg-config", "--modversion", "libbpf"],
            r"^(\d+(?:\.\d+)*)",
            floor(str(part) for part in bpf.LIBBPF_FLOOR),
            bpf.REFERENCE_PROFILE_LIBBPF,
        ),
    ]
    return tuple(rows)


TOOLCHAIN = toolchain_rows()
FETCH_TOOLS = ("curl", "cosign")
# The version at which a tool stops being admitted, exclusive. The reference
# profile carries two cosign copies, and which one a bare `cosign` names
# depends on PATH order (planning/hardware-profile.json): M10 admitted and ran
# 2.6.3, and the other copy, 3.1.3, deprecates the `--offline` flag the gate
# passes. The gate reads the version back and refuses the 3 line rather than
# trusting PATH.
CEILINGS = {"cosign": "3"}


class GateError(Exception):
    """The gate could not be run at all, as opposed to a case that failed."""


GATE_ERRORS = (
    GateError,
    harness.GateError,
    bpf.GateError,
    contract.GateError,
    requirement_check.RequirementError,
)


class Context:
    """One run: the pin, the cache it reads and the run directory it writes."""

    def __init__(self, pin, cache, run_id):
        self.pin, self.cache, self.run_id = pin, cache, run_id
        self.release = cache / "release"
        self.run_dir = cache / "runs" / run_id
        self.requirement = requirement_check.load_requirement()
        self.outcomes = []
        self.recorded = {}
        self.nonces = {}
        self.guest_probe = {"config": {}}


def cache_dir():
    """Return the out-of-repository directory, overridable by AEGIS_NUCLEUS_KERNEL_DIR."""
    return harness.cache_dir(CACHE_VARIABLE, CACHE_NAME)


def read_json(path):
    """Return the parsed JSON document at `path`, or raise GateError naming it."""
    try:
        return json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, ValueError) as error:
        raise GateError(f"{path} could not be read as JSON: {error}") from error


def asset_rows(pin):
    """Return every pinned asset as {name, sha256, size}: manifest, sums, bundle, artifacts."""
    rows = [pin["manifest"], pin["checksums"], pin["bundle"]]
    rows += [
        {"file": row["name"], "sha256": row["sha256"], "size": row["size"]}
        for row in pin["artifacts"]
    ]
    return [
        {"name": row["file"], "sha256": row["sha256"], "size": row["size"]}
        for row in rows[:MAX_ASSETS]
    ]


def asset_row_problems(row):
    """Return why one pinned asset row is malformed, or []."""
    problems = []
    if not BASENAME.match(str(row.get("name", ""))):
        problems.append(f"asset name {row.get('name')!r} is not a safe basename")
    if not HEX64.match(str(row.get("sha256", ""))):
        problems.append(f"{row.get('name')}: sha256 is not 64 lowercase hex characters")
    if not isinstance(row.get("size"), int) or row["size"] <= 0:
        problems.append(f"{row.get('name')}: size is not a positive integer")
    return problems


def pin_header_problems(pin):
    """Return why the pin's identity fields are malformed, or []."""
    problems = []
    if not COMMIT.match(str(pin.get("revision", ""))):
        problems.append("revision is not a 40-character commit")
    if not str(pin.get("download", "")).startswith("https://"):
        problems.append("download is not an https URL")
    if (pin.get("manifest") or {}).get("schema") != MANIFEST_SCHEMA:
        problems.append(f"manifest.schema is not {MANIFEST_SCHEMA}")
    return problems


def kernel_problems(pin, rows):
    """Return why the pin's kernel names no pinned image or config, or a digest that differs."""
    names = {row["name"] for row in rows}
    kernel = pin.get("kernel") or {}
    problems = [
        f"kernel.{key} {kernel.get(key)!r} is not a pinned asset"
        for key in ("image", "config")
        if kernel.get(key) not in names
    ]
    config = [row for row in rows if row["name"] == kernel.get("config")]
    if config and kernel.get("config_digest") != f"sha256:{config[0]['sha256']}":
        problems.append("kernel.config_digest is not the pinned configuration's sha256")
    return problems


def pin_problems(pin):
    """Return why the pin cannot be used, or []."""
    if not isinstance(pin, dict) or pin.get("schema") != PIN_SCHEMA:
        return [f"the pin's schema is not {PIN_SCHEMA}"]
    problems = pin_header_problems(pin)
    try:
        rows = asset_rows(pin)
    except (KeyError, TypeError) as error:
        return problems + [f"the pin lists no complete asset rows: {error}"]
    for row in rows:
        problems.extend(asset_row_problems(row))
    return problems + kernel_problems(pin, rows)


def load_pin(path=PIN):
    """Return the pin, or raise GateError naming every problem."""
    pin = read_json(path)
    problems = pin_problems(pin)
    if problems:
        raise GateError("; ".join(problems[:MAX_PROBLEM_LINES]))
    return pin


def file_sha256(path):
    """Return the sha256 of the file at `path`, read in bounded chunks."""
    digest = hashlib.sha256()
    with Path(path).open("rb") as handle:
        for _ in range(MAX_HASH_CHUNKS):
            chunk = handle.read(HASH_CHUNK)
            if not chunk:
                break
            digest.update(chunk)
    return digest.hexdigest()


def file_problems(path, row):
    """Return why the file at `path` is not the pinned asset `row`, or []."""
    path = Path(path)
    if path.is_symlink() or not path.is_file():
        return [f"{row['name']}: {path} is not a regular file"]
    size = path.stat().st_size
    if size != row["size"]:
        return [f"{row['name']}: {size} bytes, the pin records {row['size']}"]
    found = file_sha256(path)
    if found != row["sha256"]:
        return [f"{row['name']}: sha256 {found}, the pin records {row['sha256']}"]
    return []


OTHER_WORKFLOW = ".github/workflows/aegis-not-the-signer.yml"


class Refused(Exception):
    """A kernel image the gate will not boot, with the reason."""


def verified_image(path, row):
    """Return `path` when it is the pinned kernel image; raise Refused otherwise.

    Called immediately before every boot, so an image altered after the
    assets case ran is refused rather than booted.
    """
    problems = file_problems(path, row)
    if problems:
        raise Refused("; ".join(problems))
    return path


def image_row(pin):
    """Return the pinned asset row of the kernel image."""
    return next(row for row in asset_rows(pin) if row["name"] == pin["kernel"]["image"])


def report(context, name, problems, notes=()):
    """Print one case's outcome and, when it failed, why; keep it for the summary."""
    print(f"{'PASS' if not problems else 'FAIL'} {name}")
    for note in notes:
        print(f"     {note}")
    for line in problems[:MAX_PROBLEM_LINES]:
        print(f"     {line}")
    context.outcomes.append({"case": name, "problems": list(problems), "notes": list(notes)})
    return not problems


def tail(text, lines=MAX_TAIL_LINES):
    """Return the last `lines` non-empty lines of `text`, joined."""
    kept = [line for line in text.splitlines() if line.strip()]
    return " | ".join(kept[-lines:])


def cosign_argv(release, pin, identity, offline=True, revision=None):
    """Return the cosign verify-blob argument vector for SHA256SUMS under `release`.

    The identity is exact, not a pattern: the certificate must name the
    workflow and tag the pin records. The certificate must also carry the
    pinned commit as its GitHub workflow SHA and the pinned tag as its ref,
    so the provenance revision the unsigned manifest states is bound to the
    signature rather than only compared with the pin. `revision` replaces the
    pinned commit, for the case that must be refused.
    """
    argv = ["cosign", "verify-blob"]
    if offline:
        argv.append("--offline")
    argv += [
        "--bundle",
        str(release / pin["bundle"]["file"]),
        "--certificate-identity",
        identity,
        "--certificate-oidc-issuer",
        pin["signer"]["issuer"],
        "--certificate-github-workflow-sha",
        revision or pin["revision"],
        "--certificate-github-workflow-ref",
        f"refs/tags/{pin['tag']}",
        str(release / pin["checksums"]["file"]),
    ]
    return argv


def assets_case(context):
    """E10-4 input: every cached asset is the pinned one, and the manifest says so too."""
    problems = []
    for row in asset_rows(context.pin):
        problems.extend(file_problems(context.release / row["name"], row))
    manifest = read_json(context.release / context.pin["manifest"]["file"]) if not problems else {}
    problems.extend(manifest_problems(context.pin, manifest) if manifest else [])
    notes = [f"{len(asset_rows(context.pin))} assets hash to the pin under {context.release}"]
    if problems:
        notes = []
        problems.append("run `make nucleus-kernel-fetch`; nothing was booted")
    return report(context, "nucleus/assets-pinned", problems, notes)


def manifest_problems(pin, manifest):
    """Return where the downloaded manifest disagrees with the pin, or []."""
    kernel = manifest.get("kernel") or {}
    provenance = manifest.get("provenance") or {}
    rows = (
        ("schema", manifest.get("schema"), MANIFEST_SCHEMA),
        ("provider", manifest.get("provider"), pin["provider"]),
        ("stream", manifest.get("stream"), pin["stream"]),
        ("version", manifest.get("version"), pin["version"]),
        ("kernel.release", kernel.get("release"), pin["kernel"]["release"]),
        ("kernel.config_digest", kernel.get("config_digest"), pin["kernel"]["config_digest"]),
        (
            "checksums.sha256",
            (manifest.get("checksums") or {}).get("sha256"),
            pin["checksums"]["sha256"],
        ),
        ("provenance.tag", provenance.get("tag"), pin["tag"]),
        ("provenance.revision", provenance.get("revision"), pin["revision"]),
        (
            "provenance.signer_identity",
            provenance.get("signer_identity"),
            pin["signer"]["identity"],
        ),
    )
    problems = [
        f"manifest {key} is {got!r}, the pin has {want!r}" for key, got, want in rows if got != want
    ]
    listed = sorted(
        (row.get("name"), row.get("sha256"), row.get("size"))
        for row in manifest.get("artifacts") or []
    )
    pinned = sorted((row["name"], row["sha256"], row["size"]) for row in pin["artifacts"])
    if listed != pinned:
        problems.append("the manifest's artifact list is not the pinned one")
    return problems


def other_revision(revision):
    """Return the commit that differs from `revision` in its last hex digit only."""
    return revision[:-1] + ("1" if revision[-1] == "0" else "0")


def cosign_refusal(context, identity, revision):
    """Return (exit code, last output line) of a verify-blob that must be refused."""
    code, stdout, stderr = harness.run(
        cosign_argv(context.release, context.pin, identity, revision=revision),
        timeout=COSIGN_TIMEOUT,
    )
    return code, tail(stdout + stderr, 1)[:160]


def signature_case(context):
    """SHA256SUMS carries the pinned keyless signature; another identity or commit is refused."""
    pin = context.pin
    identity = pin["signer"]["identity"]
    code, stdout, stderr = harness.run(
        cosign_argv(context.release, pin, identity), timeout=COSIGN_TIMEOUT
    )
    problems = []
    if code != 0 or "Verified OK" not in stdout + stderr:
        problems.append(f"cosign verify-blob exited {code}: {tail(stdout + stderr)}")
    other = f"{pin['signer']['identity-prefix']}{OTHER_WORKFLOW}@refs/tags/{pin['tag']}"
    commit = other_revision(pin["revision"])
    refusals = (
        ("identity", other, cosign_refusal(context, other, None)),
        ("revision", commit, cosign_refusal(context, identity, commit)),
    )
    notes = [
        f"cosign verify-blob --offline: {tail(stdout + stderr, 1)} for {identity}, issuer "
        f"{pin['signer']['issuer']}, workflow SHA {pin['revision']}, ref refs/tags/{pin['tag']}"
    ]
    for label, value, (refused, line) in refusals:
        notes.append(f"another {label} {value} refused, exit {refused}: {line}")
        if refused == 0:
            problems.append(
                f"cosign accepted the bundle for {label} {value}; it is not load-bearing"
            )
    return report(context, "nucleus/signature-verified", problems, notes)


def contract_store():
    """Return M09's contract cache: the imago binary, its identity record and versions.json."""
    store = contract.cache_dir()
    return {
        "binary": store / "bin" / f"imago{contract.EXE}",
        "identity": store / "identity.json",
        "versions": store / "imago" / "versions.json",
    }


def imago_identity_problems(store):
    """Return why the cached imago is not the binary M09's fetch built at its pin, or []."""
    pin = contract.load_pin()
    record = contract.load_json(store["identity"])
    problems = contract.identity_problems(record, pin)
    digest = file_sha256(store["binary"])
    if digest != contract.recorded_binary(record):
        problems.append(f"imago sha256 {digest} is not the one `make contract-fetch` recorded")
    return problems, pin["imago"]["commit"], digest


def imago_argv(context, store, directory):
    """Return `imago kernel artifact verify` for the manifest against `directory`."""
    pin = context.pin
    return [
        str(store["binary"]),
        "kernel",
        "artifact",
        "verify",
        "--manifest",
        str(context.release / pin["manifest"]["file"]),
        "--dir",
        str(directory),
        "--versions",
        str(store["versions"]),
        "--expect-stream",
        pin["stream"],
        "--expect-tag",
        pin["tag"],
        "--expect-version",
        pin["version"],
        "--json",
    ]


def verification_problems(pin, result):
    """Return where imago's verification result disagrees with the pin, or []."""
    provenance = result.get("provenance") or {}
    rows = (
        ("kernel_release", result.get("kernel_release"), pin["kernel"]["release"]),
        ("config_digest", result.get("config_digest"), pin["kernel"]["config_digest"]),
        ("artifact_digest", result.get("artifact_digest"), f"sha256:{pin['checksums']['sha256']}"),
        ("provenance.revision", provenance.get("revision"), pin["revision"]),
        ("provenance.tag", provenance.get("tag"), pin["tag"]),
        ("bundle_present", result.get("bundle_present"), True),
    )
    problems = [
        f"imago {key} is {got!r}, the pin has {want!r}" for key, got, want in rows if got != want
    ]
    listed = sorted(
        (row.get("name"), row.get("sha256"), row.get("size"))
        for row in result.get("artifacts") or []
    )
    if listed != sorted((row["name"], row["sha256"], row["size"]) for row in pin["artifacts"]):
        problems.append("imago's artifact list is not the pinned one")
    return problems


def imago_result(stdout, problems):
    """Return imago's --json result, or {} with the reason appended to `problems`."""
    try:
        result = json.loads(stdout)
    except ValueError as error:
        problems.append(f"imago printed no JSON result: {error}")
        return {}
    if not isinstance(result, dict):
        problems.append("imago's JSON result is not an object")
        return {}
    return result


def imago_case(context):
    """E10-4 positive: imago at the pinned commit accepts the manifest; the result is recorded."""
    store = contract_store()
    problems, commit, digest = imago_identity_problems(store)
    code, stdout, stderr = harness.run(
        imago_argv(context, store, context.release), timeout=IMAGO_TIMEOUT
    )
    result = {}
    if code != 0:
        problems.append(f"imago kernel artifact verify exited {code}: {tail(stderr)}")
    else:
        result = imago_result(stdout, problems)
        problems.extend(verification_problems(context.pin, result))
    context.recorded = {
        "kernel.release": result.get("kernel_release"),
        "kernel.config_digest": result.get("config_digest"),
        "artifact_digest": result.get("artifact_digest"),
        "artifacts": {row["name"]: row["sha256"] for row in result.get("artifacts") or []},
        "provenance.revision": (result.get("provenance") or {}).get("revision"),
        "imago.commit": commit,
    }
    notes = [f"imago {commit[:12]} (sha256 {digest[:16]}...) accepted {context.pin['tag']}"]
    notes += [
        f"recorded {key}: {value}" for key, value in context.recorded.items() if key != "artifacts"
    ]
    notes += [
        f"recorded {name}: sha256 {sha}" for name, sha in context.recorded["artifacts"].items()
    ]
    notes.append(f"unlisted files imago reported, not rejected: {result.get('extra')}")
    return report(context, "nucleus/imago-accepts-and-recorded", problems, notes)


def tampered_copy(context):
    """Return a scratch copy of the release whose kernel image differs by one byte.

    Every other file is a hard link (a copy where the file system refuses one);
    the image is written fresh, so the cached bytes are never touched.
    """
    target = context.run_dir / "tampered"
    target.mkdir(parents=True, exist_ok=True)
    image_name = context.pin["kernel"]["image"]
    for row in asset_rows(context.pin):
        if row["name"] == image_name:
            continue
        try:
            os.link(context.release / row["name"], target / row["name"])
        except OSError:
            shutil.copy2(context.release / row["name"], target / row["name"])
    data = bytearray((context.release / image_name).read_bytes()[:MAX_IMAGE_BYTES])
    data[len(data) // 2] ^= 0xFF
    (target / image_name).write_bytes(bytes(data))
    return target


def mismatch_case(context):
    """E10-4 negative: an image whose digest is not the manifest's is refused before boot."""
    target = tampered_copy(context)
    row = image_row(context.pin)
    problems = []
    try:
        verified_image(target / row["name"], row)
        problems.append("the pre-boot hash accepted the tampered image")
        refusal = "none"
    except Refused as error:
        refusal = str(error)
    code, _stdout, stderr = harness.run(
        imago_argv(context, contract_store(), target), timeout=IMAGO_TIMEOUT
    )
    if code == 0:
        problems.append("imago accepted the tampered release")
    elif "digest mismatch" not in stderr or row["name"] not in stderr:
        problems.append(f"imago refused for another reason: {tail(stderr)}")
    shutil.rmtree(target, ignore_errors=True)
    notes = [
        f"pre-boot hash refused: {refusal[:200]}",
        f"imago exit {code}: {tail(stderr, 1)[:200]}",
        "no guest was started for this copy; the refusal precedes every boot",
    ]
    return report(context, "nucleus/digest-mismatch-refused-before-boot", problems, notes)


def release_below(minimum):
    """Return the highest dotted release below `minimum`, e.g. 6.11.999 below 6.12."""
    parts = requirement_check.numeric_components(minimum)
    for index in range(len(parts) - 1, -1, -1):
        if parts[index] > 0:
            return ".".join(str(part) for part in [*parts[:index], parts[index] - 1, 999])
    return None


def release_case(context):
    """E10-4 boundary: exactly abi.minimum-release is admitted, the release below it is not."""
    minimum = context.requirement["abi"]["minimum-release"]
    release = context.pin["kernel"]["release"]
    below = release_below(minimum)
    evaluations = [
        (minimum, True),
        (f"{minimum}.0-lusoris1", True),
        (below, False),
        (f"{below}-lusoris1", False),
        (release, True),
    ]
    problems = []
    notes = [f"abi.minimum-release {minimum}; these are rule evaluations, the last is the kernel"]
    for value, wanted in evaluations:
        found = requirement_check.release_at_least(value, minimum)
        notes.append(f"{value}: {'admitted' if found else 'refused'}")
        if found != wanted:
            problems.append(f"{value} was {'admitted' if found else 'refused'}")
    config = requirement_check.parse_config("CONFIG_X86_64=y\n")
    planted = requirement_check.identity_rejections(context.requirement, config, below)
    if not any("below abi.minimum-release" in line for line in planted):
        problems.append(f"a kernel reporting {below} was not rejected by the requirement check")
    notes.append(
        f"requirement check on a kernel reporting {below}: {planted[-1] if planted else 'none'}"
    )
    return report(context, "nucleus/minimum-release-boundary", problems, notes)


def new_nonce():
    """Return a value no earlier boot or load can carry."""
    return f"aegis-{os.getpid()}-{secrets.token_hex(bpf.NONCE_BYTES)}"


def stage_tree(base, phase, loader=None):
    """Lay out one guest's root: host userspace, M10's init and probe, and the phase file."""
    tree = base / "guest" / "root"
    if tree.exists():
        shutil.rmtree(tree)
    tree.mkdir(parents=True)
    harness.install_programs(tree, GUEST_PROGRAMS, () if loader is None else (loader,))
    for source, name in ((GUEST_INIT, "init"), (GUEST_PROBE, GUEST_PROBE.name)):
        shutil.copy2(source, tree / name)
        (tree / name).chmod(0o755)
    stage = tree / GUEST_STAGE
    stage.mkdir()
    (stage / "phase").write_text(f"{phase}\n", encoding="utf-8")
    return tree, stage


def guarded_boot(context, base, tree):
    """Archive `tree`, hash the kernel image against the pin, and only then boot it.

    The boot is M23's `boot()` unchanged: the same emulator argument vector
    and command line, with this kernel image and this initramfs.
    """
    initramfs = harness.archive_tree(tree, base / "guest" / "initramfs.cpio")
    row = image_row(context.pin)
    try:
        image = verified_image(context.release / row["name"], row)
    except Refused as error:
        raise GateError(
            f"the kernel image is not the pinned one, so it was not booted: {error}"
        ) from error
    nonce = new_nonce()
    text = harness.boot(image, initramfs, base, nonce)
    (base / "guest" / "retained-report.txt").write_text(text, encoding="utf-8")
    context.recorded[f"{base.name}.initramfs.sha256"] = file_sha256(initramfs)
    shutil.rmtree(tree, ignore_errors=True)
    initramfs.unlink(missing_ok=True)
    return text, nonce


def guest_lines(text):
    """Return the guest report's lines, bounded."""
    return text.splitlines()[:MAX_REPORT_LINES]


def guest_field(text, name):
    """Return one `AEGIS-M10-<name> <value>` line's value, as the guest printed it."""
    prefix = f"{GUEST_MARK}-{name} "
    for line in guest_lines(text):
        if line.startswith(prefix):
            return line[len(prefix) :].strip()
    raise GateError(f"the guest printed no {name} line; see its retained report")


def guest_section(text, name, label=""):
    """Return the lines the guest printed between one pair of markers."""
    suffix = f" {label}" if label else ""
    lines = guest_lines(text)
    start, end = f"{GUEST_MARK}-{name}-BEGIN{suffix}", f"{GUEST_MARK}-{name}-END{suffix}"
    if start not in lines or end not in lines[lines.index(start) :]:
        raise GateError(f"the guest printed no complete {name}{suffix} section")
    first = lines.index(start) + 1
    return lines[first : lines.index(end, first)]


def parse_probe(lines):
    """Return the probe's readings: config lines, the LSM list, BTF and sched_ext state."""
    readings = {"config": {}, "lsm": [], "btf": "unread", "sched_ext_state": "unread"}
    for line in lines:
        if line.startswith("LSM "):
            readings["lsm"] = line[4:].strip().split(",")
        elif line.startswith("BTF-VMLINUX "):
            readings["btf"] = line.split(" ", 1)[1].strip()
        elif line.startswith("SCHED-EXT-STATE "):
            readings["sched_ext_state"] = line.split(" ", 1)[1].strip()
    readings["config"] = requirement_check.parse_config("\n".join(lines))
    return readings


def capability_values(readings):
    """Return criterion 6's three readings in the spelling CAPABILITIES compares."""
    lsm = readings["lsm"]
    return {
        "CONFIG_SCHED_CLASS_EXT": readings["config"].get("CONFIG_SCHED_CLASS_EXT", "unobserved"),
        "bpf in the active LSM list": "yes" if "bpf" in lsm else "no",
        "/sys/kernel/btf/vmlinux": readings["btf"],
    }


def profile_values():
    """Return the reference profile's recorded host values for criterion 6."""
    profile = read_json(PROFILE)
    capabilities = profile["capabilities"]
    return {
        "CONFIG_SCHED_CLASS_EXT": profile["kernel"]["config"].get("CONFIG_SCHED_CLASS_EXT"),
        "bpf in the active LSM list": "yes" if capabilities["bpf_lsm"]["present"] else "no",
        "/sys/kernel/btf/vmlinux": "present" if capabilities["btf"]["present"] else "absent",
    }


def host_probe():
    """Run the guest's own probe script against the host's running kernel."""
    code, stdout, stderr = harness.run(["bash", str(GUEST_PROBE)], timeout=PROBE_TIMEOUT)
    if code != 0:
        raise GateError(f"the probe exited {code} on the host: {tail(stderr)}")
    return parse_probe(stdout.splitlines())


def config_text(text):
    """Return the configuration the guest printed, with the newline each line ended in."""
    return "".join(f"{line}\n" for line in guest_section(text, "CONFIG"))


def identity_problems(context, text, nonce):
    """Return why a guest report is not this boot of the pinned kernel, or []."""
    problems = []
    reported = guest_field(text, "NONCE")
    if reported != nonce:
        problems.append(f"the report carries nonce {reported!r}, not this boot's {nonce!r}")
    release = guest_field(text, "UNAME-R")
    if release != context.pin["kernel"]["release"]:
        problems.append(f"the guest reports {release!r}, not {context.pin['kernel']['release']!r}")
    host, _absent = kernel_release()
    if release == host:
        problems.append(f"the guest reports the host's own release {host!r}")
    return problems, release


def guest_identity_case(context, text, nonce):
    """The readback guest is the pinned kernel, and its own configuration is the published one."""
    problems, release = identity_problems(context, text, nonce)
    reported = guest_field(text, "CONFIG-SHA256")
    rebuilt = hashlib.sha256(config_text(text).encode("utf-8")).hexdigest()
    published = context.pin["kernel"]["config_digest"].removeprefix("sha256:")
    if reported != rebuilt:
        problems.append(f"the guest hashed its configuration to {reported}, its text to {rebuilt}")
    if reported != published:
        problems.append(f"/proc/config.gz hashes to {reported}, the manifest records {published}")
    context.recorded["guest.config_sha256"] = reported
    host, absent = kernel_release()
    notes = [
        f"nonce {nonce}",
        f"guest uname -r: {release}   host uname -r: {host or absent}",
        f"guest uname -v: {guest_field(text, 'UNAME-V')}",
        f"guest /proc/config.gz sha256 {reported}, the manifest's kernel.config_digest",
    ]
    return report(context, "nucleus/guest-identity-and-config", problems, notes)


def capability_case(context, guest, host):
    """Criterion 6: each capability is read from the guest and diffed against the host."""
    found, seen, recorded = capability_values(guest), capability_values(host), profile_values()
    problems = []
    notes = [f"guest LSM list {','.join(guest['lsm'])}; host {','.join(host['lsm'])}"]
    for name, wanted in CAPABILITIES:
        differs = "differs from the host" if found[name] != seen[name] else "same as the host"
        notes.append(
            f"{name}: guest {found[name]}, host {seen[name]}, reference profile "
            f"{recorded[name]} ({differs}; decided from the guest)"
        )
        if found[name] != wanted:
            problems.append(
                f"{name} reads {found[name]!r} in the guest: {CAPABILITY_DEFECTS[name]}"
            )
    notes.append(
        f"sched_ext state: guest {guest['sched_ext_state']}, host {host['sched_ext_state']}"
    )
    for symbol in TRAMPOLINE_SYMBOLS:
        notes.append(
            f"{symbol} ({trampoline_role(context.requirement, symbol)}): "
            f"guest {trampoline_state(guest, symbol)}, host {trampoline_state(host, symbol)}"
        )
    context.guest_probe = guest
    return report(context, "nucleus/capabilities-read-in-guest-diffed-with-host", problems, notes)


def trampoline_state(readings, symbol):
    """Return one trampoline prerequisite's state as a probe read it."""
    return readings["config"].get(symbol, "absent from the configuration")


def trampoline_role(requirement, symbol):
    """Return whether the requirement names one trampoline prerequisite (D107).

    Only a label for the record: a named symbol is decided by the requirement
    case, from the guest's configuration, and this case never decides either.
    """
    rows = requirement["features"][: requirement_check.MAX_FEATURES]
    if any(row["symbol"] == symbol for row in rows):
        return "required; decided by the requirement case"
    return "recorded, not required"


def requirement_case(context, text):
    """D94, E10-5 positive: every feature is decided on the guest's configuration."""
    release = guest_field(text, "UNAME-R")
    decided, rejected = requirement_check.check(context.requirement, config_text(text), release)
    context.recorded["requirement"] = {row["symbol"]: row["observed"] for row in decided}
    notes = [f"correlation-id {context.requirement['correlation-id']}, release {release}"]
    notes += [requirement_check.row_line(row) for row in decided]
    return report(context, "nucleus/requirement-satisfied-before-any-load", rejected, notes)


def planted(text, symbol, line):
    """Return `text` with every line for `symbol` replaced by `line`, removed when None.

    A symbol the text does not mention gets `line` appended, so a plant is
    never silently a no-op.
    """
    kept, seen = [], False
    for existing in text.splitlines():
        named = existing.startswith(f"{symbol}=") or existing == f"# {symbol} is not set"
        seen = seen or named
        if not named:
            kept.append(existing)
        elif line is not None:
            kept.append(line)
    if not seen and line is not None:
        kept.append(line)
    return "".join(f"{entry}\n" for entry in kept)


def planted_rejections(context, text, symbol, line):
    """Return the requirement check's rejections for one planted configuration line."""
    return requirement_check.check(context.requirement, planted(text, symbol, line))[1]


def single_rejection_problems(context, rejected, symbol, label):
    """Return why `rejected` is not exactly one rejection naming the id and `symbol`, or []."""
    correlation = context.requirement["correlation-id"]
    named = [line for line in rejected if correlation in line and f" {symbol} " in line]
    if len(rejected) != 1 or len(named) != 1:
        return [
            f"{label}: expected one rejection naming {correlation} and {symbol}, got {rejected}"
        ]
    return []


def requirement_negative_case(context, text):
    """E10-5 negative: a feature the configuration does not satisfy is rejected by name."""
    config = config_text(text)
    plants = (
        ("CONFIG_SCHED_CLASS_EXT", "CONFIG_SCHED_CLASS_EXT=n", "set to n"),
        ("CONFIG_BPF_LSM", "# CONFIG_BPF_LSM is not set", "not set"),
        ("CONFIG_HZ_1000", None, "line removed, so unobserved"),
    )
    problems, notes = [], ["planted into a copy of the guest's own configuration"]
    for symbol, line, label in plants:
        rejected = planted_rejections(context, config, symbol, line)
        problems += single_rejection_problems(context, rejected, symbol, f"{symbol} {label}")
        notes.append(f"{symbol} {label}: {rejected[0] if rejected else 'NOT rejected'}")
    return report(context, "nucleus/requirement-unsatisfied-rejected", problems, notes)


def requirement_boundary_case(context, text):
    """E10-5 boundary: m satisfies a module row, and m does not satisfy a built-in row."""
    config = config_text(text)
    decided, _rejected = requirement_check.check(context.requirement, config)
    modules = [row for row in decided if row["required"] == "module" and row["observed"] == "m"]
    problems = (
        []
        if modules and all(row["satisfied"] for row in modules)
        else ["no module row of the requirement is satisfied by m in the guest's configuration"]
    )
    rejected = planted_rejections(context, config, "CONFIG_BPF_SYSCALL", "CONFIG_BPF_SYSCALL=m")
    problems += single_rejection_problems(context, rejected, "CONFIG_BPF_SYSCALL", "built-in as m")
    inverse = planted_rejections(context, config, "CONFIG_KVM", "CONFIG_KVM=y")
    problems += single_rejection_problems(context, inverse, "CONFIG_KVM", "module as y")
    notes = [f"read in the guest: {row['symbol']}=m satisfies module" for row in modules]
    notes.append(f"planted CONFIG_BPF_SYSCALL=m: {rejected[0] if rejected else 'NOT rejected'}")
    notes.append(f"planted CONFIG_KVM=y: {inverse[0] if inverse else 'NOT rejected'}")
    return report(context, "nucleus/requirement-module-boundary", problems, notes)


def readback_phase(context):
    """Boot once to read the kernel back; return True when the load boot may follow."""
    base = context.run_dir / "readback"
    tree, _stage = stage_tree(base, "readback")
    text, nonce = guarded_boot(context, base, tree)
    if guest_field(text, "PROBE-STATUS") != "0":
        raise GateError("the guest's probe exited non-zero; see its retained report")
    guest = parse_probe(guest_section(text, "PROBE"))
    passed = guest_identity_case(context, text, nonce)
    passed = capability_case(context, guest, host_probe()) and passed
    passed = requirement_case(context, text) and passed
    passed = requirement_negative_case(context, text) and passed
    passed = requirement_boundary_case(context, text) and passed
    return passed


def bzimage_payload(data):
    """Return the compressed kernel a bzImage carries, located by its own setup header.

    The x86 boot protocol (2.08 and later) records the payload's offset and
    length after the real-mode setup; the gate reads them rather than
    searching for a compression magic.
    """
    if len(data) < 0x250 or data[0x202:0x206] != b"HdrS":
        raise GateError("the kernel image carries no x86 boot-protocol header")
    if int.from_bytes(data[0x206:0x208], "little") < 0x0208:
        raise GateError("the kernel image's boot protocol predates payload fields")
    setup_sects = data[0x1F1] or 4
    offset = (setup_sects + 1) * 512 + int.from_bytes(data[0x248:0x24C], "little")
    length = int.from_bytes(data[0x24C:0x250], "little")
    payload = data[offset : offset + length]
    if payload[:2] != b"\x1f\x8b":
        raise GateError(f"the payload starts {payload[:4].hex()}, not a gzip stream")
    return payload


def extract_vmlinux(image):
    """Return the ELF kernel inside a gzip-compressed bzImage, bounded in size."""
    payload = bzimage_payload(Path(image).read_bytes()[:MAX_IMAGE_BYTES])
    stream = zlib.decompressobj(16 + zlib.MAX_WBITS)
    elf = stream.decompress(payload, MAX_VMLINUX_BYTES)
    if elf[:4] != b"\x7fELF":
        raise GateError("the decompressed payload is not an ELF kernel")
    return elf


def generate_header(paths, image):
    """Write vmlinux.h from the kernel under test's own BTF, not the host's."""
    elf = paths["build"] / "vmlinux"
    elf.write_bytes(extract_vmlinux(image))
    code, stdout, stderr = bpf.run(
        ["bpftool", "btf", "dump", "file", str(elf), "format", "c"], timeout=BTF_TIMEOUT
    )
    elf.unlink(missing_ok=True)
    if code != 0 or "struct sched_ext_ops {" not in stdout:
        raise GateError(f"bpftool btf dump found no sched_ext BTF: exit {code}: {tail(stderr)}")
    header = paths["include"] / "vmlinux.h"
    header.write_text(stdout, encoding="utf-8")
    return header


def negative_row(source):
    """Return M19's recorded negative case for `source`."""
    return next(case for case in bpf.NEGATIVE_CASES if case[1] == source)


def build_objects(context):
    """Compile the loader and the M19 objects against the kernel under test's BTF.

    The sources, flags, mutation markers and loader are M19's; the functions
    that compile them are M19's gate's own.
    """
    build = context.run_dir / "build"
    paths = {"build": build, "scratch": build / "scratch", "include": build / "include"}
    for path in paths.values():
        path.mkdir(parents=True, exist_ok=True)
    header = generate_header(paths, context.release / context.pin["kernel"]["image"])
    context.recorded["vmlinux.h.sha256"] = file_sha256(header)
    objects = {"loader": bpf.compile_loader(paths)}
    for name in POSITIVE_OBJECTS:
        target = build / f"{name}.bpf.o"
        objects[name] = bpf.compile_object(
            bpf.BPF_DIR / f"{name}.bpf.c", target, (bpf.BPF_DIR,), paths
        )
    for source in NEGATIVE_SOURCES:
        _name, _source, region, _mode, _rejection = negative_row(source)
        mutated, removed = bpf.mutate(source, region, paths)
        target = build / f"{source}_negative.bpf.o"
        objects[f"{source}_negative"] = bpf.compile_object(
            mutated, target, (paths["scratch"], bpf.BPF_DIR), paths
        )
        context.recorded[f"{source}.removed_lines"] = removed
    return objects


def guest_capability_argv():
    """The M19 capability wrapper inside the guest: the same setpriv flags, uid 65534.

    On the host M19 runs setpriv through sudo; PID 1 is already root, so only
    the privilege drop remains, to an unprivileged uid with exactly
    CAPABILITIES.
    """
    return [
        "/bin/setpriv",
        f"--reuid={GUEST_UID}",
        f"--regid={GUEST_UID}",
        "--clear-groups",
        f"--bounding-set={bpf.CAPABILITIES}",
        f"--inh-caps={bpf.CAPABILITIES}",
        f"--ambient-caps={bpf.CAPABILITIES}",
        "--",
    ]


def cake_tiers():
    """Return AEGIS_CAKE_TIERS, read out of the ABI header scx_cake is compiled with."""
    header = (bpf.BPF_DIR / "aegis_bpf_abi.h").read_text(encoding="utf-8")
    match = re.search(r"^#define AEGIS_CAKE_TIERS (\d+)u$", header, re.M)
    if match is None:
        raise GateError("AEGIS_CAKE_TIERS is not defined in bpf/aegis_bpf_abi.h")
    return int(match.group(1))


def guest_cases():
    """Return (case, function, object, mode, extra arguments) in the order the guest runs them."""
    return (
        (CASE_LSM, "aegis_case", "action_gate", "lsm-probe", ("--marker", MARKER)),
        (
            CASE_LSM_NEGATIVE,
            "aegis_case",
            "action_gate_negative",
            negative_row("action_gate")[3],
            (),
        ),
        (
            CASE_SOPS,
            "aegis_case",
            "scx_cake",
            "sops-attach",
            ("--hold-ms", ATTACH_HOLD_MS, "--map-set", f"{CAKE_BUDGET_MAP}={cake_tiers()}"),
        ),
        (CASE_SOPS_NEGATIVE, "aegis_case", "scx_cake_negative", negative_row("scx_cake")[3], ()),
        (
            CASE_TP,
            "aegis_idle_case",
            "kepler_power",
            "tp-attach",
            ("--settle-ms", IDLE_SETTLE_MS, "--hold-ms", IDLE_HOLD_MS),
        ),
        (
            CASE_TP_MISSING,
            "aegis_case",
            "kepler_power",
            "tp-attach",
            ("--tracepoint", MISSING_TRACEPOINT),
        ),
    )


def cases_script(context):
    """Return the case list the load guest sources, one capability-wrapped load per line."""
    lines = ["# Written by tools/verify_nucleus_kernel.py; sourced by the M10 guest's PID 1."]
    for case, function, obj, mode, extra in guest_cases():
        nonce = new_nonce()
        context.nonces[case] = nonce
        log = f"/{GUEST_STAGE}/logs/{case}.log"
        argv = [
            *guest_capability_argv(),
            f"/bin/{GUEST_LOADER}",
            "--object",
            f"/{GUEST_STAGE}/objects/{obj}.bpf.o",
            "--log",
            log,
            "--mode",
            mode,
            "--nonce",
            nonce,
            "--deadline",
            LOAD_DEADLINE,
            *extra,
        ]
        lines.append(" ".join(shlex.quote(part) for part in [function, case, log, *argv]))
    return "\n".join(lines) + "\n"


def stage_load(context, objects, base):
    """Lay out the load guest: the loader, the objects, the marker and the case list."""
    tree, stage = stage_tree(base, "load", objects["loader"])
    (stage / "objects").mkdir()
    for name, path in objects.items():
        if name != "loader":
            shutil.copy2(path, stage / "objects" / f"{name}.bpf.o")
    (stage / "logs").mkdir()
    (stage / "logs").chmod(0o777)
    marker = stage / bpf.MARKER_NAME
    shutil.copy2(tree / "usr" / "bin" / "true", marker)
    marker.chmod(0o755)
    checked = context.recorded["guest.config_sha256"]
    (stage / "expected-config-sha256").write_text(f"{checked}\n", encoding="utf-8")
    (stage / "cases.sh").write_text(cases_script(context), encoding="utf-8")
    return tree


def decode_log(lines):
    """Return (verifier log text, problem) from the gzip+base64 lines the guest printed."""
    if lines == [f"{GUEST_MARK}-LOG-ABSENT"]:
        return None, "the guest found no verifier log for this case"
    try:
        packed = base64.b64decode("".join(lines), validate=True)
        stream = zlib.decompressobj(16 + zlib.MAX_WBITS)
        return stream.decompress(packed, MAX_LOG_BYTES).decode("utf-8", errors="replace"), None
    except (binascii.Error, zlib.error) as error:
        return None, f"the retained log did not decode: {error}"


def guest_case(context, text, case):
    """Return {output, status, log, problem} for one case the load guest framed."""
    lines = guest_lines(text)
    begin = f"{GUEST_MARK}-CASE-BEGIN {case}"
    status_prefix = f"{GUEST_MARK}-CASE-STATUS {case} "
    status = [line for line in lines if line.startswith(status_prefix)]
    if begin not in lines or len(status) != 1:
        raise GateError(f"the guest did not frame case {case}; see its retained report")
    output = lines[lines.index(begin) + 1 : lines.index(status[0])]
    log, problem = decode_log(guest_section(text, "LOG", case))
    if log is not None:
        (context.run_dir / "load" / "logs").mkdir(parents=True, exist_ok=True)
        (context.run_dir / "load" / "logs" / f"{case}.log").write_text(log, encoding="utf-8")
    return {
        "output": "\n".join(output),
        "status": int(status[0][len(status_prefix) :]),
        "log": log,
        "problem": problem,
    }


def stamped(context, result, case):
    """Return why a case's log is not the one this load wrote, or []."""
    if result["log"] is None:
        return [result["problem"]]
    if f"{bpf.NONCE_HEADER} {context.nonces[case]}" not in result["log"]:
        return [f"the log does not carry this load's nonce {context.nonces[case]}"]
    return []


def expected_lines(result, lines):
    """Return one problem per line the loader's output was required to print and did not."""
    return [f"the loader did not print {line!r}" for line in lines if line not in result["output"]]


def lsm_case(context, results):
    """E10-1 positive: action_gate loads, attaches to the LSM hook and sees a real exec."""
    result = results[CASE_LSM]
    problems = stamped(context, result, CASE_LSM)
    if result["status"] != 0:
        problems.append(f"the probe exited {result['status']}")
    problems += expected_lines(
        result, ("load_rc=0", "attached=lsm", f"filename={MARKER}", "detached=lsm marker_seen=1")
    )
    problems += lsm_attach_reason(context, result)
    events = [line for line in result["output"].splitlines() if line.startswith("event ")]
    notes = [
        f"nonce {context.nonces[CASE_LSM]}",
        *events[:2],
        "uid 65534, capabilities " + bpf.CAPABILITIES,
    ]
    return report(context, "action_gate/nucleus-loads-and-attaches", problems, notes)


def lsm_attach_reason(context, result):
    """Return the recorded reason an LSM attach failed with -EBUSY, from the guest's own config.

    Stated only when both halves were observed in this run: the loader's
    -EBUSY, and a guest configuration without the trampoline prerequisites.
    """
    if "-EBUSY" not in result["output"]:
        return []
    states = {
        symbol: trampoline_state(context.guest_probe, symbol) for symbol in TRAMPOLINE_SYMBOLS
    }
    if all(state == "y" for state in states.values()):
        return []
    read = ", ".join(f"{symbol} {state}" for symbol, state in states.items())
    return [
        "recorded reason: the BPF trampoline could not patch the LSM hook's entry (-EBUSY), and "
        f"the kernel under test reads {read}; CONFIG_BPF_LSM=y does not provide the -mfentry "
        "nop the trampoline patches"
    ]


def negative_case(context, results, case, control, source):
    """E10-1 and E10-2 negatives: the mutated object is rejected by the verifier log."""
    name, _source, region, _mode, rejection = negative_row(source)
    result = results[case]
    problems = stamped(context, result, case)
    if result["status"] != bpf.PROBE_EXIT_LOAD_FAILED:
        problems.append(f"the probe exited {result['status']}, not {bpf.PROBE_EXIT_LOAD_FAILED}")
    problems += bpf.rejection_problems(result["log"], rejection)
    control_log = results[control]["log"] or ""
    if rejection in control_log:
        problems.append(f"the unmutated object's log carries {rejection!r} too")
    found = [line for line in bpf.diagnostics(result["log"]) if rejection in line]
    removed = context.recorded.get(f"{source}.removed_lines")
    notes = [
        f"M19's {name}: region {region} removed ({removed} lines)",
        f"verifier: {found[0] if found else 'no rejection line'}",
        f"nonce {context.nonces[case]}; the unmutated object's log does not carry it",
    ]
    title = "unchecked-pointer" if source == "action_gate" else "unbounded"
    return report(context, f"{source}/nucleus-{title}-rejected", problems, notes)


def sops_case(context, results):
    """E10-2 positive: scx_cake's struct_ops attaches and holds root/ops in the guest."""
    result = results[CASE_SOPS]
    problems = stamped(context, result, CASE_SOPS)
    if result["status"] != 0:
        problems.append(f"the probe exited {result['status']}")
    budget = f"map_set={CAKE_BUDGET_MAP} value={cake_tiers()}"
    problems += expected_lines(
        result,
        (
            "load_rc=0",
            budget,
            "attached=struct_ops",
            "sched_ext_ops_during=aegis_cake",
            "sched_ext_ops_still_attached=aegis_cake",
            "detach_rc=0",
        ),
    )
    disabled = scheduler_exits(context)
    if disabled != [SCHEDULER_UNREGISTERED]:
        problems.append(f"the kernel reported {disabled}, not one user-space unregistration")
    notes = [
        f"nonce {context.nonces[CASE_SOPS]}; {budget}",
        *bpf.probe_counter_lines(result["output"]),
    ]
    notes.append(f"guest console: {disabled[0] if disabled else 'no disable line'}")
    notes.append("every guest task was scheduled by aegis_cake for the hold window")
    return report(context, "scx_cake/nucleus-struct-ops-attaches", problems, notes)


def scheduler_exits(context):
    """Return the guest console's sched_ext disable lines for aegis_cake, timestamps dropped."""
    console = context.run_dir / "load" / "guest" / "console.log"
    lines = console.read_text(encoding="utf-8", errors="replace").splitlines()[:MAX_REPORT_LINES]
    marker = 'sched_ext: BPF scheduler "aegis_cake" disabled'
    return [line[line.index(marker) :] for line in lines if marker in line]


def power_rows(output):
    """Return {(point, cgroup id): (runtime ns, pseudo-energy uJ)} from the loader's output."""
    rows = {}
    for line in output.splitlines()[:MAX_POWER_ROWS]:
        match = POWER_ROW.match(line.strip())
        if match is not None:
            point, cgroup, runtime, energy = match.groups()
            rows[(point, int(cgroup))] = (int(runtime), int(energy))
    return rows


def tracepoint_case(context, results):
    """E10-3 positive: the tracepoint attaches and the accounting moves while attached."""
    result = results[CASE_TP]
    problems = stamped(context, result, CASE_TP)
    if result["status"] != 0:
        problems.append(f"the probe exited {result['status']}")
    problems += expected_lines(
        result, ("load_rc=0", f"attached=tracepoint tracepoint={KEPLER_TRACEPOINT}", "detach_rc=0")
    )
    rows = power_rows(result["output"])
    moved = [
        cgroup
        for (point, cgroup), (runtime, _e) in rows.items()
        if point == "end" and runtime > rows.get(("begin", cgroup), (runtime, 0))[0]
    ]
    if not moved:
        problems.append("no cgroup's accounted runtime grew while the tracepoint was attached")
    notes = [
        f"nonce {context.nonces[CASE_TP]}; {len(rows)} readings; runtime grew in cgroups {moved}"
    ]
    return report(context, "kepler_power/nucleus-tracepoint-attaches", problems, notes)


def missing_tracepoint_case(context, results):
    """E10-3 negative: a tracepoint the kernel lacks is reported with its errno, not skipped."""
    result = results[CASE_TP_MISSING]
    problems = []
    prefix = f"tracepoint_attach_failed tracepoint={MISSING_TRACEPOINT} errno="
    reported = [line for line in result["output"].splitlines() if line.startswith(prefix)]
    if result["status"] != PROBE_EXIT_ATTACH_FAILED:
        problems.append(f"the probe exited {result['status']}, not {PROBE_EXIT_ATTACH_FAILED}")
    if not reported or reported[0].startswith(f"{prefix}0 "):
        problems.append(f"the loader printed no {prefix}<errno> line")
    problems += expected_lines(result, ("load_rc=0",))
    notes = [reported[0] if reported else "no report line", "the object loaded; the attach failed"]
    return report(context, "kepler_power/nucleus-missing-tracepoint-reported", problems, notes)


def idle_case(context, text, results):
    """E10-3 boundary: an idle cgroup's delta over the interval is a recorded zero."""
    result = results[CASE_TP]
    cgroup = int(guest_field(text, f"IDLE-CGROUP {CASE_TP}"))
    rows = power_rows(result["output"])
    begin, end = rows.get(("begin", cgroup)), rows.get(("end", cgroup))
    problems = []
    if begin is None or end is None:
        problems.append(f"cgroup {cgroup} has no reading at both ends of the interval")
        begin = end = (0, 0)
    delta = (end[0] - begin[0], end[1] - begin[1])
    if delta != (0, 0):
        problems.append(f"the idle cgroup moved by {delta}; the interval was not idle")
    context.recorded["kepler.idle_delta"] = {
        "runtime_ns": delta[0],
        "pseudo_energy_uj_unmeasured": delta[1],
    }
    notes = [
        f"idle cgroup {cgroup}: pseudo_energy_uj_unmeasured {begin[1]} -> {end[1]}, delta "
        f"{delta[1]} uJ recorded as a value; runtime delta {delta[0]} ns",
        f"interval {IDLE_HOLD_MS} ms inside the attach window, after {IDLE_SETTLE_MS} ms settle",
        "the value is the object's declared runtime-times-TDP figure, not a meter reading",
    ]
    return report(context, "kepler_power/nucleus-idle-zero-delta-recorded", problems, notes)


def load_phase(context):
    """Build the objects, boot the load guest and judge every M19 load on the Nucleus kernel."""
    objects = build_objects(context)
    base = context.run_dir / "load"
    tree = stage_load(context, objects, base)
    text, nonce = guarded_boot(context, base, tree)
    problems, release = identity_problems(context, text, nonce)
    check = guest_field(text, "CONFIG-CHECK")
    if check != "match":
        problems.append(f"the guest's configuration check read {check!r}; nothing was loaded")
    report(context, "nucleus/load-guest-config-matches-checked", problems, [f"{release}: {check}"])
    if problems:
        return
    results = {case[0]: guest_case(context, text, case[0]) for case in guest_cases()}
    lsm_case(context, results)
    negative_case(context, results, CASE_LSM_NEGATIVE, CASE_LSM, "action_gate")
    sops_case(context, results)
    negative_case(context, results, CASE_SOPS_NEGATIVE, CASE_SOPS, "scx_cake")
    tracepoint_case(context, results)
    missing_tracepoint_case(context, results)
    idle_case(context, text, results)


def bound_reasons(name, found, floor):
    """Return why version `found` of `name` is outside its admitted range, or []."""
    version = harness.version_tuple(found)
    if floor is not None and version < harness.version_tuple(floor):
        return [f"{name} {found} is below the admitted floor {floor}"]
    ceiling = CEILINGS.get(name)
    if ceiling is not None and version >= harness.version_tuple(ceiling):
        return [
            f"{name} {found} is not admitted; the admitted range is {floor} up to below {ceiling}"
        ]
    return []


def check_toolchain(rows):
    """Print each admitted tool read back from the host; return why the gate cannot run."""
    missing = [row[1][0] for row in rows if shutil.which(row[1][0]) is None]
    if missing:
        return [f"{name} is not on PATH" for name in missing]
    reasons = []
    for row in rows:
        name, _argv, _pattern, floor, reference = row
        banner, found = harness.read_version(row)
        if found is None:
            reasons.append(banner)
            continue
        limit = "" if floor is None else f", floor {floor}"
        limit += f", below {CEILINGS[name]}" if name in CEILINGS else ""
        note = "" if found == reference else f" [reference profile recorded {reference}]"
        print(f"     {name}: {found} (read back from {banner!r}{limit}){note}")
        reasons.extend(bound_reasons(name, found, floor))
    return reasons


def host_reasons(cache):
    """Return why this host cannot run the gate at all, without invoking any tool."""
    if not sys.platform.startswith("linux"):
        return [f"this host is {sys.platform}; the guest boots under Linux KVM only"]
    reasons = []
    if not os.access(KVM, os.R_OK | os.W_OK):
        reasons.append(
            f"{linux_path(KVM)} is absent or not read-write; M10 boots under KVM with no fallback"
        )
    if not HOST_CONFIG.exists():
        reasons.append(
            f"{linux_path(HOST_CONFIG)} does not exist; the host half of the probe cannot run"
        )
    store = contract_store()
    absent = [str(path) for path in store.values() if not path.is_file()]
    if absent:
        reasons.append(f"M09's contract cache lacks {absent[0]}; run `make contract-fetch`")
    release = cache / "release"
    if not all((release / row["name"]).exists() for row in asset_rows(load_pin())):
        reasons.append(
            f"the pinned release is not cached under {release}; run `make nucleus-kernel-fetch`"
        )
    return reasons


def fetch_asset(pin, release, row):
    """Download one pinned asset unless the cached copy already hashes to the pin."""
    target = release / row["name"]
    if target.exists() and not file_problems(target, row):
        return f"{row['name']}: cached, as pinned"
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
        f"{pin['download']}/{row['name']}",
    ]
    code, _stdout, stderr = harness.run(argv, timeout=DOWNLOAD_TIMEOUT + VERSION_TIMEOUT)
    if code != 0:
        raise GateError(f"curl exited {code} for {row['name']}: {tail(stderr)}")
    problems = file_problems(partial, row)
    if problems:
        partial.unlink(missing_ok=True)
        raise GateError(f"refused the download: {'; '.join(problems)}")
    partial.replace(target)
    return f"{row['name']}: downloaded, {row['size']} bytes, sha256 as pinned"


def fetch_reasons():
    """Return why the fetch cannot run here: a tool missing, or cosign outside its range."""
    missing = [f"{tool} is not on PATH" for tool in FETCH_TOOLS if shutil.which(tool) is None]
    return missing or check_toolchain([row for row in TOOLCHAIN if row[0] == "cosign"])


def fetch(cache):
    """The one networked step: download, hash against the pin, and verify the signature."""
    pin = load_pin()
    reasons = fetch_reasons()
    if reasons:
        for reason in reasons[:MAX_PROBLEM_LINES]:
            print(f"SKIP: {reason}; the Nucleus kernel fetch did not run.")
        return 0
    release = cache / "release"
    release.mkdir(parents=True, exist_ok=True)
    lines = [fetch_asset(pin, release, row) for row in asset_rows(pin)]
    argv = cosign_argv(release, pin, pin["signer"]["identity"], offline=False)
    code, stdout, stderr = harness.run(argv, timeout=COSIGN_TIMEOUT)
    if code != 0 or "Verified OK" not in stdout + stderr:
        raise GateError(f"cosign verify-blob refused SHA256SUMS: {tail(stdout + stderr)}")
    record = {
        "schema": "aegis.m10.nucleus-fetch.v1",
        "fetched": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "tag": pin["tag"],
        "assets": lines,
        "cosign": {"argv": argv, "exit": code, "output": tail(stdout + stderr)},
    }
    (cache / "fetch.json").write_text(json.dumps(record, indent=2) + "\n", encoding="utf-8")
    for line in lines:
        print(f"     {line}")
    print(f"PASS: {pin['tag']} fetched into {release}; cosign verified SHA256SUMS.")
    return 0


def summary(context):
    """Write the run's outcomes and recorded values next to its logs."""
    record = {
        "schema": "aegis.m10.nucleus-run.v1",
        "run": context.run_id,
        "tag": context.pin["tag"],
        "recorded": context.recorded,
        "outcomes": context.outcomes,
    }
    target = context.run_dir / "summary.json"
    target.write_text(json.dumps(record, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    return [outcome["case"] for outcome in context.outcomes if outcome["problems"]]


def record_release(context):
    """Write E10-4's record into the run directory; called before any guest can boot.

    `summary.json` is written when the run ends. This file is written when
    the release stage ends, so the values imago returned are on disk before
    the first boot starts, whatever the boots do afterwards.
    """
    record = {
        "schema": "aegis.m10.nucleus-release-record.v1",
        "run": context.run_id,
        "written": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
        "tag": context.pin["tag"],
        "recorded": context.recorded,
        "outcomes": context.outcomes,
    }
    target = context.run_dir / RELEASE_RECORD
    target.write_text(json.dumps(record, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")
    print(f"     E10-4 record written before any boot: {target}")
    return target


def release_stage(context):
    """E10-4: verify the release and record it; return True when a guest may boot.

    Every case of the stage runs, negatives and the boundary included, and a
    guest boots only when all of them passed.
    """
    if not assets_case(context):
        return False
    passed = signature_case(context)
    passed = imago_case(context) and passed
    passed = mismatch_case(context) and passed
    passed = release_case(context) and passed
    record_release(context)
    return passed


def run_cases(context):
    """Run every case in the contract's order; each stage only after the one before passed."""
    if not release_stage(context):
        print("     the release did not verify, so no guest was booted")
        return
    if not readback_phase(context):
        print("     the readback checks did not pass, so no M19 object was loaded")
        return
    load_phase(context)


def start(cache):
    """Guard the host, then run the cases; return the exit code."""
    reasons = host_reasons(cache)
    if not reasons:
        reasons = check_toolchain(TOOLCHAIN)
    if reasons:
        for reason in reasons[:MAX_PROBLEM_LINES]:
            print(f"SKIP: {reason}; {SKIP_LINE}")
        return 0
    run_id = time.strftime("r%Y%m%dT%H%M%S") + f"-{secrets.token_hex(2)}"
    context = Context(load_pin(), cache, run_id)
    context.run_dir.mkdir(parents=True, exist_ok=True)
    print(f"     run {run_id}; logs under {context.run_dir}")
    try:
        run_cases(context)
    finally:
        failed = summary(context)
    if failed:
        print(f"FAIL: {len(failed)} Nucleus kernel case(s) did not match: {', '.join(failed)}")
        return 1
    print(
        f"PASS: run {run_id}. The Nucleus kernel {context.pin['kernel']['release']} "
        f"({context.pin['tag']}, {context.pin['revision'][:12]}) was verified and recorded before "
        "it booted, satisfied the Aegis kernel requirement in the guest before any load, and "
        "loaded and attached the M19 objects under the verifier. Development evidence on the "
        "reference profile: the M19 host fixture does not close M10, and neither the hardware "
        "nor the release gate is closed."
    )
    return 0


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument(
        "--fetch", action="store_true", help="download and verify the pinned release"
    )
    args = parser.parse_args(argv)
    cache = cache_dir()
    print(f"Nucleus kernel gate (M10, D92, D94, development evidence only). Cache: {cache}")
    try:
        if args.fetch:
            return fetch(cache)
        return start(cache)
    except GATE_ERRORS as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1


if __name__ == "__main__":
    sys.exit(main())
