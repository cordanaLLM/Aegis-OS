"""Positive, negative and boundary coverage for the D94 requirement check through nucleus (M10, E10-5).

`tools/nucleus_kernel_check.py` hands `build/kernel-requirement.json` and one
kernel configuration to nucleus's single-kernel mode and reads its report. Two
kinds of case live here:

- a stub verifier written per test, which answers with a report this file
  states, holds the adapter's own refusals on every leg of the platform matrix;
- the pinned nucleus verifier from M09's contract cache holds the rules
  themselves. Those cases skip, naming `make contract-fetch`, on a checkout
  without the cache; CI fetches it before `make verify-all`.
"""

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

import nucleus_kernel_check as check

REQUIREMENT = check.load_requirement()
CORRELATION = REQUIREMENT["correlation-id"]
FLOOR = REQUIREMENT["abi"]["minimum-release"]


def conforming_config():
    """Return a configuration that states every feature exactly as the requirement asks."""
    lines = ["# generated for the test", "CONFIG_X86_64=y"]
    for feature in REQUIREMENT["features"]:
        state = feature["state"]
        if state == "absent":
            lines.append(f"# {feature['symbol']} is not set")
        else:
            lines.append(f"{feature['symbol']}={'m' if state == 'module' else 'y'}")
    return "\n".join(lines) + "\n"


def with_line(text, symbol, line):
    """Return `text` with `symbol`'s line replaced by `line`, or removed when None."""
    kept = [
        existing
        for existing in text.splitlines()
        if not existing.startswith(f"{symbol}=") and existing != f"# {symbol} is not set"
    ]
    if line is not None:
        kept.append(line)
    return "\n".join(kept) + "\n"


def nucleus_or_reason():
    """Return the pinned nucleus verifier, or the reason it cannot run here."""
    try:
        return check.verifier(), None
    except check.CheckError as error:
        return None, str(error)


NUCLEUS, NUCLEUS_MISSING = nucleus_or_reason()


def answer(release=None, reasons=(), **overrides):
    """Return the report nucleus writes when every row holds, with `overrides` applied."""
    checks = [
        {
            "symbol": feature["symbol"],
            "required": feature["state"],
            "observed": "m" if feature["state"] == "module" else "y",
            "satisfied": True,
            "required-by": feature["required-by"],
            "probe": feature["probe"],
        }
        for feature in REQUIREMENT["features"]
    ]
    report = {
        "schema": check.REPORT_SCHEMA,
        "correlation-id": CORRELATION,
        "release": release,
        "checks": checks,
        "reasons": list(reasons),
        "satisfied": not reasons,
    }
    report.update(overrides)
    return report


def stub(base, report, code):
    """Write a verifier that answers with `report` and exits `code`; return its path."""
    script = Path(base) / "stub_verifier.py"
    script.write_text(
        "import sys\n"
        "argv = sys.argv\n"
        'path = argv[argv.index("--report-json") + 1]\n'
        f'open(path, "w", encoding="utf-8").write({json.dumps(json.dumps(report))})\n'
        f"sys.exit({code})\n",
        encoding="utf-8",
    )
    return script


def stubbed(report, code, release=None):
    """Run the check against a stub verifier; return ``(rows, rejections)``."""
    with tempfile.TemporaryDirectory() as base:
        script = stub(base, report, code)
        return check.check(REQUIREMENT, conforming_config(), release, script=script)


