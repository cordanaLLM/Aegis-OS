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
