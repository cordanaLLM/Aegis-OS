"""Regressions for the workstation slices gate (M21): the half that needs no hardware.

`make verify-workstation` needs the reference profile's powercap counter, a
passwordless `sudo -n`, a read-write /dev/kvm and the fetched Firecracker.
These tests need none of them. They hold the gate's own decisions -- the pin,
the one sudo command and the program list, when each half skips and what it
prints then, the recorded zone enumeration, the static-init check, the
initramfs writer, the leftover sweep and the report parser -- so a checkout
without the hardware still fails `make verify-all` when one of those drifts.
Every test runs on Linux, macOS and Windows (HISS-21); the few that need a
POSIX facility say so and skip.
"""

import ast
import hashlib
import io
import json
import os
import re
import socket
import tarfile
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import verify_workstation as gate

ROOT = Path(__file__).resolve().parent.parent
GATE = ROOT / "tools" / "verify_workstation.py"
MAKEFILE = ROOT / "Makefile"
PAGE = ROOT / "docs" / "build" / "workstation.md"
RAPL_CASES_SOURCE = ROOT / "crates" / "aegis-tellus-rapl" / "src" / "cases.rs"
HOST_CASES_SOURCE = ROOT / "crates" / "aegis-vesta-sandbox" / "src" / "run.rs"

# The pins, held here independently of build/sandbox/firecracker.pin.json so a
# change to either is caught.
ARCHIVE_SHA256 = "06094a1108ae9e82aa4c23a775aa92758f53f1175d422270d9d6162cb9ade558"
BINARY_SHA256 = "99ad0f5cd0514a88aad0e9ae8cfdb3cc3b4ab9d190e1194602406c786b5de7a5"
LICENCE_SHA256 = "cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30"
KERNEL_SHA256 = "d8ced68bd61e27b6813e2c993cc53a4029c59e13210672180591c84109684fe4"
CONFIG_SHA256 = "9fb2be18303d2f6e8ec35b3a20ecf1209f54a4ece10114a893cb37beabee7030"
SUDO = ("sudo", "-n", "cat", "/sys/class/powercap/intel-rapl:0/energy_uj")


def zone_directory(test, path):
    """Create a powercap zone directory, or skip where the host refuses its name.

    powercap names zones `intel-rapl:0`; Windows refuses a ':' in a file name,
    so there the case is covered on the Linux leg of the matrix (HISS-21).
    """
    try:
        path.mkdir(exist_ok=True)
    except OSError as error:
        test.skipTest(f"this host cannot create {path.name!r} ({error}); Linux runs it")
    return path


def elf(headers):
    """A minimal ELF64 little-endian image whose program headers have `headers` types."""
    header = bytearray(64)
    header[0:4] = b"\x7fELF"
    header[4], header[5] = 2, 1
    header[32:40] = (64).to_bytes(8, "little")
    header[54:56] = (56).to_bytes(2, "little")
    header[56:58] = len(headers).to_bytes(2, "little")
    table = b"".join(kind.to_bytes(4, "little") + bytes(52) for kind in headers)
    return bytes(header) + table


def members(archive):
    """Parse a newc archive back into (name, mode, data) triples."""
    found, offset = [], 0
    for _ in range(64):
        fields = [int(archive[offset + 6 + 8 * i : offset + 14 + 8 * i], 16) for i in range(13)]
        name_size, size, mode = fields[11], fields[6], fields[1]
        start = offset + 110
        name = archive[start : start + name_size - 1].decode()
        data_start = start + name_size + (-(110 + name_size) % 4)
        found.append((name, mode, archive[data_start : data_start + size]))
        offset = data_start + size + (-size % 4)
        if name == "TRAILER!!!":
            break
    return found


class PinTests(unittest.TestCase):
    """The committed pin is the one this test holds, and malformed pins are refused."""

    def test_the_pin_names_the_admitted_release(self):
        """Positive: Firecracker 1.17.0, its digests, no jailer, and the guest kernel."""
        pin = gate.load_pin()
        firecracker, kernel = pin["firecracker"], pin["guest_kernel"]
        self.assertEqual(firecracker["version"], "1.17.0")
        self.assertEqual(firecracker["licence"], "Apache-2.0")
        self.assertEqual(firecracker["jailer"], "not used")
        self.assertEqual(firecracker["archive"]["sha256"], ARCHIVE_SHA256)
        self.assertEqual(firecracker["binary"]["sha256"], BINARY_SHA256)
        self.assertEqual(firecracker["licence_file"]["sha256"], LICENCE_SHA256)
        self.assertEqual(kernel["sha256"], KERNEL_SHA256)
        self.assertEqual(kernel["config"]["sha256"], CONFIG_SHA256)
        self.assertIn("CONFIG_VIRTIO_VSOCKETS=y", kernel["required_config"])
        self.assertEqual(pin["guest_memory_mib"], 64)
        self.assertEqual(gate.FIRECRACKER_NAME, firecracker["binary"]["file"])

    def test_a_short_digest_is_refused(self):
        """Negative and boundary: 63 hex digits is refused, 64 is read."""
        pin = json.loads(gate.PIN_FILE.read_text(encoding="utf-8"))
        with tempfile.TemporaryDirectory() as scratch:
            path = Path(scratch) / "pin.json"
            path.write_text(json.dumps(pin), encoding="utf-8")
            self.assertEqual(gate.load_pin(path)["guest_kernel"]["sha256"], KERNEL_SHA256)
            pin["guest_kernel"]["sha256"] = KERNEL_SHA256[:63]
            path.write_text(json.dumps(pin), encoding="utf-8")
            with self.assertRaises(gate.GateError):
                gate.load_pin(path)


