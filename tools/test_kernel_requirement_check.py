"""Positive, negative and boundary coverage for the D94 requirement check (M10, E10-5).

The check decides `build/kernel-requirement.json` against a kernel's own
configuration by the rules the M18 crate states for `RequiredState`. These
tests need no kernel: every configuration below is text written here, so the
rules are exercised on every leg of the platform matrix.
"""

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path

import kernel_requirement_check as check

REQUIREMENT = check.load_requirement()
CORRELATION = REQUIREMENT["correlation-id"]


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


class ParseTests(unittest.TestCase):
    """A configuration text reads as Kconfig writes it."""

    def test_assignments_and_unset_lines_are_read(self):
        found = check.parse_config('CONFIG_A=y\nCONFIG_B=m\n# CONFIG_C is not set\nCONFIG_D="x"\n')
        self.assertEqual(
            found, {"CONFIG_A": "y", "CONFIG_B": "m", "CONFIG_C": "not set", "CONFIG_D": '"x"'}
        )

    def test_a_later_line_wins(self):
        """Boundary: Kconfig merges fragments in order, so the last assignment stands."""
        self.assertEqual(check.parse_config("CONFIG_A=y\nCONFIG_A=m\n"), {"CONFIG_A": "m"})

    def test_prose_comments_are_not_symbols(self):
        self.assertEqual(check.parse_config("# Linux/x86 7.2.8 Kernel Configuration\n"), {})

    def test_an_oversized_configuration_is_refused(self):
        """Negative: the bound is a refusal, not a truncation."""
        with self.assertRaises(check.RequirementError):
            check.parse_config("x" * (check.MAX_CONFIG_BYTES + 1))


class StateRuleTests(unittest.TestCase):
    """The four RequiredState rules, as crates/aegis-fabrica-defs/src/kernel.rs states them."""

    def test_each_state_is_satisfied_by_its_own_reading(self):
        self.assertTrue(check.satisfied("built-in", "y"))
        self.assertTrue(check.satisfied("module", "m"))
        self.assertTrue(check.satisfied("present", "y"))
        self.assertTrue(check.satisfied("present", "m"))
        self.assertTrue(check.satisfied("absent", "not set"))

    def test_module_and_built_in_do_not_stand_in_for_each_other(self):
        """Boundary: m does not satisfy built-in, and y does not satisfy module."""
        self.assertFalse(check.satisfied("built-in", "m"))
        self.assertFalse(check.satisfied("module", "y"))

    def test_an_unobserved_symbol_satisfies_nothing_not_even_absent(self):
        """Negative: an unanswered question is not a measured absence."""
        for state in check.STATES:
            self.assertFalse(check.satisfied(state, None))

    def test_n_reads_as_not_set_and_a_string_is_no_tristate(self):
        config = check.parse_config('CONFIG_A=n\nCONFIG_B="y"\n')
        self.assertEqual(check.observed_state(config, "CONFIG_A"), "not set")
        self.assertIsNone(check.observed_state(config, "CONFIG_B"))
        self.assertIsNone(check.observed_state(config, "CONFIG_ABSENT"))


class PayloadTests(unittest.TestCase):
    """The requirement file is read as the M18 schema states it."""

    def test_the_tracked_requirement_is_accepted(self):
        self.assertEqual(check.requirement_problems(REQUIREMENT), [])
        self.assertEqual(REQUIREMENT["schema"], check.SCHEMA)

    def test_a_payload_of_another_schema_is_refused(self):
        payload = dict(REQUIREMENT, schema="aegis.other.v1")
        self.assertTrue(any("schema" in line for line in check.requirement_problems(payload)))

    def test_a_row_outside_the_schema_is_refused(self):
        row = dict(REQUIREMENT["features"][0], state="sometimes", probe="guess")
        payload = dict(REQUIREMENT, features=[row])
        problems = check.requirement_problems(payload)
        self.assertEqual(len(problems), 2)

    def test_the_feature_bound_admits_64_rows_and_refuses_65(self):
        """Boundary: the schema's maxItems is the check's own bound."""
        row = REQUIREMENT["features"][0]
        self.assertEqual(check.requirement_problems(dict(REQUIREMENT, features=[row] * 64)), [])
        refused = check.requirement_problems(dict(REQUIREMENT, features=[row] * 65))
        self.assertTrue(any("1..64" in line for line in refused))

    def test_an_unreadable_file_is_a_requirement_error(self):
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "broken.json"
            path.write_bytes(b"{not json")
            with self.assertRaises(check.RequirementError):
                check.load_requirement(path)


