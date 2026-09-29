#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative and boundary coverage for the M09 contract pair gate (HISS-15).

The gate needs go, git and a cache that `make contract-fetch` filled. These
tests need none of them, and they run on every leg of the platform matrix
(HISS-21): they cover each decision the gate makes on the way -- the pin and
this repository's payloads against it, the provenance read out of
`go version -m`, the build request an accepted payload must print, what counts
as a correlated refusal, the tampered and bounded payloads it writes, the
identity record and the cache states that skip and the ones that fail, the
checkouts that must hold nothing but the pinned commits, the environment every
command runs in, the digest the fetch records, and what nucleus's report must
say for each row (D106) -- and the surfaces the gate runs through: the
Makefile, CI, the admission page and the evidence page. The recorded outputs
below are what imago 16f964b printed on the reference profile on 2026-09-28,
and what nucleus 82aa6b7's verifier wrote on 2026-09-29, kept as bytes;
nucleus's reports are abridged by dropping fields and rows, never by adding or
changing one, and a variant nucleus did not write is built inside its test and
named synthetic there. The sweeps at the end hold the gate to HISS-02, HISS-04
and HISS-08.
"""

import ast
import contextlib
import io
import json
import os
import tempfile
import unittest
from pathlib import Path, PureWindowsPath
from unittest import mock

import verify_contract_pair as gate
from test_bpf_objects import recipe_lines, suppresses

ROOT = Path(__file__).resolve().parent.parent
GATE = ROOT / "tools" / "verify_contract_pair.py"
MAKEFILE = ROOT / "Makefile"
CI = ROOT / ".github" / "workflows" / "ci.yml"
ADMISSION = ROOT / "docs" / "roadmap" / "toolchain-admission.md"
EVIDENCE_PAGE = ROOT / "docs" / "build" / "contract-pair.md"
MKDOCS = ROOT / "mkdocs.yml"
ATTRIBUTES = ROOT / ".gitattributes"
ADMISSION_HEADING = "## The contract pair's toolchain and producer pins (M09)"
MAX_FUNCTION_LINES = 60
MAX_COMPLEXITY = 10
MAX_STATEMENTS = 50
BRANCHES = (ast.If, ast.For, ast.While, ast.IfExp, ast.ExceptHandler, ast.With, ast.Assert)
# What the gate may start: git, go, the binary under test (imago or the stand-in)
# and the interpreter running it, for nucleus's stdlib-only verifier.
ALLOWED_PROGRAMS = {"git", "go", "python3", "<binary>"}
# The only functions that may reach the network, and the commands that do.
NETWORK_FUNCTIONS = {"fetch_identity", "fetch_checkout", "fetch_build"}
PIN_COMMIT = "16f964b4dafadac2b1f0a662c7dcbb4b7bb29bee"
NUCLEUS_COMMIT = "82aa6b7a3c68a42a6330370c81ec642482014c9f"
# The pinned commit's committer time as go writes vcs.time, and as `git log
# --format=%ct` prints it.
COMMITTED = "2026-09-16T13:01:55Z"
COMMITTED_SECONDS = "1789563715"
PSEUDO_VERSION = "v0.0.0-20260916130155-16f964b4dafa"

# `go version -m` of the pinned build, abridged to three of its dep rows.
BUILDINFO = (
    "/cache/bin/imago: go1.27.1-X:nodwarf5\n"
    "\tpath\tgithub.com/cordanaLLM/imago/cmd/imago\n"
    "\tmod\tgithub.com/cordanaLLM/imago\tv0.0.0-20260916130155-16f964b4dafa\t\n"
    "\tdep\tgithub.com/spf13/cobra\tv1.10.2\th1:DMTTonx5m65Ic0GOoRY2c16WCbHxOOw6xxezuLaBpcU=\n"
    "\tdep\tgo.uber.org/fx\tv1.24.0\th1:wE8mruvpg2kiiL1Vqd0CC+tr0/24XIB10Iwp2lLWzkg=\n"
    "\tdep\tgolang.org/x/sys\tv0.48.0\th1:bbX/i/6MgT9BVLM9RT1thmxL04yeTAhbEz4SyadbXoo=\n"
    "\tbuild\t-buildmode=exe\n"
    "\tbuild\t-compiler=gc\n"
    "\tbuild\t-trimpath=true\n"
    "\tbuild\tCGO_ENABLED=0\n"
    "\tbuild\tGOARCH=amd64\n"
    "\tbuild\tGOOS=linux\n"
    "\tbuild\tvcs=git\n"
    f"\tbuild\tvcs.revision={PIN_COMMIT}\n"
    "\tbuild\tvcs.time=2026-09-16T13:01:55Z\n"
    "\tbuild\tvcs.modified=false\n"
)
# What `imago aegis validate build/product-input.json --json` printed.
ACCEPTED_STDOUT = (
    b'{\n  "correlation_id": "aegis-m18-product-input-0001",\n'
    b'  "revision": "61f2fe16bab7889e007b2fd1474b025023906e91",\n'
    b'  "distribution": {\n    "id": "arch",\n    "snapshot": "2026/09/13"\n  },\n'
    b'  "definitions": {\n    "repart": "build/repart.d",\n'
    b'    "sysupdate": "build/sysupdate.d",\n    "mkosi": "build/mkosi.conf",\n'
    b'    "kernel_requirement": "build/kernel-requirement.json"\n  },\n'
    b'  "packages": [\n    "linux-rt",\n    "systemd",\n    "systemd-ukify"\n  ],\n'
    b'  "kernel_source": "built-here",\n  "kernel_package": "linux-rt",\n'
    b'  "retry": {\n    "attempts": 3,\n    "backoff": "30s"\n  }\n}\n'
).decode("utf-8")
# The first lines `imago kernel requirement validate build/kernel-requirement.json` printed.
KERNEL_HEADER = (
    "✓ Kernel requirement aegis-m18-kernel-requirement-0001 is valid "
    "(aegis.p01-nucleus.kernel-requirement.v1).\n"
    "  Architectures: x86-64\n  ABI: minimum-release 6.12, target-release 7.3\n"
)
# What `imago` printed for the empty feature list: cobra and main each print it.
EMPTY_STDERR = (
    "Error: kernel requirement aegis-m18-kernel-requirement-0001: features: "
    "feature list is empty\n" * 2
)
# nucleus 82aa6b7's versions.json downstream.requirements, as the checkout holds it.
VERSIONS = (
    b'{"downstream": {"repository": "cordanaLLM/imago", "requirements": ['
    b'{"label": "imago", "repository": "cordanaLLM/imago", "path": "kernel/requirement.json",'
    b' "dispatched": true, "streams": ["bleeding", "mainstream", "lts", "realtime"]},'
    b'{"label": "aegis-os", "repository": "cordanaLLM/Aegis-OS",'
    b' "path": "build/kernel-requirement.json", "dispatched": false, "streams": ["realtime"]}'
    b"]}}"
)
# What `scripts/verify_kernel_requirement.py --report-json` wrote at 82aa6b7 for
# build/kernel-requirement.json, without its policy, abi, artifact or streams.
# Each report below is abridged by dropping fields, streams and rows only:
# nothing is added or changed. Gate run r20260929T102414-7f8a kept the full
# reports; a variant nucleus did not write is built inside its test and named
# synthetic there.
REPORT_HEAD = (
    b'{"schema": "nucleus.kernel-requirement-report.v1",'
    b' "nucleus_revision": "82aa6b7a3c68a42a6330370c81ec642482014c9f",'
    b' "evidence_level": "declared", '
)
PASS_REPORT = REPORT_HEAD + (
    b'"verdict": "PASS", "documents": [{"label": "aegis-os", "status": "PASS",'
    b' "source": {"label": "aegis-os", "repository": "cordanaLLM/Aegis-OS",'
    b' "path": "build/kernel-requirement.json", "ref": null,'
    b' "sha256": "d796c408b4db5dd9e5f22d35c109a8dccadcdb14f3f68ce7de2cddc594d47adb"},'
    b' "correlation_id": "aegis-m18-kernel-requirement-0001", "architectures": ["x86-64"],'
    b' "bound_streams": ["realtime"], "held": ["realtime"], "reasons": [], "streams": []}]}'
)
# ... and for the requirement with CONFIG_AEGIS_CONTRACT_UNSET planted, abridged.
FAIL_REPORT = REPORT_HEAD + (
    b'"verdict": "FAIL", "documents": [{"label": "aegis-os", "status": "FAIL",'
    b' "source": {"sha256": "706ba4ec256d9e0a6d8cb68cc5defbc8db9d74a1f2ef416edcb76ac414b9bcd9"},'
    b' "correlation_id": "aegis-m18-kernel-requirement-0001",'
    b' "bound_streams": ["realtime"], "held": [], "reasons": ["aegis-m18-kernel-requirement-0001:'
    b" realtime x86_64: CONFIG_AEGIS_CONTRACT_UNSET (required-by REQ-P07-01) requires built-in,"
    b' observed unrecorded"], "streams": [{"stream": "lts", "bound": false,'
    b' "architectures": [{"arch": "x86_64", "features": [{"symbol": "CONFIG_AEGIS_CONTRACT_UNSET",'
    b' "observed": null, "satisfied": false}]}]}, {"stream": "realtime", "bound": true,'
    b' "architectures": [{"token": "x86-64", "arch": "x86_64", "features": [{"symbol":'
    b' "CONFIG_PREEMPT_RT", "observed": "y", "satisfied": true}, {"symbol":'
    b' "CONFIG_AEGIS_CONTRACT_UNSET", "observed": null, "satisfied": false}]}]}]}]}'
)
# ... and for the empty feature list.
EMPTY_REPORT = REPORT_HEAD + (
    b'"verdict": "FAIL", "documents": [{"label": "aegis-os", "status": "REJECTED",'
    b' "source": {"sha256": "cfdcdf932ecec40229b07f68f1bfced4d78d45d6cba3d585fef5934aee67af31"},'
    b' "rejection": {"kind": "NoFeatures", "message": "the document lists no required feature;'
    b' an empty requirement is refused"}}]}'
)
# nucleus's main before D106, the commit a pre-D106 fetch recorded as pinned.
LS_REMOTE_MAIN = "8672247ff1bd22ed6b6b89d498116f20ff00c2ab"
LS_REMOTE = (
    "40c43718a6cd04884c0440d5048063e3648acfe5\trefs/heads/fix/uki-refuse-fabrication\n"
    "8672247ff1bd22ed6b6b89d498116f20ff00c2ab\trefs/heads/main\n"
    "b94f529bafe3be4d14ebd0b89a6fbfa29c9752d0\trefs/heads/release-please--branches--main\n"
)


def text(path):
    """Return a tracked file's text with LF line ends, whatever the checkout wrote."""
    return Path(path).read_bytes().decode("utf-8").replace("\r\n", "\n")


