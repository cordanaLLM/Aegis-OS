#!/usr/bin/env python3
"""Build the pinned kernel with the M18 fragment and read the result back from a guest.

Decision D70: Aegis builds the kernel here while Nucleus is a scaffold, exactly
as D56 keeps image-definition validation here while Imago is a scaffold. What
this gate produces is development evidence on the reference profile. It is never
a release artifact: nothing is packaged, signed, installed, published or written
to a device, and no boot, hardware or release gate is closed by a pass.

Seven cases, no simulation and no failure suppression:

* ``kernel/positive`` applies the two tracked fragments to ``x86_64_defconfig``
  and requires the produced ``.config`` to satisfy every row of
  ``build/kernel-requirement.json``;
* ``kernel/negative-contradictory`` appends a fragment that contradicts a
  required option and requires the gate to refuse the produced configuration;
* ``kernel/boundary-builtin-where-module`` appends a fragment that makes a
  module-state row built-in and requires the same refusal;
* ``kernel/boundary-module-where-builtin`` asks for a module where the schema
  demands built-in, records that kconfig drops the request silently because the
  symbol is a bool, and requires the check to refuse the state anyway;
* ``kernel/readback-positive`` boots the built image and requires the guest to
  print its own ``/proc/config.gz`` and a release string only this build makes;
* ``kernel/readback-negative-missing-option`` feeds the host's own running
  configuration to the same check and requires a refusal;
* ``kernel/readback-boundary-host-kernel`` requires the identity check to refuse
  the host's release string, so the read-back cannot be satisfied by the kernel
  the gate is running on.

Nothing is built inside the repository. The source tree, the object tree and the
guest live under ``AEGIS_KERNEL_BUILD_DIR`` (default
``${XDG_CACHE_HOME:-$HOME/.cache}/aegis-kernel``), and only the pin, the
fragments, the guest init and this gate are tracked.
"""

import argparse
import gzip
import hashlib
import json
import os
import re
import shutil
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
SOURCE_PIN = ROOT / "build" / "kernel" / "source.pin.json"
REQUIREMENT = ROOT / "build" / "kernel-requirement.json"
SUPPORT_FRAGMENT = ROOT / "build" / "kernel" / "10-base-support.config"
REQUIREMENT_FRAGMENT = ROOT / "build" / "kernel" / "50-aegis-requirement.config"
GUEST_INIT = ROOT / "tools" / "guest" / "aegis-readback-init.sh"
HOST_CONFIG = Path("/proc/config.gz")

# Deadlines. Every external command carries one, so a wedged compiler, a stalled
# download or a guest that never powers off fails the gate instead of hanging it.
VERSION_TIMEOUT = 30
DOWNLOAD_TIMEOUT = 1800
EXTRACT_TIMEOUT = 1800
CONFIGURE_TIMEOUT = 900
BUILD_TIMEOUT = 5400
BOOT_TIMEOUT = 180
GPG_TIMEOUT = 120

# Scalar bounds.
MAX_FEATURES = 64
MAX_CONFIG_LINES = 100000
MAX_GUEST_LINES = 200000
MAX_PROBLEM_LINES = 24
# One mebibyte per chunk; 4096 of them bound the digest read at 4 GiB, far above
# any kernel tarball and far below anything worth hashing by mistake.
MAX_DIGEST_CHUNKS = 4096
MAX_DIAGNOSTIC_LINES = 20

GUEST_MARK = "AEGIS-M26"
GUEST_MEMORY = "1024"
# The userspace in the guest is the host's own; only the kernel under test is
# built here. These are the four programs the init script calls plus the shell
# that runs it, and their shared-library closure is resolved with ldd.
GUEST_PROGRAMS = ("bash", "mount", "uname", "gzip", "sleep")
MAX_GUEST_LIBRARIES = 64