class UnknownKeyTests(unittest.TestCase):
    """A key the M18 schema does not name is refused, as deny_unknown_fields refuses it."""

    def test_the_optional_keys_the_schema_names_are_admitted(self):
        abi = dict(REQUIREMENT["abi"], **{"module-abi": "7.2.8-lusoris1-realtime"})
        payload = dict(REQUIREMENT, abi=abi, artifact={"digest": "sha256:" + "0" * 64})
        self.assertEqual(check.requirement_problems(payload), [])

    def test_an_unknown_key_at_each_level_is_refused(self):
        """Negative: the top level, abi and a feature row are each held."""
        row = dict(REQUIREMENT["features"][0], note="x")
        abi = dict(REQUIREMENT["abi"], **{"maximum-release": "8"})
        payload = dict(REQUIREMENT, extra=1, abi=abi, features=[row])
        problems = check.requirement_problems(payload)
        self.assertEqual(len(problems), 3, problems)
        for key in ("'extra'", "'maximum-release'", "'note'"):
            self.assertTrue(any(key in line for line in problems), key)
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "requirement.json"
            path.write_text(json.dumps(dict(REQUIREMENT, extra=1)), encoding="utf-8")
            with self.assertRaises(check.RequirementError):
                check.load_requirement(path)

    def test_an_empty_module_abi_and_an_abi_that_is_no_object_are_refused(self):
        """Boundary: a present module-abi must name a release."""
        abi = dict(REQUIREMENT["abi"], **{"module-abi": ""})
        self.assertEqual(len(check.requirement_problems(dict(REQUIREMENT, abi=abi))), 1)
        self.assertEqual(len(check.requirement_problems(dict(REQUIREMENT, abi="6.12"))), 1)


class ModuleAbiTests(unittest.TestCase):
    """abi.module-abi, when fixed, must be the release exactly (unmet_identity's AbiMismatch)."""

    def fixed(self, module_abi):
        abi = dict(REQUIREMENT["abi"], **{"module-abi": module_abi})
        return dict(REQUIREMENT, abi=abi)

    def test_the_fixed_release_is_accepted(self):
        requirement = self.fixed("7.2.8-lusoris1-realtime")
        self.assertEqual(
            check.check(requirement, conforming_config(), "7.2.8-lusoris1-realtime")[1], []
        )

    def test_another_release_is_rejected_with_the_id(self):
        """Negative: the case a copy of the payload with another module-abi used to pass."""
        requirement = self.fixed("7.2.8-some-other-abi")
        rejected = check.check(requirement, conforming_config(), "7.2.8-lusoris1-realtime")[1]
        self.assertEqual(len(rejected), 1)
        self.assertIn(CORRELATION, rejected[0])
        self.assertIn("is not abi.module-abi 7.2.8-some-other-abi", rejected[0])

    def test_equality_is_exact_and_the_release_is_needed(self):
        """Boundary: a numerically equal release with another suffix is not the ABI."""
        requirement = self.fixed("7.2.8-lusoris1-realtime")
        self.assertEqual(len(check.check(requirement, conforming_config(), "7.2.8")[1]), 1)
        self.assertEqual(check.check(requirement, conforming_config())[1], [])
        self.assertEqual(check.check(REQUIREMENT, conforming_config(), "7.2.8-x")[1], [])


class ReleaseTests(unittest.TestCase):
    """E10-4's boundary rule, KernelRelease::at_least."""

    def test_the_minimum_itself_is_admitted(self):
        self.assertTrue(check.release_at_least("6.12", "6.12"))
        self.assertTrue(check.release_at_least("6.12.0-lusoris1", "6.12"))

    def test_the_release_below_is_refused(self):
        self.assertFalse(check.release_at_least("6.11.999", "6.12"))
        self.assertFalse(check.release_at_least("6.11", "6.12"))

    def test_components_compare_as_numbers_not_text(self):
        """Boundary: 6.100 is above 6.12, which a comparison of the text would get wrong."""
        self.assertTrue(check.release_at_least("7.2.8-lusoris1-realtime", "6.12"))
        self.assertTrue(check.release_at_least("6.100", "6.12"))
        self.assertEqual(check.numeric_components("7.2.8-lusoris1-realtime"), [7, 2, 8])


