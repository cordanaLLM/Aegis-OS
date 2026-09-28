#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative and boundary coverage for the M24 boot harness (HISS-15).

The gate itself needs KVM, QEMU, OVMF, swtpm and a cached 630 MB artifact. These
tests need none of them. They exercise every decision the gate makes on the way
-- the pin it accepts, the producer-version floor, the timeout rule, what it
reads out of a guest report and a GPT, which PCR reading counts as extended,
the argument vectors it builds, which programs it may start at all and whether
each has a deadline -- so a checkout that cannot boot a guest still fails when
one of those drifts. The sweeps at the end hold the gate to HISS-02 and HISS-04.
"""

import ast
import contextlib
import io
import json
import re
import socket
import struct
import tempfile
import time
import unittest
import uuid
from contextlib import redirect_stderr, redirect_stdout
from pathlib import Path
from unittest import mock

import verify_boot_harness as gate

ROOT = Path(__file__).resolve().parent.parent
GATE = ROOT / "tools" / "verify_boot_harness.py"
ADMISSION = ROOT / "docs" / "roadmap" / "toolchain-admission.md"
EVIDENCE_PAGE = ROOT / "docs" / "build" / "boot-harness.md"
MAKEFILE = ROOT / "Makefile"
ADMISSION_HEADING = "## The boot harness's toolchain"
MAX_PAGE_LINES = 4000
MAX_RESOLUTION_STEPS = 8
MAX_FUNCTION_LINES = 60
MAX_COMPLEXITY = 10
MAX_STATEMENTS = 50
BRANCHES = (ast.If, ast.For, ast.While, ast.IfExp, ast.ExceptHandler, ast.With, ast.Assert)
TABLE_ROW = re.compile(r"^\|([^|]*)\|([^|]*)\|([^|]*)\|[^|]*\|[^|]*\|$")
REFERENCE = re.compile(r"[0-9a-f]{64}|\d+(?:\.\d+)+(?:\.pl\d+)?")

# Every program the gate is allowed to start. Nothing here installs a package,
# writes a bootloader entry or touches the host firmware: the host's Secure
# Boot state is read from efivarfs by the gate itself, never through a tool. A
# program added to the gate without being added here fails `make verify-all`.
ALLOWED_PROGRAMS = {
    "qemu-system-x86_64",
    "qemu-img",
    "swtpm",
    "swtpm_setup",
    "virt-fw-vars",
    "sbsign",
    "sbverify",
    "xorriso",
    "mcopy",
    "openssl",
    "gpg",
    "curl",
}
# Tools that write host firmware state or boot entries, or that stand in for
# the imported integration script REQ-BOOT-02 forbids as boot evidence.
FORBIDDEN_NAMES = ("bootctl", "efibootmgr", "mokutil", "sbctl", "efi-updatevar", "chattr")
FORBIDDEN_SOURCES = ("integration_test", ".workingdir", "notebookllmprep")

PIN = gate.load_pin(gate.PIN)
NONCE = "aegis-m24-" + "ab" * 16
SIGNED_TEXT = (
    "# Fedora-Cloud-Base-Generic-44-1.7.x86_64.qcow2: 583729152 bytes\n"
    "SHA256 (Fedora-Cloud-Base-Generic-44-1.7.x86_64.qcow2) = "
    "28680fe5b371a5a82ebf43a31926e086a168e59949d03969c5093e7071f90b7f\n"
    "# Fedora-Cloud-Base-UEFI-UKI-44-1.7.x86_64.qcow2: 630784000 bytes\n"
    "SHA256 (Fedora-Cloud-Base-UEFI-UKI-44-1.7.x86_64.qcow2) = "
    "2b0af3e6bf4add3695e52df5db3b2eb48d081ab38963ae48c35fca45d5c31b64\n"
)
EXTENDED = "246BB9CEDB66A7DA85235E267E29CF0E1F98A6F2A79D33631E423595ECE181AC"


def pin_copy(**changes):
    """Return a deep copy of the committed pin with `changes` applied at dotted keys."""
    pin = json.loads(json.dumps(PIN))
    for dotted, value in changes.items():
        section, _dot, key = dotted.rpartition(".")
        (pin[section] if section else pin)[key] = value
    return pin


def written_pin(pin, directory):
    """Write `pin` into `directory` and return the path."""
    path = Path(directory) / "pin.json"
    path.write_text(json.dumps(pin), encoding="utf-8")
    return path


def guest_report(nonce=NONCE, pcr0=EXTENDED, secure="6 0 0 0 1"):
    """Return one serial report in the shape the guest script writes, control codes included."""
    lines = [
        "\x1b[2J\x1b[001;001HBdsDxe: starting Boot0002",
        f"{gate.GUEST_MARK}-BEGIN",
        f"{gate.GUEST_MARK}-NONCE {nonce}",
        f"{gate.GUEST_MARK}-UNAME-R 6.19.10-300.fc44.x86_64",
        f"{gate.GUEST_MARK}-PCR-0 {pcr0}",
        f"{gate.GUEST_MARK}-PCR-4 {EXTENDED}",
        f"{gate.GUEST_MARK}-PCR-7 {EXTENDED}",
        f"{gate.GUEST_MARK}-PCR-11 {'0' * 64}",
        f"{gate.GUEST_MARK}-SECUREBOOT {secure}",
        f"{gate.GUEST_MARK}-SETUPMODE 6 0 0 0 0",
        f"{gate.GUEST_MARK}-END",
    ]
    return "\r\n".join(lines) + "\r\n"


def observed_boot(**changes):
    """Return one `boot_guest` result for a clean KVM boot, with `changes` applied."""
    observed = {
        "outcome": "reached",
        "elapsed": 15.0,
        "timeout": 180,
        "seen": "prompt",
        "nonce": NONCE,
        "fields": gate.guest_fields(guest_report()),
        "exit_status": 0,
        "powerdown": True,
        "kvm": {"enabled": True, "present": True},
    }
    observed.update(changes)
    return observed


def gpt_image(entries):
    """Return LBA 0 to 33 of a disk carrying a GPT whose entries are `(type, first, last)`."""
    table = bytearray(34 * 512)
    table[512:520] = b"EFI PART"
    struct.pack_into("<QII", table, 584, 2, 128, 128)
    for index, (kind, first, last) in enumerate(entries):
        start = 1024 + index * 128
        table[start : start + 16] = kind.bytes_le
        struct.pack_into("<QQ", table, start + 32, first, last)
    return bytes(table)


def admitted_rows():
    """Return the M24 admission table as ``{name: (reference, floor)}``."""
    found = {}
    inside = False
    for line in ADMISSION.read_text(encoding="utf-8").splitlines()[:MAX_PAGE_LINES]:
        if line.startswith("## "):
            inside = line.startswith(ADMISSION_HEADING)
            continue
        match = TABLE_ROW.match(line) if inside else None
        if match is None:
            continue
        name, reference, floor = (cell.strip() for cell in match.groups())
        if name == "Tool" or set(name) <= set(": -"):
            continue
        value = REFERENCE.search(reference)
        found[name.split()[0]] = (
            value.group(0) if value else reference,
            None if floor.startswith("none") else floor.split()[0],
        )
    return found


def function_index(tree):
    """Return every function definition in `tree` by name."""
    return {node.name: node for node in ast.walk(tree) if isinstance(node, ast.FunctionDef)}


def bindings(scope):
    """Return ``{name: first value assigned}`` for plain assignments inside one function."""
    bound = {}
    for node in ast.walk(scope):
        if isinstance(node, ast.Assign):
            for target in node.targets:
                if isinstance(target, ast.Name):
                    bound.setdefault(target.id, node.value)
    return bound


def returned(function):
    """Return the value expression of the first `return` in `function`."""
    for node in ast.walk(function):
        if isinstance(node, ast.Return) and node.value is not None:
            return node.value
    return None


def literal_head(node):
    """Return the first element of a list or tuple literal when it is a plain string."""
    first = node.elts[0] if node.elts else None
    if isinstance(first, ast.Constant) and isinstance(first.value, str):
        return first.value
    return None


def argv_head(expression, scope, functions):
    """Resolve the program an argv expression starts.

    Follows a local name to its assignment, a concatenation to its left operand
    and a call to one of the gate's own helpers to what that helper returns --
    iteratively, with a fixed step bound, and never by recursion (HISS-01).
    """
    for _ in range(MAX_RESOLUTION_STEPS):
        if isinstance(expression, (ast.List, ast.Tuple)):
            return literal_head(expression)
        if isinstance(expression, ast.BinOp) and isinstance(expression.op, ast.Add):
            expression = expression.left
        elif isinstance(expression, ast.Name) and expression.id in bindings(scope):
            expression = bindings(scope)[expression.id]
        elif isinstance(expression, ast.Call) and getattr(expression.func, "id", None) in functions:
            scope = functions[expression.func.id]
            expression = returned(scope)
        else:
            return None
    return None


def call_sites(path):
    """Return (programs the gate can start, functions whose call site no tree can name)."""
    tree = ast.parse(path.read_text(encoding="utf-8"))
    functions = function_index(tree)
    resolved, undecided = set(), []
    for scope in functions.values():
        for node in ast.walk(scope):
            if not isinstance(node, ast.Call) or not node.args:
                continue
            name = getattr(node.func, "id", None) or getattr(node.func, "attr", None)
            if name not in {"run", "session", "Popen"}:
                continue
            head = argv_head(node.args[0], scope, functions)
            if head is None:
                undecided.append(scope.name)
            else:
                resolved.add(head)
    return resolved, undecided


def complexity(function):
    """Return a McCabe-style count: one plus each branch and each extra boolean operand."""
    count = 1
    for node in ast.walk(function):
        if isinstance(node, BRANCHES):
            count += 1
        elif isinstance(node, ast.BoolOp):
            count += len(node.values) - 1
        elif isinstance(node, ast.comprehension):
            count += 1 + len(node.ifs)
    return count


class AdmissionTests(unittest.TestCase):
    """The page and the gate state the same admission, tools and firmware images alike."""

    def test_every_admitted_tool_and_image_appears_on_both_sides(self):
        page = admitted_rows()
        code = {row[0]: (row[4], row[3]) for row in gate.TOOLCHAIN}
        code.update({name: (digest, None) for name, digest in gate.FIRMWARE})
        self.assertEqual(set(page), set(code), "the page and the gate admit different things")
        for name, recorded in code.items():
            self.assertEqual(page[name], recorded, f"{name} differs between the two")

    def test_the_three_ovmf_images_criterion_1_names_are_admitted(self):
        names = {name for name, _digest in gate.FIRMWARE}
        self.assertEqual(names, {"OVMF_CODE.4m.fd", "OVMF_VARS.4m.fd", "OVMF_CODE.secboot.4m.fd"})
        for _name, digest in gate.FIRMWARE:
            self.assertRegex(digest, r"^[0-9a-f]{64}$")

    def test_the_milestone_pins_are_the_recorded_versions(self):
        pinned = {row[0]: row[4] for row in gate.TOOLCHAIN}
        self.assertEqual(pinned["qemu-system-x86_64"], "11.1.1")
        self.assertEqual(pinned["swtpm"], "0.10.2")
        self.assertEqual(pinned["sbsign"], "0.9.5")
        self.assertEqual(pinned["virt-firmware"], "26.9")

    def test_a_floor_is_never_above_its_recorded_reference(self):
        for name, _argv, _pattern, floor, reference in gate.TOOLCHAIN:
            if floor is not None:
                self.assertLessEqual(gate.version_tuple(floor), gate.version_tuple(reference), name)

    def test_the_page_records_the_pin_and_the_signing_key(self):
        text = ADMISSION.read_text(encoding="utf-8")
        self.assertIn(PIN["sha256"], text)
        self.assertIn(PIN["signature"]["fingerprint"], text)
        self.assertIn(PIN["version-floor"], text)

    def test_a_version_banner_is_read_through_the_row_pattern(self):
        row = ("sbsign", ["sbsign", "--version"], r"sbsign (\d+(?:\.\d+)*)", None, "0.9.5")
        with mock.patch.object(gate.shutil, "which", return_value="/usr/bin/sbsign"):
            with mock.patch.object(gate, "run", return_value=(0, "sbsign 0.9.5\n", "")):
                self.assertEqual(gate.read_version(row), ("sbsign 0.9.5", "0.9.5"))

    def test_an_absent_tool_is_a_reason_and_not_a_crash(self):
        with mock.patch.object(gate.shutil, "which", return_value=None):
            banner, found = gate.read_version(gate.TOOLCHAIN[0])
        self.assertIsNone(found)
        self.assertIn("not on PATH", banner)

    def test_the_metadata_row_reads_the_distribution_version(self):
        with mock.patch.object(gate.importlib.metadata, "version", return_value="26.9"):
            self.assertEqual(gate.metadata_version("virt-firmware")[1], "26.9")
        missing = gate.importlib.metadata.PackageNotFoundError
        with mock.patch.object(gate.importlib.metadata, "version", side_effect=missing):
            banner, found = gate.metadata_version("virt-firmware")
        self.assertIsNone(found)
        self.assertIn("no virt-firmware distribution metadata", banner)


class PinTests(unittest.TestCase):
    """The pin is refused whole unless every field is one the harness can act on."""

    def test_the_committed_pin_names_the_verified_fedora_image(self):
        self.assertEqual(PIN["file"], "Fedora-Cloud-Base-UEFI-UKI-44-1.7.x86_64.qcow2")
        self.assertEqual(
            PIN["sha256"], "2b0af3e6bf4add3695e52df5db3b2eb48d081ab38963ae48c35fca45d5c31b64"
        )
        self.assertEqual(
            PIN["signature"]["fingerprint"], "36F612DCF27F7D1A48A835E4DBFCF71C6D9F90A6"
        )
        self.assertEqual(PIN["version"], PIN["version-floor"])

    def test_a_pin_is_immutable_evidence_not_a_moving_path(self):
        for url in (PIN["url"], PIN["signature"]["checksum-url"]):
            self.assertIn("/releases/44/", url)
            self.assertNotRegex(url, r"latest|current|rawhide|development")

    def test_each_defect_is_refused(self):
        for changes in (
            {"sha256": "2B0A" + PIN["sha256"][4:]},
            {"signature.fingerprint": "36f612dcf27f7d1a48a835e4dbfcf71c6d9f90a6"},
            {"signature.scheme": "cosign"},
            {"url": PIN["url"].replace("https://", "http://")},
            {"file": "Fedora-Cloud-Base-UEFI-UKI-44-1.6.x86_64.qcow2"},
            {"version-floor": "44"},
            {"boot.guest-microsoft-db": "win23"},
            {"boot.uki-path": "../EFI/Linux/x.efi"},
            {"schema": "aegis.m24.boot-artifact-pin.v0"},
        ):
            with self.subTest(changes=changes), tempfile.TemporaryDirectory() as base:
                with self.assertRaises(gate.GateError):
                    gate.load_pin(written_pin(pin_copy(**changes), base))

    def test_a_missing_section_is_refused_before_its_values_are_read(self):
        pin = pin_copy()
        del pin["boot"]
        with tempfile.TemporaryDirectory() as base:
            with self.assertRaises(gate.GateError):
                gate.load_pin(written_pin(pin, base))

    def test_the_timeout_range_is_bounded_at_both_ends(self):
        """Boundary: zero and one past the cap are refused, one and the cap are accepted."""
        cases = ((0, False), (1, True), (gate.MAX_LOGIN_TIMEOUT, True))
        cases += ((gate.MAX_LOGIN_TIMEOUT + 1, False), (1.5, False))
        for timeout, accepted in cases:
            pin = pin_copy(**{"boot.login-prompt-timeout-seconds": timeout})
            with self.subTest(timeout=timeout), tempfile.TemporaryDirectory() as base:
                if accepted:
                    self.assertEqual(gate.load_pin(written_pin(pin, base))["boot"], pin["boot"])
                else:
                    with self.assertRaises(gate.GateError):
                        gate.load_pin(written_pin(pin, base))

    def test_text_that_is_not_json_is_refused(self):
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "pin.json"
            path.write_text("not json", encoding="utf-8")
            with self.assertRaises(gate.GateError):
                gate.load_pin(path)


class VersionFloorTests(unittest.TestCase):
    """E24-1 boundary: a producer version at the floor is admitted, one below is refused."""

    def test_at_below_and_above_the_floor(self):
        self.assertTrue(gate.version_admitted("44-1.7", "44-1.7"))
        self.assertFalse(gate.version_admitted("44-1.6", "44-1.7"))
        self.assertTrue(gate.version_admitted("44-1.8", "44-1.7"))
        self.assertTrue(gate.version_admitted("45-1.1", "44-1.7"))
        self.assertFalse(gate.version_admitted("43-9.9", "44-1.7"))

    def test_one_below_is_one_step_in_the_last_component(self):
        self.assertEqual(gate.version_below("44-1.7"), "44-1.6")

    def test_a_floor_without_a_predecessor_is_a_gate_error(self):
        with self.assertRaises(gate.GateError):
            gate.version_below("44-1.0")

    def test_an_unorderable_version_is_a_gate_error(self):
        for text in ("44", "44-1", "latest", "44-1.7-rc1", ""):
            with self.subTest(text=text), self.assertRaises(gate.GateError):
                gate.version_key(text)

    def test_the_boundary_case_reports_both_outcomes(self):
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.version_boundary_case(PIN)
        self.assertEqual(problems, [])
        self.assertIn("PASS boot/producer-version-boundary", printed.getvalue())
        self.assertIn("44-1.6 -> refused", printed.getvalue())


class TimeoutRuleTests(unittest.TestCase):
    """Criterion 6: the rule is inclusive, and it is exercised on synthetic times (D73)."""

    def test_the_recorded_timeout_is_180_seconds(self):
        """D84's timeout answer: 180 s, over a slowest measured prompt of 65.7 s."""
        self.assertEqual(PIN["boot"]["login-prompt-timeout-seconds"], 180)

    def test_one_second_under_at_and_over_the_timeout(self):
        timeout = PIN["boot"]["login-prompt-timeout-seconds"]
        self.assertEqual(gate.login_outcome(timeout - 1, timeout), "reached")
        self.assertEqual(gate.login_outcome(timeout, timeout), "reached")
        self.assertEqual(gate.login_outcome(timeout + 1, timeout), "missed")

    def test_a_prompt_never_seen_is_a_miss_whatever_the_timeout(self):
        self.assertEqual(gate.login_outcome(None, gate.MAX_LOGIN_TIMEOUT), "missed")

    def test_the_boundary_case_reports_two_outcomes_over_three_points(self):
        printed = io.StringIO()
        with redirect_stdout(printed):
            problems = gate.timeout_boundary_case(180)
        self.assertEqual(problems, [])
        text = printed.getvalue()
        self.assertIn("PASS boot/timeout-boundary", text)
        self.assertIn("recorded timeout 180 s, inclusive", text)
        self.assertIn("prompt after 179 s -> reached", text)
        self.assertIn("prompt after 180 s -> reached", text)
        self.assertIn("prompt after 181 s -> missed", text)
        self.assertIn("not boots", text)

    def test_the_deliberately_short_timeout_is_below_any_recorded_boot(self):
        self.assertLess(gate.TOO_SHORT_TIMEOUT, 10)
        self.assertGreater(PIN["boot"]["login-prompt-timeout-seconds"], 4 * gate.TOO_SHORT_TIMEOUT)

    def test_a_poll_returns_the_first_value_it_sees(self):
        value, elapsed = gate.wait_for(lambda: "prompt", 5, time.monotonic())
        self.assertEqual(value, "prompt")
        self.assertLess(elapsed, 5)

    def test_a_poll_that_sees_nothing_ends_at_its_deadline(self):
        with mock.patch.object(gate, "POLL_INTERVAL", 0.01):
            value, elapsed = gate.wait_for(lambda: None, 0.05, time.monotonic())
        self.assertIsNone(value)
        self.assertGreater(elapsed, 0.04)


