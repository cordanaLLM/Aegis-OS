"""Regressions for the M10 Nucleus kernel gate, without a kernel, an emulator or a network.

`make verify-nucleus-kernel` needs KVM, QEMU, the fetched release and M09's
imago. These tests need none of them. They hold the gate's own decisions --
the pin, the pre-boot refusal, the manifest and imago comparisons, the guest
report parser, the capability diff, the D94 cases, the load checks, the skip
rules and the set of programs the gate may start -- so a checkout that cannot
boot the guest still fails when one of them drifts. Every fixture is bytes or
text written here, so the suite runs on every leg of the platform matrix.
"""

import base64
import contextlib
import gzip
import hashlib
import io
import json
import re
import shlex
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import verify_bpf_objects as bpf
import verify_latency_fixture as harness
import verify_nucleus_kernel as gate
from test_kernel_requirement_check import conforming_config
from test_latency_fixture import call_sites, deadlined_call_sites

ROOT = Path(__file__).resolve().parent.parent
GATE = ROOT / "tools" / "verify_nucleus_kernel.py"
LOADER = ROOT / "bpf" / "loader" / "aegis_bpf_probe.c"
ADMISSION = ROOT / "docs" / "roadmap" / "toolchain-admission.md"
MAKEFILE = ROOT / "Makefile"
PIN = gate.load_pin()
MARK = gate.GUEST_MARK

# Every program the gate may start from its own call sites. Nothing here
# installs a package, writes a boot entry or loads a module; the emulator is
# started only through M23's boot(), which this file never re-implements.
ALLOWED_PROGRAMS = {"bash", "bpftool", "curl", "cosign", "imago"}
# The call sites whose argument vector comes from a builder, with the program
# that builder puts first.
UNDECIDED = (
    ("signature_case", "cosign"),
    ("cosign_refusal", "cosign"),
    ("imago_case", "imago"),
    ("mismatch_case", "imago"),
    ("fetch", "cosign"),
)
MAX_PAGE_LINES = 4000
ADMISSION_HEADING = "## The Nucleus kernel gate's toolchain"
TABLE_ROW = re.compile(r"^\|([^|]*)\|([^|]*)\|([^|]*)\|.*\|$")


def context_in(base):
    """Return a gate context whose cache and run directory live under `base`."""
    context = gate.Context(PIN, Path(base), "rtest")
    context.run_dir.mkdir(parents=True, exist_ok=True)
    return context


def quiet(function, *arguments):
    """Call `function` with stdout captured; return (result, printed text)."""
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        result = function(*arguments)
    return result, out.getvalue()


def packed_log(text):
    """Return a verifier log as the guest prints it: gzip, then base64 lines."""
    encoded = base64.b64encode(gzip.compress(text.encode())).decode()
    return [encoded[index : index + 76] for index in range(0, len(encoded), 76)]


def framed_case(case, output, status, log=None):
    """Return the report lines the load guest prints for one case."""
    lines = [
        f"{MARK}-CASE-BEGIN {case}",
        *output.splitlines(),
        f"{MARK}-CASE-STATUS {case} {status}",
    ]
    lines.append(f"{MARK}-LOG-BEGIN {case}")
    lines += packed_log(log) if log is not None else [f"{MARK}-LOG-ABSENT"]
    return lines + [f"{MARK}-LOG-END {case}", f"{MARK}-CASE-END {case}"]


def probe_lines(sched_ext="y", lsm="capability,lockdown,bpf", btf="present", tracer="y"):
    """Return one probe output as aegis-nucleus-probe.sh prints it."""
    lines = [f"CONFIG_SCHED_CLASS_EXT={sched_ext}", "CONFIG_BPF_LSM=y", f"LSM {lsm}"]
    if tracer == "y":
        lines += ["CONFIG_FUNCTION_TRACER=y", "CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS=y"]
    else:
        lines.append("# CONFIG_FUNCTION_TRACER is not set")
    return lines + [f"BTF-VMLINUX {btf}", "SCHED-EXT-STATE disabled"]


def readback_report(nonce, config, release=None, probe=None):
    """Return a readback guest's report carrying `config`, and `probe` when given."""
    digest = hashlib.sha256(config.encode()).hexdigest()
    lines = [
        f"{MARK}-BEGIN",
        f"{MARK}-NONCE {nonce}",
        f"{MARK}-PHASE readback",
        f"{MARK}-UNAME-R {release or PIN['kernel']['release']}",
        f"{MARK}-UNAME-V #lusoris1 SMP PREEMPT_RT",
    ]
    if probe is not None:
        lines += [f"{MARK}-PROBE-BEGIN", *probe, f"{MARK}-PROBE-END", f"{MARK}-PROBE-STATUS 0"]
    lines += [
        f"{MARK}-CONFIG-SHA256 {digest}",
        f"{MARK}-CONFIG-BEGIN",
        *config.splitlines(),
        f"{MARK}-CONFIG-END",
        f"{MARK}-END",
    ]
    return "\n".join(lines) + "\n"


def bzimage(payload, setup_sects=1):
    """Return a minimal x86 boot-protocol image carrying `payload` as its kernel."""
    header = bytearray(1024 * (setup_sects + 1))
    header[0x1F1] = setup_sects
    header[0x202:0x206] = b"HdrS"
    header[0x206:0x208] = (0x020F).to_bytes(2, "little")
    header[0x248:0x24C] = (0).to_bytes(4, "little")
    header[0x24C:0x250] = len(payload).to_bytes(4, "little")
    start = ((setup_sects or 4) + 1) * 512
    image = bytearray(header[:start].ljust(start, b"\0"))
    return bytes(image) + payload


