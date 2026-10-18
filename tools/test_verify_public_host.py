"""Regressions for tools/verify_public_host.py: one planted defect per rule.

Every planted value is built from fragments, so that the gate, which scans this
file as a tracked file, never refuses its own tests.
"""

import contextlib
import io
import os
import pathlib
import shutil
import subprocess
import sys
import tempfile
import unittest
import unittest.mock

sys.path.insert(0, str(pathlib.Path(__file__).resolve().parent))

import verify_public_host as gate  # noqa: E402

PLANTED_HOST = "host" + "alpha"
REAL_BUS_ADDRESS = "0000" + ":03:00.0"
FIXTURE_BUS_ADDRESS = "0000" + ":f3:00.0"
NAMED_HOME = "/home/" + "alice/.cache"
CPU_MODEL = "Ryzen" + " 9 7950X"
X3D_PART = "7800" + "X3D"


def rules(planted=()):
    """Host-name rules for the planted names only, never the running host."""
    return [rule for rule in gate._host_name_rules(planted) if rule.pattern != ""]


def found(text, prose=True, planted=()):
    return [rule for rule, _value in gate.scan_text(text, rules(planted), prose)]


class IdentityRuleTests(unittest.TestCase):
    """The identity rules bind every tracked file."""

    def test_a_deny_hosts_entry_is_refused_and_never_echoed(self):
        """Negative: the planted name is refused, inside an identifier too, and not printed."""
        for text in (f"ran on {PLANTED_HOST} today", f"ledger-{PLANTED_HOST}-correction"):
            report = gate.scan_text(text, gate._host_name_rules([PLANTED_HOST]), prose=False)
            self.assertIn("host-name", [rule for rule, _ in report])
            self.assertNotIn(PLANTED_HOST, " ".join(value for _, value in report))

    def test_a_short_deny_entry_is_ignored(self):
        """Boundary: names under MIN_HOST_NAME characters would match ordinary words."""
        short = "x" * (gate.MIN_HOST_NAME - 1)
        exact = "y" * gate.MIN_HOST_NAME
        names = {rule.pattern for rule in gate._host_name_rules([short, exact])}
        self.assertNotIn(short, names)
        self.assertIn(exact, names)

    def test_a_pci_address_is_refused_outside_the_fixture_bus(self):
        """Negative and positive: a real bus is refused, the f-bus fixture range is allowed."""
        self.assertEqual(found(f"bind {REAL_BUS_ADDRESS}", prose=False), ["pci-address"])
        self.assertEqual(found(f"bind {REAL_BUS_ADDRESS[5:]}", prose=False), ["pci-address"])
        self.assertEqual(found(f"bind {FIXTURE_BUS_ADDRESS}", prose=False), [])

    def test_a_named_home_folder_is_refused(self):
        """Negative and positive: a named user is refused, the generic users are allowed."""
        self.assertEqual(found(NAMED_HOME, prose=False), ["home-path"])
        for user in sorted(gate.GENERIC_HOME_USERS):
            self.assertEqual(found(f"/home/{user}/x", prose=False), [], user)

    def test_one_machine_s_hardware_is_refused(self):
        """Negative: a CPU model and a part number are refused; a GPU model is not."""
        self.assertIn("host-hardware", found(CPU_MODEL, prose=False))
        self.assertIn("host-hardware", found(X3D_PART, prose=False))
        self.assertEqual(found("Arc A380 decoded 60 frames", prose=False), [])


class ProseRuleTests(unittest.TestCase):
    """The state rules bind prose only; code keeps synthetic fixtures."""

    def test_a_process_id_is_refused_in_prose_only(self):
        text = "compositor (pid " + "4321)"
        self.assertEqual(found(text), ["process-id"])
        self.assertEqual(found(text, prose=False), [])

    def test_a_group_number_below_the_fixture_floor_is_refused(self):
        """Boundary: FIXTURE_GROUP_MIN - 1 is refused, FIXTURE_GROUP_MIN is allowed."""
        below, floor = gate.FIXTURE_GROUP_MIN - 1, gate.FIXTURE_GROUP_MIN
        self.assertEqual(found(f"/dev/vfio/{below}"), ["iommu-group"])
        self.assertEqual(found(f"/dev/vfio/{floor}"), [])

    def test_an_epoch_time_is_refused_inside_its_window_only(self):
        """Boundary: the first second of the window is refused, the one before is not."""
        self.assertEqual(found(f"started {gate.EPOCH_MIN}"), ["epoch-time"])
        self.assertEqual(found(f"started {gate.EPOCH_MIN - 1}"), [])

    def test_session_state_is_refused(self):
        self.assertEqual(found("it hung the " + "session"), ["session-state"])
        self.assertEqual(found("the session is unaffected"), [])

    def test_prose_scope(self):
        self.assertTrue(gate.is_prose("docs/x.rs"))
        self.assertTrue(gate.is_prose("planning/roadmap.json"))
        self.assertTrue(gate.is_prose("crates/a/README.md"))
        self.assertFalse(gate.is_prose("crates/a/tests/t.rs"))


