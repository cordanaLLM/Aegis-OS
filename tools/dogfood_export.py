#!/usr/bin/env python3
"""Allow-list export for dogfood findings.

This tool exports dogfood gate run records for public repositories only when
every key and value conforms to an explicit allow-list.

In accordance with ADR-0005, the exporter refuses rather than redacts: any
unrecognized key, malformed value, or suspicious pattern results in the whole
record being refused and no output file being written.

Verification proceeds through two independent barriers:
1. An allow-list gate enforcing exact top-level keys, schemas, formats, metric
   names, finite numeric bounds, string length limits, and existence checks
   against repository artifacts (crates, tools, roadmap milestones, requirements).
2. A deny scan over the serialized JSON representation checking for IP addresses,
   MAC addresses, email addresses, user home directories, private keys, SSH keys,
   sensitive token prefixes, the local hostname, and private deny-hosts entries.

The deny-hosts file is private to the workstation configuration repository and
must never be committed to public trees.
"""

import argparse
import datetime
import json
import math
import pathlib
import re
import socket
import sys

SCHEMA_ID = "aegis-dogfood-export/1"

METRICS = {
    "latency_us_p50": "us",
    "latency_us_p99": "us",
    "latency_us_max": "us",
    "frame_time_ms_p95": "ms",
    "cue_offset_ms_max": "ms",
    "energy_uj": "uj",
    "package_watts": "W",
    "boot_seconds": "s",
    "vfio_candidates": "count",
    "render_nodes": "count",
}

REQUIRED_KEYS = frozenset({"schema", "date", "module", "milestone", "verdict"})
OPTIONAL_KEYS = frozenset(
    {
        "requirement",
        "kernel_release",
        "driver",
        "pci_id",
        "evidence_sha256",
        "measurements",
    }
)
ALLOWED_KEYS = REQUIRED_KEYS | OPTIONAL_KEYS
VERDICTS = frozenset({"pass", "fail", "skip"})
DRIVERS = frozenset({"xe", "i915", "amdgpu", "nvidia", "vfio-pci"})

MAX_STRING_LENGTH = 128
MAX_MEASUREMENTS = 64
MAX_INPUT_BYTES = 1024 * 1024

DATE_RE = re.compile(r"^\d{4}-\d{2}-\d{2}$")
TOOL_FILE_RE = re.compile(r"^verify_[a-z0-9_]+\.py$")
MILESTONE_RE = re.compile(r"^M\d{2}$")
REQUIREMENT_RE = re.compile(r"^REQ-P\d{2}-\d{2}$")
KERNEL_RELEASE_RE = re.compile(
    r"^\d+\.\d+(?:\.\d+)?(?:-\d+)?(?:-(?:cachyos|arch|lts|zen|rt|aegis))*$"
)
PCI_ID_RE = re.compile(r"^[0-9a-f]{4}:[0-9a-f]{4}$")
SHA256_RE = re.compile(r"^[0-9a-f]{64}$")

IPV4_RE = re.compile(r"\b\d{1,3}(?:\.\d{1,3}){3}\b")
IPV6_RE = re.compile(
    r"(?:"
    r"(?:\b[0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\b"
    r"|"
    r"\b[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})*::[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})*\b"
    r"|"
    r"\b[0-9a-fA-F]{1,4}:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})*::"
    r"|"
    r"::[0-9a-fA-F]{1,4}:[0-9a-fA-F]{1,4}(?::[0-9a-fA-F]{1,4})*\b"
    r")"
)
MAC_RE = re.compile(r"(?i)\b[0-9a-f]{2}(?:[:-][0-9a-f]{2}){5}\b")
EMAIL_RE = re.compile(r"\b[A-Za-z0-9._%+-]+@[A-Za-z0-9.-]+\.[A-Za-z]{2,}\b")
PATH_RE = re.compile(r"/home/|/Users/|C:\\\\Users|C:\\Users")
KEY_RE = re.compile(r"-----BEGIN|\bssh-ed25519\b|\bssh-rsa\b|\becdsa-sha2\b")
TOKEN_RE = re.compile(r"\b(?:ghp_|gho_|github_pat_|glpat-|sk-|xox[abp]-|AKIA[0-9A-Z]{16})")