class PinTests(unittest.TestCase):
    """The tracked pin names the release the milestone recorded."""

    def test_the_tracked_pin_is_complete(self):
        self.assertEqual(gate.pin_problems(PIN), [])
        self.assertEqual(PIN["tag"], "v7.2.8-realtime-lusoris1")
        self.assertEqual(PIN["revision"], "9e050cf8fc2fb62de943229f22ab104dce455e92")
        self.assertEqual(PIN["kernel"]["release"], "7.2.8-lusoris1-realtime")
        self.assertEqual(len(gate.asset_rows(PIN)), 3 + len(PIN["artifacts"]))

    def test_the_signer_is_an_exact_identity_under_the_producer(self):
        signer = PIN["signer"]
        self.assertTrue(signer["identity"].startswith(signer["identity-prefix"]))
        self.assertTrue(signer["identity"].endswith(f"@refs/tags/{PIN['tag']}"))
        self.assertEqual(signer["issuer"], "https://token.actions.githubusercontent.com")

    def test_a_config_digest_that_is_not_the_config_asset_is_refused(self):
        """Negative: kernel.config_digest must be the pinned configuration's sha256."""
        broken = json.loads(json.dumps(PIN))
        broken["kernel"]["config_digest"] = "sha256:" + "0" * 64
        self.assertTrue(any("config_digest" in line for line in gate.pin_problems(broken)))

    def test_an_image_that_is_not_an_asset_and_a_short_digest_are_refused(self):
        broken = json.loads(json.dumps(PIN))
        broken["kernel"]["image"] = "vmlinuz-other"
        broken["artifacts"][0]["sha256"] = "abc"
        problems = gate.pin_problems(broken)
        self.assertTrue(any("kernel.image" in line for line in problems))
        self.assertTrue(any("64 lowercase hex" in line for line in problems))

    def test_another_schema_is_refused_outright(self):
        self.assertEqual(len(gate.pin_problems(dict(PIN, schema="other"))), 1)


class PreBootRefusalTests(unittest.TestCase):
    """E10-4 negative: the hash that precedes every boot refuses any other image."""

    def row_for(self, data):
        return {
            "name": "vmlinuz-test",
            "sha256": hashlib.sha256(data).hexdigest(),
            "size": len(data),
        }

    def test_the_pinned_bytes_are_admitted(self):
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "vmlinuz-test"
            path.write_bytes(b"kernel image bytes")
            self.assertEqual(gate.verified_image(path, self.row_for(b"kernel image bytes")), path)

    def test_one_flipped_byte_is_refused_by_digest(self):
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "vmlinuz-test"
            path.write_bytes(b"kernel imagE bytes")
            with self.assertRaises(gate.Refused) as caught:
                gate.verified_image(path, self.row_for(b"kernel image bytes"))
        self.assertIn("sha256", str(caught.exception))

    def test_one_byte_more_is_refused_by_size_before_hashing(self):
        """Boundary: a size one byte off is its own refusal."""
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "vmlinuz-test"
            path.write_bytes(b"kernel image bytes!")
            problems = gate.file_problems(path, self.row_for(b"kernel image bytes"))
        self.assertEqual(len(problems), 1)
        self.assertIn("19 bytes", problems[0])

    def test_the_boot_path_refuses_before_the_emulator_starts(self):
        """The negative is structural: guarded_boot raises and boot() is never called."""
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            tree = Path(base) / "tree"
            tree.mkdir()
            with mock.patch.object(harness, "archive_tree", return_value=Path(base) / "x.cpio"):
                with mock.patch.object(harness, "boot") as boot:
                    with self.assertRaises(gate.GateError):
                        gate.guarded_boot(context, Path(base), tree)
        boot.assert_not_called()


class ManifestTests(unittest.TestCase):
    """The downloaded manifest and imago's result must both say what the pin says."""

    def manifest(self):
        return {
            "schema": gate.MANIFEST_SCHEMA,
            "provider": PIN["provider"],
            "stream": PIN["stream"],
            "version": PIN["version"],
            "kernel": {
                "release": PIN["kernel"]["release"],
                "config_digest": PIN["kernel"]["config_digest"],
            },
            "artifacts": PIN["artifacts"],
            "checksums": {"file": "SHA256SUMS", "sha256": PIN["checksums"]["sha256"]},
            "provenance": {
                "tag": PIN["tag"],
                "revision": PIN["revision"],
                "signer_identity": PIN["signer"]["identity"],
            },
        }

    def verification(self):
        return {
            "kernel_release": PIN["kernel"]["release"],
            "config_digest": PIN["kernel"]["config_digest"],
            "artifact_digest": f"sha256:{PIN['checksums']['sha256']}",
            "artifacts": PIN["artifacts"],
            "provenance": {"revision": PIN["revision"], "tag": PIN["tag"]},
            "bundle_present": True,
        }

    def test_the_pinned_manifest_and_result_agree(self):
        self.assertEqual(gate.manifest_problems(PIN, self.manifest()), [])
        self.assertEqual(gate.verification_problems(PIN, self.verification()), [])

    def test_another_revision_is_reported(self):
        manifest = self.manifest()
        manifest["provenance"]["revision"] = "0" * 40
        self.assertEqual(len(gate.manifest_problems(PIN, manifest)), 1)
        result = self.verification()
        result["provenance"]["revision"] = "0" * 40
        self.assertEqual(len(gate.verification_problems(PIN, result)), 1)

    def test_an_artifact_list_one_entry_short_is_reported(self):
        """Boundary: every artifact is compared, not only the kernel image."""
        manifest = self.manifest()
        manifest["artifacts"] = PIN["artifacts"][:-1]
        self.assertEqual(
            gate.manifest_problems(PIN, manifest),
            ["the manifest's artifact list is not the pinned one"],
        )

    def test_imago_output_that_is_not_a_json_object_is_a_problem_not_a_crash(self):
        """Negative and boundary: text, and JSON that is not an object, are each reported."""
        problems = []
        self.assertEqual(gate.imago_result("Error: not json", problems), {})
        self.assertEqual(gate.imago_result("[1, 2]", problems), {})
        self.assertEqual(len(problems), 2)
        self.assertEqual(gate.imago_result('{"stream": "realtime"}', []), {"stream": "realtime"})

    def test_imago_is_run_with_every_expectation_the_pin_records(self):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            argv = gate.imago_argv(context, gate.contract_store(), context.release)
        self.assertEqual(argv[1:4], ["kernel", "artifact", "verify"])
        for flag, value in (("--expect-stream", PIN["stream"]), ("--expect-tag", PIN["tag"])):
            self.assertEqual(argv[argv.index(flag) + 1], value)
        self.assertEqual(argv[argv.index("--expect-version") + 1], PIN["version"])
        self.assertIn("--json", argv)

    def test_cosign_verifies_offline_against_the_exact_identity(self):
        argv = gate.cosign_argv(Path("/cache/release"), PIN, PIN["signer"]["identity"])
        self.assertIn("--offline", argv)
        self.assertEqual(argv[argv.index("--certificate-identity") + 1], PIN["signer"]["identity"])
        self.assertNotIn("--certificate-identity-regexp", argv)
        self.assertEqual(argv[argv.index("--certificate-github-workflow-sha") + 1], PIN["revision"])
        self.assertEqual(
            argv[argv.index("--certificate-github-workflow-ref") + 1], f"refs/tags/{PIN['tag']}"
        )
        online = gate.cosign_argv(Path("/cache/release"), PIN, PIN["signer"]["identity"], False)
        self.assertNotIn("--offline", online)


