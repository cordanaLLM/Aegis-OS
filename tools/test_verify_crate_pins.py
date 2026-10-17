#!/usr/bin/env python3
"""Positive, negative and boundary coverage for the git-pin register check.

Each case writes a small workspace of known shape into a temporary directory,
plants one defect and expects the matching FAIL line, so no case needs cargo
or the network. One case runs the check on the committed repository, and one
copies the committed register, manifests and lock and plants a drifted
revision in the copy (issue #224).
"""

import contextlib
import io
import pathlib
import shutil
import tempfile
import unittest

import verify_crate_pins

REV_A = "a" * 40
REV_B = "b" * 40
SOURCE = "https://github.com/example/widget"
HEADER = (
    "| Crate | Source | Commit | Reason | Review trigger |\n"
    "| :--- | :--- | :--- | :--- | :--- |\n"
)


def row(name="widget", source=SOURCE, commit=REV_A, reason="not on crates.io", trigger="a release"):
    return f"| `{name}` | `{source}` | `{commit}` | {reason} | {trigger} |\n"


def register(rows):
    return f"# Admission\n\n{verify_crate_pins.HEADING}\n\nText.\n\n{HEADER}{rows}\n## Next\n"


def manifest(dep=None, extra=""):
    dep = dep if dep is not None else f'widget = {{ git = "{SOURCE}", rev = "{REV_A}" }}\n'
    return (
        '[workspace]\nmembers = ["crates/one"]\n\n'
        f'[workspace.dependencies]\nserde = "1.0"\n{dep}{extra}'
    )


def lock(source=f"git+{SOURCE}?rev={REV_A}#{REV_A}", extra=""):
    return (
        "version = 4\n\n"
        '[[package]]\nname = "serde"\nversion = "1.0.0"\n'
        'source = "registry+https://github.com/rust-lang/crates.io-index"\n\n'
        f'[[package]]\nname = "widget"\nversion = "0.1.0"\nsource = "{source}"\n{extra}'
    )


MEMBER = '[package]\nname = "one"\n\n[dependencies]\nwidget.workspace = true\n'


class Workspace:
    """A temporary workspace the check reads in place of the repository."""

    def __init__(self, test, reg=None, man=None, member=MEMBER, lck=None):
        self.dir = tempfile.TemporaryDirectory()
        test.addCleanup(self.dir.cleanup)
        self.root = pathlib.Path(self.dir.name)
        self.write(verify_crate_pins.REGISTER, reg if reg is not None else register(row()))
        self.write("Cargo.toml", man if man is not None else manifest())
        self.write("crates/one/Cargo.toml", member)
        self.write("Cargo.lock", lck if lck is not None else lock())

    def write(self, path, text):
        target = self.root / path
        target.parent.mkdir(parents=True, exist_ok=True)
        target.write_text(text, encoding="utf-8")

    def run(self):
        out, err = io.StringIO(), io.StringIO()
        with contextlib.redirect_stdout(out), contextlib.redirect_stderr(err):
            code = verify_crate_pins.main(["verify_crate_pins.py", str(self.root)])
        return code, out.getvalue(), err.getvalue()


class PositiveTest(unittest.TestCase):
    def test_a_matching_workspace_passes(self):
        code, out, err = Workspace(self).run()
        self.assertEqual((code, err), (0, ""))
        self.assertIn("PASS: 1 crates pinned by git revision", out)

    def test_the_committed_repository_passes(self):
        problems, count = verify_crate_pins.check(verify_crate_pins.ROOT)
        self.assertEqual(problems, [])
        self.assertGreaterEqual(count, 1)

    def test_the_makefile_runs_the_check(self):
        makefile = (verify_crate_pins.ROOT / "Makefile").read_text(encoding="utf-8")
        self.assertIn("\tpython3 tools/verify_crate_pins.py\n", makefile)