class ProgramTests(unittest.TestCase):
    """Only listed programs start; sudo runs exactly one read."""

    def test_the_one_read_is_admitted(self):
        """Positive: the scoped read passes the check."""
        self.assertEqual(gate.SUDO_READ, SUDO)
        gate.check_argv(list(SUDO))
        gate.check_argv(["cargo", "build"])

    def test_every_other_sudo_is_refused_before_it_starts(self):
        """Negative: another file, another command, no -n, or a path to sudo never reaches it."""
        for argv in (
            ["sudo", "-n", "cat", "/etc/shadow"],
            ["sudo", "cat", SUDO[3]],
            ["sudo", "-n", "true"],
            ["sudo", "-n", "cat", SUDO[3], "/etc/passwd"],
            ["/usr/bin/sudo", "-n", "sh", "-c", "id"],
            ["/usr/bin/sudo", "-n", "cat", "/etc/shadow"],
            ["/usr/bin/sudo", *SUDO[1:]],
            ["rm", "-rf", "/"],
            ["qemu-system-x86_64", "--version"],
        ):
            with mock.patch.object(gate.subprocess, "run") as started:
                with self.assertRaises(gate.GateError):
                    gate.run(argv, 1)
                started.assert_not_called()

    def test_sudo_is_named_only_by_the_scoped_read(self):
        """Boundary: "sudo" occurs in SUDO_READ, the program list, its check and a PATH probe."""
        tree = ast.parse(GATE.read_text(encoding="utf-8"))
        sites = [
            node
            for node in ast.walk(tree)
            if isinstance(node, ast.Constant) and node.value == "sudo"
        ]
        self.assertEqual(len(sites), 4)

    def test_every_process_carries_a_deadline(self):
        """Positive: two call sites, run() with timeout= and run_session() with a bound wait."""
        tree = ast.parse(GATE.read_text(encoding="utf-8"))
        sites = []
        for function in ast.walk(tree):
            if not isinstance(function, ast.FunctionDef):
                continue
            for node in ast.walk(function):
                if (
                    isinstance(node, ast.Call)
                    and isinstance(node.func, ast.Attribute)
                    and isinstance(node.func.value, ast.Name)
                    and node.func.value.id in {"subprocess", "os"}
                    and node.func.attr in {"run", "Popen", "system", "popen", "execv"}
                ):
                    sites.append((function.name, node.func.attr, {k.arg for k in node.keywords}))
        self.assertEqual(sorted(site[0] for site in sites), ["run", "run_session"])
        self.assertIn("timeout", dict((s[0], s[2]) for s in sites)["run"])
        self.assertIn("communicate(timeout=timeout)", GATE.read_text(encoding="utf-8"))


class SudoReadTests(unittest.TestCase):
    """The privileged read: its stamp, its skip and its failure."""

    def test_a_read_is_stamped_at_its_midpoint(self):
        """Positive: the midpoint of the call and its latency are recorded."""
        clock = iter([100, 140]).__next__
        runner = mock.Mock(return_value=(0, "123\n", ""))
        read = gate.sudo_read(runner=runner, clock=clock)
        self.assertEqual(read, {"text": "123\n", "monotonic-ns": 120, "latency-ns": 40})
        runner.assert_called_once_with(list(SUDO), gate.SUDO_TIMEOUT)

    def test_a_password_prompt_is_a_skip(self):
        """Negative: without passwordless sudo the half cannot run; that is a skip.

        Classic sudo says "a password is required"; sudo-rs, Ubuntu's default,
        says "interactive authentication is required" (its Error::InteractionRequired).
        """
        for message in (
            "sudo: a password is required\n",
            "sudo: interactive authentication is required\n",
        ):
            runner = mock.Mock(return_value=(1, "", message))
            with self.assertRaises(gate.Unavailable):
                gate.sudo_read(runner=runner, clock=iter([1, 2]).__next__)

    def test_any_other_failure_is_a_failure(self):
        """Boundary: a missing counter is not a missing capability."""
        runner = mock.Mock(return_value=(1, "", "cat: No such file or directory\n"))
        with self.assertRaises(gate.GateError):
            gate.sudo_read(runner=runner, clock=iter([1, 2]).__next__)