# Toolchain admission. The floor of every row is the floor the *pinned source*
# declares -- scripts/min-tool-version.sh and Documentation/process/changes.rst
# of linux-7.2.5 -- not a number chosen here. A row with floor None is a tool
# the pinned source declares no floor for; its reference value is recorded so
# the admission is still a pin rather than whatever the workstation ships.
# Every reference value was read back from the tool on the reference profile.
TOOLCHAIN = (
    ("gcc", ["gcc", "--version"], r"gcc \(GCC\) (\d+(?:\.\d+)*)", "8.1.0", "16.2.1"),
    ("ld", ["ld", "--version"], r"GNU ld \(GNU Binutils\) (\d+(?:\.\d+)*)", "2.30", "2.47"),
    ("make", ["make", "--version"], r"GNU Make (\d+(?:\.\d+)*)", "4.0", "4.4.1"),
    ("bc", ["bc", "--version"], r"bc (\d+(?:\.\d+)*)", "1.06.95", "1.08.2"),
    ("flex", ["flex", "--version"], r"flex (\d+(?:\.\d+)*)", "2.5.35", "2.6.4"),
    ("bison", ["bison", "--version"], r"bison \(GNU Bison\) (\d+(?:\.\d+)*)", "2.0", "3.8.2"),
    ("pahole", ["pahole", "--version"], r"v(\d+(?:\.\d+)*)", "1.26", "1.31"),
    ("tar", ["tar", "--version"], r"tar \(GNU tar\) (\d+(?:\.\d+)*)", "1.28", "1.35"),
    ("perl", ["perl", "--version"], r"\(v(\d+(?:\.\d+)*)", None, "5.42.2"),
    ("cpio", ["cpio", "--version"], r"cpio \(GNU cpio\) (\d+(?:\.\d+)*)", None, "2.15"),
    ("xz", ["xz", "--version"], r"xz \(XZ Utils\) (\d+(?:\.\d+)*)", None, "5.8.3"),
    ("gzip", ["gzip", "--version"], r"gzip (\d+(?:\.\d+)*)", None, "1.14"),
    ("bash", ["bash", "--version"], r"version (\d+(?:\.\d+)*)", "4.2", "5.3.15"),
    ("mount", ["mount", "--version"], r"util-linux (\d+(?:\.\d+)*)", "2.10", "2.42.3"),
    ("gpg", ["gpg", "--version"], r"gpg \(GnuPG\) (\d+(?:\.\d+)*)", None, "2.4.9"),
    (
        "qemu-system-x86_64",
        ["qemu-system-x86_64", "--version"],
        r"QEMU emulator version (\d+(?:\.\d+)*)",
        None,
        "11.1.1",
    ),
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
            reasons.append(f"{name} {found} is below the pinned source's floor {floor}")
    return reasons


def build_base():
    """Return the out-of-repository directory every build artifact lives under."""
    override = os.environ.get("AEGIS_KERNEL_BUILD_DIR")
    if override:
        return Path(override).expanduser()
    cache = os.environ.get("XDG_CACHE_HOME")
    root = Path(cache).expanduser() if cache else Path.home() / ".cache"
    return root / "aegis-kernel"


def file_digest(path):
    """Return the SHA-256 of `path` as lower-case hexadecimal.

    The read loop is bounded rather than open-ended: a tarball larger than
    [`MAX_DIGEST_CHUNKS`] mebibytes is refused instead of hashed, because a
    file that size is not the source this pin describes.
    """
    digest = hashlib.sha256()
    with path.open("rb") as handle:
        for _ in range(MAX_DIGEST_CHUNKS):
            chunk = handle.read(1 << 20)
            if not chunk:
                return digest.hexdigest()
            digest.update(chunk)
    raise GateError(f"{path.name} exceeds {MAX_DIGEST_CHUNKS} MiB; it is not the pinned source")


def fetch(url, target, timeout):
    """Download `url` to `target` unless it is already there."""
    if target.exists():
        return
    code, _stdout, stderr = run(
        ["curl", "--fail", "--silent", "--show-error", "--location", "--output", str(target), url],
        timeout,
    )
    if code != 0:
        target.unlink(missing_ok=True)
        raise GateError(f"{url} could not be downloaded: {stderr.strip()}")


def import_key(keyring, fingerprint):
    """Make sure `fingerprint` is in the gate's own keyring, fetching it once if not."""
    keyring.mkdir(parents=True, exist_ok=True)
    keyring.chmod(0o700)
    listed = run(["gpg", "--homedir", str(keyring), "--list-keys", fingerprint], GPG_TIMEOUT)
    if listed[0] == 0:
        return
    code, _stdout, stderr = run(
        [
            "gpg",
            "--homedir",
            str(keyring),
            "--keyserver",
            "hkps://keyserver.ubuntu.com",
            "--recv-keys",
            fingerprint,
        ],
        GPG_TIMEOUT,
    )
    if code != 0:
        raise GateError(f"the signing key {fingerprint} could not be obtained: {stderr.strip()}")


def verify_signature(keyring, signature, tarball, fingerprint):
    """Verify the detached signature over the *uncompressed* tarball.

    ``Popen`` takes no deadline of its own, so the decompressor's is
    structural: the ``finally`` kills it on every path out of the body. Without
    it, a ``gpg`` that exceeds [`EXTRACT_TIMEOUT`] raises past the ``kill`` and
    ``Popen.__exit__`` waits on the still-running child with no bound at all --
    the gate hanging where its own deadline says it should fail.
    """
    with subprocess.Popen(
        ["xz", "--decompress", "--stdout", str(tarball)], stdout=subprocess.PIPE
    ) as source:
        try:
            code, _stdout, stderr = run(
                ["gpg", "--homedir", str(keyring), "--verify", str(signature), "-"],
                EXTRACT_TIMEOUT,
                stdin=source.stdout,
            )
        finally:
            source.kill()
    if code != 0:
        raise GateError(f"the pinned tarball's signature did not verify: {stderr.strip()}")
    if fingerprint.replace(" ", "") not in stderr.replace(" ", ""):
        raise GateError(f"the tarball signature is not from the pinned key {fingerprint}")
    return [line for line in stderr.splitlines() if "Good signature" in line]


def acquire_source(pin, base):
    """Download, verify and extract the pinned source; return its directory."""
    base.mkdir(parents=True, exist_ok=True)
    tarball = base / pin["tarball"]
    signature = base / Path(pin["signature-url"]).name
    fetch(pin["tarball-url"], tarball, DOWNLOAD_TIMEOUT)
    fetch(pin["signature-url"], signature, DOWNLOAD_TIMEOUT)
    found = file_digest(tarball)
    if found != pin["tarball-sha256"]:
        raise GateError(f"{tarball.name} hashes to {found}, not the pinned {pin['tarball-sha256']}")
    print(f"     source digest: sha256 {found} matches the pin")
    fingerprint = pin["signing-keys"]["tarball"]["fingerprint"]
    import_key(base / "gnupg", fingerprint)
    for line in verify_signature(base / "gnupg", signature, tarball, fingerprint):
        print(f"     source signature: {line.removeprefix('gpg: ').strip()}")
    tree = base / "src" / f"linux-{pin['version']}"
    if not (tree / "Makefile").is_file():
        (base / "src").mkdir(parents=True, exist_ok=True)
        code, _stdout, stderr = run(
            ["tar", "-xf", str(tarball), "-C", str(base / "src")], EXTRACT_TIMEOUT
        )
        if code != 0:
            raise GateError(f"the pinned tarball did not extract: {stderr.strip()}")
    return tree


def parse_config(text):
    """Return a produced kernel configuration as a symbol to state map.

    ``y``, ``m`` and ``n`` are the three states a requirement row can ask
    about. A string or integer assignment is recorded verbatim, so a caller
    that asks about one sees the value rather than a guessed state.
    """
    states = {}
    for line in text.splitlines()[:MAX_CONFIG_LINES]:
        unset = re.fullmatch(r"# (CONFIG_[A-Za-z0-9_]+) is not set", line)
        if unset is not None:
            states[unset.group(1)] = "n"
            continue
        assigned = re.fullmatch(r"(CONFIG_[A-Za-z0-9_]+)=(.*)", line)
        if assigned is not None:
            states[assigned.group(1)] = assigned.group(2)
    return states


def satisfied(required, observed):
    """Return True when `observed` satisfies `required`, failing closed on None.

    This mirrors ``RequiredState::satisfied_by`` in
    ``crates/aegis-fabrica-defs``: a symbol the configuration says nothing
    about never satisfies a row, not even an ``absent`` one, because an
    unrecorded symbol is an unanswered question rather than a measured absence.
    """
    if observed is None:
        return False
    if required == "built-in":
        return observed == "y"
    if required == "module":
        return observed == "m"
    if required == "present":
        return observed != "n"
    if required == "absent":
        return observed == "n"
    return False


def requirement_rows():
    """Return the M18 requirement payload's feature rows, bounded."""
    payload = json.loads(REQUIREMENT.read_text())
    return payload["features"][:MAX_FEATURES]


def unsatisfied(rows, states):
    """Return one line per requirement row the configuration does not satisfy."""
    problems = []
    for row in rows:
        observed = states.get(row["symbol"])
        if satisfied(row["state"], observed):
            continue
        seen = "unrecorded" if observed is None else repr(observed)
        problems.append(
            f"{row['symbol']}: {row['required-by']} requires {row['state']}, observed {seen}"
        )
    return problems


def configure(tree, output, extra):
    """Produce one `.config` from the named base plus the tracked fragments."""
    if output.exists():
        shutil.rmtree(output)
    output.mkdir(parents=True)
    pin = json.loads(SOURCE_PIN.read_text())
    code, _stdout, stderr = run(
        ["make", f"O={output}", pin["base-configuration-target"]], CONFIGURE_TIMEOUT, cwd=tree
    )
    if code != 0:
        raise GateError(f"{pin['base-configuration']} did not resolve: {stderr.strip()}")
    fragments = [str(SUPPORT_FRAGMENT), str(REQUIREMENT_FRAGMENT)]
    if extra is not None:
        scratch = output / "case.config"
        scratch.write_text(extra)
        fragments.append(str(scratch))
    merge = str(tree / "scripts" / "kconfig" / "merge_config.sh")
    code, _stdout, stderr = run(
        [merge, "-m", "-O", str(output), str(output / ".config"), *fragments],
        CONFIGURE_TIMEOUT,
        cwd=tree,
    )
    if code != 0:
        raise GateError(f"the fragments did not merge: {stderr.strip()}")
    code, _stdout, stderr = run(
        ["make", f"O={output}", "olddefconfig"], CONFIGURE_TIMEOUT, cwd=tree
    )
    if code != 0:
        raise GateError(f"olddefconfig refused the merged configuration: {stderr.strip()}")
    return output / ".config"


def compile_kernel(tree, output, jobs):
    """Build the pinned target and return (seconds, artifact path, release)."""
    pin = json.loads(SOURCE_PIN.read_text())
    started = time.monotonic()
    code, _stdout, stderr = run(
        ["make", f"O={output}", f"-j{jobs}", *pin["build-targets"]], BUILD_TIMEOUT, cwd=tree
    )
    elapsed = time.monotonic() - started
    if code != 0:
        tail = "\n".join(stderr.splitlines()[-MAX_DIAGNOSTIC_LINES:])
        raise GateError(f"the pinned target did not build:\n{tail}")
    release = (output / "include" / "config" / "kernel.release").read_text().strip()
    return elapsed, output / pin["build-artifact"], release


def library_closure(program):
    """Return the shared objects `program` loads, read back from ldd."""
    code, stdout, _stderr = run(["ldd", program], VERSION_TIMEOUT)
    if code != 0:
        raise GateError(f"ldd could not resolve {program}")
    found = re.findall(r"(/[^\s]+\.so[^\s]*)", stdout)
    return sorted(set(found))[:MAX_GUEST_LIBRARIES]


def populate_guest(tree):
    """Fill an initramfs tree with the host userspace the init script calls."""
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
        # developer account, that bit would drop PID 1 from root to the
        # developer uid and mount(8) would refuse its own mount.
        copied.chmod(0o755)
        libraries.update(library_closure(program))
    (tree / "usr" / "bin" / "sh").symlink_to("bash")
    for library in sorted(libraries)[:MAX_GUEST_LIBRARIES]:
        shutil.copy2(library, tree / "usr" / "lib" / Path(library).name)
    init = tree / "init"
    shutil.copy2(GUEST_INIT, init)
    init.chmod(0o755)


def build_initramfs(base):
    """Assemble the read-back initramfs and return the archive path."""
    tree = base / "guest" / "root"
    if tree.exists():
        shutil.rmtree(tree)
    tree.mkdir(parents=True)
    populate_guest(tree)
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
            timeout=CONFIGURE_TIMEOUT,
            check=False,
        )
    if done.returncode != 0:
        raise GateError(f"the initramfs did not archive: {done.stderr.decode(errors='replace')}")
    return archive