class SignedRecordTests(unittest.TestCase):
    """What the signed CHECKSUM text is allowed to say about the pinned file."""

    def test_the_signed_line_yields_the_digest_and_the_version(self):
        self.assertEqual(gate.signed_record(SIGNED_TEXT, PIN), (PIN["sha256"], "44-1.7"))

    def test_a_second_line_for_the_same_image_is_refused(self):
        with self.assertRaises(gate.Refused) as refused:
            gate.signed_record(SIGNED_TEXT + SIGNED_TEXT, PIN)
        self.assertEqual(refused.exception.stage, "checksum")

    def test_a_text_without_the_image_is_refused(self):
        with self.assertRaises(gate.Refused):
            gate.signed_record(SIGNED_TEXT.splitlines()[1], PIN)

    def test_gpg_status_keeps_the_first_line_of_each_keyword(self):
        status = gate.gpg_status(
            "[GNUPG:] NEWSIG\n[GNUPG:] GOODSIG DBFCF71C6D9F90A6 Fedora (44)\n"
            "[GNUPG:] VALIDSIG 36F612 2026-04-24 1777044852 0 4 0 1 8 01 36F612DC\n"
            "gpg: not a status line\n"
        )
        self.assertEqual(status["GOODSIG"], "DBFCF71C6D9F90A6 Fedora (44)")
        self.assertEqual(status["VALIDSIG"].split()[-1], "36F612DC")
        self.assertEqual(status["BADSIG"], "")


