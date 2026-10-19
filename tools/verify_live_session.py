#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Boot a pinned distribution live guest under QEMU with KVM and read its session services back.

Milestone M29, epic E29-4. The guest is an upstream live image named by
``build/session/guest.pin.json``; Aegis boots it and constructs nothing
(docs/integration/stack.md). Its bytes are cached outside the repository,
verified against the clearsigned CHECKSUM with M24's code, and hashed against
the pin immediately before the boot and again after it.

The guest boots straight from its own kernel and initramfs, which xorriso reads
out of the verified ISO, so the command line is the harness's. The read-back
script ``tools/guest/aegis-session-report.sh``, the unit that runs it and the
list of services arrive as SMBIOS type 11 systemd credentials; the image's own
bytes are never changed. The script waits for the live user's session, probes
each service on the session bus (and the at-spi2 registry on the accessibility
bus), reads back the package version of the process that owns each name, and
writes its report with this boot's nonce to the second serial line.

Cases, with no simulation and no failure suppression:

* ``session/guest-verified``: signature, signed digest, producer version and
  bytes, before any boot;
* ``session/sha256-mismatch-refused``: a pin whose sha256 differs from the
  image is refused before any program starts or any file is written;
* ``session/positive``: the guest boots under KVM, and the compositor, the
  portal and its backend, the accessibility bus, the at-spi2 registry and the
  StatusNotifierWatcher each answer on their bus with a package version, the
  compositor advertises every required Wayland global, and the portal answers
  a Settings read;
* ``session/service-deadline-boundary``: a planted name nobody owns is probed
  under a short deadline in the same guest and must fail its case by name;
* ``session/pinned-bytes-unchanged``: the cached image hashes to the pin after
  the boot.