def boot(image, initramfs, log):
    """Boot the built image with the read-back guest and return what it printed.

    ``-cpu max`` is not decoration. The guest userspace is this host's own
    glibc, which is compiled for a recent x86-64 feature level; the emulator's
    default model does not have those instructions and the dynamic loader dies
    on an invalid opcode before ``/init`` runs. Hardware virtualisation is used
    when the reference profile's ``/dev/kvm`` is usable and the run falls back
    to emulation when it is not, because a boot that needs KVM would make the
    gate unrunnable rather than slow.
    """
    report_file = log.with_name("readback.txt")
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
        "2",
        "-serial",
        "stdio",
        "-serial",
        f"file:{report_file}",
        "-kernel",
        str(image),
        "-initrd",
        str(initramfs),
        "-append",
        "console=ttyS0 panic=5 rdinit=/init",
    ]
    if os.access("/dev/kvm", os.R_OK | os.W_OK):
        argv.insert(1, "-enable-kvm")
    code, stdout, stderr = run(argv, BOOT_TIMEOUT, stdin=subprocess.DEVNULL)
    log.write_text((stdout + stderr).replace("\r\n", "\n").replace("\r", "\n"))
    if code != 0:
        raise GateError(f"the guest exited {code}; its console is in {log}")
    if not report_file.exists():
        raise GateError(f"the guest wrote no read-back; its console is in {log}")
    text = report_file.read_text(errors="replace")
    return text.replace("\r\n", "\n").replace("\r", "\n")


