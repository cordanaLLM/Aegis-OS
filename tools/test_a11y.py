#!/usr/bin/env python3
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
"""Positive, negative and boundary coverage for the M04 accessibility gate (HISS-15).

The gate itself needs a container engine, the pinned Playwright image and a
filled cache. These tests need none of them, and they run on every leg of the
platform matrix (HISS-21): they cover each decision the gate makes on the way
-- the pin and the digest it accepts, the cross-check of package.json and the
lockfile, the engine it picks, the container command line, the cache states
that skip and the ones that fail, how it reads the suite's report -- and the
static half of M04's criteria: the D76 rule that every colour, stroke and
motion value is a token, the D81 tag set and clause map, the suite carrying no
suppression, and no `|| true` in any surface the gate runs through. The D100
lint M16 added is held here too: its configuration, its local rule, its planted
violations and how the gate reads ESLint's report. The sweeps at the end hold
the gate to HISS-02, HISS-04 and HISS-08.
"""

import ast
import contextlib
import hashlib
import io
import json
import re
import tarfile
import tempfile
import unittest
from pathlib import Path
from unittest import mock

import verify_a11y as gate
from test_bpf_objects import recipe_lines, suppresses

ROOT = Path(__file__).resolve().parent.parent
GATE = ROOT / "tools" / "verify_a11y.py"
PACKAGE = ROOT / "ui" / "concordia-tokens"
TOKENS = PACKAGE / "concordia-tokens.css"
COMPONENT = PACKAGE / "src" / "ConcordiaPanel.svelte"
SPEC = PACKAGE / "tests" / "a11y.spec.js"
HARNESS = PACKAGE / "tests" / "harness.js"
PROBE = PACKAGE / "tests" / "probe.js"
STUB = PACKAGE / "tests" / "fixtures" / "stub-heads-up-theme.css"
CONFIG = PACKAGE / "playwright.config.js"
LINT_CONFIG = PACKAGE / "eslint.config.js"
LINT_RULE = PACKAGE / "hiss-lint" / "no-self-recursion.js"
PLANT_DIR = PACKAGE / "hiss-lint" / "plants"
MAKEFILE = ROOT / "Makefile"
CI = ROOT / ".github" / "workflows" / "ci.yml"
RENOVATE = ROOT / "renovate.json"
ADMISSION = ROOT / "docs" / "roadmap" / "toolchain-admission.md"
ROADMAP = ROOT / "docs" / "roadmap" / "README.md"
EVIDENCE_PAGE = ROOT / "docs" / "build" / "accessibility-harness.md"
MKDOCS = ROOT / "mkdocs.yml"
ADMISSION_HEADING = "## The accessibility gate's toolchain (M04)"
LINT_ADMISSION_HEADING = "## The ui/ HISS lint's toolchain (M16, D100)"
MAX_FUNCTION_LINES = 60
MAX_COMPLEXITY = 10
MAX_STATEMENTS = 50
BRANCHES = (ast.If, ast.For, ast.While, ast.IfExp, ast.ExceptHandler, ast.With, ast.Assert)

# What the gate may start on the host, and what it may start inside the
# container. Nothing here installs a package on the host or pulls outside
# --fetch; a program added to the gate without being added here fails.
ALLOWED_PROGRAMS = {"podman", "docker", "curl"}
CONTAINER_PROGRAMS = {"cp", "pnpm", "node"}
# D17: design tokens and no monolithic CSS library. The package declares
# exactly these, and a CSS framework added to it fails here.
ADMITTED_PACKAGES = {
    "@axe-core/playwright": "4.13.0",
    "@playwright/test": "1.63.0",
    "@sveltejs/vite-plugin-svelte": "7.3.1",
    "axe-core": "4.13.0",
    "svelte": "5.57.1",
    "vite": "8.3.1",
}
# D100 (M16): the lint's three packages, exact; with the six above they are
# every devDependency the package declares.
ADMITTED_LINT = {
    "eslint": "10.11.0",
    "eslint-plugin-svelte": "3.23.0",
    "svelte-eslint-parser": "1.8.1",
}

DECLARATION = re.compile(r"([a-z-]+)\s*:\s*([^;{}]+);")
TOKEN_PROPERTIES = re.compile(
    r"(?:color|background(?:-color)?|fill|stroke(?:-width)?|caret-color|accent-color"
    r"|text-decoration-color|box-shadow"
    r"|border(?:-(?:top|right|bottom|left))?(?:-(?:color|width|style))?"
    r"|outline(?:-(?:color|width|style|offset))?"
    r"|transition(?:-(?:duration|timing-function|delay))?"
    r"|animation(?:-(?:duration|timing-function|delay|iteration-count))?)"
)
VAR_ONLY = re.compile(r"(?:var\(--concordia-[a-z0-9-]+\)\s*)+")
INNERMOST_BLOCK = re.compile(r"([^{}]*)\{([^{}]*)\}")

# Spellings that would let the suite pass a failing test: a skip, a fixme, an
# expected failure (Playwright reports it as ok), a retry, a focused test, a
# rule exclusion or an impact filter. None may appear in the suite's sources.
FORBIDDEN_IN_SUITE = (
    "test.skip",
    "test.fixme",
    "test.fail",
    ".skip(",
    ".fixme(",
    ".fail(",
    "testInfo",
    "test.info(",
    "configure(",
    "retries",
    ".only(",
    "disableRules",
    ".exclude(",
    "violations.filter",
    "impact ===",
    "impact !==",
)
# One planted line per way of writing a suppression; each must be caught.
PLANTED_SUPPRESSIONS = (
    "test.fail();",
    "test.fail(true, 'known to clip');",
    "test.skip();",
    "test.describe.skip('focus', () => {});",
    "test.fixme('focus', async () => {});",
    "test('x', async ({ page }, testInfo) => { testInfo.skip(); });",
    "test('x', async ({ page }, testInfo) => { testInfo.fail(); });",
    "test.info().fixme();",
    "test.describe.configure({ retries: 2 });",
    "test.only('x', async () => {});",
    "new AxeBuilder({ page }).disableRules(['region']);",
    "new AxeBuilder({ page }).exclude('main');",
    "results.violations.filter((v) => v.impact === 'critical');",
    "if (v.impact !== 'minor') {}",
)


def text(path):
    """Return a tracked file's text with LF line ends."""
    return path.read_bytes().decode("utf-8").replace("\r\n", "\n")


def without_comments(css):
    return re.sub(r"/\*.*?\*/", "", css, flags=re.S)


def literal_declarations(css):
    """Return the colour, stroke and motion declarations whose value is not var()-only (D76)."""
    offenders = []
    for prop, value in DECLARATION.findall(without_comments(css)):
        if prop.startswith("--") or not TOKEN_PROPERTIES.fullmatch(prop):
            continue
        if not VAR_ONLY.fullmatch(" ".join(value.split()).strip()):
            offenders.append(f"{prop}: {' '.join(value.split())}")
    return offenders


def token_blocks(css):
    """Return the selectors of the innermost blocks that declare a Concordia token."""
    return [
        " ".join(selector.split())
        for selector, body in INNERMOST_BLOCK.findall(without_comments(css))
        if "--concordia-" in body and re.search(r"--concordia-[a-z0-9-]+\s*:", body)
    ]


def suite_suppressions(source):
    """Return the forbidden spellings `source` contains."""
    return [spelling for spelling in FORBIDDEN_IN_SUITE if spelling in source]


def style_block(svelte):
    match = re.search(r"<style>(.*?)</style>", svelte, flags=re.S)
    return match.group(1) if match else ""


def fixture_pin():
    """Return a deep copy of the committed pin file, before load_pin adds its derived keys."""
    return json.loads(text(gate.PIN))


def written(directory, name, data):
    """Write `data` (bytes) into `directory` and return the path. Bytes, so no host rewrites it."""
    path = Path(directory) / name
    path.write_bytes(data)
    return path


def json_bytes(value):
    return json.dumps(value, indent=2).encode("utf-8")


def context_in(directory, engine="podman"):
    """Return a gate Context whose cache and run directory sit in `directory`."""
    store = Path(directory) / "cache"
    context = gate.Context(engine, gate.load_pin(), store, store / "runs" / "r-test")
    context.node, context.pnpm = Path(directory) / "node", Path(directory) / "pnpm"
    return context


def quiet(function, *args):
    """Call `function` with stdout captured; return (result, printed text)."""
    buffer = io.StringIO()
    with contextlib.redirect_stdout(buffer):
        result = function(*args)
    return result, buffer.getvalue()


