"""Planted cases for tools/verify_unsafe_home.py.

Each test sets up a fixture tree based on committed manifests and plants one change
to demonstrate that each refusal catches its failure class, plus boundary cases for
comments and string literals.
"""

import contextlib
import datetime
import io
import pathlib
import re
import shutil
import tempfile
import unittest

try:
    import verify_unsafe_home as vuh
except ImportError:
    from tools import verify_unsafe_home as vuh

REPO = vuh.ROOT
# The planted cases pin the fixture entry to these dates, so a renewal of the
# live entry in .standards.yaml does not move what the cases assert.
FIXTURE_ENTERED = "2026-01-01"
FIXTURE_EXPIRES = "2026-01-11"
FIXTURE_TODAY = datetime.date(2026, 1, 5)


class UnsafeHomeFixture(unittest.TestCase):
    """Fixture tree initialized with the manifests and standards from the repository."""

    def setUp(self):
        self.tmp = tempfile.TemporaryDirectory()
        self.root = pathlib.Path(self.tmp.name)
        # Copy root Cargo.toml and .standards.yaml
        shutil.copyfile(REPO / "Cargo.toml", self.root / "Cargo.toml")
        shutil.copyfile(REPO / vuh.MANIFEST, self.root / vuh.MANIFEST)
        self._pin_fixture_dates()

        # Copy member manifests
        for line in (REPO / "Cargo.toml").read_text(encoding="utf-8").splitlines():
            if "crates/" in line:
                member = line.strip().strip('",')
                member_dir = self.root / member
                member_dir.mkdir(parents=True, exist_ok=True)
                shutil.copyfile(REPO / member / "Cargo.toml", member_dir / "Cargo.toml")

    def _pin_fixture_dates(self):
        path = self.root / vuh.MANIFEST
        text = path.read_text(encoding="utf-8")
        head, sep, tail = text.partition(f"path: {vuh.VULCAN_GPU_MANIFEST}")
        self.assertTrue(sep, "the live HISS-09 entry is missing from .standards.yaml")
        tail = re.sub(r'entered: "[0-9-]+"', f'entered: "{FIXTURE_ENTERED}"', tail, count=1)
        tail = re.sub(r'expires: "[0-9-]+"', f'expires: "{FIXTURE_EXPIRES}"', tail, count=1)
        path.write_text(head + sep + tail, encoding="utf-8")

    def problems_on(self, today):
        return vuh.check(self.root, today)

    def tearDown(self):
        self.tmp.cleanup()

    def replace_in_file(self, rel_path, old, new):
        path = self.root / rel_path
        text = path.read_text(encoding="utf-8")
        self.assertEqual(
            text.count(old),
            1,
            f"Expected exactly one occurrence of {old!r} in {rel_path}",
        )
        path.write_text(text.replace(old, new), encoding="utf-8")

    def problems(self, today=None):
        return vuh.check(self.root, today or FIXTURE_TODAY)

    def assertRefused(self, fragment, today=None):
        problems = self.problems(today)
        self.assertTrue(
            any(fragment in p for p in problems),
            f"Expected fragment {fragment!r} in problems: {problems}",
        )