class ReleaseBoundaryTests(unittest.TestCase):
    """E10-4 boundary: the release floor, evaluated as the gate does."""

    def test_the_release_below_the_floor_is_derived_not_written(self):
        self.assertEqual(gate.release_below("6.12"), "6.11.999")
        self.assertEqual(gate.release_below("7.0"), "6.999")
        self.assertIsNone(gate.release_below("0"))

    def test_the_boundary_case_passes_on_the_tracked_requirement(self):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            passed, out = quiet(gate.release_case, context)
        self.assertTrue(passed, out)
        self.assertIn("6.12: admitted", out)
        self.assertIn("6.11.999: refused", out)


class GuestReportTests(unittest.TestCase):
    """The report a guest prints is parsed strictly, and only this boot's report counts."""

    def test_fields_and_sections_are_read(self):
        text = readback_report("aegis-1-abc", "CONFIG_A=y\n")
        self.assertEqual(gate.guest_field(text, "NONCE"), "aegis-1-abc")
        self.assertEqual(gate.config_text(text), "CONFIG_A=y\n")

    def test_a_missing_field_or_an_unterminated_section_is_a_gate_error(self):
        with self.assertRaises(gate.GateError):
            gate.guest_field("nothing here\n", "NONCE")
        with self.assertRaises(gate.GateError):
            gate.guest_section(f"{MARK}-CONFIG-BEGIN\nCONFIG_A=y\n", "CONFIG")

    def test_another_boots_nonce_fails_the_identity(self):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            problems, _release = gate.identity_problems(
                context, readback_report("old", "C=y\n"), "new"
            )
        self.assertTrue(any("not this boot's" in line for line in problems))

    def test_the_guest_config_must_be_the_published_one(self):
        """Negative: a configuration other than the manifest's fails the identity case."""
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            text = readback_report("n1", "CONFIG_A=y\n")
            passed, out = quiet(gate.guest_identity_case, context, text, "n1")
        self.assertFalse(passed)
        self.assertIn("the manifest records", out)

    def test_a_log_decodes_and_a_damaged_one_is_reported(self):
        self.assertEqual(gate.decode_log(packed_log("# run-nonce x\n"))[0], "# run-nonce x\n")
        self.assertIsNotNone(gate.decode_log(["!!!not base64"])[1])
        self.assertEqual(gate.decode_log([f"{MARK}-LOG-ABSENT"])[0], None)


class CapabilityTests(unittest.TestCase):
    """Criterion 6: decided from the guest; the host is printed, never substituted."""

    def run_case(self, guest, host):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            return quiet(
                gate.capability_case, context, gate.parse_probe(guest), gate.parse_probe(host)
            )

    def test_a_guest_with_all_three_passes(self):
        passed, out = self.run_case(probe_lines(), probe_lines())
        self.assertTrue(passed, out)
        self.assertIn(
            "CONFIG_SCHED_CLASS_EXT: guest y, host y, reference profile y "
            "(same as the host; decided from the guest)",
            out,
        )

    def test_a_host_without_a_capability_is_printed_as_the_hosts_own_value(self):
        """Boundary: the guest passes on its own reading while the host column differs."""
        passed, out = self.run_case(probe_lines(), probe_lines(btf="absent"))
        self.assertTrue(passed, out)
        self.assertIn(
            "/sys/kernel/btf/vmlinux: guest present, host absent, reference profile present "
            "(differs from the host",
            out,
        )

    def test_a_guest_without_bpf_in_its_lsm_list_fails_although_the_host_has_it(self):
        """Negative, E10-1's boundary: the host's capability is not assumed of the guest."""
        passed, out = self.run_case(probe_lines(lsm="capability,lockdown"), probe_lines())
        self.assertFalse(passed)
        self.assertIn("BPF LSM absent from the kernel under test fails M10", out)
        self.assertIn(
            "bpf in the active LSM list: guest no, host yes, reference profile yes "
            "(differs from the host; decided from the guest)",
            out,
        )
        self.assertIn("guest LSM list capability,lockdown; host capability,lockdown,bpf", out)

    def test_a_guest_without_sched_ext_is_recorded_as_a_nucleus_requirement_defect(self):
        """Boundary, E10-2: the record names the defect, not only the value."""
        passed, out = self.run_case(probe_lines(sched_ext="n"), probe_lines())
        self.assertFalse(passed)
        self.assertIn("Nucleus requirement defect", out)

    def test_the_trampoline_rows_are_recorded_and_do_not_decide(self):
        passed, out = self.run_case(probe_lines(tracer="n"), probe_lines())
        self.assertTrue(passed, out)
        self.assertIn("CONFIG_FUNCTION_TRACER (recorded, not required): guest not set, host y", out)

    def test_the_trampoline_row_the_requirement_names_is_labelled_required(self):
        """Positive (D107): the label follows build/kernel-requirement.json, and this
        case still passes, because the requirement case is the one that decides."""
        passed, out = self.run_case(probe_lines(tracer="n"), probe_lines())
        self.assertTrue(passed, out)
        self.assertIn(
            "CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS (required; decided by the requirement "
            "case): guest absent from the configuration, host y",
            out,
        )

    def test_a_requirement_without_the_row_labels_it_recorded_only(self):
        """Boundary (D107): the payload of before D107, without the row, names neither."""
        symbol = "CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS"
        with tempfile.TemporaryDirectory() as base:
            requirement = context_in(base).requirement
        self.assertEqual(
            gate.trampoline_role(requirement, symbol), "required; decided by the requirement case"
        )
        before = dict(
            requirement,
            features=[row for row in requirement["features"] if row["symbol"] != symbol],
        )
        self.assertEqual(len(before["features"]), len(requirement["features"]) - 1)
        for name in gate.TRAMPOLINE_SYMBOLS:
            self.assertEqual(gate.trampoline_role(before, name), "recorded, not required")

    def test_the_reference_profile_values_are_read_from_the_register(self):
        self.assertEqual(
            gate.profile_values(),
            {
                "CONFIG_SCHED_CLASS_EXT": "y",
                "bpf in the active LSM list": "yes",
                "/sys/kernel/btf/vmlinux": "present",
            },
        )


