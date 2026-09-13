"""Preparation guard regressions; no build, boot or notebook code is executed."""

import contextlib
import hashlib
import io
import json
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import roadmap_state as state
import verify_preparation as check


class PreparationTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.patch = patch.object(check, "ROOT", self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        (self.root / "planning").mkdir()
        self.data = {
            "stage": "planning",
            "components": [
                {
                    "id": f"P{i:02}",
                    "name": f"sub-{i}",
                    "status": "proposal",
                    "activation_blockers": ["real test"],
                }
                for i in range(1, 17)
            ],
        }

    def git_init(self):
        subprocess.run(["git", "init", "-q", str(self.root)], check=True, timeout=10)
        (self.root / ".gitignore").write_text("/.workingdir/\n")

    def write_components(self):
        (self.root / "planning/components.json").write_text(json.dumps(self.data))

    def test_exact_inventory_passes(self):
        self.write_components()
        self.assertEqual(len(check.verify_components()), 16)

    def test_missing_and_duplicate_subsystems_fail(self):
        for rows in [self.data["components"][:-1], [self.data["components"][0]] * 16]:
            self.data["components"] = rows
            self.write_components()
            with self.assertRaises(ValueError):
                check.verify_components()

    def test_runtime_claim_needs_new_contract(self):
        self.data["components"][0]["status"] = "boot-verified"
        self.write_components()
        with self.assertRaises(ValueError):
            check.verify_components()

    def write_licensing(self, texts, identifiers=("EUPL-1.2", "CC-BY-SA-4.0")):
        digests = {}
        for name, body in texts.items():
            path = self.root / name
            path.parent.mkdir(parents=True, exist_ok=True)
            path.write_bytes(body)
            digests[name] = hashlib.sha256(body).hexdigest()
        tables = "".join(
            f'[[annotations]]\npath = ["**"]\nSPDX-License-Identifier = "{i}"\n'
            for i in identifiers
        )
        (self.root / "REUSE.toml").write_text("version = 1\n" + tables)
        (self.root / "LICENSING.md").write_text("split licence")
        return digests

    def test_licence_texts_match_pinned_digests(self):
        digests = self.write_licensing({"LICENSE": b"eupl", "LICENSES/CC-BY-SA-4.0.txt": b"cc"})
        with patch.object(check, "LICENSE_TEXTS", digests):
            check.verify_licensing()
            (self.root / "LICENSE").write_bytes(b"eupl altered")
            with self.assertRaises(ValueError):
                check.verify_licensing()

    def test_licence_metadata_is_exact(self):
        digests = self.write_licensing({"LICENSE": b"eupl"}, identifiers=("EUPL-1.2",))
        with patch.object(check, "LICENSE_TEXTS", digests):
            with self.assertRaises(ValueError):
                check.verify_licensing()
        digests = self.write_licensing({"LICENSE": b""})
        with patch.object(check, "LICENSE_TEXTS", digests):
            check.verify_licensing()
            (self.root / "LICENSING.md").unlink()
            with self.assertRaises(ValueError):
                check.verify_licensing()

    def write_register(self):
        digest = "a" * 64
        data = {
            "schema_version": 1,
            "source_bundle_sha256": check.REGISTER_BUNDLE,
            "candidates": [
                {
                    "candidate_id": f"C{i:02}",
                    "component": f"P{i:02}",
                    "proposed_paths": ["crates/x"],
                    "manifest_present": False,
                    "lockfile_present": False,
                    "sources": [{"id": "export-001", "sha256": digest}],
                }
                for i in range(1, 17)
            ],
            "contradictions": [
                {
                    "id": "DSP-01",
                    "side_a": {"source_id": "export-062", "sha256": digest, "summary": "a"},
                    "side_b": {"source_id": "export-002", "sha256": digest, "summary": "b"},
                    "status": "open",
                }
            ],
            "toolchain_drift": [{"item": "tokio", "sha256": digest}],
            "quarantined_artifacts": [
                {
                    "artifact": "ci",
                    "sha256": digest,
                    "tracked_in_repository": False,
                    "defects": ["suppresses failures"],
                }
            ],
        }
        (self.root / "planning/candidates.json").write_text(json.dumps(data))

    def test_privacy_guard_rejects_tracked_working_data(self):
        self.git_init()
        check.verify_privacy()
        private = self.root / ".workingdir"
        private.mkdir()
        (private / "note.md").write_text("private")
        subprocess.run(
            ["git", "-C", str(self.root), "add", "-f", ".workingdir/note.md"],
            check=True,
            timeout=10,
        )
        with self.assertRaises(ValueError):
            check.verify_privacy()

    def test_main_readiness_lists_every_component(self):
        self.write_components()
        self.git_init()
        self.write_licensing({})
        self.write_roadmap([self.milestone("M00", 0, "ready")])
        self.write_register()
        out = io.StringIO()
        argv = ["verify_preparation.py", "--readiness"]
        with patch.object(sys, "argv", argv), patch.object(check, "LICENSE_TEXTS", {}):
            with contextlib.redirect_stdout(out):
                check.main()
        self.assertEqual(out.getvalue().count(": proposal; real test"), 16)
        self.assertIn("M00 [READY]", out.getvalue())

    def test_blocked_targets_fail_closed(self):
        root = Path(__file__).resolve().parent.parent
        for target in ("build", "boot", "release"):
            result = subprocess.run(
                ["make", "-s", target], cwd=root, capture_output=True, text=True, timeout=10
            )
            self.assertNotEqual(result.returncode, 0)
            self.assertIn("blocked", result.stderr)

    def milestone(self, mid, rank, state, blocked_by=(), **extra):
        row = {
            "id": mid,
            "title": f"milestone {mid}",
            "rank": rank,
            "state": state,
            "blocked_by": list(blocked_by),
            "cost": "small",
            "exit_criteria": ["evidence"],
            "epics": [{"id": f"E{mid}-1"}],
        }
        row.update(extra)
        return row

    def write_roadmap(self, rows):
        (self.root / "planning/roadmap.json").write_text(json.dumps({"milestones": rows}))

    def test_roadmap_blocking_states_pass(self):
        self.write_roadmap(
            [
                self.milestone("M00", 0, "done", evidence=["a2c9626"]),
                self.milestone("M01", 1, "ready", ["M00"]),
                self.milestone("M02", 2, "blocked", ["M01"]),
            ]
        )
        self.assertEqual(len(check.verify_roadmap()), 3)
        self.write_roadmap([self.milestone("M00", 0, "ready")])
        self.assertEqual(len(check.verify_roadmap()), 1)

    def test_roadmap_rejects_inconsistent_states(self):
        bad = [
            [self.milestone("M00", 0, "done")],
            [self.milestone("M00", 0, "ready"), self.milestone("M01", 1, "ready", ["M00"])],
            [
                self.milestone("M00", 0, "ready", ["M01"]),
                self.milestone("M01", 1, "ready", ["M00"]),
            ],
            [self.milestone("M00", 0, "ready", ["M09"])],
            [self.milestone("M00", 0, "ready"), self.milestone("M01", 2, "blocked", ["M00"])],
            [self.milestone("M00", 1, "ready"), self.milestone("M01", 0, "blocked", ["M00"])],
            [{"id": "M00", "rank": 0, "state": "ready"}],
        ]
        for rows in bad:
            self.write_roadmap(rows)
            with self.assertRaises(ValueError):
                check.verify_roadmap()

    def test_source_integrity_and_inventory_drift(self):
        source = self.root / ".workingdir/notebookllmprep"
        source.mkdir(parents=True)
        (source / "plan.md").write_bytes(b"proposal\r\n")
        row = {
            "path": "plan.md",
            "bytes": 10,
            "sha256": hashlib.sha256(b"proposal\r\n").hexdigest(),
        }
        inventory = self.root / ".workingdir/source-inventory.json"
        inventory.write_text(json.dumps({"count": 1, "files": [row]}))
        check.verify_sources()
        (source / "plan.md").write_bytes(b"altered\r\n")
        with self.assertRaises(ValueError):
            check.verify_sources()
        (source / "extra.md").write_text("unreviewed")
        with self.assertRaises(ValueError):
            check.verify_sources()


if __name__ == "__main__":
    unittest.main()


class RegisterTests(unittest.TestCase):
    """Positive, negative and boundary coverage for the M01 candidate register."""

    BUNDLE = check.REGISTER_BUNDLE
    DIGEST = "a" * 64

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.patch = patch.object(check, "ROOT", self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        (self.root / "planning").mkdir()
        self.components = [{"id": f"P{i:02}", "status": "proposal"} for i in range(1, 17)]

    def side(self, source_id="export-062"):
        return {"source_id": source_id, "sha256": self.DIGEST, "summary": "a side"}

    def register(self, **overrides):
        data = {
            "schema_version": 1,
            "source_bundle_sha256": self.BUNDLE,
            "candidates": [
                {
                    "candidate_id": f"C{i:02}",
                    "component": f"P{i:02}",
                    "proposed_paths": ["crates/x"],
                    "manifest_present": False,
                    "lockfile_present": False,
                    "sources": [{"id": "export-001", "sha256": self.DIGEST}],
                }
                for i in range(1, 17)
            ],
            "contradictions": [
                {
                    "id": "DSP-01",
                    "side_a": self.side(),
                    "side_b": self.side("export-002"),
                    "status": "open",
                }
            ],
            "toolchain_drift": [{"item": "tokio", "sha256": self.DIGEST}],
            "quarantined_artifacts": [
                {
                    "artifact": "ci",
                    "sha256": self.DIGEST,
                    "tracked_in_repository": False,
                    "defects": ["suppresses failures"],
                }
            ],
        }
        data.update(overrides)
        (self.root / "planning/candidates.json").write_text(json.dumps(data))
        return data

    def test_complete_register_passes(self):
        self.register()
        self.assertEqual(len(check.verify_candidates(self.components)["candidates"]), 16)

    def test_pinned_bundle_and_schema_are_required(self):
        for key, value in (("schema_version", 2), ("source_bundle_sha256", self.DIGEST)):
            self.register(**{key: value})
            with self.assertRaises(ValueError):
                check.verify_candidates(self.components)

    def test_every_component_must_be_covered(self):
        data = self.register()
        self.register(candidates=data["candidates"][:-1])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)

    def test_manifest_claim_fails_closed_while_component_is_a_proposal(self):
        data = self.register()
        data["candidates"][0]["manifest_present"] = True
        self.register(candidates=data["candidates"])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)
        data["candidates"][0]["manifest_present"] = False
        data["candidates"][0]["lockfile_present"] = True
        self.register(candidates=data["candidates"])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)

    def test_digest_form_is_enforced(self):
        for bad in ("A" * 64, "a" * 63, "z" * 64, 42):
            with self.assertRaises(ValueError):
                check.verify_digest(bad, "row")
        check.verify_digest(self.DIGEST, "row")

    def test_contradictions_need_both_sides_and_a_matching_resolution(self):
        data = self.register()
        row = data["contradictions"][0]
        row["status"] = "resolved"
        self.register(contradictions=[row])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)
        row["resolution"] = "D02"
        self.register(contradictions=[row])
        self.assertTrue(check.verify_candidates(self.components))
        row["status"] = "open"
        self.register(contradictions=[row])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)

    def test_summary_length_boundary(self):
        data = self.register()
        row = data["contradictions"][0]
        row["side_a"]["summary"] = "x" * check.QUOTE_LIMIT
        self.register(contradictions=[row])
        self.assertTrue(check.verify_candidates(self.components))
        row["side_a"]["summary"] = "x" * (check.QUOTE_LIMIT + 1)
        self.register(contradictions=[row])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)

    def test_public_citation_must_hash_to_the_tracked_file(self):
        (self.root / "docs").mkdir()
        target = self.root / "docs/stack.md"
        target.write_text("contract")
        digest = hashlib.sha256(target.read_bytes()).hexdigest()
        data = self.register()
        row = data["contradictions"][0]
        row["side_b"] = {
            "source_id": "public:docs/stack.md",
            "sha256": digest,
            "summary": "public side",
        }
        self.register(contradictions=[row])
        self.assertTrue(check.verify_candidates(self.components))
        target.write_text("contract changed")
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)
        target.unlink()
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)

    def test_quarantined_artifacts_must_be_untracked_with_defects(self):
        data = self.register()
        row = data["quarantined_artifacts"][0]
        row["tracked_in_repository"] = True
        self.register(quarantined_artifacts=[row])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)
        row["tracked_in_repository"] = False
        row["defects"] = []
        self.register(quarantined_artifacts=[row])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)

    def test_row_count_bounds_and_duplicate_identifiers(self):
        self.register(toolchain_drift=[])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)
        data = self.register()
        data["candidates"][1]["candidate_id"] = data["candidates"][0]["candidate_id"]
        self.register(candidates=data["candidates"])
        with self.assertRaises(ValueError):
            check.verify_candidates(self.components)


class RoadmapStateTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.path = Path(self.temp.name) / "roadmap.json"

    def write(self, rows):
        self.path.write_text(json.dumps({"milestones": rows}))

    def test_known_milestone_state_is_printed(self):
        self.write([{"id": "M13", "state": "blocked"}, {"id": "M00", "state": "done"}])
        self.assertEqual(state.milestone_state("M13", self.path), "blocked")
        self.assertEqual(state.milestone_state("M00", self.path), "done")

    def test_unknown_milestone_and_missing_file_fail(self):
        self.write([{"id": "M00", "state": "done"}])
        with self.assertRaises(ValueError):
            state.milestone_state("M99", self.path)
        with self.assertRaises(ValueError):
            state.milestone_state("M00", self.path.with_name("absent.json"))

    def test_empty_and_oversized_boundaries(self):
        self.write([])
        with self.assertRaises(ValueError):
            state.milestone_state("M00", self.path)
        self.path.write_text(json.dumps({"milestones": [{"id": "M00", "state": "done"}]}))
        self.assertEqual(state.milestone_state("M00", self.path), "done")
        with patch.object(state, "MAX_BYTES", 1):
            with self.assertRaises(ValueError):
                state.milestone_state("M00", self.path)