class SamplingTests(unittest.TestCase):
    """The pair, the wrap watch and the document they fill."""

    def reader(self, values):
        """A reader returning `values` in order, stamped one second apart."""
        stamps = iter(range(len(values)))
        texts = iter(values)
        return lambda: {"text": f"{next(texts)}\n", "monotonic-ns": next(stamps), "latency-ns": 1}

    def test_a_pair_is_two_reads_a_spacing_apart(self):
        """Positive: two reads, one sleep of the recorded spacing."""
        sleep = mock.Mock()
        readings = gate.sample_pair(self.reader([1, 2]), sleep)
        self.assertEqual([read["text"] for read in readings], ["1\n", "2\n"])
        sleep.assert_called_once_with(gate.PAIR_SPACING)

    def test_a_watch_stops_one_read_after_the_wrap(self):
        """Boundary: the read after the wrap is kept, then the watch ends."""
        readings = gate.watch_wrap(self.reader([5, 6, 7, 1, 2, 3, 4]), mock.Mock(), 10.0, 100.0)
        self.assertEqual([read["text"] for read in readings], ["5\n", "6\n", "7\n", "1\n", "2\n"])

    def test_a_watch_without_a_wrap_ends_at_its_bound(self):
        """Negative: no wrap means the bound, not an endless watch."""
        readings = gate.watch_wrap(self.reader(list(range(1, 100))), mock.Mock(), 10.0, 30.0)
        self.assertEqual(len(readings), 5)
        self.assertFalse(gate.wrapped(readings))

    def test_the_document_carries_only_what_the_reader_judges(self):
        """Positive: the latency stays with the gate; the schema is the crate's."""
        read = {"text": "1\n", "monotonic-ns": 1, "latency-ns": 9}
        document = gate.readings_document("package-0\n", "65532610987\n", "pair", [read], read)
        self.assertEqual(document["schema"], "aegis.m21.rapl-readings.v1")
        self.assertNotIn("latency-ns", document["readings"][0])
        self.assertEqual(document["unprivileged"], {"text": "1\n", "monotonic-ns": 1})


class RaplCapabilityTests(unittest.TestCase):
    """When the RAPL half skips, and why."""

    def powercap(self, scratch):
        """A powercap tree with the one counter the half reads."""
        zone = zone_directory(self, Path(scratch) / gate.RAPL_ZONE)
        (zone / gate.ENERGY_ATTRIBUTE).write_text("1\n", encoding="ascii")
        return Path(scratch)

    def reasons(self, scratch, **overrides):
        """The reasons for a host that has everything unless `overrides` says otherwise."""
        options = {
            "platform": "linux",
            "which": lambda name: f"/usr/bin/{name}",
            "powercap": self.powercap(scratch),
            "cpu": gate.REFERENCE_CPU,
            "uid": lambda: (1000, None),
        }
        options.update(overrides)
        return gate.rapl_reasons(**options)

    def test_the_reference_profile_has_no_reason(self):
        """Positive: every capability present."""
        with tempfile.TemporaryDirectory() as scratch:
            self.assertEqual(self.reasons(scratch), [])

    def test_each_missing_capability_is_named(self):
        """Negative: no sudo, root, another CPU, another platform."""
        with tempfile.TemporaryDirectory() as scratch:
            missing = self.reasons(scratch, which=lambda name: None)
            self.assertIn("sudo", missing[0])
            self.assertIn("cargo", missing[1])
            self.assertIn("root", self.reasons(scratch, uid=lambda: (0, None))[0])
            self.assertIn("Intel", self.reasons(scratch, cpu="Intel(R) Core(TM) i9")[0])
        for platform in ("darwin", "win32"):
            reasons = gate.rapl_reasons(platform=platform)
            self.assertEqual(len(reasons), 1)
            self.assertIn(platform, reasons[0])

    def test_a_missing_counter_is_named_as_a_linux_path(self):
        """Boundary: the path in the reason is the Linux one on every host (HISS-21)."""
        with tempfile.TemporaryDirectory() as scratch:
            empty = Path(scratch) / "powercap"
            empty.mkdir()
            reasons = self.reasons(scratch, powercap=empty)
        self.assertEqual(len(reasons), 1)
        self.assertNotIn("\\", reasons[0])
        self.assertTrue(reasons[0].endswith("intel-rapl:0/energy_uj"))


