#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Verify P05 Forum shell zwlr_layer_shell_v1 surface mapping, AT-SPI tree, focus ring, and timing.

Milestone M28 E28-2, E28-3, E28-4, and E28-6, decisions D79, D82, D115, and ADR-0003.
Starts a private headless KWin compositor (`kwin_wayland --virtual`) inside a private
D-Bus session with at-spi2 infrastructure in a temporary `XDG_RUNTIME_DIR` (never the
operator's display or session bus).


Drives the Rust binaries `aegis-forum-shell-gpui`, `aegis-atspi-readback`, and
`aegis-ring-readback` through twenty-five cases:
1. surface/first-configure: direct connection maps layer shell and receives first configure;
2. surface/filter-control: verify wayland-info sees layer shell filtered out or preserved;
3. surface/layer-shell-missing: compositor lacking layer shell reports LayerShellNotSupportedError;
4. surface/first-configure-deadline: configure that never arrives terminates at deadline;
5. surface/refused-line: invalid request line is refused before touching display;
6. surface/unknown-schema: a request naming another schema is refused for the schema;
7. surface/unknown-layer: a request naming layer "middle" is refused for the layer;
8. surface/parameters-read-back: a second KWin under WAYLAND_DEBUG=server logs the
   get_layer_surface layer and namespace, anchor, exclusive zone and edge and keyboard
   interactivity the shell sent, each once and equal to the reference request;
9. atspi/sound: fixture exposes button, focus, 1000 nodes, hidden subtree; click updates label;
10. atspi/late-client: client started after surface maps receives full tree;
11. atspi/negative-button-as-container: button exposed as container fails;
12. atspi/negative-unlabelled: planted unlabelled node fails and is named;
13. atspi/negative-culled: culling that removes nodes fails 1000-node check;
14. atspi/negative-hidden-shown: unhidden hidden subtree fails;
15. ring/min: minimum camera scale focus ring >= 2.0 px and >= 3:1 contrast;
16. ring/unit: unit camera scale focus ring >= 2.0 px and >= 3:1 contrast;
17. ring/max: maximum camera scale focus ring >= 2.0 px and >= 3:1 contrast;
18. ring/partly-covered: partly covered focused node passes;
19. ring/width-2-0: boundary 2.0 px ring passes;
20. ring/scaled-layer: ring drawn inside scaled layer (1.5 px) fails width floor;
21. ring/under-panel: focused node fully under panel fails;
22. ring/width-1-9: boundary 1.9 px ring fails;
23. timing/1000-nodes: 1000 seeded nodes measured over 600 frames with a11y active;
24. timing/999-nodes: 994 seeded rows (998 tree nodes, < 1000) refused as TooFewNodes;
25. timing/adapter-inactive: fixture run without active a11y adapter refused as AdapterInactive.

Every pass is development evidence for the client half only (D79).
"""

import json
import os
import re
import select
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path, PurePosixPath

ROOT = Path(__file__).resolve().parent.parent
TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import gate_common  # noqa: E402
import host  # noqa: E402

BINARY = "aegis-forum-shell-gpui"
READBACK_BINARY = "aegis-atspi-readback"
RING_READBACK_BINARY = "aegis-ring-readback"
PROGRAMS = (
    "cargo",
    "dbus-run-session",
    "kwin_wayland",
    "wayland-info",
    "dbus-daemon",
    "gdbus",
    "pkg-config",
    "at-spi-bus-launcher",
    "at-spi2-registryd",
    Path(sys.executable).name,
    BINARY,
    READBACK_BINARY,
    RING_READBACK_BINARY,
)

ATSPI_DIRS = (
    "/usr/lib",
    "/usr/libexec",
    "/usr/lib/at-spi2-core",
    "/usr/libexec/at-spi2-core",
)

BUILD_TIMEOUT = 1800
RUN_TIMEOUT = 30
OUTPUT_WIDTH = 1280
OUTPUT_HEIGHT = 720
SHELL_HEIGHT = 48

REFERENCE_PARAMS = {
    "schema": "aegis.p05-p17.mount-shell-canvas.v1",
    "namespace": "aegis-forum",
    "layer": "top",
    "anchor": ["top", "left", "right"],
    "exclusive_zone": 48,
    "exclusive_edge": "top",
    "keyboard_interactivity": "on_demand",
}
REFERENCE_REQUEST = {
    "jsonrpc": "2.0",
    "method": "MOUNT_SHELL_CANVAS",
    "params": REFERENCE_PARAMS,
    "id": 1,
}
# The binary takes exactly one newline-terminated line, as the socket would carry it.
REFERENCE_LINE = json.dumps(REFERENCE_REQUEST) + "\n"
CASE_COUNT = 25

# The read-back compositor: its own socket, log and dbus-run-session, so the debug log
# never slows the timing cases on the main KWin.
DEBUG_SOCKET = "kwin-debug-0"
DEBUG_LOG = "kwin-debug.log"
# Wire values from wlr-layer-shell-unstable-v1 (layer, anchor bits, keyboard_interactivity).
LAYER_WIRE = {"background": 0, "bottom": 1, "top": 2, "overlay": 3}
ANCHOR_WIRE = {"top": 1, "bottom": 2, "left": 4, "right": 8}
KEYBOARD_WIRE = {"none": 0, "exclusive": 1, "on_demand": 2}
# One WAYLAND_DEBUG=server request line, `[time] interface#id.request(args)`; events
# carry ` -> ` and never match.
REQUEST_RE = re.compile(r"^\[[0-9:.]+\] zwlr_layer_(?:shell|surface)_v1#[0-9]+\.([a-z_]+)\((.*)\)$")
UNKNOWN_SCHEMA = "aegis.p05-p17.mount-shell-canvas.v2"
REFUSED_SCHEMA = "the mount request names a schema this build does not admit"
REFUSED_LAYER = "the layer is not background, bottom, top or overlay"

ATSPI_CASES = (
    ("atspi/sound", "sound", False, 0.0),
    ("atspi/late-client", "sound", True, 0.5),
    ("atspi/negative-button-as-container", "button-as-container", False, 0.0),
    ("atspi/negative-unlabelled", "unlabelled", False, 0.0),
    ("atspi/negative-culled", "culled", False, 0.0),
    ("atspi/negative-hidden-shown", "hidden-shown", False, 0.0),
)

RING_CASES = (
    ("ring/min", "min", True),
    ("ring/unit", "unit", True),
    ("ring/max", "max", True),
    ("ring/partly-covered", "partly-covered", True),
    ("ring/width-2-0", "width-2-0", True),
    ("ring/scaled-layer", "scaled-layer", False),
    ("ring/under-panel", "under-panel", False),
    ("ring/width-1-9", "width-1-9", False),
)
# The width a negative variant must measure on its best side: (low, high) in px.
# A negative that fails for another reason (contrast, a missing side) fails its gate case.
RING_NEGATIVE_WIDTH = {
    "scaled-layer": (1.4, 1.6),
    "under-panel": (0.0, 0.5),
    "width-1-9": (1.8, 1.95),
}

# Seconds the gate waits after DONE for the fixture (and a wrapping perf) to exit on its own.
DONE_EXIT_WAIT = 30
# perf report over ten seconds of samples took over 60 s at load 49 (2026-10-18).
PERF_REPORT_TIMEOUT = 300

TIMING_CASES = (
    ("timing/1000-nodes", 1000, True),
    ("timing/999-nodes", 994, False),
    ("timing/adapter-inactive", 1000, False),
)

ALLOCATOR_SYMBOLS = (
    "malloc",
    "calloc",
    "realloc",
    "free",
    "cfree",
    "_int_malloc",
    "_int_free",
    "_int_realloc",
    "malloc_consolidate",
    "__libc_malloc",
    "__libc_free",
    "__libc_calloc",
    "__libc_realloc",
    "tcache_get",
    "tcache_put",
    "__rust_alloc",
    "__rust_dealloc",
    "__rust_realloc",
    "alloc::alloc::",
)

NO_ACTIVATION_CONFIG = (
    '<!DOCTYPE busconfig PUBLIC "-//freedesktop//DTD D-Bus Bus Configuration 1.0//EN"\n'
    ' "http://www.freedesktop.org/standards/dbus/1.0/busconfig.dtd">\n'
    "<busconfig>\n"
    "  <type>session</type>\n"
    "  <keep_umask/>\n"
    "  <listen>unix:tmpdir=/tmp</listen>\n"
    "  <auth>EXTERNAL</auth>\n"
    '  <policy context="default">\n'
    '    <allow send_destination="*" eavesdrop="true"/>\n'
    '    <allow eavesdrop="true"/>\n'
    '    <allow own="*"/>\n'
    "  </policy>\n"
    "</busconfig>\n"
)


def write_no_activation_config(runtime_dir):
    """Writes a minimal session bus config that omits standard_session_servicedirs."""
    config_path = runtime_dir / "session-noactivation.conf"
    config_path.write_text(NO_ACTIVATION_CONFIG, encoding="utf-8")
    return config_path


READBACK_CASES = (
    "atspi/button-exposed",
    "atspi/focus-reported",
    "atspi/seeded-reachable",
    "atspi/every-node-named",
    "atspi/hidden-subtree-hidden",
)
UPDATE_CASE = "atspi/incremental-update"
SEEDED_NODES = 1000
# Measured 2026-10-18: each planted variant fails exactly these readback cases; the named
# case must carry the token. An unlabelled row is also missing from the seeded set.
NEGATIVE_EXPECT = {
    "button-as-container": ({"atspi/button-exposed"}, "atspi/button-exposed", "panel"),
    "unlabelled": (
        {"atspi/seeded-reachable", "atspi/every-node-named"},
        "atspi/every-node-named",
        "517",
    ),
    "culled": ({"atspi/seeded-reachable"}, "atspi/seeded-reachable", "10 of 1000"),
    "hidden-shown": (
        {"atspi/hidden-subtree-hidden"},
        "atspi/hidden-subtree-hidden",
        "Hidden panel",
    ),
}
READY_TIMEOUT = 30.0
CLICK_EVENT_TIMEOUT = 5.0


class GateError(Exception):
    """The gate could not be run, as opposed to a decided test case failure."""


def run_command(argv, timeout, environ=None, cwd=None):
    """Runs a program under timeout and returns (returncode, stdout, stderr)."""
    return gate_common.run_command(
        argv, timeout=timeout, environ=environ, cwd=cwd, gate_error_cls=GateError
    )


def has_unix_fd_passing():
    """Reports whether this Python exposes AF_UNIX with SCM_RIGHTS."""
    return hasattr(socket, "AF_UNIX") and hasattr(socket, "SCM_RIGHTS")


def find_atspi_program(name, dirs=ATSPI_DIRS, exists=os.path.exists):
    """Finds an at-spi program on PATH or in the distributions' libexec directories.

    The directories are Linux paths, so they are joined as POSIX on every host (HISS-21).
    """
    w = shutil.which(name)
    if w is not None:
        return Path(w)
    for d in dirs:
        candidate = PurePosixPath(d) / name
        if exists(str(candidate)):
            return candidate
    return None


def capability_reasons(
    platform=None,
    which=shutil.which,
    unix_fds=has_unix_fd_passing,
    atspi_finder=find_atspi_program,
):
    """Returns missing host capabilities, or an empty list if capable."""
    platform = sys.platform if platform is None else platform
    if not platform.startswith("linux"):
        return [f"the native shell gate needs Linux and a Wayland runtime; this host is {platform}"]
    if not unix_fds():
        return ["AF_UNIX and SCM_RIGHTS are POSIX-only and missing on this host"]
    reasons = []
    for prog in (
        "kwin_wayland",
        "dbus-run-session",
        "wayland-info",
        "cargo",
        "dbus-daemon",
        "gdbus",
        "pkg-config",
    ):
        if which(prog) is None:
            reasons.append(f"{prog} is not installed")
    for atspi_prog in ("at-spi-bus-launcher", "at-spi2-registryd"):
        if atspi_finder(atspi_prog) is None:
            reasons.append(f"{atspi_prog} is not installed")
    return reasons


def executable_from(messages, name=BINARY):
    """Extracts executable path from cargo JSON compiler messages."""
    for line in messages.splitlines()[:20000]:
        if not line.startswith("{"):
            continue
        try:
            msg = json.loads(line)
        except json.JSONDecodeError:
            continue
        target = msg.get("target") or {}
        if msg.get("reason") == "compiler-artifact" and target.get("name") == name:
            if msg.get("executable"):
                return Path(msg["executable"])
    return None


def build_shell_binaries(runner=run_command):
    """Builds shell and both readback binaries with cargo."""
    env = dict(os.environ)
    if "CARGO_BUILD_JOBS" not in env:
        env["CARGO_BUILD_JOBS"] = "2"
    argv = [
        "cargo",
        "build",
        "--locked",
        "-p",
        "aegis-forum-shell-gpui",
        "--bins",
        "--message-format=json-render-diagnostics",
    ]
    code, stdout, stderr = runner(argv, BUILD_TIMEOUT, environ=env, cwd=ROOT)
    if code != 0:
        tail = "\n".join(stderr.strip().splitlines()[-24:])
        raise GateError(f"cargo build failed ({code}):\n{tail}")
    shell_bin = executable_from(stdout, BINARY)
    if shell_bin is None or not shell_bin.is_file():
        raise GateError(f"cargo reported no executable for {BINARY}")
    readback_bin = executable_from(stdout, READBACK_BINARY)
    if readback_bin is None or not readback_bin.is_file():
        raise GateError(f"cargo reported no executable for {READBACK_BINARY}")
    ring_readback_bin = executable_from(stdout, RING_READBACK_BINARY)
    if ring_readback_bin is None or not ring_readback_bin.is_file():
        raise GateError(f"cargo reported no executable for {RING_READBACK_BINARY}")
    return shell_bin, readback_bin, ring_readback_bin


def build_shell_binary(runner=run_command):
    """Builds the aegis-forum-shell-gpui binary with cargo (backward compatibility)."""
    shell_bin, _, _ = build_shell_binaries(runner=runner)
    return shell_bin


def build_timing_shell_binary(runner=run_command):
    """Builds aegis-forum-shell-gpui with the timing feature enabled."""
    env = dict(os.environ)
    if "CARGO_BUILD_JOBS" not in env:
        env["CARGO_BUILD_JOBS"] = "2"
    argv = [
        "cargo",
        "build",
        "--locked",
        "-p",
        "aegis-forum-shell-gpui",
        "--bin",
        BINARY,
        "--features",
        "timing",
        "--message-format=json-render-diagnostics",
    ]
    code, stdout, stderr = runner(argv, BUILD_TIMEOUT, environ=env, cwd=ROOT)
    if code != 0:
        tail = "\n".join(stderr.strip().splitlines()[-24:])
        raise GateError(f"cargo build with timing feature failed ({code}):\n{tail}")
    shell_bin = executable_from(stdout, BINARY)
    if shell_bin is None or not shell_bin.is_file():
        raise GateError(f"cargo reported no executable for {BINARY} with timing feature")
    return shell_bin


def read_zed_commit(cargo_toml_path=ROOT / "Cargo.toml"):
    """Reads the pinned zed commit hash from root Cargo.toml."""
    try:
        text = cargo_toml_path.read_text(encoding="utf-8")
    except OSError:
        return ""
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("gpui") and 'rev = "' in line:
            return line.split('rev = "')[1].split('"')[0]
    return ""


def read_versions(runner=run_command):
    """Queries installed atspi-2 and dbus-daemon versions."""
    code, stdout, _stderr = runner(["pkg-config", "--modversion", "atspi-2"], timeout=10)
    if code != 0 or not stdout.strip():
        raise GateError("pkg-config --modversion atspi-2 failed or returned empty")
    atspi_ver = stdout.strip().splitlines()[0]

    code, stdout, _stderr = runner(["dbus-daemon", "--version"], timeout=10)
    if code != 0 or not stdout.strip():
        raise GateError("dbus-daemon --version failed or returned empty")
    dbus_first_line = stdout.strip().splitlines()[0]
    dbus_ver = dbus_first_line.split()[-1] if dbus_first_line else "unknown"

    return {"atspi-2": atspi_ver, "dbus": dbus_ver}


def kwin_version(runner=run_command):
    """Queries kwin_wayland --version."""
    code, stdout, _stderr = runner(["kwin_wayland", "--version"], timeout=10)
    if code == 0 and stdout.strip():
        return stdout.strip().splitlines()[0]
    return "kwin_wayland (unknown version)"


KWIN_CONFIG = "[KDE]\nAnimationDurationFactor=0\n"


def write_kwin_config(runtime_dir):
    """Writes a gate-owned KWin configuration with animations off; returns its directory.

    KWin otherwise reads the host user's configuration, and its fade-in effect leaves a
    window half-composited for a capture taken right after the client's first frame.
    """
    config_dir = runtime_dir / "config"
    config_dir.mkdir(exist_ok=True)
    for name in ("kdeglobals", "kwinrc"):
        (config_dir / name).write_text(KWIN_CONFIG, encoding="utf-8")
    return config_dir


def kwin_command(socket_name, config_file=None, dbus_addr=None):
    """Returns the argument list for headless KWin, wrapped in a private bus unless given one."""
    kwin = [
        "kwin_wayland",
        "--virtual",
        "--socket",
        socket_name,
        "--no-lockscreen",
        "--width",
        str(OUTPUT_WIDTH),
        "--height",
        str(OUTPUT_HEIGHT),
    ]
    if dbus_addr is not None:
        return kwin
    cmd = ["dbus-run-session"]
    if config_file is not None:
        cmd.append(f"--config-file={config_file}")
    return cmd + ["--"] + kwin


def kwin_env(runtime_dir, dbus_addr=None):
    """Returns KWin's environment: no host display, gate-owned runtime and config dirs."""
    env = dict(os.environ)
    env.pop("WAYLAND_DISPLAY", None)
    env.pop("DISPLAY", None)
    env["XDG_RUNTIME_DIR"] = str(runtime_dir)
    env["XDG_CONFIG_HOME"] = str(write_kwin_config(runtime_dir))
    if dbus_addr is not None:
        env["DBUS_SESSION_BUS_ADDRESS"] = dbus_addr
    return env


def start_kwin(
    runtime_dir,
    socket_name="kwin-wayland-0",
    config_file=None,
    dbus_addr=None,
    popen_fn=subprocess.Popen,
    log_name="kwin.log",
    extra_env=None,
):
    """Launches private headless KWin instance."""
    socket_path = runtime_dir / socket_name
    log_file_path = runtime_dir / log_name
    log_file = open(log_file_path, "w", encoding="utf-8")
    env = dict(kwin_env(runtime_dir, dbus_addr), **(extra_env or {}))
    cmd = kwin_command(socket_name, config_file, dbus_addr)
    # The child holds its own copy of the descriptor; the parent's is not needed past Popen.
    with log_file:
        proc = popen_fn(
            cmd,
            stdout=log_file,
            stderr=subprocess.STDOUT,
            env=env,
            cwd=runtime_dir,
            start_new_session=True,
        )
    for _ in range(200):
        if socket_path.exists():
            return proc, socket_path
        if proc.poll() is not None:
            break
        time.sleep(0.1)

    log_tail = ""
    if log_file_path.exists():
        lines = log_file_path.read_text(encoding="utf-8", errors="replace").splitlines()
        log_tail = "\n".join(lines[-24:])
    raise GateError(f"kwin_wayland failed to create socket {socket_path}:\n{log_tail}")


def start_filter(listen_path, upstream_path, mode, popen_fn=subprocess.Popen):
    """Launches wayland_filter proxy in background and awaits READY line."""
    filter_script = TOOLS_DIR / "wayland_filter.py"
    cmd = [
        sys.executable,
        str(filter_script),
        "--listen",
        str(listen_path),
        "--upstream",
        str(upstream_path),
        "--mode",
        mode,
        "--seconds",
        "120",
    ]
    proc = popen_fn(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        start_new_session=True,
    )
    ready_line = f"READY {listen_path}"
    for _ in range(100):
        line = proc.stdout.readline().strip()
        if line == ready_line:
            return proc
        if proc.poll() is not None:
            break
        time.sleep(0.1)
    raise GateError(f"wayland_filter ({mode}) failed to announce ready at {listen_path}")


def start_dbus_daemon(runtime_dir, config_file=None, popen_fn=subprocess.Popen):
    """Starts private dbus-daemon and returns (proc, session_bus_address)."""
    socket_path = runtime_dir / "a11y-session-bus"
    cmd = [
        "dbus-daemon",
        f"--config-file={config_file}" if config_file is not None else "--session",
        "--nofork",
        "--print-address=1",
        f"--address=unix:path={socket_path}",
    ]
    proc = popen_fn(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        text=True,
        start_new_session=True,
    )
    for _ in range(100):
        line = proc.stdout.readline().strip()
        if line.startswith("unix:path="):
            return proc, line
        if proc.poll() is not None:
            break
        time.sleep(0.05)
    raise GateError("dbus-daemon failed to print session bus address")


def enable_a11y_status(env, runner=run_command):
    """Enables org.a11y.Status IsEnabled via gdbus."""
    cmd = [
        "gdbus",
        "call",
        "--session",
        "-d",
        "org.a11y.Bus",
        "-o",
        "/org/a11y/bus",
        "-m",
        "org.freedesktop.DBus.Properties.Set",
        "org.a11y.Status",
        "IsEnabled",
        "<true>",
    ]
    for _ in range(50):
        code, _out, _err = runner(cmd, timeout=5, environ=env)
        if code == 0:
            return
        time.sleep(0.1)
    raise GateError("failed to set org.a11y.Status IsEnabled to true")


def parse_a11y_address(gdbus_output):
    """Extracts unix:path=... from gdbus call output."""
    raw = gdbus_output.strip()
    if "'" in raw:
        return raw.split("'")[1]
    if '"' in raw:
        return raw.split('"')[1]
    return raw.strip("(), ")


def wait_for_a11y_registry(a11y_addr, env, runner=run_command):
    """Waits for at-spi2-registryd to answer Accessible.GetChildren on the a11y bus."""
    cmd = [
        "gdbus",
        "call",
        "--address",
        a11y_addr,
        "-d",
        "org.a11y.atspi.Registry",
        "-o",
        "/org/a11y/atspi/accessible/root",
        "-m",
        "org.a11y.atspi.Accessible.GetChildren",
    ]
    for _ in range(50):
        code, _out, _err = runner(cmd, timeout=5, environ=env)
        if code == 0:
            return
        time.sleep(0.1)
    raise GateError("at-spi2-registryd not ready on a11y bus")


def query_a11y_address(env, runner=run_command):
    """Queries the a11y bus address from org.a11y.Bus via gdbus."""
    cmd = [
        "gdbus",
        "call",
        "--session",
        "-d",
        "org.a11y.Bus",
        "-o",
        "/org/a11y/bus",
        "-m",
        "org.a11y.Bus.GetAddress",
    ]
    code, addr_out, _err = runner(cmd, timeout=5, environ=env)
    if code != 0 or not addr_out.strip():
        raise GateError("failed to get a11y bus address from org.a11y.Bus")
    return parse_a11y_address(addr_out)


def start_a11y_bus(
    runtime_dir,
    config_file=None,
    session_bus=None,
    popen_fn=subprocess.Popen,
    runner=run_command,
    atspi_finder=find_atspi_program,
):
    """Launches private D-Bus session and at-spi2 infrastructure in runtime_dir."""
    procs = []
    if session_bus is not None:
        session_addr = session_bus
    else:
        daemon_proc, session_addr = start_dbus_daemon(
            runtime_dir, config_file=config_file, popen_fn=popen_fn
        )
        procs.append(daemon_proc)

    env = dict(os.environ)
    env["DBUS_SESSION_BUS_ADDRESS"] = session_addr
    env["XDG_RUNTIME_DIR"] = str(runtime_dir)
    env["GSETTINGS_BACKEND"] = "memory"

    launcher_path = atspi_finder("at-spi-bus-launcher")
    if launcher_path is None:
        raise GateError("at-spi-bus-launcher not found")
    launcher_proc = popen_fn(
        [str(launcher_path)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=env,
        start_new_session=True,
    )
    procs.append(launcher_proc)

    enable_a11y_status(env, runner=runner)

    registryd_path = atspi_finder("at-spi2-registryd")
    if registryd_path is None:
        raise GateError("at-spi2-registryd not found")
    registryd_proc = popen_fn(
        [str(registryd_path)],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        env=env,
        start_new_session=True,
    )
    procs.append(registryd_proc)

    a11y_addr = query_a11y_address(env, runner=runner)
    wait_for_a11y_registry(a11y_addr, env, runner=runner)
    return procs, env


def judge_first_configure(code, stdout, output_width=OUTPUT_WIDTH, shell_height=SHELL_HEIGHT):
    """Judges case 1: surface/first-configure."""
    if code != 0:
        return False, f"exited {code}, expected 0"
    expected = f"OUTCOME configured {output_width}x{shell_height}"
    for line in stdout.splitlines():
        if line.strip() == expected:
            return True, ""
    return False, f"missing expected line '{expected}' in stdout: {stdout.strip()[:100]}"


def judge_filter_control(hide_code, hide_stdout, drop_code, drop_stdout):
    """Judges case 2: surface/filter-control."""
    if hide_code != 0:
        return False, f"wayland-info on hide proxy exited {hide_code}"
    if drop_code != 0:
        return False, f"wayland-info on drop proxy exited {drop_code}"
    if "xdg_wm_base" not in hide_stdout:
        return False, "hide proxy wayland-info did not list xdg_wm_base"
    if "zwlr_layer_shell_v1" in hide_stdout:
        return False, "hide proxy wayland-info unexpectedly listed zwlr_layer_shell_v1"
    if "zwlr_layer_shell_v1" not in drop_stdout:
        return False, "drop proxy wayland-info did not list zwlr_layer_shell_v1"
    return True, ""


def judge_layer_shell_missing(code, stdout):
    """Judges case 3: surface/layer-shell-missing."""
    if code != 4:
        return False, f"exited {code}, expected 4"
    for line in stdout.splitlines():
        if "OUTCOME layer-shell-missing:" in line and "LayerShellNotSupportedError" in line:
            return True, ""
    return False, f"OUTCOME line missing LayerShellNotSupportedError: {stdout.strip()[:100]}"


def judge_deadline(code, stdout):
    """Judges case 4: surface/first-configure-deadline."""
    if code != 5:
        return False, f"exited {code}, expected 5"
    for line in stdout.splitlines():
        if "OUTCOME deadline:" in line and "zwlr_layer_surface_v1.configure" in line:
            return True, ""
    return False, f"OUTCOME line missing configure deadline: {stdout.strip()[:100]}"


def judge_refused_line(code, stdout):
    """Judges case 5: surface/refused-line, refused for its jsonrpc version only."""
    if code != 6:
        return False, f"exited {code}, expected 6"
    for line in stdout.splitlines():
        if "OUTCOME refused:" in line and "jsonrpc" in line:
            return True, ""
    return False, f"missing OUTCOME refused line naming jsonrpc: {stdout.strip()[:100]}"


def judge_refused_for(code, stdout, reason, label):
    """Judges a refusal: exit 6 and an `OUTCOME refused:` line carrying `reason`."""
    if code != 6:
        return False, f"exited {code}, expected 6"
    for line in stdout.splitlines():
        if "OUTCOME refused:" in line and reason in line:
            return True, ""
    return False, f"missing OUTCOME refused line naming the {label}: {stdout.strip()[:100]}"


def judge_refused_schema(code, stdout):
    """Judges case 6: surface/unknown-schema, refused for its schema only."""
    return judge_refused_for(code, stdout, REFUSED_SCHEMA, "schema")


def judge_refused_layer(code, stdout):
    """Judges case 7: surface/unknown-layer, refused for its layer only."""
    return judge_refused_for(code, stdout, REFUSED_LAYER, "layer")


def layer_requests(log_text):
    """Maps each layer-shell request name in a server debug log to its argument strings."""
    requests = {}
    for line in log_text.splitlines():
        match = REQUEST_RE.match(line.strip())
        if match is None:
            continue
        name, args = match.groups()
        if name == "get_layer_surface":
            # new id, wl_surface, output, layer, namespace: keep layer and namespace.
            fields = args.split(", ")
            args = ", ".join(fields[3:]).replace('"', "")
        requests.setdefault(name, []).append(args)
    return requests


def expected_layer_requests(params):
    """Returns the request arguments KWin must log for mount `params`."""
    anchor = sum(ANCHOR_WIRE[edge] for edge in params["anchor"])
    return {
        "get_layer_surface": [f"{LAYER_WIRE[params['layer']]}, {params['namespace']}"],
        "set_anchor": [str(anchor)],
        "set_exclusive_zone": [str(params["exclusive_zone"])],
        "set_exclusive_edge": [str(ANCHOR_WIRE[params["exclusive_edge"]])],
        "set_keyboard_interactivity": [str(KEYBOARD_WIRE[params["keyboard_interactivity"]])],
    }


def judge_parameters_read_back(code, stdout, log_text, params=REFERENCE_PARAMS):
    """Judges case 8: every mount parameter reached KWin once, with its reference value."""
    passed, why = judge_first_configure(code, stdout)
    if not passed:
        return False, why
    seen = layer_requests(log_text)
    for name, want in expected_layer_requests(params).items():
        got = seen.get(name, [])
        if got != want:
            return False, f"KWin logged {name} {got}, expected {want}"
    return True, ""


def case_verdicts(stdout):
    """Maps each `CASE <name> pass|fail...` line of the readback to (passed, line)."""
    verdicts = {}
    for line in stdout.splitlines():
        parts = line.split(" ", 2)
        if len(parts) >= 3 and parts[0] == "CASE":
            verdicts[parts[1]] = (parts[2] == "pass", line.strip())
    return verdicts


def node_count(stdout):
    """Returns the integer from the readback's `NODES <n>` line, or None."""
    for line in stdout.splitlines():
        parts = line.split()
        if len(parts) == 2 and parts[0] == "NODES" and parts[1].isdigit():
            return int(parts[1])
    return None


def judge_readback_lines(stdout, expected_fail, require_update):
    """Every readback case ran; exactly `expected_fail` failed; the update case as required."""
    verdicts = case_verdicts(stdout)
    names = READBACK_CASES + ((UPDATE_CASE,) if require_update else ())
    for name in names:
        if name not in verdicts:
            return False, f"readback printed no {name} line"
        passed, line = verdicts[name]
        if name in expected_fail and passed:
            return False, f"{name} passed, want fail"
        if name not in expected_fail and not passed:
            return False, f"readback case failed: {line}"
    return True, ""


def judge_atspi_full_tree(code, stdout):
    """Shared checks of the sound tree: exit 0 and at least the seeded nodes read."""
    if code != 0:
        return False, f"readback exited {code}, expected 0: {stdout.strip()[:100]}"
    nodes = node_count(stdout)
    if nodes is None or nodes < SEEDED_NODES:
        return False, f"readback read {nodes} nodes, want at least {SEEDED_NODES}"
    return True, ""


def judge_atspi_sound(code, stdout, event_click_seen):
    """Judges case 6: atspi/sound."""
    ok, why = judge_atspi_full_tree(code, stdout)
    if not ok:
        return ok, why
    ok, why = judge_readback_lines(stdout, set(), require_update=True)
    if not ok:
        return ok, why
    if not event_click_seen:
        return False, "fixture did not emit EVENT click"
    return True, ""


def judge_atspi_late(code, stdout):
    """Judges case 7: atspi/late-client."""
    ok, why = judge_atspi_full_tree(code, stdout)
    if not ok:
        return ok, why
    return judge_readback_lines(stdout, set(), require_update=False)


def judge_atspi_negative(variant, code, stdout):
    """Judges AT-SPI negative cases 8..11: the planted defect, and only it, is caught."""
    if variant not in NEGATIVE_EXPECT:
        return False, f"unknown negative variant {variant}"
    if code == 0:
        return False, f"readback unexpectedly passed for negative variant {variant}"
    if code != 1:
        return False, f"readback exited {code}, want 1 (a CASE failure): {stdout.strip()[:100]}"
    expected_fail, case_name, token = NEGATIVE_EXPECT[variant]
    ok, why = judge_readback_lines(stdout, expected_fail, require_update=True)
    if not ok:
        return ok, why
    line = case_verdicts(stdout)[case_name][1]
    if token not in line:
        return False, f"failure message did not contain '{token}': {line}"
    return True, ""


def read_until(stream, token, timeout, select_fn=select.select, read_fn=os.read):
    """Reads a binary pipe until a line holds `token`, EOF or the deadline; (found, text)."""
    deadline = time.monotonic() + timeout
    buf = b""
    for _ in range(100000):
        left = deadline - time.monotonic()
        if left <= 0:
            break
        ready, _, _ = select_fn([stream], [], [], left)
        if not ready:
            break
        chunk = read_fn(stream.fileno(), 4096)
        if not chunk:
            break
        buf += chunk
        if token.encode() in buf:
            return True, buf.decode(errors="replace")
    return False, buf.decode(errors="replace")


def run_child(argv, env, timeout=RUN_TIMEOUT):
    """Runs a child process with a timeout, returning (code, stdout)."""
    try:
        proc = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            timeout=timeout,
            env=env,
            stdin=subprocess.DEVNULL,
            check=False,
        )
        return proc.returncode, proc.stdout
    except subprocess.TimeoutExpired:
        return 124, "child timed out"
    except OSError as err:
        return 127, f"child error: {err}"


def run_surface_cases(binary, kwin_sock, hide_sock, drop_sock, runner=run_child):
    """Runs the 7 surface cases that need no debug compositor and reports results."""
    results = []
    base_env = dict(os.environ)
    base_env.pop("WAYLAND_DISPLAY", None)
    base_env.pop("DISPLAY", None)

    # 1. surface/first-configure
    env1 = dict(base_env, WAYLAND_DISPLAY=str(kwin_sock))
    c1, out1 = runner([str(binary), "surface", REFERENCE_LINE], env1)
    results.append(("surface/first-configure", *judge_first_configure(c1, out1)))

    # 2. surface/filter-control
    env_h = dict(base_env, WAYLAND_DISPLAY=str(hide_sock))
    ch, outh = runner(["wayland-info"], env_h)
    env_d = dict(base_env, WAYLAND_DISPLAY=str(drop_sock))
    cd, outd = runner(["wayland-info"], env_d)
    results.append(("surface/filter-control", *judge_filter_control(ch, outh, cd, outd)))

    # 3. surface/layer-shell-missing
    c3, out3 = runner([str(binary), "surface", REFERENCE_LINE], env_h)
    results.append(("surface/layer-shell-missing", *judge_layer_shell_missing(c3, out3)))

    # 4. surface/first-configure-deadline
    c4, out4 = runner([str(binary), "surface", REFERENCE_LINE], env_d)
    results.append(("surface/first-configure-deadline", *judge_deadline(c4, out4)))

    # 5. surface/refused-line
    bad_req = dict(REFERENCE_REQUEST, jsonrpc="1.0")
    bad_line = json.dumps(bad_req) + "\n"
    c5, out5 = runner([str(binary), "surface", bad_line], base_env)
    results.append(("surface/refused-line", *judge_refused_line(c5, out5)))

    # 6. surface/unknown-schema and 7. surface/unknown-layer: one member changed each.
    for name, key, value, judge in (
        ("surface/unknown-schema", "schema", UNKNOWN_SCHEMA, judge_refused_schema),
        ("surface/unknown-layer", "layer", "middle", judge_refused_layer),
    ):
        changed = dict(REFERENCE_REQUEST, params=dict(REFERENCE_PARAMS, **{key: value}))
        code, out = runner([str(binary), "surface", json.dumps(changed) + "\n"], base_env)
        results.append((name, *judge(code, out)))

    return results


def run_read_back_case(
    binary,
    runtime_dir,
    runner=run_child,
    start_fn=start_kwin,
    teardown_fn=None,
):
    """Runs case 8 against its own KWin, which logs every request it receives."""
    teardown_fn = teardown_fn or teardown_process
    config_file = write_no_activation_config(runtime_dir)
    proc, sock = start_fn(
        runtime_dir,
        socket_name=DEBUG_SOCKET,
        config_file=config_file,
        dbus_addr=None,
        log_name=DEBUG_LOG,
        extra_env={"WAYLAND_DEBUG": "server"},
    )
    try:
        env = dict(os.environ, WAYLAND_DISPLAY=str(sock))
        env.pop("DISPLAY", None)
        env.pop("DBUS_SESSION_BUS_ADDRESS", None)
        code, out = runner([str(binary), "surface", REFERENCE_LINE], env)
    finally:
        teardown_fn(proc)
    log_text = (runtime_dir / DEBUG_LOG).read_text(encoding="utf-8", errors="replace")
    return ("surface/parameters-read-back", *judge_parameters_read_back(code, out, log_text))


def run_atspi_case(
    shell_bin,
    readback_bin,
    variant,
    env,
    read_only=False,
    delay=0.0,
    popen_fn=subprocess.Popen,
    runner=run_child,
):
    """Runs fixture with given variant, then runs readback and judges outcome."""
    cmd = [str(shell_bin), "a11y", REFERENCE_LINE, variant]
    fixture_proc = popen_fn(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        bufsize=0,
        env=env,
        start_new_session=True,
    )
    ready, _ = read_until(fixture_proc.stdout, "READY", READY_TIMEOUT)
    if not ready:
        teardown_process(fixture_proc)
        return False, "fixture failed to announce READY"

    if delay > 0:
        time.sleep(delay)

    rb_cmd = [str(readback_bin)]
    if read_only:
        rb_cmd.append("--read-only")
    code, stdout = runner(rb_cmd, env=env)

    event_click = False
    if not read_only and code == 0:
        event_click, _ = read_until(fixture_proc.stdout, "EVENT click", CLICK_EVENT_TIMEOUT)

    teardown_process(fixture_proc)

    if variant == "sound":
        if delay > 0:
            return judge_atspi_late(code, stdout)
        return judge_atspi_sound(code, stdout, event_click)
    return judge_atspi_negative(variant, code, stdout)


def run_atspi_cases(
    shell_bin,
    readback_bin,
    env,
    runner=run_child,
    case_runner=run_atspi_case,
):
    """Runs the 6 AT-SPI test cases and returns list of (name, passed, why)."""
    results = []
    for name, variant, read_only, delay in ATSPI_CASES:
        passed, why = case_runner(
            shell_bin,
            readback_bin,
            variant,
            env,
            read_only=read_only,
            delay=delay,
            runner=runner,
        )
        results.append((name, passed, why))
    return results


def judge_ring_positive(variant, code, stdout):
    """Judges a positive ring case: readback must exit 0 and pass."""
    if code != 0:
        return False, f"readback exited {code}, want 0: {stdout.strip()[:100]}"
    case_line = f"CASE ring/{variant} pass"
    for line in stdout.splitlines():
        if line.strip() == case_line:
            return True, measure_summary(stdout)
    return False, f"missing pass line '{case_line}': {stdout.strip()[:100]}"


def measured_sides(stdout):
    """Returns `(width, side, contrast)` for every well-formed `MEASURE` line."""
    sides = []
    for line in stdout.splitlines():
        parts = line.split()
        if len(parts) != 4 or parts[0] != "MEASURE":
            continue
        try:
            sides.append((float(parts[2]), parts[1], float(parts[3])))
        except ValueError:
            continue
    return sides


def best_measured_width(stdout):
    """Returns the widest `MEASURE <side> <width> <contrast>` width, or None if none parse."""
    sides = measured_sides(stdout)
    return max(sides)[0] if sides else None


def measure_summary(stdout):
    """Returns `best <side> <width> px, <contrast>:1` for the widest side, or an empty string."""
    sides = measured_sides(stdout)
    if not sides:
        return ""
    width, side, contrast = max(sides)
    return f"best {side} {width:.3f} px, {contrast:.2f}:1"


def judge_ring_negative(variant, code, stdout):
    """Judges a negative ring case: exit 1, a fail line, and the planted width measured."""
    if code == 0:
        return False, f"readback unexpectedly passed for negative variant {variant}"
    if code != 1:
        return False, f"readback exited {code}, want 1 (a CASE failure): {stdout.strip()[:100]}"
    fail_prefix = f"CASE ring/{variant} fail:"
    reasons = [
        line.strip()[len(fail_prefix) :].strip()
        for line in stdout.splitlines()
        if line.strip().startswith(fail_prefix)
    ]
    if not any(reasons):
        return (
            False,
            f"missing fail line with a reason after '{fail_prefix}': {stdout.strip()[:100]}",
        )
    low, high = RING_NEGATIVE_WIDTH.get(variant, (0.0, 0.0))
    best = best_measured_width(stdout)
    if variant == "under-panel" and best is None:
        return True, ""
    if best is None or not low <= best <= high:
        return False, f"best measured width {best}, want {low}..{high} px for {variant}"
    return True, measure_summary(stdout)


def run_ring_case(
    shell_bin,
    ring_readback_bin,
    variant,
    is_positive,
    env,
    popen_fn=subprocess.Popen,
    runner=run_child,
):
    """Runs focus-ring fixture with given variant, reads geometry, runs readback, judges outcome."""
    cmd = [str(shell_bin), "ring", REFERENCE_LINE, variant]
    fixture_proc = popen_fn(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        bufsize=0,
        env=env,
        start_new_session=True,
    )
    ready, out = read_until(fixture_proc.stdout, "READY", READY_TIMEOUT)
    if not ready:
        teardown_process(fixture_proc)
        return False, "fixture failed to announce READY"

    ring_line = None
    for line in out.splitlines():
        if line.startswith("RING "):
            ring_line = line.strip()
            break
    if ring_line is None:
        teardown_process(fixture_proc)
        return False, "fixture did not emit RING line"

    rb_cmd = [str(ring_readback_bin), variant, ring_line]
    code, stdout = runner(rb_cmd, env=env)
    teardown_process(fixture_proc)

    if is_positive:
        return judge_ring_positive(variant, code, stdout)
    return judge_ring_negative(variant, code, stdout)


def run_ring_cases(
    shell_bin,
    ring_readback_bin,
    env,
    runner=run_child,
    case_runner=run_ring_case,
):
    """Runs the 8 focus-ring test cases and returns list of (name, passed, why)."""
    results = []
    for name, variant, is_positive in RING_CASES:
        passed, why = case_runner(
            shell_bin,
            ring_readback_bin,
            variant,
            is_positive,
            env,
            runner=runner,
        )
        results.append((name, passed, why))
    return results


def timing_argv(binary, seeded, perf_path_or_None, data_path=None):
    """Builds the argv list for running the timing fixture, optionally under perf."""
    inner = [str(binary), "timing", REFERENCE_LINE, str(seeded)]
    if perf_path_or_None is not None:
        perf_cmd = [str(perf_path_or_None), "record", "-F", "999"]
        if data_path is not None:
            perf_cmd.extend(["-o", str(data_path)])
        perf_cmd.append("--")
        return perf_cmd + inner
    return inner


def is_allocator_symbol(sym):
    """Checks whether a symbol matches the allocator allow-list."""
    for alloc_sym in ALLOCATOR_SYMBOLS:
        if alloc_sym == "alloc::alloc::":
            if "alloc::alloc::" in sym:
                return True
        elif sym == alloc_sym or sym.startswith(f"{alloc_sym}@"):
            return True
    return False


def parse_report_line(line):
    """Extracts (percentage, symbol) from a perf report stdio line."""
    line = line.strip()
    if not line or line.startswith("#"):
        return None, None
    parts = line.split()
    if not parts:
        return None, None
    pct = None
    if parts[0].endswith("%"):
        try:
            pct = float(parts[0].rstrip("%"))
        except ValueError:
            return None, None
    elif len(parts) > 1 and parts[1] == "%":
        try:
            pct = float(parts[0])
        except ValueError:
            return None, None
    if pct is None:
        return None, None
    if "[.]" in line:
        sym = line.split("[.]", 1)[1].strip()
    elif "[k]" in line:
        sym = line.split("[k]", 1)[1].strip()
    else:
        sym = parts[-1]
    return pct, sym


def allocator_share(report_text):
    """Parses perf report stdio output and sums percentage share of allocator symbols."""
    if not report_text or not report_text.strip():
        return None
    total = 0.0
    found_any = False
    for line in report_text.splitlines():
        pct, sym = parse_report_line(line)
        if pct is None:
            continue
        found_any = True
        if is_allocator_symbol(sym):
            total += pct
    if not found_any:
        return None
    return round(total, 2)


def parse_gpu_line(line):
    """Parses a 'GPU <device>\t<driver>\t<driver_info>\t<software>' line."""
    line = line.strip()
    if not line.startswith("GPU "):
        return None
    rest = line[4:].strip()
    if rest == "-":
        return {
            "gpu": "",
            "driver": "",
            "driver_info": "",
            "software": False,
        }
    parts = rest.split("\t")
    if len(parts) < 4:
        return None
    return {
        "gpu": parts[0],
        "driver": parts[1],
        "driver_info": parts[2],
        "software": parts[3] in ("1", "true", "True"),
    }


def parse_frame_line(line):
    """Parses a FRAME line: 'FRAME <draw_ns> <phases_ns> <nodes|->'."""
    line = line.strip()
    if not line.startswith("FRAME "):
        return None
    parts = line.split()
    if len(parts) != 4:
        return None
    try:
        draw_ns = int(parts[1])
        phases_ns = int(parts[2])
    except ValueError:
        return None
    nodes = None
    if parts[3] != "-":
        try:
            nodes = int(parts[3])
        except ValueError:
            return None
    residual_ns = max(0, draw_ns - phases_ns)
    return {
        "draw_ns": draw_ns,
        "phases_ns": phases_ns,
        "nodes": nodes,
        "residual_ns": residual_ns,
    }


def nearest_rank(sorted_vals, percentile):
    """Computes nearest-rank percentile value matching Rust timing.rs."""
    n = len(sorted_vals)
    if n == 0:
        return None
    rank = (percentile * n + 99) // 100
    idx = max(0, min(rank - 1, n - 1))
    return sorted_vals[idx]


def compute_distribution(values):
    """Computes min, p50, p95, p99, max, mean matching Rust timing.rs."""
    if not values:
        return None
    sorted_vals = sorted(values)
    return {
        "min": sorted_vals[0],
        "p50": nearest_rank(sorted_vals, 50),
        "p95": nearest_rank(sorted_vals, 95),
        "p99": nearest_rank(sorted_vals, 99),
        "max": sorted_vals[-1],
        "mean": sum(sorted_vals) // len(sorted_vals),
    }


def evaluate_timing(lines, info, frames=600, warmup=60, min_nodes=1000):
    """Evaluates timing samples against acceptance invariants.

    Returns (measurement_dict, None) on success or (None, refusal_str) on refusal.
    """
    if info.get("software", False):
        return None, f"REFUSED software renderer emulated: {info.get('gpu', '')}"
    for field in ("gpu", "driver", "compositor", "zed_commit"):
        if not info.get(field, "").strip():
            return None, f"REFUSED missing info field: {field}"

    samples = []
    for line in lines:
        sample = parse_frame_line(line)
        if sample is not None:
            samples.append(sample)

    kept = samples[warmup:]
    if len(kept) < frames:
        return None, f"REFUSED too few frames: got {len(kept)}, want {frames}"

    eval_samples = kept[:frames]
    frames_without_update = 0
    min_seen = float("inf")
    max_seen = 0

    for s in eval_samples:
        n = s["nodes"]
        if n is None:
            frames_without_update += 1
        else:
            if n < min_seen:
                min_seen = n
            if n > max_seen:
                max_seen = n

    if frames_without_update > 0:
        return None, (
            f"REFUSED accessibility adapter inactive: "
            f"{frames_without_update} frames without tree update"
        )

    if min_seen < min_nodes:
        reported = 0 if min_seen == float("inf") else min_seen
        return None, f"REFUSED too few nodes: min seen {reported}, want {min_nodes}"

    frame_dist = compute_distribution([s["draw_ns"] for s in eval_samples])
    tree_dist = compute_distribution([s["residual_ns"] for s in eval_samples])

    measurement = {
        "frames": frames,
        "nodes_min": min_seen,
        "nodes_max": max_seen,
        "frame": frame_dist,
        "tree_update": tree_dist,
    }
    return measurement, None


def judge_timing(lines, info, frames=600, warmup=60, min_nodes=1000):
    """Judges timing lines against acceptance criteria. Returns (ok, reason_or_summary)."""
    measurement, refusal = evaluate_timing(
        lines, info, frames=frames, warmup=warmup, min_nodes=min_nodes
    )
    if refusal is not None:
        return False, refusal
    f_p50 = measurement["frame"]["p50"] / 1_000_000.0
    f_p95 = measurement["frame"]["p95"] / 1_000_000.0
    f_p99 = measurement["frame"]["p99"] / 1_000_000.0
    t_p50 = measurement["tree_update"]["p50"] / 1_000_000.0
    t_p95 = measurement["tree_update"]["p95"] / 1_000_000.0
    t_p99 = measurement["tree_update"]["p99"] / 1_000_000.0
    d_info = f" {info['driver_info']}" if info.get("driver_info") else ""
    summary_text = (
        f"frame p50={f_p50:.2f}ms p95={f_p95:.2f}ms p99={f_p99:.2f}ms | "
        f"tree-update p50={t_p50:.2f}ms p95={t_p95:.2f}ms p99={t_p99:.2f}ms | "
        f"gpu {info.get('gpu', '')} | driver {info.get('driver', '')}{d_info} | "
        f"compositor {info.get('compositor', '')} | zed {info.get('zed_commit', '')}"
    )
    return True, summary_text


def format_timing_lines(measurement, info):
    """Formats TIMING summary lines matching Rust measurement_lines."""
    d_info = f" {info['driver_info']}" if info.get("driver_info") else ""
    frames = measurement["frames"]
    n_min, n_max = measurement["nodes_min"], measurement["nodes_max"]
    return [
        f"TIMING frames {frames} nodes {n_min}..{n_max}",
        (
            f"TIMING frame_ns min {measurement['frame']['min']} "
            f"p50 {measurement['frame']['p50']} "
            f"p95 {measurement['frame']['p95']} "
            f"p99 {measurement['frame']['p99']} "
            f"max {measurement['frame']['max']} "
            f"mean {measurement['frame']['mean']}"
        ),
        (
            f"TIMING tree_update_ns min {measurement['tree_update']['min']} "
            f"p50 {measurement['tree_update']['p50']} "
            f"p95 {measurement['tree_update']['p95']} "
            f"p99 {measurement['tree_update']['p99']} "
            f"max {measurement['tree_update']['max']} "
            f"mean {measurement['tree_update']['mean']}"
        ),
        (
            f"TIMING gpu {info.get('gpu', '-')} | "
            f"driver {info.get('driver', '-')}{d_info} | "
            f"compositor {info.get('compositor', '-')} | "
            f"zed {info.get('zed_commit', '-')}"
        ),
    ]


def wait_child(proc, timeout):
    """Waits up to `timeout` seconds for `proc` to exit; returns its code, or None if it did not."""
    try:
        return proc.wait(timeout=timeout)
    except subprocess.TimeoutExpired:
        return None


def run_timing_case(
    binary,
    env,
    seeded,
    timeout=120,
    popen_fn=subprocess.Popen,
    perf_bin=None,
    perf_data=None,
    select_fn=select.select,
    read_fn=os.read,
):
    """Runs timing fixture for `seeded` count, collects GPU and FRAME lines until DONE."""
    cmd = timing_argv(binary, seeded, perf_bin, perf_data)
    proc = popen_fn(
        cmd,
        stdout=subprocess.PIPE,
        stderr=subprocess.DEVNULL,
        bufsize=0,
        env=env,
        start_new_session=True,
    )
    lines = []
    deadline = time.monotonic() + timeout
    line_buf = ""
    try:
        if proc.stdout:
            fd = proc.stdout.fileno()
            for _ in range(200000):
                left = deadline - time.monotonic()
                if left <= 0:
                    break
                ready, _, _ = select_fn([proc.stdout], [], [], min(1.0, max(0.01, left)))
                if ready:
                    chunk = read_fn(fd, 4096)
                    if not chunk:
                        break
                    line_buf += chunk.decode(errors="replace")
                    while "\n" in line_buf:
                        line, line_buf = line_buf.split("\n", 1)
                        line = line.strip()
                        if line.startswith("GPU ") or line.startswith("FRAME "):
                            lines.append(line)
                        elif line == "DONE":
                            # The fixture quits on its own after DONE; waiting lets a wrapping
                            # `perf record` finish writing its data before the group is killed.
                            return wait_child(proc, DONE_EXIT_WAIT), lines
                elif proc.poll() is not None:
                    break
        try:
            code = proc.wait(timeout=2)
        except Exception:
            code = proc.poll()
        return code, lines
    finally:
        teardown_process(proc)


def judge_timing_case_result(name, is_positive, lines, gpu_info):
    """Judges a timing case result for positive or negative expectation."""
    ok, reason = judge_timing(lines, gpu_info)
    if is_positive:
        return ok, reason
    if ok:
        return False, f"unexpectedly accepted measurement for {name}"
    if name == "timing/999-nodes":
        if "too few nodes" in reason.lower():
            return True, reason
        return False, f"expected refusal 'too few nodes', got '{reason}'"
    if name == "timing/adapter-inactive":
        if "accessibility adapter inactive" in reason.lower():
            return True, reason
        return False, f"expected refusal 'accessibility adapter inactive', got '{reason}'"
    return False, reason


def write_timing_out(lines, gpu_info, timing_out):
    """Writes FRAME lines and TIMING summary lines to timing_out destination."""
    measurement, _ = evaluate_timing(lines, gpu_info)
    if measurement is not None:
        t_lines = format_timing_lines(measurement, gpu_info)
        f_lines = [frame_line for frame_line in lines if frame_line.startswith("FRAME ")]
        content = "\n".join(f_lines + t_lines) + "\n"
        out_path = Path(timing_out)
        out_path.parent.mkdir(parents=True, exist_ok=True)
        out_path.write_text(content, encoding="utf-8")


def handle_perf_report(perf_bin, perf_data, env, runner, perf_path):
    """Processes perf report and prints allocation share note or skip note."""
    if perf_bin is not None and perf_data is not None and perf_data.is_file():
        rep_cmd = [
            str(perf_bin),
            "report",
            "-i",
            str(perf_data),
            "--no-children",
            "--sort",
            "sym",
            "--stdio",
            "--percent-limit",
            "0",
        ]
        code_rep, rep_out = runner(rep_cmd, env=env, timeout=PERF_REPORT_TIMEOUT)
        share = allocator_share(rep_out) if code_rep == 0 else None
        if share is not None:
            sys.stdout.write(
                f"NOTE timing/1000-nodes: allocator share {share:.1f} % "
                "of sampled CPU in the fixture process (perf, all threads)\n"
            )
        else:
            sys.stdout.write(
                f"NOTE timing/1000-nodes: allocation share not measured: perf report exited "
                f"{code_rep} or named no sampled symbol\n"
            )
    elif perf_path is None:
        sys.stdout.write("SKIP: perf not installed; allocation share not measured\n")
    else:
        sys.stdout.write("NOTE timing/1000-nodes: allocation share not measured: no perf data\n")
    sys.stdout.flush()


def inactive_adapter_env(a11y_env, runtime_dir):
    """Returns the a11y environment with the session bus pointed at a socket that does not exist.

    An unset DBUS_SESSION_BUS_ADDRESS lets a D-Bus client fall back to a default address; a dead
    path inside the gate's own runtime directory leaves AccessKit no bus to find, so the adapter
    stays inactive and nothing reaches the operator's session.
    """
    env = dict(a11y_env)
    env.pop("AT_SPI_BUS_ADDRESS", None)
    base = Path(runtime_dir) if runtime_dir is not None else Path(env.get("XDG_RUNTIME_DIR", "."))
    env["DBUS_SESSION_BUS_ADDRESS"] = f"unix:path={host.target(base / 'no-session-bus')}"
    return env


def run_timing_cases(
    shell_bin,
    a11y_env,
    kwin_ver,
    zed_commit,
    runner=run_child,
    case_runner=run_timing_case,
    perf_path=None,
    runtime_dir=None,
    timing_out=None,
):
    """Runs the 3 timing test cases and returns list of (name, passed, why)."""
    results = []
    for name, seeded, is_positive in TIMING_CASES:
        if name == "timing/adapter-inactive":
            env = inactive_adapter_env(a11y_env, runtime_dir)
        else:
            env = a11y_env

        perf_bin = perf_path if (is_positive and perf_path and runtime_dir) else None
        perf_data = (runtime_dir / "timing.perf.data") if perf_bin else None

        _code, lines = case_runner(
            shell_bin,
            env,
            seeded,
            perf_bin=perf_bin,
            perf_data=perf_data,
        )

        gpu_info = None
        for line in lines:
            if line.startswith("GPU "):
                gpu_info = parse_gpu_line(line)
                break
        if gpu_info is None:
            gpu_info = {"gpu": "", "driver": "", "driver_info": "", "software": False}
        gpu_info["compositor"] = f"KWin {kwin_ver.removeprefix('kwin ')}"
        gpu_info["zed_commit"] = zed_commit

        passed, why = judge_timing_case_result(name, is_positive, lines, gpu_info)
        results.append((name, passed, why))

        if is_positive and passed:
            if timing_out is not None:
                write_timing_out(lines, gpu_info, timing_out)
            handle_perf_report(perf_bin, perf_data, env, runner, perf_path)
    return results


def run_all_cases(
    binary,
    kwin_sock,
    hide_sock,
    drop_sock,
    runner=run_child,
    readback_bin=None,
    a11y_env=None,
    atspi_case_runner=run_atspi_case,
    ring_readback_bin=None,
    ring_env=None,
    ring_case_runner=run_ring_case,
    timing_bin=None,
    timing_case_runner=run_timing_case,
    kwin_ver=None,
    zed_commit=None,
    perf_path=None,
    runtime_dir=None,
    timing_out=None,
    read_back_runner=run_read_back_case,
):
    """Runs all test cases (8 surface + 6 AT-SPI + 8 focus-ring + 3 timing)."""
    results = run_surface_cases(binary, kwin_sock, hide_sock, drop_sock, runner=runner)
    results.append(read_back_runner(binary, runtime_dir, runner))
    if readback_bin is not None and a11y_env is not None:
        atspi_results = run_atspi_cases(
            binary,
            readback_bin,
            a11y_env,
            runner=runner,
            case_runner=atspi_case_runner,
        )
        results.extend(atspi_results)
    if ring_readback_bin is not None and ring_env is not None:
        ring_results = run_ring_cases(
            binary,
            ring_readback_bin,
            ring_env,
            runner=runner,
            case_runner=ring_case_runner,
        )
        results.extend(ring_results)
    t_bin = timing_bin if timing_bin is not None else binary
    if a11y_env is not None:
        timing_results = run_timing_cases(
            t_bin,
            a11y_env,
            kwin_ver or "",
            zed_commit or "",
            runner=runner,
            case_runner=timing_case_runner,
            perf_path=perf_path,
            runtime_dir=runtime_dir,
            timing_out=timing_out,
        )
        results.extend(timing_results)
    return results


def summary(case_results):
    """Returns the RESULT line; every one of the CASE_COUNT cases must have run and passed."""
    return gate_common.summary(case_results, CASE_COUNT)


def teardown_process(proc):
    """Terminates process group and reaps child process."""
    if proc is None:
        return
    ended, _ = host.end_session(proc.pid)
    if not ended:
        try:
            proc.kill()
        except OSError:
            pass
    try:
        proc.wait(timeout=10)
    except Exception:
        pass


def report_run_summary(case_results, version, versions):
    """Prints per-case results, versions note, D79 note, and returns exit code."""
    for name, passed, why in case_results:
        sys.stdout.write(f"CASE {name} pass\n" if passed else f"CASE {name} fail: {why}\n")
        if passed and why:
            sys.stdout.write(f"NOTE {name}: {why}\n")
    sys.stdout.write(
        f"NOTE versions: at-spi2-core {versions['atspi-2']}, dbus {versions['dbus']}\n"
    )
    sys.stdout.write(
        f"NOTE D79: headless KWin {version} (kwin_wayland --virtual); "
        "development evidence for the client half only, not evidence that "
        "P04 serves zwlr_layer_shell_v1\n"
    )
    result = summary(case_results)
    sys.stdout.write(result + "\n")
    return 0 if result == "RESULT pass" else 1


def parse_timing_out(argv):
    """Returns (path or None, ok); ok is False when --timing-out lacks its path."""
    if "--timing-out" not in argv:
        return None, True
    idx = argv.index("--timing-out")
    if idx + 1 < len(argv):
        return Path(argv[idx + 1]), True
    return None, False


def start_services(runtime_dir, procs):
    """Starts the session bus, KWin, both filters and the a11y bus; appends each to procs."""
    config_file = write_no_activation_config(runtime_dir)
    daemon_proc, session_addr = start_dbus_daemon(runtime_dir, config_file=config_file)
    procs.append(daemon_proc)

    kwin_proc, kwin_sock = start_kwin(runtime_dir, config_file=config_file, dbus_addr=session_addr)
    procs.append(kwin_proc)

    hide_sock = runtime_dir / "wayland-filter-hide"
    procs.append(start_filter(hide_sock, kwin_sock, "hide-layer-shell"))

    drop_sock = runtime_dir / "wayland-filter-drop"
    procs.append(start_filter(drop_sock, kwin_sock, "drop-layer-configure"))

    a11y_procs, a11y_env = start_a11y_bus(
        runtime_dir, config_file=config_file, session_bus=session_addr
    )
    procs.extend(a11y_procs)
    a11y_env["WAYLAND_DISPLAY"] = str(kwin_sock)
    return kwin_sock, hide_sock, drop_sock, a11y_env


def main(argv=None):
    """Main verification harness driver."""
    if argv is None:
        argv = sys.argv[1:]
    timing_out, ok = parse_timing_out(argv)
    if not ok:
        sys.stdout.write("RESULT error: --timing-out requires a path argument\n")
        return 1

    reasons = capability_reasons()
    if reasons:
        sys.stdout.write(f"SKIP: {'; '.join(reasons)}; the native shell gate did not run.\n")
        sys.stdout.flush()
        return 0

    runtime_dir = Path(tempfile.mkdtemp(prefix="aegis-shell-"))
    os.chmod(runtime_dir, 0o700)
    procs = []

    try:
        shell_bin, readback_bin, ring_readback_bin = build_shell_binaries()
        nontiming_bin = runtime_dir / f"{BINARY}-nontiming"
        shutil.copy2(shell_bin, nontiming_bin)
        timing_bin = build_timing_shell_binary()
        version = kwin_version()
        versions = read_versions()
        zed_commit = read_zed_commit()
        perf_path = shutil.which("perf")

        kwin_sock, hide_sock, drop_sock, a11y_env = start_services(runtime_dir, procs)

        case_results = run_all_cases(
            nontiming_bin,
            kwin_sock,
            hide_sock,
            drop_sock,
            readback_bin=readback_bin,
            a11y_env=a11y_env,
            ring_readback_bin=ring_readback_bin,
            ring_env=a11y_env,
            timing_bin=timing_bin,
            kwin_ver=version,
            zed_commit=zed_commit,
            perf_path=perf_path,
            runtime_dir=runtime_dir,
            timing_out=timing_out,
        )
        return report_run_summary(case_results, version, versions)
    except GateError as err:
        sys.stdout.write(f"RESULT error: {err}\n")
        return 1
    finally:
        for proc in procs:
            teardown_process(proc)
        shutil.rmtree(runtime_dir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