A host that cannot run it prints ``SKIP: <reason>; the live session gate did
not run.`` and exits 0. The cache and the retained logs live under
``AEGIS_SESSION_DIR`` (default ``${XDG_CACHE_HOME:-$HOME/.cache}/aegis-session``).
A pass is development evidence in a VM on the reference profile; the guest's
compositor is not P04 (D79's client-half rule), and it closes no hardware,
accessibility or release gate.
"""

import argparse
import base64
import json
import re
import secrets
import shutil
import sys
import time
from pathlib import Path

import verify_boot_harness as harness
import gate_common

ROOT = Path(__file__).resolve().parent.parent
PIN = ROOT / "build" / "session" / "guest.pin.json"
REPORT_SCRIPT = ROOT / "tools" / "guest" / "aegis-session-report.sh"
PIN_SCHEMA = "aegis.m29.session-guest-pin.v1"
MEMINFO = Path("/proc/meminfo")

GUEST_MARK = "AEGIS-M29"
NONCE_SHAPE = re.compile(r"aegis-m29-[0-9a-f]{32}")
UNIT_NAME = "aegis-session-report.service"
SCRIPT_CREDENTIAL = "aegis.session-report.script"
SERVICES_CREDENTIAL = "aegis.session-report.services"
PLACEHOLDERS = ("@AEGIS_NONCE@", "@AEGIS_LIVE_USER@", "@AEGIS_SESSION_DEADLINE@")
BUSES = ("session", "a11y")
BUS_NAME = re.compile(r"[A-Za-z_][A-Za-z0-9_-]*(?:\.[A-Za-z_][A-Za-z0-9_-]*)+")
LABEL = re.compile(r"[a-z][a-z0-9-]{0,40}")
USER = re.compile(r"[a-z_][a-z0-9_-]{0,31}")
VOLUME_ID = re.compile(r"[A-Za-z0-9._-]{1,32}")
ISO_PATH = re.compile(r"(?:[A-Za-z0-9][A-Za-z0-9._-]*/){1,4}[A-Za-z0-9][A-Za-z0-9._-]*")
GLOBAL = re.compile(r"[a-z][a-z0-9_]{0,80}")

# A name nobody in the guest owns, probed in the same boot under a short
# deadline: the boundary case requires the gate to fail it and to name it.
PLANTED_ABSENT = {
    "label": "planted-absent",
    "bus": "session",
    "name": "org.aegis.M29.PlantedAbsent",
    "deadline-seconds": 3,
}

# The tools this gate runs, read back through M24's admission rows: QEMU,
# xorriso for the kernel and initramfs, gpg and curl for the signed CHECKSUM.
TOOLS = ("qemu-system-x86_64", "xorriso", "gpg", "curl")

MIN_MEM_AVAILABLE_KIB = 12 << 20
MIN_SWAP_FREE_KIB = 15 << 20
MAX_DEADLINE = 1800
MAX_SERVICES = 16
MAX_GLOBALS = 128
MAX_REPORT_LINES = 400
MAX_PROBLEM_LINES = 24
MAX_MEMINFO_LINES = 128
MAX_RETAINED_RUNS = 8
GUEST_MEMORY_RANGE = (2048, 16384)
GUEST_CPU_RANGE = (1, 16)


class Context:
    """One run: the pin, the cache, this run's directory and every process it started."""

    def __init__(self, pin, store, run_dir):
        self.pin = pin
        self.store = store
        self.run_dir = run_dir
        self.artifact = store / pin["file"]
        self.checksum = store / pin["signature"]["checksum-file"]
        self.keyring = store / "gnupg"
        self.spawned = []

    def nonce(self):
        """Return a fresh per-boot nonce."""
        return f"aegis-m29-{secrets.token_hex(harness.NONCE_BYTES)}"


class Boot:
    """The files one boot reads and writes, all under its own directory."""

    def __init__(self, directory):
        self.dir = directory
        self.kernel = directory / "vmlinuz"
        self.initrd = directory / "initrd.img"
        self.console = directory / "console.log"
        self.report = directory / "report.txt"


def pin_shape_problems(pin):
    """Return the fields a pin lacks, section by section."""
    top = ("schema", "name", "version", "version-floor", "architecture", "format", "file")
    top += ("url", "sha256", "size", "signature", "boot", "services", "required-globals")
    boot = ("kernel", "initrd", "volume-id", "live-user", "memory-mib", "cpus")
    boot += ("session-deadline-seconds", "report-deadline-seconds", "shutdown-deadline-seconds")
    problems = [f"missing {key}" for key in top if key not in pin]
    if problems:
        return problems
    sig, fields = pin["signature"], harness.SIGNATURE_FIELDS[harness.GPG_SCHEME]
    if not isinstance(sig, dict) or not isinstance(pin["boot"], dict):
        return ["signature and boot must be objects"]
    problems += [f"signature lacks {key}" for key in fields if key not in sig]
    problems += [f"boot lacks {key}" for key in boot if key not in pin["boot"]]
    return problems


def deadline_ok(value):
    """Return whether `value` is a whole number of seconds the gate accepts as a deadline."""
    return isinstance(value, int) and not isinstance(value, bool) and 0 < value <= MAX_DEADLINE


def in_range(value, bounds):
    """Return whether `value` is an integer within the inclusive `bounds`."""
    return isinstance(value, int) and bounds[0] <= value <= bounds[1]


def service_problems(service):
    """Return why one service entry cannot be probed, or []."""
    if not isinstance(service, dict):
        return ["a service entry is not an object"]
    label = str(service.get("label"))
    checks = (
        (LABEL.fullmatch(label), f"service label {label!r} is not a plain label"),
        (service.get("bus") in BUSES, f"{label}: bus is not one of {', '.join(BUSES)}"),
        (BUS_NAME.fullmatch(str(service.get("name"))), f"{label}: name is not a bus name"),
        (deadline_ok(service.get("deadline-seconds")), f"{label}: deadline out of range"),
    )
    return [message for passed, message in checks if not passed]


def services_problems(services):
    """Return why the service list cannot be probed, or []."""
    if not isinstance(services, list) or not 0 < len(services) < MAX_SERVICES:
        return [f"services must list 1 to {MAX_SERVICES - 1} entries"]
    problems = [line for service in services for line in service_problems(service)]
    labels = [service.get("label") for service in services if isinstance(service, dict)]
    if len(set(labels)) != len(labels) or PLANTED_ABSENT["label"] in labels:
        problems.append("service labels must be unique and must not reuse the planted label")
    return problems


def boot_problems(boot):
    """Return the boot values this gate refuses to act on."""
    deadlines = ("session-deadline-seconds", "report-deadline-seconds", "shutdown-deadline-seconds")
    checks = (
        (ISO_PATH.fullmatch(str(boot["kernel"])), "kernel is not a relative ISO path"),
        (ISO_PATH.fullmatch(str(boot["initrd"])), "initrd is not a relative ISO path"),
        (VOLUME_ID.fullmatch(str(boot["volume-id"])), "volume-id is not a plain label"),
        (USER.fullmatch(str(boot["live-user"])), "live-user is not a plain user name"),
        (in_range(boot["memory-mib"], GUEST_MEMORY_RANGE), "memory-mib out of range"),
        (in_range(boot["cpus"], GUEST_CPU_RANGE), "cpus out of range"),
        (all(deadline_ok(boot[key]) for key in deadlines), "a boot deadline is out of range"),
    )
    return [message for passed, message in checks if not passed]


def source_problems(pin):
    """Return why the pin's URLs or required globals cannot be acted on, or []."""
    sig, globals_ = pin["signature"], pin["required-globals"]
    problems = []
    urls = (pin["url"], sig.get("checksum-url", ""), sig.get("key-url", ""))
    if not all(str(url).startswith("https://") for url in urls):
        problems.append("every URL must be https")
    if isinstance(globals_, list) and not all(GLOBAL.fullmatch(str(g)) for g in globals_):
        problems.append("a required global is not a Wayland interface name")
    return problems


def pin_value_problems(pin):
    """Return the pin values this gate refuses to act on."""
    sig = pin["signature"]
    expected_file = f"{pin['name']}-{pin['version']}.{pin['architecture']}.{pin['format']}"
    globals_ = pin["required-globals"]
    checks = (
        (pin["schema"] == PIN_SCHEMA, f"schema is not {PIN_SCHEMA}"),
        (sig.get("scheme") == harness.GPG_SCHEME, f"signature scheme is not {harness.GPG_SCHEME}"),
        (harness.PCR_VALUE.fullmatch(str(pin["sha256"])), "sha256 is not 64 lower-case hex digits"),
        (pin["format"] == "iso", "format must be iso"),
        (pin["file"] == expected_file, f"file is not {expected_file}"),
        (harness.VERSION.fullmatch(str(pin["version"])), "version is not MAJOR-MINOR.RESPIN"),
        (harness.VERSION.fullmatch(str(pin["version-floor"])), "version-floor is malformed"),
        (isinstance(pin["size"], int) and pin["size"] > 0, "size is not a positive integer"),
        (isinstance(globals_, list) and 0 < len(globals_) <= MAX_GLOBALS, "no required globals"),
    )
    problems = [message for passed, message in checks if not passed]
    problems += source_problems(pin)
    if not problems:
        problems = harness.gpg_signature_problems(sig)
    return problems + boot_problems(pin["boot"]) + services_problems(pin["services"])


def load_pin(path):
    """Read and validate the guest pin; refuse it whole rather than use part of it."""
    try:
        pin = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise harness.GateError(f"the pin {path} could not be read: {error}") from error
    if not isinstance(pin, dict):
        raise harness.GateError(f"the pin {path} is not a JSON object")
    problems = pin_shape_problems(pin)
    if not problems:
        problems = pin_value_problems(pin)
    if problems:
        raise harness.GateError(f"the pin {path} is refused: {'; '.join(problems)}")
    return pin


def cache_dir():
    """Return the out-of-repository directory the image and the run logs live under."""
    return gate_common.cache_dir("AEGIS_SESSION_DIR", "aegis-session")


def meminfo(text):
    """Return /proc/meminfo text as {field: kibibytes}, bounded in lines."""
    fields = {}
    for line in text.splitlines()[:MAX_MEMINFO_LINES]:
        name, _colon, rest = line.partition(":")
        parts = rest.split()
        if parts and parts[0].isdigit():
            fields[name.strip()] = int(parts[0])
    return fields


def memory_reasons(fields):
    """Return why the host has too little memory to boot the guest now, or []."""
    wanted = (("MemAvailable", MIN_MEM_AVAILABLE_KIB), ("SwapFree", MIN_SWAP_FREE_KIB))
    reasons = []
    for name, floor in wanted:
        found = fields.get(name)
        if found is None:
            reasons.append(f"{MEMINFO} reports no {name}")
        elif found < floor:
            reasons.append(
                f"{name} is {found >> 20} GiB, below the {floor >> 20} GiB the boot needs"
            )
    return reasons


def check_memory():
    """Return why the host's memory pool cannot take the guest now; POSIX /proc only."""
    try:
        text = MEMINFO.read_text(encoding="utf-8")
    except OSError as error:
        return [f"{MEMINFO} could not be read ({error.strerror}); memory headroom is unknown"]
    return memory_reasons(meminfo(text))


def check_toolchain():
    """Print the M24-admitted tools this gate runs, read back; return why it cannot run.

    The same rule as M24's own check: a floor refuses an older tool, and a
    version other than the reference profile's is printed, not refused.
    """
    reasons = []
    for row in (row for row in harness.TOOLCHAIN if row[0] in TOOLS):
        name, _argv, _pattern, floor, reference = row
        banner, found = harness.read_version(row)
        if found is None:
            reasons.append(banner)
            continue
        limit = "" if floor is None else f", floor {floor}"
        note = "" if found == reference else f" [reference profile recorded {reference}]"
        print(f"     {name}: {found} (read back from {banner!r}{limit}){note}")
        if floor is not None and harness.version_tuple(found) < harness.version_tuple(floor):
            reasons.append(f"{name} {found} is below the admitted floor {floor}")
    return reasons


def check_cache(context):
    """Return why the cached inputs are not all present."""
    wanted = (context.artifact, context.checksum, context.keyring)
    missing = [str(path) for path in wanted if not path.exists()]
    if not missing:
        return []
    return [
        f"{', '.join(missing)} not cached; run `python3 tools/verify_live_session.py --fetch` "
        "to download and verify the pinned guest image"
    ]


def report_script(nonce, pin):
    """Return the in-guest read-back with this boot's values in place of its placeholders."""
    if NONCE_SHAPE.fullmatch(nonce) is None:
        raise harness.GateError(f"{nonce!r} is not a nonce this gate generated")
    text = REPORT_SCRIPT.read_text(encoding="utf-8")
    values = (nonce, pin["boot"]["live-user"], str(pin["boot"]["session-deadline-seconds"]))
    for placeholder, value in zip(PLACEHOLDERS, values):
        if text.count(placeholder) != 1:
            raise harness.GateError(f"{REPORT_SCRIPT.name} must hold {placeholder} exactly once")
        text = text.replace(placeholder, value)
    return text


def probed_services(pin):
    """Return the services the guest probes: the pinned ones, then the planted absent one."""
    return list(pin["services"]) + [PLANTED_ABSENT]


def services_text(services):
    """Return the service list the guest script reads, one `label bus name deadline` per line."""
    rows = [f"{s['label']} {s['bus']} {s['name']} {s['deadline-seconds']}" for s in services]
    return "\n".join(rows) + "\n"


def unit_text(pin):
    """Return the unit the guest installs from its credential and the command line pulls in."""
    deadline = pin["boot"]["report-deadline-seconds"]
    return (
        "[Unit]\n"
        "Description=Aegis M29 live session read-back\n"
        "After=graphical.target\n"
        "[Service]\n"
        "Type=oneshot\n"
        f"ImportCredential={SCRIPT_CREDENTIAL}\n"
        f"ImportCredential={SERVICES_CREDENTIAL}\n"
        f"ExecStart=/bin/sh %d/{SCRIPT_CREDENTIAL}\n"
        f"TimeoutStartSec={deadline}\n"
    )


def credential(name, text):
    """Return one SMBIOS type 11 option carrying `text` as a base64 systemd credential."""
    encoded = base64.b64encode(text.encode("utf-8")).decode("ascii")
    return ["-smbios", f"type=11,value=io.systemd.credential.binary:{name}={encoded}"]


def kernel_command_line(pin):
    """Return the guest kernel command line: live root by volume id, console, our unit."""
    volume = pin["boot"]["volume-id"]
    return f"root=live:CDLABEL={volume} rd.live.image console=ttyS0 " f"systemd.wants={UNIT_NAME}"


def guest_argv(pin, boot, artifact, nonce):
    """Return the full QEMU argv: KVM with no fallback, direct kernel boot, credentials."""
    memory, cpus = pin["boot"]["memory-mib"], pin["boot"]["cpus"]
    argv = ["qemu-system-x86_64", "-nodefaults", "-machine", "q35,accel=kvm", "-cpu", "host"]
    argv += ["-m", str(memory), "-smp", str(cpus), "-display", "none", "-no-reboot"]
    argv += ["-nic", "none", "-run-with", "exit-with-parent=on"]
    argv += ["-device", "virtio-vga", "-device", "virtio-rng-pci"]
    iso = harness.option_path(artifact)
    argv += ["-drive", f"if=virtio,format=raw,readonly=on,file={iso}"]
    argv += ["-kernel", str(boot.kernel), "-initrd", str(boot.initrd)]
    argv += ["-append", kernel_command_line(pin)]
    argv += credential(f"systemd.extra-unit.{UNIT_NAME}", unit_text(pin))
    argv += credential(SCRIPT_CREDENTIAL, report_script(nonce, pin))
    argv += credential(SERVICES_CREDENTIAL, services_text(probed_services(pin)))
    argv += ["-serial", f"file:{harness.option_path(boot.console)}"]
    argv += ["-serial", f"file:{harness.option_path(boot.report)}"]
    return argv


def extract_boot_files(pin, artifact, boot):
    """Read the guest's own kernel and initramfs out of the verified ISO with xorriso."""
    argv = ["xorriso", "-osirrox", "on", "-indev", str(artifact)]
    for source, target in (
        (pin["boot"]["kernel"], boot.kernel),
        (pin["boot"]["initrd"], boot.initrd),
    ):
        argv += ["-extract", f"/{source}", str(target)]
    code, _stdout, stderr = harness.run(argv, harness.TOOL_TIMEOUT)
    if code != 0 or not (boot.kernel.is_file() and boot.initrd.is_file()):
        raise harness.GateError(f"xorriso could not extract the kernel: {stderr.strip()[-300:]}")


def prepare_boot(context, boot, pin):
    """Check the bytes before anything is written, extract the kernel, check the bytes again.

    The first check refuses a wrong image before any program runs or any file is
    written for it; the second is taken last, immediately before QEMU starts.
    """
    harness.require_pinned(pin, context.artifact)
    boot.dir.mkdir(parents=True, exist_ok=True)
    for path in (boot.kernel, boot.initrd, boot.console, boot.report):
        path.unlink(missing_ok=True)
    extract_boot_files(pin, context.artifact, boot)
    return harness.require_pinned(pin, context.artifact)


def report_ready(path):
    """Return the report text once the guest wrote its end marker."""
    text = harness.console_text(path)
    return text if f"{GUEST_MARK}-END" in text.splitlines() else None


def run_guest(context, boot, pin, nonce):
    """Run QEMU until the report ends and the guest powers off, each under its deadline."""
    deadlines = pin["boot"]
    argv = guest_argv(pin, boot, context.artifact, nonce)
    with harness.session(argv, boot.dir / "qemu.log", context.spawned) as qemu:
        started = time.monotonic()
        text, elapsed = harness.wait_for(
            lambda: report_ready(boot.report), deadlines["report-deadline-seconds"], started
        )
        exited, _waited = harness.wait_for(
            qemu.poll, deadlines["shutdown-deadline-seconds"], time.monotonic()
        )
    return {
        "report": text or harness.console_text(boot.report),
        "complete": text is not None,
        "elapsed": elapsed,
        "exit_status": exited,
        "argv": argv,
    }


def boot_guest(context, pin):
    """Boot the pinned guest once and return what the gate observed."""
    boot = Boot(context.run_dir / "positive")
    nonce = context.nonce()
    digest = prepare_boot(context, boot, pin)
    observed = run_guest(context, boot, pin, nonce)
    observed.update({"nonce": nonce, "digest": digest})
    observed["fields"] = report_fields(observed["report"])
    (boot.dir / "result.json").write_text(json.dumps(observed, indent=2) + "\n", encoding="utf-8")
    return observed


def report_fields(text):
    """Return the guest report as {kind: [rest of line, ...]}, bounded in lines."""
    fields = {}
    prefix = f"{GUEST_MARK}-"
    for line in text.splitlines()[:MAX_REPORT_LINES]:
        if line.startswith(prefix):
            kind, _space, rest = line[len(prefix) :].partition(" ")
            fields.setdefault(kind, []).append(rest.strip())
    return fields


def answered(fields):
    """Return {name: (label, waited, package, version)} for every SERVICE line."""
    found = {}
    for rest in fields.get("SERVICE", []):
        parts = rest.split()
        if len(parts) == 5 and parts[2].isdigit():
            label, name, waited, package, version = parts
            found[name] = (label, int(waited), package, version)
    return found


def within_deadline(waited, deadline):
    """Decide one probe against its deadline: inclusive, so waited <= deadline answered in time."""
    return waited is not None and 0 <= waited <= deadline


def service_verdict(service, found):
    """Return why one probed service did not answer in time with a package version, or None."""
    label, name, deadline = service["label"], service["name"], service["deadline-seconds"]
    if name not in found:
        return f"{label}: {name} did not appear within its {deadline} s deadline"
    _label, waited, package, version = found[name]
    if not within_deadline(waited, deadline):
        return f"{label}: {name} answered after {waited} s, past its {deadline} s deadline"
    if package in ("unpackaged", "unknown-executable"):
        return f"{label}: {name} answered, but no package version was read back ({version})"
    return None


def session_problems(fields, nonce, pin):
    """Return why a report is not this boot's complete live-session read-back."""
    problems = []
    if fields.get("NONCE") != [nonce]:
        problems.append(f"the report carries nonce {fields.get('NONCE')}, not this boot's {nonce}")
    if not fields.get("SESSION"):
        problems.append(f"the live user's session did not appear: {fields.get('SESSION-MISSING')}")
    found = answered(fields)
    problems += [line for s in pin["services"] if (line := service_verdict(s, found))]
    advertised = {rest.split()[0] for rest in fields.get("GLOBAL", []) if rest.split()}
    missing = [name for name in pin["required-globals"] if name not in advertised]
    problems += [f"the compositor does not advertise {name}" for name in missing]
    return problems + fact_problems(fields)


def fact_problems(fields):
    """Return the portal and watcher facts the report did not read back."""
    keys = ("PORTAL-SETTINGS-VERSION", "PORTAL-COLOR-SCHEME", "WATCHER-PROTOCOL-VERSION")
    return [
        f"{key.lower()} was not read back"
        for key in keys
        if fields.get(key, ["absent"])[0] in ("absent", "")
    ]


def version_notes(fields, pin):
    """Return one note per service with the version read back, then the protocol facts."""
    found = answered(fields)
    notes = []
    for service in pin["services"]:
        label, waited, package, version = found.get(service["name"], ("", 0, "-", "-"))
        notes.append(f"{service['label']}: {service['name']} {package} {version} after {waited} s")
    advertised = dict(
        rest.split()[:2] for rest in fields.get("GLOBAL", []) if len(rest.split()) > 1
    )
    notes += [
        f"wayland global {name} version {advertised.get(name)}" for name in pin["required-globals"]
    ]
    for key in ("OS", "UNAME-R", "PORTAL-SETTINGS-VERSION", "PORTAL-COLOR-SCHEME"):
        notes.append(f"{key.lower()}: {fields.get(key, ['absent'])[0]}")
    notes.append(
        f"watcher-protocol-version: {fields.get('WATCHER-PROTOCOL-VERSION', ['absent'])[0]}"
    )
    return notes


def verified_case(context):
    """session/guest-verified: signature, signed digest, version and bytes before any boot."""
    try:
        verified = harness.verify_artifact_gpg(context, context.artifact, context.checksum)
    except harness.Refused as refusal:
        harness.report("session/guest-verified", [f"refused at {refusal.stage}: {refusal}"])
        return [str(refusal)]
    notes = [verified["good"], f"version {verified['version']}, sha256 {verified['digest']}"]
    harness.report("session/guest-verified", [], notes)
    return []


def planted_pin(pin):
    """Return a copy of `pin` whose sha256 differs from the image in its last digit."""
    planted = json.loads(json.dumps(pin))
    last = planted["sha256"][-1]
    planted["sha256"] = planted["sha256"][:-1] + ("0" if last != "0" else "1")
    return planted


def mismatch_case(context):
    """session/sha256-mismatch-refused: refused before anything starts or is written."""
    boot = Boot(context.run_dir / "mismatch")
    started = len(context.spawned)
    problems = []
    try:
        prepare_boot(context, boot, planted_pin(context.pin))
        problems.append("a pin whose sha256 differs from the image was not refused")
    except harness.Refused as refusal:
        if refusal.stage != "digest":
            problems.append(f"refused at {refusal.stage}, not at the digest: {refusal}")
    if len(context.spawned) != started:
        problems.append("a program was started for the refused image")
    if boot.dir.exists():
        problems.append(f"{boot.dir} was written for the refused image")
    notes = ["refused at the digest, before boot"]
    harness.report("session/sha256-mismatch-refused", problems, notes)
    return problems


def positive_case(observed, pin):
    """session/positive: the guest booted under KVM and every pinned service answered."""
    problems = []
    if not observed["complete"]:
        problems.append("the guest wrote no complete report before its deadline")
    problems += session_problems(observed["fields"], observed["nonce"], pin)
    if observed["exit_status"] != 0:
        problems.append(f"QEMU ended with status {observed['exit_status']}, not a guest power-off")
    harness.report("session/positive", problems, version_notes(observed["fields"], pin))
    return problems


def boundary_case(observed):
    """session/service-deadline-boundary: the planted absent name fails by name at its deadline."""
    verdict = service_verdict(PLANTED_ABSENT, answered(observed["fields"]))
    expected = (
        f"{PLANTED_ABSENT['label']}: {PLANTED_ABSENT['name']} did not appear within its "
        f"{PLANTED_ABSENT['deadline-seconds']} s deadline"
    )
    missing = " ".join(str(PLANTED_ABSENT[key]) for key in ("label", "name", "deadline-seconds"))
    problems = []
    if verdict != expected:
        problems.append(f"the planted absent service gave {verdict!r}, not {expected!r}")
    if missing not in observed["fields"].get("MISSING", []):
        problems.append(f"the guest did not report {missing!r} as missing")
    notes = [f"failed as required: {expected}"]
    harness.report("session/service-deadline-boundary", problems, notes)
    return problems


def pinned_bytes_case(context):
    """session/pinned-bytes-unchanged: the cached image still hashes to the pin after the boot."""
    try:
        harness.require_pinned(context.pin, context.artifact)
    except harness.Refused as refusal:
        harness.report("session/pinned-bytes-unchanged", [str(refusal)])
        return [str(refusal)]
    harness.report("session/pinned-bytes-unchanged", [], [f"sha256 {context.pin['sha256']}"])
    return []


def run_cases(context):
    """Run every case in order and return the number that failed."""
    if verified_case(context):
        raise harness.GateError(
            "the pinned guest image was refused before boot, so nothing was booted"
        )
    outcomes = [mismatch_case(context)]
    observed = boot_guest(context, context.pin)
    outcomes += [positive_case(observed, context.pin), boundary_case(observed)]
    outcomes.append(pinned_bytes_case(context))
    return sum(1 for problems in outcomes if problems)


def discard_bulk(run_dir):
    """Remove the kernels and initramfs images a run extracted; keep its logs and results."""
    for name in ("positive", "mismatch"):
        for path in (run_dir / name / "vmlinuz", run_dir / name / "initrd.img"):
            path.unlink(missing_ok=True)


def prune_runs(store):
    """Keep the newest MAX_RETAINED_RUNS run directories."""
    runs = store / "runs"
    if not runs.is_dir():
        return
    names = sorted(path for path in runs.iterdir() if path.is_dir())[: harness.MAX_PRUNED_FILES]
    for old in names[:-MAX_RETAINED_RUNS]:
        shutil.rmtree(old, ignore_errors=True)


def parse_arguments(argv):
    """Return the parsed options."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--pin", type=Path, default=PIN, help="the guest pin to boot")
    parser.add_argument(
        "--fetch", action="store_true", help="download and cache the pinned guest image first"
    )
    return parser.parse_args(argv)


def preflight(context, fetch_first):
    """Return the reasons the gate cannot run here; fetch first when asked."""
    reasons = check_toolchain() + harness.check_kvm() + check_memory()
    if fetch_first and not reasons:
        harness.fetch_pinned(context)
    return reasons + check_cache(context)


def execute(context, run_dir, store):
    """Run the cases; return the exit status, printing the verdict line."""
    run_dir.mkdir(parents=True, exist_ok=True)
    print(f"     guest image: {context.artifact} ({context.pin['name']} {context.pin['version']})")
    print(f"     run directory: {run_dir}")
    try:
        failed = run_cases(context)
    except harness.GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    except harness.Refused as refusal:
        print(f"FAIL: the pinned guest image was refused at {refusal.stage} mid-run: {refusal}")
        return 1
    finally:
        discard_bulk(run_dir)
        prune_runs(store)
    if failed:
        print(f"FAIL: {failed} live session case(s) did not match their recorded outcome.")
        return 1
    print(
        "PASS: live session in a VM on the reference profile (M29 E29-4). The pinned guest "
        "booted under KVM and its session services answered with versions read back; this is "
        "development evidence only. The guest's compositor is not P04 (D79), and it closes no "
        "hardware, accessibility or release gate."
    )
    return 0


def main(argv=None):
    """Run the gate: SKIP with a reason, FAIL with a reason, or PASS with every case above it."""
    options = parse_arguments(argv)
    harness.install_signal_handlers()
    store = cache_dir()
    run_dir = store / "runs" / f"{time.strftime('r%Y%m%dT%H%M%S')}-{secrets.token_hex(2)}"
    print(f"Live session gate (M29 E29-4; development evidence only). Cache: {store}")
    try:
        context = Context(load_pin(options.pin), store, run_dir)
        reasons = preflight(context, options.fetch)
    except harness.GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    if reasons:
        for reason in reasons[:MAX_PROBLEM_LINES]:
            print(f"SKIP: {reason}; the live session gate did not run.")
        return 0
    return execute(context, run_dir, store)


if __name__ == "__main__":
    sys.exit(main())