class EnumerationTests(unittest.TestCase):
    """The zone enumeration is compared with the recorded one, not assumed."""

    def entries(self):
        """The reference profile's enumeration, as the gate records it."""
        row = {"range": "65532610987", "mode": 0o400, "owner": 0}
        return [
            {"entry": "intel-rapl", "name": None, "range": None, "mode": None, "owner": None},
            dict(row, entry="intel-rapl:0", name="package-0"),
            dict(row, entry="intel-rapl:0:0", name="core"),
        ]

    def test_the_recorded_enumeration_has_no_problem(self):
        """Positive: three entries, two named zones, root-only counters."""
        self.assertEqual(gate.enumeration_problems(self.entries()), [])

    def test_a_dram_zone_or_a_readable_counter_is_a_problem(self):
        """Negative: another zone, a world-readable counter, another range."""
        extra = self.entries() + [
            {"entry": "intel-rapl:0:1", "name": "dram", "range": "1", "mode": 0o400, "owner": 0}
        ]
        self.assertTrue(gate.enumeration_problems(extra))
        readable = self.entries()
        readable[1]["mode"] = 0o444
        self.assertIn("not 0400 root", gate.enumeration_problems(readable)[0])
        ranged = self.entries()
        ranged[2]["range"] = "262143328850"
        self.assertIn("262143328850", gate.enumeration_problems(ranged)[0])

    def test_the_tree_is_read_as_found(self):
        """Boundary: names and ranges are read from the tree; the control type has none."""
        with tempfile.TemporaryDirectory() as scratch:
            root = Path(scratch)
            (root / "intel-rapl").mkdir()
            zone = zone_directory(self, root / "intel-rapl:0")
            (zone / "name").write_text("package-0\n", encoding="ascii")
            (zone / "max_energy_range_uj").write_text("65532610987\n", encoding="ascii")
            (zone / "energy_uj").write_text("1\n", encoding="ascii")
            entries = gate.enumerate_powercap(root)
        self.assertEqual([entry["entry"] for entry in entries], ["intel-rapl", "intel-rapl:0"])
        self.assertIsNone(entries[0]["name"])
        self.assertEqual(entries[1]["name"], "package-0")
        self.assertEqual(entries[1]["range"], "65532610987")

    def test_an_unprivileged_read_records_its_outcome(self):
        """Negative and positive: a missing file is an error; a readable one is text."""
        with tempfile.TemporaryDirectory() as scratch:
            missing = gate.unprivileged_read(Path(scratch) / "absent", clock=lambda: 7)
            present = Path(scratch) / "energy_uj"
            present.write_text("42\n", encoding="ascii")
            readable = gate.unprivileged_read(present, clock=lambda: 8)
        self.assertIn("error", missing)
        self.assertEqual(missing["monotonic-ns"], 7)
        self.assertEqual(readable, {"text": "42\n", "monotonic-ns": 8})


class StaticInitTests(unittest.TestCase):
    """The guest init must be static: no PT_INTERP program header."""

    def test_a_static_executable_is_admitted(self):
        """Positive: PT_LOAD and PT_DYNAMIC-free headers without PT_INTERP."""
        self.assertEqual(gate.elf_interpreter(elf([1, 1, 4])), (True, None))

    def test_a_dynamic_executable_is_refused(self):
        """Negative: a PT_INTERP header names a loader the initramfs does not have."""
        static, reason = gate.elf_interpreter(elf([6, 3, 1]))
        self.assertFalse(static)
        self.assertIn("PT_INTERP", reason)
        self.assertFalse(gate.elf_interpreter(b"#!/bin/sh\n" + bytes(64))[0])

    def test_the_header_bounds_are_held(self):
        """Boundary: 64 headers are read, 65 refused, a truncated table refused."""
        self.assertTrue(gate.elf_interpreter(elf([1] * gate.MAX_ELF_HEADERS))[0])
        self.assertFalse(gate.elf_interpreter(elf([1] * (gate.MAX_ELF_HEADERS + 1)))[0])
        self.assertFalse(gate.elf_interpreter(elf([1, 1])[:-60])[0])


class InitramfsTests(unittest.TestCase):
    """The newc writer: exact, aligned, deterministic, and refusing escapes."""

    def test_the_archive_reads_back(self):
        """Positive: /dev, /init with its bytes and mode, then the trailer."""
        archive = gate.newc_archive([("dev", 0o040755, b""), ("init", 0o100755, b"\x7fELF!")])
        self.assertEqual(
            members(archive),
            [("dev", 0o040755, b""), ("init", 0o100755, b"\x7fELF!"), ("TRAILER!!!", 0, b"")],
        )
        self.assertTrue(archive.startswith(b"070701"))
        self.assertEqual(archive, gate.newc_archive(members(archive)[:2]))

    def test_an_escaping_name_is_refused(self):
        """Negative: absolute, parent-relative, empty and over-long names."""
        for name in ("/init", "../init", "", "a/../../b", "x" * 256):
            with self.assertRaises(gate.GateError):
                gate.newc_archive([(name, 0o100755, b"")])

    def test_every_member_is_four_byte_aligned(self):
        """Boundary: names and data of every length modulo four stay aligned."""
        for length in range(1, 9):
            archive = gate.newc_archive([("n" * length, 0o100644, b"d" * length)])
            self.assertEqual(len(archive) % 4, 0)
            self.assertEqual(members(archive)[0], ("n" * length, 0o100644, b"d" * length))

    def test_a_dynamic_guest_is_not_packed(self):
        """Negative: build_initramfs refuses a guest that needs a loader."""
        with tempfile.TemporaryDirectory() as scratch:
            guest = Path(scratch) / "guest"
            guest.write_bytes(elf([3]))
            with self.assertRaises(gate.GateError):
                gate.build_initramfs(guest, Path(scratch) / "initramfs.cpio")
            guest.write_bytes(elf([1]))
            digest = gate.build_initramfs(guest, Path(scratch) / "initramfs.cpio")
            data = (Path(scratch) / "initramfs.cpio").read_bytes()
        self.assertEqual(digest, hashlib.sha256(data).hexdigest())