def guest_section(console, name):
    """Return the text the guest printed between one pair of markers."""
    start = f"{GUEST_MARK}-{name}-BEGIN"
    end = f"{GUEST_MARK}-{name}-END"
    lines = console.splitlines()[:MAX_GUEST_LINES]
    if start not in lines or end not in lines:
        raise GateError(f"the guest printed no {name} section; it did not reach the read-back")
    return "\n".join(lines[lines.index(start) + 1 : lines.index(end)])


def guest_field(console, name):
    """Return one `AEGIS-M26-<name> <value>` line's value, as the guest printed it."""
    prefix = f"{GUEST_MARK}-{name} "
    for line in console.splitlines()[:MAX_GUEST_LINES]:
        if line.startswith(prefix):
            return line[len(prefix) :].strip()
    raise GateError(f"the guest printed no {name} line")


def identity_problems(reported, built, host):
    """Refuse a read-back whose release is not the one this build produced."""
    problems = []
    if reported != built:
        problems.append(f"the guest reports {reported!r}, not the built release {built!r}")
    if reported == host:
        problems.append(f"the guest reports the host's own release {host!r}")
    return problems


def report(name, problems, notes=()):
    """Print one case's outcome and, when it failed, why."""
    print(f"{'PASS' if not problems else 'FAIL'} {name}")
    for note in notes:
        print(f"     {note}")
    for line in problems[:MAX_PROBLEM_LINES]:
        print(f"     {line}")