class DigestTests(unittest.TestCase):
    """The bytes are refused unless their size and digest are the pinned ones."""

    def pinned_file(self, base, data):
        path = Path(base) / "artifact.qcow2"
        path.write_bytes(data)
        return path, pin_copy(size=len(data), sha256=gate.hashlib.sha256(data).hexdigest())

    def test_the_pinned_bytes_pass(self):
        with tempfile.TemporaryDirectory() as base:
            path, pin = self.pinned_file(base, b"pinned bytes" * 100)
            self.assertEqual(gate.require_pinned(pin, path), pin["sha256"])

    def test_one_flipped_bit_is_refused_at_the_digest(self):
        with tempfile.TemporaryDirectory() as base:
            path, pin = self.pinned_file(base, b"pinned bytes" * 100)
            gate.flip_byte(path, 600)
            with self.assertRaises(gate.Refused) as refused:
                gate.require_pinned(pin, path)
        self.assertEqual(refused.exception.stage, "digest")

    def test_a_different_size_is_refused_before_hashing(self):
        with tempfile.TemporaryDirectory() as base:
            path, pin = self.pinned_file(base, b"x" * 10)
            pin["size"] = 11
            with mock.patch.object(gate, "file_digest") as digest:
                with self.assertRaises(gate.Refused):
                    gate.require_pinned(pin, path)
            digest.assert_not_called()

    def test_a_refused_artifact_runs_no_program_and_writes_no_file(self):
        """The digest is checked first, so a bad copy gets no seed, store, TPM or overlay."""
        with tempfile.TemporaryDirectory() as base:
            path, pin = self.pinned_file(base, b"pinned bytes" * 100)
            gate.flip_byte(path, 7)
            context = gate.Context(pin, Path(base), Path(base) / "run")
            context.key = {"key": Path("k"), "cert": Path("c"), "owner": "o", "subject": "s"}
            boot = gate.Boot(Path(base) / "boot")
            with mock.patch.object(gate, "run", return_value=(0, "", "")) as run:
                with mock.patch.object(gate, "session") as spawn:
                    with self.assertRaises(gate.Refused):
                        gate.boot_guest(context, boot, "secure", path)
            written = sorted(entry.name for entry in boot.dir.iterdir())
        run.assert_not_called()
        spawn.assert_not_called()
        self.assertEqual(written, [])
        self.assertEqual(context.spawned, [])

    def test_the_pinned_artifact_is_hashed_again_just_before_the_overlay(self):
        """The first check guards the writes, the last one is the check before boot."""
        with tempfile.TemporaryDirectory() as base:
            path, pin = self.pinned_file(base, b"pinned bytes" * 100)
            context = gate.Context(pin, Path(base), Path(base) / "run")
            context.key = {"key": Path("k"), "cert": Path("c"), "owner": "o", "subject": "s"}
            order = []
            checked = gate.require_pinned

            def record_check(*args):
                order.append("digest")
                return checked(*args)

            def record_run(argv, _timeout, cwd=None):
                order.append(argv[0])
                return 0, "", ""

            with mock.patch.object(gate, "require_pinned", side_effect=record_check):
                with mock.patch.object(gate, "run", side_effect=record_run):
                    gate.prepare_guest(context, gate.Boot(Path(base) / "b"), "secure", path, NONCE)
        self.assertEqual(order[0], "digest")
        self.assertEqual(order[-2:], ["digest", "qemu-img"])
        self.assertEqual(order.count("digest"), 2)


