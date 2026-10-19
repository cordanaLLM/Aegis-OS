#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative and boundary coverage for the M29 live session gate (HISS-15).

The gate itself needs KVM, QEMU and a cached 3.4 GB live image. These tests need
none of them. They exercise every decision the gate makes on the way -- the pin
it accepts, the host memory it requires, the credentials and argument vector it
builds, what it reads out of the guest report, the inclusive deadline rule,
that a wrong image is refused before anything runs, and which programs it may
start -- so a checkout that cannot boot the guest still fails when one of them
drifts. The sweeps at the end hold the gate to HISS-02, HISS-04 and HISS-08.
"""

import ast
import base64
import hashlib
import io
import json
import tempfile
import unittest
from contextlib import redirect_stdout
from pathlib import Path
from unittest import mock

import test_boot_harness as sweeps
import verify_boot_harness as harness
import verify_live_session as gate

ROOT = Path(__file__).resolve().parent.parent
GATE = ROOT / "tools" / "verify_live_session.py"
SCRIPT = ROOT / "tools" / "guest" / "aegis-session-report.sh"
PAGE = ROOT / "docs" / "build" / "live-session.md"
MAKEFILE = ROOT / "Makefile"
MAX_MAKEFILE_LINES = 4000

# Every program the gate starts itself; M24's helpers start the rest (gpg and
# curl, through the signature check and the fetch) and are swept in M24's tests.
ALLOWED_PROGRAMS = {"qemu-system-x86_64", "xorriso"}

PIN = gate.load_pin(gate.PIN)
NONCE = "aegis-m29-" + "cd" * 16


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


def guest_report(nonce=NONCE, drop=(), extra=()):
    """Return one serial report in the shape the guest script writes, minus `drop` kinds."""
    mark = gate.GUEST_MARK
    lines = [
        "BEGIN",
        f"NONCE {nonce}",
        "UNAME-R 6.19.10-300.fc44.x86_64",
        "OS fedora-44",
        "SESSION liveuser 41",
        "SERVICE compositor org.kde.KWin 0 kwin 6.6.4-1.fc44",
        "SERVICE portal org.freedesktop.portal.Desktop 1 xdg-desktop-portal 1.20.3-1.fc44",
        "SERVICE portal-backend org.freedesktop.impl.portal.desktop.kde 1 "
        "xdg-desktop-portal-kde 6.6.4-1.fc44",
        "SERVICE a11y-bus org.a11y.Bus 0 at-spi2-core 2.58.1-1.fc44",
        "SERVICE at-spi2-registry org.a11y.atspi.Registry 2 at-spi2-core 2.58.1-1.fc44",
        "SERVICE watcher org.kde.StatusNotifierWatcher 0 kf6-kstatusnotifieritem 6.22.0-1.fc44",
        "MISSING planted-absent org.aegis.M29.PlantedAbsent 3",
        "GLOBAL wl_compositor 6",
        "GLOBAL zwlr_layer_shell_v1 5",
        "PORTAL-SETTINGS-VERSION 2",
        "PORTAL-COLOR-SCHEME 0",
        "WATCHER-PROTOCOL-VERSION 0",
        *extra,
        "END",
    ]
    kept = [line for line in lines if line.split()[0] not in drop]
    return "\r\n".join(f"{mark}-{line}" for line in kept) + "\r\n"


def observed(report=None, **changes):
    """Return one `boot_guest` result for a clean boot, with `changes` applied."""
    text = guest_report() if report is None else report
    result = {
        "report": text,
        "complete": True,
        "elapsed": 90.0,
        "exit_status": 0,
        "nonce": NONCE,
        "fields": gate.report_fields(text),
    }
    result.update(changes)
    return result


def quiet(function, *args):
    """Call `function` with stdout captured; return (result, printed text)."""
    buffer = io.StringIO()
    with redirect_stdout(buffer):
        result = function(*args)
    return result, buffer.getvalue()


class PinTests(unittest.TestCase):
    """The committed pin, and every defect the loader refuses."""

    def test_the_committed_pin_names_the_signed_fedora_kde_live_image(self):
        self.assertEqual(PIN["schema"], gate.PIN_SCHEMA)
        self.assertEqual(PIN["file"], "Fedora-KDE-Desktop-Live-44-1.7.x86_64.iso")
        self.assertEqual(PIN["size"], 3368683520)
        self.assertTrue(PIN["sha256"].startswith("c8295961d4c4"))
        self.assertEqual(PIN["signature"]["scheme"], harness.GPG_SCHEME)
        self.assertEqual(
            PIN["signature"]["fingerprint"], "36F612DCF27F7D1A48A835E4DBFCF71C6D9F90A6"
        )

    def test_the_pin_probes_compositor_portal_at_spi2_and_watcher(self):
        names = {service["name"] for service in PIN["services"]}
        for wanted in (
            "org.kde.KWin",
            "org.freedesktop.portal.Desktop",
            "org.a11y.atspi.Registry",
            "org.kde.StatusNotifierWatcher",
        ):
            self.assertIn(wanted, names)

    def test_each_defect_is_refused(self):
        defects = {
            "sha256": pin_copy(sha256="C8" * 32),
            "http": pin_copy(url="http://example.invalid/x.iso"),
            "file": pin_copy(file="other.iso"),
            "format": pin_copy(format="qcow2"),
            "kernel": pin_copy(**{"boot.kernel": "/boot/../linux"}),
            "volume": pin_copy(**{"boot.volume-id": "two words"}),
            "user": pin_copy(**{"boot.live-user": "Root User"}),
            "memory": pin_copy(**{"boot.memory-mib": 512}),
            "globals": pin_copy(**{"required-globals": []}),
            "global-name": pin_copy(**{"required-globals": ["Not An Interface"]}),
            "no-services": pin_copy(services=[]),
            "bus": pin_copy(services=[dict(PIN["services"][0], bus="system")]),
            "name": pin_copy(services=[dict(PIN["services"][0], name="nodots")]),
            "duplicate": pin_copy(services=[PIN["services"][0], PIN["services"][0]]),
            "planted-label": pin_copy(services=[dict(PIN["services"][0], label="planted-absent")]),
            "fingerprint": pin_copy(**{"signature.fingerprint": "36f612dc"}),
        }
        with tempfile.TemporaryDirectory() as directory:
            for label, pin in defects.items():
                with self.subTest(label), self.assertRaises(harness.GateError):
                    gate.load_pin(written_pin(pin, directory))

    def test_a_missing_section_is_refused_before_its_values_are_read(self):
        with tempfile.TemporaryDirectory() as directory:
            for key in ("boot", "services", "signature", "sha256"):
                pin = pin_copy()
                del pin[key]
                with self.subTest(key), self.assertRaisesRegex(harness.GateError, key):
                    gate.load_pin(written_pin(pin, directory))
            with self.assertRaisesRegex(harness.GateError, "objects"):
                gate.load_pin(written_pin(pin_copy(boot=[]), directory))

    def test_deadlines_are_bounded_at_both_ends(self):
        service = PIN["services"][0]
        with tempfile.TemporaryDirectory() as directory:
            for value, accepted in ((0, False), (1, True), (gate.MAX_DEADLINE, True)):
                pin = pin_copy(services=[dict(service, **{"deadline-seconds": value})])
                with self.subTest(value):
                    if accepted:
                        gate.load_pin(written_pin(pin, directory))
                    else:
                        self.assertRaises(
                            harness.GateError, gate.load_pin, written_pin(pin, directory)
                        )
            pin = pin_copy(**{"boot.report-deadline-seconds": gate.MAX_DEADLINE + 1})
            self.assertRaises(harness.GateError, gate.load_pin, written_pin(pin, directory))
            pin = pin_copy(services=[dict(service, **{"deadline-seconds": True})])
            self.assertRaises(harness.GateError, gate.load_pin, written_pin(pin, directory))

    def test_text_that_is_not_json_is_refused(self):
        with tempfile.TemporaryDirectory() as directory:
            path = Path(directory) / "pin.json"
            path.write_text("{", encoding="utf-8")
            self.assertRaises(harness.GateError, gate.load_pin, path)
            path.write_text("[]", encoding="utf-8")
            self.assertRaises(harness.GateError, gate.load_pin, path)

    def test_the_signed_checksum_line_yields_the_pinned_digest(self):
        text = (
            "# Fedora-KDE-Desktop-Live-44-1.7.x86_64.iso: 3368683520 bytes\n"
            f"SHA256 (Fedora-KDE-Desktop-Live-44-1.7.x86_64.iso) = {PIN['sha256']}\n"
        )
        self.assertEqual(harness.signed_record(text, PIN), (PIN["sha256"], "44-1.7"))


class MemoryTests(unittest.TestCase):
    """The boot waits for 12 GiB available and 15 GiB swap free."""

    def fields(self, available, swap):
        return gate.meminfo(
            f"MemTotal: 65000000 kB\nMemAvailable: {available} kB\nSwapFree: {swap} kB\n"
        )

    def test_at_the_floor_the_boot_may_run(self):
        self.assertEqual(gate.memory_reasons(self.fields(12 << 20, 15 << 20)), [])

    def test_one_kibibyte_below_either_floor_is_a_reason(self):
        reasons = gate.memory_reasons(self.fields((12 << 20) - 1, (15 << 20) - 1))
        self.assertEqual(len(reasons), 2)
        self.assertIn("MemAvailable", reasons[0])
        self.assertIn("SwapFree", reasons[1])

    def test_a_missing_field_is_a_reason_not_a_crash(self):
        reasons = gate.memory_reasons(gate.meminfo("MemTotal: 1 kB\nnonsense\n"))
        self.assertEqual(len(reasons), 2)
        self.assertIn("reports no MemAvailable", reasons[0])


class GuestInputTests(unittest.TestCase):
    """The credentials, the kernel command line and the QEMU argv the gate builds."""

    def test_every_placeholder_is_replaced_once(self):
        text = gate.report_script(NONCE, PIN)
        for placeholder in gate.PLACEHOLDERS:
            self.assertNotIn(placeholder, text)
        self.assertIn(f"NONCE='{NONCE}'", text)
        self.assertIn("LIVE_USER='liveuser'", text)

    def test_a_nonce_the_gate_did_not_make_is_refused(self):
        for nonce in ("aegis-m29-" + "cd" * 15, "aegis-m29-" + "cd" * 17, "x'; reboot; '"):
            with self.subTest(nonce), self.assertRaises(harness.GateError):
                gate.report_script(nonce, PIN)

    def test_the_script_and_the_gate_agree_on_marker_and_limits(self):
        text = SCRIPT.read_text(encoding="utf-8")
        self.assertIn(f"MARK='{gate.GUEST_MARK}'", text)
        self.assertIn(f"MAX_SERVICES={gate.MAX_SERVICES}", text)
        self.assertIn(gate.SERVICES_CREDENTIAL, text)
        for placeholder in gate.PLACEHOLDERS:
            self.assertEqual(text.count(placeholder), 1, placeholder)

    def test_the_planted_absent_service_is_probed_last(self):
        lines = gate.services_text(gate.probed_services(PIN)).splitlines()
        self.assertEqual(len(lines), len(PIN["services"]) + 1)
        self.assertEqual(lines[-1], "planted-absent session org.aegis.M29.PlantedAbsent 3")

    def test_the_credentials_decode_back_to_their_text(self):
        option = gate.credential("a.b", "hello\n")
        self.assertEqual(option[0], "-smbios")
        prefix = "type=11,value=io.systemd.credential.binary:a.b="
        self.assertTrue(option[1].startswith(prefix))
        self.assertEqual(base64.b64decode(option[1][len(prefix) :]), b"hello\n")

    def test_the_unit_imports_both_credentials_and_runs_the_script(self):
        unit = gate.unit_text(PIN)
        self.assertIn(f"ImportCredential={gate.SCRIPT_CREDENTIAL}", unit)
        self.assertIn(f"ImportCredential={gate.SERVICES_CREDENTIAL}", unit)
        self.assertIn(f"ExecStart=/bin/sh %d/{gate.SCRIPT_CREDENTIAL}", unit)
        self.assertIn(f"TimeoutStartSec={PIN['boot']['report-deadline-seconds']}", unit)

    def test_the_argv_is_kvm_only_offline_and_read_only(self):
        boot = gate.Boot(Path("/run/x"))
        argv = gate.guest_argv(PIN, boot, Path("/cache/guest.iso"), NONCE)
        joined = " ".join(argv)
        self.assertEqual(argv[0], "qemu-system-x86_64")
        self.assertIn("q35,accel=kvm", argv)
        self.assertNotIn("tcg", joined)
        self.assertEqual(argv[argv.index("-nic") + 1], "none")
        self.assertIn("if=virtio,format=raw,readonly=on,file=/cache/guest.iso", argv)
        self.assertIn("root=live:CDLABEL=Fedora-KDE-Live-44", argv[argv.index("-append") + 1])
        self.assertIn(f"systemd.wants={gate.UNIT_NAME}", argv[argv.index("-append") + 1])
        self.assertEqual(argv.count("-smbios"), 3)
        self.assertEqual(argv.count("-serial"), 2)


class JudgeTests(unittest.TestCase):
    """What the gate reads out of a guest report, and how it decides."""

    def test_a_complete_report_passes_and_prints_every_version(self):
        problems, printed = quiet(gate.positive_case, observed(), PIN)
        self.assertEqual(problems, [])
        for wanted in ("kwin 6.6.4-1.fc44", "xdg-desktop-portal 1.20.3", "at-spi2-core 2.58.1"):
            self.assertIn(wanted, printed)
        self.assertIn("kf6-kstatusnotifieritem", printed)
        self.assertIn("wayland global zwlr_layer_shell_v1 version 5", printed)

    def test_a_missing_service_fails_by_name(self):
        report = guest_report().replace(
            "SERVICE watcher org.kde.StatusNotifierWatcher 0 kf6-kstatusnotifieritem 6.22.0-1.fc44",
            "MISSING watcher org.kde.StatusNotifierWatcher 60",
        )
        problems, _printed = quiet(gate.positive_case, observed(report), PIN)
        self.assertEqual(
            problems,
            ["watcher: org.kde.StatusNotifierWatcher did not appear within its 60 s deadline"],
        )

    def test_the_deadline_is_inclusive(self):
        self.assertTrue(gate.within_deadline(60, 60))
        self.assertTrue(gate.within_deadline(0, 60))
        self.assertFalse(gate.within_deadline(61, 60))
        self.assertFalse(gate.within_deadline(-1, 60))
        self.assertFalse(gate.within_deadline(None, 60))

    def test_an_answer_past_its_deadline_fails(self):
        report = guest_report().replace("org.kde.KWin 0 kwin", "org.kde.KWin 61 kwin")
        problems, _printed = quiet(gate.positive_case, observed(report), PIN)
        self.assertEqual(
            problems, ["compositor: org.kde.KWin answered after 61 s, past its 60 s deadline"]
        )

    def test_an_answer_without_a_package_version_fails(self):
        report = guest_report().replace("kwin 6.6.4-1.fc44", "unpackaged /usr/local/bin/kwin")
        problems, _printed = quiet(gate.positive_case, observed(report), PIN)
        self.assertEqual(len(problems), 1)
        self.assertIn("no package version", problems[0])

    def test_another_boots_report_is_refused(self):
        report = guest_report(nonce="aegis-m29-" + "ef" * 16)
        problems, _printed = quiet(gate.positive_case, observed(report), PIN)
        self.assertIn("not this boot's", problems[0])

    def test_a_missing_global_and_unread_portal_fail(self):
        report = guest_report(drop=("GLOBAL", "PORTAL-COLOR-SCHEME"))
        problems, _printed = quiet(gate.positive_case, observed(report), PIN)
        self.assertIn("the compositor does not advertise zwlr_layer_shell_v1", problems)
        self.assertIn("portal-color-scheme was not read back", problems)

    def test_no_session_and_no_power_off_fail(self):
        report = guest_report(drop=("SESSION",))
        result = observed(report, complete=False, exit_status=None)
        problems, _printed = quiet(gate.positive_case, result, PIN)
        self.assertIn("the guest wrote no complete report before its deadline", problems)
        self.assertTrue(any("session did not appear" in line for line in problems))
        self.assertTrue(any("not a guest power-off" in line for line in problems))

    def test_the_planted_absent_service_must_fail_by_name(self):
        problems, printed = quiet(gate.boundary_case, observed())
        self.assertEqual(problems, [])
        self.assertIn("org.aegis.M29.PlantedAbsent did not appear within its 3 s deadline", printed)

    def test_a_planted_service_that_answered_fails_the_boundary(self):
        report = guest_report().replace(
            "MISSING planted-absent org.aegis.M29.PlantedAbsent 3",
            "SERVICE planted-absent org.aegis.M29.PlantedAbsent 0 impostor 1-1",
        )
        problems, _printed = quiet(gate.boundary_case, observed(report))
        self.assertEqual(len(problems), 2)

    def test_the_report_is_read_up_to_its_line_bound(self):
        flood = tuple(f"GLOBAL filler_{index} 1" for index in range(gate.MAX_REPORT_LINES))
        fields = gate.report_fields(guest_report(extra=flood))
        self.assertNotIn("END", fields)
        self.assertLessEqual(len(fields["GLOBAL"]), gate.MAX_REPORT_LINES)


class RefusalTests(unittest.TestCase):
    """A wrong image is refused at its digest before any program starts or file is written."""

    def context(self, directory, data=b"live image bytes\n"):
        store = Path(directory) / "store"
        store.mkdir()
        pin = pin_copy(size=len(data), sha256=hashlib.sha256(data).hexdigest())
        (store / pin["file"]).write_bytes(data)
        return gate.Context(pin, store, Path(directory) / "run")

    def test_a_planted_wrong_sha256_is_refused_before_boot(self):
        with tempfile.TemporaryDirectory() as directory:
            context = self.context(directory)
            with mock.patch.object(harness, "run") as run:
                problems, printed = quiet(gate.mismatch_case, context)
            self.assertEqual(problems, [])
            run.assert_not_called()
            self.assertEqual(context.spawned, [])
            self.assertFalse(context.run_dir.exists())
            self.assertIn("PASS session/sha256-mismatch-refused", printed)

    def test_the_planted_pin_differs_in_its_last_digit_only(self):
        for last, planted in (("a", "0"), ("0", "1")):
            pin = pin_copy(sha256="c" * 63 + last)
            self.assertEqual(gate.planted_pin(pin)["sha256"], "c" * 63 + planted)

    def test_a_wrong_size_is_refused_before_it_is_hashed(self):
        with tempfile.TemporaryDirectory() as directory:
            context = self.context(directory)
            pin = dict(context.pin, size=context.pin["size"] + 1)
            with self.assertRaisesRegex(harness.Refused, "bytes, not"):
                gate.prepare_boot(context, gate.Boot(context.run_dir / "x"), pin)
            self.assertFalse(context.run_dir.exists())

    def test_the_matching_image_passes_the_after_boot_check(self):
        with tempfile.TemporaryDirectory() as directory:
            problems, printed = quiet(gate.pinned_bytes_case, self.context(directory))
            self.assertEqual(problems, [])
            self.assertIn("PASS session/pinned-bytes-unchanged", printed)

    def test_a_refused_signature_stops_the_run_before_any_boot(self):
        with tempfile.TemporaryDirectory() as directory:
            context = self.context(directory)
            refusal = harness.Refused("signature", "BADSIG")
            with mock.patch.object(harness, "verify_artifact_gpg", side_effect=refusal):
                with mock.patch.object(gate, "boot_guest") as boot:
                    with self.assertRaises(harness.GateError):
                        quiet(gate.run_cases, context)
            boot.assert_not_called()


class EntryTests(unittest.TestCase):
    """SKIP, FAIL and the preflight, without booting."""

    def test_a_host_that_cannot_run_skips_with_its_reasons(self):
        with mock.patch.object(gate, "preflight", return_value=["no /dev/kvm"]):
            code, printed = quiet(gate.main, [])
        self.assertEqual(code, 0)
        self.assertIn("SKIP: no /dev/kvm; the live session gate did not run.", printed)

    def test_an_unreadable_pin_fails(self):
        code, printed = quiet(gate.main, ["--pin", "/nonexistent/pin.json"])
        self.assertEqual(code, 1)
        self.assertIn("FAIL: the gate could not run", printed)

    def test_the_toolchain_rows_are_m24_admissions(self):
        rows = {row[0] for row in harness.TOOLCHAIN}
        self.assertLessEqual(set(gate.TOOLS), rows)

    def test_the_cache_follows_the_override(self):
        with mock.patch.dict("os.environ", {"AEGIS_SESSION_DIR": "/srv/cache"}):
            self.assertEqual(gate.cache_dir(), Path("/srv/cache"))


class WiringTests(unittest.TestCase):
    """The make target sits outside verify-all, and the page states what a pass means."""

    def makefile(self):
        return MAKEFILE.read_text(encoding="utf-8").splitlines()[:MAX_MAKEFILE_LINES]

    def test_verify_session_runs_the_gate_and_verify_all_does_not(self):
        lines = self.makefile()
        self.assertIn("verify-session:", lines)
        recipe = lines[lines.index("verify-session:") + 1]
        self.assertIn("tools/verify_live_session.py", recipe)
        verify_all = next(line for line in lines if line.startswith("verify-all:"))
        self.assertNotIn("verify-session", verify_all)

    def test_the_page_names_the_evidence_class(self):
        text = PAGE.read_text(encoding="utf-8")
        for wanted in ("make verify-session", "development evidence", "D79", "guest.pin.json"):
            self.assertIn(wanted, text)


class SourceSweeps(unittest.TestCase):
    """HISS-02 (deadlines), HISS-04 (size and complexity) and HISS-08 over the gate."""

    tree = ast.parse(GATE.read_text(encoding="utf-8"))

    def test_the_gate_starts_only_allowed_programs(self):
        resolved, undecided = sweeps.call_sites(GATE)
        self.assertEqual(resolved, ALLOWED_PROGRAMS)
        self.assertEqual(undecided, [])

    def test_every_run_call_carries_a_deadline(self):
        runs = [
            node
            for node in ast.walk(self.tree)
            if isinstance(node, ast.Call) and getattr(node.func, "attr", None) == "run"
        ]
        self.assertTrue(runs)
        for node in runs:
            self.assertGreaterEqual(len(node.args), 2, f"run() without a deadline at {node.lineno}")

    def test_no_process_is_started_outside_the_harness_helpers(self):
        names = {node.id for node in ast.walk(self.tree) if isinstance(node, ast.Name)}
        self.assertNotIn("subprocess", names)
        self.assertFalse(names & {"eval", "exec", "compile"})

    def test_no_function_exceeds_the_length_or_complexity_limit(self):
        offenders = []
        for node in sweeps.function_index(self.tree).values():
            length = node.end_lineno - node.lineno + 1
            statements = sum(1 for inner in ast.walk(node) if isinstance(inner, ast.stmt)) - 1
            if (
                length > sweeps.MAX_FUNCTION_LINES
                or sweeps.complexity(node) > sweeps.MAX_COMPLEXITY
            ):
                offenders.append(
                    f"{node.name}: {length} lines, complexity {sweeps.complexity(node)}"
                )
            elif statements > sweeps.MAX_STATEMENTS:
                offenders.append(f"{node.name}: {statements} statements")
        self.assertEqual(offenders, [])


if __name__ == "__main__":
    unittest.main()
