#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Boot an externally supplied artifact under QEMU, OVMF and swtpm, and keep the evidence.

Milestone M24. Decision D72 gives this milestone the harness and the real boot
evidence, over whatever bootable artifact is supplied: a pinned upstream
distribution image today, an Imago return at M11. Aegis constructs no image
here. The artifact is named by a pin file (``build/boot/artifact.pin.json`` by
default, ``--pin`` otherwise), its bytes are cached outside the repository, and
they are hashed against the pin immediately before every boot. Every boot runs
on a throwaway qcow2 overlay, so the pinned bytes never change; the last case
hashes them again to show it.

Decisions D62 and D84 scope Secure Boot to the guest. The host firmware is read,
never written: its SecureBoot and SetupMode variables are recorded from
efivarfs. Each guest variable store is generated per run with virt-fw-vars from
the shipped ``OVMF_VARS.4m.fd`` and a key pair made for that run.

Cases, with no simulation and no failure suppression:

* ``boot/host-secure-boot`` records the host's own Secure Boot state;
* ``boot/artifact-verified`` verifies the clearsigned CHECKSUM against the
  pinned key fingerprint, the signed digest and producer version against the
  pin, and the cached bytes against the signed digest, before any boot;
* ``boot/artifact-tampered-refused`` feeds a copy with one flipped byte to the
  same verification and to the same boot entry, and requires both to refuse it
  at the digest; the boot entry refuses before it runs any program or writes
  any file for that copy;
* ``boot/checksum-bad-signature-refused`` alters one digit inside the signed
  CHECKSUM and requires the signature check to refuse it;
* ``boot/producer-version-boundary`` evaluates the version floor exactly at the
  floor and one below it;
* ``boot/positive`` boots the artifact under KVM on OVMF with Secure Boot and a
  swtpm TPM, requires the login prompt within the recorded timeout, and reads
  PCR 0, 4, 7 and 11 back from inside the guest with this boot's nonce;
* ``boot/firmware-contrast`` boots the same bytes on the firmware without
  Secure Boot and requires a different PCR 7, because PCR 7 records the
  firmware's state;
* ``boot/login-timeout-negative`` boots with a deliberately short timeout and
  requires the harness to report the miss with QEMU still running at the
  deadline, and to stop the guest itself;
* ``boot/timeout-boundary`` evaluates the timeout rule one second under, at and
  one second over the recorded timeout: two outcomes, inclusive at the edge;
* ``boot/uki-signed`` signs the artifact's UKI with this run's key, verifies it
  with sbverify, and boots it on a store holding only that key, where the same
  UKI without the signature is refused by the firmware;
* ``boot/pinned-bytes-unchanged`` hashes the cached artifact after every boot.

Nothing is written into the repository. The cache, the keyring and the retained
logs live under ``AEGIS_BOOT_HARNESS_DIR`` (default
``${XDG_CACHE_HOME:-$HOME/.cache}/aegis-boot-harness``).

The pin schema implements one signature scheme, a clearsigned CHECKSUM verified
by key fingerprint, and M24 is proven on the upstream kind of artifact it
covers. An Imago return's signature form is not pinned yet (M09); under
decision D85, M11 adds that scheme to this harness once M09 pins it.
"""

import argparse
import hashlib
import importlib.metadata
import json
import os
import re
import secrets
import shutil
import signal
import socket
import statistics
import struct
import subprocess
import sys
import time
import uuid
from contextlib import contextmanager
from pathlib import Path

from host import end_session
from host import target as posix_target

ROOT = Path(__file__).resolve().parent.parent
PIN = ROOT / "build" / "boot" / "artifact.pin.json"
REPORT_SCRIPT = ROOT / "tools" / "guest" / "aegis-boot-report.sh"
NONCE_PLACEHOLDER = "@AEGIS_NONCE@"
KVM_DEVICE = Path("/dev/kvm")
EFIVARS = Path("/sys/firmware/efi/efivars")
HOST_PCRS = Path("/sys/class/tpm/tpm0/pcr-sha256")
EFI_GLOBAL = "8be4df61-93ca-11d2-aa0d-00e098032b8c"
ESP_TYPE = uuid.UUID("c12a7328-f81f-11d2-ba4b-00a0c93ec93b")
DEFAULT_OVMF_DIR = "/usr/share/edk2/x64"
KEY_FILE = "signing-keys.gpg"
PIN_SCHEMA = "aegis.m24.boot-artifact-pin.v1"

# Deadlines. Every external command carries one, and every process the harness
# starts in the background is ended with its whole session on every way out.
VERSION_TIMEOUT = 30
TOOL_TIMEOUT = 120
GPG_TIMEOUT = 120
FETCH_TIMEOUT = 1800
SOCKET_TIMEOUT = 30
REPORT_TIMEOUT = 120
SHUTDOWN_TIMEOUT = 120
FIRMWARE_BOOT_TIMEOUT = 90
STOP_TIMEOUT = 15
QMP_TIMEOUT = 10
POLL_INTERVAL = 0.25
# Signals that end the harness through its finally blocks, not around them.
TERMINATING_SIGNALS = ("SIGTERM", "SIGHUP", "SIGQUIT")
# The negative login case. Measured boots reach the prompt in about sixteen
# seconds (docs/build/boot-harness.md); five is below the firmware stage alone.
TOO_SHORT_TIMEOUT = 5
MAX_LOGIN_TIMEOUT = 1800

# Scalar bounds.
MAX_DIGEST_CHUNKS = 4096
MAX_CONSOLE_BYTES = 32 << 20
MAX_REPORT_LINES = 400
MAX_PROBLEM_LINES = 24
MAX_QMP_LINES = 64
MAX_QMP_BYTES = 65536
MAX_EFIVAR_BYTES = 64
MAX_EFIVAR_ENTRIES = 4096
MAX_GPT_ENTRIES = 128
MAX_OUTPUT_CHARS = 1 << 20
MAX_RETAINED_RUNS = 16
MAX_MEASURED_BOOTS = 20
MAX_SOCKET_PATH = 100
MAX_PRUNED_FILES = 4096

GUEST_MARK = "AEGIS-M24"
GUEST_HOSTNAME = "aegis-m24-guest"
GUEST_MEMORY = "2048"
GUEST_CPUS = "2"
NONCE_BYTES = 16
NONCE_SHAPE = re.compile(r"aegis-m24-[0-9a-f]{32}")
PCR_INDICES = (0, 4, 7, 11)
FIRMWARE_PCRS = (0, 4, 7)
PCR_VALUE = re.compile(r"[0-9a-f]{64}")
EFI_BYTES = re.compile(r"\d{1,3}(?: \d{1,3}){4}")
VERSION = re.compile(r"(\d+)-(\d+)\.(\d+)")
SAFE_NAME = re.compile(r"[A-Za-z0-9][A-Za-z0-9._-]{0,200}")
UKI_PATH = re.compile(r"(?:[A-Za-z0-9][A-Za-z0-9._-]*/){1,4}[A-Za-z0-9][A-Za-z0-9._-]*\.efi")
MICROSOFT_DB = ("none", "uefi11", "uefi23")
LOGIN_PROMPT = re.compile(r"(?m)^[A-Za-z0-9][A-Za-z0-9.-]{0,62} login: ")
KERNEL_SECURE = "secureboot: Secure boot enabled"
FIRMWARE_DENIED = "Access Denied"
ESCAPES = re.compile(
    r"\x1b\[[0-?]*[ -/]*[@-~]|\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)|\x1b[P^_][^\x1b]*\x1b\\"
    r"|\x1b[@-Z\\-_]"
)
BULKY_SUFFIXES = (".qcow2", ".raw", ".efi", ".EFI", ".fd", ".iso", ".key", ".bin", ".sock")
BULKY_DIRECTORIES = ("tpm", "esp", "tampered")

# Three firmware profiles. Secure Boot needs the SMM build and a secure pflash;
# the plain build is the contrast that shows what PCR 7 records.
FIRMWARE_PROFILES = {
    "secure": ("OVMF_CODE.secboot.4m.fd", "q35,smm=on,accel=kvm", True),
    "plain": ("OVMF_CODE.4m.fd", "q35,smm=off,accel=kvm", False),
}

# Firmware images, admitted by file digest because they print no version. Every
# digest was read on the reference profile from edk2-ovmf 202608-1.
FIRMWARE = (
    ("OVMF_CODE.secboot.4m.fd", "cc150d941d4f1d39e596dedc545384a66ccfb3c9ba5cf9bc3a54d8d427d4d88f"),
    ("OVMF_VARS.4m.fd", "5d2ac383371b408398accee7ec27c8c09ea5b74a0de0ceea6513388b15be5d1e"),
    ("OVMF_CODE.4m.fd", "2febd26c0b4cf95a636a941afa37d64a552723443ed9ac72f763f6840da98cb4"),
)

# Toolchain admission. Every row is a tool this gate runs, read back from the
# tool before anything boots. A floor of None is a tool for which no source this
# gate relies on declares one; its reference value is recorded so the admission
# is a pin rather than whatever the workstation ships. virt-fw-vars prints no
# version, so it is run once and its distribution metadata is read instead.
METADATA = "distribution-metadata"
TOOLCHAIN = (
    (
        "qemu-system-x86_64",
        ["qemu-system-x86_64", "--version"],
        r"QEMU emulator version (\d+(?:\.\d+)*)",
        None,
        "11.1.1",
    ),
    ("qemu-img", ["qemu-img", "--version"], r"qemu-img version (\d+(?:\.\d+)*)", None, "11.1.1"),
    ("swtpm", ["swtpm", "--version"], r"TPM emulator version (\d+(?:\.\d+)*)", None, "0.10.2"),
    (
        "swtpm_setup",
        ["swtpm_setup", "--version"],
        r"TPM emulator setup tool version (\d+(?:\.\d+)*)",
        None,
        "0.10.2",
    ),
    ("virt-firmware", ["virt-fw-vars", "--help"], METADATA, None, "26.9"),
    ("sbsign", ["sbsign", "--version"], r"sbsign (\d+(?:\.\d+)*)", None, "0.9.5"),
    ("sbverify", ["sbverify", "--version"], r"sbverify (\d+(?:\.\d+)*)", None, "0.9.5"),
    # xorriso prints its patch level as a ".plNN" suffix; it is part of the pin.
    (
        "xorriso",
        ["xorriso", "-version"],
        r"xorriso (\d+(?:\.\d+)*(?:\.pl\d+)?)",
        None,
        "1.5.8.pl02",
    ),
    ("mcopy", ["mcopy", "--version"], r"mcopy \(GNU mtools\) (\d+(?:\.\d+)*)", None, "4.0.49"),
    ("openssl", ["openssl", "version"], r"OpenSSL (\d+(?:\.\d+)*)", None, "3.6.4"),
    ("gpg", ["gpg", "--version"], r"gpg \(GnuPG\) (\d+(?:\.\d+)*)", None, "2.4.9"),
    ("curl", ["curl", "--version"], r"curl (\d+(?:\.\d+)*)", None, "8.22.0"),
)


class GateError(Exception):
    """The gate could not be run at all, as opposed to a case that failed."""


class Refused(Exception):
    """The artifact was refused before boot. `stage` names the check that refused it."""

    def __init__(self, stage, message):
        super().__init__(message)
        self.stage = stage


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
        try:
            process.wait(timeout=STOP_TIMEOUT)
        except subprocess.TimeoutExpired as error:
            raise GateError(f"process {process.pid} survived SIGKILL") from error


def run(argv, timeout, cwd=None):
    """Run `argv` in its own session under a hard deadline; return (code, stdout, stderr).

    The child leads its own session, so what it starts -- swtpm_setup starts a
    swtpm -- dies with it when the deadline expires instead of outliving the gate.
    """
    try:
        process = subprocess.Popen(
            argv,
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.PIPE,
            env=c_locale(),
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


@contextmanager
def session(argv, log, spawned):
    """Start `argv` in the background in its own session; end the session on every exit path."""
    spawned.append(list(argv))
    with open(log, "wb") as handle:
        try:
            process = subprocess.Popen(
                argv,
                stdin=subprocess.DEVNULL,
                stdout=handle,
                stderr=subprocess.STDOUT,
                env=c_locale(),
                start_new_session=True,
            )
        except OSError as error:
            raise GateError(f"{argv[0]} could not be started: {error}") from error
        try:
            yield process
        finally:
            stop(process)


def wait_for(check, deadline, started):
    """Poll `check` until it returns a value or `deadline` seconds after `started` pass.

    Returns (value, elapsed seconds since `started`). The iteration count is
    bounded as well as the clock, so a check that returns instantly cannot spin.
    """
    for _ in range(int(deadline / POLL_INTERVAL) + 4):
        value = check()
        elapsed = time.monotonic() - started
        if value is not None or elapsed > deadline:
            return value, elapsed
        time.sleep(POLL_INTERVAL)
    return None, time.monotonic() - started


def login_outcome(elapsed, timeout):
    """Decide one boot against the recorded timeout: inclusive, so t <= T reached it."""
    if elapsed is None or elapsed > timeout:
        return "missed"
    return "reached"


def version_tuple(text):
    """Return a dotted version as a tuple of integers, for ordering."""
    return tuple(int(part) for part in text.split("."))


def metadata_version(name):
    """Return (banner, version) from a Python distribution's own metadata."""
    try:
        found = importlib.metadata.version(name)
    except importlib.metadata.PackageNotFoundError:
        return f"no {name} distribution metadata is importable", None
    return f"{name} {found} (distribution metadata)", found