class TamperedCaseTests(unittest.TestCase):
    """The tampered case fails when the boot entry wrote or started anything for the copy."""

    def run_case(self, boot_entry):
        printed = io.StringIO()
        with tempfile.TemporaryDirectory() as base:
            artifact = Path(base) / PIN["file"]
            artifact.write_bytes(b"x" * 64)
            context = gate.Context(pin_copy(size=64), Path(base), Path(base) / "run")
            context.artifact = artifact
            refused = gate.Refused("digest", "hashes to something else")
            with contextlib.ExitStack() as stack:
                stack.enter_context(mock.patch.object(gate, "verify_artifact", side_effect=refused))
                stack.enter_context(mock.patch.object(gate, "boot_guest", side_effect=boot_entry))
                stack.enter_context(redirect_stdout(printed))
                problems = gate.tampered_case(context)
        return problems, printed.getvalue()

    def test_a_boot_entry_that_refuses_first_passes(self):
        def refuse(_context, boot, _profile, _artifact):
            raise gate.Refused("digest", "hashes to something else")

        problems, text = self.run_case(refuse)
        self.assertEqual(problems, [])
        self.assertIn("wrote for the tampered copy: none", text)

    def test_a_boot_entry_that_writes_before_refusing_fails(self):
        def write_then_refuse(_context, boot, _profile, _artifact):
            (boot.dir / "seed").mkdir()
            raise gate.Refused("digest", "hashes to something else")

        problems, text = self.run_case(write_then_refuse)
        self.assertTrue(problems)
        self.assertIn("FAIL boot/artifact-tampered-refused", text)


class TimeoutNegativeTests(unittest.TestCase):
    """Criterion 5: the short-timeout miss must be the deadline, not a QEMU that died."""

    def observed(self, **changes):
        observed = {"outcome": "missed", "timeout": 5, "seen": None, "exit_status": -9}
        observed.update(changes)
        return observed

    def test_a_deadline_that_expired_with_qemu_running_passes(self):
        self.assertEqual(gate.timeout_negative_problems(self.observed()), [])

    def test_a_qemu_that_exited_on_its_own_fails(self):
        problems = gate.timeout_negative_problems(self.observed(seen="exited", exit_status=1))
        self.assertEqual(len(problems), 2, problems)
        self.assertIn("'exited'", problems[0])

    def test_a_prompt_within_the_short_timeout_fails(self):
        observed = self.observed(outcome="reached", seen="prompt", exit_status=0)
        self.assertEqual(len(gate.timeout_negative_problems(observed)), 3)

    def test_an_exit_status_that_is_not_the_harness_signal_fails(self):
        """Boundary: status 0 and an unread status are not the harness's kill."""
        for status in (0, None):
            with self.subTest(status=status):
                problems = gate.timeout_negative_problems(self.observed(exit_status=status))
                self.assertEqual(len(problems), 1)

    def test_the_case_prints_what_the_poll_saw(self):
        printed = io.StringIO()
        with tempfile.TemporaryDirectory() as base:
            context = gate.Context(PIN, Path(base), Path(base) / "run")
            exited = self.observed(seen="exited", exit_status=1)
            with mock.patch.object(gate, "boot_guest", return_value=exited):
                with redirect_stdout(printed):
                    problems = gate.timeout_negative_case(context)
        self.assertTrue(problems)
        self.assertIn("seen by the deadline: exited", printed.getvalue())
        self.assertNotIn("stopped by the harness", printed.getvalue())


