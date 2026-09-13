"""Preparation guard regressions; no build, boot or notebook code is executed."""

import contextlib
import hashlib
import io
import json
import os
from pathlib import Path
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch

import roadmap_state as state
import verify_preparation as check


def git(*args, timeout=10):
    """Run git without the repository environment a hook exports.

    A pre-commit hook sets GIT_DIR and GIT_INDEX_FILE. Without scrubbing them a
    fixture's `git add` would run against the repository being committed instead
    of its own temporary clone, and would stage files nobody asked for.
    """
    env = {key: value for key, value in os.environ.items() if not key.startswith("GIT_")}
    return subprocess.run(list(args), check=True, timeout=timeout, env=env)


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
            "stage": "activation",
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
        git("git", "init", "-q", str(self.root))
        (self.root / ".gitignore").write_text("/.workingdir/\n")

    def write_components(self):
        (self.root / "planning/components.json").write_text(json.dumps(self.data))

    def test_exact_inventory_passes(self):
        self.write_components()
        self.assertEqual(len(check.verify_components()), 16)

    def test_the_tracked_inventory_declares_the_required_stage(self):
        """Positive: the literal this gate requires is the literal the tracked
        planning/components.json declares, so the repository's machine-readable
        self-description and the check that enforces it cannot drift apart."""
        repo = Path(check.__file__).resolve().parent.parent
        tracked = json.loads((repo / "planning/components.json").read_text())
        self.assertEqual(tracked["stage"], check.DECLARED_STAGE)

    def test_a_stale_or_overclaiming_stage_fails(self):
        """Negative: the superseded token, an invented one, and any token that
        would claim a product are all refused rather than quietly accepted."""
        for stage in ("planning", "construction", "shipping", "released", ""):
            self.data["stage"] = stage
            self.write_components()
            with self.assertRaises(ValueError):
                check.verify_components()

    def test_a_near_miss_stage_fails(self):
        """Boundary: only the exact literal passes - not a case variant, not a
        surrounding space, and not the adjacent component-status word."""
        for stage in ("Activation", "ACTIVATION", " activation", "activation ", "activated"):
            self.data["stage"] = stage
            self.write_components()
            with self.assertRaises(ValueError):
                check.verify_components()

    def test_missing_and_duplicate_subsystems_fail(self):
        for rows in [self.data["components"][:-1], [self.data["components"][0]] * 16]:
            self.data["components"] = rows
            self.write_components()
            with self.assertRaises(ValueError):
                check.verify_components()

    def test_runtime_claim_needs_new_contract(self):
        for status in ("boot-verified", "released", "activated-ish", ""):
            self.data["components"][0]["status"] = status
            self.write_components()
            with self.assertRaises(ValueError):
                check.verify_components()

    def track(self, *paths, crate="sub-1"):
        """Write and stage a manifest-shaped file at each path under the fixture root.

        A manifest declares a crate name because the evidence binding reads it;
        a lockfile carries no package table and is written empty.
        """
        for path in paths:
            target = self.root / path
            target.parent.mkdir(parents=True, exist_ok=True)
            body = f'[package]\nname = "{crate}"\n' if path.endswith("Cargo.toml") else ""
            target.write_text(body)
        git("git", "-C", str(self.root), "add", *paths)

    def activate(self, **evidence):
        """Advance P01 to the activated status with tracked evidence on disk.

        Every field the guards read is written afresh, blockers included, so a
        test that breaks one field cannot leave a second field broken for the
        next call and satisfy a later guard by accident.
        """
        self.git_init()
        self.track("crates/sub-1/Cargo.toml", "Cargo.lock")
        row = self.data["components"][0]
        row["status"] = check.ACTIVATED_STATUS
        row["milestone"] = "M02"
        row["activation_blockers"] = ["real hardware evidence"]
        row["activation_evidence"] = {
            "component_path": "crates/sub-1",
            "manifest_path": "crates/sub-1/Cargo.toml",
            "lockfile_path": "Cargo.lock",
            "test_command": "cargo test -p sub-1",
        }
        row["activation_evidence"].update(evidence)
        self.write_components()
        return row

    def test_activation_with_tracked_evidence_passes(self):
        """Positive: a component may advance when it carries tracked evidence."""
        self.activate()
        rows = check.verify_components()
        self.assertEqual(rows[0]["status"], check.ACTIVATED_STATUS)

    def test_activation_without_evidence_fails_closed(self):
        """Negative: an unqualified advance is refused."""
        self.git_init()
        row = self.data["components"][0]
        row["status"] = check.ACTIVATED_STATUS
        self.write_components()
        with self.assertRaises(ValueError):
            check.verify_components()
        row["activation_evidence"] = "crates/demo/Cargo.toml"
        self.write_components()
        with self.assertRaises(ValueError):
            check.verify_components()

    def test_activation_evidence_must_be_tracked_and_complete(self):
        """Boundary: each required field, one at a time, and an untracked path."""
        for field in check.ACTIVATION_EVIDENCE:
            self.activate()
            del self.data["components"][0]["activation_evidence"][field]
            self.write_components()
            with self.assertRaises(ValueError):
                check.verify_components()
        for blank in ("", "   "):
            self.activate(test_command=blank)
            with self.assertRaises(ValueError):
                check.verify_components()
        self.activate()
        (self.root / "untracked.lock").write_text("version = 4\n")
        self.data["components"][0]["activation_evidence"]["lockfile_path"] = "untracked.lock"
        self.write_components()
        with self.assertRaises(ValueError):
            check.verify_components()
        self.activate()
        self.data["components"][0]["activation_evidence"]["manifest_path"] = "absent/Cargo.toml"
        self.write_components()
        with self.assertRaises(ValueError):
            check.verify_components()

    def test_evidence_under_the_components_own_path_certifies_it(self):
        """Positive: a manifest inside the component's path, named by its test command."""
        self.activate()
        rows = check.verify_components()
        evidence = rows[0]["activation_evidence"]
        self.assertEqual(evidence["component_path"], "crates/sub-1")
        self.assertTrue(evidence["manifest_path"].startswith("crates/sub-1/"))
        self.assertIn(rows[0]["name"], evidence["test_command"])

    def test_another_components_manifest_does_not_certify_this_one(self):
        """Negative: a tracked manifest belonging to another crate is refused."""
        self.activate()
        self.track("crates/sub-9/Cargo.toml")
        self.data["components"][0]["activation_evidence"][
            "manifest_path"
        ] = "crates/sub-9/Cargo.toml"
        self.write_components()
        with self.assertRaises(ValueError) as caught:
            check.verify_components()
        self.assertIn("outside its own path", str(caught.exception))

    def test_the_evidence_binding_is_exact(self):
        """Boundary: a sibling prefix, a foreign path and an unnamed command all fail."""
        # "crates/sub-10" merely starts with "crates/sub-1"; it is not inside it.
        self.activate()
        self.track("crates/sub-10/Cargo.toml")
        self.data["components"][0]["activation_evidence"][
            "manifest_path"
        ] = "crates/sub-10/Cargo.toml"
        self.write_components()
        with self.assertRaises(ValueError) as caught:
            check.verify_components()
        self.assertIn("outside its own path", str(caught.exception))

        self.activate(component_path="crates")
        with self.assertRaises(ValueError) as caught:
            check.verify_components()
        self.assertIn("does not name", str(caught.exception))

        self.activate(component_path="crates/sub-1/Cargo.toml")
        with self.assertRaises(ValueError) as caught:
            check.verify_components()
        self.assertIn("is not a directory", str(caught.exception))

        self.activate(test_command="make verify-rust")
        with self.assertRaises(ValueError) as caught:
            check.verify_components()
        self.assertIn("test command does not name", str(caught.exception))

    def test_activation_still_needs_remaining_blockers(self):
        """Boundary: activation is not a runtime claim, so blockers must remain."""
        self.activate()
        self.data["components"][0]["activation_blockers"] = []
        self.write_components()
        with self.assertRaises(ValueError) as caught:
            check.verify_components()
        self.assertNotIn("names no milestone", str(caught.exception))

    def test_activation_needs_a_milestone_with_every_other_guard_satisfied(self):
        """Boundary: the missing-milestone guard is reached, not masked by another."""
        row = self.activate()
        self.assertTrue(row["activation_blockers"], "blockers must be intact here")
        del self.data["components"][0]["milestone"]
        self.write_components()
        with self.assertRaises(ValueError) as caught:
            check.verify_components()
        self.assertIn("names no milestone", str(caught.exception))

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

    def write_profile(self, **overrides):
        data = {
            "schema_version": 1,
            "evidence_class": "Development evidence only. It closes no gate.",
            "cpu": {"model": "reference cpu"},
            "capabilities": {
                name: {
                    "present": name not in ("realtime_kernel", "secure_boot_enrolment"),
                    "evidence_command": "ls /dev/null",
                }
                for name in check.PROFILE_CAPABILITIES
            },
        }
        data.update(overrides)
        (self.root / "planning/hardware-profile.json").write_text(json.dumps(data))
        return data

    def test_privacy_guard_rejects_tracked_working_data(self):
        self.git_init()
        check.verify_privacy()
        private = self.root / ".workingdir"
        private.mkdir()
        (private / "note.md").write_text("private")
        git("git", "-C", str(self.root), "add", "-f", ".workingdir/note.md")
        with self.assertRaises(ValueError):
            check.verify_privacy()

    def test_main_readiness_lists_every_component(self):
        self.write_components()
        self.git_init()
        self.write_licensing({})
        rows = [self.milestone("M00", 0, "ready")]
        self.write_roadmap(rows)
        self.write_roadmap_document(rows)
        self.write_register()
        self.write_profile()
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
            "unblocks": [],
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

    def write_roadmap_document(self, rows, table=None, sections=None):
        """Render both rank-and-state surfaces, optionally overriding either."""
        table = rows if table is None else table
        sections = rows if sections is None else sections
        head = (
            "# Roadmap\n\n| Rank | ID | Title | State | Cost |\n| --- | --- | --- | --- | --- |\n"
        )
        body = "".join(
            f"| {row['rank']} | {row['id']} | {row['title']} | {row['state']} | small |\n"
            for row in table
        )
        text = head + body
        for row in sections:
            text += (
                f"\n### {row['id']} - {row['title']}\n\n"
                f"Rank {row['rank']}. State: {row['state']}. Cost: small.\n"
            )
        (self.root / "docs/roadmap").mkdir(parents=True, exist_ok=True)
        (self.root / "docs/roadmap/README.md").write_text(text)

    def test_roadmap_document_matching_both_surfaces_passes(self):
        rows = [
            self.milestone("M00", 0, "done", evidence=["a2c9626"]),
            self.milestone("M01", 1, "ready", ["M00"]),
        ]
        self.write_roadmap_document(rows)
        check.verify_roadmap_document(rows)

    def test_roadmap_document_rejects_a_stale_surface(self):
        rows = [
            self.milestone("M00", 0, "done", evidence=["a2c9626"]),
            self.milestone("M01", 1, "ready", ["M00"]),
        ]
        stale = [dict(rows[0], state="ready"), rows[1]]
        # Either surface alone is enough to fail: they drift independently.
        for table, sections in ((stale, rows), (rows, stale)):
            self.write_roadmap_document(rows, table=table, sections=sections)
            with self.assertRaises(ValueError):
                check.verify_roadmap_document(rows)
        # A rank that disagrees fails for the same reason a state does.
        moved = [dict(rows[0], rank=7), rows[1]]
        self.write_roadmap_document(rows, table=moved, sections=moved)
        with self.assertRaises(ValueError):
            check.verify_roadmap_document(rows)

    def test_roadmap_document_rejects_a_missing_or_extra_milestone(self):
        rows = [
            self.milestone("M00", 0, "done", evidence=["a2c9626"]),
            self.milestone("M01", 1, "ready", ["M00"]),
        ]
        # One short on either surface, and one too many: the count is the boundary.
        self.write_roadmap_document(rows, table=rows[:1])
        with self.assertRaises(ValueError):
            check.verify_roadmap_document(rows)
        self.write_roadmap_document(rows, sections=rows[:1])
        with self.assertRaises(ValueError):
            check.verify_roadmap_document(rows)
        extra = rows + [self.milestone("M02", 2, "blocked", ["M01"])]
        self.write_roadmap_document(rows, table=extra, sections=extra)
        with self.assertRaises(ValueError):
            check.verify_roadmap_document(rows)

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

    def test_a_manifest_declaring_another_crate_does_not_certify_this_one(self):
        """Negative: the manifest's own package name must match the component."""
        self.activate()
        self.track("crates/sub-1/Cargo.toml", crate="someone-else")
        with self.assertRaises(ValueError) as caught:
            check.verify_components()
        self.assertIn("not 'sub-1'", str(caught.exception))

    def test_a_test_command_naming_a_longer_crate_does_not_certify_this_one(self):
        """Boundary: the component name must appear as a whole word."""
        self.activate(test_command="cargo test -p sub-10")
        with self.assertRaises(ValueError):
            check.verify_components()
        self.activate(test_command="cargo test --locked -p sub-1")
        self.assertEqual(len(check.verify_components()), 16)

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

    def activated_components(self):
        """Return the inventory with P01 advanced to the activated status."""
        rows = [dict(row) for row in self.components]
        rows[0]["status"] = check.ACTIVATED_STATUS
        return rows

    def test_activated_component_may_record_its_manifest(self):
        """Positive: once the component is activated the register may claim it."""
        data = self.register()
        data["candidates"][0]["manifest_present"] = True
        data["candidates"][0]["lockfile_present"] = True
        self.register(candidates=data["candidates"])
        self.assertTrue(check.verify_candidates(self.activated_components()))

    def test_activated_component_without_a_recorded_manifest_fails(self):
        """Negative: an activated component whose row claims nothing fails closed."""
        self.register()
        with self.assertRaises(ValueError):
            check.verify_candidates(self.activated_components())

    def test_activated_component_needs_both_manifest_and_lock(self):
        """Boundary: one of the two claims is not enough; both are required."""
        for manifest, lockfile in ((True, False), (False, True)):
            data = self.register()
            data["candidates"][0]["manifest_present"] = manifest
            data["candidates"][0]["lockfile_present"] = lockfile
            self.register(candidates=data["candidates"])
            with self.assertRaises(ValueError):
                check.verify_candidates(self.activated_components())
        data = self.register()
        data["candidates"][0]["manifest_present"] = True
        data["candidates"][0]["lockfile_present"] = True
        self.register(candidates=data["candidates"])
        self.assertTrue(check.verify_candidates(self.activated_components()))

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


