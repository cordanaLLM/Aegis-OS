#!/usr/bin/env python3
"""Hold unsafe code to crates/aegis-vulcan-gpu with safety proofs and scoped exception.

Per ledger decision aegis-os-m25-unsafe-home, crates/aegis-vulcan-gpu is the only
workspace member permitted to relax unsafe_code from 'forbid' to 'deny' to enable
ioctl and mmap calls for DMA-BUF, udmabuf, PRIME and VFIO.

This check fails when:
1. any member other than crates/aegis-vulcan-gpu lacks [lints] workspace = true,
   or root workspace.lints.rust.unsafe_code != 'forbid';
2. crates/aegis-vulcan-gpu lints differ from root workspace.lints in any key other than
   unsafe_code, or its unsafe_code != 'deny';
3. an unsafe block/fn/impl/trait/extern token appears in a .rs file outside
   crates/aegis-vulcan-gpu/ (skip target/; ignore occurrences inside // comments and
   string literals; scan crates/ and scaffold/ if it has .rs);
4. inside crates/aegis-vulcan-gpu an unsafe { block is not directly preceded
   (previous non-blank line(s)) by a // SAFETY: comment, or an unsafe item
   lacks #[allow(unsafe_code)] on its function;
5. .standards.yaml has not exactly one exceptions entry rule HISS-09, path
   crates/aegis-vulcan-gpu/Cargo.toml, with category C, an entered date,
   expires <= entered + 10 days, expires >= today, and a reason naming
   aegis-os-m25-unsafe-home.

Usage: ``python3 tools/verify_unsafe_home.py [ROOT]``.
``tools/test_verify_unsafe_home.py`` covers each refusal with a planted case.
"""

import datetime
import pathlib
import re
import sys

import tomllib

try:
    from verify_deviations import exception_entries
except ImportError:
    from tools.verify_deviations import exception_entries

ROOT = pathlib.Path(__file__).resolve().parent.parent
MANIFEST = ".standards.yaml"
VULCAN_GPU_MEMBER = "crates/aegis-vulcan-gpu"
VULCAN_GPU_MANIFEST = f"{VULCAN_GPU_MEMBER}/Cargo.toml"
UNSAFE_HOME_REASON = "aegis-os-m25-unsafe-home"
_PATTERN = r"\bunsafe\s*(?:\{|\b(?:async\s+)?fn\b|\bimpl\b|\btrait\b|\bextern\b)"
UNSAFE_PATTERN = re.compile(_PATTERN)


def _mask_block_comment(text, i, n, out):
    depth = 1
    out.extend([" ", " "])
    i += 2
    while i < n and depth > 0:
        if i + 1 < n and text[i : i + 2] == "/*":
            depth += 1
            out.extend([" ", " "])
            i += 2
        elif i + 1 < n and text[i : i + 2] == "*/":
            depth -= 1
            out.extend([" ", " "])
            i += 2
        else:
            out.append("\n" if text[i] == "\n" else " ")
            i += 1
    return i


def _mask_raw_string(text, i, n, out):
    j = i + 1
    while j < n and text[j] == "#":
        j += 1
    if j < n and text[j] == '"':
        hashes = j - (i + 1)
        ender = '"' + ("#" * hashes)
        out.extend([" "] * (j + 1 - i))
        i = j + 1
        while i < n:
            if text[i : i + len(ender)] == ender:
                out.extend([" "] * len(ender))
                i += len(ender)
                break
            out.append("\n" if text[i] == "\n" else " ")
            i += 1
    return i


def _mask_string(text, i, n, out):
    out.append(" ")
    i += 1
    while i < n:
        if text[i] == "\\" and i + 1 < n:
            out.append(" ")
            out.append("\n" if text[i + 1] == "\n" else " ")
            i += 2
        elif text[i] == '"':
            out.append(" ")
            i += 1
            break
        else:
            out.append("\n" if text[i] == "\n" else " ")
            i += 1
    return i


def mask_rust(text):
    """Replace comments and string literals with spaces, preserving newlines."""
    out = []
    i = 0
    n = len(text)
    while i < n:
        if i + 1 < n and text[i : i + 2] == "//":
            while i < n and text[i] != "\n":
                out.append(" ")
                i += 1
            continue
        if i + 1 < n and text[i : i + 2] == "/*":
            i = _mask_block_comment(text, i, n, out)
            continue
        if text[i] == "r" and i + 1 < n and (text[i + 1] in ('"', "#")):
            orig_i = i
            i = _mask_raw_string(text, i, n, out)
            if i != orig_i:
                continue
        if text[i] == '"':
            i = _mask_string(text, i, n, out)
            continue
        if text[i] == "'":
            if i + 2 < n and text[i + 2] == "'":
                out.extend([" "] * 3)
                i += 3
                continue
            if i + 3 < n and text[i + 1] == "\\" and text[i + 3] == "'":
                out.extend([" "] * 4)
                i += 4
                continue
        out.append(text[i])
        i += 1
    return "".join(out)