class DecisionTests(unittest.TestCase):
    """E10-5: every row decided and recorded; an unsatisfied row rejected by name."""

    def test_a_conforming_configuration_satisfies_every_row(self):
        rows, rejected = check.check(REQUIREMENT, conforming_config(), "7.2.8")
        self.assertEqual(rejected, [])
        self.assertEqual(len(rows), len(REQUIREMENT["features"]))
        self.assertTrue(all(row["satisfied"] for row in rows))

    def test_a_required_symbol_set_to_n_is_rejected_with_the_id_and_the_symbol(self):
        text = with_line(conforming_config(), "CONFIG_SCHED_CLASS_EXT", "CONFIG_SCHED_CLASS_EXT=n")
        _rows, rejected = check.check(REQUIREMENT, text)
        self.assertEqual(len(rejected), 1)
        self.assertIn(CORRELATION, rejected[0])
        self.assertIn(" CONFIG_SCHED_CLASS_EXT ", rejected[0])

    def test_a_module_row_is_satisfied_by_m_and_a_built_in_row_is_not(self):
        """Boundary: the D94 acceptance, both halves."""
        rows, rejected = check.check(REQUIREMENT, conforming_config())
        modules = [row for row in rows if row["required"] == "module"]
        self.assertTrue(modules and all(row["observed"] == "m" for row in modules))
        self.assertEqual(rejected, [])
        text = with_line(conforming_config(), "CONFIG_BPF_SYSCALL", "CONFIG_BPF_SYSCALL=m")
        self.assertIn("CONFIG_BPF_SYSCALL", check.check(REQUIREMENT, text)[1][0])

    def test_a_missing_line_is_rejected_as_unobserved(self):
        text = with_line(conforming_config(), "CONFIG_HZ_1000", None)
        rejected = check.check(REQUIREMENT, text)[1]
        self.assertEqual(len(rejected), 1)
        self.assertIn("unobserved", rejected[0])

    def test_another_architecture_and_a_low_release_are_rejected(self):
        text = with_line(conforming_config(), "CONFIG_X86_64", "CONFIG_ARM64=y")
        rejected = check.check(REQUIREMENT, text, "6.11.2")[1]
        self.assertTrue(any("architecture x86-64" in line for line in rejected))
        self.assertTrue(any("below abi.minimum-release" in line for line in rejected))


class CommandLineTests(unittest.TestCase):
    """The tool's exit code is its verdict."""

    def run_tool(self, text):
        with tempfile.TemporaryDirectory() as base:
            config = Path(base) / "config"
            config.write_text(text, encoding="utf-8")
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                code = check.main(["--config", str(config), "--release", "7.2.8"])
        return code, out.getvalue()

    def test_a_conforming_configuration_exits_zero(self):
        code, out = self.run_tool(conforming_config())
        self.assertEqual(code, 0)
        self.assertIn(f"PASS: correlation-id {CORRELATION}", out)

    def test_an_unsatisfied_configuration_exits_one(self):
        code, out = self.run_tool(with_line(conforming_config(), "CONFIG_BPF_LSM", None))
        self.assertEqual(code, 1)
        self.assertIn("rejected: correlation-id", out)

    def test_an_unreadable_requirement_exits_two(self):
        """Boundary: a check that could not run is neither a pass nor a rejection."""
        with tempfile.TemporaryDirectory() as base:
            requirement = Path(base) / "requirement.json"
            requirement.write_text(json.dumps({"schema": "other"}), encoding="utf-8")
            out = io.StringIO()
            with contextlib.redirect_stdout(out):
                code = check.main(["--config", str(requirement), "--requirement", str(requirement)])
        self.assertEqual(code, 2)


if __name__ == "__main__":
    unittest.main()