class SandboxCapabilityTests(unittest.TestCase):
    """When the sandbox half skips, and when the cache is a failure instead."""

    def cache(self, scratch, pin):
        """Fill a cache with files named as the pin names them."""
        for path in gate.cached_paths(Path(scratch), pin).values():
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_text("CONFIG_VIRTIO_VSOCKETS=y\n", encoding="ascii")
        return Path(scratch)

    def test_a_host_with_everything_has_no_reason(self):
        """Positive: KVM, the cache, the memory and cargo."""
        pin = gate.load_pin()
        with tempfile.TemporaryDirectory() as scratch:
            cache = self.cache(scratch, pin)
            kvm = cache / "kvm"
            kvm.write_text("", encoding="ascii")
            reasons = gate.sandbox_reasons(
                cache, pin, "linux", lambda name: name, kvm, gate.MIN_AVAILABLE_MIB, "x86_64"
            )
        self.assertEqual(reasons, [])

    def test_each_missing_capability_is_named(self):
        """Negative: no KVM, no fetch, too little memory, no cargo, another platform."""
        pin = gate.load_pin()
        with tempfile.TemporaryDirectory() as scratch:
            reasons = gate.sandbox_reasons(
                Path(scratch),
                pin,
                "linux",
                lambda name: None,
                Path(scratch) / "no-kvm",
                1024,
                "aarch64",
            )
        text = "\n".join(reasons)
        self.assertIn("this host is aarch64", text)
        self.assertIn("no-kvm", text)
        self.assertIn("make workstation-fetch", text)
        self.assertIn("1024 MiB available", text)
        self.assertIn("cargo", text)
        self.assertIn("win32", gate.sandbox_reasons(Path("."), pin, "win32")[0])

    def test_the_memory_floor_is_closed_at_its_edge(self):
        """Boundary: exactly the floor runs, one MiB less skips."""
        pin = gate.load_pin()
        with tempfile.TemporaryDirectory() as scratch:
            cache = self.cache(scratch, pin)
            kvm = cache / "kvm"
            kvm.write_text("", encoding="ascii")
            below = gate.MIN_AVAILABLE_MIB - 1
            reasons = gate.sandbox_reasons(
                cache, pin, "linux", lambda name: name, kvm, below, "x86_64"
            )
        self.assertEqual(len(reasons), 1)
        self.assertIn(str(below), reasons[0])

    def test_only_the_pinned_architecture_runs(self):
        """Positive, negative and boundary: x86_64, then aarch64, then an unread machine."""
        self.assertEqual(gate.arch_reasons("x86_64"), [])
        self.assertEqual(
            gate.arch_reasons("aarch64"),
            ["the pinned Firecracker and guest are x86_64; this host is aarch64"],
        )
        self.assertIn("this host is unknown", gate.arch_reasons("")[0])

    def test_a_cache_that_differs_from_the_pin_is_a_problem(self):
        """Negative: every mismatching file is named with both digests."""
        pin = gate.load_pin()
        with tempfile.TemporaryDirectory() as scratch:
            problems = gate.cache_problems(self.cache(scratch, pin), pin)
        self.assertEqual(sum("not the pinned" in problem for problem in problems), 4)

    def test_a_cache_that_matches_its_pin_has_no_problem(self):
        """Positive and negative: a pin of the files' own digests, then a missing option."""
        pin = json.loads(json.dumps(gate.load_pin()))
        with tempfile.TemporaryDirectory() as scratch:
            cache = self.cache(scratch, pin)
            paths = gate.cached_paths(cache, pin)
            digest = gate.file_sha256(paths["kernel"])
            pin["firecracker"]["binary"]["sha256"] = digest
            pin["firecracker"]["licence_file"]["sha256"] = digest
            pin["guest_kernel"]["sha256"] = digest
            pin["guest_kernel"]["config"]["sha256"] = digest
            pin["guest_kernel"]["required_config"] = ["CONFIG_VIRTIO_VSOCKETS=y"]
            self.assertEqual(gate.cache_problems(cache, pin), [])
            pin["guest_kernel"]["required_config"].append("CONFIG_BLK_DEV_INITRD=y")
            self.assertEqual(len(gate.cache_problems(cache, pin)), 1)

    def test_memory_is_read_from_meminfo(self):
        """Positive and negative: MemAvailable in MiB, or None."""
        with tempfile.TemporaryDirectory() as scratch:
            meminfo = Path(scratch) / "meminfo"
            meminfo.write_text("MemTotal: 1 kB\nMemAvailable:    9437184 kB\n", encoding="ascii")
            self.assertEqual(gate.available_mib(meminfo), 9216)
            self.assertIsNone(gate.available_mib(Path(scratch) / "absent"))

    def test_the_firecracker_version_is_read_back(self):
        """Positive and negative: the pinned line, or a refusal."""
        pin = gate.load_pin()
        good = mock.Mock(return_value=(0, "Firecracker v1.17.0\n\nlog line\n", ""))
        self.assertEqual(
            gate.firecracker_version(Path(gate.FIRECRACKER_NAME), pin, good), "Firecracker v1.17.0"
        )
        bad = mock.Mock(return_value=(0, "Firecracker v1.16.2\n", ""))
        with self.assertRaises(gate.GateError):
            gate.firecracker_version(Path(gate.FIRECRACKER_NAME), pin, bad)


