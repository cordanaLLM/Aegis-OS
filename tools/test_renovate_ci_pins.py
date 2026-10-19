#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative and boundary coverage for Renovate CI tool pin manager (HISS-15).

Verifies that Renovate regex customManager extracts every *_VERSION pin in ci.yml
except PRAETOR_REF with expected datasource, depName, and currentValue. Covers
the packageRules that strip leading 'v' from GitHub release tags and document manual
SHA256 digest refreshing.
"""

import json
from pathlib import Path
import re
import unittest

ROOT = Path(__file__).resolve().parent.parent
CI = ROOT / ".github" / "workflows" / "ci.yml"
RENOVATE = ROOT / "renovate.json"

# Datasource and depName per pin. The version is read from ci.yml itself, so a
# Renovate bump of any pin keeps this test green.
EXPECTED_PINS = {
    "LEFTHOOK_VERSION": ("github-releases", "evilmartians/lefthook"),
    "REUSE_VERSION": ("pypi", "reuse"),
    "MARKDOWNLINT_VERSION": ("npm", "markdownlint-cli2"),
    "YAMLLINT_VERSION": ("pypi", "yamllint"),
    "FLAKE8_VERSION": ("pypi", "flake8"),
    "BLACK_VERSION": ("pypi", "black"),
    "SHFMT_VERSION": ("pypi", "shfmt-py"),
    "CODESPELL_VERSION": ("pypi", "codespell"),
    "GITLEAKS_VERSION": ("github-releases", "gitleaks/gitleaks"),
    "TRIVY_VERSION": ("github-releases", "aquasecurity/trivy"),
    "SEMGREP_VERSION": ("pypi", "semgrep"),
    "LIBVA_VERSION": ("github-releases", "intel/libva"),
    "MESON_VERSION": ("pypi", "meson"),
    "NINJA_VERSION": ("pypi", "ninja"),
}

EXPECTED_SHA256_DEPS = {
    "evilmartians/lefthook",
    "gitleaks/gitleaks",
    "aquasecurity/trivy",
    "intel/libva",
    "meson",
    "ninja",
}


def text(path):
    """Read path as UTF-8 text."""
    return path.read_text(encoding="utf-8")


def js_regex_to_python(pattern):
    """Translate JavaScript named capture groups (?<name>...) to Python (?P<name>...)."""
    return re.sub(r"\(\?<([a-zA-Z0-9_]+)>", r"(?P<\1>", pattern)


def ci_manager(config):
    """Return the customManager for ci.yml or None."""
    for manager in config.get("customManagers", []):
        for pattern in manager.get("managerFilePatterns", []):
            if "ci\\.yml" in pattern:
                return manager
    return None


def ci_env_versions(ci_text):
    """Return {key: value} for every *_VERSION pin in the workflow env block."""
    env_block = ci_text.split("\njobs:")[0]
    return dict(re.findall(r"^\s*([A-Z0-9_]+_VERSION):\s*\"([^\"]+)\"", env_block, re.MULTILINE))


def ci_env_version_keys(ci_text):
    """Return all *_VERSION keys declared in the workflow env block."""
    return set(ci_env_versions(ci_text))


def extract_pins_from_text(ci_text, manager):
    """Extract pins from ci_text using the customManager regex patterns."""
    pins = {}
    matches_seen = []
    for pattern in manager.get("matchStrings", []):
        py_regex = js_regex_to_python(pattern)
        for m in re.finditer(py_regex, ci_text):
            data = m.groupdict()
            key = data.get("envKey")
            if not key:
                m_key = re.search(r"([A-Z0-9_]+_VERSION):", m.group(0))
                key = m_key.group(1) if m_key else None
            if key:
                matches_seen.append(key)
                pins[key] = (data["datasource"], data["depName"], data["currentValue"])
    return pins, matches_seen


class PositiveTest(unittest.TestCase):
    """Positive: verification of valid extraction and Renovate rules."""

    def setUp(self):
        self.config = json.loads(text(RENOVATE))
        self.manager = ci_manager(self.config)
        self.ci_text = text(CI)

    def test_ci_manager_is_declared_with_expected_pattern(self):
        """Positive: renovate.json declares customManager targeting .github/workflows/ci.yml."""
        self.assertIsNotNone(self.manager, "Expected customManager for ci.yml in renovate.json")
        self.assertEqual(self.manager.get("customType"), "regex")
        patterns = self.manager.get("managerFilePatterns", [])
        self.assertTrue(any("ci\\.yml" in p for p in patterns))

    def test_every_version_pin_is_extracted_once_with_expected_metadata(self):
        """Positive: every *_VERSION pin is extracted once with expected metadata."""
        pins, matches_seen = extract_pins_from_text(self.ci_text, self.manager)
        versions = ci_env_versions(self.ci_text)
        self.assertEqual(len(matches_seen), len(EXPECTED_PINS))
        self.assertEqual(len(pins), len(EXPECTED_PINS))
        for key, expected_tuple in EXPECTED_PINS.items():
            with self.subTest(key=key):
                self.assertIn(key, pins)
                self.assertEqual(pins[key], (*expected_tuple, versions[key]))

    def test_extracted_env_keys_match_all_workflow_version_pins(self):
        """Positive: extracted pin keys match all *_VERSION keys in ci.yml env block."""
        env_keys = ci_env_version_keys(self.ci_text)
        pins, _ = extract_pins_from_text(self.ci_text, self.manager)
        self.assertEqual(set(pins.keys()), env_keys)

    def test_manager_strips_leading_v_tags(self):
        """Positive: v-tagged releases resolve to the bare version the pins use."""
        pattern = re.compile(js_regex_to_python(self.manager["extractVersionTemplate"]))
        for tag in ("v2.2.2", "2.24.1"):
            match = pattern.match(tag)
            self.assertIsNotNone(match, tag)
            self.assertEqual(match.group("version"), tag.lstrip("v"))

    def test_package_rule_requires_manual_sha256_refresh(self):
        """Positive: packageRule defines prBodyNotes for tools with verified digests."""
        rules = [
            r
            for r in self.config.get("packageRules", [])
            if EXPECTED_SHA256_DEPS <= set(r.get("matchPackageNames", []))
        ]
        self.assertGreaterEqual(len(rules), 1)
        rule = rules[0]
        self.assertEqual(rule.get("matchFileNames"), [".github/workflows/ci.yml"])
        self.assertIn("sha256sum -c fails until then", rule.get("description", ""))
        notes = " ".join(rule.get("prBodyNotes", []))
        self.assertIn("refreshed by hand", notes)
        self.assertIn("sha256sum -c", notes)


class NegativeTest(unittest.TestCase):
    """Negative: failure cases on omitted or invalid annotations."""

    def setUp(self):
        self.config = json.loads(text(RENOVATE))
        self.manager = ci_manager(self.config)
        self.ci_text = text(CI)

    def test_pin_lacking_annotation_is_not_extracted_and_env_comparison_fails(self):
        """Negative: pin lacking annotation is not extracted and env comparison fails."""
        # Mutate ci.yml by stripping the renovate annotation above FLAKE8_VERSION
        mutated = re.sub(
            r"# renovate: datasource=pypi depName=flake8\s*\n\s*FLAKE8_VERSION:",
            "  FLAKE8_VERSION:",
            self.ci_text,
        )
        pins, _ = extract_pins_from_text(mutated, self.manager)
        env_keys = ci_env_version_keys(mutated)
        self.assertNotIn("FLAKE8_VERSION", pins)
        self.assertIn("FLAKE8_VERSION", env_keys)
        with self.assertRaises(AssertionError):
            self.assertEqual(set(pins.keys()), env_keys)

    def test_broken_annotation_without_depname_is_not_extracted(self):
        """Negative: annotation lacking depName fails to match pin."""
        mutated = re.sub(
            r"depName=flake8",
            "",
            self.ci_text,
        )
        pins, _ = extract_pins_from_text(mutated, self.manager)
        self.assertNotIn("FLAKE8_VERSION", pins)


class BoundaryTest(unittest.TestCase):
    """Boundary: edge cases including PRAETOR_REF and block boundaries."""

    def setUp(self):
        self.config = json.loads(text(RENOVATE))
        self.manager = ci_manager(self.config)
        self.ci_text = text(CI)

    def test_praetor_ref_is_never_extracted(self):
        """Boundary: PRAETOR_REF must stay outside every manager."""
        pins, _ = extract_pins_from_text(self.ci_text, self.manager)
        self.assertNotIn("PRAETOR_REF", pins)
        for _, dep_name, _ in pins.values():
            self.assertNotIn("praetor", dep_name.lower())

    def test_last_pin_in_block_matches_correctly(self):
        """Boundary: annotation for the last pin in env block still matches."""
        pins, _ = extract_pins_from_text(self.ci_text, self.manager)
        self.assertIn("NINJA_VERSION", pins)
        self.assertEqual(pins["NINJA_VERSION"], ("pypi", "ninja", "1.13.2"))

    def test_pins_with_preceding_comments_match(self):
        """Boundary: pins preceded by other comments still match cleanly."""
        pins, _ = extract_pins_from_text(self.ci_text, self.manager)
        self.assertIn("SHFMT_VERSION", pins)
        self.assertIn("GITLEAKS_VERSION", pins)
        self.assertIn("LIBVA_VERSION", pins)


if __name__ == "__main__":
    unittest.main()
