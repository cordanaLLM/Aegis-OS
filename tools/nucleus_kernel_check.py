#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Check the Aegis kernel requirement against one kernel's configuration through nucleus (D94).

Milestone M10, epic E10-5. `build/kernel-requirement.json` is the M18 payload
Aegis hands to whoever builds the kernel. The rules that decide it against a
kernel's own configuration -- in M10 the text the Nucleus kernel printed from
its `/proc/config.gz` inside the guest -- are nucleus's: since nucleus#176 its
`scripts/verify_kernel_requirement.py` has a single-kernel mode,
``--kernel-config``, that holds them, and this repository no longer keeps a
Python copy of them. The rules it applies are those of
`crates/aegis-fabrica-defs/src/kernel.rs`: built-in by ``=y`` only, module by
``=m`` only, present by either, absent by ``=n`` or ``# CONFIG_X is not set``,
an unrecorded symbol satisfies no row, every listed architecture must be the
configuration's own (D104), and a given release must reach
``abi.minimum-release`` and equal ``abi.module-abi`` when the payload fixes one.

This module runs that mode from the nucleus checkout M09's `make
contract-fetch` keeps at the commit `build/contract/producers.pin.json` names,
under this interpreter with ``-I`` and ``-B`` as the contract gate runs the
same script (D106), and reads its ``nucleus.kernel-check-report.v1`` report,
never its text. A report whose verdict disagrees with the exit status, that
names another correlation id or release, or that decides other rows than the
payload lists is refused as no answer.

The runtime probes a row names (the LSM list, the BTF blob, the powercap and
IOMMU trees) are M10's guest readings, recorded beside this decision by
`tools/verify_nucleus_kernel.py`; this check decides the configuration only.
"""

import argparse
import json
import sys
import tempfile
from pathlib import Path

import verify_contract_pair as contract

ROOT = Path(__file__).resolve().parent.parent
REQUIREMENT = ROOT / "build" / "kernel-requirement.json"
REPORT_SCHEMA = "nucleus.kernel-check-report.v1"
# The M18 schema's bound on feature rows, which nucleus's decoder enforces.
MAX_FEATURES = 64
# Scalar bounds (HISS-02).
CHECK_TIMEOUT = 60
MAX_REPORT_BYTES = 1 << 20
MAX_TAIL_CHARS = 400
CHECK_KEYS = ("symbol", "required", "observed", "satisfied", "required-by", "probe")
# How a row records what the configuration gave a symbol.
UNOBSERVED = "unobserved"
NOT_SET = "not set"


class CheckError(Exception):
    """The check could not run, or what it returned is not an answer."""


def refuse_duplicates(pairs):
    """Return the object `pairs` makes, refusing a key that appears twice."""
    found = {}
    for key, value in pairs:
        if key in found:
            raise ValueError(f"duplicate key {key!r}")
        found[key] = value
    return found


def read_json(path, limit=MAX_REPORT_BYTES):
    """Return the JSON document at `path`, or raise CheckError."""
    try:
        data = Path(path).read_bytes()
        if len(data) > limit:
            raise CheckError(f"{path} is longer than {limit} bytes")
        return json.loads(data.decode("utf-8"), object_pairs_hook=refuse_duplicates)
    except (OSError, ValueError) as error:
        raise CheckError(f"{path} could not be read as JSON: {error}") from error


def load_requirement(path=REQUIREMENT):
    """Return the requirement payload; nucleus validates it on every check."""
    payload = read_json(path)
    if not isinstance(payload, dict) or not isinstance(payload.get("features"), list):
        raise CheckError(f"{path} is not a kernel requirement with a feature list")
    return payload


def verifier(store=None, pin=None):
    """Return the pinned nucleus verifier in M09's cache, or raise CheckError.

    The contract gate inside `make verify-all` proves the checkout is the pinned
    commit with nothing added; this reads its HEAD so that a cache fetched for
    another pin is refused here too, not used.
    """
    store = Path(store) if store is not None else contract.cache_dir()
    try:
        pin = pin if pin is not None else contract.load_pin()
    except contract.GateError as error:
        raise CheckError(str(error)) from error
    checkout = store / "nucleus"
    script = checkout / pin["nucleus"]["verifier"]
    if not script.is_file():
        raise CheckError(f"{script} does not exist; run `make contract-fetch`")
    try:
        head = (checkout / ".git" / "HEAD").read_text(encoding="utf-8").strip()
    except OSError as error:
        raise CheckError(f"the nucleus checkout has no readable HEAD: {error}") from error
    if head != pin["nucleus"]["commit"]:
        raise CheckError(
            f"the nucleus checkout is at {head!r}, not the pinned "
            f"{pin['nucleus']['commit']}; run `make contract-fetch`"
        )
    return script


def observed_text(value):
    """Return how a row records the value nucleus observed for its symbol."""
    if value is None:
        return UNOBSERVED
    return NOT_SET if value == "n" else value


def entry_problems(index, entry, feature):
    """Return why one decided check is not the payload's row `feature`, or []."""
    if not isinstance(entry, dict) or set(entry) != set(CHECK_KEYS):
        return [f"checks[{index}] is not an object with exactly {CHECK_KEYS}"]
    problems = []
    if not isinstance(entry["satisfied"], bool):
        problems.append(f"checks[{index}].satisfied is not a boolean")
    if entry["observed"] is not None and not isinstance(entry["observed"], str):
        problems.append(f"checks[{index}].observed is neither null nor a string")
    if not isinstance(feature, dict):
        return [*problems, f"checks[{index}] decides a row the payload does not list"]
    for key, wanted in (("symbol", "symbol"), ("required", "state"), ("probe", "probe")):
        if entry[key] != feature.get(wanted):
            problems.append(
                f"checks[{index}].{key} is {entry[key]!r}, the payload's is "
                f"{feature.get(wanted)!r}"
            )
    return problems


def report_problems(report, requirement, release, code):
    """Return why `report` is not this check's answer, exit status `code` included."""
    if not isinstance(report, dict):
        return ["the verifier wrote no JSON object"]
    problems = []
    if report.get("schema") != REPORT_SCHEMA:
        problems.append(f"schema is {report.get('schema')!r}, not {REPORT_SCHEMA}")
    if report.get("correlation-id") != requirement.get("correlation-id"):
        problems.append(f"correlation-id is {report.get('correlation-id')!r}")
    if report.get("release") != release:
        problems.append(f"release is {report.get('release')!r}, the check gave {release!r}")
    satisfied, reasons = report.get("satisfied"), report.get("reasons")
    if not isinstance(reasons, list) or not all(isinstance(line, str) for line in reasons):
        problems.append("reasons is not a list of strings")
    elif satisfied is not (not reasons) or satisfied is not (code == 0):
        problems.append(f"satisfied {satisfied!r}, {len(reasons)} reasons and exit {code} disagree")
    checks = report.get("checks")
    features = requirement["features"][:MAX_FEATURES]
    if not isinstance(checks, list) or len(checks) != len(features):
        return [*problems, f"checks does not decide the payload's {len(features)} rows"]
    for index, (entry, feature) in enumerate(zip(checks, features)):
        problems.extend(entry_problems(index, entry, feature))
    return problems


def tail(text):
    """Return the last characters of `text` on one line."""
    return " ".join(text.split())[-MAX_TAIL_CHARS:]


def run_verifier(script, config_path, requirement_path, release, report_path):
    """Run the single-kernel mode once; return its exit status and output."""
    argv = [
        sys.executable,
        "-I",
        "-B",
        str(script),
        "--kernel-config",
        str(config_path),
        "--requirement-file",
        str(requirement_path),
        "--report-json",
        str(report_path),
    ]
    if release is not None:
        argv += ["--kernel-release", release]
    try:
        code, stdout, stderr = contract.run(argv, CHECK_TIMEOUT, cwd=Path(script).parent)
    except contract.GateError as error:
        raise CheckError(str(error)) from error
    return code, stdout + stderr


def decide(config_text, release=None, requirement_path=REQUIREMENT, script=None):
    """Return the report nucleus wrote for `config_text`, or raise CheckError."""
    script = script if script is not None else verifier()
    requirement = load_requirement(requirement_path)
    with tempfile.TemporaryDirectory(prefix="aegis-kernel-check-") as scratch:
        config_path = Path(scratch) / "config"
        report_path = Path(scratch) / "report.json"
        config_path.write_text(config_text, encoding="utf-8")
        code, output = run_verifier(script, config_path, requirement_path, release, report_path)
        if code not in (0, 1):
            raise CheckError(f"the nucleus check exited {code}: {tail(output)}")
        report = read_json(report_path)
    problems = report_problems(report, requirement, release, code)
    if problems:
        raise CheckError(f"the nucleus report is no answer: {'; '.join(problems)}")
    return report


def check(requirement, config_text, release=None, requirement_path=REQUIREMENT, script=None):
    """Decide `requirement` against one configuration text through nucleus.

    Returns ``(rows, rejections)``; the kernel satisfies the requirement when
    the second is empty. Each rejection is nucleus's reason, which names the
    correlation id and, for a row, the symbol.
    """
    report = decide(config_text, release, requirement_path, script)
    if report["correlation-id"] != requirement.get("correlation-id"):
        raise CheckError("the requirement given is not the one nucleus checked")
    rows = [
        {
            "symbol": entry["symbol"],
            "required": entry["required"],
            "observed": observed_text(entry["observed"]),
            "probe": entry["probe"],
            "required-by": entry["required-by"],
            "satisfied": entry["satisfied"],
        }
        for entry in report["checks"]
    ]
    return rows, [f"rejected: {reason}" for reason in report["reasons"]]


def row_line(row):
    """Return the printable record of one decided row."""
    verdict = "satisfied" if row["satisfied"] else "NOT satisfied"
    return (
        f"{row['symbol']}: required {row['required']}, config {row['observed']} -> {verdict} "
        f"({row['required-by']}, probe {row['probe']})"
    )


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__.split("\n", 1)[0])
    parser.add_argument("--config", required=True, help="a kernel configuration text")
    parser.add_argument("--requirement", default=str(REQUIREMENT))
    parser.add_argument("--release", help="the kernel release, checked against the ABI floor")
    args = parser.parse_args(argv)
    try:
        requirement = load_requirement(args.requirement)
        text = Path(args.config).read_text(encoding="utf-8", errors="replace")
        decided, rejected = check(requirement, text, args.release, args.requirement)
    except (CheckError, OSError) as error:
        print(f"FAIL: the requirement could not be checked: {error}")
        return 2
    for row in decided:
        print(row_line(row))
    for line in rejected:
        print(line)
    if rejected:
        return 1
    print(
        f"PASS: correlation-id {requirement['correlation-id']}: {len(decided)} features satisfied"
    )
    return 0


if __name__ == "__main__":
    sys.exit(main())