class RequirementCaseTests(unittest.TestCase):
    """D94 as the gate runs it, on the configuration a guest printed."""

    def run_case(self, function, config):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            return quiet(function, context, readback_report("n", config))

    def test_a_conforming_guest_satisfies_the_requirement(self):
        passed, out = self.run_case(gate.requirement_case, conforming_config())
        self.assertTrue(passed, out)
        self.assertIn("CONFIG_KVM: required module, config m -> satisfied", out)

    def test_an_unsatisfied_guest_is_rejected_before_any_load(self):
        config = gate.planted(conforming_config(), "CONFIG_BPF_LSM", "CONFIG_BPF_LSM=n")
        passed, out = self.run_case(gate.requirement_case, config)
        self.assertFalse(passed)
        self.assertIn(
            "rejected: correlation-id aegis-m18-kernel-requirement-0001: CONFIG_BPF_LSM", out
        )

    def test_a_guest_without_the_trampoline_row_is_rejected_before_any_load(self):
        """Negative (D107): a configuration without DYNAMIC_FTRACE_WITH_DIRECT_CALLS, the
        shape of v7.2.8-realtime-lusoris1's, fails the requirement, so nothing loads."""
        symbol = "CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS"
        config = gate.planted(conforming_config(), symbol, None)
        passed, out = self.run_case(gate.requirement_case, config)
        self.assertFalse(passed)
        self.assertIn(f"rejected: correlation-id aegis-m18-kernel-requirement-0001: {symbol}", out)

    def test_the_negative_and_boundary_cases_pass_on_a_conforming_guest(self):
        self.assertTrue(self.run_case(gate.requirement_negative_case, conforming_config())[0])
        self.assertTrue(self.run_case(gate.requirement_boundary_case, conforming_config())[0])

    def test_planting_replaces_removes_and_appends(self):
        """Boundary: a plant replaces in place, removes, or appends; it is never a no-op."""
        text = "CONFIG_A=y\nCONFIG_B=y\n"
        self.assertEqual(gate.planted(text, "CONFIG_A", "CONFIG_A=m"), "CONFIG_A=m\nCONFIG_B=y\n")
        self.assertEqual(gate.planted("# CONFIG_A is not set\n", "CONFIG_A", None), "")
        self.assertEqual(gate.planted(text, "CONFIG_C", "CONFIG_C=n"), text + "CONFIG_C=n\n")


class SignatureTests(unittest.TestCase):
    """cosign's verdict binds the identity and the provenance revision to the signature."""

    def cosign(self, accepts_any_revision=False):
        """Return a stand-in for harness.run that behaves as cosign against the pinned bundle."""
        calls = []

        def run(argv, timeout):
            calls.append(argv)
            identity = argv[argv.index("--certificate-identity") + 1]
            revision = argv[argv.index("--certificate-github-workflow-sha") + 1]
            good = identity == PIN["signer"]["identity"]
            good = good and (accepts_any_revision or revision == PIN["revision"])
            if good:
                return 0, "", "Verified OK"
            return 1, "", "Error: expected GitHub Workflow SHA not found in certificate"

        return run, calls

    def run_case(self, accepts_any_revision=False):
        run, calls = self.cosign(accepts_any_revision)
        with tempfile.TemporaryDirectory() as base, mock.patch.object(
            harness, "run", side_effect=run
        ):
            passed, out = quiet(gate.signature_case, context_in(base))
        return passed, out, calls

    def test_the_pinned_identity_and_revision_verify_and_both_others_are_refused(self):
        passed, out, calls = self.run_case()
        self.assertTrue(passed, out)
        self.assertEqual(len(calls), 3)
        self.assertIn(f"another revision {gate.other_revision(PIN['revision'])} refused", out)

    def test_a_signature_that_does_not_bind_the_revision_fails(self):
        """Negative: a verdict that ignores the workflow SHA leaves the revision unsigned."""
        passed, out, _calls = self.run_case(accepts_any_revision=True)
        self.assertFalse(passed)
        self.assertIn("accepted the bundle for revision", out)

    def test_the_refused_revision_differs_in_its_last_digit_only(self):
        """Boundary: the refusal is of an exact commit, not a prefix."""
        other = gate.other_revision(PIN["revision"])
        self.assertEqual(other[:-1], PIN["revision"][:-1])
        self.assertNotEqual(other, PIN["revision"])
        self.assertEqual(gate.other_revision("0" * 40), "0" * 39 + "1")


