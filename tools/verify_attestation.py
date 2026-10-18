#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2

"""Verify TPM2 PCR quote creation, qualifying data and selection over swtpm.

Milestone M20 E20-1 development slice on swtpm (issue #65).
Starts a private software TPM (swtpm socket) in an ephemeral directory,
creates EK and AK, extends PCRs 0, 4, 7, 11, quotes against the canonical
audit-record digest from aegis-justitia, and verifies attestation:
1. attest/quote-verifies: quote over 0,4,7,11 with record digest verifies
2. attest/other-record-fails: checkquote with another record's digest fails
3. attest/pcr-state-changed-fails: quote verified against stale PCR file fails
4. attest/exact-selection-accepted: selection judge on 0,4,7,11 quote accepts
5. attest/missing-pcr11-rejected: quote missing PCR 11 rejected by selection judge

Every pass is development evidence on swtpm; not rooted in a firmware-verified
boot chain (M11). Closes no release gate.
"""

import os
import re
import shutil
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

CASE_COUNT = 5
CHILD_TIMEOUT = 20
REQUIRED_PCRS = (0, 4, 7, 11)

REQUIRED_PROGRAMS = (
    "swtpm",
    "tpm2_quote",
    "tpm2_checkquote",
    "tpm2_print",
    "tpm2_createek",
    "tpm2_createak",
    "tpm2_pcrextend",
    "tpm2_flushcontext",
    "cargo",
)

TCTI_SWTPM_PATHS = (
    "/usr/lib/libtss2-tcti-swtpm.so",
    "/usr/lib64/libtss2-tcti-swtpm.so",
    "/usr/lib/x86_64-linux-gnu/libtss2-tcti-swtpm.so",
)


class GateError(Exception):
    """The gate could not be run, as opposed to a decided test case failure."""


def run_command(argv, timeout=CHILD_TIMEOUT, environ=None, cwd=None):
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


def has_tcti_swtpm(paths=TCTI_SWTPM_PATHS, exists=os.path.exists):
    """Checks if libtss2-tcti-swtpm.so is present on the host."""
    for p in paths:
        if exists(p):
            return True
    return False


def capability_reasons(
    platform=None,
    which=shutil.which,
    tcti_check=has_tcti_swtpm,
):
    """Returns missing host capabilities, or empty list if capable."""
    platform = sys.platform if platform is None else platform
    if not platform.startswith("linux"):
        return [f"the attestation gate needs Linux; this host is {platform}"]
    reasons = []
    for prog in REQUIRED_PROGRAMS:
        if which(prog) is None:
            reasons.append(f"{prog} is not installed")
    if not tcti_check():
        reasons.append("libtss2-tcti-swtpm.so is missing")
    return reasons


def probe_swtpm_version(runner=run_command):
    """Queries swtpm --version and returns version string."""
    code, stdout, _ = runner(["swtpm", "--version"], timeout=5)
    if code == 0:
        match = re.search(r"TPM emulator version (\d+(?:\.\d+)*)", stdout)
        if match:
            return match.group(1)
    return "unknown"


def probe_tpm2_tools_version(runner=run_command):
    """Queries tpm2_quote --version and returns version string."""
    code, stdout, _ = runner(["tpm2_quote", "--version"], timeout=5)
    if code == 0:
        match = re.search(r'version="(\d+(?:\.\d+)*)"', stdout)
        if match:
            return match.group(1)
    return "unknown"


def parse_digest(stdout):
    """Extracts 64-character lowercase hex digest from `DIGEST <hex>` stdout."""
    for line in stdout.splitlines():
        line = line.strip()
        if line.startswith("DIGEST "):
            parts = line.split()
            if len(parts) == 2 and len(parts[1]) == 64:
                hex_str = parts[1].lower()
                if all(c in "0123456789abcdef" for c in hex_str):
                    return hex_str
    raise GateError(f"no valid 'DIGEST <64-hex>' line found in cargo output: {stdout[:100]}")