def positive_case(tree, base, rows):
    """Case 1: the tracked fragments produce a configuration that meets every row."""
    produced = configure(tree, base / "out" / "positive", None)
    problems = unsatisfied(rows, parse_config(produced.read_text()))
    report("kernel/positive", problems, [f"produced {produced}"])
    return produced, problems


def refusal_case(tree, base, rows, case):
    """One case that must be refused, with the symbol the refusal has to name."""
    name, directory, extra, symbol = case
    produced = configure(tree, base / "out" / directory, extra)
    problems = unsatisfied(rows, parse_config(produced.read_text()))
    failures = []
    if not problems:
        failures.append("the contradictory configuration was accepted")
    elif not any(line.startswith(f"{symbol}:") for line in problems):
        failures.append(f"the refusal does not name {symbol}: {problems}")
    report(name, failures, [f"refused: {line}" for line in problems[:2]])
    return failures


def module_boundary_case(tree, base, rows):
    """Boundary: a module asked for where the schema demands built-in is rejected.

    The interesting half is what kconfig does with the request. Every symbol
    the payload demands built-in is a bool in the pinned source, so ``=m`` is
    not a state any of them can reach; kconfig neither honours the line nor
    complains about it, and resolves the symbol to ``n`` instead. A fragment
    that asks for a module therefore produces a kernel *without* the feature
    rather than one with it as a module, which is exactly the silently
    non-conforming outcome this gate exists to refuse.
    """
    symbol = "CONFIG_BPF_SYSCALL"
    produced = configure(tree, base / "out" / "boundary-module", f"{symbol}=m\n")
    states = parse_config(produced.read_text())
    problems = unsatisfied(rows, states)
    failures = []
    if not any(line.startswith(f"{symbol}:") for line in problems):
        failures.append(f"a fragment asking {symbol}=m was not refused: {problems}")
    notes = [f"kconfig resolved a requested 'm' on the bool {symbol} to {states.get(symbol)!r}"]
    notes.extend(f"refused: {line}" for line in problems[:3])
    report("kernel/boundary-module-where-builtin", failures, notes)
    return failures