def committed_pin():
    """Return a fresh copy of the committed pin."""
    return json.loads(text(gate.PIN))


def result(code=0, stdout="", stderr=""):
    """Return an invocation result in the shape the gate's invoke() returns."""
    return {"exit": code, "stdout": stdout, "stderr": stderr}


def product_input():
    return json.loads(text(gate.PRODUCT_INPUT))


def kernel_requirement():
    return json.loads(text(gate.KERNEL_REQUIREMENT))


def kernel_stdout(requirement):
    """Return what imago prints for an accepted requirement, in its column layout."""
    rows = [
        f"    {f['symbol']:<32} {f['state']:<9} {f['probe']:<14} {f['required-by']}\n"
        for f in requirement["features"]
    ]
    header = KERNEL_HEADER.replace(
        "aegis-m18-kernel-requirement-0001", requirement["correlation-id"]
    )
    return header + f"  Features: {len(rows)}\n" + "".join(rows)


def quiet(function, *args, **kwargs):
    """Call `function` with stdout captured; return (result, printed text)."""
    buffer = io.StringIO()
    with contextlib.redirect_stdout(buffer):
        value = function(*args, **kwargs)
    return value, buffer.getvalue()


def context_in(directory, pin=None):
    """Return a gate Context whose cache and run directory sit in `directory`."""
    store = Path(directory) / "cache"
    return gate.Context(pin or gate.load_pin(), store, store / "runs" / "r-test")


def answering(*rows):
    """Return a stand-in for the gate's run(): the first row whose words all occur in argv
    answers with its (exit, stdout, stderr); any other command exits 0 silently."""

    def answer(argv, *args, **kwargs):
        for words, reply in rows:
            if all(word in argv for word in words):
                return reply
        return (0, "", "")

    return answer


def reporting(name, problems):
    """Return a stand-in for one identity case that reports `problems` under `name`."""
    return lambda context: gate.report(context, name, problems)


def filled_cache(directory, pinned=None):
    """Return a Context over a cache with every piece present, fetched for `pinned`."""
    context = context_in(directory)
    (context.checkout / ".git").mkdir(parents=True)
    (context.nucleus / ".git").mkdir(parents=True)
    context.binary.parent.mkdir(parents=True)
    context.binary.write_bytes(b"\x7fELF")
    pins = pinned or {name: context.pin[name]["commit"] for name in gate.PRODUCERS}
    record = {"producers": {name: {"pinned": commit} for name, commit in pins.items()}}
    context.identity.write_bytes(json.dumps(record).encode("utf-8"))
    return context


class PinTests(unittest.TestCase):
    """The committed pin loads, each defect is refused, and the payloads match it."""

    def test_the_committed_pin_loads(self):
        pin = gate.load_pin()
        self.assertEqual(pin["imago"]["commit"], PIN_COMMIT)
        self.assertEqual(pin["nucleus"]["commit"], NUCLEUS_COMMIT)
        self.assertEqual(pin["nucleus"]["label"], "aegis-os")
        self.assertEqual(pin["nucleus"]["bound-streams"], ["realtime"])
        self.assertEqual(pin["nucleus"]["evidence-level"], "declared")
        self.assertEqual(pin["imago"]["go"], "1.27.1")
        self.assertEqual(len(pin["imago"]["fixtures"]), 3)

    def test_this_repositorys_payloads_hash_to_the_pin(self):
        """Positive: the bytes imago vendors are the bytes this checkout carries."""
        self.assertEqual(gate.payload_problems(gate.load_pin()), [])

    def test_a_changed_payload_is_named(self):
        """Negative: a payload edited without a re-pin fails with the file and the cure."""
        with tempfile.TemporaryDirectory() as base:
            for row in committed_pin()["imago"]["fixtures"]:
                target = Path(base) / row["aegis"]
                target.parent.mkdir(parents=True, exist_ok=True)
                target.write_bytes((ROOT / row["aegis"]).read_bytes())
            (Path(base) / "build" / "product-input.json").write_bytes(b"{}\n")
            problems = gate.payload_problems(gate.load_pin(), root=base)
        self.assertEqual(len(problems), 1)
        self.assertIn("build/product-input.json hashes", problems[0])
        self.assertIn("re-pin", problems[0])

    def test_each_pin_defect_is_refused(self):
        defects = (
            (("schema",), "aegis.m09.contract-pin.v0", "schema"),
            (("imago", "repository"), "https://github.com/lusoris/imago.git", "repository"),
            (("nucleus", "commit"), PIN_COMMIT[:12], "40 lowercase hex"),
            (("imago", "commit"), PIN_COMMIT.upper(), "40 lowercase hex"),
            (("imago", "go"), "1.27", "x.y.z"),
            (("imago", "main-package"), "example.com/imago/cmd/imago", "main-package"),
            (("imago", "bounds", "MaxPackages"), 0, "positive integer"),
            (("imago", "bounds", "MaxRetryAttempts"), True, "positive integer"),
            (("imago", "bounds", "source"), "../pkg/aegis/aegis.go", "relative path"),
            (("imago", "fixtures"), [], "1..8"),
            (("nucleus", "verifier"), "../scripts/verify.py", "relative path"),
            (("nucleus", "label"), "Aegis OS", "versions.json label"),
            (("nucleus", "report-schema"), "nucleus.kernel-requirement-report.v2", "report-schema"),
            (("nucleus", "evidence-level"), "built", "evidence-level"),
            (("nucleus", "bound-streams"), [], "bound-streams"),
            (("nucleus", "bound-streams"), ["Realtime"], "bound-streams"),
        )
        for keys, value, needle in defects:
            with self.subTest(field=".".join(keys)):
                pin = committed_pin()
                target = pin
                for key in keys[:-1]:
                    target = target[key]
                target[keys[-1]] = value
                problems = gate.pin_problems(pin)
                self.assertTrue(any(needle in line for line in problems), problems)

    def test_a_fixture_row_is_checked_field_by_field(self):
        pin = committed_pin()
        pin["imago"]["fixtures"][0]["sha256"] = "0" * 63
        pin["imago"]["fixtures"][1]["imago"] = "/etc/passwd"
        problems = gate.pin_problems(pin)
        self.assertEqual(len(problems), 2, problems)

    def test_the_fixture_bound_is_inclusive(self):
        """Boundary: eight fixtures are admitted, nine are refused."""
        pin = committed_pin()
        row = pin["imago"]["fixtures"][0]
        pin["imago"]["fixtures"] = [dict(row) for _ in range(gate.MAX_FIXTURES)]
        self.assertEqual(gate.pin_problems(pin), [])
        pin["imago"]["fixtures"].append(dict(row))
        self.assertTrue(gate.pin_problems(pin))

    def test_the_stream_bound_is_inclusive(self):
        """Boundary: eight bound streams are admitted, nine are refused."""
        pin = committed_pin()
        pin["nucleus"]["bound-streams"] = [f"s{index}" for index in range(gate.MAX_STREAMS)]
        self.assertEqual(gate.pin_problems(pin), [])
        pin["nucleus"]["bound-streams"].append("one-more")
        self.assertTrue(gate.pin_problems(pin))

    def test_a_pin_that_is_not_json_is_a_gate_error(self):
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "pin.json"
            path.write_bytes(b"{not json")
            with self.assertRaises(gate.GateError):
                gate.load_pin(path)

    def test_the_payloads_keep_lf_on_every_platform(self):
        """The pin names bytes, so a checkout may not rewrite their line ends."""
        attributes = text(ATTRIBUTES)
        for row in committed_pin()["imago"]["fixtures"]:
            self.assertIn(f"{row['aegis']} text eol=lf", attributes)
            self.assertNotIn(b"\r\n", (ROOT / row["aegis"]).read_bytes())


class ProvenanceTests(unittest.TestCase):
    """The binary counts only when `go version -m` reads the pinned build out of it."""

    pin = gate.load_pin()

    def test_the_pinned_build_is_admitted(self):
        info = gate.parse_buildinfo(BUILDINFO)
        self.assertEqual(info["path"], "github.com/cordanaLLM/imago/cmd/imago")
        self.assertEqual(info["mod version"], PSEUDO_VERSION)
        self.assertEqual(info["build"]["vcs.revision"], PIN_COMMIT)
        self.assertEqual(gate.provenance_problems(info, self.pin, COMMITTED), [])

    def test_the_module_version_is_the_one_go_stamps_at_the_commit(self):
        self.assertEqual(gate.pseudo_version(COMMITTED, PIN_COMMIT), PSEUDO_VERSION)
        self.assertIsNone(gate.pseudo_version(None, PIN_COMMIT))

    def test_an_unread_commit_time_fails_closed(self):
        """Boundary: without the checkout's commit time, the rows that need it fail."""
        problems = gate.provenance_problems(gate.parse_buildinfo(BUILDINFO), self.pin, None)
        self.assertEqual(
            sorted(line.split(" is ")[0] for line in problems), ["mod version", "vcs.time"]
        )

    def test_each_departure_from_the_pinned_build_is_refused(self):
        cases = (
            (f"vcs.revision={PIN_COMMIT}", "vcs.revision=" + "a" * 40, "vcs.revision is"),
            ("vcs.modified=false", "vcs.modified=true", "vcs.modified is"),
            ("CGO_ENABLED=0", "CGO_ENABLED=1", "CGO_ENABLED is"),
            ("-trimpath=true", "-trimpath=false", "-trimpath is"),
            ("imago/cmd/imago\n", "imago/cmd/other\n", "path is"),
            ("vcs=git", "vcs=hg", "vcs is"),
            (COMMITTED, "2026-09-16T13:01:56Z", "vcs.time is"),
            (PSEUDO_VERSION, PSEUDO_VERSION + "+dirty", "mod version is"),
        )
        for old, new, needle in cases:
            with self.subTest(change=new):
                info = gate.parse_buildinfo(BUILDINFO.replace(old, new))
                problems = gate.provenance_problems(info, self.pin, COMMITTED)
                self.assertTrue(any(line.startswith(needle) for line in problems), problems)

    def test_a_build_without_vcs_stamping_is_refused(self):
        """Negative: -buildvcs=false leaves no revision, which is refused as absent."""
        stripped = "".join(line for line in BUILDINFO.splitlines(True) if "vcs" not in line)
        problems = gate.provenance_problems(gate.parse_buildinfo(stripped), self.pin, COMMITTED)
        self.assertIn("vcs.revision is None, the pinned build has " + repr(PIN_COMMIT), problems)

    def test_output_that_is_not_build_information_is_refused(self):
        for output in ("", "not a Go executable\n", "/bin/true: could not read Go build info"):
            with self.subTest(output=output):
                info = gate.parse_buildinfo(output)
                self.assertEqual(
                    gate.provenance_problems(info, self.pin, COMMITTED),
                    ["not a Go executable with build information"],
                )

    def test_the_go_floor_is_inclusive(self):
        """Boundary: go1.27.1 is admitted, go1.27.0 and go1.27 are refused, go1.28 admitted."""
        for version, admitted in (
            ("go1.27.1", True),
            ("go1.27.1-X:nodwarf5", True),
            ("go1.28", True),
            ("go1.27.0", False),
            ("go1.27", False),
            ("devel +abc", False),
        ):
            with self.subTest(version=version):
                info = gate.parse_buildinfo(BUILDINFO.replace("go1.27.1-X:nodwarf5", version))
                problems = gate.provenance_problems(info, self.pin, COMMITTED)
                self.assertEqual(not any("built by" in line for line in problems), admitted)

    def test_go_releases_are_read_with_and_without_a_patch(self):
        self.assertEqual(gate.go_release("go1.27.1-X:nodwarf5"), (1, 27, 1))
        self.assertEqual(gate.go_release("go1.27"), (1, 27, 0))
        self.assertIsNone(gate.go_release("devel go1.28-abc"))
        self.assertEqual(gate.floor_tuple("1.27.1"), (1, 27, 1))