def _check_vulcan_gpu_manifest(member, member_toml, ws_rust, ws_clippy):
    problems = []
    m_lints = member_toml.get("lints", {})
    if m_lints.get("workspace") is True:
        problems.append(
            f"{member}/Cargo.toml: must not use '[lints] workspace = true'; "
            "must write out full lints table"
        )
    m_rust = m_lints.get("rust", {})
    m_clippy = m_lints.get("clippy", {})

    if m_rust.get("unsafe_code") != "deny":
        problems.append(
            f"{member}/Cargo.toml: lints.rust.unsafe_code must be 'deny', "
            f"found {m_rust.get('unsafe_code')!r}"
        )

    ws_rust_rest = {k: v for k, v in ws_rust.items() if k != "unsafe_code"}
    m_rust_rest = {k: v for k, v in m_rust.items() if k != "unsafe_code"}
    if ws_rust_rest != m_rust_rest:
        problems.append(
            f"{member}/Cargo.toml: lints.rust keys differ from workspace.lints.rust "
            f"(expected {ws_rust_rest}, found {m_rust_rest})"
        )
    if ws_clippy != m_clippy:
        err = f"{member}/Cargo.toml: lints.clippy differs from workspace.lints.clippy"
        problems.append(err)
    return problems


def check_workspace_manifests(root):
    """Check rule 1 and rule 2: workspace and member lints tables."""
    problems = []
    root_toml_path = root / "Cargo.toml"
    if not root_toml_path.is_file():
        return ["Cargo.toml: root workspace manifest missing"]

    root_toml = tomllib.loads(root_toml_path.read_text(encoding="utf-8"))
    ws_lints = root_toml.get("workspace", {}).get("lints", {})
    ws_rust = ws_lints.get("rust", {})
    ws_clippy = ws_lints.get("clippy", {})

    if ws_rust.get("unsafe_code") != "forbid":
        problems.append(
            f"Cargo.toml: workspace.lints.rust.unsafe_code must be 'forbid', "
            f"found {ws_rust.get('unsafe_code')!r}"
        )

    members = root_toml.get("workspace", {}).get("members", [])
    for member in members:
        member_manifest = root / member / "Cargo.toml"
        if not member_manifest.is_file():
            problems.append(f"{member}/Cargo.toml: manifest not found")
            continue
        member_toml = tomllib.loads(member_manifest.read_text(encoding="utf-8"))

        if member == VULCAN_GPU_MEMBER:
            errs = _check_vulcan_gpu_manifest(
                member,
                member_toml,
                ws_rust,
                ws_clippy,
            )
            problems.extend(errs)
        else:
            # Rule 1: every other member must have [lints] workspace = true
            if member_toml.get("lints", {}).get("workspace") is not True:
                msg = f"{member}/Cargo.toml: missing [lints] workspace = true"
                problems.append(msg)

    return problems


def check_unsafe_outside(root):
    """Check rule 3: no unsafe tokens outside crates/aegis-vulcan-gpu/."""
    problems = []
    scan_roots = [root / "crates"]
    scaffold = root / "scaffold"
    if scaffold.is_dir():
        scan_roots.append(scaffold)

    vulcan_gpu_dir = (root / VULCAN_GPU_MEMBER).resolve()

    for base in scan_roots:
        for rs_file in base.glob("**/*.rs"):
            if "target" in rs_file.parts:
                continue
            if vulcan_gpu_dir in rs_file.resolve().parents:
                continue
            text = rs_file.read_text(encoding="utf-8")
            masked = mask_rust(text)
            match = UNSAFE_PATTERN.search(masked)
            if match:
                rel = rs_file.relative_to(root)
                line_no = masked[: match.start()].count("\n") + 1
                problems.append(
                    f"{rel}:{line_no}: unsafe token outside {VULCAN_GPU_MEMBER} is forbidden"
                )
    return problems


def _has_preceding_safety(raw_lines, line_idx):
    """Check if unsafe block at line_idx is directly preceded by // SAFETY:."""
    prev_idx = line_idx - 1
    while prev_idx >= 0 and not raw_lines[prev_idx].strip():
        prev_idx -= 1
    while prev_idx >= 0 and raw_lines[prev_idx].strip().startswith("//"):
        if "// SAFETY:" in raw_lines[prev_idx]:
            return True
        prev_idx -= 1
    return False


def _has_enclosing_allow(raw_lines, line_idx):
    """Check that enclosing function (or item) has #[allow(unsafe_code)]."""
    fn_idx = line_idx
    fn_re = re.compile(r"^(?:pub\s+)?(?:async\s+)?(?:unsafe\s+)?fn\b")
    allow_tokens = ("#[allow(unsafe_code)]", "#[expect(unsafe_code)]")
    while fn_idx >= 0:
        line = raw_lines[fn_idx].strip()
        if any(tok in line for tok in allow_tokens):
            return True
        if fn_idx < line_idx and fn_re.match(line):
            attr_idx = fn_idx - 1
            while attr_idx >= 0:
                attr_line = raw_lines[attr_idx].strip()
                if not (attr_line.startswith("#") or not attr_line):
                    break
                if any(tok in attr_line for tok in allow_tokens):
                    return True
                attr_idx -= 1
            break
        fn_idx -= 1
    return False