class StubAnswerTests(unittest.TestCase):
    """The adapter reads nucleus's report, and refuses one that is no answer."""

    def test_a_satisfied_report_gives_every_row_and_no_rejection(self):
        rows, rejected = stubbed(answer(), 0)
        self.assertEqual(rejected, [])
        self.assertEqual([row["symbol"] for row in rows],
                         [feature["symbol"] for feature in REQUIREMENT["features"]])
        self.assertTrue(all(row["satisfied"] for row in rows))

    def test_a_rejected_report_gives_nucleus_reasons(self):
        reason = f"{CORRELATION}: CONFIG_HZ_1000 (required-by x) requires built-in, observed unrecorded"
        report = answer(reasons=[reason])
        report["checks"][1].update(observed=None, satisfied=False)
        rows, rejected = stubbed(report, 1)
        self.assertEqual(rejected, [f"rejected: {reason}"])
        self.assertEqual(rows[1]["observed"], check.UNOBSERVED)
        self.assertFalse(rows[1]["satisfied"])

    def test_an_unset_observation_reads_as_not_set(self):
        """Boundary: nucleus's ``n`` is the row's ``not set``."""
        self.assertEqual(check.observed_text("n"), check.NOT_SET)
        self.assertEqual(check.observed_text(None), check.UNOBSERVED)
        self.assertEqual(check.observed_text("m"), "m")

    def test_the_release_is_passed_and_must_come_back(self):
        rows, rejected = stubbed(answer(release="7.2.8"), 0, release="7.2.8")
        self.assertEqual(rejected, [])
        with self.assertRaisesRegex(check.CheckError, "release"):
            stubbed(answer(release="7.2.9"), 0, release="7.2.8")

    def test_reports_that_are_no_answer_are_refused(self):
        """Negative: each disagreement is a refusal, never a pass."""
        short = answer()
        short["checks"] = short["checks"][:-1]
        renamed = answer()
        renamed["checks"][0]["symbol"] = "CONFIG_OTHER"
        restated = answer()
        restated["checks"][0]["required"] = "module"
        cases = {
            "schema": (answer(schema="nucleus.other.v1"), 0),
            "correlation-id": (answer(**{"correlation-id": "another"}), 0),
            "exit 1": (answer(), 1),
            "exit 0": (answer(reasons=["x"]), 0),
            "satisfied": (answer(satisfied=False), 0),
            "rows": (short, 0),
            "symbol": (renamed, 0),
            "required": (restated, 0),
            "reasons": (answer(reasons="none"), 0),
        }
        for name, (report, code) in cases.items():
            with self.subTest(name), self.assertRaisesRegex(check.CheckError, "no answer"):
                stubbed(report, code)

    def test_a_requirement_other_than_the_checked_file_is_refused(self):
        """Negative: rows decided for the file are not passed off as another payload's."""
        other = dict(REQUIREMENT, **{"correlation-id": "another"})
        with tempfile.TemporaryDirectory() as base:
            script = stub(base, answer(), 0)
            with self.assertRaisesRegex(check.CheckError, "not the one nucleus checked"):
                check.check(other, conforming_config(), script=script)

    def test_a_check_that_could_not_run_is_an_error(self):
        """Boundary: exit 2 is neither a pass nor a rejection."""
        with self.assertRaisesRegex(check.CheckError, "exited 2"):
            stubbed(answer(), 2)

    def test_a_report_with_a_repeated_key_is_refused(self):
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "report.json"
            path.write_text('{"schema": 1, "schema": 2}', encoding="utf-8")
            with self.assertRaisesRegex(check.CheckError, "duplicate"):
                check.read_json(path)


class VerifierLocationTests(unittest.TestCase):
    """Only the pinned nucleus checkout answers."""

    def checkout(self, base, head):
        pin = {"nucleus": {"verifier": "scripts/verify.py", "commit": "a" * 40}}
        script = Path(base) / "nucleus" / "scripts" / "verify.py"
        script.parent.mkdir(parents=True)
        script.write_text("", encoding="utf-8")
        (Path(base) / "nucleus" / ".git").mkdir()
        (Path(base) / "nucleus" / ".git" / "HEAD").write_text(head + "\n", encoding="utf-8")
        return pin, script

    def test_the_pinned_checkout_is_used(self):
        with tempfile.TemporaryDirectory() as base:
            pin, script = self.checkout(base, "a" * 40)
            self.assertEqual(check.verifier(base, pin), script)

    def test_another_commit_is_refused(self):
        with tempfile.TemporaryDirectory() as base:
            pin, _script = self.checkout(base, "b" * 40)
            with self.assertRaisesRegex(check.CheckError, "make contract-fetch"):
                check.verifier(base, pin)

    def test_a_missing_checkout_is_refused(self):
        """Boundary: no cache names the target that fetches it."""
        pin = {"nucleus": {"verifier": "scripts/verify.py", "commit": "a" * 40}}
        with tempfile.TemporaryDirectory() as base:
            with self.assertRaisesRegex(check.CheckError, "make contract-fetch"):
                check.verifier(base, pin)