class RequestTests(unittest.TestCase):
    """An accepted payload must be printed back field by field."""

    def test_the_recorded_output_is_the_payload_mapped(self):
        """Positive: what imago printed for the committed payload is what the gate expects."""
        self.assertEqual(json.loads(ACCEPTED_STDOUT), gate.build_request(product_input()))
        problems = gate.accepted_problems(result(0, ACCEPTED_STDOUT), product_input())
        self.assertEqual(problems, [])

    def test_a_changed_field_in_the_output_fails(self):
        """Negative: exit 0 alone is not an acceptance."""
        altered = ACCEPTED_STDOUT.replace('"attempts": 3', '"attempts": 4')
        self.assertTrue(gate.accepted_problems(result(0, altered), product_input()))
        self.assertTrue(gate.accepted_problems(result(0, "accepted"), product_input()))
        self.assertTrue(gate.accepted_problems(result(1, ACCEPTED_STDOUT), product_input()))
        stderr = "Error: aegis product-input x: y: z\n"
        self.assertTrue(gate.accepted_problems(result(0, ACCEPTED_STDOUT, stderr), product_input()))

    def test_durations_are_rendered_as_go_renders_them(self):
        """Boundary: each unit appears exactly when Go's Duration.String() writes it."""
        for seconds, rendered in (
            (0, "0s"),
            (59, "59s"),
            (60, "1m0s"),
            (3599, "59m59s"),
            (3600, "1h0m0s"),
            (3661, "1h1m1s"),
        ):
            with self.subTest(seconds=seconds):
                self.assertEqual(gate.go_duration(seconds), rendered)


class RefusalTests(unittest.TestCase):
    """A refusal is exit 1 and the payload's correlated error on stderr, never one alone."""

    reason = "features: feature list is empty"
    cid = "aegis-m18-kernel-requirement-0001"

    def problems(self, code, stdout="", stderr=EMPTY_STDERR):
        return gate.refused_problems(
            result(code, stdout, stderr), gate.KERNEL_PREFIX, self.cid, self.reason
        )

    def test_the_recorded_refusal_passes(self):
        self.assertEqual(self.problems(1), [])

    def test_an_exit_code_without_the_text_fails(self):
        self.assertTrue(self.problems(1, stderr="Error: something else\n"))
        self.assertTrue(self.problems(1, stdout=EMPTY_STDERR, stderr=""))

    def test_the_text_without_the_exit_code_fails(self):
        """Boundary: exit 0 is an acceptance and exit 2 is a crash; only 1 is a refusal."""
        self.assertTrue(self.problems(0))
        self.assertTrue(self.problems(2))

    def test_another_correlation_id_fails(self):
        other = EMPTY_STDERR.replace(self.cid, "aegis-m18-kernel-reference-0001")
        self.assertTrue(self.problems(1, stderr=other))


class KernelOutputTests(unittest.TestCase):
    """An accepted requirement must be listed feature by feature."""

    def test_every_feature_listed_passes(self):
        requirement = kernel_requirement()
        printed = kernel_stdout(requirement)
        self.assertEqual(gate.kernel_accepted_problems(result(0, printed), requirement), [])

    def test_a_missing_feature_or_count_fails(self):
        requirement = kernel_requirement()
        printed = kernel_stdout(requirement).splitlines(True)
        self.assertTrue(
            gate.kernel_accepted_problems(result(0, "".join(printed[:-1])), requirement)
        )
        wrong = "".join(printed).replace("Features: 13", "Features: 12")
        self.assertTrue(gate.kernel_accepted_problems(result(0, wrong), requirement))
        self.assertTrue(gate.kernel_accepted_problems(result(1, "".join(printed)), requirement))


class PayloadTests(unittest.TestCase):
    """The payloads the gate writes change exactly what their row names."""

    def test_each_tamper_changes_one_field(self):
        payload = product_input()
        rows = gate.product_tamper_rows(payload)
        self.assertEqual(len(rows), len(gate.PRODUCT_TAMPERS))
        for (label, document, reason), (_, path, value, _) in zip(rows, gate.PRODUCT_TAMPERS):
            with self.subTest(row=label):
                self.assertEqual(document["correlation-id"], payload["correlation-id"])
                target = document
                for key in path.split(".")[:-1]:
                    target = target[key]
                self.assertEqual(target[path.split(".")[-1]], value)
                self.assertNotEqual(document, payload)
                self.assertTrue(reason)
        self.assertEqual(payload, product_input(), "the source payload must stay untouched")

    def test_the_retry_rows_sit_on_and_one_above_the_bound(self):
        rows = gate.retry_rows(product_input(), 10)
        self.assertEqual([row[1]["retries"]["max-attempts"] for row in rows], [10, 11])
        self.assertIsNone(rows[0][2])
        self.assertEqual(rows[1][2], "retries.max-attempts: 11 outside 1..10")

    def test_the_package_rows_sit_on_and_one_above_the_bound(self):
        rows = gate.packages_rows(product_input(), 256)
        for (_, document, _), count in zip(rows, (256, 257)):
            packages = document["packages"]
            self.assertEqual(len(packages), count)
            self.assertEqual(len(set(packages)), count)
            self.assertIn(document["kernel"]["default-package"], packages)
        self.assertEqual(rows[1][2], "packages: count 257 outside 1..256")

    def test_a_package_count_below_the_payloads_own_is_not_padded(self):
        """Boundary: one package keeps the payload's first, the kernel package."""
        self.assertEqual(gate.with_packages(product_input(), 1)["packages"], ["linux-rt"])

    def test_the_aegis_maxima_are_read_from_the_crate(self):
        bounds = gate.rust_bounds(text(gate.AEGIS_MANIFEST_SOURCE))
        self.assertEqual(
            bounds, {"MAX_PACKAGES": 64, "MAX_RETRY_ATTEMPTS": 5, "MAX_BACKOFF_SECONDS": 3600}
        )
        document = gate.aegis_maxima(product_input(), bounds)
        self.assertEqual(len(document["packages"]), 64)
        self.assertEqual(document["retries"], {"max-attempts": 5, "backoff-seconds": 3600})

    def test_a_crate_without_a_bound_is_a_gate_error(self):
        with self.assertRaises(gate.GateError):
            gate.rust_bounds("pub const MAX_PACKAGES: usize = 64;\n")

    def test_an_aegis_bound_above_imagos_is_named(self):
        imago = committed_pin()["imago"]["bounds"]
        inside = {"MAX_PACKAGES": 256, "MAX_RETRY_ATTEMPTS": 10}
        self.assertEqual(gate.bounds_problems(inside, imago), [])
        above = {"MAX_PACKAGES": 257, "MAX_RETRY_ATTEMPTS": 10}
        self.assertEqual(len(gate.bounds_problems(above, imago)), 1)

    def test_the_kernel_rows(self):
        requirement = kernel_requirement()
        invalid = gate.kernel_invalid_rows(requirement)
        self.assertEqual([row[1]["features"][0]["state"] for row in invalid][:1], ["yes"])
        self.assertEqual(invalid[1][1]["features"][0]["probe"], "dmesg")
        duplicate = invalid[2][1]["features"]
        self.assertEqual(duplicate[0]["symbol"], duplicate[1]["symbol"])
        empty, single = gate.empty_rows(requirement)
        self.assertEqual(empty[1]["features"], [])
        self.assertEqual(empty[2], "features: feature list is empty")
        self.assertEqual(len(single[1]["features"]), 1)
        self.assertIsNone(single[2])
        self.assertEqual(requirement, kernel_requirement())

    def test_the_imago_constants_are_read_from_go_source(self):
        source = "const (\n\tMaxRetryAttempts = 10\n\t// x\n\tMaxPackages = 256\n)\n"
        found = gate.go_constants(source, ("MaxRetryAttempts", "MaxPackages", "Absent"))
        self.assertEqual(found, {"MaxRetryAttempts": 10, "MaxPackages": 256, "Absent": None})


class StandinTests(unittest.TestCase):
    """The stand-in claims imago's identity and replays its output byte for byte."""

    def test_the_standin_embeds_the_reply_and_claims_the_module(self):
        sources = gate.standin_sources(ACCEPTED_STDOUT + "✓ `\\\n")
        self.assertEqual(sources["cmd/imago/reply.txt"], (ACCEPTED_STDOUT + "✓ `\\\n").encode())
        self.assertIn(b"module github.com/cordanaLLM/imago\n", sources["go.mod"])
        self.assertIn(b"//go:embed reply.txt", sources["cmd/imago/main.go"])

    def test_the_standin_commits_as_itself_through_invoke(self):
        """The stand-in names its own author; invoke() shuts the workstation's git out."""
        self.assertTrue(gate.STANDIN_GIT["GIT_AUTHOR_EMAIL"].endswith("@invalid"))
        self.assertTrue(
            all(key.startswith(("GIT_AUTHOR_", "GIT_COMMITTER_")) for key in gate.STANDIN_GIT)
        )
        body = ast.get_source_segment(text(GATE), SourceSweeps.functions["build_standin"])
        self.assertIn("env=STANDIN_GIT", body)

    def test_a_rebuilt_standin_is_refused_by_what_go_stamped(self):
        """Negative: a stand-in built at its own commit differs in revision, time and version."""
        standin = (
            BUILDINFO.replace(PIN_COMMIT, "cda61556a71a77c66859d4aec883358fedbd1593")
            .replace(PSEUDO_VERSION, "v0.0.0-20260928211201-cda61556a71a")
            .replace(COMMITTED, "2026-09-28T21:12:01Z")
        )
        problems = gate.provenance_problems(
            gate.parse_buildinfo(standin), gate.load_pin(), COMMITTED
        )
        self.assertEqual(
            sorted(line.split(" is ")[0] for line in problems),
            ["mod version", "vcs.revision", "vcs.time"],
        )
        for needle in gate.STANDIN_REFUSALS:
            self.assertTrue(any(line.startswith(needle) for line in problems), needle)

    def test_a_patched_revision_is_still_refused(self):
        """Negative: the pinned revision byte-patched into a stand-in leaves version and time."""
        forged = BUILDINFO.replace(PSEUDO_VERSION, "v0.0.0-20260928211201-da4446463b2b").replace(
            COMMITTED, "2026-09-28T21:12:01Z"
        )
        problems = gate.provenance_problems(
            gate.parse_buildinfo(forged), gate.load_pin(), COMMITTED
        )
        self.assertEqual(
            sorted(line.split(" is ")[0] for line in problems), ["mod version", "vcs.time"]
        )