class Refused(Exception):
    """Raised when a dogfood record violates allow-list or deny policies."""

    def __init__(self, reasons: list[str]) -> None:
        super().__init__("; ".join(reasons))
        self.reasons = list(reasons)


def _check_top_level_keys(record: dict) -> list[str]:
    reasons = []
    for key in REQUIRED_KEYS:
        if key not in record:
            reasons.append(f"{key}: missing required field")
    for key in record:
        if key not in ALLOWED_KEYS:
            reasons.append(f"{key}: unknown top-level key")
    return reasons


def _check_schema(record: dict) -> list[str]:
    if "schema" not in record:
        return []
    val = record["schema"]
    if val != SCHEMA_ID:
        return [f"schema: must equal {SCHEMA_ID}"]
    return []


def _check_date(record: dict) -> list[str]:
    if "date" not in record:
        return []
    val = record["date"]
    if not isinstance(val, str) or not DATE_RE.fullmatch(val):
        return ["date: must match format YYYY-MM-DD"]
    try:
        datetime.date.fromisoformat(val)
    except ValueError:
        return ["date: invalid calendar date"]
    return []


def _check_module(record: dict, root: pathlib.Path) -> list[str]:
    if "module" not in record:
        return []
    val = record["module"]
    if not isinstance(val, str) or not val:
        return ["module: must be a non-empty string"]
    is_crate = (root / "crates" / val).is_dir() and pathlib.Path(val).name == val
    is_tool = (
        bool(TOOL_FILE_RE.fullmatch(val))
        and (root / "tools" / val).is_file()
        and pathlib.Path(val).name == val
    )
    if not (is_crate or is_tool):
        return ["module: must name an existing crate directory or verify_*.py tool"]
    return []


def _check_milestone(record: dict, root: pathlib.Path) -> list[str]:
    if "milestone" not in record:
        return []
    val = record["milestone"]
    if not isinstance(val, str) or not MILESTONE_RE.fullmatch(val):
        return ["milestone: must match pattern ^M\\d{2}$"]
    roadmap_path = root / "planning" / "roadmap.json"
    try:
        content = json.loads(roadmap_path.read_text(encoding="utf-8"))
        milestones = content.get("milestones", [])
        known = {m.get("id") for m in milestones if isinstance(m, dict)}
        if val not in known:
            return [f"milestone: milestone {val} not found in planning/roadmap.json"]
    except Exception as err:
        return [f"milestone: cannot verify against planning/roadmap.json: {err}"]
    return []


def _check_verdict(record: dict) -> list[str]:
    if "verdict" not in record:
        return []
    val = record["verdict"]
    if val not in VERDICTS:
        return ["verdict: must be one of pass, fail, skip"]
    return []


def _check_requirement(record: dict, root: pathlib.Path) -> list[str]:
    if "requirement" not in record:
        return []
    val = record["requirement"]
    if not isinstance(val, str) or not REQUIREMENT_RE.fullmatch(val):
        return ["requirement: must match pattern ^REQ-P\\d{2}-\\d{2}$"]
    req_path = root / "docs" / "roadmap" / "requirements.md"
    try:
        text = req_path.read_text(encoding="utf-8")
        if val not in text:
            return [f"requirement: requirement {val} not found in docs/roadmap/requirements.md"]
    except Exception as err:
        return [f"requirement: cannot read docs/roadmap/requirements.md: {err}"]
    return []


def _check_kernel_release(record: dict) -> list[str]:
    if "kernel_release" not in record:
        return []
    val = record["kernel_release"]
    if not isinstance(val, str) or not KERNEL_RELEASE_RE.fullmatch(val):
        return ["kernel_release: invalid format"]
    return []


def _check_driver(record: dict) -> list[str]:
    if "driver" not in record:
        return []
    val = record["driver"]
    if val not in DRIVERS:
        return ["driver: must be one of xe, i915, amdgpu, nvidia, vfio-pci"]
    return []


def _check_pci_id(record: dict) -> list[str]:
    if "pci_id" not in record:
        return []
    val = record["pci_id"]
    if not isinstance(val, str) or not PCI_ID_RE.fullmatch(val):
        return ["pci_id: must match format ^[0-9a-f]{4}:[0-9a-f]{4}$"]
    return []


