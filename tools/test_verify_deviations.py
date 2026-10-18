"""Planted cases for tools/verify_deviations.py (issue #223).

Each test copies the committed .standards.yaml and Cargo.lock into a temporary
tree, plants one change and runs the check on the copy, so the refusals are
shown against the real records rather than a hand-written fixture.
"""

import contextlib
import datetime
import io
import pathlib
import shutil
import sys
import tempfile
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import verify_deviations as vd  # noqa: E402

REPO = vd.ROOT
D83 = vd.DEVIATIONS[0]
D115 = vd.DEVIATIONS[1]


class DeviationTree(unittest.TestCase):
    """A copy of the files the check reads, with helpers to plant changes."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        for name in (vd.MANIFEST, vd.LOCK):
            shutil.copyfile(REPO / name, self.root / name)
        self.entry = self.d83_entry()

    def tearDown(self):
        self.tmp.cleanup()

    def d83_entry(self):
        entries = vd.exception_entries((self.root / vd.MANIFEST).read_text(encoding="utf-8"))
        return next(entry for entry in entries if entry.get("path") == D83["path"])

    def replace(self, name, old, new):
        path = self.root / name
        text = path.read_text(encoding="utf-8")
        self.assertEqual(text.count(old), 1, old)
        path.write_text(text.replace(old, new), encoding="utf-8")

    def problems(self, today=None):
        return vd.check(self.root, today or datetime.date.fromisoformat(self.entry["entered"]))

    def assertRefused(self, fragment, today=None):
        problems = self.problems(today)
        self.assertTrue(any(fragment in problem for problem in problems), problems)


class DeviationTests(DeviationTree):
    def test_the_committed_entry_holds_d83(self):
        self.assertEqual(self.problems(), [])
        self.assertEqual(vd.check(REPO, datetime.date.today()), [])

    def test_the_entry_holds_on_its_expiry_day_and_not_after(self):
        expires = datetime.date.fromisoformat(self.entry["expires"])
        self.assertEqual(self.problems(expires), [])
        self.assertRefused("expired", expires + datetime.timedelta(days=1))

    def test_a_missing_entry_is_refused(self):
        self.replace(vd.MANIFEST, f"path: {D83['path']}", "path: crates/aegis-scaena/src/lib.rs")
        self.assertRefused("0 exceptions entries")

    def test_a_repeated_entry_is_refused(self):
        path = self.root / vd.MANIFEST
        text = path.read_text(encoding="utf-8")
        block = text[text.index("  - rule: HISS-03") : text.index("\nregister:") + 1]
        path.write_text(text.replace(block, block + block), encoding="utf-8")
        self.assertRefused("2 exceptions entries")

    def test_an_entry_without_a_category_is_refused(self):
        self.replace(
            vd.MANIFEST,
            "re-pin of the decoder (category A, 90 days)'\n    category: A\n",
            "re-pin of the decoder (category A, 90 days)'\n",
        )
        self.assertRefused("no category")

    def test_a_reason_without_the_decision_is_refused(self):
        self.replace(vd.MANIFEST, "'D83:", "'deviation:")
        self.assertRefused("does not name the decision")

    def test_a_repin_of_the_decoder_is_refused_until_the_entry_is_renewed(self):
        revision = vd.locked_revision((REPO / vd.LOCK).read_text(encoding="utf-8"), "cros-libva")
        self.assertIsNotNone(revision)
        path = self.root / vd.LOCK
        path.write_text(path.read_text(encoding="utf-8").replace(revision, "0" * 40))
        self.assertRefused("review the deviation and renew the entry")

    def test_a_package_without_a_pinned_revision_names_none(self):
        lock = (REPO / vd.LOCK).read_text(encoding="utf-8")
        self.assertIsNone(vd.locked_revision(lock, "serde"))
        self.assertIsNone(vd.locked_revision(lock, "no-such-package"))

    def test_the_reader_refuses_shapes_it_does_not_model(self):
        for line in ("    owner: someone\n", "    reason: >\n", "      folded\n", "\tpath: x\n"):
            with self.subTest(line=line):
                with self.assertRaises(vd.DeviationError):
                    vd.exception_entries(f"exceptions:\n  - rule: HISS-03\n{line}")
        for text in ("    rule: HISS-03\n", "  - rule: HISS-03\n    rule: HISS-04\n"):
            with self.subTest(text=text):
                with self.assertRaises(vd.DeviationError):
                    vd.exception_entries(f"exceptions:\n{text}")
        self.assertEqual(vd.exception_entries("version: 1\n"), [])
        self.assertEqual(
            vd.exception_entries("exceptions:\n  - rule: X\n    reason: 'it''s'\nnext: 1\n"),
            [{"rule": "X", "reason": "it's"}],
        )

    def test_main_reports_pass_and_fail(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(vd.main(["verify_deviations.py", str(REPO)]), 0)
        self.assertIn("PASS: 2 scoped deviation(s)", out.getvalue())
        self.assertIn("D83, D115", out.getvalue())
        self.replace(vd.MANIFEST, "'D83:", "'deviation:")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(vd.main(["verify_deviations.py", str(self.root)]), 1)
        self.assertIn("FAIL:", err.getvalue())


class GpuiDeviationTests(DeviationTree):
    """D115: gpui rebuilds its AccessKit tree every frame at the pinned zed commit."""

    def test_d115_names_gpui_through_its_one_member(self):
        self.assertEqual(D115["decision"], "D115")
        self.assertEqual(D115["rule"], "HISS-03")
        self.assertEqual(D115["package"], "gpui")
        self.assertEqual(D115["path"], "crates/aegis-forum-shell-gpui/Cargo.toml")

    def test_the_committed_entry_names_the_locked_zed_commit(self):
        revision = vd.locked_revision((REPO / vd.LOCK).read_text(encoding="utf-8"), "gpui")
        self.assertIsNotNone(revision)
        entries = vd.exception_entries((REPO / vd.MANIFEST).read_text(encoding="utf-8"))
        entry = next(
            entry
            for entry in entries
            if entry.get("path") == D115["path"] and entry.get("rule") == "HISS-03"
        )
        self.assertIn(revision, entry["reason"])
        self.assertIn("crates/gpui/src/window/a11y.rs", entry["reason"])
        self.assertEqual(entry["category"], "A")

    def test_a_repin_of_gpui_is_refused_until_the_entry_is_renewed(self):
        revision = vd.locked_revision((REPO / vd.LOCK).read_text(encoding="utf-8"), "gpui")
        path = self.root / vd.LOCK
        path.write_text(path.read_text(encoding="utf-8").replace(revision, "f" * 40))
        problems = self.problems()
        self.assertTrue(
            any(p.startswith("D115") and "renew the entry" in p for p in problems), problems
        )
        self.assertFalse(any(p.startswith("D83") for p in problems), problems)

    def test_a_missing_d115_entry_is_refused(self):
        self.replace(vd.MANIFEST, "'D115:", "'deviation:")
        self.assertRefused("D115 (HISS-03, crates/aegis-forum-shell-gpui/Cargo.toml): the reason")


if __name__ == "__main__":
    unittest.main()