class HardwareProfileTests(unittest.TestCase):
    """Positive, negative and boundary coverage for the reference profile record."""

    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.patch = patch.object(check, "ROOT", self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        (self.root / "planning").mkdir()

    def write(self, **overrides):
        data = {
            "schema_version": 1,
            "evidence_class": "Development evidence only. It closes no gate.",
            "cpu": {"model": "reference cpu"},
            "capabilities": {
                name: {"present": name != "realtime_kernel", "evidence_command": "ls /dev/null"}
                for name in check.PROFILE_CAPABILITIES
            },
        }
        data.update(overrides)
        (self.root / "planning/hardware-profile.json").write_text(json.dumps(data))
        return data

    def milestone(self, support, requires):
        return [{"id": "M20", "reference_profile": {"support": support, "requires": requires}}]

    def test_complete_profile_and_supported_claim_pass(self):
        self.write()
        self.assertTrue(check.verify_hardware_profile(self.milestone("full", ["kvm", "tpm2"])))

    def test_evidence_class_and_schema_are_required(self):
        self.write(schema_version=2)
        with self.assertRaises(ValueError):
            check.verify_hardware_profile([])
        self.write(evidence_class="Qualifies the hardware for release.")
        with self.assertRaises(ValueError):
            check.verify_hardware_profile([])

    def test_capability_set_and_shape_are_exact(self):
        data = self.write()
        caps = dict(data["capabilities"])
        caps.pop("kvm")
        self.write(capabilities=caps)
        with self.assertRaises(ValueError):
            check.verify_hardware_profile([])
        caps = dict(data["capabilities"])
        caps["kvm"] = {"present": "yes", "evidence_command": "ls"}
        self.write(capabilities=caps)
        with self.assertRaises(ValueError):
            check.verify_hardware_profile([])
        caps["kvm"] = {"present": True}
        self.write(capabilities=caps)
        with self.assertRaises(ValueError):
            check.verify_hardware_profile([])

    def test_machine_identifiers_are_rejected(self):
        self.write(hostname="workstation")
        with self.assertRaises(ValueError):
            check.verify_hardware_profile([])

    def test_milestone_claims_are_bounded_by_the_profile(self):
        self.write()
        with self.assertRaises(ValueError):
            check.verify_hardware_profile(self.milestone("full", ["realtime_kernel"]))
        with self.assertRaises(ValueError):
            check.verify_hardware_profile(self.milestone("full", ["time_travel"]))
        self.assertTrue(
            check.verify_hardware_profile(self.milestone("partial", ["realtime_kernel"]))
        )
        self.assertTrue(check.verify_hardware_profile([{"id": "M02"}]))