class PinTests(unittest.TestCase):
    """The pin names one image by digest and agrees with package.json and the lockfile."""

    def test_the_committed_pin_loads_with_its_derived_values(self):
        pin = gate.load_pin()
        self.assertEqual(pin["image"]["playwright"], "1.63.0")
        self.assertEqual(
            pin["image"]["by-digest"],
            "mcr.microsoft.com/playwright@sha256:"
            "eff16c30e6f3f4af0a03fa4b706120d5e9b0891c344a27d64559aff5900a4a27",
        )
        self.assertEqual(pin["image"]["browser"]["revision"], "1243")
        self.assertTrue(pin["pnpm"]["integrity"].startswith("sha512-qFWBneHJ"))

    def test_the_node_and_pnpm_lines_are_the_decided_ones(self):
        """D65: the 26.x line, exact; D43 as decided at M04: pnpm 12.x, exact."""
        pin = gate.load_pin()
        self.assertEqual(pin["node"]["version"].split(".")[0], "26")
        self.assertEqual(pin["pnpm"]["version"].split(".")[0], "12")
        self.assertEqual(pin["node"]["version"], "26.10.0")
        self.assertEqual(pin["pnpm"]["version"], "12.6.0")

    def test_a_full_digest_pins_and_a_truncated_one_is_refused(self):
        digest = "sha256:" + "ab" * 32
        self.assertIsNone(gate.digest_problem(digest))
        for truncated in (digest[:-1], digest[:19], "sha256:", digest + "0", digest.upper()):
            with self.subTest(truncated=truncated):
                self.assertIn("truncated digest", gate.digest_problem(truncated))

    def test_the_truncated_digest_case_reports_one_accept_and_three_refusals(self):
        problems, printed = quiet(gate.truncated_digest_case, gate.load_pin())
        self.assertEqual(problems, [])
        self.assertEqual(printed.count(": refused"), 3)
        self.assertEqual(printed.count(": accepted"), 1)
        self.assertIn(f"{gate.SOURCE_REFERENCE}: refused", printed)

    def test_the_sources_own_truncated_digest_is_refused(self):
        """REQ-P12-04's source names another image by six hex digits and an ellipsis."""
        repository, _, digest = gate.SOURCE_REFERENCE.partition("@")
        self.assertEqual(repository, "ghcr.io/lusoris/concordia-validate")
        self.assertEqual(digest, "sha256:8f47c3...")
        self.assertIn("truncated digest", gate.digest_problem(digest))
        self.assertIn("truncated digest", gate.digest_problem(digest.rstrip(".")))

    def test_each_pin_defect_is_refused(self):
        defects = {
            "schema": lambda pin: pin.update(schema="other"),
            "section": lambda pin: pin.pop("node"),
            "tag": lambda pin: pin["image"].update(
                reference="mcr.microsoft.com/playwright@sha256:"
            ),
            "platform": lambda pin: pin["image"]["platform-digests"].update({"x": "sha256:ab"}),
            "revision": lambda pin: pin["image"]["browser"].update(revision="r1243"),
            "node-version": lambda pin: pin["node"].update(version="26.10"),
            "node-sha": lambda pin: pin["node"].update(sha256="ab"),
        }
        for name, change in defects.items():
            with self.subTest(defect=name):
                pin = fixture_pin()
                change(pin)
                self.assertTrue(gate.pin_problems(pin), name)
        self.assertEqual(gate.pin_problems(fixture_pin()), [])

    def test_package_json_and_the_lockfile_must_agree_with_the_pin(self):
        pin, lock = fixture_pin(), text(gate.LOCKFILE)
        manifest = json.loads(text(gate.MANIFEST))
        self.assertEqual(gate.manifest_problems(pin, manifest, lock), [])
        drifts = {
            "packageManager": ("packageManager", "pnpm@12.5.0"),
            "not pnpm": ("packageManager", "npm@11.0.0"),
            "engines.node": ("engines", {"node": "24.20.0", "pnpm": "12.6.0"}),
            "range": ("devDependencies", dict(manifest["devDependencies"], svelte="^5.57.1")),
            "playwright": (
                "devDependencies",
                dict(manifest["devDependencies"], **{"@playwright/test": "1.62.0"}),
            ),
        }
        for name, (key, value) in drifts.items():
            with self.subTest(drift=name):
                self.assertTrue(gate.manifest_problems(pin, dict(manifest, **{key: value}), lock))
        stale = lock.replace("version: 12.6.0", "version: 12.5.0", 1)
        self.assertTrue(gate.manifest_problems(pin, manifest, stale))

    def test_the_pnpm_integrity_is_read_from_the_lockfile(self):
        lock = text(gate.LOCKFILE)
        self.assertRegex(gate.lock_integrity(lock, "12.6.0"), r"^sha512-[A-Za-z0-9+/]{86}==$")
        with self.assertRaises(gate.GateError):
            gate.lock_integrity(lock, "12.5.0")

    def test_a_pin_that_is_not_json_is_a_gate_error(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            path = written(base, "pin.json", b"{not json")
            with self.assertRaises(gate.GateError):
                gate.load_pin(path)


class SuppressionTests(unittest.TestCase):
    """No surface the gate runs through can turn a failure into a pass (REQ-CI-02)."""

    def test_package_scripts_carry_no_suppression(self):
        scripts = json.loads(text(gate.MANIFEST))["scripts"]
        self.assertEqual(gate.script_problems(scripts), [])
        self.assertEqual(scripts["test:a11y"], "playwright test")
        self.assertEqual(scripts["lint:hiss"], "eslint --max-warnings 0 .")

    def test_every_suppressing_spelling_is_caught(self):
        for command in (
            "playwright test || true",
            "playwright test; true",
            "playwright test & wait",
            "playwright test | tee log",
            "-playwright test",
        ):
            with self.subTest(command=command):
                self.assertTrue(gate.script_problems({"test": command}))
        self.assertEqual(gate.script_problems({"build": "vite build"}), [])

    def test_the_make_targets_run_the_gate_unsuppressed(self):
        makefile = text(MAKEFILE)
        self.assertEqual(recipe_lines(makefile, "verify-a11y"), ["python3 tools/verify_a11y.py"])
        self.assertEqual(
            recipe_lines(makefile, "a11y-fetch"), ["python3 tools/verify_a11y.py --fetch"]
        )
        verify_all = recipe_lines(makefile, "verify-all")
        wired = [line for line in verify_all if "verify-a11y" in line]
        self.assertEqual(wired, ["$(MAKE) --no-print-directory verify-a11y"])
        for line in (
            wired + recipe_lines(makefile, "verify-a11y") + recipe_lines(makefile, "a11y-fetch")
        ):
            self.assertFalse(suppresses(line), line)

    def test_the_recipe_check_bites_on_a_planted_suppression(self):
        self.assertTrue(suppresses("python3 tools/verify_a11y.py || true"))
        self.assertTrue(suppresses("-python3 tools/verify_a11y.py"))
        self.assertFalse(suppresses("python3 tools/verify_a11y.py"))

    def test_ci_fetches_before_the_gate_and_suppresses_nothing(self):
        workflow = text(CI)
        lines = [line.strip() for line in workflow.splitlines()]
        self.assertIn("run: make a11y-fetch", lines)
        self.assertLess(lines.index("run: make a11y-fetch"), lines.index("run: make verify-all"))
        self.assertNotIn("continue-on-error", workflow)
        self.assertNotIn("|| true", workflow)
        for line in (line for line in lines if line.startswith("uses:")):
            self.assertRegex(line, r"@[0-9a-f]{40} # v\d", line)

    def test_the_suite_skips_filters_and_retries_nothing(self):
        suite = text(SPEC) + text(HARNESS) + text(PROBE)
        self.assertEqual(suite_suppressions(suite), [])
        config = text(CONFIG)
        self.assertIn("retries: 0", config)
        self.assertIn("forbidOnly: true", config)

    def test_every_planted_suppression_is_caught(self):
        for planted in PLANTED_SUPPRESSIONS:
            with self.subTest(planted=planted):
                self.assertTrue(suite_suppressions(text(SPEC) + "\n" + planted))
        self.assertEqual(suite_suppressions("test('x', async ({ page }) => {});"), [])


class TokenTests(unittest.TestCase):
    """D76: every colour, stroke and motion value is a token; D16 and D17 hold."""

    def test_the_token_file_and_the_component_use_tokens_only(self):
        self.assertEqual(literal_declarations(text(TOKENS)), [])
        self.assertEqual(literal_declarations(style_block(text(COMPONENT))), [])
        self.assertGreater(len(DECLARATION.findall(style_block(text(COMPONENT)))), 20)

    def test_a_literal_colour_stroke_or_motion_value_is_caught(self):
        for planted in (
            ".a { outline: 3px solid #000; }",
            ".a { color: red; }",
            ".a { outline: var(--concordia-focus-ring-width) 3px; }",
            ".a { transition-duration: 150ms; }",
            ".a { border-top-width: 1px; }",
        ):
            with self.subTest(planted=planted):
                self.assertTrue(literal_declarations(planted))
        accepted = ".a { outline: var(--concordia-a) var(--concordia-b)\n var(--concordia-c); }"
        self.assertEqual(literal_declarations(accepted), [])
        self.assertEqual(literal_declarations(".a { margin: -1px; font: inherit; }"), [])

    def test_token_values_are_declared_on_root_only(self):
        blocks = token_blocks(text(TOKENS))
        self.assertEqual(set(blocks), {":root"})
        self.assertEqual(len(blocks), 3, "the base block, reduced motion and forced colours")
        self.assertEqual(token_blocks(".x { --concordia-a: 1px; }"), [".x"])

    def test_the_d16_token_is_3px_and_the_floor_is_2px(self):
        tokens = text(TOKENS)
        self.assertIn("--concordia-focus-ring-width: 3px;", tokens)
        probe = text(PROBE)
        self.assertIn("const FOCUS_FLOOR_PX = 2;", probe)
        self.assertIn("const CONTRAST_FLOOR = 3;", probe)

    def test_reduced_motion_zeroes_the_motion_token(self):
        tokens = without_comments(text(TOKENS))
        block = tokens.split("@media (prefers-reduced-motion: reduce)", 1)[1].split("}", 2)[0]
        self.assertIn("--concordia-motion-duration: 0ms;", block)

    def test_the_d76_stub_overrides_one_token_below_the_floor(self):
        stub = without_comments(text(STUB))
        self.assertEqual(DECLARATION.findall(stub), [("--concordia-focus-ring-width", "1px")])
        self.assertNotIn("stub-heads-up-theme", text(PACKAGE / "src" / "main.js"))

    def test_d17_no_monolithic_css_library_is_declared(self):
        manifest = json.loads(text(gate.MANIFEST))
        self.assertEqual(manifest["devDependencies"], ADMITTED_PACKAGES | ADMITTED_LINT)
        self.assertNotIn("dependencies", manifest)


class ClauseMapTests(unittest.TestCase):
    """D81: both editions per criterion, 'none in V3.2.1' for the WCAG 2.2 additions."""

    clauses = json.loads(text(gate.CLAUSE_MAP))

    def test_every_a_and_aa_criterion_of_wcag_22_is_mapped(self):
        criteria = self.clauses["criteria"]
        levels = [row["level"] for row in criteria.values()]
        self.assertEqual(levels.count("A"), 31)
        self.assertEqual(levels.count("AA"), 24)
        self.assertEqual(criteria["4.1.1"]["level"], "obsolete")
        for number, row in criteria.items():
            self.assertTrue(row["v4.1.1"].startswith(f"11.{number}") or "none" in row["v4.1.1"])

    def test_the_d81_rows_read_as_the_decision_says(self):
        criteria = self.clauses["criteria"]
        for number in ("2.4.11", "2.5.7", "2.5.8", "3.3.7", "3.3.8"):
            with self.subTest(criterion=number):
                self.assertEqual(criteria[number]["v4.1.1"], f"11.{number}")
                self.assertEqual(criteria[number]["v3.2.1"], "none in V3.2.1")
        self.assertEqual(criteria["3.2.6"]["v4.1.1"], "11.3.2.6 (Void)")
        self.assertEqual(criteria["2.1.1"]["v3.2.1"], "11.2.1.1.1", "open functionality")
        self.assertEqual(self.clauses["not-covered-by-axe"], ["2.4.11", "2.5.7"])

    def test_the_sources_are_the_digests_the_register_cites(self):
        register = text(ROADMAP)
        for name, source in self.clauses["sources"].items():
            with self.subTest(source=name):
                self.assertIn(source["sha256"], register)

    def test_the_gate_and_the_suite_use_the_same_tag_set(self):
        self.assertEqual(gate.D81_TAGS, ["wcag2a", "wcag2aa", "wcag21a", "wcag21aa", "wcag22aa"])
        declared = re.search(r"export const D81_TAGS = \[([^\]]*)\];", text(HARNESS))
        self.assertIsNotNone(declared)
        self.assertEqual(re.findall(r"'([^']+)'", declared.group(1)), gate.D81_TAGS)


class SuiteShapeTests(unittest.TestCase):
    """The gate's expectations about the suite match the suite that is committed."""

    titles = re.findall(r"^test\(\s*'([^']+)'", text(SPEC), flags=re.M)

    def test_the_expected_test_count_is_the_number_of_tests(self):
        self.assertEqual(len(self.titles), gate.EXPECTED_TESTS)

    def test_each_planted_defect_selects_one_existing_test(self):
        plants = set(re.findall(r"^  '([a-z-]+)': async", text(HARNESS), flags=re.M))
        self.assertEqual(plants, {plant for _name, plant, _title, _needle in gate.PLANTS})
        for _name, _plant, title, _needle in gate.PLANTS:
            with self.subTest(title=title):
                self.assertEqual(sum(1 for t in self.titles if t.startswith(title)), 1)

    def test_emulation_results_are_labelled_as_such(self):
        labelled = [t for t in self.titles if t.startswith("[")]
        self.assertEqual(len(labelled), 3)
        for title in labelled:
            self.assertRegex(title, r"not portal evidence|not AT-SPI2")

    def test_the_package_entries_are_what_the_package_holds(self):
        present = {
            path.name
            for path in PACKAGE.iterdir()
            if path.name not in {"node_modules", "dist", "test-output", "toolchain.pin.json"}
        }
        self.assertEqual(present, set(gate.PACKAGE_ENTRIES))


def on_path(*present):
    """Return a shutil.which stand-in that finds exactly `present`."""

    def which(name):
        return f"/usr/bin/{name}" if name in present else None

    return which


class EngineTests(unittest.TestCase):
    """Which engine runs the container, and the command line it gets."""

    def test_rootless_podman_is_preferred_and_docker_is_the_fallback(self):
        with mock.patch.dict(gate.os.environ, {gate.ENGINE_VARIABLE: ""}):
            with mock.patch.object(gate.shutil, "which", side_effect=on_path("podman", "docker")):
                self.assertEqual(gate.pick_engine(), ("podman", None))
            with mock.patch.object(gate.shutil, "which", side_effect=on_path("docker")):
                self.assertEqual(gate.pick_engine(), ("docker", None))
            with mock.patch.object(gate.shutil, "which", return_value=None):
                engine, reason = gate.pick_engine()
        self.assertIsNone(engine)
        self.assertIn("neither podman nor docker", reason)

    def test_a_forced_engine_is_honoured_and_an_unknown_one_refused(self):
        with mock.patch.object(gate.shutil, "which", return_value="/usr/bin/x"):
            with mock.patch.dict(gate.os.environ, {gate.ENGINE_VARIABLE: "docker"}):
                self.assertEqual(gate.pick_engine(), ("docker", None))
            with mock.patch.dict(gate.os.environ, {gate.ENGINE_VARIABLE: "nerdctl"}):
                self.assertIsNone(gate.pick_engine()[0])

    def test_the_container_runs_offline_read_only_and_by_digest(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            context = context_in(base)
            argv = gate.container_argv(context, "n", ["pnpm", "run", "build"], {})
        for flag in ("--rm", "--pull=never", "--read-only", "--network=none"):
            self.assertIn(flag, argv)
        mounts = [argv[i + 1] for i, token in enumerate(argv) if token == "--mount"]
        self.assertTrue(any(m.endswith(",target=/repo,readonly") for m in mounts))
        self.assertTrue(any(m.endswith(",target=/offline,readonly") for m in mounts))
        self.assertTrue(any(m.endswith(",target=/work") for m in mounts))
        image = argv.index(context.pin["image"]["by-digest"])
        self.assertEqual(argv[image + 1 :], ["pnpm", "run", "build"])
        self.assertNotIn("--user", argv)

    def test_only_the_fetch_has_a_network_and_a_writable_store(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            argv = gate.container_argv(context_in(base), "n", ["pnpm", "fetch"], {"network": True})
        self.assertNotIn("--network=none", argv)
        self.assertTrue(any(token.endswith(",target=/offline") for token in argv))

    def test_docker_runs_as_the_invoking_user_where_the_host_has_one(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            context = context_in(base, engine="docker")
            with mock.patch.object(gate, "uid", return_value=(1000, None)):
                with mock.patch.object(gate, "gid", return_value=(1000, None)):
                    argv = gate.container_argv(context, "n", ["node"], {})
            self.assertEqual(argv[argv.index("--user") + 1], "1000:1000")
            with mock.patch.object(gate, "uid", return_value=(None, "no uid here")):
                with mock.patch.object(gate, "gid", return_value=(None, "no gid here")):
                    self.assertNotIn("--user", gate.container_argv(context, "n", ["node"], {}))

    def test_a_mount_source_with_a_comma_is_refused(self):
        with self.assertRaises(gate.GateError):
            gate.bind(Path("a,b"), "/work")

    def test_image_presence_is_read_from_the_repo_digests(self):
        image = gate.load_pin()["image"]
        found = json.dumps([image["by-digest"]])
        with mock.patch.object(gate, "run", return_value=(0, found, "")):
            self.assertTrue(gate.image_present("podman", image)[0])
        with mock.patch.object(gate, "run", return_value=(0, '["other@sha256:00"]', "")):
            self.assertFalse(gate.image_present("podman", image)[0])
        with mock.patch.object(gate, "run", return_value=(125, "", "no such image")):
            self.assertEqual(gate.image_present("podman", image), (False, []))
        with mock.patch.object(gate, "run", return_value=(0, "not json", "")):
            self.assertEqual(gate.image_present("podman", image), (False, []))


class CacheTests(unittest.TestCase):
    """An absent input skips the gate; a present one that does not verify fails it."""

    def archives(self, base, node_bytes, pnpm_bytes):
        store = Path(base)
        downloads = store / "downloads"
        downloads.mkdir(parents=True)
        pin = gate.load_pin()
        written(downloads, pin["node"]["file"], node_bytes)
        written(downloads, pin["pnpm"]["file"], pnpm_bytes)
        (store / "offline" / "store").mkdir(parents=True)
        written(store / "offline", gate.STORE_MARKER, pin["lock-sha256"].encode("ascii") + b"\n")
        return pin, store

    def verified(self, base):
        """Return a pin and a cache whose archives hash to that pin and whose store is marked."""
        pin, store = self.archives(base, b"node", b"pnpm")
        pin["node"]["sha256"] = hashlib.sha256(b"node").hexdigest()
        pin["pnpm"]["integrity"] = gate.file_digests(store / "downloads" / pin["pnpm"]["file"])[1]
        return pin, store

    def test_absent_inputs_are_reasons_to_skip(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            reasons, problems = gate.archive_state(gate.load_pin(), Path(base))
        self.assertEqual(problems, [])
        self.assertEqual(len(reasons), 3)
        self.assertTrue(all("make a11y-fetch" in reason for reason in reasons))

    def test_matching_inputs_admit_the_run(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            pin, store = self.verified(base)
            self.assertEqual(gate.archive_state(pin, store), ([], []))

    def test_a_cached_archive_that_does_not_verify_is_a_failure(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            pin, store = self.archives(base, b"tampered", b"tampered")
            reasons, problems = gate.archive_state(pin, store)
        self.assertEqual(reasons, [])
        self.assertEqual(len(problems), 2)

    def test_the_store_marker_is_the_lockfile_digest(self):
        pin = gate.load_pin()
        self.assertEqual(
            pin["lock-sha256"], hashlib.sha256(text(gate.LOCKFILE).encode("utf-8")).hexdigest()
        )

    def test_a_store_filled_for_this_lockfile_serves_it(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            pin, store = self.archives(base, b"node", b"pnpm")
            self.assertIsNone(gate.store_reason(store, pin["lock-sha256"]))
            written(store / "offline", gate.STORE_MARKER, pin["lock-sha256"].encode() + b"\r\n")
            self.assertIsNone(gate.store_reason(store, pin["lock-sha256"]), "CRLF marker")

    def test_a_store_filled_for_another_lockfile_is_a_skip_not_a_failure(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            pin, store = self.verified(base)
            wanted = pin["lock-sha256"]
            other = ("0" if wanted[0] != "0" else "1") + wanted[1:]
            written(store / "offline", gate.STORE_MARKER, other.encode("ascii"))
            reason = gate.store_reason(store, wanted)
            reasons, problems = gate.archive_state(pin, store)
        self.assertIn("filled for another pnpm-lock.yaml", reason)
        self.assertIn("make a11y-fetch", reason)
        self.assertEqual(problems, [])
        self.assertEqual(reasons, [reason])

    def test_a_store_without_its_marker_is_a_skip(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            pin, store = self.archives(base, b"node", b"pnpm")
            (store / "offline" / gate.STORE_MARKER).unlink()
            no_marker = gate.store_reason(store, pin["lock-sha256"])
            written(store / "offline", gate.STORE_MARKER, b"\xff\xfe")
            unreadable = gate.store_reason(store, pin["lock-sha256"])
        self.assertIn("records no pnpm-lock.yaml digest", no_marker)
        self.assertIn("records no pnpm-lock.yaml digest", unreadable)

    def test_the_digests_are_the_standard_ones(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            sha256, sri = gate.file_digests(written(base, "abc", b"abc"))
        self.assertEqual(sha256, hashlib.sha256(b"abc").hexdigest())
        self.assertEqual(
            sri,
            "sha512-3a81oZNherrMQXNJriBBMRLm+k6JqX6iCp7u5ktV05ohkpkqJ0/BqDa6PCOj/uu9RU1EI2Q86A4q"
            "mslPpUyknw==",
        )

    def test_an_archive_member_that_escapes_is_refused(self):
        for name in ("../escape", "/absolute", "a/../../b"):
            with self.subTest(name=name):
                buffer = io.BytesIO()
                with tarfile.open(fileobj=buffer, mode="w") as archive:
                    info = tarfile.TarInfo(name)
                    archive.addfile(info, io.BytesIO(b""))
                buffer.seek(0)
                with tarfile.open(fileobj=buffer, mode="r") as archive:
                    with self.assertRaises(gate.GateError):
                        gate.safe_members(archive)

    def test_bulk_goes_and_the_evidence_stays(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            run_dir = Path(base) / "r1"
            for part in ("node/bin", "pnpm", "work/pkg/node_modules/x", "work/pkg/dist", "logs"):
                (run_dir / part).mkdir(parents=True)
            written(run_dir / "logs", "suite.log", b"PASS")
            (run_dir / "work" / "out").mkdir(parents=True)
            written(run_dir / "work" / "out", "default-state.json", b"{}")
            gate.discard_bulk(run_dir)
            self.assertFalse((run_dir / "node").exists())
            self.assertFalse((run_dir / "work" / "pkg" / "node_modules").exists())
            self.assertTrue((run_dir / "logs" / "suite.log").is_file())
            self.assertTrue((run_dir / "work" / "out" / "default-state.json").is_file())

    def test_only_the_newest_runs_are_kept(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            runs = Path(base) / "runs"
            for index in range(gate.MAX_RETAINED_RUNS + 3):
                (runs / f"r{index:02d}").mkdir(parents=True)
            gate.prune_runs(Path(base))
            kept = sorted(path.name for path in runs.iterdir())
        self.assertEqual(len(kept), gate.MAX_RETAINED_RUNS)
        self.assertEqual(kept[0], "r03")

    def test_the_cache_follows_its_variable(self):
        with mock.patch.dict(gate.os.environ, {gate.CACHE_VARIABLE: "/tmp/a11y-cache"}):
            self.assertEqual(gate.cache_dir(), Path("/tmp/a11y-cache"))


def report_with(specs):
    """Return a Playwright JSON report whose specs sit one suite deep."""
    return {"suites": [{"title": "a11y.spec.js", "specs": [], "suites": [{"specs": specs}]}]}


def spec(title, passed=True, message="", **fields):
    """Return one spec as Playwright's JSON reporter writes it; `fields` override its test's.

    `ok` is computed the way Playwright computes it, true for an expected, a
    flaky and a skipped outcome, so the tests show the gate does not read it.
    """
    errors = [{"message": message}] if message else []
    test = {
        "expectedStatus": "passed",
        "status": "expected" if passed else "unexpected",
        "results": [{"status": "passed" if passed else "failed", "errors": errors}],
    }
    test.update(fields)
    ok = test["status"] in {"expected", "flaky", "skipped"}
    return {"title": title, "ok": ok, "tests": [test]}


def outcomes_of(*specs):
    """Return the gate's reading of a report holding `specs`."""
    return gate.spec_outcomes(report_with(list(specs)))


def eleven_passing():
    return outcomes_of(*(spec(f"test {index}") for index in range(gate.EXPECTED_TESTS)))


class ReportTests(unittest.TestCase):
    """How the gate reads what the suite wrote."""

    def test_nested_specs_are_all_read_and_escapes_dropped(self):
        outcomes = outcomes_of(spec("a"), spec("b", False, "\x1b[31mimage-alt\x1b[39m"))
        self.assertEqual([o["title"] for o in outcomes], ["a", "b"])
        self.assertEqual([o["passed"] for o in outcomes], [True, False])
        self.assertEqual(outcomes[1]["message"], ["image-alt"])

    def test_only_a_test_meant_to_pass_that_passed_counts(self):
        """Playwright's ok is true for each of these; the gate counts none of them."""
        failed = {"status": "failed", "errors": []}
        outcomes = {
            "expected failure": spec(
                "x", expectedStatus="failed", status="expected", results=[failed]
            ),
            "skipped": spec(
                "x", expectedStatus="skipped", status="skipped", results=[{"status": "skipped"}]
            ),
            "flaky": spec("x", status="flaky", results=[failed, {"status": "passed"}]),
            "no result": spec("x", results=[]),
        }
        for name, planted in outcomes.items():
            with self.subTest(outcome=name):
                self.assertTrue(planted["ok"], "Playwright would call this ok")
                self.assertFalse(outcomes_of(planted)[0]["passed"])
        self.assertFalse(outcomes_of({"title": "x", "ok": True, "tests": []})[0]["passed"])
        self.assertTrue(outcomes_of(spec("x"))[0]["passed"])

    def test_a_report_nested_past_the_bound_is_refused(self):
        nested = {"suites": []}
        cursor = nested
        for _ in range(gate.MAX_SPECS + 2):
            child = {"suites": []}
            cursor["suites"].append(child)
            cursor = child
        with self.assertRaises(gate.GateError):
            gate.spec_outcomes(nested)

    def test_a_planted_run_must_fail_the_selected_test_for_the_right_reason(self):
        failing = [
            {"title": "default state: x", "passed": False, "message": ["image-alt (critical)"]}
        ]
        self.assertEqual(gate.plant_problems(1, failing, "default state", "image-alt"), [])
        self.assertTrue(gate.plant_problems(0, failing, "default state", "image-alt"))
        passing = [dict(failing[0], passed=True)]
        self.assertTrue(gate.plant_problems(1, passing, "default state", "image-alt"))
        self.assertTrue(gate.plant_problems(1, failing, "default state", "button-name"))
        other = [dict(failing[0], title="focus indicator: y")]
        self.assertTrue(gate.plant_problems(1, other, "default state", "image-alt"))

    def default_state(self, **changes):
        state = {
            "runOnly": {"type": "tag", "values": list(gate.D81_TAGS)},
            "violations": [],
            "incomplete": [],
            "targetSize": {"bucket": "passes", "nodes": 3},
            "unmapped": [],
        }
        state.update(changes)
        return state

    def test_the_default_state_report_is_held_to_d81(self):
        self.assertEqual(gate.default_state_problems(self.default_state()), [])
        defects = {
            "tags": {"runOnly": {"type": "tag", "values": ["wcag2a", "best-practice"]}},
            "violation": {"violations": [{"id": "region", "impact": "moderate"}]},
            "incomplete": {"incomplete": [{"id": "color-contrast", "impact": "serious"}]},
            "target-size inapplicable": {"targetSize": {"bucket": "inapplicable", "nodes": 0}},
            "target-size absent": {"targetSize": None},
            "unmapped": {"unmapped": ["x: 9.9.9"]},
        }
        for name, change in defects.items():
            with self.subTest(defect=name):
                self.assertTrue(gate.default_state_problems(self.default_state(**change)))
        self.assertTrue(gate.default_state_problems(None))

    def readback(self, **changes):
        found = {
            "node": "v26.10.0",
            "playwrightTest": "1.63.0",
            "playwrightCore": "1.63.0",
            "axeCore": "4.13.0",
            "axePlaywright": "4.13.0",
            "svelte": "5.57.1",
            "vite": "8.3.1",
            "eslint": "10.11.0",
            "eslintPluginSvelte": "3.23.0",
            "svelteEslintParser": "1.8.1",
            "headlessShell": {"revision": "1243", "browserVersion": "153.0.8010.12"},
            "browsersPath": "/ms-playwright",
            "revisionPresent": True,
            "launched": "153.0.8010.12",
        }
        found.update(changes)
        return found

    def test_the_readback_must_equal_every_pin(self):
        pin, manifest = gate.load_pin(), json.loads(text(gate.MANIFEST))
        self.assertEqual(gate.readback_problems(pin, manifest, self.readback()), [])
        for key, value in (
            ("node", "v24.20.0"),
            ("revisionPresent", False),
            ("headlessShell", {"revision": "1228", "browserVersion": "153.0.8010.12"}),
            ("launched", "152.0.0.0"),
            ("svelte", "5.57.0"),
            ("eslint", "10.10.0"),
            ("svelteEslintParser", None),
        ):
            with self.subTest(key=key):
                self.assertTrue(
                    gate.readback_problems(pin, manifest, self.readback(**{key: value}))
                )

    def test_the_last_json_line_is_the_report(self):
        self.assertEqual(gate.last_json('noise\n{"a": 1}\ntrailing\n'), {"a": 1})
        self.assertIsNone(gate.last_json("no report\n[1, 2]\n"))

    def test_the_criterion_lines_name_both_editions(self):
        state = {
            "axe": "4.13.0",
            "criteria": {
                "2.5.8": {
                    "title": "Target Size (Minimum)",
                    "level": "AA",
                    "v4.1.1": "11.2.5.8",
                    "v3.2.1": "none in V3.2.1",
                    "evaluated": ["target-size"],
                    "inapplicable": [],
                }
            },
            "notCovered": {
                "2.5.7": {
                    "title": "Dragging Movements",
                    "v4.1.1": "11.2.5.7",
                    "v3.2.1": "none in V3.2.1",
                }
            },
        }
        lines = gate.criterion_notes(state)
        self.assertIn("V4.1.1 11.2.5.8, V3.2.1 none in V3.2.1", lines[0])
        self.assertIn("not covered by this gate: V4.1.1 11.2.5.7", lines[1])


class GuardTests(unittest.TestCase):
    """HISS-21: the gate runs, or says why it did not; the fetch fails instead."""

    def test_no_engine_is_a_stated_skip_and_exit_zero(self):
        with mock.patch.object(gate, "install_signal_handlers", return_value=[]):
            with mock.patch.object(gate, "pick_engine", return_value=(None, "no engine here")):
                code, printed = quiet(gate.main, [])
        self.assertEqual(code, 0)
        self.assertIn("SKIP: no engine here; the accessibility gate did not run.", printed)

    def test_the_fetch_without_an_engine_fails(self):
        with mock.patch.object(gate, "install_signal_handlers", return_value=[]):
            with mock.patch.object(gate, "pick_engine", return_value=(None, "no engine here")):
                code, printed = quiet(gate.main, ["--fetch"])
        self.assertEqual(code, 1)
        self.assertIn("FAIL: no engine here", printed)

    def test_an_absent_image_or_cache_skips_before_anything_runs(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            context = context_in(base)
            with mock.patch.object(gate, "image_present", return_value=(False, [])):
                with mock.patch.object(gate, "run_cases") as cases:
                    code, printed = quiet(gate.gate_main, context)
        cases.assert_not_called()
        self.assertEqual(code, 0)
        self.assertEqual(printed.count("SKIP:"), 4)
        self.assertIn("run `make a11y-fetch`", printed)

    def test_a_store_filled_for_another_lockfile_skips_before_anything_runs(self):
        """The case the review reproduced: a store from before a lockfile bump."""
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            context = context_in(base)
            downloads, offline = context.store / "downloads", context.store / "offline"
            downloads.mkdir(parents=True)
            (offline / "store").mkdir(parents=True)
            written(offline, gate.STORE_MARKER, b"0" * 64)
            for key, name, payload in (("node", "sha256", b"node"), ("pnpm", "integrity", b"pnpm")):
                path = written(downloads, context.pin[key]["file"], payload)
                context.pin[key][name] = gate.file_digests(path)[0 if key == "node" else 1]
            with mock.patch.object(gate, "image_present", return_value=(True, [])):
                with mock.patch.object(gate, "run_cases") as cases:
                    code, printed = quiet(gate.gate_main, context)
        cases.assert_not_called()
        self.assertEqual(code, 0)
        self.assertEqual(printed.count("SKIP:"), 1)
        self.assertIn("filled for another pnpm-lock.yaml", printed)

    def test_a_tampered_cache_is_a_failure_not_a_skip(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            context = context_in(base)
            downloads = context.store / "downloads"
            downloads.mkdir(parents=True)
            written(downloads, context.pin["node"]["file"], b"not node")
            with mock.patch.object(gate, "image_present", return_value=(True, [])):
                with self.assertRaises(gate.GateError):
                    gate.preflight(context)

    def test_every_terminating_signal_this_host_has_is_routed(self):
        handled = []
        with mock.patch.object(gate.signal, "signal", side_effect=lambda *a: handled.append(a)):
            names = gate.install_signal_handlers()
        self.assertEqual(len(handled), len(names))
        self.assertTrue(set(names) <= set(gate.TERMINATING_SIGNALS))
        with self.assertRaises(SystemExit):
            gate.terminated(15, None)


def stop(width=3, kind="outline", reasons=()):
    """Return one measured focus stop as tests/probe.js reports it."""
    return {
        "element": 'button "Show token values"',
        "kind": kind,
        "width": width,
        "ratio": 6.438461,
        "background": [255, 255, 255, 1],
        "pass": not reasons,
        "reasons": list(reasons),
    }


def passing_found():
    """Return the report files a passing suite writes, in the shape the suite writes them."""
    below = ["indicator 1px is below the D16 floor of 2px"]
    return {
        "default-state": {
            "axe": "4.13.0",
            "browser": "153.0.8010.12",
            "runOnly": {"type": "tag", "values": list(gate.D81_TAGS)},
            "counts": {"violations": 0, "incomplete": 0, "passes": 22, "inapplicable": 41},
            "violations": [],
            "incomplete": [],
            "targetSize": {"bucket": "passes", "tags": ["wcag22aa", "wcag258"], "nodes": 3},
            "criteria": {},
            "unmapped": [],
            "notCovered": {},
        },
        "focus-stops": {"stops": [stop(), stop(), stop()]},
        "falsifier-stripped-outline": {
            "before": stop(),
            "after": stop(0, "none", ["indicator 0px is below the D16 floor of 2px"]),
            "axeViolationsWithTheOutlineStripped": [],
        },
        "boundary-width": {
            "rows": [
                {"token": "3px", "measured": 3, "pass": True},
                {"token": "2px", "measured": 2, "pass": True},
                {"token": "1px", "measured": 1, "pass": False},
            ]
        },
        "boundary-contrast": {
            "rows": [
                {"ring": "#fa34ff", "ratio": 3.000002016598204, "pass": True},
                {"ring": "#b278ff", "ratio": 2.9899265930087537, "pass": False},
            ],
            "thresholds": [
                {"ratio": 3, "pass": True},
                {"ratio": 2.9999, "pass": False},
                {"ratio": 2.99, "pass": False},
            ],
        },
        "d76-stub-theme": {"stop": stop(1, reasons=below)},
        "text-scaling-200": {
            "base": [],
            "fontSize": "32px",
            "scaled": [],
            "planted": ['div "a fixed-width box" clips its content'],
        },
        "emulation-reduced-motion": {"normal": "0.15s", "reduced": "0s"},
        "emulation-forced-colors": {"stops": [stop(), stop(), stop()], "violations": []},
    }


class MeasuredReportTests(unittest.TestCase):
    """The gate reads the measured values itself; a test that stopped asserting one fails."""

    def test_the_passing_reports_raise_nothing(self):
        self.assertEqual(gate.measured_problems(passing_found()), [])

    def test_each_wrong_measurement_is_caught(self):
        clipped = ['p "This panel draws only from the P12 base " clips its content']
        defects = {
            "a stop at 2 px": ("focus-stops", lambda b: b["stops"][1].update(width=2)),
            "a stop gone": ("focus-stops", lambda b: b["stops"].pop()),
            "outline kept": ("falsifier-stripped-outline", lambda b: b.update(after=stop())),
            "stripped passes": (
                "falsifier-stripped-outline",
                lambda b: b["after"].update({"pass": True}),
            ),
            "1 px passes": ("boundary-width", lambda b: b["rows"][2].update({"pass": True})),
            "2.99 passes": ("boundary-contrast", lambda b: b["rows"][1].update({"pass": True})),
            "ring at 3.01": ("boundary-contrast", lambda b: b["rows"][0].update(ratio=3.01)),
            "2.9999 passes": (
                "boundary-contrast",
                lambda b: b["thresholds"][1].update({"pass": True}),
            ),
            "stub passes": ("d76-stub-theme", lambda b: b["stop"].update({"pass": True})),
            "200% clips": ("text-scaling-200", lambda b: b.update(scaled=clipped)),
            "detector blind": ("text-scaling-200", lambda b: b.update(planted=[])),
            "motion kept": ("emulation-reduced-motion", lambda b: b.update(reduced="0.15s")),
            "forced colours": ("emulation-forced-colors", lambda b: b.update(violations=["x"])),
            "malformed": ("boundary-width", lambda b: b.update(rows=[{}])),
        }
        for name, (report, change) in defects.items():
            with self.subTest(defect=name):
                found = passing_found()
                change(found[report])
                self.assertTrue(gate.measured_problems(found), name)

    def test_a_missing_report_is_caught(self):
        for name in {entry[0] for entry in gate.MEASURED_REPORTS}:
            with self.subTest(report=name):
                found = passing_found()
                del found[name]
                problems = gate.measured_problems(found)
                self.assertIn(f"the suite wrote no {name} report", problems)


class SuiteCaseTests(unittest.TestCase):
    """How the suite case and the refusal cases turn a run into a verdict."""

    def run_suite(self, code, outcomes, found):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            context = context_in(base)
            with mock.patch.object(gate, "step", return_value=(code, "playwright output")):
                with mock.patch.object(gate, "suite_results", return_value=outcomes):
                    with mock.patch.object(gate, "reports", return_value=found):
                        problems, printed = quiet(gate.suite_case, context)
        return problems, printed

    def test_eleven_passes_and_exit_zero_pass(self):
        problems, printed = self.run_suite(0, eleven_passing(), passing_found())
        self.assertEqual(problems, [])
        self.assertIn("PASS a11y/suite", printed)
        self.assertIn("11/11 tests passed; exit 0", printed)

    def test_a_non_zero_exit_fails_even_with_every_spec_passed(self):
        problems, printed = self.run_suite(1, eleven_passing(), passing_found())
        self.assertIn("playwright test exited 1", problems)
        self.assertIn("FAIL a11y/suite", printed)

    def test_an_expected_failure_is_not_a_pass(self):
        """The review's case: test.fail() on the 200% test while the text clips."""
        failed = {"status": "failed", "errors": []}
        specs = [spec(f"test {index}") for index in range(gate.EXPECTED_TESTS - 1)]
        specs.append(
            spec("200% text", expectedStatus="failed", status="expected", results=[failed])
        )
        found = passing_found()
        found["text-scaling-200"]["scaled"] = ['p "This panel" clips its content']
        problems, _printed = self.run_suite(0, outcomes_of(*specs), found)
        self.assertIn("10 of 11 tests passed; 11 are expected", problems)
        self.assertIn("not a pass: 200% text (outcome ['expected'])", problems)
        self.assertTrue(any(p.startswith("text-scaling-200 records") for p in problems))

    def test_a_missing_or_extra_test_fails(self):
        for outcomes in (eleven_passing()[:-1], eleven_passing() + outcomes_of(spec("extra"))):
            with self.subTest(count=len(outcomes)):
                problems, _printed = self.run_suite(0, outcomes, passing_found())
                self.assertTrue(problems)

    def test_a_refusal_needs_a_non_zero_exit_and_its_reason(self):
        needle = "ERR_PNPM_OUTDATED_LOCKFILE"
        cases = {
            "refused for the reason": (1, f"Error: {needle}", []),
            "exit 0 with the reason": (0, f"Error: {needle}", ["expected a refusal"]),
            "refused for another reason": (1, "Error: ERR_PNPM_OTHER", [f"expected {needle!r}"]),
        }
        for name, (code, output, expected) in cases.items():
            with self.subTest(case=name):
                problems, _printed = quiet(gate.refusal_case, "a11y/x", code, output, needle, "n")
                self.assertEqual(len(problems), len(expected))
                for fragment, problem in zip(expected, problems):
                    self.assertIn(fragment, problem)


def curl_writing(payload, code=0):
    """Return a gate.run stand-in that writes `payload` where curl was told to write."""

    def fake(argv, _timeout):
        out = Path(argv[argv.index("-o") + 1])
        if code == 0:
            written(out.parent, out.name, payload)
        return code, "", "curl: (22) The requested URL returned error: 404" if code else ""

    return fake


class FetchTests(unittest.TestCase):
    """`make a11y-fetch`: nothing is kept or reported fetched unless it verifies."""

    url = "https://example.invalid/node.tar.xz"

    def test_a_download_that_does_not_hash_to_its_pin_is_discarded(self):
        wanted = hashlib.sha256(b"right").hexdigest()
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            target = Path(base) / "downloads" / "node.tar.xz"
            with mock.patch.object(gate, "run", side_effect=curl_writing(b"wrong")):
                problems = gate.download(self.url, target, 0, wanted)
            leftovers = sorted(path.name for path in target.parent.iterdir())
        self.assertEqual(len(problems), 1)
        self.assertIn("the download was discarded", problems[0])
        self.assertEqual(leftovers, [])

    def test_a_download_that_hashes_to_its_pin_is_kept(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            target = Path(base) / "downloads" / "pnpm.tgz"
            wanted = gate.file_digests(written(base, "reference", b"right"))[1]
            with mock.patch.object(gate, "run", side_effect=curl_writing(b"right")) as run:
                self.assertEqual(gate.download(self.url, target, 1, wanted), [])
            argv = run.call_args.args[0]
            self.assertEqual(target.read_bytes(), b"right")
            self.assertFalse(target.with_name("pnpm.tgz.partial").exists())
        self.assertEqual(argv[argv.index("--proto") + 1], "=https")
        self.assertIn("--max-time", argv)

    def test_a_cached_match_is_not_fetched_again_and_a_stale_one_is_replaced(self):
        wanted = hashlib.sha256(b"right").hexdigest()
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            target = written(base, "node.tar.xz", b"right")
            with mock.patch.object(gate, "run", side_effect=AssertionError("fetched")) as run:
                self.assertEqual(gate.download(self.url, target, 0, wanted), [])
            run.assert_not_called()
            written(base, "node.tar.xz", b"stale")
            with mock.patch.object(gate, "run", side_effect=curl_writing(b"right")):
                self.assertEqual(gate.download(self.url, target, 0, wanted), [])
            self.assertEqual(target.read_bytes(), b"right")

    def test_a_failed_download_leaves_nothing(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            target = Path(base) / "downloads" / "node.tar.xz"
            with mock.patch.object(gate, "run", side_effect=curl_writing(b"", code=22)):
                problems = gate.download(self.url, target, 0, "0" * 64)
            self.assertFalse(target.exists())
        self.assertIn("curl exited 22", problems[0])

    def pull(self, code, present):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            context = context_in(base)
            with mock.patch.object(gate, "run", return_value=(code, "", "pull output")):
                with mock.patch.object(gate, "image_present", return_value=(present, [])):
                    problems, _printed = quiet(gate.fetch_image, context)
        return problems

    def test_a_pull_counts_only_when_the_engine_stores_the_digest(self):
        self.assertEqual(self.pull(0, True), [])
        self.assertTrue(self.pull(0, False), "exit 0 without the digest stored")
        self.assertTrue(self.pull(125, True), "a failed pull over an old copy")

    def test_the_store_is_marked_only_after_a_successful_fetch(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            context = context_in(base)
            marker = context.store / "offline" / gate.STORE_MARKER
            with mock.patch.object(gate, "prepare"):
                with mock.patch.object(gate, "step", return_value=(0, "done")):
                    self.assertEqual(quiet(gate.fetch_store, context)[0], [])
                recorded = marker.read_bytes()
                with mock.patch.object(gate, "step", return_value=(1, "ERR_PNPM_FETCH_404")):
                    self.assertTrue(quiet(gate.fetch_store, context)[0])
            self.assertFalse(marker.exists(), "a failed fetch leaves no marker")
        self.assertEqual(recorded, context.pin["lock-sha256"].encode("ascii") + b"\n")


def admitted_table(heading=ADMISSION_HEADING):
    """Return {tool: cells} from one table on the admission page, the M04 one by default."""
    page = text(ADMISSION)
    section = page.split(heading, 1)[1].split("\n## ", 1)[0]
    rows = {}
    for line in section.splitlines():
        cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
        if len(cells) == 5 and cells[0] not in {"Tool", ":---"} and not set(cells[0]) <= {":-"}:
            rows[cells[0].strip("`")] = cells
    return rows


def admitted_rows():
    """Return {tool: version} from the M04 table on the admission page."""
    return {tool: cells[1] for tool, cells in admitted_table().items()}


def admitted_roles():
    """Return {tool: role} from the M04 table on the admission page."""
    return {tool: cells[4] for tool, cells in admitted_table().items()}


class AdmissionTests(unittest.TestCase):
    """The admission page, the pin and package.json state one admission."""

    def test_every_admitted_version_is_on_the_page(self):
        rows = admitted_rows()
        pin = gate.load_pin()
        expected = dict(ADMITTED_PACKAGES)
        expected.update({"Node.js": pin["node"]["version"], "pnpm": pin["pnpm"]["version"]})
        for name, version in expected.items():
            with self.subTest(tool=name):
                self.assertIn(name, rows)
                self.assertIn(version, rows[name])
        self.assertIn(pin["image"]["digest"], rows["Playwright image"])
        self.assertIn(pin["image"]["browser"]["revision"], rows["chromium-headless-shell"])
        for engine in gate.ENGINES:
            self.assertIn(engine, rows)

    def test_every_program_the_gate_may_start_is_admitted(self):
        rows = admitted_rows()
        for program in sorted(ALLOWED_PROGRAMS):
            with self.subTest(program=program):
                self.assertIn(program, rows)
        self.assertIn("--fetch", rows["curl"] + admitted_roles()["curl"])

    def test_the_page_records_the_digests_and_the_evidence_page_the_run(self):
        page, evidence = text(ADMISSION), text(EVIDENCE_PAGE)
        pin = gate.load_pin()
        for value in (pin["node"]["sha256"], pin["pnpm"]["integrity"], pin["image"]["digest"]):
            self.assertIn(value, page)
        self.assertIn(pin["image"]["digest"], evidence)
        self.assertIn(gate.SOURCE_REFERENCE, evidence)
        self.assertIn("not portal evidence", evidence)
        self.assertIn("not AT-SPI2", evidence)
        self.assertIn("build/accessibility-harness.md", text(MKDOCS))

    def test_the_lint_rows_are_the_manifests_versions(self):
        """D100: every package the lint runs has a row on the admission page, at its pin."""
        rows = {tool: cells[1] for tool, cells in admitted_table(LINT_ADMISSION_HEADING).items()}
        self.assertEqual(set(rows), set(ADMITTED_LINT))
        for name, version in ADMITTED_LINT.items():
            with self.subTest(tool=name):
                self.assertIn(version, rows[name])
        section = text(ADMISSION).split(LINT_ADMISSION_HEADING, 1)[1].split("\n## ", 1)[0]
        self.assertIn("cordanaLLM/praetor#589", section)
        self.assertIn("mutual recursion", section)
        self.assertIn("ui/concordia-tokens/tests/probe.js", section)

    def test_renovate_groups_the_lint(self):
        config = json.loads(text(RENOVATE))
        groups = [
            rule
            for rule in config["packageRules"]
            if set(ADMITTED_LINT) <= set(rule.get("matchPackageNames", []))
        ]
        self.assertEqual(len(groups), 1)
        self.assertIn("groupName", groups[0])
        self.assertEqual(groups[0].get("matchFileNames"), ["ui/concordia-tokens/**"])

    def test_renovate_moves_the_image_with_playwright(self):
        config = json.loads(text(RENOVATE))
        managers = config.get("customManagers", [])
        patterns = [p for manager in managers for p in manager.get("managerFilePatterns", [])]
        self.assertTrue(any("toolchain\\.pin\\.json" in pattern for pattern in patterns))
        datasources = {manager.get("datasourceTemplate") for manager in managers}
        self.assertLessEqual({"docker", "node-version"}, datasources)
        groups = [
            rule
            for rule in config["packageRules"]
            if {"@playwright/test", "mcr.microsoft.com/playwright"}
            <= set(rule.get("matchPackageNames", []))
        ]
        self.assertEqual(len(groups), 1)
        self.assertIn("groupName", groups[0])


def js_function_spans(source):
    """Return {name: lines} for each top-level `export function` in a planted file."""
    lines, spans = source.splitlines(), {}
    for index, line in enumerate(lines):
        match = re.match(r"export function (\w+)\(", line)
        if match:
            end = next(i for i in range(index, len(lines)) if lines[i] == "}")
            spans[match.group(1)] = end - index + 1
    return spans


def lint_report(*files):
    """Return an ESLint JSON report: each file a (path, [rule ids], suppressed count) triple."""
    return [
        {
            "filePath": f"{gate.IN_PKG}/{path}",
            "messages": [{"ruleId": rule, "severity": 2} for rule in rules],
            "suppressedMessages": [{}] * suppressed,
        }
        for path, rules, suppressed in files
    ]


class LintTests(unittest.TestCase):
    """D100: the ESLint configuration, its planted violations, and how the gate reads them."""

    config = text(LINT_CONFIG)

    def test_the_configuration_states_the_hiss_limits(self):
        for fragment in (
            "maxLines: 60,",
            "maxComplexity: 10,",
            "maxStatements: 50,",
            "noInlineConfig: true,",
            "'no-eval': 'error',",
            "'no-implied-eval': 'error',",
            "'no-new-func': 'error',",
            "'aegis-hiss/no-self-recursion': 'error',",
            "skipBlankLines: false, skipComments: false, IIFEs: true",
            "cordanaLLM/praetor#589",
        ):
            with self.subTest(fragment=fragment):
                self.assertIn(fragment, self.config)
        self.assertNotIn("'warn'", self.config)
        self.assertNotIn("'off'", self.config)

    def test_the_local_rule_says_what_it_does_not_detect(self):
        rule = text(LINT_RULE)
        self.assertIn("mutual recursion", rule)
        self.assertIn("NOT detected", self.config)
        self.assertIn("cordanaLLM/praetor#589", rule)

    def test_the_gate_lints_with_zero_warnings_and_json_output(self):
        self.assertEqual(
            gate.lint_argv("/work/lint/x.json", "."),
            [
                "pnpm",
                "exec",
                "eslint",
                "--max-warnings",
                "0",
                "--format",
                "json",
                "--output-file",
                "/work/lint/x.json",
                ".",
            ],
        )
        self.assertIn("--no-ignore", gate.lint_argv("/o", "hiss-lint/plants/x.js", True))
        self.assertIn("eslint.config.js", gate.PACKAGE_ENTRIES)
        self.assertIn("hiss-lint", gate.PACKAGE_ENTRIES)

    def test_every_plant_exists_names_d100_and_lists_sorted_findings(self):
        files = {file for _family, file, _expected in gate.LINT_PLANTS} | {gate.LINT_BOUNDARY}
        self.assertEqual(files, {path.name for path in PLANT_DIR.iterdir()})
        for family, file, expected in gate.LINT_PLANTS:
            with self.subTest(family=family):
                self.assertIn("D100", text(PLANT_DIR / file))
                self.assertTrue(expected)
                self.assertEqual(expected, sorted(expected))
        self.assertEqual(len({family for family, _f, _e in gate.LINT_PLANTS}), 7)

    def test_the_planted_sizes_sit_one_past_and_exactly_on_the_limits(self):
        """Boundary: 61 lines fails and 60 passes; 11 and 10 of complexity; 51 and 50 statements."""
        self.assertEqual(js_function_spans(text(PLANT_DIR / "function-length.js")), {"tooLong": 61})
        limits = js_function_spans(text(PLANT_DIR / gate.LINT_BOUNDARY))
        self.assertEqual(limits["sixtyLines"], 60)
        self.assertEqual(text(PLANT_DIR / "complexity.js").count("  if ("), 10)
        boundary = text(PLANT_DIR / gate.LINT_BOUNDARY)
        self.assertEqual(boundary.count("  if ("), 9)
        self.assertEqual(text(PLANT_DIR / "statements.js").count("  result += 1;"), 49)
        self.assertEqual(boundary.count("  result += 1;"), 48)

    def test_a_clean_package_lint_passes(self):
        files, findings, suppressed = gate.lint_findings(
            lint_report(("a/b.js", [], 0), ("a/c.svelte", [], 0))
        )
        self.assertEqual(files, ["a/b.js", "a/c.svelte"])
        self.assertEqual(gate.lint_problems(0, files, findings, suppressed, ["a/b.js"]), [])

    def test_a_finding_a_suppression_a_skipped_file_or_no_svelte_fails(self):
        cases = {
            "finding": (1, lint_report(("a.svelte", ["complexity"], 0)), []),
            "suppressed": (0, lint_report(("a.svelte", [], 1)), []),
            "exit": (2, lint_report(("a.svelte", [], 0)), []),
            "skipped": (0, lint_report(("a.svelte", [], 0)), ["b/unlinted.js"]),
            "no svelte": (0, lint_report(("a.js", [], 0)), []),
        }
        for name, (code, results, expected_files) in cases.items():
            with self.subTest(case=name):
                files, findings, suppressed = gate.lint_findings(results)
                self.assertTrue(
                    gate.lint_problems(code, files, findings, suppressed, expected_files)
                )

    def test_an_ignored_directive_reads_as_a_finding(self):
        _files, findings, _suppressed = gate.lint_findings(
            lint_report(("a.js", [None, "no-eval"], 0))
        )
        self.assertEqual(findings, [gate.DIRECTIVE, "no-eval"])
        self.assertEqual(gate.lint_findings(None), ([], [], 0))

    def test_lintable_finds_every_source_but_the_ignored_ones(self):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            root = Path(base)
            for name in (
                "eslint.config.js",
                "src/App.svelte",
                "tests/readback.mjs",
                "x.cjs",
                "README.md",
                "node_modules/dep/index.js",
                "dist/app.js",
                "test-output/r.js",
                "hiss-lint/no-self-recursion.js",
                "hiss-lint/plants/eval.js",
            ):
                (root / name).parent.mkdir(parents=True, exist_ok=True)
                (root / name).write_bytes(b"x")
            found = gate.lintable(root)
        self.assertEqual(
            found,
            [
                "eslint.config.js",
                "hiss-lint/no-self-recursion.js",
                "src/App.svelte",
                "tests/readback.mjs",
                "x.cjs",
            ],
        )

    def plant_case(self, code, results, file="complexity.js", expected=("complexity",)):
        with tempfile.TemporaryDirectory(prefix="aegis-a11y-test-") as base:
            context = context_in(base)
            with mock.patch.object(gate, "lint_run", return_value=(code, results, "eslint output")):
                problems, printed = quiet(
                    gate.lint_plant_case, context, "complexity", file, list(expected)
                )
        return problems, printed

    def test_a_plant_passes_only_with_exit_one_and_exactly_its_findings(self):
        path = f"{gate.LINT_PLANT_DIR}/complexity.js"
        exact = lint_report((path, ["complexity"], 0))
        self.assertEqual(self.plant_case(1, exact)[0], [])
        self.assertIn("PASS a11y/hiss-lint-complexity-refused", self.plant_case(1, exact)[1])
        for name, (code, results) in {
            "exit 0": (0, exact),
            "crash": (2, exact),
            "no report": (1, None),
            "extra finding": (1, lint_report((path, ["complexity", "no-eval"], 0))),
            "other file": (1, lint_report(("elsewhere.js", ["complexity"], 0))),
        }.items():
            with self.subTest(case=name):
                self.assertTrue(self.plant_case(code, results)[0])

    def test_the_boundary_file_passes_only_clean_with_exit_zero(self):
        path = f"{gate.LINT_PLANT_DIR}/{gate.LINT_BOUNDARY}"
        clean = lint_report((path, [], 0))
        problems, printed = self.plant_case(0, clean, gate.LINT_BOUNDARY, ())
        self.assertEqual(problems, [])
        self.assertIn("PASS a11y/hiss-lint-complexity\n", printed)
        self.assertTrue(
            self.plant_case(1, lint_report((path, ["complexity"], 0)), gate.LINT_BOUNDARY, ())[0]
        )

    def test_the_lint_runs_after_the_install_and_before_the_build(self):
        line = {}
        for node in ast.walk(function_index(ast.parse(text(GATE)))["run_cases"]):
            if isinstance(node, ast.Call) and isinstance(node.func, ast.Name):
                line.setdefault(node.func.id, node.lineno)
        self.assertLess(line["install_case"], line["lint_cases"])
        self.assertLess(line["lint_cases"], line["build_case"])


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
    """Name what an argv list literal starts: a string, or the engine variable."""
    if not isinstance(node, ast.List) or not node.elts:
        return None
    first = node.elts[0]
    if isinstance(first, ast.Constant) and isinstance(first.value, str):
        return first.value
    if isinstance(first, ast.Name) and first.id == "engine":
        return "<engine>"
    if isinstance(first, ast.Attribute) and first.attr == "engine":
        return "<engine>"
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

    def test_the_host_programs_are_the_allowed_ones(self):
        programs = set()
        for call in self.calls("run"):
            head = call.args[0]
            if isinstance(head, ast.Call) and getattr(head.func, "id", "") == "container_argv":
                programs.add("<engine>")
            else:
                programs.add(program_of(head))
        self.assertEqual(programs, {"curl", "<engine>"})
        self.assertLessEqual(set(gate.ENGINES) | {"curl"}, ALLOWED_PROGRAMS)
        base = [n for n in ast.walk(self.functions["container_argv"]) if isinstance(n, ast.List)]
        self.assertEqual(program_of(base[0]), "<engine>")

    def test_the_container_programs_are_the_allowed_ones(self):
        programs = set()
        for call in self.calls("step"):
            argv = call.args[2]
            if isinstance(argv, ast.Call):
                argv = [
                    n for n in ast.walk(self.functions[argv.func.id]) if isinstance(n, ast.List)
                ][0]
            programs.add(program_of(argv))
        self.assertEqual(programs, CONTAINER_PROGRAMS)

    def test_every_run_and_step_carries_a_deadline(self):
        for call in self.calls("run") + self.calls("step"):
            needed = 2 if getattr(call.func, "id", "") == "run" else 4
            self.assertGreaterEqual(len(call.args), needed, f"no deadline at line {call.lineno}")

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

    def test_the_gate_never_pulls_outside_the_fetch(self):
        pulls = [
            function.name
            for function in self.functions.values()
            for node in ast.walk(function)
            if isinstance(node, ast.Constant) and node.value == "pull"
        ]
        self.assertEqual(pulls, ["fetch_image"])
        self.assertIn('"--pull=never"', text(GATE))

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