class OrderTests(unittest.TestCase):
    """The contract's order: nothing boots before the release stage passed, nothing loads
    before the readback stage passed, and the release record is on disk before any boot."""

    STAGE = ("assets_case", "signature_case", "imago_case", "mismatch_case", "release_case")

    def drive(self, failing=(), readback=True, record=None):
        calls = []

        def recorder(name, result):
            def call(*_arguments):
                calls.append(name)
                return result

            return call

        def readback_phase(context):
            calls.append("readback_phase")
            if record is not None:
                record.append((context.run_dir / gate.RELEASE_RECORD).is_file())
            return readback

        patches = [
            mock.patch.object(gate, name, side_effect=recorder(name, name not in failing))
            for name in self.STAGE
        ]
        patches.append(mock.patch.object(gate, "readback_phase", side_effect=readback_phase))
        patches.append(mock.patch.object(gate, "load_phase", side_effect=recorder("load", None)))
        with tempfile.TemporaryDirectory() as base, contextlib.ExitStack() as stack:
            for patch in patches:
                stack.enter_context(patch)
            quiet(gate.run_cases, context_in(base))
        return calls

    def test_every_stage_runs_in_order_when_every_case_passes(self):
        self.assertEqual(self.drive(), [*self.STAGE, "readback_phase", "load"])

    def test_any_failed_release_case_prevents_every_boot(self):
        """Negative: a failed positive, negative or boundary of E10-4 each stop the run."""
        for name in self.STAGE:
            calls = self.drive(failing=(name,))
            self.assertNotIn("readback_phase", calls, name)
            self.assertNotIn("load", calls, name)
        self.assertEqual(self.drive(failing=("assets_case",)), ["assets_case"])

    def test_a_failed_readback_prevents_every_load(self):
        self.assertEqual(self.drive(readback=False), [*self.STAGE, "readback_phase"])

    def test_the_release_record_is_on_disk_when_the_first_boot_starts(self):
        """Boundary: E10-4's values are persisted before the readback boot, not at the end."""
        seen = []
        self.drive(record=seen)
        self.assertEqual(seen, [True])

    def test_the_release_record_carries_the_recorded_values(self):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            context.recorded = {"kernel.release": PIN["kernel"]["release"]}
            context.outcomes = [{"case": "nucleus/assets-pinned", "problems": [], "notes": []}]
            path, _out = quiet(gate.record_release, context)
            record = json.loads(path.read_text(encoding="utf-8"))
        self.assertEqual(record["schema"], "aegis.m10.nucleus-release-record.v1")
        self.assertEqual(record["recorded"]["kernel.release"], PIN["kernel"]["release"])
        self.assertEqual(record["outcomes"][0]["case"], "nucleus/assets-pinned")


class ReadbackPhaseTests(unittest.TestCase):
    """The readback stage admits the load boot only when every one of its cases passed."""

    def run_phase(self, config, guest=None, identity=True, patches=()):
        text = readback_report("n", config, probe=guest or probe_lines())
        stack_patches = [
            mock.patch.object(gate, "stage_tree", return_value=(Path("t"), Path("s"))),
            mock.patch.object(gate, "guarded_boot", return_value=(text, "n")),
            mock.patch.object(gate, "host_probe", return_value=gate.parse_probe(probe_lines())),
            mock.patch.object(gate, "guest_identity_case", return_value=identity),
            *patches,
        ]
        with tempfile.TemporaryDirectory() as base, contextlib.ExitStack() as stack:
            for patch in stack_patches:
                stack.enter_context(patch)
            context = context_in(base)
            return quiet(gate.readback_phase, context)

    def test_a_conforming_guest_admits_the_load_boot(self):
        passed, out = self.run_phase(conforming_config())
        self.assertTrue(passed, out)

    def test_a_requirement_rejection_withholds_the_load_boot(self):
        """Negative, D94: an unsatisfied row stops the run before any M19 object loads."""
        config = gate.planted(conforming_config(), "CONFIG_BPF_LSM", "CONFIG_BPF_LSM=n")
        # The negative and boundary cases pass here, so the positive alone decides.
        others = tuple(
            mock.patch.object(gate, name, return_value=True)
            for name in ("requirement_negative_case", "requirement_boundary_case")
        )
        passed, out = self.run_phase(config, patches=others)
        self.assertFalse(passed)
        self.assertIn("FAIL nucleus/requirement-satisfied-before-any-load", out)
        self.assertTrue(self.run_phase(conforming_config(), patches=others)[0])

    def test_a_guest_without_a_capability_or_its_identity_withholds_the_load_boot(self):
        self.assertFalse(self.run_phase(conforming_config(), probe_lines(sched_ext="n"))[0])
        self.assertFalse(self.run_phase(conforming_config(), identity=False)[0])

    def test_a_failed_requirement_negative_or_boundary_withholds_the_load_boot(self):
        """Boundary: the D94 negative and boundary gate the loads as the positive does."""
        for name in ("requirement_negative_case", "requirement_boundary_case"):
            patch = mock.patch.object(gate, name, return_value=False)
            self.assertFalse(self.run_phase(conforming_config(), patches=(patch,))[0], name)

    def test_a_rejecting_guest_loads_nothing_through_run_cases(self):
        config = gate.planted(conforming_config(), "CONFIG_KVM", "CONFIG_KVM=y")
        text = readback_report("n", config, probe=probe_lines())
        patches = (
            mock.patch.object(gate, "release_stage", return_value=True),
            mock.patch.object(gate, "stage_tree", return_value=(Path("t"), Path("s"))),
            mock.patch.object(gate, "guarded_boot", return_value=(text, "n")),
            mock.patch.object(gate, "host_probe", return_value=gate.parse_probe(probe_lines())),
            mock.patch.object(gate, "guest_identity_case", return_value=True),
            mock.patch.object(gate, "load_phase"),
        )
        with tempfile.TemporaryDirectory() as base, contextlib.ExitStack() as stack:
            mocks = [stack.enter_context(patch) for patch in patches]
            _result, out = quiet(gate.run_cases, context_in(base))
        mocks[-1].assert_not_called()
        self.assertIn("no M19 object was loaded", out)


class LoadPhaseTests(unittest.TestCase):
    """The load guest's own configuration check decides whether any case is judged."""

    class Loaded(Exception):
        """Raised by the stand-in for guest_case: a case was about to be judged."""

    def run_phase(self, check):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            text = "\n".join(
                (
                    f"{MARK}-NONCE n",
                    f"{MARK}-UNAME-R {PIN['kernel']['release']}",
                    f"{MARK}-CONFIG-CHECK {check}",
                )
            )
            patches = (
                mock.patch.object(gate, "build_objects", return_value={}),
                mock.patch.object(gate, "stage_load", return_value=Path("t")),
                mock.patch.object(gate, "guarded_boot", return_value=(text + "\n", "n")),
                mock.patch.object(gate, "guest_case", side_effect=self.Loaded),
            )
            with contextlib.ExitStack() as stack:
                for patch in patches:
                    stack.enter_context(patch)
                try:
                    quiet(gate.load_phase, context)
                    loaded = False
                except self.Loaded:
                    loaded = True
            return loaded, context.outcomes

    def test_a_matching_configuration_goes_on_to_the_cases(self):
        loaded, outcomes = self.run_phase("match")
        self.assertTrue(loaded)
        self.assertEqual(outcomes[0]["problems"], [])

    def test_a_mismatching_configuration_judges_no_case(self):
        """Negative: the guest's CONFIG-CHECK is load-bearing, not only printed."""
        loaded, outcomes = self.run_phase("mismatch; no object was loaded")
        self.assertFalse(loaded)
        self.assertEqual(outcomes[0]["case"], "nucleus/load-guest-config-matches-checked")
        self.assertTrue(any("nothing was loaded" in line for line in outcomes[0]["problems"]))