class GuestReportTests(unittest.TestCase):
    """What the gate reads out of the second serial line, and what it refuses."""

    def test_fields_are_read_between_the_markers_through_control_codes(self):
        fields = gate.guest_fields(guest_report())
        self.assertEqual(fields["NONCE"], NONCE)
        self.assertEqual(fields["PCR-0"], EXTENDED)
        self.assertNotIn("BdsDxe:", " ".join(fields))

    def test_a_report_without_its_end_marker_yields_nothing(self):
        text = guest_report().replace(f"{gate.GUEST_MARK}-END", "")
        self.assertEqual(gate.guest_fields(text), {})

    def test_a_pcr_reading_is_annotated(self):
        fields = gate.guest_fields(guest_report())
        self.assertEqual(gate.pcr_reading(fields, 0), (EXTENDED.lower(), "extended"))
        self.assertEqual(gate.pcr_reading(fields, 11), ("0" * 64, "reset (never extended)"))
        self.assertEqual(gate.pcr_reading({"PCR-4": "absent"}, 4)[1], "unreadable")
        self.assertEqual(gate.pcr_reading({"PCR-4": "ab" * 31}, 4)[1], "unreadable")

    def test_an_efi_variable_reads_as_its_value_byte(self):
        fields = gate.guest_fields(guest_report())
        self.assertEqual(gate.efi_state(fields, "SECUREBOOT"), "1")
        self.assertEqual(gate.efi_state({"SECUREBOOT": "absent"}, "SECUREBOOT"), "absent")
        self.assertEqual(gate.efi_state({}, "SETUPMODE"), "absent")

    def test_the_login_prompt_is_found_and_the_target_name_is_not(self):
        console = "\x1b[0;32m  OK  \x1b[0m] Reached target getty.target - Login Prompts.\r\n"
        self.assertIsNone(gate.LOGIN_PROMPT.search(gate.clean(console)))
        console += "\x1b]3008;start=1\x1b\\Fedora Linux 44\r\n\r\naegis-m24-guest login: "
        self.assertIsNotNone(gate.LOGIN_PROMPT.search(gate.clean(console)))

    def test_a_clean_kvm_boot_has_no_problems(self):
        self.assertEqual(gate.boot_problems(observed_boot()), [])

    def test_each_departure_from_a_clean_boot_is_named(self):
        for changes, fragment in (
            ({"outcome": "missed", "seen": "exited"}, "login prompt was missed"),
            ({"nonce": "aegis-m24-" + "cd" * 16}, "nonce"),
            ({"fields": gate.guest_fields(guest_report(pcr0="0" * 64))}, "PCR 0"),
            ({"exit_status": -9}, "power off"),
            ({"powerdown": False}, "power off"),
            ({"kvm": {"enabled": False, "present": True}}, "KVM"),
            ({"kvm": None}, "KVM"),
        ):
            with self.subTest(changes=changes):
                problems = gate.boot_problems(observed_boot(**changes))
                self.assertTrue(any(fragment in line for line in problems), problems)

    def test_the_printed_notes_carry_the_pcrs_and_the_decision(self):
        observed = observed_boot(fields=gate.guest_fields(guest_report()))
        notes = "\n".join(gate.boot_notes(observed))
        self.assertIn("15.0 s against the recorded 180 s -> reached", notes)
        self.assertIn("PCR 11 sha256 " + "0" * 64 + " (reset (never extended))", notes)


class GuestScriptTests(unittest.TestCase):
    """The user-data the seed carries: one nonce, read-only probes, no power control."""

    def test_the_nonce_replaces_the_one_placeholder(self):
        script = gate.user_data(NONCE)
        self.assertIn(f"NONCE='{NONCE}'", script)
        self.assertNotIn(gate.NONCE_PLACEHOLDER, script)

    def test_a_nonce_the_gate_did_not_make_is_refused(self):
        for nonce in ("aegis-m24-x';reboot;'", "aegis-m24-" + "ab" * 15, ""):
            with self.subTest(nonce=nonce), self.assertRaises(gate.GateError):
                gate.user_data(nonce)

    def test_the_generated_nonce_has_the_accepted_shape(self):
        context = gate.Context(PIN, Path("cache"), Path("run"))
        self.assertIsNotNone(gate.NONCE_SHAPE.fullmatch(context.nonce()))
        self.assertNotEqual(context.nonce(), context.nonce())

    def test_the_script_reads_and_never_writes_the_guest_platform(self):
        text = gate.REPORT_SCRIPT.read_text(encoding="utf-8")
        self.assertEqual(text.count(gate.NONCE_PLACEHOLDER), 1)
        self.assertIn(f"MARK='{gate.GUEST_MARK}'", text)
        self.assertIn("/sys/class/tpm/tpm0/pcr-sha256", text)
        self.assertIn("REPORT=/dev/ttyS1", text)
        for index in gate.PCR_INDICES:
            self.assertIn(str(index), text.split("for index in")[1].splitlines()[0])
        for forbidden in ("poweroff", "reboot", "tpm2_", "mokutil", "chattr", "efivar ", "dd "):
            self.assertNotIn(forbidden, text, f"the guest script names {forbidden}")

    def test_the_seed_names_the_instance_by_its_nonce(self):
        with tempfile.TemporaryDirectory() as base:
            boot = gate.Boot(Path(base) / "boot")
            with mock.patch.object(gate, "run", return_value=(0, "", "")) as run:
                gate.write_seed(boot, NONCE)
            meta = (boot.dir / "seed" / "meta-data").read_text(encoding="utf-8")
        self.assertIn(f"instance-id: {NONCE}", meta)
        argv = run.call_args.args[0]
        self.assertEqual(argv[:3], ["xorriso", "-as", "mkisofs"])
        self.assertEqual(argv[argv.index("-volid") + 1], "cidata")


