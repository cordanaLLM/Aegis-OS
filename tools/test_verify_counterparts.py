"""Planted cases for tools/verify_counterparts.py (issue #230).

Each test copies the committed root manifest, every member manifest and the two
planning files into a temporary tree, plants one change and runs the check on
the copy, so the refusals are shown against the real records rather than a
hand-written fixture.
"""

import contextlib
import io
import json
import pathlib
import shutil
import sys
import tempfile
import tomllib
import unittest

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import verify_counterparts as vc  # noqa: E402

REPO = vc.ROOT
LEAVES = {
    "aegis-fabrica-defs",
    "aegis-hestia",
    "aegis-janus-lifecycle",
    "aegis-vulcan",
    "aegis-vulcan-gpu",
}


class CounterpartTree(unittest.TestCase):
    """A copy of the files the check reads, with helpers to plant changes."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        shutil.copy(REPO / "Cargo.toml", self.root / "Cargo.toml")
        manifest = tomllib.loads((REPO / "Cargo.toml").read_text(encoding="utf-8"))
        for member in manifest["workspace"]["members"]:
            (self.root / member).mkdir(parents=True)
            shutil.copy(REPO / member / "Cargo.toml", self.root / member / "Cargo.toml")
        (self.root / "planning").mkdir()
        for name in (vc.COMPONENTS, vc.ROADMAP):
            shutil.copy(REPO / name, self.root / name)

    def tearDown(self):
        self.tmp.cleanup()

    def components(self):
        return json.loads((self.root / vc.COMPONENTS).read_text(encoding="utf-8"))

    def write_components(self, data):
        (self.root / vc.COMPONENTS).write_text(json.dumps(data), encoding="utf-8")

    def row(self, data, component):
        return next(row for row in data["components"] if row["id"] == component)

    def edit_entry(self, component, index, **changes):
        data = self.components()
        self.row(data, component)[vc.FIELD][index].update(changes)
        self.write_components(data)

    def append_manifest(self, member, text):
        path = self.root / "crates" / member / "Cargo.toml"
        path.write_text(path.read_text(encoding="utf-8") + text, encoding="utf-8")

    def problems(self):
        problems, _, _ = vc.check(self.root)
        return problems

    def assert_refused(self, fragment):
        problems = self.problems()
        self.assertTrue(
            any(fragment in problem for problem in problems),
            f"no problem names {fragment!r}: {problems}",
        )


class Positive(CounterpartTree):
    def test_the_committed_records_pass(self):
        problems, leaves, entries = vc.check(self.root)
        self.assertEqual(problems, [])
        self.assertEqual(leaves, len(LEAVES))
        self.assertEqual(entries, 13)

    def test_the_leaf_set_is_the_five_recorded_crates(self):
        self.assertEqual(vc.leaf_crates(vc.workspace_graph(self.root)), LEAVES)

    def test_the_shell_pairs_the_token_crate(self):
        # M28 E28-5: the gpui shell compiles the Concordia constants, so
        # aegis-concordia has its P05 counterpart and is no leaf.
        graph = vc.workspace_graph(self.root)
        self.assertIn("aegis-concordia", graph["aegis-forum-shell-gpui"])
        self.assertNotIn("aegis-concordia", vc.leaf_crates(graph))

    def test_main_passes_on_the_repository(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(vc.main(["verify_counterparts.py", str(self.root)]), 0)
        self.assertIn("PASS: 5 leaf crates", out.getvalue())


class Negative(CounterpartTree):
    def test_a_missing_edge_entry_fails(self):
        data = self.components()
        entries = self.row(data, "P03")[vc.FIELD]
        self.row(data, "P03")[vc.FIELD] = [
            e
            for e in entries
            if not (e["crate"] == "aegis-vulcan" and e["relation"] == "ZERO_COPY_MEDIA_INGEST")
        ]
        self.write_components(data)
        self.assert_refused("aegis-vulcan: P03 edge ZERO_COPY_MEDIA_INGEST with P15 has no entry")

    def test_a_leaf_crate_without_entries_fails(self):
        data = self.components()
        del self.row(data, "P15")[vc.FIELD]
        self.write_components(data)
        self.assert_refused("aegis-hestia: leaf crate needs its counterpart_gaps")

    def test_an_entry_for_a_paired_crate_is_stale(self):
        self.append_manifest(
            "aegis-hestia",
            '\n[dev-dependencies]\naegis-vulcan = { path = "../aegis-vulcan" }\n',
        )
        self.assert_refused("aegis-hestia: has counterpart_gaps entries but is not a leaf any more")
        self.assert_refused("aegis-vulcan: has counterpart_gaps entries but is not a leaf any more")

    def test_an_entry_for_a_crate_outside_the_workspace_fails(self):
        self.edit_entry("P01", 0, crate="aegis-fabrica")
        self.assert_refused(
            "aegis-fabrica: has counterpart_gaps entries but is not a workspace member"
        )

    def test_an_edge_the_component_lacks_fails(self):
        self.edit_entry("P03", 0, counterpart="P10")
        self.assert_refused("GPUDIRECT_WEIGHT_STREAMING with P10 is not an edge of this component")

    def test_an_unknown_milestone_fails(self):
        self.edit_entry("P02", 3, milestone="M99")
        self.assert_refused("milestone 'M99' is not in planning/roadmap.json")

    def test_a_done_milestone_fails(self):
        self.edit_entry("P15", 0, milestone="M27")
        self.assert_refused("milestone M27 is done")

    def test_an_empty_reason_fails(self):
        self.edit_entry("P15", 1, reason="  ")
        self.assert_refused("P15 counterpart_gaps[1]: the reason is empty")

    def test_an_extra_or_missing_key_fails(self):
        self.edit_entry("P15", 0, note="extra")
        self.assert_refused("P15 counterpart_gaps[0]: an entry must carry exactly")
        data = self.components()
        del self.row(data, "P15")[vc.FIELD][0]["note"]
        del self.row(data, "P15")[vc.FIELD][0]["milestone"]
        self.write_components(data)
        self.assert_refused("P15 counterpart_gaps[0]: an entry must carry exactly")

    def test_a_repeated_entry_fails(self):
        data = self.components()
        entries = self.row(data, "P15")[vc.FIELD]
        entries.append(dict(entries[0]))
        self.write_components(data)
        self.assert_refused("aegis-hestia: ZERO_COPY_MEDIA_INGEST with P03 is recorded twice")

    def test_entries_split_over_two_components_fail(self):
        data = self.components()
        moved = self.row(data, "P01")[vc.FIELD].pop()
        moved["crate"] = "aegis-janus-lifecycle"
        self.row(data, "P01")[vc.FIELD].append(moved)
        self.write_components(data)
        self.assert_refused("aegis-janus-lifecycle: leaf crate needs its counterpart_gaps")

    def test_an_unreadable_planning_file_fails_closed(self):
        (self.root / vc.ROADMAP).write_text("{", encoding="utf-8")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(vc.main(["verify_counterparts.py", str(self.root)]), 1)
        self.assertIn("FAIL:", err.getvalue())

    def test_main_reports_each_problem(self):
        self.edit_entry("P15", 0, reason="")
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(vc.main(["verify_counterparts.py", str(self.root)]), 1)
        self.assertIn("FAIL: P15 counterpart_gaps[0]: the reason is empty", err.getvalue())


class Boundary(CounterpartTree):
    def test_a_null_milestone_is_accepted(self):
        self.edit_entry("P03", 0, milestone=None)
        self.assertEqual(self.problems(), [])

    def test_a_milestone_not_yet_done_is_accepted(self):
        self.edit_entry("P15", 1, milestone="M29")
        self.assertEqual(self.problems(), [])

    def test_a_renamed_dependency_still_pairs_the_crates(self):
        self.append_manifest(
            "aegis-hestia",
            '\n[dev-dependencies]\nshell = { package = "aegis-forum-shell", '
            'path = "../aegis-forum-shell" }\n',
        )
        self.assert_refused("aegis-hestia: has counterpart_gaps entries but is not a leaf any more")

    def test_a_target_specific_dependency_still_pairs_the_crates(self):
        self.append_manifest(
            "aegis-vulcan",
            "\n[target.'cfg(unix)'.dependencies]\n"
            'aegis-forum-shell = { path = "../aegis-forum-shell" }\n',
        )
        self.assert_refused("aegis-vulcan: has counterpart_gaps entries but is not a leaf any more")

    def test_a_dependency_outside_the_workspace_keeps_a_leaf(self):
        self.append_manifest("aegis-vulcan", '\n[dev-dependencies]\nserde_json = "1"\n')
        self.assertEqual(self.problems(), [])

    def test_an_empty_workspace_has_no_leaves(self):
        self.assertEqual(vc.leaf_crates({}), set())


if __name__ == "__main__":
    unittest.main()