class IdentityTests(unittest.TestCase):
    """The retained `git ls-remote` record names both producers at their canonical URLs."""

    def record(self, **changes):
        pin = gate.load_pin()
        producers = {
            name: {
                "repository": pin[name]["repository"],
                "exit": 0,
                "main": pin[name]["commit"],
                "pinned": pin[name]["commit"],
            }
            for name in gate.PRODUCERS
        }
        for name, row in changes.items():
            producers[name].update(row)
        return {"producers": producers, "binary": {"file": "imago", "sha256": "a" * 64}}

    def test_main_is_read_from_ls_remote(self):
        self.assertEqual(gate.ls_remote_main(LS_REMOTE), "8672247ff1bd22ed6b6b89d498116f20ff00c2ab")
        self.assertIsNone(gate.ls_remote_main(LS_REMOTE.replace("refs/heads/main", "refs/heads/x")))
        self.assertIsNone(gate.ls_remote_main("8672247\trefs/heads/main\n"))
        self.assertIsNone(gate.ls_remote_main(""))

    def test_a_complete_record_passes(self):
        self.assertEqual(gate.identity_problems(self.record(), gate.load_pin()), [])

    def test_a_record_for_another_repository_or_a_failed_command_fails(self):
        pin = gate.load_pin()
        moved = self.record(imago={"repository": "https://github.com/lusoris/imago.git"})
        self.assertTrue(gate.identity_problems(moved, pin))
        failed = self.record(nucleus={"exit": 128, "main": None})
        self.assertTrue(gate.identity_problems(failed, pin))
        self.assertTrue(gate.identity_problems({}, pin))

    def test_main_having_moved_is_not_a_failure(self):
        """Boundary: the pin, not main, is what the gate runs; a newer main is recorded."""
        record = self.record(imago={"main": "b" * 40})
        self.assertEqual(gate.identity_problems(record, gate.load_pin()), [])

    def test_a_record_fetched_for_another_pin_fails_and_names_the_fetch(self):
        """Negative: a cache present for another pin is wrong, not absent (D93)."""
        record = self.record(imago={"pinned": "c" * 40})
        problems = gate.identity_problems(record, gate.load_pin())
        self.assertEqual(len(problems), 1)
        self.assertIn("the cache was fetched for " + "c" * 40, problems[0])
        self.assertIn("run `make contract-fetch`", problems[0])

    def test_a_record_without_the_binary_digest_fails(self):
        """Boundary: a record from before the digest was kept cannot vouch for the binary."""
        pin = gate.load_pin()
        for binary in (None, {}, {"sha256": "a" * 63}, {"sha256": "A" * 64}):
            with self.subTest(binary=binary):
                record = self.record()
                record["binary"] = binary
                problems = gate.identity_problems(record, pin)
                self.assertEqual(
                    problems,
                    ["the identity record carries no binary sha256; run `make contract-fetch`"],
                )

    def test_the_recorded_rows_are_read_defensively(self):
        empty = {"imago": {}, "nucleus": {}}
        self.assertEqual(gate.recorded_rows({}), empty)
        self.assertEqual(gate.recorded_rows([]), empty)
        self.assertEqual(gate.recorded_rows({"producers": {"imago": "x", "nucleus": []}}), empty)
        self.assertTrue(gate.identity_problems({"producers": []}, gate.load_pin()))


def on_path(*present):
    """Patch shutil.which so exactly `present` are found."""
    return mock.patch.object(
        gate.shutil,
        "which",
        side_effect=lambda name: f"/usr/bin/{name}" if name in present else None,
    )


class CacheTests(unittest.TestCase):
    """What skips, what fails, and where the cache lives."""

    def test_an_empty_cache_skips_and_names_the_fetch(self):
        with tempfile.TemporaryDirectory() as base:
            reasons = gate.cache_reasons(context_in(base))
        self.assertEqual(len(reasons), 4)
        for reason in reasons:
            self.assertIn("run `make contract-fetch`", reason)

    def test_a_cache_for_this_pin_admits_the_run(self):
        with tempfile.TemporaryDirectory() as base:
            self.assertEqual(gate.cache_reasons(filled_cache(base)), [])

    def test_a_cache_for_another_pin_is_not_a_skip(self):
        """Boundary: only absence skips; a cache for another pin runs, and fails."""
        with tempfile.TemporaryDirectory() as base:
            context = filled_cache(base, {"imago": "c" * 40, "nucleus": "d" * 40})
            self.assertEqual(gate.cache_reasons(context), [])

    def test_one_missing_piece_is_one_reason(self):
        with tempfile.TemporaryDirectory() as base:
            context = filled_cache(base)
            context.binary.unlink()
            reasons = gate.cache_reasons(context)
        self.assertEqual(len(reasons), 1)
        self.assertIn("no imago binary under", reasons[0])

    def test_a_missing_nucleus_checkout_is_its_own_reason(self):
        """Boundary: a record for this pin without the nucleus checkout skips and says why."""
        with tempfile.TemporaryDirectory() as base:
            context = filled_cache(base)
            (context.nucleus / ".git").rmdir()
            reasons = gate.cache_reasons(context)
            with on_path("git", "go"):
                self.assertEqual(gate.skip_reasons(context), (reasons, []))
        self.assertEqual(len(reasons), 1)
        self.assertIn("no nucleus checkout under", reasons[0])

    def test_a_record_for_another_pin_is_found_beside_an_absent_piece(self):
        """Negative (D93): the pre-D106 cache -- nucleus 8672247, no nucleus checkout --
        is present but wrong, and the absent checkout does not turn it into a skip."""
        with tempfile.TemporaryDirectory() as base:
            imago = gate.load_pin()["imago"]["commit"]
            context = filled_cache(base, {"imago": imago, "nucleus": LS_REMOTE_MAIN})
            (context.nucleus / ".git").rmdir()
            self.assertEqual(len(gate.cache_reasons(context)), 1)
            with on_path("git", "go"):
                reasons, stale = gate.skip_reasons(context)
        self.assertEqual(reasons, [])
        self.assertEqual(len(stale), 1)
        self.assertIn(f"nucleus: the cache was fetched for {LS_REMOTE_MAIN}", stale[0])
        self.assertIn("run `make contract-fetch`", stale[0])

    def test_no_record_or_one_for_this_pin_is_not_stale(self):
        """Boundary: only a record that is present and names another pin is stale."""
        with tempfile.TemporaryDirectory() as base:
            context = filled_cache(base)
            self.assertEqual(gate.stale_problems(context), [])
            context.identity.unlink()
            self.assertEqual(gate.stale_problems(context), [])
            with on_path("git", "go"):
                reasons, stale = gate.skip_reasons(context)
        self.assertEqual(stale, [])
        self.assertEqual(len(reasons), 1)
        self.assertIn("no identity record under", reasons[0])

    def test_a_record_without_producers_is_stale(self):
        """Negative: a record that names no pin at all cannot vouch for this one."""
        with tempfile.TemporaryDirectory() as base:
            context = filled_cache(base)
            context.identity.write_bytes(b'{"producers": []}')
            self.assertEqual(len(gate.stale_problems(context)), len(gate.PRODUCERS))

    def test_missing_tools_skip_before_the_record_is_read(self):
        """Boundary: without go the gate cannot run at all, stale record or not."""
        with tempfile.TemporaryDirectory() as base, on_path("git"):
            context = filled_cache(base, {"imago": "c" * 40, "nucleus": "d" * 40})
            self.assertEqual(gate.skip_reasons(context), (["go is not on PATH"], []))

    def test_an_unreadable_identity_record_is_a_failure(self):
        with tempfile.TemporaryDirectory() as base:
            context = filled_cache(base)
            context.identity.write_bytes(b"{truncated")
            self.assertEqual(gate.cache_reasons(context), [])
            with self.assertRaises(gate.GateError):
                gate.identity_case(context)
            with self.assertRaises(gate.GateError):
                gate.stale_problems(context)

    def test_missing_tools_are_reasons(self):
        with on_path("git"):
            self.assertEqual(gate.tool_reasons(), ["go is not on PATH"])
        with on_path():
            self.assertEqual(len(gate.tool_reasons()), 2)
        with on_path("go", "git"):
            self.assertEqual(gate.tool_reasons(), [])

    def test_the_cache_follows_its_variable(self):
        with mock.patch.dict(gate.os.environ, {gate.CACHE_VARIABLE: "/srv/contract"}):
            self.assertEqual(gate.cache_dir(), Path("/srv/contract"))
        env = {"XDG_CACHE_HOME": "/xdg"}
        with mock.patch.dict(gate.os.environ, env), mock.patch.dict(gate.os.environ):
            gate.os.environ.pop(gate.CACHE_VARIABLE, None)
            self.assertEqual(gate.cache_dir(), Path("/xdg") / "aegis-contract")

    def test_only_the_newest_runs_and_the_recorded_fetch_are_kept(self):
        with tempfile.TemporaryDirectory() as base:
            store = Path(base)
            names = [f"r2026{index:04d}" for index in range(20)]
            names += [f"f2026{index:04d}" for index in range(20)]
            for name in names:
                (store / "runs" / name).mkdir(parents=True)
            (store / "identity.json").write_bytes(b'{"run": "f20260000"}')
            gate.prune_runs(store)
            kept = sorted(path.name for path in (store / "runs").iterdir())
        self.assertEqual(len(kept), 2 * gate.MAX_RETAINED_RUNS + 1)
        self.assertIn("f20260000", kept)
        self.assertIn("r20260019", kept)
        self.assertNotIn("r20260000", kept)

    def test_run_names_say_which_command_made_them(self):
        self.assertTrue(gate.run_name(True).startswith("f"))
        self.assertTrue(gate.run_name(False).startswith("r"))