class ToolchainBoundTests(unittest.TestCase):
    """cosign is read back and held to the admitted line, whichever copy PATH names."""

    COSIGN = next(row for row in gate.TOOLCHAIN if row[0] == "cosign")

    def test_the_admitted_version_is_admitted(self):
        self.assertEqual(gate.bound_reasons("cosign", "2.6.3", self.COSIGN[3]), [])
        self.assertEqual(gate.bound_reasons("cosign", "2.99.99", self.COSIGN[3]), [])

    def test_the_other_copy_on_the_reference_profile_is_refused(self):
        """Negative: /usr/bin/cosign 3.1.3 is not the admitted cosign."""
        with mock.patch.object(gate.shutil, "which", return_value="/usr/bin/cosign"):
            with mock.patch.object(
                harness, "read_version", return_value=("GitVersion: v3.1.3", "3.1.3")
            ):
                reasons, _out = quiet(gate.check_toolchain, [self.COSIGN])
                fetched, out = quiet(gate.fetch, Path("/nonexistent"))
        self.assertEqual(len(reasons), 1)
        self.assertIn("cosign 3.1.3 is not admitted", reasons[0])
        self.assertEqual(fetched, 0)
        self.assertIn("the Nucleus kernel fetch did not run", out)

    def test_the_ceiling_is_exclusive_and_the_floor_inclusive(self):
        """Boundary: 3.0.0 is refused, 2.6.3 admitted, 2.6.2 refused."""
        self.assertEqual(len(gate.bound_reasons("cosign", "3.0.0", "2.6.3")), 1)
        self.assertIn("below the admitted floor", gate.bound_reasons("cosign", "2.6.2", "2.6.3")[0])
        self.assertEqual(gate.CEILINGS, {"cosign": "3"})


class BzImageTests(unittest.TestCase):
    """The kernel's own BTF is read out of the image the boot protocol describes."""

    def test_the_payload_is_located_by_the_setup_header(self):
        elf = b"\x7fELF" + b"\0" * 60
        self.assertEqual(gate.bzimage_payload(bzimage(gzip.compress(elf)))[:2], b"\x1f\x8b")
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "bzImage"
            path.write_bytes(bzimage(gzip.compress(elf)))
            self.assertEqual(gate.extract_vmlinux(path), elf)

    def test_an_image_without_the_header_or_with_another_compressor_is_refused(self):
        with self.assertRaises(gate.GateError):
            gate.bzimage_payload(b"\0" * 4096)
        with self.assertRaises(gate.GateError):
            gate.bzimage_payload(bzimage(b"\x28\xb5\x2f\xfd zstd"))

    def test_zero_setup_sectors_means_four(self):
        """Boundary: the boot protocol reads setup_sects 0 as 4."""
        payload = gzip.compress(b"\x7fELF")
        self.assertEqual(gate.bzimage_payload(bzimage(payload, setup_sects=0)), payload)


