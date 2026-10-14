#!/usr/bin/env python3
"""Run the systemd and mkosi definition gates inside a pinned Arch Linux image.

Both gates run the host's own tools and skip below their floors: systemd 261
for systemd-repart and systemd-sysupdate, mkosi 27. The ubuntu-26.04 runner
ships systemd 259 and no mkosi, so on the runner both gates could only skip.
This module gives them a pinned userland instead, recorded in
build/definitions/toolchain.pin.json:

* `--fetch` (`make definitions-fetch`, the one networked step) pulls the base
  image by digest, installs the pinned packages from one Arch Linux Archive
  snapshot, and refuses the image unless pacman reports every package at
  exactly its pinned version;
* `dispatch()` runs one gate script inside that image, without a network and
  with the repository and the root file system read-only, when the run sets
  AEGIS_DEFINITIONS_CONTAINER=1.

The choice is explicit. A run that asks for the container and finds no image
fails; it never falls back to the host's tools, and a run that does not ask
never reaches for the container.
"""

import argparse
import hashlib
import json
import os
import shutil
import subprocess
import sys
import tempfile
from pathlib import Path

import gate_policy
from host import gid, uid

ROOT = Path(__file__).resolve().parent.parent
PIN = ROOT / "build" / "definitions" / "toolchain.pin.json"
VARIABLE = "AEGIS_DEFINITIONS_CONTAINER"
# The only values VARIABLE may take: unset or empty selects the host, "1" the
# container. Anything else is refused rather than read as either.
SELECTS_CONTAINER = "1"
ENGINES = ("podman", "docker")
IMAGE_NAME = "localhost/aegis-definitions"
TAG_LENGTH = 16
IN_REPO = "/repo"
PIN_KEYS = ("schema", "note", "image", "archive", "packages")
PULL_TIMEOUT = 900
BUILD_TIMEOUT = 1800
INSPECT_TIMEOUT = 60
GATE_TIMEOUT = 1800
MAX_PACKAGES = 32


class FetchError(Exception):
    """The pinned image could not be produced or did not match its pin."""


def unique_keys(pairs):
    """object_pairs_hook for json: refuse an object that names one key twice."""
    keys = [key for key, _ in pairs]
    repeated = sorted({key for key in keys if keys.count(key) > 1})
    if repeated:
        raise FetchError(f"the pin names {repeated} more than once")
    return dict(pairs)


def load_pin(path=PIN):
    """Return (pin, raw bytes); refuse a pin whose top-level keys differ from PIN_KEYS."""
    raw = path.read_bytes()
    pin = json.loads(raw, object_pairs_hook=unique_keys)
    if sorted(pin) != sorted(PIN_KEYS):
        raise FetchError(f"{path} has keys {sorted(pin)}, expected {sorted(PIN_KEYS)}")
    if not 0 < len(pin["packages"]) <= MAX_PACKAGES:
        raise FetchError(f"{path} must name between 1 and {MAX_PACKAGES} packages")
    return pin, raw


def image_tag(raw):
    """Return the local image name for a pin: any change to the pin file changes it."""
    return f"{IMAGE_NAME}:{hashlib.sha256(raw).hexdigest()[:TAG_LENGTH]}"


def containerfile(pin):
    """Return the Containerfile that installs the pinned packages from the snapshot."""
    packages = " ".join(sorted(pin["packages"]))
    server = pin["archive"]["server"]
    return (
        f"FROM {pin['image']['reference']}\n"
        f"RUN printf 'Server = %s\\n' '{server}' > /etc/pacman.d/mirrorlist \\\n"
        f" && pacman -Syuu --noconfirm --needed {packages} \\\n"
        f" && pacman -Scc --noconfirm\n"
    )


def version_problems(pinned, listing):
    """Return one line per package whose `pacman -Q` version differs from its pin.

    `listing` is pacman's output, one `name version` pair per line. A package
    is matched by its whole name, so `systemd-libs` never stands in for
    `systemd`.
    """
    found = {}
    for line in listing.splitlines()[: MAX_PACKAGES * 2]:
        fields = line.split()
        if len(fields) == 2:
            found[fields[0]] = fields[1]
    problems = []
    for name in sorted(pinned):
        actual = found.get(name)
        if actual != pinned[name]:
            problems.append(f"{name}: pinned {pinned[name]}, image has {actual or 'nothing'}")
    return problems


def selected(environ=None):
    """Return (True or False, None), or (None, reason) for a value that is neither."""
    environ = os.environ if environ is None else environ
    value = environ.get(VARIABLE, "")
    if value == "":
        return False, None
    if value == SELECTS_CONTAINER:
        return True, None
    return None, f"{VARIABLE}={value!r} is neither empty nor {SELECTS_CONTAINER!r}"


def pick_engine():
    """Return (engine, None), or (None, reason) when neither podman nor docker is here."""
    for engine in ENGINES:
        if shutil.which(engine) is not None:
            return engine, None
    return None, "neither podman nor docker is on PATH"


def run(argv, timeout, capture=True):
    """Run `argv` with a hard deadline; return (exit code, stdout, stderr)."""
    try:
        done = subprocess.run(argv, capture_output=capture, text=True, timeout=timeout, check=False)
    except (subprocess.TimeoutExpired, OSError) as error:
        return 1, "", f"{argv[0]} did not complete: {error}"
    return done.returncode, done.stdout or "", done.stderr or ""


