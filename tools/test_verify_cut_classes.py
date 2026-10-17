"""Planted cases for tools/verify_cut_classes.py (issue #221).

Each test copies the committed root manifest, every member manifest, the BPF
programs, the ui directories and planning/components.json into a temporary
tree, plants one change and runs the check on the copy, so the refusals are
shown against the real records rather than a hand-written fixture.
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

import verify_cut_classes as vcc  # noqa: E402

REPO = vcc.ROOT
OS_DEPENDENCY = "rustix.workspace = true\n"


class CutTree(unittest.TestCase):
    """A copy of the files the check reads, with helpers to plant changes."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name).resolve()
        shutil.copy(REPO / "Cargo.toml", self.root / "Cargo.toml")
        manifest = tomllib.loads((REPO / "Cargo.toml").read_text(encoding="utf-8"))
        for member in manifest["workspace"]["members"]:
            (self.root / member).mkdir(parents=True)
            shutil.copy(REPO / member / "Cargo.toml", self.root / member / "Cargo.toml")
        (self.root / "bpf").mkdir()
        for program in (REPO / "bpf").glob("*.bpf.c"):
            shutil.copy(program, self.root / "bpf" / program.name)
        for entry in (REPO / "ui").iterdir():
            if entry.is_dir():
                (self.root / "ui" / entry.name).mkdir(parents=True)
        (self.root / "planning").mkdir()
        shutil.copy(REPO / vcc.COMPONENTS, self.root / vcc.COMPONENTS)

    def tearDown(self):
        self.tmp.cleanup()

    def components(self):
        return json.loads((self.root / vcc.COMPONENTS).read_text(encoding="utf-8"))

    def write_components(self, data):
        (self.root / vcc.COMPONENTS).write_text(json.dumps(data), encoding="utf-8")

    def edit_cut(self, component, change):
        data = self.components()
        row = next(row for row in data["components"] if row["id"] == component)
        change(row)
        self.write_components(data)

    def add_dependency(self, member, table, line):
        path = self.root / "crates" / member / "Cargo.toml"
        text = path.read_text(encoding="utf-8")
        if f"[{table}]\n" in text:
            text = text.replace(f"[{table}]\n", f"[{table}]\n{line}", 1)
        else:
            text += f"\n[{table}]\n{line}"
        path.write_text(text, encoding="utf-8")

    def run_main(self, *args):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = vcc.main(["verify_cut_classes.py", *args, str(self.root)])
        return code, out.getvalue(), err.getvalue()

    def assert_refused(self, fragment):
        problems, _ = vcc.check(self.root)
        self.assertTrue(
            any(fragment in problem for problem in problems),
            f"no problem names {fragment!r}: {problems}",
        )


class Positive(CutTree):
    def test_the_committed_records_pass(self):
        problems, rows = vcc.check(self.root)
        self.assertEqual(problems, [])
        self.assertEqual(len(rows), 17)

    def test_main_prints_one_line_per_component(self):
        code, out, _ = self.run_main("--print")
        self.assertEqual(code, 0)
        self.assertIn("P17 aegis-scaena cut: os-only; crates/aegis-scaena (os-bound)", out)
        self.assertIn(
            "P10 aegis-vesta cut: core-and-adapter; crates/aegis-vesta (os-free), "
            "crates/aegis-vesta-sandbox (os-bound)",
            out,
        )
        self.assertIn("PASS: 17 components carry the cut class their code gives", out)

    def test_main_without_print_shows_only_the_summary(self):
        code, out, _ = self.run_main()
        self.assertEqual(code, 0)
        self.assertNotIn(" cut: ", out)