class UnsafeHomeTests(UnsafeHomeFixture):
    def test_the_real_tree_passes(self):
        today = datetime.datetime.now(datetime.timezone.utc).date()
        self.assertEqual(vuh.check(REPO, today), [])

    def test_the_fixture_tree_passes(self):
        self.assertEqual(self.problems(), [])

    def test_unsafe_in_another_member_is_refused(self):
        tellus_src = self.root / "crates/aegis-tellus/src"
        tellus_src.mkdir(parents=True, exist_ok=True)
        planted_file = tellus_src / "planted.rs"
        planted_file.write_text(
            "pub fn bad() { unsafe { std::ptr::null::<i32>(); } }\n",
            encoding="utf-8",
        )
        self.assertRefused("unsafe token outside crates/aegis-vulcan-gpu is forbidden")

    def test_unsafe_fn_in_another_member_is_refused(self):
        minerva_src = self.root / "crates/aegis-minerva/src"
        minerva_src.mkdir(parents=True, exist_ok=True)
        planted_file = minerva_src / "planted.rs"
        planted_file.write_text(
            "pub unsafe fn bad_fn() {}\n",
            encoding="utf-8",
        )
        self.assertRefused("unsafe token outside crates/aegis-vulcan-gpu is forbidden")

    def test_block_without_safety_in_vulcan_gpu_is_refused(self):
        vulcan_src = self.root / "crates/aegis-vulcan-gpu/src"
        vulcan_src.mkdir(parents=True, exist_ok=True)
        planted_file = vulcan_src / "planted.rs"
        planted_file.write_text(
            """
#[allow(unsafe_code)]
pub fn do_call() {
    unsafe {
        let _ = 1;
    }
}
""",
            encoding="utf-8",
        )
        msg = "unsafe block is not directly preceded by a '// SAFETY:' comment"
        self.assertRefused(msg)

    def test_unsafe_item_without_allow_in_vulcan_gpu_is_refused(self):
        vulcan_src = self.root / "crates/aegis-vulcan-gpu/src"
        vulcan_src.mkdir(parents=True, exist_ok=True)
        planted_file = vulcan_src / "planted.rs"
        planted_file.write_text(
            """
pub fn do_call() {
    // SAFETY: verified valid
    unsafe {
        let _ = 1;
    }
}
""",
            encoding="utf-8",
        )
        self.assertRefused("unsafe item lacks #[allow(unsafe_code)] on its function")

    def test_safe_unsafe_block_with_safety_and_allow_passes(self):
        vulcan_src = self.root / "crates/aegis-vulcan-gpu/src"
        vulcan_src.mkdir(parents=True, exist_ok=True)
        planted_file = vulcan_src / "planted.rs"
        planted_file.write_text(
            """
#[allow(unsafe_code)]
pub fn do_call() {
    // SAFETY: verified pointer arithmetic is within bounds
    unsafe {
        let _ = 1;
    }
}
""",
            encoding="utf-8",
        )
        self.assertEqual(self.problems(), [])

    def test_vulcan_gpu_lints_drift_is_refused(self):
        self.replace_in_file(
            "crates/aegis-vulcan-gpu/Cargo.toml",
            'unsafe_code = "deny"',
            'unsafe_code = "warn"',
        )
        self.assertRefused("lints.rust.unsafe_code must be 'deny'")

    def test_vulcan_gpu_missing_lint_key_is_refused(self):
        self.replace_in_file(
            "crates/aegis-vulcan-gpu/Cargo.toml",
            'missing_docs = "deny"\n',
            "",
        )
        self.assertRefused("lints.rust keys differ from workspace.lints.rust")

    def test_other_member_missing_workspace_lints_is_refused(self):
        self.replace_in_file(
            "crates/aegis-tellus/Cargo.toml",
            "[lints]\nworkspace = true",
            "[lints]\nworkspace = false",
        )
        msg = "crates/aegis-tellus/Cargo.toml: missing [lints] workspace = true"
        self.assertRefused(msg)

    def test_forbid_relaxed_at_root_is_refused(self):
        self.replace_in_file(
            "Cargo.toml",
            'unsafe_code = "forbid"',
            'unsafe_code = "deny"',
        )
        self.assertRefused("workspace.lints.rust.unsafe_code must be 'forbid'")

    def test_missing_exception_entry_is_refused(self):
        self.replace_in_file(
            vuh.MANIFEST,
            "path: crates/aegis-vulcan-gpu/Cargo.toml",
            "path: crates/aegis-vulcan-gpu/unused.toml",
        )
        self.assertRefused("expected exactly one HISS-09 exceptions entry")

    def test_category_b_exception_is_refused(self):
        self.replace_in_file(
            vuh.MANIFEST,
            "category: C",
            "category: B",
        )
        self.assertRefused("exception category must be 'C'")

    def test_expired_exception_is_refused(self):
        expired = datetime.date(2026, 1, 12)
        self.assertRefused(f"exception expired on {FIXTURE_EXPIRES}", today=expired)

    def test_the_last_day_still_passes(self):
        self.assertEqual(self.problems_on(datetime.date(2026, 1, 11)), [])

    def test_excessive_expiry_interval_is_refused(self):
        self.replace_in_file(
            vuh.MANIFEST,
            f'expires: "{FIXTURE_EXPIRES}"',
            'expires: "2026-01-12"',
        )
        self.assertRefused("exception expires", today=datetime.date(2026, 1, 5))

    def test_reason_missing_decision_is_refused(self):
        self.replace_in_file(
            vuh.MANIFEST,
            vuh.UNSAFE_HOME_REASON,
            "other-reason",
        )
        self.assertRefused("exception reason does not name 'aegis-os-m25-unsafe-home'")

    def test_boundary_unsafe_in_comment_or_string_in_another_member_passes(self):
        tellus_src = self.root / "crates/aegis-tellus/src"
        tellus_src.mkdir(parents=True, exist_ok=True)
        planted_file = tellus_src / "planted.rs"
        planted_file.write_text(
            """
// unsafe { this is in a comment }
/* unsafe fn in a block comment */
pub const MSG: &str = "unsafe { in a string }";
pub const RAW: &str = r#"unsafe fn in a raw string"#;
""",
            encoding="utf-8",
        )
        self.assertEqual(self.problems(), [])

    def test_main_reports_pass_and_fail(self):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            self.assertEqual(vuh.main(["verify_unsafe_home.py", str(REPO)]), 0)
        self.assertIn("PASS:", out.getvalue())

        self.replace_in_file(
            vuh.MANIFEST,
            "category: C",
            "category: B",
        )
        err = io.StringIO()
        with contextlib.redirect_stderr(err):
            self.assertEqual(vuh.main(["verify_unsafe_home.py", str(self.root)]), 1)
        self.assertIn("FAIL:", err.getvalue())


if __name__ == "__main__":
    unittest.main()