class GuardTests(unittest.TestCase):
    """The gate's three outcomes: a stated skip, a stated failure, or cases above a PASS."""

    def test_no_go_is_a_stated_skip_and_exit_zero(self):
        with tempfile.TemporaryDirectory() as base, on_path("git"), mock.patch.dict(
            gate.os.environ, {gate.CACHE_VARIABLE: base}
        ):
            code, printed = quiet(gate.main, [])
        self.assertEqual(code, 0)
        self.assertIn("SKIP: go is not on PATH; the contract pair gate did not run.", printed)
        self.assertNotIn("PASS", printed)

    def test_an_empty_cache_skips_before_anything_runs(self):
        with tempfile.TemporaryDirectory() as base, on_path("go", "git"), mock.patch.dict(
            gate.os.environ, {gate.CACHE_VARIABLE: base}
        ), mock.patch.object(gate, "run") as run:
            code, printed = quiet(gate.main, [])
        self.assertEqual(code, 0)
        self.assertIn("SKIP: no imago checkout", printed)
        run.assert_not_called()

    def test_the_fetch_without_git_fails(self):
        with tempfile.TemporaryDirectory() as base, on_path("go"), mock.patch.dict(
            gate.os.environ, {gate.CACHE_VARIABLE: base}
        ):
            code, printed = quiet(gate.main, ["--fetch"])
        self.assertEqual(code, 1)
        self.assertIn("FAIL contract-fetch/toolchain", printed)
        self.assertIn("git is not on PATH", printed)

    def test_the_fetch_refuses_a_go_below_the_floor(self):
        """Boundary: the fetch reads go's version and stops below imago's go.mod floor."""
        answers = {"go": (0, "go version go1.27.0 linux/amd64\n", ""), "git": (0, "git 2\n", "")}
        with tempfile.TemporaryDirectory() as base, on_path("go", "git"), mock.patch.dict(
            gate.os.environ, {gate.CACHE_VARIABLE: base}
        ), mock.patch.object(gate, "run", side_effect=lambda argv, *a, **k: answers[argv[0]]):
            code, printed = quiet(gate.main, ["--fetch"])
        self.assertEqual(code, 1)
        self.assertIn("below go 1.27.1", printed)

    def test_an_invalid_pin_fails(self):
        with tempfile.TemporaryDirectory() as base:
            broken = Path(base) / "pin.json"
            broken.write_bytes(b'{"schema": "other"}')
            with mock.patch.object(gate, "PIN", broken), mock.patch.object(
                gate.load_pin, "__defaults__", (broken,)
            ):
                code, printed = quiet(gate.main, [])
        self.assertEqual(code, 1)
        self.assertIn("FAIL: the gate could not run: the pin does not declare schema", printed)

    def identity_cases(self, failing):
        """Run run_cases with the five identity cases stubbed; `failing` maps a case to its
        problems. Return (failed count, printed text, whether the payload cases ran)."""
        names = (
            ("pin_case", "contract/pin"),
            ("identity_case", "contract/identity"),
            ("checkout_case", "contract/checkout"),
            ("nucleus_checkout_case", "contract/nucleus-checkout"),
            ("provenance_case", "contract/binary-provenance"),
        )
        with tempfile.TemporaryDirectory() as base, contextlib.ExitStack() as stack:
            context = context_in(base)
            context.run_dir.mkdir(parents=True)
            for function, name in names:
                stub = reporting(name, failing.get(name, []))
                stack.enter_context(mock.patch.object(gate, function, side_effect=stub))
            accepted = stack.enter_context(mock.patch.object(gate, "product_accepted_case"))
            failed, printed = quiet(gate.run_cases, context)
        return failed, printed, accepted.called

    def test_a_cache_that_is_not_the_pinned_build_stops_the_payload_cases(self):
        """Negative: a wrong checkout fails, and nothing the binary prints is counted."""
        failed, printed, ran = self.identity_cases({"contract/checkout": ["HEAD is 'x'"]})
        self.assertEqual(failed, 1)
        self.assertIn("the payload cases did not run, because contract/checkout failed", printed)
        self.assertFalse(ran)

    def test_a_wrong_nucleus_checkout_stops_the_payload_cases(self):
        """Negative (D106): a nucleus checkout that is not the pin runs nothing it would verify."""
        failed, printed, ran = self.identity_cases(
            {"contract/nucleus-checkout": ["versions.json aegis-os streams is ['lts']"]}
        )
        self.assertEqual(failed, 1)
        self.assertIn("because contract/nucleus-checkout failed", printed)
        self.assertFalse(ran)

    def test_the_stated_reason_names_the_cases_that_failed(self):
        """Boundary: an edited Aegis payload fails the pin alone and blames nothing else."""
        failed, printed, ran = self.identity_cases({"contract/pin": ["build/product-input.json"]})
        self.assertEqual(failed, 1)
        self.assertIn("the payload cases did not run, because contract/pin failed\n", printed)
        self.assertNotIn("imago", printed.split("did not run")[1])
        self.assertFalse(ran)

    def test_a_cache_fetched_for_another_pin_fails_rather_than_skips(self):
        """Negative (D93): a cache present but wrong is a FAIL naming the fetch, never a SKIP."""
        other = {"imago": "c" * 40, "nucleus": "d" * 40}
        with tempfile.TemporaryDirectory() as base, on_path("go", "git"):
            filled_cache(base, other)
            store = {gate.CACHE_VARIABLE: str(Path(base) / "cache")}
            with mock.patch.dict(gate.os.environ, store), mock.patch.object(
                gate, "run", return_value=(0, "", "")
            ):
                code, printed = quiet(gate.main, [])
        self.assertEqual(code, 1)
        self.assertIn("FAIL contract/identity", printed)
        self.assertIn("the cache was fetched for " + "c" * 40, printed)
        self.assertNotIn("SKIP", printed)

    def test_the_cache_a_pre_d106_fetch_left_fails_rather_than_skips(self):
        """Negative (D93): its record names nucleus 8672247 and it holds no nucleus checkout;
        the gate fails contract/identity, runs nothing else and prints no SKIP."""
        imago = gate.load_pin()["imago"]["commit"]
        with tempfile.TemporaryDirectory() as base, on_path("go", "git"):
            context = filled_cache(base, {"imago": imago, "nucleus": LS_REMOTE_MAIN})
            (context.nucleus / ".git").rmdir()
            store = {gate.CACHE_VARIABLE: str(context.store)}
            with mock.patch.dict(gate.os.environ, store), mock.patch.object(
                gate, "run", return_value=(0, "", "")
            ), mock.patch.object(gate, "checkout_case") as checkout:
                code, printed = quiet(gate.main, [])
        self.assertEqual(code, 1)
        self.assertIn("FAIL contract/identity", printed)
        self.assertIn(f"nucleus: the cache was fetched for {LS_REMOTE_MAIN}", printed)
        self.assertIn("the other cases did not run", printed)
        self.assertNotIn("SKIP", printed)
        checkout.assert_not_called()

    def test_a_cache_for_this_pin_without_a_piece_still_skips(self):
        """Boundary: absence alone, under a record for this pin, stays a stated skip."""
        with tempfile.TemporaryDirectory() as base, on_path("go", "git"):
            context = filled_cache(base)
            (context.nucleus / ".git").rmdir()
            store = {gate.CACHE_VARIABLE: str(context.store)}
            with mock.patch.dict(gate.os.environ, store), mock.patch.object(gate, "run") as run:
                code, printed = quiet(gate.main, [])
        self.assertEqual(code, 0)
        self.assertIn("SKIP: no nucleus checkout under", printed)
        self.assertNotIn("FAIL", printed)
        run.assert_not_called()

    def test_every_terminating_signal_this_host_has_is_routed(self):
        with mock.patch.object(gate.signal, "signal") as install:
            handled = gate.install_signal_handlers()
        self.assertEqual(install.call_count, len(handled))
        self.assertLessEqual(set(handled), set(gate.TERMINATING_SIGNALS))


class RecordTests(unittest.TestCase):
    """Every invocation is retained with its argv, exit code, stdout and stderr."""

    def test_an_invocation_is_retained(self):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            with mock.patch.object(gate, "run", return_value=(1, "", EMPTY_STDERR)):
                returned = gate.invoke(context, "kernel-empty", ["imago", "x"], 5)
            kept = json.loads((context.run_dir / "logs" / "kernel-empty.json").read_bytes())
        self.assertEqual(returned, result(1, "", EMPTY_STDERR))
        self.assertEqual(
            kept, {"argv": ["imago", "x"], "exit": 1, "stdout": "", "stderr": EMPTY_STDERR}
        )

    def test_a_case_is_printed_and_kept_for_the_summary(self):
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            problems, printed = quiet(gate.report, context, "contract/pin", ["x"], ["note"])
        self.assertEqual(problems, ["x"])
        self.assertEqual(printed, "FAIL contract/pin\n     note\n     x\n")
        self.assertEqual(context.outcomes[0]["case"], "contract/pin")

    def test_a_row_note_carries_the_correlated_error(self):
        document = product_input()
        note = gate.row_outcome("product-input", document, "schema: want x", [])
        self.assertEqual(
            note, "refused: aegis product-input aegis-m18-product-input-0001: schema: want x"
        )
        self.assertEqual(
            gate.row_outcome("kernel-requirement", kernel_requirement(), None, []),
            "accepted aegis-m18-kernel-requirement-0001",
        )
        self.assertEqual(gate.row_outcome("x", document, None, ["p"]), "NOT as the row expects")