class GuestLoadTests(unittest.TestCase):
    """The load guest's case list and the checks applied to what it printed."""

    def test_every_load_runs_under_the_m19_capability_set_as_an_unprivileged_uid(self):
        argv = gate.guest_capability_argv()
        with mock.patch.object(bpf, "host_uid", return_value=(1000, None)), mock.patch.object(
            bpf, "host_gid", return_value=(1000, None)
        ):
            host = bpf.capability_argv()
        self.assertEqual(argv[0], "/bin/setpriv")
        self.assertEqual(argv[3:], host[5:])
        self.assertEqual(argv[1:3], ["--reuid=65534", "--regid=65534"])

    def test_the_case_script_names_every_case_once_with_its_own_nonce(self):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            script = gate.cases_script(context)
        lines = [shlex.split(line) for line in script.splitlines() if not line.startswith("#")]
        self.assertEqual([line[1] for line in lines], [case[0] for case in gate.guest_cases()])
        self.assertEqual(len(set(context.nonces.values())), len(lines))
        for line in lines:
            self.assertEqual(line[3:11], gate.guest_capability_argv())

    def test_the_scheduler_budget_and_the_missing_tracepoint_are_passed(self):
        cases = {case[0]: case for case in gate.guest_cases()}
        self.assertIn("cake_tier_budget=4", cases[gate.CASE_SOPS][4])
        self.assertIn(gate.MISSING_TRACEPOINT, cases[gate.CASE_TP_MISSING][4])
        self.assertEqual(gate.cake_tiers(), 4)

    def results(self, case, output, status, log):
        text = "\n".join(framed_case(case, output, status, log)) + "\n"
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            context.nonces[case] = "aegis-1-n"
            return context, {case: gate.guest_case(context, text, case)}

    def test_a_missing_tracepoint_reported_with_its_errno_passes(self):
        context, results = self.results(
            gate.CASE_TP_MISSING,
            f"load_rc=0\ntracepoint_attach_failed tracepoint={gate.MISSING_TRACEPOINT} errno=2 (x)",
            2,
            "# run-nonce aegis-1-n\n",
        )
        self.assertTrue(quiet(gate.missing_tracepoint_case, context, results)[0])

    def test_a_missing_tracepoint_that_was_ignored_fails(self):
        """Negative: exit 0 without the report line is an ignored tracepoint."""
        context, results = self.results(
            gate.CASE_TP_MISSING, "load_rc=0", 0, "# run-nonce aegis-1-n\n"
        )
        self.assertFalse(quiet(gate.missing_tracepoint_case, context, results)[0])

    def test_an_attach_failure_without_its_named_report_line_fails(self):
        """Boundary: exit 2 alone, or a report with errno 0, does not name the failure."""
        for output in ("load_rc=0\nsomething else failed", f"load_rc=0\n{self.missing_line(0)}"):
            context, results = self.results(gate.CASE_TP_MISSING, output, 2, "x")
            passed, out = quiet(gate.missing_tracepoint_case, context, results)
            self.assertFalse(passed, output)
            self.assertIn("printed no tracepoint_attach_failed", out)

    def missing_line(self, errno):
        """Return the loader's report line for the missing tracepoint with `errno`."""
        return f"tracepoint_attach_failed tracepoint={gate.MISSING_TRACEPOINT} errno={errno} (x)"

    def test_an_idle_cgroup_zero_delta_is_recorded_as_a_value(self):
        """Boundary: zero is a value; a missing reading or a moved one fails."""
        rows = (
            "power_begin cgroup_id=24 runtime_ns=0 pseudo_energy_uj_unmeasured=0\n"
            "power_end cgroup_id=24 runtime_ns=0 pseudo_energy_uj_unmeasured=0\n"
        )
        context, results = self.results(gate.CASE_TP, rows, 0, "# run-nonce aegis-1-n\n")
        text = f"{MARK}-IDLE-CGROUP {gate.CASE_TP} 24\n"
        passed, out = quiet(gate.idle_case, context, text, results)
        self.assertTrue(passed, out)
        self.assertEqual(context.recorded["kepler.idle_delta"]["pseudo_energy_uj_unmeasured"], 0)
        moved = rows.replace(
            "power_end cgroup_id=24 runtime_ns=0", "power_end cgroup_id=24 runtime_ns=5"
        )
        context, results = self.results(gate.CASE_TP, moved, 0, "x")
        passed, out = quiet(gate.idle_case, context, text, results)
        self.assertFalse(passed)
        self.assertIn("the interval was not idle", out)
        context, results = self.results(gate.CASE_TP, "power_begin_rows=0\n", 0, "x")
        passed, out = quiet(gate.idle_case, context, text, results)
        self.assertFalse(passed)
        self.assertIn("no reading at both ends", out)

    def test_an_ebusy_attach_names_the_trampoline_prerequisites_only_when_both_were_seen(self):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            context.guest_probe = gate.parse_probe(probe_lines(tracer="n"))
            result = {"output": "libbpf: prog 'x': failed to attach: -EBUSY"}
            self.assertIn(
                "CONFIG_FUNCTION_TRACER not set", gate.lsm_attach_reason(context, result)[0]
            )
            context.guest_probe = gate.parse_probe(probe_lines(tracer="y"))
            self.assertEqual(gate.lsm_attach_reason(context, result), [])
            self.assertEqual(gate.lsm_attach_reason(context, {"output": "attached=lsm"}), [])

    def test_the_scheduler_must_leave_by_user_space_unregistration(self):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            console = context.run_dir / "load" / "guest" / "console.log"
            console.parent.mkdir(parents=True)
            console.write_text(
                '[ 0.7] sched_ext: BPF scheduler "aegis_cake" enabled\n'
                '[31.1] sched_ext: BPF scheduler "aegis_cake" disabled (runnable task stall)\n'
            )
            self.assertNotEqual(gate.scheduler_exits(context), [gate.SCHEDULER_UNREGISTERED])
            console.write_text(f"[ 1.8] {gate.SCHEDULER_UNREGISTERED}\n")
            self.assertEqual(gate.scheduler_exits(context), [gate.SCHEDULER_UNREGISTERED])


class LoaderSourceTests(unittest.TestCase):
    """The M19 loader gained M10's modes without changing M19's."""

    def test_the_tracepoint_mode_reports_a_failed_attach_by_name(self):
        text = LOADER.read_text(encoding="utf-8")
        self.assertIn('strcmp(options->mode, "tp-attach") == 0', text)
        self.assertIn('"tracepoint_attach_failed tracepoint=%s errno=%d (%s)\\n"', text)
        self.assertIn('print_power_rows(map, "begin");', text)
        self.assertIn('print_power_rows(map, "end");', text)

    def test_the_map_name_bound_admits_the_scheduler_budget_map(self):
        """Boundary: a 16-byte bound refused cake_tier_budget, which is 16 bytes long."""
        bound = int(
            re.search(
                r"#define AEGIS_MAP_NAME_BOUND (\d+)", LOADER.read_text(encoding="utf-8")
            ).group(1)
        )
        self.assertGreater(bound, len(gate.CAKE_BUDGET_MAP))
        self.assertEqual(len(gate.CAKE_BUDGET_MAP), 16)

    def test_m19s_modes_are_unchanged(self):
        text = LOADER.read_text(encoding="utf-8")
        for mode in ("load", "lsm-probe", "sops-load", "sops-attach"):
            self.assertIn(f'strcmp(options->mode, "{mode}") == 0', text)


class GuestScriptTests(unittest.TestCase):
    """PID 1 of the M10 guest, and the probe it shares with the host."""

    def test_nothing_is_loaded_before_the_checked_configuration_is_matched(self):
        init = gate.GUEST_INIT.read_text(encoding="utf-8")
        self.assertLess(
            init.index('"$(/bin/cat "$STAGE/expected-config-sha256")"'),
            init.index('. "$STAGE/cases.sh"'),
        )
        self.assertIn("mismatch; no object was loaded", init)

    def test_tracefs_is_group_owned_by_the_loads_gid(self):
        init = gate.GUEST_INIT.read_text(encoding="utf-8")
        self.assertIn(f"gid={gate.GUEST_UID} tracefs /sys/kernel/tracing", init)

    def test_the_probe_reads_every_symbol_the_gate_compares(self):
        probe = gate.GUEST_PROBE.read_text(encoding="utf-8")
        for symbol in (
            "SCHED_CLASS_EXT",
            *[s.removeprefix("CONFIG_") for s in gate.TRAMPOLINE_SYMBOLS],
        ):
            self.assertIn(symbol, probe)
        self.assertIn("/sys/kernel/security/lsm", probe)
        self.assertIn("/sys/kernel/btf/vmlinux", probe)