def image_present(engine, tag):
    """Return whether the engine stores `tag` locally."""
    code, _, _ = run([engine, "image", "inspect", tag], INSPECT_TIMEOUT)
    return code == 0


def user_arguments(engine):
    """docker runs as root unless told otherwise; run it as the invoking user."""
    if engine != "docker":
        return []
    user, _reason = uid()
    group, _reason = gid()
    return [] if user is None or group is None else ["--user", f"{user}:{group}"]


def gate_argv(engine, tag, script, environ=None):
    """Return the command line that runs tools/`script` inside the pinned image.

    No pull, no network, a read-only root with a scratch /tmp, and the
    repository mounted read-only. Inside, VARIABLE is empty so the gate runs on
    the image's tools, and the run's required gates are passed on unchanged.
    """
    environ = os.environ if environ is None else environ
    required = environ.get(gate_policy.VARIABLE, "")
    command = [engine, "run", "--rm", "--pull=never", "--network=none"]
    command += ["--read-only", "--tmpfs", "/tmp"] + user_arguments(engine)
    command += ["--mount", f"type=bind,source={ROOT},target={IN_REPO},readonly"]
    command += ["--workdir", IN_REPO, "--env", "HOME=/tmp", "--env", "SYSTEMD_COLORS=0"]
    command += ["--env", f"{VARIABLE}=", "--env", f"{gate_policy.VARIABLE}={required}"]
    return command + [tag, "python3", "-B", f"tools/{script}"]


def ready(environ=None):
    """Return (engine, pin, tag, None) when the selected container can run, else a reason last."""
    _chosen, problem = selected(environ)
    if problem is not None:
        return None, None, None, problem
    if "," in str(ROOT):
        return None, None, None, f"{ROOT} contains a comma, which --mount cannot carry"
    engine, problem = pick_engine()
    if problem is not None:
        return None, None, None, problem
    try:
        pin, raw = load_pin()
    except (OSError, ValueError, FetchError) as error:
        return None, None, None, f"{PIN} cannot be read: {error}"
    tag = image_tag(raw)
    if not image_present(engine, tag):
        return None, None, None, f"{engine} does not store {tag}; run `make definitions-fetch`"
    return engine, pin, tag, None


def dispatch(gate, script, environ=None):
    """Return None when the run does not select the container; else the gate's exit code.

    A run that selects the container and cannot use it fails whether or not
    the gate is required: the container was asked for by name, and running the
    host's tools instead would report a path that was not the one requested.
    """
    chosen, _ = selected(environ)
    if chosen is False:
        return None
    engine, pin, tag, problem = ready(environ)
    if problem is not None:
        print(f"FAIL: {problem}; the {gate} definition gate did not run.")
        return 1
    print(
        f"{gate} definition gate: running in {tag} with {engine} "
        f"({pin['image']['reference']}, Arch Linux Archive {pin['archive']['snapshot']}).",
        flush=True,
    )
    code, _, stderr = run(gate_argv(engine, tag, script, environ), GATE_TIMEOUT, capture=False)
    if stderr:
        print(f"FAIL: {stderr}")
    return code


def build(engine, pin, tag):
    """Pull the base image and build `tag` from it; raise FetchError on any failure."""
    reference = pin["image"]["reference"]
    code, _, stderr = run([engine, "pull", reference], PULL_TIMEOUT, capture=False)
    if code != 0:
        raise FetchError(f"{engine} pull {reference} exited {code} {stderr}".strip())
    with tempfile.TemporaryDirectory(prefix="aegis-definitions-") as context:
        (Path(context) / "Containerfile").write_text(containerfile(pin))
        argv = [engine, "build", "-t", tag, "-f", f"{context}/Containerfile", context]
        code, _, stderr = run(argv, BUILD_TIMEOUT, capture=False)
    if code != 0:
        raise FetchError(f"{engine} build exited {code} {stderr}".strip())


def check_versions(engine, pin, tag):
    """Raise FetchError, and remove `tag`, unless every package has its pinned version."""
    argv = [engine, "run", "--rm", "--pull=never", "--network=none", tag, "pacman", "-Q"]
    code, stdout, stderr = run(argv + sorted(pin["packages"]), INSPECT_TIMEOUT)
    problems = version_problems(pin["packages"], stdout) if code == 0 else [stderr.strip()]
    if problems:
        run([engine, "rmi", "--force", tag], INSPECT_TIMEOUT)
        raise FetchError("the built image differs from its pin: " + "; ".join(problems))


def fetch():
    """Produce the pinned image; return 0, or 1 after saying why it was refused."""
    engine, problem = pick_engine()
    if problem is not None:
        print(f"FAIL: {problem}; nothing was fetched.")
        return 1
    pin, raw = load_pin()
    tag = image_tag(raw)
    try:
        build(engine, pin, tag)
        check_versions(engine, pin, tag)
    except FetchError as error:
        print(f"FAIL: {error}")
        return 1
    pinned = ", ".join(f"{name} {pin['packages'][name]}" for name in sorted(pin["packages"]))
    print(f"PASS: {tag} built with {engine} and holds exactly {pinned}.")
    return 0


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--fetch", action="store_true", help="build the pinned image")
    options = parser.parse_args()
    if not options.fetch:
        parser.error("the module runs gates through dispatch(); pass --fetch to build the image")
    return fetch()


if __name__ == "__main__":
    sys.exit(main())