class ArgumentVectorTests(unittest.TestCase):
    """KVM with no fallback, the firmware profiles, and what the guest is handed."""

    def boot(self):
        """Return a Boot under a directory removed when the test ends."""
        base = tempfile.TemporaryDirectory()
        self.addCleanup(base.cleanup)
        return gate.Boot(Path(base.name) / "b")

    def test_every_guest_requires_kvm_and_none_can_fall_back(self):
        for profile in gate.FIRMWARE_PROFILES:
            argv = gate.machine_argv(profile, self.boot())
            machine = argv[argv.index("-machine") + 1]
            self.assertIn("accel=kvm", machine)
            self.assertNotIn("tcg", machine)
        self.assertNotIn("tcg", GATE.read_text(encoding="utf-8"))

    def test_secure_boot_uses_the_smm_build_and_a_secure_flash(self):
        secure = gate.machine_argv("secure", self.boot())
        plain = gate.machine_argv("plain", self.boot())
        self.assertIn("q35,smm=on,accel=kvm", secure)
        self.assertIn("driver=cfi.pflash01,property=secure,value=on", secure)
        self.assertTrue(any("OVMF_CODE.secboot.4m.fd" in part for part in secure))
        self.assertIn("q35,smm=off,accel=kvm", plain)
        self.assertFalse(any("secure,value=on" in part for part in plain))
        self.assertTrue(any(part.endswith("OVMF_CODE.4m.fd") for part in plain))

    def test_the_guest_sees_swtpm_the_overlay_and_a_read_only_seed(self):
        argv = " ".join(gate.guest_argv("secure", self.boot()))
        self.assertIn("-device tpm-crb,tpmdev=tpm0", argv)
        self.assertIn("format=qcow2,file=", argv)
        self.assertIn("overlay.qcow2", argv)
        self.assertIn("readonly=on,file=", argv)
        self.assertNotIn(PIN["file"], argv, "the guest must boot the overlay, not the pin")
        self.assertIn("-no-reboot", argv)

    def test_qemu_dies_with_the_harness_even_when_it_is_killed(self):
        for profile in gate.FIRMWARE_PROFILES:
            argv = gate.machine_argv(profile, self.boot())
            self.assertEqual(argv[argv.index("-run-with") + 1], "exit-with-parent=on")

    def test_gpg_never_starts_an_agent(self):
        with mock.patch.object(gate, "run", return_value=(0, "", "")) as run:
            with tempfile.TemporaryDirectory() as base:
                gate.import_key(Path(base) / "gnupg", Path(base) / "keys.gpg", "F" * 40)
        self.assertEqual(run.call_count, 2)
        for call in run.call_args_list:
            self.assertEqual(call.args[0][0], "gpg")
            self.assertIn("--no-autostart", call.args[0])
        self.assertIn("--no-autostart", gate.gpg_argv(Path("keyring")))

    def test_swtpm_ends_with_its_client(self):
        argv = gate.swtpm_argv(self.boot())
        self.assertEqual(argv[:3], ["swtpm", "socket", "--tpm2"])
        self.assertIn("--terminate", argv)

    def test_an_option_string_path_is_refused_where_qemu_would_split_it(self):
        with self.assertRaises(gate.GateError):
            gate.option_path(Path("/cache/a,b"))
        with self.assertRaises(gate.GateError):
            gate.socket_path(Path("/" + "s" * gate.MAX_SOCKET_PATH))
        self.assertEqual(gate.option_path(Path("/cache/run/x.fd")), "/cache/run/x.fd")

    def test_the_three_stores_differ_only_where_they_must(self):
        context = gate.Context(PIN, Path("cache"), Path("run"))
        context.key = {"key": Path("k.key"), "cert": Path("k.crt"), "owner": "o", "subject": "s"}
        secure = gate.store_options(context, "secure")
        custom = gate.store_options(context, "custom-only")
        plain = gate.store_options(context, "plain")
        self.assertIn("--secure-boot", secure)
        self.assertEqual(secure[secure.index("--microsoft-kek") + 1], "none")
        self.assertEqual(secure[secure.index("--microsoft-db") + 1], "uefi11")
        self.assertIn("--set-fallback-no-reboot", secure)
        self.assertEqual(custom.count("k.crt"), 3)
        self.assertFalse(any("microsoft" in option for option in custom))
        self.assertNotIn("--secure-boot", plain)


class FirmwareSigningTests(unittest.TestCase):
    """The UKI extraction reads a GPT, and the firmware's answer is read off its console."""

    def test_the_esp_extent_is_read_from_the_gpt(self):
        root = uuid.UUID("4f68bce3-e8cd-4db1-96e7-fbcaf984b709")
        image = gpt_image([(root, 9000, 9999), (gate.ESP_TYPE, 6144, 2054143)])
        self.assertEqual(gate.esp_extent(image), (6144, 2048000))

    def test_a_disk_without_a_gpt_or_an_esp_is_a_gate_error(self):
        with self.assertRaises(gate.GateError):
            gate.esp_extent(bytes(34 * 512))
        with self.assertRaises(gate.GateError):
            gate.esp_extent(gpt_image([]))

    def test_a_gpt_declaring_too_many_entries_is_refused(self):
        image = bytearray(gpt_image([(gate.ESP_TYPE, 2048, 4095)]))
        struct.pack_into("<QII", image, 584, 2, gate.MAX_GPT_ENTRIES + 1, 128)
        with self.assertRaises(gate.GateError):
            gate.esp_extent(bytes(image))

    def verdict(self, console_text, returncode=None):
        with tempfile.TemporaryDirectory() as base:
            console = Path(base) / "console.log"
            console.write_bytes(console_text.encode())
            process = mock.Mock(returncode=returncode)
            process.poll.return_value = returncode
            return gate.firmware_verdict(process, console)

    def test_the_firmware_answer_is_read_from_the_console(self):
        started = "[    0.000000] secureboot: Secure boot enabled\r\n"
        denied = "BdsDxe: failed to load Boot0002: Access Denied -- rejected by Secure Boot\r\n"
        self.assertTrue(self.verdict(started).startswith("kernel started"))
        self.assertTrue(self.verdict(denied).startswith("firmware refused"))
        self.assertIsNone(self.verdict("BdsDxe: loading Boot0002\r\n"))
        self.assertEqual(self.verdict("", returncode=1), "QEMU exited 1")

    def test_a_qmp_reply_skips_events_and_a_silent_monitor_is_an_error(self):
        stream = io.BytesIO(b'{"event": "POWERDOWN"}\n{"return": {"enabled": true}}\n')
        self.assertEqual(gate.qmp_reply(stream), {"return": {"enabled": True}})
        with self.assertRaises(gate.GateError):
            gate.qmp_reply(io.BytesIO(b""))
        with self.assertRaises(gate.GateError):
            gate.qmp_reply(io.BytesIO(b'{"event": "X"}\n' * (gate.MAX_QMP_LINES + 1)))


class HostStateTests(unittest.TestCase):
    """The host's Secure Boot state is read and recorded, never written."""

    def efivars(self, base, values):
        directory = Path(base) / "efivars"
        directory.mkdir()
        for name, data in values.items():
            (directory / f"{name}-{gate.EFI_GLOBAL}").write_bytes(bytes(data))
        return directory

    def run_case(self, directory):
        printed = io.StringIO()
        with mock.patch.object(gate, "EFIVARS", directory), redirect_stdout(printed):
            problems = gate.host_secure_boot_case()
        return problems, printed.getvalue()

    def test_the_reference_host_reading_is_recorded(self):
        with tempfile.TemporaryDirectory() as base:
            directory = self.efivars(
                base, {"SecureBoot": [6, 0, 0, 0, 0], "SetupMode": [6, 0, 0, 0, 1]}
            )
            problems, text = self.run_case(directory)
        self.assertEqual(problems, [])
        self.assertIn("efivarfs SecureBoot: 6 0 0 0 0", text)
        self.assertIn("none of PK, KEK, db, dbx", text)
        self.assertIn("host Secure Boot is disabled", text)

    def test_an_enabled_host_is_reported_differently(self):
        values = {"SecureBoot": [6, 0, 0, 0, 1], "SetupMode": [6, 0, 0, 0, 0], "PK": [7]}
        with tempfile.TemporaryDirectory() as base:
            problems, text = self.run_case(self.efivars(base, values))
        self.assertEqual(problems, [])
        self.assertIn("not reported disabled", text)
        self.assertIn("host key variables present: PK", text)

    def test_an_unreadable_variable_fails_the_case(self):
        with tempfile.TemporaryDirectory() as base:
            problems, _text = self.run_case(self.efivars(base, {"SetupMode": [6, 0, 0, 0, 1]}))
        self.assertTrue(problems)

    def test_a_host_without_efivarfs_skips_with_a_reason(self):
        with tempfile.TemporaryDirectory() as base:
            problems, text = self.run_case(Path(base) / "absent")
        self.assertEqual(problems, [])
        self.assertIn("SKIP boot/host-secure-boot", text)

    def test_no_kvm_is_a_stated_skip_with_no_fallback(self):
        with mock.patch.object(gate, "KVM_DEVICE", Path(tempfile.gettempdir()) / "no-kvm-here"):
            reasons = gate.check_kvm()
        self.assertEqual(len(reasons), 1)
        self.assertIn("no TCG fallback", reasons[0])