class LeftoverTests(unittest.TestCase):
    """Nothing the run started may outlive it."""

    def test_a_process_naming_the_run_is_found(self):
        """Positive: the pinned Firecracker and the sandbox host naming the run are found."""
        firecracker = f"/c/firecracker/v1.17.0/{gate.FIRECRACKER_NAME}".encode()
        with tempfile.TemporaryDirectory() as scratch:
            proc = Path(scratch)
            for pid, command in (
                ("12", firecracker + b"\0--no-api\0--config-file\0/r/run-1/vms/vm-1/vm.json"),
                ("15", b"/t/debug/aegis-vesta-sandbox\0run\0" + firecracker + b"\0/r/run-1/vms"),
                ("13", b"bash\0"),
                ("self", firecracker + b"\0/r/run-1"),
            ):
                (proc / pid).mkdir()
                (proc / pid / "cmdline").write_bytes(command)
            (proc / "14").mkdir()
            self.assertEqual(sorted(gate.leftover_processes("/r/run-1", proc)), [12, 15])

    def test_a_process_that_only_names_the_run_is_not_the_runs(self):
        """Negative: a tail, a shell or a pager naming the run directory is left alone."""
        with tempfile.TemporaryDirectory() as scratch:
            proc = Path(scratch)
            for pid, command in (
                ("20", b"tail\0-f\0/r/run-1/vms/vm-100/console.log"),
                ("21", b"/bin/fish\0-c\0less /r/run-1/sandbox.log"),
                ("22", b"python3\0tools/verify_workstation.py\0/r/run-1"),
            ):
                (proc / pid).mkdir()
                (proc / pid / "cmdline").write_bytes(command)
            self.assertEqual(gate.leftover_processes("/r/run-1", proc), [])

    def test_the_program_is_matched_by_name_not_by_prefix(self):
        """Boundary: another build, another run or the pinned name as an argument is not it."""
        name = gate.FIRECRACKER_NAME.encode()
        self.assertTrue(gate.started_by_run(name + b"\0/r/run-1/vm.json", "/r/run-1"))
        self.assertFalse(gate.started_by_run(name + b"\0/r/run-2/vm.json", "/r/run-1"))
        self.assertFalse(gate.started_by_run(name + b"x\0/r/run-1/vm.json", "/r/run-1"))
        self.assertFalse(gate.started_by_run(b"cat\0" + name + b"\0/r/run-1", "/r/run-1"))
        self.assertFalse(gate.started_by_run(b"", "/r/run-1"))

    def test_network_devices_are_listed(self):
        """Boundary: an absent class is an empty list, a present one is sorted."""
        with tempfile.TemporaryDirectory() as scratch:
            net = Path(scratch)
            self.assertEqual(gate.net_devices(net / "absent"), [])
            for name in ("lo", "eth0"):
                (net / name).mkdir()
            self.assertEqual(gate.net_devices(net), ["eth0", "lo"])

    @unittest.skipUnless(os.name == "posix", "socket files are a POSIX facility; Linux runs it")
    def test_a_socket_left_behind_is_found(self):
        """Negative: a bound socket file under the run directory is reported."""
        with tempfile.TemporaryDirectory(dir="/tmp" if os.path.isdir("/tmp") else None) as scratch:
            run = Path(scratch)
            (run / "plain").write_text("", encoding="ascii")
            server = socket.socket(socket.AF_UNIX, socket.SOCK_STREAM)
            try:
                server.bind(str(run / "v.sock"))
            except OSError as error:
                server.close()
                self.skipTest(f"this host cannot bind a socket file ({error})")
            try:
                self.assertEqual(gate.leftover_sockets(run), [run / "v.sock"])
            finally:
                server.close()


class ReportTests(unittest.TestCase):
    """Each child's report is read the way the binaries print it."""

    OUTPUT = (
        "info zone: package-0\n"
        "PASS rapl/recorded-range\n"
        "     max_energy_range_uj reads 65532610987\n"
        "FAIL rapl/measured-sci\n"
        "     the sample is labelled simulated\n"
        "RESULT fail: 1 case(s) failed\n"
    )

    def test_facts_cases_notes_and_result_are_read(self):
        """Positive: every line kind lands where it belongs."""
        info, cases, result, notes = gate.parse_child(self.OUTPUT)
        self.assertEqual(info, {"zone": "package-0"})
        self.assertEqual(cases, {"rapl/recorded-range": True, "rapl/measured-sci": False})
        self.assertEqual(result, "fail: 1 case(s) failed")
        self.assertEqual(notes["rapl/measured-sci"], ["the sample is labelled simulated"])

    def test_a_failed_or_missing_case_is_a_problem(self):
        """Negative: FAIL and absence both count."""
        problems = gate.judge_child(1, self.OUTPUT, gate.RAPL_CASES, "aegis-tellus-rapl")
        self.assertIn("rapl/measured-sci failed", problems)
        self.assertIn("rapl/wrap-at-live-range did not report", problems)

    def test_a_non_zero_exit_after_every_pass_is_a_problem(self):
        """Boundary: the exit status is judged too."""
        output = "".join(f"PASS {case}\n" for case in gate.SANDBOX_CASES) + "RESULT pass\n"
        self.assertEqual(gate.judge_child(0, output, gate.SANDBOX_CASES, "x"), [])
        self.assertEqual(len(gate.judge_child(3, output, gate.SANDBOX_CASES, "x")), 1)

    def test_the_required_cases_are_the_ones_the_binaries_print(self):
        """Positive: the gate's lists equal the crates' own constants."""
        rapl = RAPL_CASES_SOURCE.read_text(encoding="utf-8")
        host_run = HOST_CASES_SOURCE.read_text(encoding="utf-8")
        pair = re.search(r"PAIR_CASES: \[&str; 6\] = \[(.*?)\];", rapl, re.S).group(1)
        self.assertEqual(tuple(re.findall(r'"([^"]+)"', pair)), gate.RAPL_CASES)
        self.assertIn(f'OBSERVED_WRAP_CASE: &str = "{gate.WRAP_CASE}"', rapl)
        cases = re.search(r"HOST_CASES: \[&str; 5\] = \[(.*?)\];", host_run, re.S).group(1)
        self.assertEqual(tuple(re.findall(r'"([^"]+)"', cases)), gate.SANDBOX_CASES)


