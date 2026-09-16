"""Render a path the way the Linux consumer of a gate's output reads it.

HISS-21 (platform neutrality) forbids a gate whose emitted artifact only works on
the platform that generated it. These gates do not merely *run* on a host: they
write systemd unit text, build `systemd-repart` and `systemd-sysupdate` command
lines, and name sysfs attributes. Every one of those is read by Linux, so the
separator is a property of the *target*, never of the machine that happened to
produce it.

`str(Path("/defs"))` renders the host separator. On Windows that produced
`--definitions=\\defs` on a repart command line and `Path=\\img.raw` inside a
`.transfer` unit -- artifacts that are wrong rather than merely unportable, and
which no Linux-only CI run could ever have surfaced.

`target()` renders the POSIX form on every host. It is deliberately not
`os.path.normpath` or `filepath.ToSlash`'s Python analogue applied at the call
site: a single named function is what the HISS-21 sweep in
`tools/test_host.py` can find, so a new call site that forgets it is caught
by a test rather than by a Linux user reading a broken unit file.
"""

import os
import pathlib
import tempfile
from pathlib import PurePath, PureWindowsPath

# A scalar bound on the path length this helper will render (HISS-02). Linux caps
# a path at PATH_MAX; anything longer is a caller defect, not a value to pass on.
MAX_TARGET_PATH = 4096


def target(path):
    """Return `path` spelled the way the Linux tool that consumes it reads it.

    Positive: a POSIX path renders unchanged on every host. Negative: a value
    longer than `MAX_TARGET_PATH` is refused rather than emitted. Boundary: a
    Windows path renders with forward separators and without its drive letter,
    because a drive letter has no meaning to the consumer.
    """
    text = str(path)
    if len(text) > MAX_TARGET_PATH:
        raise ValueError(f"path exceeds {MAX_TARGET_PATH} characters; refusing to emit it")
    pure = PurePath(text)
    if isinstance(pure, PureWindowsPath) and pure.drive:
        # A drive-qualified path cannot be handed to a Linux tool as-is. Drop the
        # drive and keep the rooted remainder, which is what a scratch tree under
        # a temporary directory means on either host.
        pure = PureWindowsPath(text[len(pure.drive) :])
    return pure.as_posix()


def kernel_release():
    """Return `(release, None)`, or `(None, reason)` where the host publishes none.

    HISS-21 permits exactly two states for a gate on a platform: running, or
    skipped with the reason printed. `os.uname()` does not exist off POSIX, so a
    gate that called it directly raised `AttributeError` -- neither state, and a
    crash rather than a skip. Callers print the reason and skip the case.

    Positive: a POSIX host returns its release and no reason. Negative: a host
    without `os.uname` returns no release and a reason naming what is absent.
    Boundary: the reason is returned, never logged here, so the caller decides
    where the coverage is recovered.
    """
    uname = getattr(os, "uname", None)
    if uname is None:
        return None, "os.uname() is POSIX-only and absent on this host, so the running kernel release cannot be read; this case is covered on the Linux leg of the platform matrix"
    return uname().release, None


def uid():
    """Return `(uid, None)`, or `(None, reason)` where the host has no POSIX uid.

    Same contract as `kernel_release()`. `os.getuid` is absent on Windows, and a
    gate that drops privileges with `setpriv --reuid` has nothing to pass there.
    """
    return _posix_id("getuid")


def gid():
    """Return `(gid, None)`, or `(None, reason)`. The `setpriv --regid` counterpart."""
    return _posix_id("getgid")


def _posix_id(name):
    """Shared body for the POSIX id probes, so both answer in the same shape."""
    probe = getattr(os, name, None)
    if probe is None:
        return None, (
            f"os.{name}() is POSIX-only and absent on this host, so an unprivileged "
            "re-exec cannot be constructed; this case is covered on the Linux leg "
            "of the platform matrix"
        )
    return probe(), None


def readonly_directory_blocks_removal():
    """Return `(True, None)` where a read-only directory prevents unlinking a file in it.

    HISS-21 names `chmod` semantics as a platform difference that must be guarded
    rather than assumed. POSIX refuses the unlink when the containing directory has
    no write bit; Windows ignores the mode and removes the file, so a case built on
    that refusal cannot run there and must say so instead of reporting a pass.

    The answer is measured on a throwaway directory rather than inferred from the
    platform name, so a POSIX host that happens to allow it -- a container running
    as root, where the write bit is not consulted -- is also reported honestly.
    """
    with tempfile.TemporaryDirectory(prefix="aegis-chmod-probe-") as base:
        probe = pathlib.Path(base) / "probe"
        probe.mkdir()
        victim = probe / "file"
        victim.write_text("x", encoding="utf-8")
        probe.chmod(0o555)
        try:
            victim.unlink()
        except OSError:
            return True, None
        finally:
            probe.chmod(0o755)
    return False, "this host removes a file from a read-only directory, so a refusal cannot be provoked here; this case is covered on the Linux leg of the platform matrix"