class CheckoutTests(unittest.TestCase):
    """The checkout counts only when nothing in it differs from the pinned commit."""

    index = "H go.mod\nH cmd/imago/main.go\n"

    def problems(self, status="", index=None, head=PIN_COMMIT):
        """Return checkout_problems() over the given git answers, and the mocked run()."""
        rows = (
            (("rev-parse",), (0, head + "\n", "")),
            (("status",), (0, status, "")),
            (("ls-files",), (0, self.index if index is None else index, "")),
        )
        with tempfile.TemporaryDirectory() as base, mock.patch.object(
            gate, "run", side_effect=answering(*rows)
        ) as run:
            problems = gate.checkout_problems(context_in(base), "t")
        return problems, run

    def test_a_checkout_that_is_the_commit_passes(self):
        """Positive, and the status command counts every untracked and ignored file."""
        problems, run = self.problems()
        self.assertEqual(problems, [])
        status = next(call.args[0] for call in run.call_args_list if "status" in call.args[0])
        for flag in ("--porcelain", "--untracked-files=all", "--ignored", "core.fsmonitor=false"):
            self.assertIn(flag, status)

    def test_an_untracked_ignored_or_changed_file_fails(self):
        """Negative: what the review planted -- an excluded .go file, an untracked one -- fails."""
        for line in (
            "!! cmd/imago/zz_standin.go\n",
            "?? cmd/imago/zz_untracked.go\n",
            " M pkg/aegis/validate.go\n",
        ):
            with self.subTest(line=line):
                problems, _ = self.problems(status=line)
                self.assertEqual(len(problems), 1)
                self.assertTrue(problems[0].startswith("the checkout is modified"))
                self.assertIn(line.strip(), problems[0])

    def test_an_index_entry_hidden_from_status_fails(self):
        """Boundary: assume-unchanged (lowercase) and skip-worktree (S) hide a change."""
        for index in ("h pkg/aegis/validate.go\n", "S pkg/aegis/aegis.go\n"):
            with self.subTest(index=index):
                problems, _ = self.problems(index=self.index + index)
                self.assertEqual(len(problems), 1)
                self.assertIn("the index hides entries from git status", problems[0])
                self.assertIn(index.strip(), problems[0])

    def test_another_head_fails(self):
        problems, _ = self.problems(head="a" * 40)
        self.assertEqual(problems, [f"HEAD is {'a' * 40!r}"])

    def test_the_commit_time_is_read_as_go_writes_it(self):
        rows = ((("log",), (0, COMMITTED_SECONDS + "\n", "")),)
        with tempfile.TemporaryDirectory() as base, mock.patch.object(
            gate, "run", side_effect=answering(*rows)
        ):
            self.assertEqual(gate.commit_time(context_in(base), "t"), COMMITTED)
        for reply in ((128, "", "fatal: not a git repository"), (0, "yesterday\n", "")):
            with self.subTest(reply=reply), tempfile.TemporaryDirectory() as base:
                with mock.patch.object(gate, "run", return_value=reply):
                    self.assertIsNone(gate.commit_time(context_in(base), "t"))


class EnvironmentTests(unittest.TestCase):
    """What every command inherits, and how its paths are spelled."""

    def test_a_hooks_git_variables_are_not_inherited(self):
        """Negative: GIT_DIR and friends from a hook would point git at another repository."""
        hook = {
            "GIT_DIR": "/elsewhere/.git",
            "GIT_WORK_TREE": "/elsewhere",
            "GIT_INDEX_FILE": "/elsewhere/.git/index",
            "GIT_CONFIG_PARAMETERS": "'status.showuntrackedfiles'='no'",
            "GIT_CONFIG_COUNT": "1",
            "GIT_CONFIG_KEY_0": "status.showUntrackedFiles",
            "GIT_CONFIG_VALUE_0": "no",
        }
        with mock.patch.dict(gate.os.environ, hook):
            env = gate.child_environment({"GOPROXY": "off"})
        for key in hook:
            self.assertNotIn(key, env)
        self.assertEqual(env["LC_ALL"], "C")
        self.assertEqual(env["GIT_TERMINAL_PROMPT"], "0")
        self.assertEqual(env["GOPROXY"], "off")

    def test_every_command_runs_with_the_git_configuration_shut_out(self):
        """Positive: the system and user git configuration are shut out, the caller's env kept."""
        with tempfile.TemporaryDirectory() as base, mock.patch.object(
            gate, "run", return_value=(0, "", "")
        ) as run:
            context = context_in(base)
            gate.invoke(context, "go-build", ["go", "build"], 5, env={"GOPROXY": "off"})
            env = run.call_args.kwargs["env"]
            self.assertEqual(Path(env["GIT_CONFIG_GLOBAL"]).read_bytes(), b"")
        self.assertEqual(env["GIT_CONFIG_NOSYSTEM"], "1")
        self.assertEqual(env["GOPROXY"], "off")

    def test_host_paths_keep_their_drive(self):
        """Boundary: a Windows path reaches git and go as the host spells it, drive and all."""
        windows = r"C:\Users\runneradmin\.cache\aegis-contract\imago"
        self.assertEqual(gate.native(PureWindowsPath(windows)), windows)
        checkout = Path("cache") / "imago"
        self.assertEqual(gate.git(checkout, "status")[:3], ["git", "-C", os.fspath(checkout)])

    def test_the_binary_is_named_as_the_host_starts_it(self):
        pin = gate.load_pin()
        for suffix, name in ((".exe", "imago.exe"), ("", "imago")):
            with self.subTest(suffix=suffix), mock.patch.object(gate, "EXE", suffix):
                context = gate.Context(pin, Path("store"), Path("store") / "runs" / "r")
                self.assertEqual(context.binary.name, name)


class DigestTests(unittest.TestCase):
    """The binary counts only when it is the one the fetch built and recorded."""

    rows = (
        (("version", "-m"), (0, BUILDINFO, "")),
        (("log",), (0, COMMITTED_SECONDS + "\n", "")),
        (("version",), (0, "go version go1.27.1 linux/amd64\n", "")),
    )

    def case(self, recorded, binary=b"\x7fELF imago"):
        """Run provenance_case over a binary holding `binary` and a record naming `recorded`."""
        with tempfile.TemporaryDirectory() as base, mock.patch.object(
            gate, "run", side_effect=answering(*self.rows)
        ):
            context = context_in(base)
            context.binary.parent.mkdir(parents=True)
            context.binary.write_bytes(binary)
            record = {} if recorded is None else {"binary": {"sha256": gate.sha256_of(recorded)}}
            context.identity.write_bytes(json.dumps(record).encode("utf-8"))
            problems, printed = quiet(gate.provenance_case, context)
        return problems, printed

    def test_the_binary_the_fetch_recorded_passes(self):
        problems, printed = self.case(b"\x7fELF imago")
        self.assertEqual(problems, [])
        self.assertIn("as the fetch recorded", printed)
        self.assertIn(f"(commit time {COMMITTED})", printed)

    def test_a_binary_replaced_after_the_fetch_fails(self):
        """Negative: the build information alone can be forged; the recorded digest cannot."""
        problems, _ = self.case(b"\x7fELF imago", binary=b"\x7fELF forged")
        self.assertEqual(len(problems), 1)
        self.assertIn("the fetch recorded " + gate.sha256_of(b"\x7fELF imago"), problems[0])
        self.assertIn("run `make contract-fetch`", problems[0])

    def test_a_record_without_a_digest_fails(self):
        problems, _ = self.case(None)
        self.assertEqual(len(problems), 1)
        self.assertIn("the fetch recorded None", problems[0])


def planted_bytes():
    """Return the bytes the gate writes for the requirement with the planted symbol."""
    requirement = kernel_requirement()
    features = requirement["features"] + [gate.PLANTED_FEATURE]
    return gate.json_bytes(gate.with_field(requirement, "features", features))


def empty_bytes():
    """Return the bytes the gate writes for the requirement with no feature."""
    return gate.json_bytes(gate.with_field(kernel_requirement(), "features", []))


def stream_row(document, name):
    """Return the one stream row named `name` in a report document."""
    (row,) = [stream for stream in document["streams"] if stream["stream"] == name]
    return row


class NucleusReportTests(unittest.TestCase):
    """What nucleus's report must say for a row, read from its JSON and never its text (D106)."""

    pin = gate.load_pin()
    sent = (ROOT / "build" / "kernel-requirement.json").read_bytes()

    def problems(self, record, expected, sent=None):
        verdict = "PASS" if expected["status"] == "PASS" else "FAIL"
        document = gate.document_of(record)
        return gate.report_problems(record, self.pin, verdict) + gate.document_problems(
            document, expected, self.pin["nucleus"], self.sent if sent is None else sent
        )

    def test_the_recorded_pass_is_admitted(self):
        self.assertEqual(self.problems(json.loads(PASS_REPORT), gate.PASSED), [])

    def test_the_recorded_correlated_refusal_is_admitted(self):
        """Positive: the gate's own planted payload is the one nucleus hashed and refused."""
        record = json.loads(FAIL_REPORT)
        self.assertEqual(self.problems(record, gate.FAILED, planted_bytes()), [])

    def test_the_recorded_rejection_is_admitted(self):
        record = json.loads(EMPTY_REPORT)
        self.assertEqual(self.problems(record, gate.rejected("NoFeatures"), empty_bytes()), [])

    def test_another_evidence_level_or_revision_fails(self):
        """Negative: a resolved report, or one for another commit, is not what the pin records."""
        for key, value in (("evidence_level", "resolved"), ("nucleus_revision", "a" * 40)):
            with self.subTest(key=key):
                record = dict(json.loads(PASS_REPORT), **{key: value})
                problems = self.problems(record, gate.PASSED)
                self.assertEqual(len(problems), 1, problems)
                self.assertIn(f"report {key} is {value!r}", problems[0])

    def test_a_pass_not_held_by_exactly_the_bound_streams_fails(self):
        for key, value in (("held", []), ("bound_streams", ["lts"]), ("reasons", ["x"])):
            with self.subTest(key=key):
                record = json.loads(PASS_REPORT)
                record["documents"][0][key] = value
                self.assertEqual(len(self.problems(record, gate.PASSED)), 1)

    def test_a_refusal_that_drops_the_correlation_id_or_the_symbol_fails(self):
        record = json.loads(FAIL_REPORT)
        reason = record["documents"][0]["reasons"][0]
        record["documents"][0]["reasons"] = [reason.replace("-0001:", "-0002:")]
        problems = self.problems(record, gate.FAILED, planted_bytes())
        self.assertTrue(any("not every reason opens with" in line for line in problems))
        record["documents"][0]["reasons"] = ["aegis-m18-kernel-requirement-0001: realtime x86_64"]
        problems = self.problems(record, gate.FAILED, planted_bytes())
        self.assertEqual(problems, ["no reason names CONFIG_AEGIS_CONTRACT_UNSET"])

    def test_a_planted_symbol_nucleus_sets_fails(self):
        """Negative: a refusal that records the planted symbol as set is not the one planted."""
        record = json.loads(FAIL_REPORT)
        realtime = stream_row(record["documents"][0], "realtime")
        check = realtime["architectures"][0]["features"][1]
        check.update(observed="y", satisfied=True)
        problems = self.problems(record, gate.FAILED, planted_bytes())
        self.assertEqual(len(problems), 1)
        self.assertIn("does not record CONFIG_AEGIS_CONTRACT_UNSET unset", problems[0])

    def test_another_kind_status_or_document_fails(self):
        record = json.loads(EMPTY_REPORT)
        wrong_kind = self.problems(record, gate.rejected("DigestMismatch"), empty_bytes())
        self.assertEqual(wrong_kind, ["rejected as 'NoFeatures', expected DigestMismatch"])
        self.assertTrue(self.problems(record, gate.PASSED, empty_bytes()))
        other_bytes = self.problems(record, gate.rejected("NoFeatures"), self.sent)
        self.assertEqual(len(other_bytes), 1)
        self.assertIn("hashes the document as", other_bytes[0])

    def test_no_report_or_two_documents_fail(self):
        self.assertEqual(
            gate.report_problems(None, self.pin, "PASS"), ["the verifier wrote no JSON report"]
        )
        record = json.loads(PASS_REPORT)
        record["documents"] *= 2
        self.assertEqual(gate.document_of(record), {})
        self.assertIn(
            "the report does not hold exactly one document",
            gate.report_problems(record, self.pin, "PASS"),
        )

    def test_only_bound_streams_count_and_no_check_fails_closed(self):
        """Boundary: an unbound stream is information only. nucleus recorded the planted
        symbol unset on lts as well, so a synthetic copy sets it there: bound to realtime
        alone the refusal still holds, and with lts bound too it would not. A report
        with no check of the planted symbol at all is refused rather than passed."""
        document = gate.document_of(json.loads(FAIL_REPORT))
        self.assertEqual(len(gate.planted_checks(document, ["realtime"])), 1)
        self.assertEqual(len(gate.planted_checks(document, ["realtime", "lts"])), 2)
        self.assertIsNone(gate.unset_problem(gate.planted_checks(document, ["realtime", "lts"])))
        synthetic = gate.document_of(json.loads(FAIL_REPORT))
        check = stream_row(synthetic, "lts")["architectures"][0]["features"][0]
        check.update(observed="y", satisfied=True)
        self.assertIsNone(gate.unset_problem(gate.planted_checks(synthetic, ["realtime"])))
        both = gate.planted_checks(synthetic, ["realtime", "lts"])
        self.assertIsNotNone(gate.unset_problem(both))
        self.assertIsNotNone(gate.unset_problem(gate.planted_checks(document, ["mainstream"])))
        self.assertIsNotNone(gate.unset_problem([]))

    def test_another_report_schema_or_verdict_fails(self):
        """Negative: the report's schema id and its top-level verdict each count alone."""
        schema = dict(json.loads(PASS_REPORT), schema="nucleus.kernel-requirement-report.v2")
        problems = self.problems(schema, gate.PASSED)
        self.assertEqual(len(problems), 1, problems)
        self.assertIn("report schema is 'nucleus.kernel-requirement-report.v2'", problems[0])
        verdict = dict(json.loads(FAIL_REPORT), verdict="PASS")
        problems = self.problems(verdict, gate.FAILED, planted_bytes())
        self.assertEqual(problems, ["report verdict is 'PASS', expected 'FAIL'"])

    def test_another_label_or_correlation_id_fails(self):
        """Negative: a document for another label, or a PASS naming another document's
        correlation id, is not this repository's requirement held."""
        record = json.loads(PASS_REPORT)
        record["documents"][0]["label"] = "imago"
        problems = self.problems(record, gate.PASSED)
        self.assertEqual(problems, ["document 'imago' is 'PASS', expected aegis-os PASS"])
        record = json.loads(PASS_REPORT)
        record["documents"][0]["correlation_id"] = "aegis-m18-kernel-reference-0001"
        problems = self.problems(record, gate.PASSED)
        self.assertEqual(problems, ["document correlation_id is 'aegis-m18-kernel-reference-0001'"])