def readback_case(base, produced, image, release, rows):
    """Boot the built image and check what the guest says about itself.

    Three independent claims have to hold together. The guest's release must be
    the one this build produced and must not be the host's, so the read-back
    cannot be satisfied by the machine running the gate. Every requirement row
    must hold in the text the guest itself printed, not in the build tree. And
    that text must be the produced ``.config`` byte for byte, which is the part
    no amount of guessing can fake: it is 5,520 lines the running kernel
    carries in its own image.
    """
    initramfs = build_initramfs(base)
    reported_text = boot(image, initramfs, base / "guest" / "console.log")
    reported = guest_field(reported_text, "UNAME-R")
    captured = guest_section(reported_text, "CONFIG")
    (base / "guest" / "guest-config").write_text(captured + "\n")
    host = os.uname().release
    problems = identity_problems(reported, release, host)
    problems.extend(unsatisfied(rows, parse_config(captured)))
    identical = captured.strip() == produced.read_text().strip()
    if not identical:
        problems.append("the guest's own configuration differs from the produced .config")
    verdict = "is" if identical else "is NOT"
    report(
        "kernel/readback-positive",
        problems,
        [
            f"guest uname -r: {reported}   host uname -r: {host}",
            f"guest uname -v: {guest_field(reported_text, 'UNAME-V')}",
            f"the guest's own /proc/config.gz {verdict} byte-identical to {produced}",
        ],
    )
    return problems


def host_readback_cases(rows, release):
    """The read-back's own negative and boundary: the host kernel must be refused.

    One outcome list per case, so a run that fails both counts two failures
    rather than one. Only the negative case needs the host's own configuration;
    the boundary case reads ``os.uname()`` alone, so a kernel built without
    ``CONFIG_IKCONFIG_PROC`` skips the negative case *by name* and never takes
    the boundary case down with it.
    """
    outcomes = []
    if HOST_CONFIG.exists():
        states = parse_config(gzip.decompress(HOST_CONFIG.read_bytes()).decode(errors="replace"))
        missing = unsatisfied(rows, states)
        negative = [] if missing else ["the host's own configuration satisfied every required row"]
        shown = [f"refused: {line}" for line in missing[:2]]
        report("kernel/readback-negative-missing-option", negative, shown)
        outcomes.append(negative)
    else:
        print(f"SKIP kernel/readback-negative-missing-option: {HOST_CONFIG} does not exist.")
    host = os.uname().release
    identity = identity_problems(host, release, host)
    boundary = [] if identity else ["the host's release passed the identity check"]
    report("kernel/readback-boundary-host-kernel", boundary, [f"refused: {i}" for i in identity])
    outcomes.append(boundary)
    return outcomes


REFUSALS = (
    (
        "kernel/negative-contradictory",
        "negative",
        "# CONFIG_PREEMPT_RT is not set\n",
        "CONFIG_PREEMPT_RT",
    ),
    ("kernel/boundary-builtin-where-module", "boundary-builtin", "CONFIG_VFIO=y\n", "CONFIG_VFIO"),
)


def run_cases(tree, base, jobs):
    """Run every case in order and return the number that failed."""
    rows = requirement_rows()
    print(f"     {len(rows)} requirement rows from {REQUIREMENT.relative_to(ROOT)}")
    produced, failed = positive_case(tree, base, rows)
    count = 1 if failed else 0
    for case in REFUSALS:
        count += 1 if refusal_case(tree, base, rows, case) else 0
    count += 1 if module_boundary_case(tree, base, rows) else 0
    elapsed, image, release = compile_kernel(tree, base / "out" / "positive", jobs)
    print(f"     built {image.name} as {release} in {elapsed:.0f}s")
    count += 1 if readback_case(base, produced, image, release, rows) else 0
    count += sum(1 for outcome in host_readback_cases(rows, release) if outcome)
    return count


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jobs", type=int, default=os.cpu_count() or 1)
    arguments = parser.parse_args()
    base = build_base()
    print(f"Kernel build gate (D70, development evidence only). Build tree: {base}")
    reasons = check_toolchain()
    if reasons:
        for reason in reasons[:MAX_PROBLEM_LINES]:
            print(f"SKIP: {reason}; the kernel build gate did not run.")
        return 0
    try:
        pin = json.loads(SOURCE_PIN.read_text())
        print(f"     pinned source: linux-{pin['version']} on {pin['base-configuration']}")
        failed = run_cases(acquire_source(pin, base), base, arguments.jobs)
    except GateError as error:
        print(f"FAIL: the gate could not run: {error}")
        return 1
    if failed:
        print(f"FAIL: {failed} kernel build case(s) did not match their recorded outcome.")
        return 1
    print(
        "PASS: kernel build gate on the reference profile (D70). A kernel was "
        "built and booted in a guest as development evidence; nothing was "
        "packaged, signed or installed, and the boot, hardware and release "
        "gates remain blocked."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