class Negative(CutTree):
    def test_an_os_dependency_on_an_os_free_crate_fails(self):
        self.add_dependency("aegis-tellus", "dependencies", OS_DEPENDENCY)
        self.assert_refused("P13 cut: crates/aegis-tellus records os-free, measured os-bound")

    def test_an_os_bound_crate_binds_its_dependents(self):
        self.add_dependency("aegis-tellus", "dependencies", OS_DEPENDENCY)
        self.assert_refused("P16 cut: crates/aegis-athena records os-free, measured os-bound")

    def test_a_recorded_os_bound_crate_that_is_free_fails(self):
        self.edit_cut("P13", lambda row: row["cut"]["parts"][1].update(ties="os-bound"))
        self.assert_refused("crates/aegis-tellus-rapl records os-bound, measured os-free")

    def test_a_class_the_parts_do_not_give_fails(self):
        self.edit_cut("P17", lambda row: row["cut"].update({"class": "standalone"}))
        self.assert_refused("P17 cut: class standalone recorded, its parts give os-only")

    def test_an_unknown_class_fails(self):
        self.edit_cut("P03", lambda row: row["cut"].update({"class": "partial"}))
        self.assert_refused("P03 cut: class 'partial' is not one of")

    def test_a_part_of_no_component_fails(self):
        self.edit_cut("P13", lambda row: row["cut"]["parts"].pop())
        self.assert_refused(
            "bpf/kepler_power.bpf.c: must be a part of exactly one component, found []"
        )

    def test_a_new_program_without_an_owner_fails(self):
        (self.root / "bpf" / "planted.bpf.c").write_text("", encoding="utf-8")
        self.assert_refused("bpf/planted.bpf.c: must be a part of exactly one component")

    def test_a_part_of_two_components_fails(self):
        part = {"path": "crates/aegis-tellus", "ties": "os-free"}
        self.edit_cut("P16", lambda row: row["cut"]["parts"].append(part))
        self.assert_refused(
            "crates/aegis-tellus: must be a part of exactly one component, " "found ['P13', 'P16']"
        )

    def test_an_unknown_path_fails(self):
        part = {"path": "crates/aegis-fabrica", "ties": "os-bound"}
        self.edit_cut("P01", lambda row: row["cut"]["parts"].append(part))
        self.assert_refused("P01 cut: crates/aegis-fabrica is no workspace crate")

    def test_a_missing_record_fails(self):
        self.edit_cut("P05", lambda row: row.pop("cut"))
        self.assert_refused("P05 cut: must carry exactly ['class', 'parts']")

    def test_main_reports_each_problem_and_fails(self):
        self.edit_cut("P17", lambda row: row["cut"].update({"class": "standalone"}))
        code, out, err = self.run_main("--print")
        self.assertEqual(code, 1)
        self.assertEqual(out, "")
        self.assertIn("FAIL: P17 cut: class standalone", err)

    def test_an_unreadable_record_fails(self):
        (self.root / vcc.COMPONENTS).write_text("{", encoding="utf-8")
        code, _, err = self.run_main()
        self.assertEqual(code, 1)
        self.assertIn("FAIL: JSONDecodeError", err)


class Boundary(CutTree):
    def test_a_dev_dependency_does_not_bind(self):
        self.add_dependency("aegis-tellus", "dev-dependencies", OS_DEPENDENCY)
        problems, _ = vcc.check(self.root)
        self.assertEqual(problems, [])

    def test_a_target_dependency_binds(self):
        self.add_dependency("aegis-vulcan", "target.'cfg(unix)'.dependencies", OS_DEPENDENCY)
        self.assert_refused("P03 cut: crates/aegis-vulcan records os-free, measured os-bound")

    def test_a_renamed_allowed_package_stays_free(self):
        self.add_dependency(
            "aegis-vulcan", "dependencies", 'hashing = { package = "sha2", version = "0.11" }\n'
        )
        problems, _ = vcc.check(self.root)
        self.assertEqual(problems, [])

    def test_an_empty_part_list_fails(self):
        self.edit_cut("P09", lambda row: row["cut"].update({"parts": []}))
        self.assert_refused("P09 cut: parts must be a non-empty list")

    def test_a_part_with_an_extra_key_fails(self):
        self.edit_cut("P09", lambda row: row["cut"]["parts"][0].update(basis="x"))
        self.assert_refused("P09 cut.parts[0]: must carry exactly ['path', 'ties']")

    def test_mixed_and_uniform_parts_give_the_three_classes(self):
        self.assertEqual(vcc.derived_class(["os-free"]), "standalone")
        self.assertEqual(vcc.derived_class(["os-free", "os-bound"]), "core-and-adapter")
        self.assertEqual(vcc.derived_class(["os-bound", "os-bound"]), "os-only")


if __name__ == "__main__":
    unittest.main()
