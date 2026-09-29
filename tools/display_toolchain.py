#!/usr/bin/env python3
"""Read the P17 display slice's build toolchain back before the crate gate builds.

Milestone M27, decision D80. `crates/aegis-scaena` depends on cros-libva,
git-pinned, whose build script finds libva through pkg-config and generates its
bindings from the libva headers with bindgen, which loads libclang. None of the
three is a Cargo input, so `cargo build --locked` alone would build against
whatever the host happens to ship. `make verify-rust` runs this first, and it
prints every value it reads next to the admitted floor and the reference-profile
value, so the evidence names what actually built the crate.

Two kinds of row:

* ``TOOLCHAIN`` rows are read back from a program on ``PATH`` under a deadline
  (HISS-02) and held to their floor;
* ``LOCKED`` rows are read back from ``Cargo.lock``, where the version is exact:
  bindgen is cros-libva's build dependency, and cros-libva must come from the
  git revision D80 pins.

It is part of the crate gate, and a crate gate on a host that cannot build the
workspace fails rather than skips: a missing program or a value below its floor
prints ``FAIL:`` and exits 1. The Verification gate's runner builds the libva
floor from its release tarball before this runs (``.github/workflows/ci.yml``).
``docs/roadmap/toolchain-admission.md`` records the same rows, and
``tools/test_display_toolchain.py`` holds the two to each other.
"""

import os
import re
import shutil
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
LOCK = ROOT / "Cargo.lock"

# Every external command carries a deadline, so a wedged pkg-config or clang
# fails the gate instead of hanging it.
VERSION_TIMEOUT = 30
# Scalar bounds on the files and outputs this reads (HISS-02).
MAX_LOCK_BYTES = 1 << 22
MAX_OUTPUT_CHARS = 1 << 16

# (name, argv, pattern, floor, reference). The floor is the lowest value a gate
# may build with; the reference is what the reference profile read on 2026-09-29.
TOOLCHAIN = (
    ("pkgconf", ["pkg-config", "--version"], r"^\s*(\d+(?:\.\d+)*)\s*$", "1.8.1", "3.0.7"),
    (
        "libva",
        ["pkg-config", "--variable=libva_version", "libva"],
        r"^\s*(\d+(?:\.\d+)*)\s*$",
        "2.24.1",
        "2.24.1",
    ),
    (
        "VA-API",
        ["pkg-config", "--modversion", "libva"],
        r"^\s*(\d+(?:\.\d+)*)\s*$",
        "1.24.0",
        "1.24.0",
    ),
    ("clang", ["clang", "--version"], r"clang version (\d+(?:\.\d+)*)", "6.0", "22.1.8"),
)

# (name, exact version, exact source or None for crates.io).
CROS_LIBVA_REV = "59384456ac2ae78c0c3e5515f41ef1efd9b802cf"
LOCKED = (
    ("bindgen", "0.70.1", "registry+https://github.com/rust-lang/crates.io-index"),
    (
        "cros-libva",
        "0.0.13",
        f"git+https://github.com/chromeos/cros-libva?rev={CROS_LIBVA_REV}#{CROS_LIBVA_REV}",
    ),
)


class ToolchainError(Exception):
    """A row could not be read at all, as opposed to one read below its floor."""


def run(argv, timeout=VERSION_TIMEOUT):
    """Run `argv` under a hard deadline and return (exit code, combined output)."""
    env = dict(os.environ)
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
            stdin=subprocess.DEVNULL,
            check=False,
        )
    except subprocess.TimeoutExpired as error:
        raise ToolchainError(f"{argv[0]} exceeded its {timeout}s deadline") from error
    except OSError as error:
        raise ToolchainError(f"{argv[0]} could not be run: {error}") from error
    return done.returncode, (done.stdout + done.stderr)[:MAX_OUTPUT_CHARS]


def version_tuple(text):
    """Return a dotted version as a tuple of integers, for ordering."""
    return tuple(int(part) for part in text.split("."))


def at_least(found, floor):
    """True when `found` is at or above `floor`, missing components counting as zero."""
    left, right = version_tuple(found), version_tuple(floor)
    width = max(len(left), len(right))
    return left + (0,) * (width - len(left)) >= right + (0,) * (width - len(right))


def read_version(row, runner=run, which=shutil.which):
    """Return the version one `TOOLCHAIN` row reads back, or raise why it cannot."""
    name, argv, pattern, _floor, _reference = row
    if which(argv[0]) is None:
        raise ToolchainError(f"{name}: {argv[0]} is not on PATH")
    code, output = runner(argv)
    if code != 0:
        raise ToolchainError(f"{name}: {' '.join(argv)} exited {code}: {output.strip()[:200]}")
    match = re.search(pattern, output, re.MULTILINE)
    if match is None:
        raise ToolchainError(f"{name}: no version matched {pattern!r} in {' '.join(argv)}")
    return match.group(1)


def lock_entries(text):
    """Return ``{name: [(version, source), ...]}`` for every package in a lock text."""
    entries = {}
    for block in text.split("[[package]]")[1:]:
        fields = dict(re.findall(r'^(name|version|source) = "([^"]*)"$', block, re.MULTILINE))
        if "name" in fields and "version" in fields:
            entries.setdefault(fields["name"], []).append((fields["version"], fields.get("source")))
    return entries


def read_lock(path=LOCK):
    """Return the committed lock text, bounded, or raise why it cannot be read."""
    if path.is_symlink() or not path.is_file() or path.stat().st_size > MAX_LOCK_BYTES:
        raise ToolchainError(f"{path.name} is missing, a symlink or larger than {MAX_LOCK_BYTES}")
    return path.read_text(encoding="utf-8")


def check_locked(text):
    """Return one reason per `LOCKED` row the lock does not carry exactly once."""
    entries = lock_entries(text)
    reasons = []
    for name, version, source in LOCKED:
        found = entries.get(name, [])
        if found != [(version, source)]:
            reasons.append(f"{name}: Cargo.lock carries {found}; admitted {version} {source}")
        else:
            print(f"     {name}: {version} (read back from Cargo.lock, {source})")
    return reasons


def check_toolchain(runner=run, which=shutil.which):
    """Print every `TOOLCHAIN` row read back; return one reason per row that fails."""
    reasons = []
    for row in TOOLCHAIN:
        name, argv, _pattern, floor, reference = row
        try:
            found = read_version(row, runner, which)
        except ToolchainError as error:
            reasons.append(str(error))
            continue
        note = "" if found == reference else f" [reference profile recorded {reference}]"
        print(f"     {name}: {found} (read back from {' '.join(argv)}, floor {floor}){note}")
        if not at_least(found, floor):
            reasons.append(f"{name} {found} is below the admitted floor {floor}")
    return reasons


def main():
    """Read every row back; exit 1 with one FAIL line per reason, 0 otherwise."""
    print("display slice build toolchain (M27, D80):")
    try:
        reasons = check_toolchain() + check_locked(read_lock())
    except ToolchainError as error:
        reasons = [str(error)]
    for reason in reasons:
        print(f"FAIL: {reason}")
    if reasons:
        return 1
    print("PASS: the display slice's build toolchain is at or above every admitted floor.")
    return 0


if __name__ == "__main__":
    sys.exit(main())