class FetchTests(unittest.TestCase):
    """The one networked step refuses bytes that are not the pinned ones."""

    def test_without_curl_the_fetch_skips(self):
        """Negative: no curl is a skip with the convention's line."""
        output = io.StringIO()
        with mock.patch.object(gate.shutil, "which", return_value=None), redirect_stdout(output):
            self.assertEqual(gate.fetch(Path(".")), 0)
        self.assertIn("SKIP: curl is not on PATH", output.getvalue())

    def test_another_architecture_fetches_nothing(self):
        """Negative: a host that cannot run the pinned x86_64 bytes skips before any download."""
        output = io.StringIO()
        with mock.patch.object(gate.shutil, "which", return_value="curl"), mock.patch.object(
            gate, "platform_machine", return_value="aarch64"
        ), mock.patch.object(gate, "load_pin") as pinned, redirect_stdout(output):
            self.assertEqual(gate.fetch(Path(".")), 0)
        pinned.assert_not_called()
        self.assertIn("this host is aarch64; the workstation fetch did not run.", output.getvalue())

    def test_a_download_of_other_bytes_is_refused(self):
        """Negative: the partial file is removed and nothing is kept."""
        with tempfile.TemporaryDirectory() as scratch:
            target = Path(scratch) / "vmlinux"

            def runner(argv, timeout):
                Path(argv[argv.index("--output") + 1]).write_bytes(b"other")
                return 0, "", ""

            with self.assertRaises(gate.GateError):
                gate.download("https://x/vmlinux", target, 5, KERNEL_SHA256, runner)
            self.assertEqual(list(Path(scratch).iterdir()), [])

    def test_a_cached_download_is_not_fetched_again(self):
        """Positive and boundary: pinned bytes on disk short-circuit the network."""
        with tempfile.TemporaryDirectory() as scratch:
            target = Path(scratch) / "blob"
            target.write_bytes(b"pinned")
            digest = hashlib.sha256(b"pinned").hexdigest()
            runner = mock.Mock()
            self.assertIn("cached", gate.download("https://x/blob", target, 6, digest, runner))
            runner.assert_not_called()

    def test_a_member_is_extracted_and_checked(self):
        """Positive and negative: the pinned member, then a wrong digest and a directory."""
        with tempfile.TemporaryDirectory() as scratch:
            base = Path(scratch)
            (base / "release").mkdir()
            (base / "release" / "firecracker").write_bytes(b"binary")
            archive = base / "release.tgz"
            with tarfile.open(archive, "w:gz") as bundle:
                bundle.add(base / "release", arcname="release")
            digest = hashlib.sha256(b"binary").hexdigest()
            out = base / "firecracker"
            self.assertIn(
                "extracted", gate.extract_member(archive, "release/firecracker", out, digest)
            )
            with self.assertRaises(gate.GateError):
                gate.extract_member(archive, "release/firecracker", out, KERNEL_SHA256)
            self.assertFalse(out.exists())
            with self.assertRaises(gate.GateError):
                gate.extract_member(archive, "release", out, digest)