def read_version(row):
    """Return (banner, version string) for one toolchain row, or (reason, None)."""
    name, argv, pattern, _floor, _reference = row
    if shutil.which(argv[0]) is None:
        return f"{argv[0]} is not on PATH", None
    code, stdout, stderr = run(argv, VERSION_TIMEOUT)
    if code != 0:
        return f"{' '.join(argv)} exited {code}", None
    if pattern == METADATA:
        return metadata_version(name)
    match = re.search(pattern, stdout + stderr)
    if match is None:
        return f"no version matched {pattern!r} in the output of {' '.join(argv)}", None
    return match.group(0).strip(), match.group(1)


def check_toolchain():
    """Print the admitted toolchain read back from the host; return why it cannot run."""
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


def ovmf_dir():
    """Return the directory the edk2 OVMF images are read from."""
    return Path(os.environ.get("AEGIS_OVMF_DIR", DEFAULT_OVMF_DIR))


def file_digest(path):
    """Return the SHA-256 of `path`, reading at most MAX_DIGEST_CHUNKS mebibytes."""
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for _ in range(MAX_DIGEST_CHUNKS):
            chunk = handle.read(1 << 20)
            if not chunk:
                return digest.hexdigest()
            digest.update(chunk)
    raise GateError(f"{Path(path).name} exceeds {MAX_DIGEST_CHUNKS} MiB")


def check_firmware():
    """Print each admitted OVMF image's digest; return why the harness cannot boot."""
    reasons = []
    for name, admitted in FIRMWARE:
        path = ovmf_dir() / name
        if not path.is_file():
            reasons.append(f"{path} does not exist")
            continue
        found = file_digest(path)
        note = "" if found == admitted else f" [reference profile recorded {admitted}]"
        print(f"     {name}: sha256 {found}{note}")
    return reasons


def check_kvm():
    """Return why KVM cannot be used; the harness has no TCG fallback."""
    if not KVM_DEVICE.exists():
        return [f"{KVM_DEVICE} does not exist; KVM is required and there is no TCG fallback"]
    if not os.access(KVM_DEVICE, os.R_OK | os.W_OK):
        return [f"{KVM_DEVICE} is not read-write for this account; KVM is required"]
    print(f"     {KVM_DEVICE}: read-write; every guest runs with accel=kvm and no fallback")
    return []


def cache_dir():
    """Return the out-of-repository directory the artifact and the run logs live under."""
    override = os.environ.get("AEGIS_BOOT_HARNESS_DIR")
    if override:
        return Path(override).expanduser()
    cache = os.environ.get("XDG_CACHE_HOME")
    root = Path(cache).expanduser() if cache else Path.home() / ".cache"
    return root / "aegis-boot-harness"


def version_key(text):
    """Order a producer version such as 44-1.7 as (44, 1, 7)."""
    match = VERSION.fullmatch(text)
    if match is None:
        raise GateError(f"{text!r} is not a producer version this harness can order")
    return tuple(int(part) for part in match.groups())


def version_admitted(version, floor):
    """Return True when `version` is at or above the pinned floor."""
    return version_key(version) >= version_key(floor)


def version_below(floor):
    """Return the version one step below `floor` in its last component."""
    major, minor, respin = version_key(floor)
    if respin == 0:
        raise GateError(f"the floor {floor} has no predecessor in its last component")
    return f"{major}-{minor}.{respin - 1}"


def pin_shape_problems(pin):
    """Return the fields a pin lacks, section by section."""
    top = ("schema", "name", "version", "version-floor", "architecture", "format", "file")
    top += ("url", "sha256", "size", "signature", "boot")
    signature = ("scheme", "checksum-file", "checksum-url", "key-url", "fingerprint")
    boot = ("login-prompt-timeout-seconds", "guest-microsoft-db", "shim-fallback-no-reboot")
    problems = [f"missing {key}" for key in top if key not in pin]
    if problems:
        return problems
    problems += [f"signature lacks {key}" for key in signature if key not in pin["signature"]]
    problems += [f"boot lacks {key}" for key in boot + ("uki-path",) if key not in pin["boot"]]
    return problems