class CommandLineTests(unittest.TestCase):
    """The command line: a PR text file and a tracked tree."""

    def run_main(self, argv):
        out = io.StringIO()
        with contextlib.redirect_stdout(out):
            code = gate.main(argv)
        return code, out.getvalue()

    def test_a_pull_request_text_with_a_planted_address_fails(self):
        with tempfile.TemporaryDirectory() as scratch:
            body = pathlib.Path(scratch) / "body.md"
            body.write_text(f"Bound {REAL_BUS_ADDRESS} to vfio-pci.\n", encoding="utf-8")
            code, out = self.run_main(["--text-file", str(body)])
        self.assertEqual(code, 1)
        self.assertIn("pci-address", out)

    def test_a_clean_pull_request_text_passes(self):
        with tempfile.TemporaryDirectory() as scratch:
            body = pathlib.Path(scratch) / "body.md"
            body.write_text("The Arc A380 is found at run time.\n", encoding="utf-8")
            code, out = self.run_main(["--text-file", str(body)])
        self.assertEqual(code, 0, out)

    def test_the_deny_hosts_file_reaches_the_text_scan(self):
        with tempfile.TemporaryDirectory() as scratch:
            deny = pathlib.Path(scratch) / "deny"
            deny.write_text(f"# private\n{PLANTED_HOST}\n", encoding="utf-8")
            body = pathlib.Path(scratch) / "body.md"
            body.write_text(f"ran on {PLANTED_HOST.upper()}\n", encoding="utf-8")
            code, out = self.run_main(["--text-file", str(body), "--deny-hosts", str(deny)])
        self.assertEqual(code, 1)
        self.assertNotIn(PLANTED_HOST, out.lower())

    @staticmethod
    def make_repo(root):
        """A repository at root with one tracked and one untracked planted file."""
        env = gate.git_env()
        subprocess.run(["git", "init", "-q", str(root)], check=True, timeout=60, env=env)
        (root / "docs").mkdir()
        (root / "docs" / "a.md").write_text(f"see {NAMED_HOME}\n", encoding="utf-8")
        (root / "untracked.md").write_text(f"see {NAMED_HOME}\n", encoding="utf-8")
        add = ["git", "-C", str(root), "add", "docs/a.md"]
        subprocess.run(add, check=True, timeout=60, env=env)

    @unittest.skipIf(shutil.which("git") is None, "git not found")
    def test_a_tracked_file_is_scanned_and_an_untracked_one_is_not(self):
        with tempfile.TemporaryDirectory() as scratch:
            root = pathlib.Path(scratch)
            self.make_repo(root)
            report = gate.scan_tree(root, rules())
        self.assertEqual(report, [f"docs/a.md:1: home-path: {NAMED_HOME.rsplit('/', 1)[0]}"])

    @unittest.skipIf(shutil.which("git") is None, "git not found")
    def test_a_hook_s_git_environment_neither_steers_nor_receives_the_scan(self):
        """Boundary: a commit hook exports GIT_DIR and GIT_INDEX_FILE of the outer repository.

        The scan reads the repository at root, and the fixture never writes the outer index.
        """
        with tempfile.TemporaryDirectory() as scratch:
            outer, root = pathlib.Path(scratch) / "outer", pathlib.Path(scratch) / "inner"
            init = ["git", "init", "-q", str(outer)]
            subprocess.run(init, check=True, timeout=60, env=gate.git_env())
            hook_env = {
                "GIT_DIR": str(outer / ".git"),
                "GIT_INDEX_FILE": str(outer / ".git" / "index"),
            }
            with unittest.mock.patch.dict(os.environ, hook_env):
                root.mkdir()
                self.make_repo(root)
                report = gate.scan_tree(root, rules())
            listed = subprocess.run(
                ["git", "-C", str(outer), "ls-files"],
                capture_output=True,
                text=True,
                check=True,
                timeout=60,
                env=gate.git_env(),
            ).stdout
        self.assertEqual(len(report), 1, report)
        self.assertEqual(listed, "")


if __name__ == "__main__":
    unittest.main()
