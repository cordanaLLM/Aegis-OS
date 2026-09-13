"""Preparation guard regressions; no build, boot or notebook code is executed."""

import hashlib
import json
from pathlib import Path
import tempfile
import unittest
from unittest.mock import patch

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
        self.data = {"stage": "planning", "components": [
            {"id": f"P{i:02}", "status": "proposal", "activation_blockers": ["real test"]}
            for i in range(1, 17)
        ]}

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

    def test_source_integrity_and_inventory_drift(self):
        source = self.root / ".workingdir/notebookllmprep"
        source.mkdir(parents=True)
        (source / "plan.md").write_bytes(b"proposal\r\n")
        row = {"path": "plan.md", "bytes": 10,
               "sha256": hashlib.sha256(b"proposal\r\n").hexdigest()}
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