def pin_value_problems(pin):
    """Return the pin values this harness refuses to act on."""
    sig, boot = pin["signature"], pin["boot"]
    timeout = boot["login-prompt-timeout-seconds"]
    expected_file = f"{pin['name']}-{pin['version']}.{pin['architecture']}.{pin['format']}"
    urls = (pin["url"], sig["checksum-url"], sig["key-url"])
    checks = (
        (pin["schema"] == PIN_SCHEMA, f"schema is not {PIN_SCHEMA}"),
        (PCR_VALUE.fullmatch(pin["sha256"]), "sha256 is not 64 lower-case hex digits"),
        (re.fullmatch(r"[0-9A-F]{40}", sig["fingerprint"]), "fingerprint is not 40 hex digits"),
        (sig["scheme"] == "gpg-clearsigned-checksum", "only gpg-clearsigned-checksum is built"),
        (VERSION.fullmatch(pin["version"]), "version is not MAJOR-MINOR.RESPIN"),
        (VERSION.fullmatch(pin["version-floor"]), "version-floor is not MAJOR-MINOR.RESPIN"),
        (pin["file"] == expected_file, f"file is not {expected_file}"),
        (SAFE_NAME.fullmatch(sig["checksum-file"]), "checksum-file is not a plain file name"),
        (all(url.startswith("https://") for url in urls), "every URL must be https"),
        (isinstance(timeout, int) and 0 < timeout <= MAX_LOGIN_TIMEOUT, "timeout out of range"),
        (boot["guest-microsoft-db"] in MICROSOFT_DB, "guest-microsoft-db is not admitted"),
        (isinstance(boot["shim-fallback-no-reboot"], bool), "shim-fallback-no-reboot is no bool"),
        (UKI_PATH.fullmatch(boot["uki-path"]), "uki-path is not a relative .efi path"),
        (isinstance(pin["size"], int) and pin["size"] > 0, "size is not a positive integer"),
    )
    return [message for passed, message in checks if not passed]