class SkipAndRetentionTests(unittest.TestCase):
    """What the gate says when it cannot run, and what a run leaves behind."""

    def test_an_empty_cache_names_the_fetch_command(self):
        with tempfile.TemporaryDirectory() as base:
            context = gate.Context(PIN, Path(base), Path(base) / "run")
            reasons = gate.check_cache(context)
        self.assertEqual(len(reasons), 1)
        self.assertIn("--fetch", reasons[0])

    def test_an_inadmissible_host_prints_skip_and_exits_zero(self):
        printed = io.StringIO()
        with tempfile.TemporaryDirectory() as base:
            with contextlib.ExitStack() as stack:
                stack.enter_context(
                    mock.patch.dict(gate.os.environ, {"AEGIS_BOOT_HARNESS_DIR": base})
                )
                stack.enter_context(mock.patch.object(gate, "check_toolchain", return_value=["x"]))
                stack.enter_context(mock.patch.object(gate, "check_firmware", return_value=[]))
                stack.enter_context(mock.patch.object(gate, "check_kvm", return_value=[]))
                stack.enter_context(mock.patch.object(gate.signal, "signal"))
                stack.enter_context(redirect_stdout(printed))
                code = gate.main([])
        self.assertEqual(code, 0)
        self.assertIn("SKIP: x; the boot harness gate did not run.", printed.getvalue())
        self.assertNotIn("PASS", printed.getvalue())

    def test_an_artifact_that_changes_mid_run_is_a_failure_and_not_a_crash(self):
        printed = io.StringIO()
        refusal = gate.Refused("digest", "hashes to something else")
        with tempfile.TemporaryDirectory() as base:
            with contextlib.ExitStack() as stack:
                stack.enter_context(
                    mock.patch.dict(gate.os.environ, {"AEGIS_BOOT_HARNESS_DIR": base})
                )
                stack.enter_context(mock.patch.object(gate, "preflight", return_value=[]))
                stack.enter_context(mock.patch.object(gate, "run_cases", side_effect=refusal))
                stack.enter_context(mock.patch.object(gate.signal, "signal"))
                stack.enter_context(redirect_stdout(printed))
                code = gate.main([])
        self.assertEqual(code, 1)
        self.assertIn("FAIL: the pinned artifact was refused at digest", printed.getvalue())

    def test_every_terminating_signal_this_host_has_ends_through_finally(self):
        expected = [name for name in gate.TERMINATING_SIGNALS if hasattr(gate.signal, name)]
        self.assertIn("SIGTERM", expected)
        with mock.patch.object(gate.signal, "signal") as installed:
            self.assertEqual(gate.install_signal_handlers(), expected)
        numbers = [call.args[0] for call in installed.call_args_list]
        self.assertEqual(numbers, [getattr(gate.signal, name) for name in expected])
        for call in installed.call_args_list:
            self.assertIs(call.args[1], gate.terminated)
        with self.assertRaises(SystemExit) as ended:
            gate.terminated(1, None)
        self.assertEqual(ended.exception.code, 129)

    def test_main_installs_the_handlers(self):
        with contextlib.ExitStack() as stack:
            stack.enter_context(
                mock.patch.object(gate, "load_pin", side_effect=gate.GateError("x"))
            )
            handlers = stack.enter_context(mock.patch.object(gate, "install_signal_handlers"))
            stack.enter_context(redirect_stdout(io.StringIO()))
            self.assertEqual(gate.main([]), 1)
        handlers.assert_called_once_with()

    def test_the_measure_count_is_bounded_at_both_ends(self):
        """Boundary: 1 and the cap are accepted; 0, -1, one past the cap and text are refused."""
        self.assertIsNone(gate.parse_arguments([]).measure)
        for accepted in (1, gate.MAX_MEASURED_BOOTS):
            self.assertEqual(gate.parse_arguments(["--measure", str(accepted)]).measure, accepted)
        for refused in ("0", "-1", str(gate.MAX_MEASURED_BOOTS + 1), "many"):
            with self.subTest(refused=refused), redirect_stderr(io.StringIO()):
                with self.assertRaises(SystemExit):
                    gate.parse_arguments(["--measure", refused])

    def test_a_measure_that_booted_nothing_fails_without_a_traceback(self):
        context = gate.Context(PIN, Path("cache"), Path("run"))
        printed = io.StringIO()
        with mock.patch.object(gate, "generate_key", return_value={}), redirect_stdout(printed):
            self.assertEqual(gate.measure(context, 0), 1)
        self.assertIn("booted nothing", printed.getvalue())

    @unittest.skipUnless(
        getattr(socket, "AF_UNIX", None),
        "AF_UNIX is absent on this platform; the Linux runner covers socket cleanup",
    )
    def test_a_left_behind_monitor_socket_is_removed(self):
        with tempfile.TemporaryDirectory() as base:
            case = Path(base) / "run" / "timeout"
            case.mkdir(parents=True)
            with socket.socket(getattr(socket, "AF_UNIX")) as listener:
                listener.bind(str(case / "qmp.sock"))
            (case / "console.log").write_text("x", encoding="utf-8")
            self.assertFalse((case / "qmp.sock").is_file())
            gate.discard_bulk(Path(base) / "run")
            left = sorted(path.name for path in case.iterdir())
        self.assertEqual(left, ["console.log"])

    def test_bulky_files_go_and_the_evidence_stays(self):
        with tempfile.TemporaryDirectory() as base:
            run = Path(base) / "run"
            (run / "positive" / "tpm").mkdir(parents=True)
            for name in ("overlay.qcow2", "vars.fd", "seed.iso", "console.log", "result.json"):
                (run / "positive" / name).write_text("x", encoding="utf-8")
            (run / "positive" / "tpm" / "tpm2-00.permall").write_text("x", encoding="utf-8")
            gate.discard_bulk(run)
            left = sorted(path.name for path in (run / "positive").iterdir())
        self.assertEqual(left, ["console.log", "result.json"])

    def test_only_the_newest_runs_are_kept(self):
        with tempfile.TemporaryDirectory() as base:
            for index in range(gate.MAX_RETAINED_RUNS + 3):
                (Path(base) / "runs" / f"r2026092{index:02d}").mkdir(parents=True)
            gate.prune_runs(Path(base))
            kept = sorted(path.name for path in (Path(base) / "runs").iterdir())
        self.assertEqual(len(kept), gate.MAX_RETAINED_RUNS)
        self.assertEqual(kept[-1], f"r2026092{gate.MAX_RETAINED_RUNS + 2:02d}")