def parse_pcr_selection(text):
    """Extracts hex string and PCR indices set from `tpm2_print` TPMS_ATTEST text."""
    for line in text.splitlines():
        line = line.strip()
        if line.startswith("pcrSelect:"):
            parts = line.split(":", 1)
            candidate = parts[1].strip()
            if candidate and all(c in "0123456789abcdefABCDEF" for c in candidate):
                raw_hex = candidate.lower()
                pcrs = set()
                try:
                    raw_bytes = bytes.fromhex(raw_hex)
                except ValueError:
                    continue
                for byte_idx, byte_val in enumerate(raw_bytes):
                    for bit in range(8):
                        if (byte_val >> bit) & 1:
                            pcrs.add(byte_idx * 8 + bit)
                return raw_hex, pcrs
    raise GateError(f"no valid pcrSelect line found in TPMS_ATTEST output: {text[:100]}")


def judge_selection(attest_text, expected_pcrs=REQUIRED_PCRS):
    """Judges whether TPMS_ATTEST pcrSelect matches expected PCR set.

    Returns (passed, reason). Rejection reason explicitly names missing PCRs.
    """
    try:
        raw_hex, pcrs = parse_pcr_selection(attest_text)
    except GateError as error:
        return False, str(error)

    expected_set = set(expected_pcrs)
    if pcrs == expected_set:
        return True, ""
    missing = expected_set - pcrs
    if 11 in missing:
        return False, f"quote selection rejected: missing PCR 11 (bitmap: {raw_hex})"
    if missing:
        missing_sorted = sorted(missing)
        return (
            False,
            f"quote selection rejected: missing PCR(s) {missing_sorted} (bitmap: {raw_hex})",
        )
    extra = pcrs - expected_set
    if extra:
        extra_sorted = sorted(extra)
        return (
            False,
            f"quote selection rejected: unexpected PCR(s) {extra_sorted} (bitmap: {raw_hex})",
        )
    return False, f"quote selection rejected: bitmap {raw_hex} decodes to {sorted(pcrs)}"


def judge_quote_verifies(checkquote_rc, selection_ok, selection_reason):
    """Judges case attest/quote-verifies."""
    if checkquote_rc != 0:
        return False, f"tpm2_checkquote exited {checkquote_rc}, expected 0"
    if not selection_ok:
        return False, f"selection rejected: {selection_reason}"
    return True, ""


def judge_other_record_fails(checkquote_rc):
    """Judges case attest/other-record-fails."""
    if checkquote_rc == 0:
        return False, "tpm2_checkquote unexpectedly succeeded with other record digest"
    return True, ""


def judge_pcr_state_changed_fails(checkquote_rc):
    """Judges case attest/pcr-state-changed-fails."""
    if checkquote_rc == 0:
        return False, "tpm2_checkquote unexpectedly succeeded against stale captured PCR file"
    return True, ""


def judge_exact_selection_accepted(selection_ok, selection_reason):
    """Judges case attest/exact-selection-accepted."""
    if not selection_ok:
        return False, f"selection judge rejected: {selection_reason}"
    return True, ""


def judge_missing_pcr11_rejected(checkquote_rc, selection_ok, selection_reason):
    """Judges case attest/missing-pcr11-rejected."""
    if checkquote_rc != 0:
        return False, f"tpm2_checkquote exited {checkquote_rc} for 0,4,7 quote, expected 0"
    if selection_ok:
        return False, "selection judge unexpectedly accepted quote missing PCR 11"
    if "11" not in selection_reason:
        return False, f"selection rejection did not name PCR 11: {selection_reason}"
    return True, ""


def summary(case_results):
    """Computes RESULT pass or RESULT fail line."""
    if len(case_results) != CASE_COUNT:
        return f"RESULT fail: {len(case_results)} of {CASE_COUNT} cases ran"
    failures = sum(1 for _name, passed, _why in case_results if not passed)
    if failures:
        return f"RESULT fail: {failures} case(s) failed"
    return "RESULT pass"


def swtpm_command(state_dir, sock_path, ctrl_sock_path):
    """Constructs swtpm socket command line."""
    return [
        "swtpm",
        "socket",
        "--tpm2",
        f"--tpmstate=dir={host.target(state_dir)}",
        f"--server=type=unixio,path={host.target(sock_path)}",
        f"--ctrl=type=unixio,path={host.target(ctrl_sock_path)}",
        "--flags=not-need-init,startup-clear",
    ]