@unittest.skipIf(NUCLEUS is None, f"the pinned nucleus verifier is not here: {NUCLEUS_MISSING}")
class NucleusRuleTests(unittest.TestCase):
    """The D94 rules, decided by the pinned nucleus verifier itself."""

    def decide(self, text, release=None):
        return check.check(REQUIREMENT, text, release)

    def test_a_conforming_configuration_satisfies_every_row(self):
        rows, rejected = self.decide(conforming_config(), FLOOR)
        self.assertEqual(rejected, [])
        self.assertEqual(len(rows), len(REQUIREMENT["features"]))

    def test_a_missing_required_option_is_rejected(self):
        """Negative, the planted defect: a configuration without one required option."""
        rows, rejected = self.decide(with_line(conforming_config(), "CONFIG_HZ_1000", None))
        self.assertTrue(any("CONFIG_HZ_1000" in line and "unrecorded" in line for line in rejected),
                        rejected)
        row = next(row for row in rows if row["symbol"] == "CONFIG_HZ_1000")
        self.assertEqual((row["observed"], row["satisfied"]), (check.UNOBSERVED, False))

    def test_an_unset_required_option_is_rejected(self):
        text = with_line(conforming_config(), "CONFIG_BPF_LSM", "# CONFIG_BPF_LSM is not set")
        rows, rejected = self.decide(text)
        self.assertTrue(any("CONFIG_BPF_LSM" in line for line in rejected), rejected)
        row = next(row for row in rows if row["symbol"] == "CONFIG_BPF_LSM")
        self.assertEqual(row["observed"], check.NOT_SET)

    def test_module_and_built_in_do_not_stand_in_for_each_other(self):
        """Boundary: m does not satisfy built-in, and y does not satisfy module."""
        as_module = with_line(conforming_config(), "CONFIG_BPF_SYSCALL", "CONFIG_BPF_SYSCALL=m")
        built_in = with_line(conforming_config(), "CONFIG_KVM", "CONFIG_KVM=y")
        self.assertTrue(any("CONFIG_BPF_SYSCALL" in line for line in self.decide(as_module)[1]))
        self.assertTrue(any("CONFIG_KVM" in line for line in self.decide(built_in)[1]))

    def test_the_release_floor_is_inclusive(self):
        """Boundary: the floor itself is admitted; the release just below it is not."""
        self.assertEqual(self.decide(conforming_config(), FLOOR)[1], [])
        rejected = self.decide(conforming_config(), "6.11.999")[1]
        self.assertTrue(any("below abi.minimum-release" in line for line in rejected), rejected)

    def test_another_architecture_is_rejected(self):
        text = with_line(conforming_config(), "CONFIG_X86_64", "CONFIG_ARM64=y")
        self.assertNotEqual(self.decide(text)[1], [])


class CommandLineTests(unittest.TestCase):
    """The tool's exit code is its verdict."""

    def run_tool(self, text, *extra):
        with tempfile.TemporaryDirectory() as base:
            config = Path(base) / "config"
            config.write_text(text, encoding="utf-8")
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                code = check.main(["--config", str(config), *extra])
        return code, out.getvalue()

    @unittest.skipIf(NUCLEUS is None, f"the pinned nucleus verifier is not here: {NUCLEUS_MISSING}")
    def test_a_conforming_configuration_exits_zero(self):
        code, out = self.run_tool(conforming_config(), "--release", "7.2.8")
        self.assertEqual(code, 0, out)
        self.assertIn(f"PASS: correlation-id {CORRELATION}", out)

    @unittest.skipIf(NUCLEUS is None, f"the pinned nucleus verifier is not here: {NUCLEUS_MISSING}")
    def test_an_unsatisfied_configuration_exits_one(self):
        code, out = self.run_tool(with_line(conforming_config(), "CONFIG_BPF_LSM", None))
        self.assertEqual(code, 1, out)
        self.assertIn("rejected: ", out)

    def test_an_unreadable_requirement_exits_two(self):
        """Boundary: a check that could not run is neither a pass nor a rejection."""
        with tempfile.TemporaryDirectory() as base:
            requirement = Path(base) / "requirement.json"
            requirement.write_text(json.dumps({"schema": "other"}), encoding="utf-8")
            code, out = self.run_tool(conforming_config(), "--requirement", str(requirement))
        self.assertEqual(code, 2, out)


if __name__ == "__main__":
    unittest.main()