class RunTests(unittest.TestCase):
    """The run's conventions: skips, failures, identifiers, where logs go."""

    def test_both_halves_skipping_is_exit_zero_with_skip_lines(self):
        """Negative: a host without either capability exits 0 and says so."""
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as scratch, redirect_stdout(output), mock.patch.dict(
            os.environ, {"AEGIS_WORKSTATION_DIR": scratch}
        ), mock.patch.object(gate, "rapl_reasons", return_value=["no counter"]), mock.patch.object(
            gate, "rustc_version", return_value="rustc (mocked)"
        ), mock.patch.object(
            gate, "sandbox_reasons", return_value=["no kvm"]
        ):
            self.assertEqual(gate.main([]), 0)
        text = output.getvalue()
        self.assertIn("SKIP: no counter; the RAPL half did not run.", text)
        self.assertIn("SKIP: no kvm; the sandbox half did not run.", text)
        self.assertIn("PASS: nothing ran", text)

    def test_the_run_keeps_its_host_and_its_output(self):
        """Positive: host.json beside the readings, and gate.log holding every printed line."""
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as scratch, redirect_stdout(output), mock.patch.dict(
            os.environ, {"AEGIS_WORKSTATION_DIR": scratch}
        ), mock.patch.object(gate, "rapl_reasons", return_value=["no counter"]), mock.patch.object(
            gate, "rustc_version", return_value="rustc (mocked)"
        ), mock.patch.object(
            gate, "sandbox_reasons", return_value=["no kvm"]
        ), mock.patch.object(
            gate, "host_facts", return_value={"cpu": "CPU (mocked)", "kernel": "7.2.8-mocked"}
        ):
            self.assertEqual(gate.main([]), 0)
            (run,) = (Path(scratch) / "runs").iterdir()
            facts = json.loads((run / "host.json").read_text(encoding="utf-8"))
            log = (run / "gate.log").read_text(encoding="utf-8")
        self.assertEqual(facts["schema"], gate.HOST_SCHEMA)
        self.assertEqual(facts["run"], run.name)
        self.assertEqual(
            (facts["cpu"], facts["kernel"], facts["rustc"]),
            ("CPU (mocked)", "7.2.8-mocked", "rustc (mocked)"),
        )
        self.assertEqual(log, output.getvalue())
        self.assertIn("host: CPU (mocked); kernel 7.2.8-mocked (kept in host.json)", log)
        self.assertIn("SKIP: no kvm; the sandbox half did not run.", log)

    def test_a_failing_half_fails_the_gate(self):
        """Positive: one failure is exit 1, naming the half."""
        output = io.StringIO()
        with tempfile.TemporaryDirectory() as scratch, redirect_stdout(output), mock.patch.dict(
            os.environ, {"AEGIS_WORKSTATION_DIR": scratch}
        ), mock.patch.object(
            gate, "rapl_half", return_value=(gate.FAIL, ["x failed"])
        ), mock.patch.object(
            gate, "sandbox_reasons", return_value=["no kvm"]
        ), mock.patch.object(
            gate, "rustc_version", return_value="rustc (mocked)"
        ):
            self.assertEqual(gate.main([]), 1)
        self.assertIn("FAIL: RAPL half did not pass.", output.getvalue())

    def test_logs_never_land_in_the_repository(self):
        """Boundary: the default cache is under the user's cache, the override wins."""
        default = gate.cache_dir({})
        self.assertEqual(default.name, "aegis-workstation")
        self.assertNotIn(str(ROOT), str(default))
        self.assertEqual(gate.cache_dir({"AEGIS_WORKSTATION_DIR": "/x/y"}), Path("/x/y"))
        self.assertTrue(re.fullmatch(r"r\d{8}T\d{6}-[0-9a-f]{4}", gate.run_id()))


class RustcTests(unittest.TestCase):
    """The compiler is printed with every run, or why it was not."""

    def test_the_version_is_read_back(self):
        """Positive: the line rustc prints."""
        runner = mock.Mock(return_value=(0, "rustc 1.98.1 (48a229cea 2026-09-01)\n", ""))
        self.assertEqual(
            gate.rustc_version(runner, lambda name: name), "rustc 1.98.1 (48a229cea 2026-09-01)"
        )

    def test_a_missing_or_failing_rustc_is_unread(self):
        """Negative and boundary: absence and a non-zero exit are both named."""
        self.assertIn("not on PATH", gate.rustc_version(mock.Mock(), lambda name: None))
        failing = mock.Mock(return_value=(1, "", "error: no default toolchain"))
        self.assertIn("exit 1", gate.rustc_version(failing, lambda name: name))


class MakefileTests(unittest.TestCase):
    """The target runs the gate, and verify-all does not."""

    def recipe(self, target):
        """Return the recipe lines of `target`."""
        lines = MAKEFILE.read_text(encoding="utf-8").splitlines()
        start = lines.index(f"{target}:")
        body = []
        for line in lines[start + 1 :]:
            if not line.startswith("\t"):
                break
            body.append(line)
        return body

    def test_the_targets_run_the_gate(self):
        """Positive: verify-workstation and workstation-fetch."""
        self.assertEqual(
            self.recipe("verify-workstation"), ["\tpython3 tools/verify_workstation.py"]
        )
        self.assertEqual(
            self.recipe("workstation-fetch"), ["\tpython3 tools/verify_workstation.py --fetch"]
        )

    def test_verify_all_does_not_run_it(self):
        """Negative: the hardware gate is not part of verify-all."""
        recipe = "\n".join(self.recipe("verify-all"))
        self.assertNotIn("workstation", recipe)

    def test_the_page_records_the_pin_and_the_cases(self):
        """Boundary: docs/build/workstation.md carries the pins, the read and every case."""
        page = PAGE.read_text(encoding="utf-8")
        for value in (ARCHIVE_SHA256, BINARY_SHA256, KERNEL_SHA256, " ".join(SUDO)):
            self.assertIn(value, page)
        for case in gate.RAPL_CASES + (gate.WRAP_CASE,) + gate.SANDBOX_CASES:
            self.assertIn(case, page)


if __name__ == "__main__":
    unittest.main()
