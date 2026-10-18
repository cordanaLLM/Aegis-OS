#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Verify P05 Forum shell zwlr_layer_shell_v1 surface mapping against headless KWin.

Milestone M28 E28-2, decisions D79, D82 and ADR-0003. Starts a private headless
KWin compositor (`kwin_wayland --virtual`) inside `dbus-run-session` in a private
temporary `XDG_RUNTIME_DIR` (never the operator's display session).

Drives the Rust binary `aegis-forum-shell-gpui` through five cases:
1. surface/first-configure: direct connection maps layer shell and receives first configure;
2. surface/filter-control: verify wayland-info sees layer shell filtered out or preserved;
3. surface/layer-shell-missing: compositor lacking layer shell reports LayerShellNotSupportedError;
4. surface/first-configure-deadline: configure that never arrives terminates at deadline;
5. surface/refused-line: invalid request line is refused before touching display.

Every pass is development evidence for the client half only (D79).
"""

import json
import os
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
PROGRAMS = (
    "cargo",
    "dbus-run-session",
    "kwin_wayland",
    "wayland-info",
    Path(sys.executable).name,
    BINARY,
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
CASE_COUNT = 5


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


def capability_reasons(platform=None, which=shutil.which):
    """Returns missing host capabilities, or an empty list if capable."""
    platform = sys.platform if platform is None else platform
    if not platform.startswith("linux"):
        return [f"the native shell gate needs Linux and a Wayland runtime; this host is {platform}"]
    if not (hasattr(socket, "AF_UNIX") and hasattr(socket, "SCM_RIGHTS")):
        return ["AF_UNIX and SCM_RIGHTS are POSIX-only and missing on this host"]
    reasons = []
    for prog in ("kwin_wayland", "dbus-run-session", "wayland-info", "cargo"):
        if which(prog) is None:
            reasons.append(f"{prog} is not installed")
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


def build_shell_binary(runner=run_command):
    """Builds the aegis-forum-shell-gpui binary with cargo."""
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
        "--message-format=json-render-diagnostics",
    ]
    code, stdout, stderr = runner(argv, BUILD_TIMEOUT, environ=env, cwd=ROOT)
    if code != 0:
        tail = "\n".join(stderr.strip().splitlines()[-24:])
        raise GateError(f"cargo build failed ({code}):\n{tail}")
    executable = executable_from(stdout, BINARY)
    if executable is None or not executable.is_file():
        raise GateError(f"cargo reported no executable for {BINARY}")
    return executable


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


def run_all_cases(binary, kwin_sock, hide_sock, drop_sock, runner=run_child):
    """Runs the 5 test cases and reports results."""
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
        binary = build_shell_binary()
        version = kwin_version()
        kwin_proc, kwin_sock = start_kwin(runtime_dir)
        procs.append(kwin_proc)

        hide_sock = runtime_dir / "wayland-filter-hide"
        hide_proc = start_filter(hide_sock, kwin_sock, "hide-layer-shell")
        procs.append(hide_proc)

        drop_sock = runtime_dir / "wayland-filter-drop"
        drop_proc = start_filter(drop_sock, kwin_sock, "drop-layer-configure")
        procs.append(drop_proc)

        case_results = run_all_cases(binary, kwin_sock, hide_sock, drop_sock)
        for name, passed, why in case_results:
            sys.stdout.write(f"CASE {name} pass\n" if passed else f"CASE {name} fail: {why}\n")

        sys.stdout.write(
            f"NOTE D79: headless KWin {version} (kwin_wayland --virtual); "
            "development evidence for the client half only, not evidence that "
            "P04 serves zwlr_layer_shell_v1\n"
        )
        result = summary(case_results)
        sys.stdout.write(result + "\n")
        return 0 if result == "RESULT pass" else 1
    except GateError as err:
        sys.stdout.write(f"RESULT error: {err}\n")
        return 1
    finally:
        for proc in procs:
            teardown_process(proc)
        shutil.rmtree(runtime_dir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