def load_pin(path):
    """Read and validate one artifact pin; refuse it whole rather than use part of it."""
    try:
        pin = json.loads(Path(path).read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as error:
        raise GateError(f"the pin {path} could not be read: {error}") from error
    if not isinstance(pin, dict):
        raise GateError(f"the pin {path} is not a JSON object")
    problems = pin_shape_problems(pin)
    if not problems:
        problems = pin_value_problems(pin)
    if problems:
        raise GateError(f"the pin {path} is refused: {'; '.join(problems)}")
    return pin


class Context:
    """One run: the pin, the cache, this run's directory and every process it started."""

    def __init__(self, pin, store, run_dir):
        self.pin = pin
        self.store = store
        self.run_dir = run_dir
        self.artifact = store / pin["file"]
        self.checksum = store / pin["signature"]["checksum-file"]
        self.keyring = store / "gnupg"
        self.timeout = pin["boot"]["login-prompt-timeout-seconds"]
        self.spawned = []
        self.key = None

    def nonce(self):
        """Return a fresh per-boot nonce."""
        return f"aegis-m24-{secrets.token_hex(NONCE_BYTES)}"


class Boot:
    """The files one boot reads and writes, all under its own directory."""

    def __init__(self, directory):
        self.dir = directory
        directory.mkdir(parents=True, exist_ok=True)
        self.overlay = directory / "overlay.qcow2"
        self.vars = directory / "vars.fd"
        self.seed = directory / "seed.iso"
        self.tpm_state = directory / "tpm"
        self.tpm_socket = directory / "swtpm.sock"
        self.qmp = directory / "qmp.sock"
        self.console = directory / "console.log"
        self.report = directory / "report.txt"


def fetch(url, target, timeout):
    """Download `url` to `target` through a partial file, https only."""
    partial = target.with_name(target.name + ".part")
    partial.unlink(missing_ok=True)
    argv = ["curl", "--fail", "--silent", "--show-error", "--location", "--proto", "=https"]
    code, _stdout, stderr = run(argv + ["--output", str(partial), url], timeout)
    if code != 0:
        partial.unlink(missing_ok=True)
        raise GateError(f"{url} could not be downloaded: {stderr.strip()}")
    partial.replace(target)


def gpg_argv(keyring):
    """Return gpg's argv on the gate's keyring, never starting a gpg-agent.

    Importing public keys and verifying a signature need no agent. Without
    --no-autostart gpg starts one as a daemon in its own session, where neither
    a deadline nor end_session reaches it, and it outlives the gate.
    """
    return ["gpg", "--homedir", str(keyring), "--batch", "--no-autostart"]


def import_key(keyring, key_file, fingerprint):
    """Import the published keys into the gate's own keyring and require the pinned one."""
    keyring.mkdir(parents=True, exist_ok=True)
    keyring.chmod(0o700)
    home = gpg_argv(keyring)
    code, _stdout, stderr = run(home + ["--import", str(key_file)], GPG_TIMEOUT)
    if code != 0:
        raise GateError(f"{key_file.name} did not import: {stderr.strip()}")
    listed, _stdout, _stderr = run(home + ["--list-keys", fingerprint], GPG_TIMEOUT)
    if listed != 0:
        raise GateError(f"{key_file.name} does not carry the pinned key {fingerprint}")


def fetch_pinned(context):
    """Populate the cache: the signed CHECKSUM, the signing keys and the artifact."""
    pin, store = context.pin, context.store
    store.mkdir(parents=True, exist_ok=True)
    fetch(pin["signature"]["checksum-url"], context.checksum, FETCH_TIMEOUT)
    fetch(pin["signature"]["key-url"], store / KEY_FILE, FETCH_TIMEOUT)
    import_key(context.keyring, store / KEY_FILE, pin["signature"]["fingerprint"])
    if context.artifact.exists():
        print(f"     cached artifact kept: {context.artifact}")
        return
    fetch(pin["url"], context.artifact, FETCH_TIMEOUT)
    found = file_digest(context.artifact)
    if found != pin["sha256"]:
        context.artifact.unlink()
        raise GateError(f"the download hashes to {found}, not the pinned {pin['sha256']}")
    context.artifact.chmod(0o444)
    print(f"     downloaded {pin['url']} to {context.artifact}")


def check_cache(context):
    """Return why the cached inputs are not all present."""
    wanted = (context.artifact, context.checksum, context.keyring)
    missing = [str(path) for path in wanted if not path.exists()]
    if not missing:
        return []
    return [
        f"{', '.join(missing)} not cached; run `python3 tools/verify_boot_harness.py --fetch` "
        "to download and verify the pinned artifact"
    ]


def signed_checksum(context, checksum, plain):
    """Verify the clearsigned CHECKSUM against the pinned fingerprint; return the signed text."""
    fingerprint = context.pin["signature"]["fingerprint"]
    plain.unlink(missing_ok=True)
    argv = gpg_argv(context.keyring) + ["--yes", "--status-fd", "1"]
    code, stdout, stderr = run(
        argv + ["--output", str(plain), "--decrypt", str(checksum)], GPG_TIMEOUT
    )
    status = gpg_status(stdout)
    signer = status.get("VALIDSIG", "").split()
    if code != 0 or not signer:
        plain.unlink(missing_ok=True)
        named = [f"{word} {status[word]}" for word in ("BADSIG", "ERRSIG", "NO_PUBKEY")]
        detail = "; ".join(line for line in named if not line.endswith(" ")) or stderr[-200:]
        raise Refused("signature", f"the CHECKSUM signature did not verify: {detail.strip()}")
    if signer[-1] != fingerprint:
        raise Refused("signature", f"the CHECKSUM is signed by {signer[-1]}, not {fingerprint}")
    return plain.read_text(encoding="utf-8"), f"GOODSIG {status.get('GOODSIG', '')}"


def gpg_status(stdout):
    """Return gpg's machine-readable status lines as {keyword: rest}, first occurrence kept."""
    status = {"BADSIG": "", "ERRSIG": "", "NO_PUBKEY": ""}
    for line in stdout.splitlines()[:MAX_REPORT_LINES]:
        keyword, _space, rest = line.removeprefix("[GNUPG:] ").partition(" ")
        if line.startswith("[GNUPG:] ") and not status.get(keyword):
            status[keyword] = rest.strip()
    return status


def signed_record(text, pin):
    """Return (digest, version) the signed CHECKSUM text records for the pinned file."""
    name = re.escape(pin["name"])
    suffix = re.escape(f".{pin['architecture']}.{pin['format']}")
    pattern = re.compile(rf"^SHA256 \({name}-(\d+-\d+\.\d+){suffix}\) = ([0-9a-f]{{64}})$", re.M)
    found = pattern.findall(text)
    if len(found) != 1:
        raise Refused("checksum", f"the signed CHECKSUM names {len(found)} {pin['name']} images")
    version, digest = found[0]
    return digest, version


def require_pinned(pin, artifact):
    """Refuse `artifact` unless its size and SHA-256 are the pinned ones; return the digest."""
    size = Path(artifact).stat().st_size
    if size != pin["size"]:
        raise Refused("digest", f"{Path(artifact).name} is {size} bytes, not {pin['size']}")
    found = file_digest(artifact)
    if found != pin["sha256"]:
        raise Refused("digest", f"{Path(artifact).name} hashes to {found}, not {pin['sha256']}")
    return found


def verify_artifact(context, artifact, checksum):
    """E24-1: signature, signed digest, producer version and the bytes, before any boot."""
    pin = context.pin
    plain = context.run_dir / f"{Path(checksum).name}.signed-text"
    text, good = signed_checksum(context, checksum, plain)
    recorded, version = signed_record(text, pin)
    if recorded != pin["sha256"]:
        raise Refused("checksum", f"the signed digest {recorded} is not the pinned {pin['sha256']}")
    if version != pin["version"] or not version_admitted(version, pin["version-floor"]):
        raise Refused("version", f"signed version {version} against pin {pin['version']}")
    found = require_pinned(pin, artifact)
    return {"good": good, "version": version, "digest": found}


def report(name, problems, notes=()):
    """Print one case's outcome and, when it failed, why."""
    print(f"{'PASS' if not problems else 'FAIL'} {name}")
    for note in notes:
        print(f"     {note}")
    for line in problems[:MAX_PROBLEM_LINES]:
        print(f"     {line}")


def host_efi_value(name):
    """Return one host EFI global variable as its raw bytes, or why it cannot be read."""
    path = EFIVARS / f"{name}-{EFI_GLOBAL}"
    try:
        with open(path, "rb") as handle:
            data = handle.read(MAX_EFIVAR_BYTES)
    except FileNotFoundError:
        return "absent"
    except OSError as error:
        return f"unreadable ({error.strerror})"
    return " ".join(str(byte) for byte in data)


def host_key_variables():
    """Return which of PK, KEK, db and dbx exist in the host's efivarfs."""
    names = [entry.name for entry in list(EFIVARS.iterdir())[:MAX_EFIVAR_ENTRIES]]
    return sorted({name.split("-", 1)[0] for name in names} & {"PK", "KEK", "db", "dbx"})


def host_secure_boot_case():
    """Record the host's own Secure Boot state; this milestone reads it and never writes it."""
    if not EFIVARS.is_dir():
        print(f"SKIP boot/host-secure-boot: {EFIVARS} does not exist on this host")
        return []
    secure, setup = host_efi_value("SecureBoot"), host_efi_value("SetupMode")
    keys = host_key_variables()
    problems = [] if EFI_BYTES.fullmatch(secure) else [f"SecureBoot could not be read: {secure}"]
    disabled = secure.endswith(" 0")
    meaning = (
        "host Secure Boot is disabled, so a PCR 7 value records a firmware state and "
        "attests no trusted chain on this host"
        if disabled
        else "host Secure Boot is not reported disabled; record this before citing PCR 7"
    )
    notes = [
        f"efivarfs SecureBoot: {secure} (attributes, then the value)",
        f"efivarfs SetupMode: {setup}",
        f"host key variables present: {', '.join(keys) or 'none of PK, KEK, db, dbx'}",
        meaning,
        "nothing here writes an EFI variable; the host firmware is not this milestone's subject",
    ]
    report("boot/host-secure-boot", problems, notes)
    return problems


def artifact_case(context):
    """E24-1 positive: every check that precedes a boot passes on the pinned artifact."""
    try:
        verified = verify_artifact(context, context.artifact, context.checksum)
    except Refused as refusal:
        report("boot/artifact-verified", [f"refused at {refusal.stage}: {refusal}"])
        return [str(refusal)]
    pin = context.pin
    notes = [f"gpg status: {verified['good']}"]
    notes += [
        f"signing key: {pin['signature']['fingerprint']} (pinned; {pin['signature']['identity']})",
        f"signed record: SHA256 ({pin['file']}) = {pin['sha256']}",
        f"producer version {verified['version']} against floor {pin['version-floor']}: admitted",
        f"cached bytes: sha256 {verified['digest']}, {pin['size']} bytes: match the pin",
    ]
    report("boot/artifact-verified", [], notes)
    return []


def flip_byte(path, offset):
    """Invert the lowest bit of one byte of `path`."""
    with open(path, "r+b") as handle:
        handle.seek(offset)
        value = handle.read(1)
        handle.seek(offset)
        handle.write(bytes([value[0] ^ 0x01]))


def refusal_attempts(context, copy, tampered_boot):
    """Feed `copy` to the verification and to the boot entry; return (problems, notes)."""
    problems, notes = [], []
    for label, attempt in (
        ("verification", lambda: verify_artifact(context, copy, context.checksum)),
        ("boot entry", lambda: boot_guest(context, Boot(tampered_boot), "secure", copy)),
    ):
        try:
            attempt()
            problems.append(f"the {label} accepted a copy with one flipped byte")
        except Refused as refusal:
            notes.append(f"{label}: refused at {refusal.stage}: {refusal}")
            if refusal.stage != "digest":
                problems.append(f"the {label} refused at {refusal.stage}, not at the digest")
    return problems, notes


def tampered_case(context):
    """Negative: a copy with one flipped byte is refused before boot, and nothing runs for it.

    The boot entry refuses before it runs a program or writes a file: no seed,
    variable store, TPM state (swtpm_setup starts a swtpm of its own), overlay or
    guest. The verification runs gpg on the CHECKSUM, which is not the copy.
    """
    copy = context.run_dir / "tampered" / context.pin["file"]
    copy.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(context.artifact, copy)
    flip_byte(copy, context.pin["size"] // 2)
    before = len(context.spawned)
    tampered_boot = context.run_dir / "tampered-boot"
    try:
        problems, notes = refusal_attempts(context, copy, tampered_boot)
    finally:
        copy.unlink(missing_ok=True)
    entries = tampered_boot.iterdir() if tampered_boot.is_dir() else ()
    written = sorted(path.name for path in entries)[:MAX_PROBLEM_LINES]
    sessions = len(context.spawned) - before
    if sessions or written:
        problems.append(f"the boot entry wrote {written} or started {sessions} session(s)")
    shown = written or "none (no seed, store, TPM state or overlay)"
    notes.append(
        f"files the boot entry wrote for the tampered copy: {shown}; "
        f"swtpm or QEMU sessions started: {sessions}"
    )
    report("boot/artifact-tampered-refused", problems, notes)
    return problems


def bad_signature_case(context):
    """Negative: one digit changed inside the signed text fails the signature, not a later check."""
    altered = context.run_dir / "bad-signature" / context.checksum.name
    altered.parent.mkdir(parents=True, exist_ok=True)
    digest = context.pin["sha256"]
    changed = ("1" if digest[0] != "1" else "2") + digest[1:]
    text = context.checksum.read_text(encoding="utf-8")
    if text.count(digest) != 1:
        raise GateError("the cached CHECKSUM does not carry the pinned digest exactly once")
    altered.write_text(text.replace(digest, changed), encoding="utf-8")
    problems, notes = [], [f"altered digest in the signed text: {digest} -> {changed}"]
    try:
        verify_artifact(context, context.artifact, altered)
        problems.append("a CHECKSUM with an altered signed line was accepted")
    except Refused as refusal:
        notes.append(f"refused at {refusal.stage}: {refusal}")
        if refusal.stage != "signature":
            problems.append(f"refused at {refusal.stage}, not by the signature check")
    report("boot/checksum-bad-signature-refused", problems, notes)
    return problems


def version_boundary_case(pin):
    """Boundary: a producer version exactly at the floor is admitted, one below is refused."""
    floor = pin["version-floor"]
    below = version_below(floor)
    decided = [(floor, version_admitted(floor, floor)), (below, version_admitted(below, floor))]
    problems = []
    if decided != [(floor, True), (below, False)]:
        problems.append(f"expected the floor admitted and {below} refused, got {decided}")
    notes = [f"floor {floor}; these are rule evaluations, not downloads"]
    notes += [f"{value} -> {'admitted' if ok else 'refused'}" for value, ok in decided]
    report("boot/producer-version-boundary", problems, notes)
    return problems


def generate_key(context):
    """Make this run's guest key pair; it becomes PK, KEK and, for the UKI case, db."""
    directory = context.run_dir / "keys"
    directory.mkdir(parents=True, exist_ok=True)
    key, cert = directory / "guest-test.key", directory / "guest-test.crt"
    subject = f"/CN=Aegis M24 guest test key {context.run_dir.name}/"
    argv = ["openssl", "req", "-x509", "-newkey", "rsa:2048", "-noenc", "-sha256", "-days", "1"]
    code, _stdout, stderr = run(
        argv + ["-subj", subject, "-keyout", str(key), "-out", str(cert)], TOOL_TIMEOUT
    )
    if code != 0:
        raise GateError(f"openssl could not make the guest key: {stderr.strip()[-300:]}")
    return {"key": key, "cert": cert, "owner": str(uuid.uuid4()), "subject": subject}


def store_options(context, profile):
    """Return the virt-fw-vars options for one guest store profile."""
    key, boot = context.key, context.pin["boot"]
    fallback = ["--set-fallback-no-reboot"] if boot["shim-fallback-no-reboot"] else []
    if profile == "secure":
        enrol = ["--enroll-cert", str(key["cert"]), "--microsoft-kek", "none"]
        db = ["--microsoft-db", boot["guest-microsoft-db"]]
        return enrol + db + ["--secure-boot"] + fallback
    if profile == "custom-only":
        owner, cert = key["owner"], str(key["cert"])
        pairs = ["--set-pk", owner, cert, "--add-kek", owner, cert, "--add-db", owner, cert]
        return pairs + ["--secure-boot"]
    return fallback


def guest_store(context, boot, profile):
    """Generate one guest variable store from the shipped template; return what it enrolled."""
    template = ovmf_dir() / "OVMF_VARS.4m.fd"
    argv = ["virt-fw-vars", "--input", str(template), "--output", str(boot.vars)]
    code, stdout, stderr = run(argv + store_options(context, profile), TOOL_TIMEOUT)
    if code != 0:
        raise GateError(
            f"virt-fw-vars could not write the {profile} store: {stderr.strip()[-300:]}"
        )
    lines = (stdout + stderr).splitlines()
    return [line.removeprefix("INFO: ") for line in lines if re.search(r"add |set variable", line)]


def user_data(nonce):
    """Return the guest report script with this boot's nonce in its one placeholder."""
    if NONCE_SHAPE.fullmatch(nonce) is None:
        raise GateError(f"{nonce!r} is not a nonce this harness generated")
    text = REPORT_SCRIPT.read_text(encoding="utf-8")
    if text.count(NONCE_PLACEHOLDER) != 1:
        raise GateError(f"{REPORT_SCRIPT.name} must carry {NONCE_PLACEHOLDER} exactly once")
    return text.replace(NONCE_PLACEHOLDER, nonce)


def write_seed(boot, nonce):
    """Write the NoCloud cidata seed: this boot's instance id and the report script."""
    seed = boot.dir / "seed"
    seed.mkdir(parents=True, exist_ok=True)
    (seed / "meta-data").write_text(
        f"instance-id: {nonce}\nlocal-hostname: {GUEST_HOSTNAME}\n", encoding="utf-8"
    )
    (seed / "user-data").write_text(user_data(nonce), encoding="utf-8")
    argv = ["xorriso", "-as", "mkisofs", "-output", str(boot.seed), "-volid", "cidata"]
    argv += ["-joliet", "-rational-rock", str(seed / "user-data"), str(seed / "meta-data")]
    code, _stdout, stderr = run(argv, TOOL_TIMEOUT)
    if code != 0:
        raise GateError(f"xorriso could not write the cidata seed: {stderr.strip()[-300:]}")


def option_path(path):
    """Render a path for a QEMU or swtpm option string, refusing what the parser would split."""
    text = posix_target(path)
    if "," in text or ":" in text:
        raise GateError(f"{text} contains ',' or ':', which an option string would split")
    return text


def socket_path(path):
    """Render a UNIX socket path, refusing one longer than the kernel accepts."""
    text = option_path(path)
    if len(text) > MAX_SOCKET_PATH:
        raise GateError(f"{text} is too long for a UNIX socket; set AEGIS_BOOT_HARNESS_DIR")
    return text


def machine_argv(profile, boot):
    """Return the QEMU argv common to every boot: KVM, OVMF and no display."""
    code, machine, secure = FIRMWARE_PROFILES[profile]
    argv = ["qemu-system-x86_64", "-nodefaults", "-machine", machine, "-cpu", "host"]
    argv += ["-m", GUEST_MEMORY, "-smp", GUEST_CPUS, "-display", "none", "-no-reboot"]
    # QEMU dies with the harness even when the harness cannot run its finally
    # blocks (SIGKILL); swtpm then ends through --terminate when QEMU is gone.
    argv += ["-nic", "none", "-run-with", "exit-with-parent=on"]
    if secure:
        argv += ["-global", "driver=cfi.pflash01,property=secure,value=on"]
    firmware = option_path(ovmf_dir() / code)
    argv += ["-drive", f"if=pflash,format=raw,unit=0,readonly=on,file={firmware}"]
    argv += ["-drive", f"if=pflash,format=raw,unit=1,file={option_path(boot.vars)}"]
    return argv


def guest_argv(profile, boot):
    """Return the full argv of a guest booted from the overlay with swtpm and the seed."""
    argv = machine_argv(profile, boot)
    argv += ["-chardev", f"socket,id=chrtpm,path={socket_path(boot.tpm_socket)}"]
    argv += ["-tpmdev", "emulator,id=tpm0,chardev=chrtpm", "-device", "tpm-crb,tpmdev=tpm0"]
    argv += ["-drive", f"if=virtio,format=qcow2,file={option_path(boot.overlay)}"]
    argv += ["-drive", f"if=virtio,format=raw,readonly=on,file={option_path(boot.seed)}"]
    argv += ["-qmp", f"unix:{socket_path(boot.qmp)},server=on,wait=off"]
    argv += ["-serial", f"file:{option_path(boot.console)}"]
    argv += ["-serial", f"file:{option_path(boot.report)}"]
    return argv


def swtpm_argv(boot):
    """Return the swtpm argv; --terminate ends it when QEMU disconnects."""
    return [
        "swtpm",
        "socket",
        "--tpm2",
        "--tpmstate",
        f"dir={option_path(boot.tpm_state)}",
        "--ctrl",
        f"type=unixio,path={socket_path(boot.tpm_socket)}",
        "--terminate",
    ]


def prepare_guest(context, boot, profile, artifact, nonce):
    """Check the bytes, write the seed, the store and the TPM state, check again, lay the overlay.

    The first check refuses a wrong artifact before any program runs or any
    file is written for it. The second is taken last, on the exact path the
    overlay then names as its read-only backing file, so the check that counts
    sits immediately before the boot.
    """
    require_pinned(context.pin, artifact)
    for path in (boot.console, boot.report, boot.overlay, boot.tpm_socket, boot.qmp):
        path.unlink(missing_ok=True)
    write_seed(boot, nonce)
    enrolled = guest_store(context, boot, profile)
    shutil.rmtree(boot.tpm_state, ignore_errors=True)
    boot.tpm_state.mkdir(parents=True)
    setup = ["swtpm_setup", "--tpm2", "--tpmstate", str(boot.tpm_state), "--createek"]
    code, _stdout, stderr = run(setup + ["--pcr-banks", "sha256", "--overwrite"], TOOL_TIMEOUT)
    if code != 0:
        raise GateError(f"swtpm_setup could not manufacture the TPM: {stderr.strip()[-300:]}")
    digest = require_pinned(context.pin, artifact)
    argv = ["qemu-img", "create", "-q", "-f", "qcow2", "-F", "qcow2", "-b", option_path(artifact)]
    code, _stdout, stderr = run(argv + [str(boot.overlay)], TOOL_TIMEOUT)
    if code != 0:
        raise GateError(f"qemu-img could not create the overlay: {stderr.strip()}")
    return {"digest": digest, "enrolled": enrolled}


def clean(text):
    """Drop terminal control sequences and carriage returns from console text."""
    return ESCAPES.sub("", text).replace("\r\n", "\n").replace("\r", "\n")


def console_text(path):
    """Return what a serial file holds so far, cleaned, bounded in size."""
    try:
        with open(path, "rb") as handle:
            raw = handle.read(MAX_CONSOLE_BYTES)
    except FileNotFoundError:
        return ""
    return clean(raw.decode("utf-8", errors="replace"))


def prompt_or_exit(qemu, console):
    """Return 'prompt' once the console shows a login prompt, 'exited' if QEMU ended first."""
    if LOGIN_PROMPT.search(console_text(console)):
        return "prompt"
    if qemu.poll() is not None:
        return "exited"
    return None


def report_ready(path):
    """Return the report text once the guest wrote its end marker."""
    text = console_text(path)
    return text if f"{GUEST_MARK}-END" in text.splitlines() else None


def wait_for_path(path, deadline):
    """Wait until `path` exists, bounded by `deadline` seconds."""
    found, _elapsed = wait_for(lambda: True if path.exists() else None, deadline, time.monotonic())
    if found is None:
        raise GateError(f"{path.name} did not appear within {deadline}s")


def qmp_reply(stream):
    """Read QMP lines until a command's reply, skipping asynchronous events."""
    for _ in range(MAX_QMP_LINES):
        line = stream.readline(MAX_QMP_BYTES)
        if not line:
            raise GateError("QMP closed the connection before replying")
        message = json.loads(line)
        if "return" in message or "error" in message:
            return message
    raise GateError(f"QMP sent {MAX_QMP_LINES} lines without a reply")


def qmp(path, commands):
    """Run QMP commands in order over the guest's monitor socket; return their replies."""
    family = getattr(socket, "AF_UNIX", None)
    if family is None:
        raise GateError("this host has no AF_UNIX sockets; QMP cannot be reached")
    try:
        with socket.socket(family, socket.SOCK_STREAM) as channel:
            channel.settimeout(QMP_TIMEOUT)
            channel.connect(str(path))
            stream = channel.makefile("rwb")
            stream.readline(MAX_QMP_BYTES)
            replies = []
            for command in ("qmp_capabilities",) + tuple(commands):
                stream.write(json.dumps({"execute": command}).encode() + b"\n")
                stream.flush()
                replies.append(qmp_reply(stream))
    except (OSError, ValueError) as error:
        raise GateError(f"QMP on {path.name} failed: {error}") from error
    return replies[1:]


def shut_down(qemu, boot, observed):
    """Wait for the report, read KVM state over QMP, power the guest off and keep the status."""
    text, _elapsed = wait_for(lambda: report_ready(boot.report), REPORT_TIMEOUT, time.monotonic())
    observed["report"] = text or console_text(boot.report)
    kvm, powerdown = qmp(boot.qmp, ("query-kvm", "system_powerdown"))
    observed["kvm"] = kvm.get("return", kvm)
    observed["powerdown"] = "error" not in powerdown
    try:
        observed["exit_status"] = qemu.wait(timeout=SHUTDOWN_TIMEOUT)
    except subprocess.TimeoutExpired:
        observed["exit_status"] = None


def observe(qemu, boot, timeout):
    """Watch one running guest: the login prompt against the timeout, then the report."""
    started = time.monotonic()
    seen, elapsed = wait_for(lambda: prompt_or_exit(qemu, boot.console), timeout, started)
    elapsed = elapsed if seen == "prompt" else None
    observed = {"outcome": login_outcome(elapsed, timeout), "elapsed": elapsed, "timeout": timeout}
    observed.update({"seen": seen, "report": "", "kvm": None, "exit_status": qemu.poll()})
    if observed["outcome"] == "reached":
        shut_down(qemu, boot, observed)
    return observed


def boot_guest(context, boot, profile, artifact, timeout=None):
    """Boot the pinned artifact once and return what the harness observed.

    The digest is checked before anything is written for the boot and again on
    the file the overlay is laid over, immediately before swtpm and QEMU start,
    so a refused artifact starts no program. Both lead their own session and are
    ended on every way out of this function.
    """
    timeout = context.timeout if timeout is None else timeout
    nonce = context.nonce()
    prepared = prepare_guest(context, boot, profile, artifact, nonce)
    argv = guest_argv(profile, boot)
    with session(swtpm_argv(boot), boot.dir / "swtpm.log", context.spawned):
        wait_for_path(boot.tpm_socket, SOCKET_TIMEOUT)
        with session(argv, boot.dir / "qemu.log", context.spawned) as qemu:
            observed = observe(qemu, boot, timeout)
    if observed["exit_status"] is None:
        observed["exit_status"] = qemu.returncode
    observed.update({"nonce": nonce, "argv": argv, "profile": profile, **prepared})
    observed["fields"] = guest_fields(observed["report"])
    retain(boot.dir / "result.json", observed)
    return observed


def retain(path, observed):
    """Write one boot's observations beside its console log."""
    path.write_text(json.dumps(observed, indent=2, default=str) + "\n", encoding="utf-8")


def guest_fields(report_text):
    """Return the `AEGIS-M24-<name> <value>` fields between the guest's two markers."""
    lines = clean(report_text).splitlines()[:MAX_REPORT_LINES]
    begin, end = f"{GUEST_MARK}-BEGIN", f"{GUEST_MARK}-END"
    if begin not in lines or end not in lines:
        return {}
    fields = {}
    for line in lines[lines.index(begin) + 1 : lines.index(end)]:
        name, _space, value = line.partition(" ")
        if name.startswith(f"{GUEST_MARK}-"):
            fields[name[len(GUEST_MARK) + 1 :]] = value.strip()
    return fields


def pcr_reading(fields, index):
    """Return (value, annotation) for one PCR the guest reported."""
    value = fields.get(f"PCR-{index}", "absent").lower()
    if PCR_VALUE.fullmatch(value) is None:
        return value, "unreadable"
    if value.strip("0") == "":
        return value, "reset (never extended)"
    return value, "extended"


def efi_state(fields, name):
    """Return the value byte of a guest EFI variable, or 'absent'."""
    parts = fields.get(name, "absent").split()
    if len(parts) != 5 or not all(part.isdigit() for part in parts):
        return "absent"
    return parts[-1]


def host_pcrs():
    """Return the host TPM's own PCR values, where this account can read them."""
    values = {}
    for index in PCR_INDICES:
        try:
            values[index] = (HOST_PCRS / str(index)).read_text(encoding="utf-8").strip().lower()
        except OSError:
            continue
    return values


def boot_notes(observed):
    """Return the lines every boot prints: timing, status and the PCR reading."""
    fields = observed["fields"]
    elapsed = observed["elapsed"]
    timing = "not seen" if elapsed is None else f"{elapsed:.1f} s"
    notes = [
        f"login prompt: {timing} against the recorded {observed['timeout']} s "
        f"-> {observed['outcome']}",
        f"nonce handed to the guest: {observed['nonce']}; reported: {fields.get('NONCE', 'none')}",
        f"guest kernel: {fields.get('UNAME-R', 'not reported')}; "
        f"TPM major {fields.get('TPM-MAJOR', '?')}",
        f"guest SecureBoot {efi_state(fields, 'SECUREBOOT')}, "
        f"SetupMode {efi_state(fields, 'SETUPMODE')}; guest systemd-tpm2-setup.service: "
        f"{fields.get('TPM2-SETUP', 'not reported')}",
    ]
    for index in PCR_INDICES:
        value, annotation = pcr_reading(fields, index)
        notes.append(f"PCR {index:>2} sha256 {value} ({annotation})")
    notes.append(f"QEMU exit status: {observed['exit_status']}; KVM over QMP: {observed['kvm']}")
    return notes


def boot_problems(observed):
    """Return what a completed boot failed to show, whatever its firmware."""
    fields = observed["fields"]
    problems = []
    if observed["outcome"] != "reached":
        problems.append(f"the login prompt was {observed['outcome']} ({observed['seen']})")
    if fields.get("NONCE") != observed["nonce"]:
        problems.append("the report does not carry this boot's nonce; it proves nothing about it")
    for index in FIRMWARE_PCRS:
        if pcr_reading(fields, index)[1] != "extended":
            problems.append(f"PCR {index} was not extended; the TPM did not record the firmware")
    if pcr_reading(fields, 11)[1] == "unreadable":
        problems.append("PCR 11 could not be read from inside the guest")
    return problems + exit_problems(observed)


def exit_problems(observed):
    """Return how the guest's end or its accelerator fell short of a clean KVM run."""
    problems = []
    if observed["exit_status"] != 0 or not observed.get("powerdown"):
        problems.append(f"the guest did not power off cleanly: {observed['exit_status']}")
    kvm = observed["kvm"] or {}
    if not (kvm.get("enabled") and kvm.get("present")):
        problems.append(f"QMP does not report KVM enabled: {kvm}")
    return problems


def positive_case(context):
    """Positive: the artifact boots headless under KVM and reports its PCRs with this nonce."""
    observed = boot_guest(context, Boot(context.run_dir / "positive"), "secure", context.artifact)
    problems = boot_problems(observed)
    fields = observed["fields"]
    if efi_state(fields, "SECUREBOOT") != "1" or efi_state(fields, "SETUPMODE") != "0":
        problems.append("the guest does not report Secure Boot on with its keys enrolled")
    host = host_pcrs()
    if host.get(0) and host.get(0) == pcr_reading(fields, 0)[0]:
        problems.append("the guest PCR 0 equals the host's own; the reading is not the guest's")
    notes = boot_notes(observed)
    notes.append(f"artifact sha256 checked immediately before boot: {observed['digest']}")
    notes += [f"guest store: {line}" for line in observed["enrolled"]]
    notes.append(f"host PCR 0 for contrast: {host.get(0, 'unreadable')}")
    notes.append(f"retained: {context.run_dir / 'positive'}")
    report("boot/positive", problems, notes)
    return problems, observed


def contrast_case(context, positive):
    """The same bytes on the firmware without Secure Boot: PCR 7 records the difference."""
    observed = boot_guest(context, Boot(context.run_dir / "contrast"), "plain", context.artifact)
    problems = boot_problems(observed)
    fields, first = observed["fields"], positive["fields"]
    if efi_state(fields, "SECUREBOOT") == "1":
        problems.append("the guest reports Secure Boot on without the Secure Boot firmware")
    if pcr_reading(fields, 7)[0] == pcr_reading(first, 7)[0]:
        problems.append("PCR 7 did not change with the firmware's Secure Boot state")
    same11 = pcr_reading(fields, 11)[0] == pcr_reading(first, 11)[0]
    notes = boot_notes(observed)
    notes.append(
        f"PCR 7 differs from boot/positive: {pcr_reading(fields, 7)[0] != pcr_reading(first, 7)[0]}"
    )
    notes.append(f"PCR 11 equals boot/positive (same UKI, same boot phases): {same11}")
    report("boot/firmware-contrast", problems, notes)
    return problems


def timeout_negative_problems(observed):
    """Return why a short-timeout boot does not show the timeout path itself.

    A miss counts only when the deadline expired with QEMU still running (the
    poll saw neither a prompt nor an exit) and the harness then ended QEMU with
    a signal (a negative exit status). A QEMU that crashed or exited on its own
    before the deadline is also a miss, but it would not show the timeout path.
    """
    problems = []
    if observed["outcome"] != "missed":
        problems.append(f"a {observed['timeout']} s timeout was {observed['outcome']}")
    if observed["seen"] is not None:
        problems.append(f"the poll ended on {observed['seen']!r}, not on the deadline")
    status = observed["exit_status"]
    if not isinstance(status, int) or status >= 0:
        problems.append(f"QEMU ended with status {status}, not by the harness's signal")
    return problems


def timeout_negative_case(context):
    """Negative: with a deliberately short timeout the harness reports the miss and stops."""
    boot = Boot(context.run_dir / "timeout")
    observed = boot_guest(context, boot, "secure", context.artifact, TOO_SHORT_TIMEOUT)
    problems = timeout_negative_problems(observed)
    tail = [line for line in console_text(boot.console).splitlines() if line.strip()][-1:]
    seen = "neither a prompt nor an exit" if observed["seen"] is None else observed["seen"]
    notes = [
        f"recorded timeout for this run: {TOO_SHORT_TIMEOUT} s (deliberately short)",
        f"harness decision: {observed['outcome']}; seen by the deadline: {seen}",
        f"QEMU exit status after the harness ended its session: {observed['exit_status']}",
        f"last console line when stopped: {tail[0][:160] if tail else 'nothing yet'}",
    ]
    report("boot/login-timeout-negative", problems, notes)
    return problems


def timeout_boundary_case(timeout):
    """Boundary: one second under, at and one second over the timeout are two outcomes."""
    points = (timeout - 1, timeout, timeout + 1)
    decided = [(point, login_outcome(point, timeout)) for point in points]
    problems = []
    if [outcome for _point, outcome in decided] != ["reached", "reached", "missed"]:
        problems.append(f"expected reached, reached, missed; got {decided}")
    if len({outcome for _point, outcome in decided}) != 2:
        problems.append("the three points were not reported as two outcomes")
    notes = [f"recorded timeout {timeout} s, inclusive; these are rule evaluations, not boots"]
    notes += [f"prompt after {point} s -> {outcome}" for point, outcome in decided]
    report("boot/timeout-boundary", problems, notes)
    return problems


def esp_extent(table):
    """Return (first LBA, sector count) of the EFI system partition in a GPT read from LBA 0."""
    if table[512:520] != b"EFI PART":
        raise GateError("the artifact carries no GPT header at LBA 1")
    entries_lba, count, size = struct.unpack_from("<QII", table, 584)
    if count > MAX_GPT_ENTRIES or size < 128:
        raise GateError(f"the GPT declares {count} entries of {size} bytes")
    for index in range(count):
        start = entries_lba * 512 + index * size
        entry = table[start : start + size]
        if len(entry) < 48:
            break
        if uuid.UUID(bytes_le=bytes(entry[:16])) == ESP_TYPE:
            first, last = struct.unpack_from("<QQ", entry, 32)
            return first, last - first + 1
    raise GateError("the artifact's GPT names no EFI system partition")


def extract_uki(context, work):
    """Copy the artifact's UKI out of its ESP; the pinned image is read and never written."""
    require_pinned(context.pin, context.artifact)
    image = option_path(context.artifact)
    table = work / "gpt.bin"
    dd = ["qemu-img", "dd", "-f", "qcow2", "-O", "raw"]
    code, _stdout, stderr = run(
        dd + ["bs=512", "count=34", f"if={image}", f"of={table}"], TOOL_TIMEOUT
    )
    if code != 0:
        raise GateError(f"qemu-img could not read the partition table: {stderr.strip()}")
    first, sectors = esp_extent(table.read_bytes())
    if (first * 512) % (1 << 20) or (sectors * 512) % (1 << 20):
        raise GateError("the ESP is not MiB-aligned; the extraction refuses to guess")
    esp = work / "esp.raw"
    window = [f"skip={first * 512 >> 20}", f"count={sectors * 512 >> 20}"]
    code, _stdout, stderr = run(dd + ["bs=1M", *window, f"if={image}", f"of={esp}"], TOOL_TIMEOUT)
    if code != 0:
        raise GateError(f"qemu-img could not copy the ESP: {stderr.strip()}")
    uki = work / "uki.efi"
    source = "::" + context.pin["boot"]["uki-path"]
    code, _stdout, stderr = run(["mcopy", "-n", "-i", str(esp), source, str(uki)], TOOL_TIMEOUT)
    if code != 0:
        raise GateError(f"mcopy could not read {source}: {stderr.strip()}")
    return uki


def sign_uki(context, uki, signed):
    """Sign the UKI with this run's key; return the two sbverify outcomes against that key."""
    key = context.key
    argv = ["sbsign", "--key", str(key["key"]), "--cert", str(key["cert"]), "--output", str(signed)]
    code, _stdout, stderr = run(argv + [str(uki)], TOOL_TIMEOUT)
    if code != 0:
        raise GateError(f"sbsign could not sign the UKI: {stderr.strip()[-300:]}")
    verify = ["sbverify", "--cert", str(key["cert"])]
    signed_code, signed_out, signed_err = run(verify + [str(signed)], TOOL_TIMEOUT)
    plain_code, plain_out, plain_err = run(verify + [str(uki)], TOOL_TIMEOUT)
    return (
        (signed_code, (signed_out + signed_err).strip()),
        (plain_code, (plain_out + plain_err).strip()),
    )


def firmware_verdict(qemu, console):
    """Return what the firmware did with the binary: started a kernel, refused it, or exited."""
    text = console_text(console)
    if KERNEL_SECURE in text:
        return "kernel started with Secure Boot enabled"
    if FIRMWARE_DENIED in text:
        return "firmware refused the image (Access Denied)"
    if qemu.poll() is not None:
        return f"QEMU exited {qemu.returncode}"
    return None


def firmware_boot(context, boot, binary):
    """Boot one EFI binary from a QEMU-synthesised FAT directory on the custom-only store."""
    loader = boot.dir / "esp" / "EFI" / "BOOT"
    loader.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(binary, loader / "BOOTX64.EFI")
    boot.console.unlink(missing_ok=True)
    guest_store(context, boot, "custom-only")
    argv = machine_argv("secure", boot)
    argv += ["-drive", f"file=fat:{option_path(boot.dir / 'esp')},format=raw,if=virtio,readonly=on"]
    argv += ["-serial", f"file:{option_path(boot.console)}"]
    with session(argv, boot.dir / "qemu.log", context.spawned) as qemu:
        started = time.monotonic()
        verdict, elapsed = wait_for(
            lambda: firmware_verdict(qemu, boot.console), FIRMWARE_BOOT_TIMEOUT, started
        )
    return verdict or f"nothing within {FIRMWARE_BOOT_TIMEOUT} s", elapsed


def uki_signing_case(context):
    """The UKI signed with this run's key verifies, and only the signed copy boots on that key."""
    work = context.run_dir / "uki"
    work.mkdir(parents=True, exist_ok=True)
    uki = extract_uki(context, work)
    signed = work / "uki.signed.efi"
    (signed_code, signed_text), (plain_code, plain_text) = sign_uki(context, uki, signed)
    accepted, accepted_after = firmware_boot(context, Boot(context.run_dir / "uki-signed"), signed)
    refused, refused_after = firmware_boot(context, Boot(context.run_dir / "uki-unsigned"), uki)
    problems = []
    if signed_code != 0:
        problems.append(f"sbverify refused the signed UKI: {signed_text[-200:]}")
    if plain_code == 0:
        problems.append("sbverify accepted the UKI without this run's signature")
    if not accepted.startswith("kernel started"):
        problems.append(f"the signed UKI did not boot on the custom-only store: {accepted}")
    if not refused.startswith("firmware refused"):
        problems.append(f"the unsigned UKI was not refused by the firmware: {refused}")
    notes = [
        f"UKI copied from the artifact's ESP: {context.pin['boot']['uki-path']}, "
        f"sha256 {file_digest(uki)}",
        f"signed with {context.key['subject']}: sha256 {file_digest(signed)}",
        f"sbverify --cert <run key> signed: exit {signed_code} ({signed_text.splitlines()[-1:]})",
        f"sbverify --cert <run key> unsigned: exit {plain_code} ({plain_text.splitlines()[-1:]})",
        f"custom-only store, signed UKI: {accepted} after {accepted_after:.1f} s",
        f"custom-only store, same UKI unsigned: {refused} after {refused_after:.1f} s",
    ]
    report("boot/uki-signed", problems, notes)
    return problems


def pinned_bytes_case(context):
    """The cached artifact still hashes to the pin after every boot that used it."""
    found = file_digest(context.artifact)
    problems = [] if found == context.pin["sha256"] else [f"the cached artifact now hashes {found}"]
    mode = oct(context.artifact.stat().st_mode & 0o777)
    notes = [f"sha256 after every boot: {found}", f"mode {mode}; every boot wrote to an overlay"]
    report("boot/pinned-bytes-unchanged", problems, notes)
    return problems


def run_cases(context):
    """Run every case in order and return the number that failed."""
    outcomes = [host_secure_boot_case()]
    if artifact_case(context):
        raise GateError("the pinned artifact was refused before boot, so nothing was booted")
    context.key = generate_key(context)
    outcomes += [tampered_case(context), bad_signature_case(context)]
    outcomes.append(version_boundary_case(context.pin))
    problems, positive = positive_case(context)
    outcomes += [problems, contrast_case(context, positive), timeout_negative_case(context)]
    outcomes += [timeout_boundary_case(context.timeout), uki_signing_case(context)]
    outcomes.append(pinned_bytes_case(context))
    return sum(1 for problems in outcomes if problems)


def measure(context, count):
    """Boot the artifact `count` times and print the login-prompt times against the timeout."""
    context.key = generate_key(context)
    times = []
    for index in range(min(count, MAX_MEASURED_BOOTS)):
        boot = Boot(context.run_dir / f"measure-{index}")
        observed = boot_guest(context, boot, "secure", context.artifact)
        problems = boot_problems(observed)
        report(f"boot/measure-{index}", problems, boot_notes(observed)[:1])
        if problems:
            return 1
        times.append(observed["elapsed"])
    if not times:
        print(f"FAIL: --measure {count} booted nothing; give 1 to {MAX_MEASURED_BOOTS}")
        return 1
    print(
        f"     {len(times)} boots: min {min(times):.1f} s, "
        f"median {statistics.median(times):.1f} s, "
        f"max {max(times):.1f} s; recorded timeout {context.timeout} s, "
        f"margin over the slowest {context.timeout - max(times):.1f} s"
    )
    return 0


def discard_bulk(run_dir):
    """Remove the overlays, stores, images and keys a run wrote; keep its logs and results."""
    if not run_dir.is_dir():
        return
    for path in sorted(run_dir.rglob("*"))[:MAX_PRUNED_FILES]:
        if path.is_dir():
            if path.name in BULKY_DIRECTORIES:
                shutil.rmtree(path, ignore_errors=True)
        elif path.suffix in BULKY_SUFFIXES:
            # Not only regular files: a killed guest leaves its QMP socket behind.
            path.unlink(missing_ok=True)


def prune_runs(store):
    """Keep the newest MAX_RETAINED_RUNS run directories."""
    runs = store / "runs"
    if not runs.is_dir():
        return
    names = sorted(path for path in runs.iterdir() if path.is_dir())[:MAX_PRUNED_FILES]
    for old in names[:-MAX_RETAINED_RUNS]:
        shutil.rmtree(old, ignore_errors=True)


def terminated(signum, _frame):
    """Turn a termination signal into SystemExit, so every session is ended on the way out."""
    raise SystemExit(128 + signum)


def install_signal_handlers():
    """Route SIGTERM, SIGHUP and SIGQUIT through `terminated`; return the signals handled.

    SIGINT already raises KeyboardInterrupt. Left at their defaults, SIGHUP (a
    closed terminal or a dropped SSH session) and SIGQUIT end Python without its
    finally blocks, and swtpm and QEMU, in sessions of their own, never see the
    hangup. SIGHUP and SIGQUIT are POSIX-only, so each is looked up (HISS-21).
    """
    handled = []
    for name in TERMINATING_SIGNALS:
        number = getattr(signal, name, None)
        if number is not None:
            signal.signal(number, terminated)
            handled.append(name)
    return handled


def measure_count(text):
    """Parse --measure: a whole number of boots from 1 to MAX_MEASURED_BOOTS."""
    try:
        count = int(text)
    except ValueError as error:
        raise argparse.ArgumentTypeError(f"{text!r} is not a whole number") from error
    if not 1 <= count <= MAX_MEASURED_BOOTS:
        raise argparse.ArgumentTypeError(f"{count} is not from 1 to {MAX_MEASURED_BOOTS}")
    return count


def parse_arguments(argv):
    """Return the parsed options."""
    parser = argparse.ArgumentParser(description=__doc__.splitlines()[0])
    parser.add_argument("--pin", type=Path, default=PIN, help="the artifact pin to boot")
    parser.add_argument(
        "--fetch", action="store_true", help="download and cache the pinned artifact first"
    )
    parser.add_argument(
        "--measure",
        type=measure_count,
        default=None,
        help=f"only boot N times (1 to {MAX_MEASURED_BOOTS}) and print the prompt times",
    )
    return parser.parse_args(argv)


def preflight(context, fetch_first):
    """Return the reasons the harness cannot run here; fetch first when asked."""
    reasons = check_toolchain() + check_firmware() + check_kvm()
    if fetch_first and not reasons:
        fetch_pinned(context)
    return reasons + check_cache(context)


def main(argv=None):
    """Run the gate: SKIP with a reason, FAIL with a reason, or PASS with every case above it."""
    options = parse_arguments(argv)
    install_signal_handlers()
    store = cache_dir()
    run_dir = store / "runs" / f"{time.strftime('r%Y%m%dT%H%M%S')}-{secrets.token_hex(2)}"
    print(f"Boot harness gate (M24, D62, D72, D84; development evidence only). Cache: {store}")
    try:
        context = Context(load_pin(options.pin), store, run_dir)
        reasons = preflight(context, options.fetch)
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    if reasons:
        for reason in reasons[:MAX_PROBLEM_LINES]:
            print(f"SKIP: {reason}; the boot harness gate did not run.")
        return 0
    run_dir.mkdir(parents=True, exist_ok=True)
    print(f"     artifact: {context.artifact} ({context.pin['name']} {context.pin['version']})")
    print(f"     run directory: {run_dir}")
    try:
        if options.measure is None:
            failed = run_cases(context)
        else:
            failed = measure(context, options.measure)
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    except Refused as refusal:
        print(f"FAIL: the pinned artifact was refused at {refusal.stage} mid-run: {refusal}")
        return 1
    finally:
        discard_bulk(run_dir)
        prune_runs(store)
    if failed:
        print(f"FAIL: {failed} boot harness case(s) did not match their recorded outcome.")
        return 1
    print(
        "PASS: boot harness on the reference profile (M24, D62, D72, D84). The pinned "
        "artifact booted under KVM with OVMF and swtpm; this is development evidence only. "
        "It closes no image, boot, hardware or release gate and is not evidence for M11."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