def _check_vulcan_gpu_file(rel, text):
    """Check a single Rust source file in crates/aegis-vulcan-gpu."""
    problems = []
    masked = mask_rust(text)
    raw_lines = text.splitlines()

    for match in re.finditer(r"\bunsafe\b", masked):
        start = match.start()
        line_idx = masked[:start].count("\n")
        line_no = line_idx + 1

        is_block = bool(re.match(r"\bunsafe\s*\{", masked[start:]))
        if is_block and not _has_preceding_safety(raw_lines, line_idx):
            msg = (
                f"{rel}:{line_no}: unsafe block is not directly preceded by "
                "a '// SAFETY:' comment"
            )
            problems.append(msg)

        if not _has_enclosing_allow(raw_lines, line_idx):
            msg = f"{rel}:{line_no}: unsafe item lacks #[allow(unsafe_code)] on its function"
            problems.append(msg)
    return problems


def check_vulcan_gpu_safety(root):
    """Check rule 4: inside crates/aegis-vulcan-gpu, unsafe { requires // SAFETY: and allow."""
    problems = []
    vulcan_dir = root / VULCAN_GPU_MEMBER
    if not vulcan_dir.is_dir():
        return problems

    for rs_file in vulcan_dir.glob("**/*.rs"):
        if "target" in rs_file.parts:
            continue
        rel = rs_file.relative_to(root)
        content = rs_file.read_text(encoding="utf-8")
        problems.extend(_check_vulcan_gpu_file(rel, content))

    return problems


def check_exception_entry(root, today):
    """Check rule 5: exactly one valid HISS-09 exception in .standards.yaml."""
    manifest_path = root / MANIFEST
    if not manifest_path.is_file():
        return [f"{MANIFEST}: file not found"]

    entries = exception_entries(manifest_path.read_text(encoding="utf-8"))
    matching = []
    for e in entries:
        if e.get("rule") == "HISS-09" and e.get("path") == VULCAN_GPU_MANIFEST:
            matching.append(e)

    if len(matching) != 1:
        return [
            (
                f"{VULCAN_GPU_MANIFEST}: expected exactly one HISS-09 exceptions entry, "
                f"found {len(matching)}"
            )
        ]

    entry = matching[0]
    problems = []

    if entry.get("category") != "C":
        problems.append(
            f"{VULCAN_GPU_MANIFEST}: exception category must be 'C', "
            f"found {entry.get('category')!r}"
        )

    try:
        entered = datetime.date.fromisoformat(entry.get("entered", ""))
    except ValueError:
        entered = None
        msg = f"{VULCAN_GPU_MANIFEST}: exception entered date missing or invalid"
        problems.append(msg)

    try:
        expires = datetime.date.fromisoformat(entry.get("expires", ""))
    except ValueError:
        expires = None
        msg = f"{VULCAN_GPU_MANIFEST}: exception expires date missing or invalid"
        problems.append(msg)

    if entered and expires:
        if expires > entered + datetime.timedelta(days=10):
            problems.append(
                f"{VULCAN_GPU_MANIFEST}: exception expires {expires} > entered + 10 days "
                f"({entered + datetime.timedelta(days=10)})"
            )
        if expires < today:
            problems.append(f"{VULCAN_GPU_MANIFEST}: exception expired on {expires}")

    reason = entry.get("reason", "")
    if UNSAFE_HOME_REASON not in reason:
        problems.append(
            f"{VULCAN_GPU_MANIFEST}: exception reason does not name {UNSAFE_HOME_REASON!r}"
        )

    return problems


def check(root, today):
    """Run all checks and return the list of problem descriptions."""
    problems = []
    problems.extend(check_workspace_manifests(root))
    problems.extend(check_unsafe_outside(root))
    problems.extend(check_vulcan_gpu_safety(root))
    problems.extend(check_exception_entry(root, today))
    return problems


def main(argv):
    """Run the check and report PASS or each FAIL line."""
    root = pathlib.Path(argv[1]) if len(argv) > 1 else ROOT
    try:
        today = datetime.datetime.now(datetime.timezone.utc).date()
        problems = check(root, today)
    except (OSError, tomllib.TOMLDecodeError) as error:
        print(f"FAIL: {error!r}", file=sys.stderr)
        return 1

    for problem in problems:
        print(f"FAIL: {problem}", file=sys.stderr)
    if problems:
        return 1

    print(
        f"PASS: unsafe code held to {VULCAN_GPU_MEMBER} with deny lints, "
        "safety proofs, and live Category C exception."
    )
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