class SkipTests(unittest.TestCase):
    """HISS-21: a host that cannot run the gate says why and exits 0."""

    def test_macos_and_windows_skip_with_their_platform_named(self):
        for platform in ("darwin", "win32"):
            with mock.patch.object(gate.sys, "platform", platform):
                reasons = gate.host_reasons(Path("/nonexistent"))
            self.assertEqual(len(reasons), 1)
            self.assertIn(platform, reasons[0])

    def test_a_host_without_kvm_skips_with_the_reason(self):
        with mock.patch.object(gate.sys, "platform", "linux"), mock.patch.object(
            gate.os, "access", return_value=False
        ):
            reasons = gate.host_reasons(Path("/nonexistent"))
        self.assertTrue(any("/dev/kvm" in reason for reason in reasons))
        self.assertTrue(any("nucleus-kernel-fetch" in reason for reason in reasons))

    def test_a_skip_prints_the_line_and_exits_zero(self):
        with mock.patch.object(gate, "host_reasons", return_value=["no KVM here"]):
            code, out = quiet(gate.main, [])
        self.assertEqual(code, 0)
        self.assertIn(f"SKIP: no KVM here; {gate.SKIP_LINE}", out)

    def test_a_present_but_wrong_run_fails(self):
        """Negative: a gate error is a FAIL with exit 1, never a skip."""
        with mock.patch.object(gate, "start", side_effect=gate.GateError("wrong cache")):
            code, out = quiet(gate.main, [])
        self.assertEqual(code, 1)
        self.assertIn("FAIL: the gate could not run: wrong cache", out)


class FetchTests(unittest.TestCase):
    """The one networked step keeps only bytes that hash to the pin."""

    def row(self, data):
        return {"name": "asset", "sha256": hashlib.sha256(data).hexdigest(), "size": len(data)}

    def fetch_with(self, served, row, cached=None):
        with tempfile.TemporaryDirectory() as base:
            release = Path(base)
            if cached is not None:
                (release / "asset").write_bytes(cached)

            def curl(argv, timeout):
                Path(argv[argv.index("--output") + 1]).write_bytes(served)
                return 0, "", ""

            with mock.patch.object(harness, "run", side_effect=curl) as run:
                try:
                    line = gate.fetch_asset(PIN, release, row)
                except gate.GateError as error:
                    line = f"refused: {error}"
            return line, run.call_count, sorted(path.name for path in release.iterdir())

    def test_matching_bytes_are_kept(self):
        line, calls, files = self.fetch_with(b"asset bytes", self.row(b"asset bytes"))
        self.assertIn("downloaded", line)
        self.assertEqual((calls, files), (1, ["asset"]))

    def test_other_bytes_are_refused_and_not_kept(self):
        line, _calls, files = self.fetch_with(b"other bytes", self.row(b"asset bytes"))
        self.assertIn("refused", line)
        self.assertEqual(files, [])

    def test_a_cached_copy_that_hashes_to_the_pin_is_not_downloaded_again(self):
        """Boundary: the fetch is idempotent over a correct cache."""
        line, calls, _files = self.fetch_with(b"x", self.row(b"asset bytes"), cached=b"asset bytes")
        self.assertEqual((calls, line), (0, "asset: cached, as pinned"))


class HostSurfaceTests(unittest.TestCase):
    """What the gate may start, and that every start carries a deadline."""

    def test_the_gate_starts_only_allowed_programs(self):
        resolved, undecided = call_sites(GATE)
        rows = {row[1][0] for row in gate.TOOLCHAIN}
        self.assertLessEqual(resolved, ALLOWED_PROGRAMS)
        self.assertEqual(sorted(undecided), sorted(name for name, _program in UNDECIDED))
        self.assertLessEqual({program for _name, program in UNDECIDED}, ALLOWED_PROGRAMS)
        self.assertNotIn("qemu-system-x86_64", resolved, "M10 must not build a second harness")
        self.assertIn("qemu-system-x86_64", rows)

    def test_the_builders_put_the_recorded_program_first(self):
        self.assertEqual(gate.cosign_argv(Path("/r"), PIN, "id")[0], "cosign")
        with tempfile.TemporaryDirectory() as base:
            argv = gate.imago_argv(context_in(base), gate.contract_store(), Path(base))
        self.assertEqual(Path(argv[0]).stem, "imago")

    def test_every_external_call_site_carries_a_deadline(self):
        total, deadlined = deadlined_call_sites(GATE)
        self.assertGreater(total, 0)
        self.assertEqual(total, deadlined)

    def test_the_guest_is_booted_through_m23s_harness(self):
        source = GATE.read_text(encoding="utf-8")
        self.assertIn("harness.boot(image, initramfs, base, nonce)", source)
        self.assertIn("harness.archive_tree(tree,", source)
        for flag in ('"-kernel"', '"-initrd"', '"-append"'):
            self.assertNotIn(flag, source, "the emulator argument vector is M23's alone")


class AdmissionTests(unittest.TestCase):
    """The page and the gate state the same admission."""

    def admitted(self):
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
            found[name.split()[0]] = (reference, None if floor.startswith("none") else floor)
        return found

    def test_every_toolchain_row_is_admitted_with_its_reference_and_floor(self):
        page = self.admitted()
        self.assertEqual(set(page), {row[0] for row in gate.TOOLCHAIN})
        for name, _argv, _pattern, floor, reference in gate.TOOLCHAIN:
            self.assertEqual(page[name], (reference, floor), name)

    def test_every_ceiling_is_stated_on_the_tools_row(self):
        lines = ADMISSION.read_text(encoding="utf-8").splitlines()[:MAX_PAGE_LINES]
        for name, ceiling in gate.CEILINGS.items():
            rows = [line for line in lines if line.startswith(f"| {name} |")]
            self.assertEqual(len(rows), 1, name)
            self.assertIn(f"below {ceiling}", rows[0])

    def test_the_targets_are_outside_verify_all(self):
        text = MAKEFILE.read_text(encoding="utf-8").replace("\r\n", "\n")
        self.assertIn("verify-nucleus-kernel:\n\tpython3 tools/verify_nucleus_kernel.py\n", text)
        self.assertIn(
            "nucleus-kernel-fetch:\n\tpython3 tools/verify_nucleus_kernel.py --fetch\n", text
        )
        recipe = text.split("verify-all:\n", 1)[1].split("\n\n", 1)[0]
        self.assertNotIn("nucleus", recipe)


if __name__ == "__main__":
    unittest.main()