class NucleusBindingTests(unittest.TestCase):
    """nucleus's versions.json must bind the pinned label to this repository's payload."""

    nucleus = gate.load_pin()["nucleus"]

    def test_the_recorded_binding_passes(self):
        row = gate.bound_row(json.loads(VERSIONS), "aegis-os")
        self.assertEqual(row["streams"], ["realtime"])
        self.assertEqual(gate.binding_problems(row, self.nucleus), [])

    def test_another_path_stream_or_repository_fails(self):
        for key, value in (
            ("path", "build/kernel-requirement.reference.json"),
            ("streams", ["realtime", "lts"]),
            ("repository", "lusoris/Aegis-OS"),
        ):
            with self.subTest(key=key):
                row = dict(gate.bound_row(json.loads(VERSIONS), "aegis-os"), **{key: value})
                problems = gate.binding_problems(row, self.nucleus)
                self.assertEqual(len(problems), 1, problems)
                self.assertIn(f"aegis-os {key} is", problems[0])

    def test_an_absent_or_repeated_label_binds_nothing(self):
        versions = json.loads(VERSIONS)
        self.assertIsNone(gate.bound_row(versions, "aegis"))
        versions["downstream"]["requirements"].append({"label": "aegis-os"})
        self.assertIsNone(gate.bound_row(versions, "aegis-os"))
        self.assertEqual(
            gate.binding_problems(None, self.nucleus),
            ["versions.json binds no single downstream row to aegis-os"],
        )
        self.assertIsNone(gate.bound_row([], "aegis-os"))

    def test_the_row_bound_is_inclusive(self):
        """Boundary: the label as the 64th row is read; as the 65th it is not, and fails."""
        row = gate.bound_row(json.loads(VERSIONS), "aegis-os")
        filler = [{"label": f"other-{index}"} for index in range(gate.MAX_REQUIREMENT_ROWS - 1)]
        versions = {"downstream": {"requirements": filler + [row]}}
        self.assertEqual(gate.bound_row(versions, "aegis-os"), row)
        versions["downstream"]["requirements"].insert(0, {"label": "one-more"})
        self.assertIsNone(gate.bound_row(versions, "aegis-os"))


class NucleusRunTests(unittest.TestCase):
    """How the gate runs nucleus's verifier, and the rows it sends."""

    def test_the_verifier_runs_isolated_from_the_checkout_with_absolute_paths(self):
        with tempfile.TemporaryDirectory() as base, mock.patch.object(
            gate, "run", return_value=(0, "", "")
        ) as run:
            context = context_in(base)
            result, record = gate.verify_requirement(
                context, "nucleus-x", gate.KERNEL_REQUIREMENT, ["--sha256", "aegis-os=ab"]
            )
            argv, cwd = run.call_args.args[0], run.call_args.kwargs["cwd"]
        self.assertEqual(
            argv[:4], [gate.PYTHON, "-I", "-B", "scripts/verify_kernel_requirement.py"]
        )
        self.assertEqual(argv[4:6], ["--requirement", f"aegis-os={gate.KERNEL_REQUIREMENT}"])
        self.assertTrue(Path(argv[7]).is_absolute())
        self.assertEqual(
            argv[8:], ["--nucleus-revision", NUCLEUS_COMMIT, "--sha256", "aegis-os=ab"]
        )
        self.assertEqual(cwd, context.nucleus)
        self.assertEqual((result["exit"], record), (0, None))

    def test_the_rows_send_the_committed_bytes_and_one_field_edits(self):
        requirement = kernel_requirement()
        digest = gate.sha256_of(gate.KERNEL_REQUIREMENT.read_bytes())
        with tempfile.TemporaryDirectory() as base:
            context = context_in(base)
            rows = gate.nucleus_rows(context)
            written = {
                label: Path(path).read_bytes()
                for rows_ in rows.values()
                for label, path, _, _ in rows_
            }
        accepted = rows["nucleus/accepted"][0]
        self.assertEqual(accepted[1], gate.KERNEL_REQUIREMENT)
        self.assertEqual(
            accepted[2],
            [
                "--sha256",
                f"aegis-os={digest}",
                "--correlation-id",
                f"aegis-os={requirement['correlation-id']}",
            ],
        )
        self.assertEqual(written["planted-unset-symbol"], planted_bytes())
        self.assertEqual(written["features-0"], empty_bytes())
        self.assertEqual(len(json.loads(written["features-1"])["features"]), 1)
        binding = {row[0]: row for row in rows["nucleus/dispatch-binding-refused"]}
        reference = gate.KERNEL_REFERENCE.read_bytes()
        self.assertIn(
            f"aegis-os={gate.sha256_of(reference)}", binding["sha256-of-another-document"][2]
        )
        self.assertIn(
            "aegis-os=aegis-m18-kernel-reference-0001", binding["correlation-id-of-another"][2]
        )
        self.assertEqual(
            requirement, kernel_requirement(), "the source payload must stay untouched"
        )

    def test_a_case_asserts_the_exit_code_beside_the_report(self):
        """Negative: the right report with the wrong exit code fails, and the reverse."""
        rows = [("requirement", gate.KERNEL_REQUIREMENT, [], gate.PASSED)]
        for code, record, failed in (
            (0, PASS_REPORT, False),
            (1, PASS_REPORT, True),
            (0, FAIL_REPORT, True),
        ):
            with self.subTest(code=code, failed=failed), tempfile.TemporaryDirectory() as base:
                answer = (result(code), json.loads(record))
                with mock.patch.object(gate, "verify_requirement", return_value=answer):
                    problems, _ = quiet(gate.nucleus_case, context_in(base), "nucleus/x", rows)
                self.assertEqual(bool(problems), failed, problems)


# A verifier that imports a sibling module the way nucleus's does since 82aa6b7:
# it puts its own directory on sys.path, which -I keeps off, and writes the report
# it is asked for. Synthetic: nucleus did not write it.
SIBLING_VERIFIER = (
    "import sys\n"
    "from pathlib import Path\n"
    "sys.path.insert(0, str(Path(__file__).resolve().parent))\n"
    "import sibling\n"
    "Path(sys.argv[sys.argv.index('--report-json') + 1]).write_text(sibling.REPORT)\n"
)


class NucleusBytecodeTests(unittest.TestCase):
    """The verifier's imports write nothing into the checkout the next run requires clean.

    Run r20260929T102148-b707 found it: the first gate run at 82aa6b7 left
    scripts/__pycache__ in the nucleus checkout, and the next one failed
    contract/nucleus-checkout on it.
    """

    def checkout(self, base):
        context = context_in(base)
        scripts = context.nucleus / "scripts"
        scripts.mkdir(parents=True)
        (scripts / "verify_kernel_requirement.py").write_text(SIBLING_VERIFIER, encoding="utf-8")
        (scripts / "sibling.py").write_text('REPORT = "{}"\n', encoding="utf-8")
        return context

    def test_the_gate_run_leaves_no_bytecode(self):
        with tempfile.TemporaryDirectory() as base:
            context = self.checkout(base)
            result, record = gate.verify_requirement(context, "nucleus-x", gate.KERNEL_REQUIREMENT)
            cached = sorted(context.nucleus.rglob("__pycache__"))
        self.assertEqual((result["exit"], record, cached), (0, {}, []), result["stderr"])

    def test_the_same_run_without_b_leaves_bytecode(self):
        """Negative control: without -B this interpreter writes the cache the gate refuses."""
        with tempfile.TemporaryDirectory() as base:
            context = self.checkout(base)
            argv = [gate.PYTHON, "-I", "scripts/verify_kernel_requirement.py"]
            argv += ["--report-json", str(Path(base) / "report.json")]
            code, _, stderr = gate.run(argv, gate.VALIDATE_TIMEOUT, cwd=context.nucleus)
            cached = [path.name for path in context.nucleus.rglob("__pycache__")]
        self.assertEqual((code, cached), (0, ["__pycache__"]), stderr)