def createek_command(ek_ctx):
    """Constructs tpm2_createek command line."""
    return ["tpm2_createek", "-c", host.target(ek_ctx), "-G", "rsa"]


def createak_command(ek_ctx, ak_ctx, ak_pub, ak_name):
    """Constructs tpm2_createak command line."""
    return [
        "tpm2_createak",
        "-C",
        host.target(ek_ctx),
        "-c",
        host.target(ak_ctx),
        "-G",
        "ecc",
        "-g",
        "sha256",
        "-s",
        "ecdsa",
        "-u",
        host.target(ak_pub),
        "-f",
        "pem",
        "-n",
        host.target(ak_name),
    ]


def flush_context_command(context_type):
    """Constructs tpm2_flushcontext command line for -t or -s."""
    return ["tpm2_flushcontext", context_type]


def pcrextend_command(pcr_idx, digest_hex):
    """Constructs tpm2_pcrextend command line."""
    return ["tpm2_pcrextend", f"{pcr_idx}:sha256={digest_hex}"]


def quote_command(ak_ctx, pcr_list_str, digest_hex, msg_path, sig_path, pcrs_path):
    """Constructs tpm2_quote command line."""
    return [
        "tpm2_quote",
        "-c",
        host.target(ak_ctx),
        "-l",
        f"sha256:{pcr_list_str}",
        "-q",
        digest_hex,
        "-m",
        host.target(msg_path),
        "-s",
        host.target(sig_path),
        "-o",
        host.target(pcrs_path),
        "-g",
        "sha256",
    ]


def checkquote_command(ak_pub, msg_path, sig_path, pcrs_path, digest_hex):
    """Constructs tpm2_checkquote command line."""
    return [
        "tpm2_checkquote",
        "-u",
        host.target(ak_pub),
        "-m",
        host.target(msg_path),
        "-s",
        host.target(sig_path),
        "-f",
        host.target(pcrs_path),
        "-g",
        "sha256",
        "-q",
        digest_hex,
    ]


def print_attest_command(msg_path):
    """Constructs tpm2_print command line."""
    return ["tpm2_print", "-t", "TPMS_ATTEST", host.target(msg_path)]


def cargo_digest_command(other=False):
    """Constructs cargo command line for record_digest example."""
    cmd = [
        "cargo",
        "run",
        "--locked",
        "--offline",
        "-q",
        "-p",
        "aegis-justitia",
        "--example",
        "record_digest",
    ]
    if other:
        cmd.append("other")
    return cmd


def get_record_digest(other=False, runner=run_command, cwd=ROOT):
    """Runs cargo to fetch canonical record digest."""
    env = dict(os.environ)
    if "CARGO_BUILD_JOBS" not in env:
        env["CARGO_BUILD_JOBS"] = "2"
    argv = cargo_digest_command(other=other)
    code, stdout, stderr = runner(argv, timeout=CHILD_TIMEOUT, environ=env, cwd=cwd)
    if code != 0:
        tail = "\n".join(stderr.strip().splitlines()[-10:])
        raise GateError(f"cargo run record_digest failed ({code}):\n{tail}")
    return parse_digest(stdout)


def run_step(runner, cmd, env, cwd):
    """Runs a required setup step; raises GateError on non-zero exit."""
    code, stdout, stderr = runner(cmd, timeout=CHILD_TIMEOUT, environ=env, cwd=cwd)
    if code != 0:
        tail = "\n".join(stderr.strip().splitlines()[-10:])
        raise GateError(f"{cmd[0]} failed ({code}):\n{tail}")
    return stdout