class RepositoryTests(unittest.TestCase):
    """The target sits outside verify-all, and the evidence page records the pin."""

    def test_verify_boot_is_its_own_target_and_not_part_of_verify_all(self):
        text = MAKEFILE.read_text(encoding="utf-8")
        self.assertIn("verify-boot:\n\tpython3 tools/verify_boot_harness.py", text)
        recipe = text.split("verify-all:\n", 1)[1].split("\n\n", 1)[0]
        self.assertNotIn("verify-boot", recipe)
        self.assertNotIn("verify_boot_harness", recipe)

    def test_the_evidence_page_records_the_pin_the_timeout_and_the_scope(self):
        text = EVIDENCE_PAGE.read_text(encoding="utf-8")
        self.assertIn(PIN["sha256"], text)
        self.assertIn(f"{PIN['boot']['login-prompt-timeout-seconds']} s", text)
        self.assertIn("not evidence for M11", text)
        self.assertIn("D84", text)


class HostSurfaceTests(unittest.TestCase):
    """What the gate can start, and that it cannot touch the host's firmware."""

    def test_the_gate_starts_only_allowed_programs(self):
        resolved, _undecided = call_sites(GATE)
        self.assertTrue(resolved, "the sweep found no call site; it is reading nothing")
        self.assertLessEqual(resolved, ALLOWED_PROGRAMS, "the gate starts an unlisted program")

    def test_the_undecidable_call_sites_are_the_ones_that_have_to_be(self):
        """`run` and `session` start what they are handed; `read_version` runs one table row."""
        _resolved, undecided = call_sites(GATE)
        self.assertEqual(sorted(undecided), ["read_version", "run", "session"])
        rows = {row[1][0] for row in gate.TOOLCHAIN}
        self.assertLessEqual(rows, ALLOWED_PROGRAMS)

    def test_the_allowlist_has_no_entry_nothing_runs(self):
        resolved, _undecided = call_sites(GATE)
        self.assertEqual(resolved | {row[1][0] for row in gate.TOOLCHAIN}, ALLOWED_PROGRAMS)
        self.assertEqual(resolved, ALLOWED_PROGRAMS, "an admitted tool is never actually run")

    def test_the_sweep_follows_helpers_and_would_notice_an_unlisted_program(self):
        planted = ast.parse(
            "def helper():\n    return ['bootctl', 'status']\n"
            "def f():\n    argv = helper()\n    run(argv + ['--x'], 10)\n"
        )
        functions = function_index(planted)
        call = [n for n in ast.walk(functions["f"]) if isinstance(n, ast.Call)][-1]
        self.assertEqual(argv_head(call.args[0], functions["f"], functions), "bootctl")
        self.assertNotIn("bootctl", ALLOWED_PROGRAMS)

    def test_the_gate_names_no_firmware_writer_and_no_imported_script(self):
        source = GATE.read_text(encoding="utf-8")
        for forbidden in FORBIDDEN_NAMES + FORBIDDEN_SOURCES:
            self.assertNotIn(forbidden, source, f"the gate names {forbidden}")
        self.assertFalse(ALLOWED_PROGRAMS & {"sh", "bash", "dash"}, "a script runner is allowed")

    def test_host_efi_variables_are_only_ever_opened_for_reading(self):
        function = function_index(ast.parse(GATE.read_text(encoding="utf-8")))["host_efi_value"]
        modes = [
            call.args[1].value
            for call in ast.walk(function)
            if isinstance(call, ast.Call) and getattr(call.func, "id", None) == "open"
        ]
        self.assertEqual(modes, ["rb"])


class SourceSweeps(unittest.TestCase):
    """HISS-02 (deadlines), HISS-04 (size and complexity) and HISS-08 over the gate."""

    tree = ast.parse(GATE.read_text(encoding="utf-8"))

    def calls(self, name):
        return [
            node
            for node in ast.walk(self.tree)
            if isinstance(node, ast.Call)
            and (getattr(node.func, "id", None) or getattr(node.func, "attr", None)) == name
        ]

    def test_every_run_call_carries_a_deadline(self):
        runs = [node for node in self.calls("run") if isinstance(node.func, ast.Name)]
        self.assertGreater(len(runs), 10, "the sweep found too few call sites")
        for node in runs:
            self.assertGreaterEqual(len(node.args), 2, f"run() without a deadline at {node.lineno}")

    def test_only_run_and_session_start_a_process(self):
        starters = []
        for function in function_index(self.tree).values():
            for node in ast.walk(function):
                if (
                    isinstance(node, ast.Attribute)
                    and getattr(node.value, "id", "") == "subprocess"
                ):
                    if node.attr in {"Popen", "run", "call", "check_output", "check_call"}:
                        starters.append((function.name, node.attr))
        self.assertEqual(sorted(starters), [("run", "Popen"), ("session", "Popen")])

    def test_every_wait_on_a_process_is_bounded(self):
        waits = self.calls("wait") + self.calls("communicate")
        self.assertTrue(waits)
        for node in waits:
            keywords = {keyword.arg for keyword in node.keywords}
            self.assertIn("timeout", keywords, f"an unbounded wait at line {node.lineno}")

    def test_a_started_session_is_ended_on_every_path(self):
        functions = function_index(self.tree)
        for name in ("session", "run"):
            stops = [node for node in ast.walk(functions[name]) if isinstance(node, ast.Call)]
            self.assertTrue(any(getattr(n.func, "id", "") == "stop" for n in stops), name)
        finals = [node for node in ast.walk(functions["session"]) if isinstance(node, ast.Try)]
        self.assertTrue(any(node.finalbody for node in finals), "session() has no finally")
        stop_calls = [n for n in ast.walk(functions["stop"]) if isinstance(n, ast.Call)]
        self.assertTrue(any(getattr(n.func, "id", "") == "end_session" for n in stop_calls))

    def test_the_run_key_exists_before_the_first_case_that_writes_a_store(self):
        """A regression the first real run found: the tampered boot entry needs a store key."""
        function = function_index(self.tree)["run_cases"]
        lines = {}
        for node in ast.walk(function):
            if isinstance(node, ast.Assign) and any(
                getattr(target, "attr", "") == "key" for target in node.targets
            ):
                lines.setdefault("key", node.lineno)
            if isinstance(node, ast.Call) and getattr(node.func, "id", "") == "tampered_case":
                lines.setdefault("tampered", node.lineno)
        self.assertLess(lines["key"], lines["tampered"])

    def test_the_monitor_socket_carries_a_timeout(self):
        self.assertTrue(self.calls("settimeout"))

    def test_no_function_exceeds_the_length_or_complexity_limit(self):
        offenders = []
        for node in function_index(self.tree).values():
            length = node.end_lineno - node.lineno + 1
            statements = sum(1 for inner in ast.walk(node) if isinstance(inner, ast.stmt)) - 1
            if length > MAX_FUNCTION_LINES or complexity(node) > MAX_COMPLEXITY:
                offenders.append(f"{node.name}: {length} lines, complexity {complexity(node)}")
            elif statements > MAX_STATEMENTS:
                offenders.append(f"{node.name}: {statements} statements")
        self.assertEqual(offenders, [])

    def test_the_complexity_count_sees_branches(self):
        planted = ast.parse(
            "def f(a, b):\n    if a and b:\n        return 1\n    for x in a:\n        pass\n"
        )
        self.assertEqual(complexity(planted.body[0]), 4)

    def test_the_gate_never_evaluates_code(self):
        names = {node.id for node in ast.walk(self.tree) if isinstance(node, ast.Name)}
        self.assertFalse(names & {"eval", "exec", "compile"})


if __name__ == "__main__":
    unittest.main()