class NucleusCheckoutTests(unittest.TestCase):
    """The nucleus checkout counts only as the pinned commit binding this repository."""

    rows = (
        (("rev-parse",), (0, NUCLEUS_COMMIT + "\n", "")),
        (("ls-files",), (0, "H versions.json\n", "")),
    )

    def case(self, versions=VERSIONS, verifier=True):
        with tempfile.TemporaryDirectory() as base, mock.patch.object(
            gate, "run", side_effect=answering(*self.rows)
        ):
            context = context_in(base)
            (context.nucleus / "scripts").mkdir(parents=True)
            (context.nucleus / "versions.json").write_bytes(versions)
            if verifier:
                (context.nucleus / "scripts" / "verify_kernel_requirement.py").write_bytes(b"")
            return quiet(gate.nucleus_checkout_case, context)

    def test_the_pinned_checkout_passes(self):
        problems, printed = self.case()
        self.assertEqual(problems, [])
        self.assertIn(
            "binds aegis-os to cordanaLLM/Aegis-OS build/kernel-requirement.json", printed
        )

    def test_a_missing_verifier_or_another_binding_fails(self):
        problems, _ = self.case(verifier=False)
        self.assertEqual(problems, ["the checkout has no scripts/verify_kernel_requirement.py"])
        problems, _ = self.case(versions=VERSIONS.replace(b'["realtime"]', b'["lts"]'))
        self.assertEqual(len(problems), 1)
        self.assertIn("aegis-os streams is ['lts']", problems[0])


class SuppressionTests(unittest.TestCase):
    """No surface the gate runs through can turn a failure into a pass (REQ-CI-02)."""

    def test_the_make_targets_run_the_gate_unsuppressed(self):
        makefile = text(MAKEFILE)
        self.assertEqual(
            recipe_lines(makefile, "verify-contract"), ["python3 tools/verify_contract_pair.py"]
        )
        self.assertEqual(
            recipe_lines(makefile, "contract-fetch"),
            ["python3 tools/verify_contract_pair.py --fetch"],
        )
        wired = [line for line in recipe_lines(makefile, "verify-all") if "verify-contract" in line]
        self.assertEqual(wired, ["$(MAKE) --no-print-directory verify-contract"])
        for line in wired + recipe_lines(makefile, "verify-contract"):
            self.assertFalse(suppresses(line), line)

    def test_ci_fetches_before_the_gate(self):
        lines = [line.strip() for line in text(CI).splitlines()]
        self.assertIn("run: make contract-fetch", lines)
        self.assertLess(
            lines.index("run: make contract-fetch"), lines.index("run: make verify-all")
        )
        self.assertLess(
            lines.index("go-version-file: praetor-src/go.mod"),
            lines.index("run: make contract-fetch"),
        )


def admitted_table():
    """Return {tool: cells} from the M09 table on the admission page."""
    section = text(ADMISSION).split(ADMISSION_HEADING, 1)[1].split("\n## ", 1)[0]
    rows = {}
    for line in section.splitlines():
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) == 5 and cells[0] not in {"Tool", "Tool or producer"}:
            if not set(cells[0]) <= {":-"}:
                rows[cells[0].strip("`")] = cells
    return rows


class AdmissionTests(unittest.TestCase):
    """The admission page, the evidence page and the pin state one admission."""

    def test_every_program_and_producer_is_on_the_page(self):
        rows = admitted_table()
        pin = gate.load_pin()
        self.assertIn(pin["imago"]["go"], rows["go"][1])
        self.assertIn("git", rows)
        self.assertIn(pin["imago"]["commit"], rows["cordanaLLM/imago"][1])
        self.assertIn(pin["nucleus"]["commit"], rows["cordanaLLM/nucleus"][1])
        self.assertIn(pin["nucleus"]["verifier"], rows["cordanaLLM/nucleus"][2])
        for program in ALLOWED_PROGRAMS - {"<binary>"}:
            self.assertIn(program, rows)

    def test_the_evidence_page_is_published_and_names_the_pins(self):
        page = text(EVIDENCE_PAGE)
        pin = gate.load_pin()
        for value in (pin["imago"]["commit"], pin["nucleus"]["commit"], "D92", "D93", "D106"):
            self.assertIn(value, page)
        self.assertIn("build/contract-pair.md", text(MKDOCS))


def function_index(tree):
    return {node.name: node for node in ast.walk(tree) if isinstance(node, ast.FunctionDef)}


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


def program_of(node):
    """Name what an argv list starts: a string literal, or <binary> for a path under test."""
    if isinstance(node, ast.Call) and getattr(node.func, "id", "") == "git":
        return "git"
    if not isinstance(node, ast.List) or not node.elts:
        return None
    first = node.elts[0]
    if isinstance(first, ast.Constant) and isinstance(first.value, str):
        return first.value
    if isinstance(first, ast.Call) and getattr(first.func, "id", "") == "native":
        return "<binary>"
    if isinstance(first, ast.Name) and first.id == "PYTHON":
        return "python3"
    return None


class SourceSweeps(unittest.TestCase):
    """What the gate may start, with which deadline, and at what size (HISS-02/04/08)."""

    tree = ast.parse(text(GATE))
    functions = function_index(tree)

    def calls(self, name):
        return [
            node
            for node in ast.walk(self.tree)
            if isinstance(node, ast.Call) and getattr(node.func, "id", None) == name
        ]

    def assigned(self, function, name):
        """Return the values `name` is assigned inside `function`."""
        return [
            inner.value
            for inner in ast.walk(function)
            if isinstance(inner, ast.Assign)
            and any(getattr(target, "id", None) == name for target in inner.targets)
        ]

    def bindings(self, function, name):
        """Return what `name` is bound to: its assignments, and as a loop variable the
        matching element of each row of the tuple the loop iterates."""
        values = self.assigned(function, name)
        for loop in (inner for inner in ast.walk(function) if isinstance(inner, ast.For)):
            names = [getattr(elt, "id", None) for elt in getattr(loop.target, "elts", [])]
            if name not in names:
                continue
            tables = [loop.iter]
            if isinstance(loop.iter, ast.Name):
                tables = self.assigned(function, loop.iter.id)
            values += [row.elts[names.index(name)] for table in tables for row in table.elts]
        return values

    def resolved(self, function, start):
        """Return the argv nodes `start` stands for inside `function`, following names.

        A worklist with a fixed number of rounds, not recursion (HISS-01, HISS-02).
        """
        found, frontier = [], [start]
        for _ in range(4):
            pending = []
            for node in frontier:
                if isinstance(node, ast.Name):
                    pending += self.bindings(function, node.id)
                else:
                    found.append(node)
            frontier = pending
        return found

    def started_programs(self):
        """Return {program} for every argv any invoke() call in the gate passes."""
        programs = set()
        for function in self.functions.values():
            for call in ast.walk(function):
                if isinstance(call, ast.Call) and getattr(call.func, "id", "") == "invoke":
                    for argv in self.resolved(function, call.args[2]):
                        programs.add(program_of(argv))
        return programs

    def test_the_programs_are_the_allowed_ones(self):
        self.assertEqual(self.started_programs(), ALLOWED_PROGRAMS)

    def test_only_invoke_calls_run(self):
        callers = {
            function.name
            for function in self.functions.values()
            for node in ast.walk(function)
            if isinstance(node, ast.Call) and getattr(node.func, "id", "") == "run"
        }
        self.assertEqual(callers, {"invoke"})

    def test_every_run_and_invoke_carries_a_deadline(self):
        for call in self.calls("run"):
            self.assertGreaterEqual(len(call.args), 2, f"no deadline at line {call.lineno}")
        for call in self.calls("invoke"):
            self.assertGreaterEqual(len(call.args), 4, f"no deadline at line {call.lineno}")

    def test_only_run_starts_a_process_and_every_wait_is_bounded(self):
        starters = [
            (function.name, node.attr)
            for function in self.functions.values()
            for node in ast.walk(function)
            if isinstance(node, ast.Attribute)
            and getattr(node.value, "id", "") == "subprocess"
            and node.attr in {"Popen", "run", "call", "check_output", "check_call"}
        ]
        self.assertEqual(starters, [("run", "Popen")])
        waits = [
            node
            for node in ast.walk(self.tree)
            if isinstance(node, ast.Call)
            and getattr(node.func, "attr", "") in {"wait", "communicate"}
        ]
        self.assertTrue(waits)
        for node in waits:
            self.assertIn("timeout", {keyword.arg for keyword in node.keywords}, node.lineno)

    def test_the_network_is_reached_only_by_the_fetch(self):
        """ls-remote, fetch and `go mod download` appear in the fetch functions only."""
        for word in ("ls-remote", "fetch", "download"):
            users = {
                function.name
                for function in self.functions.values()
                for node in ast.walk(function)
                if isinstance(node, ast.Constant) and node.value == word
            }
            with self.subTest(word=word):
                self.assertTrue(users)
                self.assertLessEqual(users, NETWORK_FUNCTIONS)
        body = ast.get_source_segment(text(GATE), self.functions["build_standin"])
        self.assertIn("offline=True", body)

    def test_the_offline_go_environment_forbids_the_proxy(self):
        self.assertEqual(gate.go_environment(offline=True)["GOPROXY"], "off")
        self.assertNotIn("GOPROXY", gate.go_environment(offline=False))
        for offline in (True, False):
            env = gate.go_environment(offline)
            self.assertEqual(env["GOTOOLCHAIN"], "local")
            self.assertEqual(env["CGO_ENABLED"], "0")

    def test_no_function_exceeds_the_length_or_complexity_limit(self):
        offenders = []
        for node in self.functions.values():
            length = node.end_lineno - node.lineno + 1
            statements = sum(1 for inner in ast.walk(node) if isinstance(inner, ast.stmt)) - 1
            if length > MAX_FUNCTION_LINES or complexity(node) > MAX_COMPLEXITY:
                offenders.append(f"{node.name}: {length} lines, complexity {complexity(node)}")
            elif statements > MAX_STATEMENTS:
                offenders.append(f"{node.name}: {statements} statements")
        self.assertEqual(offenders, [])

    def test_the_gate_never_evaluates_code(self):
        names = {node.id for node in ast.walk(self.tree) if isinstance(node, ast.Name)}
        self.assertFalse(names & {"eval", "exec", "compile"})


if __name__ == "__main__":
    unittest.main()