def start_swtpm(runtime_dir, popen_fn=subprocess.Popen):
    """Starts private swtpm instance in runtime_dir and waits for socket."""
    state_dir = runtime_dir / "tpmstate"
    state_dir.mkdir(exist_ok=True)
    sock_path = runtime_dir / "tpm.sock"
    ctrl_sock_path = runtime_dir / "tpm.sock.ctrl"
    log_path = runtime_dir / "swtpm.log"

    cmd = swtpm_command(state_dir, sock_path, ctrl_sock_path)
    log_file = open(log_path, "w", encoding="utf-8")
    with log_file:
        proc = popen_fn(
            cmd,
            stdout=log_file,
            stderr=subprocess.STDOUT,
            cwd=runtime_dir,
            start_new_session=True,
        )

    deadline = time.monotonic() + 5.0
    for _ in range(50):
        if sock_path.exists():
            return proc, sock_path
        if proc.poll() is not None:
            break
        if time.monotonic() >= deadline:
            break
        time.sleep(0.1)

    log_tail = ""
    if log_path.exists():
        lines = log_path.read_text(encoding="utf-8", errors="replace").splitlines()
        log_tail = "\n".join(lines[-20:])
    raise GateError(f"swtpm failed to create socket {sock_path}:\n{log_tail}")


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
        proc.wait(timeout=5)
    except subprocess.TimeoutExpired:
        pass


def setup_tpm_keys(runner, runtime_dir, tpm_env):
    """Creates EK and AK in the private TPM instance."""
    ek_ctx = runtime_dir / "ek.ctx"
    run_step(runner, createek_command(ek_ctx), tpm_env, runtime_dir)
    run_step(runner, flush_context_command("-t"), tpm_env, runtime_dir)
    run_step(runner, flush_context_command("-s"), tpm_env, runtime_dir)

    ak_ctx = runtime_dir / "ak.ctx"
    ak_pub = runtime_dir / "ak.pub"
    ak_name = runtime_dir / "ak.name"
    run_step(runner, createak_command(ek_ctx, ak_ctx, ak_pub, ak_name), tpm_env, runtime_dir)
    run_step(runner, flush_context_command("-t"), tpm_env, runtime_dir)
    run_step(runner, flush_context_command("-s"), tpm_env, runtime_dir)
    return ak_ctx, ak_pub


class Tpm:
    """One private TPM instance: the runner, its TCTI environment and the AK."""

    def __init__(self, runner, env, runtime_dir, ak_ctx, ak_pub):
        self.runner = runner
        self.env = env
        self.dir = runtime_dir
        self.ak_ctx = ak_ctx
        self.ak_pub = ak_pub

    def step(self, cmd):
        return run_step(self.runner, cmd, self.env, self.dir)

    def rc(self, cmd):
        code, _, _ = self.runner(cmd, timeout=CHILD_TIMEOUT, environ=self.env, cwd=self.dir)
        return code

    def text(self, cmd):
        _, stdout, _ = self.runner(cmd, timeout=CHILD_TIMEOUT, environ=self.env, cwd=self.dir)
        return stdout


def make_quote(tpm, pcr_list_str, digest_hex, stem):
    """Quotes the PCR list over digest_hex; returns (msg, sig, pcrs) paths."""
    msg = tpm.dir / f"{stem}.msg"
    sig = tpm.dir / f"{stem}.sig"
    pcrs = tpm.dir / f"{stem}.pcrs"
    tpm.step(quote_command(tpm.ak_ctx, pcr_list_str, digest_hex, msg, sig, pcrs))
    tpm.step(flush_context_command("-t"))
    return msg, sig, pcrs


def check_rc(tpm, quote, pcrs_path, digest_hex):
    """Runs tpm2_checkquote on quote against pcrs_path and returns its exit code."""
    msg, sig, _ = quote
    return tpm.rc(checkquote_command(tpm.ak_pub, msg, sig, pcrs_path, digest_hex))


def fresh_quote_cases(tpm, approved, other):
    """Cases 1, 2 and 4 on one quote over 0,4,7,11; returns them and the quote."""
    quote = make_quote(tpm, "0,4,7,11", approved, "q")
    sel_ok, sel_why = judge_selection(tpm.text(print_attest_command(quote[0])))
    ok1, why1 = judge_quote_verifies(check_rc(tpm, quote, quote[2], approved), sel_ok, sel_why)
    ok2, why2 = judge_other_record_fails(check_rc(tpm, quote, quote[2], other))
    ok4, why4 = judge_exact_selection_accepted(sel_ok, sel_why)
    cases = [
        ("attest/quote-verifies", ok1, why1),
        ("attest/other-record-fails", ok2, why2),
        ("attest/exact-selection-accepted", ok4, why4),
    ]
    return cases, quote