class NegativeTest(unittest.TestCase):
    def assertFails(self, workspace, needle):
        code, out, err = workspace.run()
        self.assertEqual(code, 1)
        self.assertEqual(out, "")
        self.assertIn(needle, err)

    def test_a_planted_drift_in_the_committed_files_fails(self):
        copy = tempfile.TemporaryDirectory()
        self.addCleanup(copy.cleanup)
        root = pathlib.Path(copy.name)
        source = verify_crate_pins.ROOT
        manifest_text = (source / "Cargo.toml").read_text(encoding="utf-8")
        for member in verify_crate_pins.tomllib.loads(manifest_text)["workspace"]["members"]:
            (root / member).mkdir(parents=True)
            shutil.copy(source / member / "Cargo.toml", root / member / "Cargo.toml")
        (root / "docs/roadmap").mkdir(parents=True)
        shutil.copy(source / verify_crate_pins.REGISTER, root / verify_crate_pins.REGISTER)
        shutil.copy(source / "Cargo.lock", root / "Cargo.lock")
        pins = verify_crate_pins.workspace_pins(verify_crate_pins.tomllib.loads(manifest_text))
        name, (_, rev) = sorted(pins.items())[0]
        planted = "0" * 40
        (root / "Cargo.toml").write_text(manifest_text.replace(rev, planted, 1), encoding="utf-8")
        problems, _ = verify_crate_pins.check(root)
        self.assertIn(f"{name}: Cargo.toml rev {planted} != register {rev}", problems)

    def test_a_git_pin_without_a_row_fails(self):
        self.assertFails(
            Workspace(self, reg=register("")),
            "widget: pinned by git in Cargo.toml but has no register row",
        )

    def test_a_stale_row_fails(self):
        reg = register(row() + row(name="gadget"))
        self.assertFails(
            Workspace(self, reg=reg), "gadget: has a register row but is not a git dependency"
        )

    def test_a_moved_revision_fails(self):
        self.assertFails(
            Workspace(self, reg=register(row(commit=REV_B))), f"rev {REV_A} != register {REV_B}"
        )

    def test_a_different_source_fails(self):
        reg = register(row(source="https://github.com/example/fork"))
        self.assertFails(Workspace(self, reg=reg), "!= register https://github.com/example/fork")

    def test_a_branch_pin_fails(self):
        dep = f'widget = {{ git = "{SOURCE}", branch = "main" }}\n'
        self.assertFails(Workspace(self, man=manifest(dep)), "widget is pinned by branch")

    def test_a_tag_pin_fails(self):
        dep = f'widget = {{ git = "{SOURCE}", tag = "v1" }}\n'
        self.assertFails(Workspace(self, man=manifest(dep)), "widget is pinned by tag")

    def test_a_member_git_dependency_fails(self):
        member = (
            MEMBER + f"\n[target.'cfg(unix)'.dev-dependencies]\nextra = {{ git = \"{SOURCE}\" }}\n"
        )
        self.assertFails(
            Workspace(self, member=member), "dev-dependencies] extra: git dependency outside"
        )

    def test_a_patch_git_source_fails(self):
        extra = f'\n[patch.crates-io]\nserde = {{ git = "{SOURCE}" }}\n'
        self.assertFails(
            Workspace(self, man=manifest(extra=extra)),
            "[patch.crates-io] serde: git source outside",
        )

    def test_a_lock_on_another_commit_fails(self):
        lck = lock(source=f"git+{SOURCE}?rev={REV_A}#{REV_B}")
        self.assertFails(
            Workspace(self, lck=lck), f"widget: Cargo.lock source git+{SOURCE}?rev={REV_A}#{REV_B}"
        )

    def test_an_unregistered_git_package_in_the_lock_fails(self):
        extra = (
            f'\n[[package]]\nname = "hidden"\nversion = "0.1.0"\nsource = "git+{SOURCE}#{REV_B}"\n'
        )
        self.assertFails(
            Workspace(self, lck=lock(extra=extra)), "hidden: Cargo.lock takes it from git"
        )

    def test_an_empty_reason_fails(self):
        self.assertFails(
            Workspace(self, reg=register(row(reason=""))), "empty reason or review trigger"
        )

    def test_an_empty_trigger_fails(self):
        self.assertFails(
            Workspace(self, reg=register(row(trigger=""))), "empty reason or review trigger"
        )

    def test_a_duplicated_row_fails(self):
        self.assertFails(Workspace(self, reg=register(row() + row())), "crate widget has two rows")

    def test_an_unreadable_row_fails(self):
        reg = register(row().replace(f"`{REV_A}`", f"{REV_A}"))
        self.assertFails(Workspace(self, reg=reg), "unreadable register row")

    def test_a_missing_heading_fails(self):
        reg = register(row()).replace(verify_crate_pins.HEADING, "## Elsewhere")
        self.assertFails(Workspace(self, reg=reg), "appears 0 times")


class BoundaryTest(unittest.TestCase):
    def assertRevRefused(self, rev):
        dep = f'widget = {{ git = "{SOURCE}", rev = "{rev}" }}\n'
        code, _, err = Workspace(self, man=manifest(dep)).run()
        self.assertEqual(code, 1)
        self.assertIn("is not a full 40-character commit", err)

    def test_a_39_character_rev_is_refused(self):
        self.assertRevRefused("a" * 39)

    def test_a_41_character_rev_is_refused(self):
        self.assertRevRefused("a" * 41)

    def test_an_upper_case_rev_is_refused(self):
        self.assertRevRefused("A" * 40)

    def test_a_git_dependency_without_rev_is_refused(self):
        code, _, err = Workspace(self, man=manifest(f'widget = {{ git = "{SOURCE}" }}\n')).run()
        self.assertEqual(code, 1)
        self.assertIn("rev '' is not a full 40-character commit", err)

    def test_no_git_dependency_and_no_row_passes(self):
        man = manifest(dep="")
        lck = "version = 4\n"
        code, out, _ = Workspace(self, reg=register(""), man=man, lck=lck).run()
        self.assertEqual(code, 0)
        self.assertIn("PASS: 0 crates", out)

    def test_a_second_heading_fails(self):
        reg = register(row()) + f"\n{verify_crate_pins.HEADING}\n"
        code, _, err = Workspace(self, reg=reg).run()
        self.assertEqual(code, 1)
        self.assertIn("appears 2 times", err)

    def test_rows_end_at_the_next_heading(self):
        reg = register(row()) + "\n" + HEADER + row(name="later")
        code, _, err = Workspace(self, reg=reg).run()
        self.assertEqual((code, err), (0, ""))


if __name__ == "__main__":
    unittest.main()
