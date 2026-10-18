#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Verify P05 Forum shell zwlr_layer_shell_v1 surface mapping and AT-SPI tree.

Milestone M28 E28-2 and E28-3, decisions D79, D82 and ADR-0003. Starts a private
headless KWin compositor (`kwin_wayland --virtual`) inside `dbus-run-session`
and a private D-Bus session with at-spi2 infrastructure in a temporary
`XDG_RUNTIME_DIR` (never the operator's display or session bus).

Drives the Rust binaries `aegis-forum-shell-gpui` and `aegis-atspi-readback`
through eleven cases:
1. surface/first-configure: direct connection maps layer shell and receives first configure;
2. surface/filter-control: verify wayland-info sees layer shell filtered out or preserved;
3. surface/layer-shell-missing: compositor lacking layer shell reports LayerShellNotSupportedError;
4. surface/first-configure-deadline: configure that never arrives terminates at deadline;
5. surface/refused-line: invalid request line is refused before touching display;
6. atspi/sound: fixture exposes button, focus, 1000 nodes, hidden subtree; click updates label;
7. atspi/late-client: client started after surface maps receives full tree;
8. atspi/negative-button-as-container: button exposed as container fails;
9. atspi/negative-unlabelled: planted unlabelled node fails and is named;
10. atspi/negative-culled: culling that removes nodes fails 1000-node check;
11. atspi/negative-hidden-shown: unhidden hidden subtree fails.

Every pass is development evidence for the client half only (D79).
"""

import json
import os
import select
import shutil
import socket
import subprocess
import sys
import tempfile
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
TOOLS_DIR = Path(__file__).resolve().parent
if str(TOOLS_DIR) not in sys.path:
    sys.path.insert(0, str(TOOLS_DIR))

import host  # noqa: E402

BINARY = "aegis-forum-shell-gpui"
READBACK_BINARY = "aegis-atspi-readback"
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
CASE_COUNT = 11

ATSPI_CASES = (
    ("atspi/sound", "sound", False, 0.0),
    ("atspi/late-client", "sound", True, 0.5),
    ("atspi/negative-button-as-container", "button-as-container", False, 0.0),
    ("atspi/negative-unlabelled", "unlabelled", False, 0.0),
    ("atspi/negative-culled", "culled", False, 0.0),
    ("atspi/negative-hidden-shown", "hidden-shown", False, 0.0),
)

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
    env = dict(os.environ if environ is None else environ)
    env["LC_ALL"] = "C"
    env["LANG"] = "C"
    try:
        proc = subprocess.run(
            argv,
            capture_output=True,
            text=True,
            errors="replace",
            timeout=timeout,
            env=env,
            cwd=cwd,
            stdin=subprocess.DEVNULL,
            check=False,
        )
        return proc.returncode, proc.stdout, proc.stderr
    except subprocess.TimeoutExpired as error:
        raise GateError(f"{Path(argv[0]).name} exceeded {timeout}s timeout") from error
    except OSError as error:
        raise GateError(f"{argv[0]} could not be run: {error}") from error


def has_unix_fd_passing():
    """Reports whether this Python exposes AF_UNIX with SCM_RIGHTS."""
    return hasattr(socket, "AF_UNIX") and hasattr(socket, "SCM_RIGHTS")


def find_atspi_program(name, dirs=ATSPI_DIRS, exists=os.path.exists):
    """Finds an at-spi program in system directories or PATH."""
    w = shutil.which(name)
    if w is not None:
        return Path(w)
    for d in dirs:
        candidate = Path(d) / name
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
    """Builds both shell and readback binaries with cargo."""
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
    return shell_bin, readback_bin


def build_shell_binary(runner=run_command):
    """Builds the aegis-forum-shell-gpui binary with cargo (backward compatibility)."""
    shell_bin, _ = build_shell_binaries(runner=runner)
    return shell_bin


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


def start_kwin(runtime_dir, socket_name="kwin-wayland-0", popen_fn=subprocess.Popen):
    """Launches private headless KWin instance inside dbus-run-session."""
    socket_path = runtime_dir / socket_name
    log_file_path = runtime_dir / "kwin.log"
    log_file = open(log_file_path, "w", encoding="utf-8")
    env = dict(os.environ)
    env.pop("WAYLAND_DISPLAY", None)
    env.pop("DISPLAY", None)
    env["XDG_RUNTIME_DIR"] = str(runtime_dir)

    cmd = [
        "dbus-run-session",
        "--",
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


def start_dbus_daemon(runtime_dir, popen_fn=subprocess.Popen):
    """Starts private dbus-daemon and returns (proc, session_bus_address)."""
    socket_path = runtime_dir / "a11y-session-bus"
    cmd = [
        "dbus-daemon",
        "--session",
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
    popen_fn=subprocess.Popen,
    runner=run_command,
    atspi_finder=find_atspi_program,
):
    """Launches private D-Bus session and at-spi2 infrastructure in runtime_dir."""
    procs = []
    daemon_proc, session_addr = start_dbus_daemon(runtime_dir, popen_fn=popen_fn)
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
    """Runs the 5 surface test cases and reports results."""
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

    return results


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


def run_all_cases(
    binary,
    kwin_sock,
    hide_sock,
    drop_sock,
    runner=run_child,
    readback_bin=None,
    a11y_env=None,
    atspi_case_runner=run_atspi_case,
):
    """Runs all test cases (5 surface + 6 AT-SPI when readback_bin is provided)."""
    results = run_surface_cases(binary, kwin_sock, hide_sock, drop_sock, runner=runner)
    if readback_bin is not None and a11y_env is not None:
        atspi_results = run_atspi_cases(
            binary,
            readback_bin,
            a11y_env,
            runner=runner,
            case_runner=atspi_case_runner,
        )
        results.extend(atspi_results)
    return results


def summary(case_results):
    """Returns the RESULT line; every one of the CASE_COUNT cases must have run and passed."""
    if len(case_results) != CASE_COUNT:
        return f"RESULT fail: {len(case_results)} of {CASE_COUNT} cases ran"
    failures = sum(1 for _name, passed, _why in case_results if not passed)
    if failures:
        return f"RESULT fail: {failures} case(s) failed"
    return "RESULT pass"


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


def main():
    """Main verification harness driver."""
    reasons = capability_reasons()
    if reasons:
        sys.stdout.write(f"SKIP: {'; '.join(reasons)}; the native shell gate did not run.\n")
        sys.stdout.flush()
        return 0

    runtime_dir = Path(tempfile.mkdtemp(prefix="aegis-shell-"))
    os.chmod(runtime_dir, 0o700)
    procs = []

    try:
        shell_bin, readback_bin = build_shell_binaries()
        version = kwin_version()
        versions = read_versions()

        kwin_proc, kwin_sock = start_kwin(runtime_dir)
        procs.append(kwin_proc)

        hide_sock = runtime_dir / "wayland-filter-hide"
        hide_proc = start_filter(hide_sock, kwin_sock, "hide-layer-shell")
        procs.append(hide_proc)

        drop_sock = runtime_dir / "wayland-filter-drop"
        drop_proc = start_filter(drop_sock, kwin_sock, "drop-layer-configure")
        procs.append(drop_proc)

        a11y_procs, a11y_env = start_a11y_bus(runtime_dir)
        procs.extend(a11y_procs)
        a11y_env["WAYLAND_DISPLAY"] = str(kwin_sock)

        case_results = run_all_cases(
            shell_bin,
            kwin_sock,
            hide_sock,
            drop_sock,
            readback_bin=readback_bin,
            a11y_env=a11y_env,
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