def stale_pcr_case(tpm, approved, earlier_quote):
    """Case 3: a quote taken after PCR 7 moved fails against the earlier PCR file."""
    captured = tpm.dir / "captured.pcrs"
    shutil.copyfile(earlier_quote[2], captured)
    tpm.step(pcrextend_command(7, "22" * 32))
    quote = make_quote(tpm, "0,4,7,11", approved, "q_new")
    ok, why = judge_pcr_state_changed_fails(check_rc(tpm, quote, captured, approved))
    return ("attest/pcr-state-changed-fails", ok, why)


def missing_pcr11_case(tpm, approved):
    """Case 5: a valid quote over 0,4,7 only is refused by the selection judge."""
    quote = make_quote(tpm, "0,4,7", approved, "q_missing")
    rc = check_rc(tpm, quote, quote[2], approved)
    sel_ok, sel_why = judge_selection(tpm.text(print_attest_command(quote[0])))
    ok, why = judge_missing_pcr11_rejected(rc, sel_ok, sel_why)
    return ("attest/missing-pcr11-rejected", ok, why)


def run_all_cases(runtime_dir, sock_path, runner=run_command, cwd=ROOT):
    """Executes all 5 attestation cases against the private TPM instance."""
    tpm_env = dict(os.environ)
    tpm_env["TPM2TOOLS_TCTI"] = f"swtpm:path={host.target(sock_path)}"

    approved = get_record_digest(other=False, runner=runner, cwd=cwd)
    other = get_record_digest(other=True, runner=runner, cwd=cwd)

    ak_ctx, ak_pub = setup_tpm_keys(runner, runtime_dir, tpm_env)
    tpm = Tpm(runner, tpm_env, runtime_dir, ak_ctx, ak_pub)
    for pcr in REQUIRED_PCRS:
        tpm.step(pcrextend_command(pcr, "11" * 32))

    fresh, quote = fresh_quote_cases(tpm, approved, other)
    stale = stale_pcr_case(tpm, approved, quote)
    missing = missing_pcr11_case(tpm, approved)
    return [fresh[0], fresh[1], stale, fresh[2], missing]


def report_run_summary(case_results, swtpm_ver, tpm2_ver):
    """Prints per-case results, versions note, and returns exit code."""
    for name, passed, why in case_results:
        sys.stdout.write(f"CASE {name} pass\n" if passed else f"CASE {name} fail: {why}\n")
        if passed and why:
            sys.stdout.write(f"NOTE {name}: {why}\n")
    sys.stdout.write(
        f"NOTE versions: swtpm {swtpm_ver}, tpm2-tools {tpm2_ver}; "
        "development evidence; not rooted in a firmware-verified boot chain (M11)\n"
    )
    result = summary(case_results)
    sys.stdout.write(result + "\n")
    return 0 if result == "RESULT pass" else 1


def main(argv=None):
    """Main verification harness driver."""
    reasons = capability_reasons()
    if reasons:
        sys.stdout.write(f"SKIP: {'; '.join(reasons)}; the attestation gate did not run.\n")
        sys.stdout.flush()
        return 0

    runtime_dir = Path(tempfile.mkdtemp(prefix="aegis-attest-"))
    os.chmod(runtime_dir, 0o700)
    swtpm_proc = None

    try:
        swtpm_ver = probe_swtpm_version(run_command)
        tpm2_ver = probe_tpm2_tools_version(run_command)

        swtpm_proc, sock_path = start_swtpm(runtime_dir)
        case_results = run_all_cases(runtime_dir, sock_path, runner=run_command, cwd=ROOT)
        return report_run_summary(case_results, swtpm_ver, tpm2_ver)
    except GateError as err:
        sys.stdout.write(f"RESULT error: {err}\n")
        return 1
    finally:
        if swtpm_proc is not None:
            teardown_process(swtpm_proc)
        shutil.rmtree(runtime_dir, ignore_errors=True)


if __name__ == "__main__":
    sys.exit(main())