def _check_evidence_sha256(record: dict) -> list[str]:
    if "evidence_sha256" not in record:
        return []
    val = record["evidence_sha256"]
    if not isinstance(val, str) or not SHA256_RE.fullmatch(val):
        return ["evidence_sha256: must be a 64-character lowercase hex string"]
    return []


def _check_measurements(record: dict) -> list[str]:
    if "measurements" not in record:
        return []
    val = record["measurements"]
    if not isinstance(val, dict):
        return ["measurements: must be a JSON object"]
    reasons = []
    if len(val) > MAX_MEASUREMENTS:
        reasons.append(f"measurements: exceeds maximum of {MAX_MEASUREMENTS} entries")
    for key, num in val.items():
        if key not in METRICS:
            reasons.append(f"measurements: unknown metric {key}")
        if isinstance(num, bool) or not isinstance(num, (int, float)):
            reasons.append(f"measurements.{key}: value must be a finite int or float (not bool)")
        elif not math.isfinite(num):
            reasons.append(f"measurements.{key}: value must be finite")
    return reasons


def _check_strings(record: dict) -> list[str]:
    reasons = []
    for key, val in record.items():
        if isinstance(val, str) and len(val) > MAX_STRING_LENGTH:
            reasons.append(f"{key}: string length exceeds {MAX_STRING_LENGTH} characters")
    return reasons


def check_allow_list(record: dict, root: pathlib.Path) -> list[str]:
    """Validate record against barrier-1 allow-list rules."""
    reasons = []
    reasons.extend(_check_top_level_keys(record))
    reasons.extend(_check_schema(record))
    reasons.extend(_check_date(record))
    reasons.extend(_check_module(record, root))
    reasons.extend(_check_milestone(record, root))
    reasons.extend(_check_verdict(record))
    reasons.extend(_check_requirement(record, root))
    reasons.extend(_check_kernel_release(record))
    reasons.extend(_check_driver(record))
    reasons.extend(_check_pci_id(record))
    reasons.extend(_check_evidence_sha256(record))
    reasons.extend(_check_measurements(record))
    reasons.extend(_check_strings(record))
    return reasons


def _build_hostnames(deny_hosts: frozenset[str]) -> set[str]:
    """Deny-hosts entries plus the running host's name and its first label.

    A host name that cannot be read leaves the scan incomplete, so it refuses
    instead of scanning without it.
    """
    hostnames = {item.strip() for item in deny_hosts if item.strip()}
    try:
        local_host = socket.gethostname().strip()
    except OSError as err:
        raise Refused([f"deny_scan: cannot read the host name: {err}"])
    if local_host:
        hostnames.add(local_host)
        first_label = local_host.split(".")[0].strip()
        if first_label:
            hostnames.add(first_label)
    return hostnames


def collect_deny_reasons(json_text: str, deny_hosts: frozenset[str]) -> list[str]:
    """Scan JSON text for prohibited patterns and denied hosts."""
    reasons = []
    if IPV4_RE.search(json_text):
        reasons.append("deny_scan: IPv4 address detected")
    if IPV6_RE.search(json_text):
        reasons.append("deny_scan: IPv6 address detected")
    if MAC_RE.search(json_text):
        reasons.append("deny_scan: MAC address detected")
    if EMAIL_RE.search(json_text):
        reasons.append("deny_scan: email address detected")
    if PATH_RE.search(json_text):
        reasons.append("deny_scan: forbidden path detected")
    if KEY_RE.search(json_text):
        reasons.append("deny_scan: private key or SSH key marker detected")
    if TOKEN_RE.search(json_text):
        reasons.append("deny_scan: sensitive token prefix detected")

    # The reason names no entry: the deny-hosts list is private and stderr may be kept.
    for host in _build_hostnames(deny_hosts):
        if re.search(rf"\b{re.escape(host)}\b", json_text, re.IGNORECASE):
            reasons.append("deny_scan: denied host name detected")
            break
    return reasons


def deny_scan(record: dict, deny_hosts: frozenset[str]) -> None:
    """Run barrier 2 independently; raise Refused if any pattern matched."""
    try:
        json_text = json.dumps(record, sort_keys=True)
    except Exception as err:
        raise Refused([f"deny_scan: failed to serialize to JSON: {err}"])
    reasons = collect_deny_reasons(json_text, deny_hosts)
    if reasons:
        raise Refused(reasons)


def to_json(obj: dict) -> str:
    """Render dict as deterministic canonical JSON with trailing newline."""
    return json.dumps(obj, sort_keys=True, indent=2) + "\n"


def export(record: dict, root: pathlib.Path, deny_hosts: frozenset[str] = frozenset()) -> dict:
    """Validate record against both barriers and return canonical dict."""
    if not isinstance(record, dict):
        raise Refused(["record: input must be a JSON object"])

    reasons = list(check_allow_list(record, root))
    canonical = {}
    for key in sorted(record):
        if key == "measurements" and isinstance(record[key], dict):
            canonical[key] = {m: record[key][m] for m in sorted(record[key])}
        else:
            canonical[key] = record[key]
    # Barrier 2 reads the output itself, not the input it was built from.
    try:
        deny_scan(canonical, deny_hosts)
    except Refused as exc:
        reasons.extend(exc.reasons)
    if reasons:
        raise Refused(reasons)
    return canonical


def load_deny_hosts(path: pathlib.Path) -> frozenset[str]:
    """Read deny-hosts file, ignoring comments and blank lines."""
    hosts = set()
    text = path.read_text(encoding="utf-8")
    for line in text.splitlines():
        line = line.split("#", 1)[0].strip()
        if line:
            hosts.add(line)
    return frozenset(hosts)


def _read_record(path: pathlib.Path) -> tuple[dict | None, str | None]:
    try:
        with open(path, "rb") as handle:
            raw_bytes = handle.read(MAX_INPUT_BYTES + 1)
    except Exception as err:
        return None, f"unreadable: {err}"
    if len(raw_bytes) >= MAX_INPUT_BYTES:
        return None, "file size exceeds 1 MiB limit"
    try:
        data = json.loads(raw_bytes.decode("utf-8"))
    except Exception as err:
        return None, f"invalid JSON: {err}"
    if not isinstance(data, dict):
        return None, "root value must be a JSON object"
    return data, None


def _handle_deny_hosts(path: pathlib.Path | None) -> tuple[frozenset[str] | None, str | None]:
    if path is None:
        return frozenset(), None
    try:
        return load_deny_hosts(path), None
    except Exception as err:
        return None, f"cannot read deny-hosts file: {err}"


def _write_output(content: str, out_path: pathlib.Path | None) -> int:
    if out_path is None:
        sys.stdout.write(content)
        return 0
    try:
        out_path.write_text(content, encoding="utf-8")
        return 0
    except Exception as err:
        sys.stderr.write(f"REFUSED {out_path}: cannot write output: {err}\n")
        return 1


def main(argv: list[str] | None = None) -> int:
    """CLI entry point for dogfood finding exports."""
    parser = argparse.ArgumentParser(
        description="Export dogfood findings under strict allow-list and deny rules."
    )
    parser.add_argument("record", type=pathlib.Path, help="Path to input record JSON")
    parser.add_argument("--out", type=pathlib.Path, default=None, help="Output destination file")
    parser.add_argument(
        "--deny-hosts", type=pathlib.Path, default=None, help="Path to private deny-hosts file"
    )
    parser.add_argument(
        "--root",
        type=pathlib.Path,
        default=pathlib.Path(__file__).resolve().parent.parent,
        help="Repository root directory",
    )
    args = parser.parse_args(argv)

    data, err_msg = _read_record(args.record)
    if err_msg is not None:
        sys.stderr.write(f"REFUSED {args.record}: {err_msg}\n")
        return 1

    deny_hosts, deny_err = _handle_deny_hosts(args.deny_hosts)
    if deny_err is not None:
        sys.stderr.write(f"REFUSED {args.deny_hosts}: {deny_err}\n")
        return 1

    try:
        canonical = export(data, args.root, deny_hosts)
    except Refused as exc:
        for reason in exc.reasons:
            sys.stderr.write(f"REFUSED {reason}\n")
        return 1

    return _write_output(to_json(canonical), args.out)


if __name__ == "__main__":
    sys.exit(main())
