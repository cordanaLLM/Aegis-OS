"""Regressions for handing a pull request's definition image to the run on main."""

import contextlib
import io
import json
import tempfile
import unittest
import zipfile
from pathlib import Path
from unittest.mock import patch

import definitions_container as container
import definitions_handoff as handoff

REPOSITORY = "cordanaLLM/Aegis-OS"
REPOSITORY_ID = 1368312619
NAME = "definitions-floor-4aeface2"


def artifact(number, created, head=REPOSITORY_ID, expired=False, name=NAME):
    """One entry of the artifact listing, in the shape the REST API returns."""
    return {
        "id": number,
        "name": name,
        "expired": expired,
        "created_at": created,
        "workflow_run": {
            "id": number * 10,
            "repository_id": REPOSITORY_ID,
            "head_repository_id": head,
        },
    }


class HandoffTests(unittest.TestCase):
    def setUp(self):
        _, raw = container.load_pin(container.FLOOR_PIN)
        self.tag = container.image_tag(raw)
        scratch = tempfile.TemporaryDirectory()
        self.addCleanup(scratch.cleanup)
        self.cache = Path(scratch.name) / "floor"
        self.environ = {
            container.CACHE_VARIABLE: str(self.cache),
            "GITHUB_REPOSITORY": REPOSITORY,
            "GITHUB_REPOSITORY_ID": str(REPOSITORY_ID),
        }
        self.archive = container.cache_archive(self.tag, self.environ)
        self.listing = [artifact(1, "2026-10-17T11:22:36Z")]
        self.members = {self.archive.name: b"image"}
        self.downloads = []

    def forge(self, argv, timeout, stdout=None):
        """Stand in for gh api: the listing as JSON, a download as a zip of `members`."""
        if argv[0].endswith("/zip"):
            self.downloads.append(argv[0])
            with zipfile.ZipFile(stdout, "w") as bundle:
                for member, data in self.members.items():
                    bundle.writestr(member, data)
            return 0, "", ""
        return 0, json.dumps({"total_count": len(self.listing), "artifacts": self.listing}), ""

    def run_handoff(self, environ=None):
        return handoff.handoff(NAME, container.FLOOR_PIN, environ or self.environ, self.forge)

    def test_a_missed_cache_is_filled_from_the_artifact(self):
        """Positive: the archive the fetch reads appears, byte for byte."""
        line = self.run_handoff()
        self.assertEqual(line, f"PASS: {self.archive} handed off from artifact 1 of run 10.")
        self.assertEqual(self.archive.read_bytes(), b"image")

    def test_the_newest_artifact_is_picked(self):
        """Positive: of two builds of one pin, the later one."""
        self.listing = [artifact(1, "2026-10-17T11:22:36Z"), artifact(2, "2026-10-17T12:00:00Z")]
        self.assertIn("artifact 2 of run 20", self.run_handoff())
        self.assertEqual(len(self.downloads), 1)
        self.assertTrue(self.downloads[0].endswith("/artifacts/2/zip"))

    def test_a_fork_or_expired_artifact_is_never_used(self):
        """Negative: an image from a fork's run, or one past retention, is skipped."""
        self.listing = [
            artifact(1, "2026-10-17T12:00:00Z", head=99),
            artifact(2, "2026-10-17T12:00:00Z", expired=True),
            artifact(3, "2026-10-17T12:00:00Z", name="definitions-other"),
        ]
        self.assertEqual(
            self.run_handoff(), f"SKIP: no unexpired artifact {NAME} from this repository."
        )
        self.assertEqual((self.downloads, self.archive.exists()), ([], False))

    def test_an_artifact_without_the_pin_archive_writes_nothing(self):
        """Negative: only the member named after the pin is extracted, nothing else."""
        self.members = {"../escape.tar": b"x", "aegis-definitions-other.tar": b"x"}
        self.assertIn(f"the artifact holds no {self.archive.name}", self.run_handoff())
        self.assertFalse(self.archive.exists())
        self.assertFalse((self.cache.parent / "escape.tar").exists())

    def test_a_failed_listing_or_download_skips(self):
        """Negative: a forge error is a skip with its reason, never a crash."""

        def failing(argv, timeout, stdout=None):
            return 1, "", "HTTP 502"

        line = handoff.handoff(NAME, container.FLOOR_PIN, self.environ, failing)
        self.assertEqual(line, "SKIP: the artifact listing exited 1 HTTP 502")

        def broken(argv, timeout, stdout=None):
            if argv[0].endswith("/zip"):
                stdout.write(b"not a zip")
                return 0, "", ""
            return self.forge(argv, timeout)

        line = handoff.handoff(NAME, container.FLOOR_PIN, self.environ, broken)
        self.assertIn("the artifact did not unpack", line)
        self.assertFalse(self.archive.exists())

    def test_boundaries_leave_the_forge_alone(self):
        """Boundary: no cache directory, an archive already restored, no workflow context."""
        unset = dict(self.environ, **{container.CACHE_VARIABLE: ""})
        self.assertIn(f"{container.CACHE_VARIABLE} is not set", self.run_handoff(unset))
        outside = {container.CACHE_VARIABLE: str(self.cache)}
        self.assertIn("not in a workflow run", self.run_handoff(outside))
        self.archive.parent.mkdir(parents=True)
        self.archive.write_bytes(b"restored")
        self.assertIn("is already present", self.run_handoff())
        self.assertEqual((self.downloads, self.archive.read_bytes()), ([], b"restored"))

    def test_a_planted_double_outage_completes_from_the_handed_off_image(self):
        """Planted: registry and archive server unreachable, cache missed; the fetch passes."""
        self.run_handoff()
        stall = container.FetchError("docker pull exited 1 connect: connection timed out")
        loaded = []

        def engine(argv, timeout, capture=True):
            loaded.append(argv[1])
            return 0, "", ""

        out = io.StringIO()
        with patch.object(container, "build", side_effect=stall), patch.object(
            container, "pick_engine", return_value=("docker", None)
        ), patch.object(container, "run", side_effect=engine), patch.object(
            container, "check_versions"
        ) as checked, contextlib.redirect_stdout(
            out
        ):
            code = container.fetch(container.FLOOR_PIN, self.environ, sleep=lambda _: None)
        self.assertEqual(code, 0, out.getvalue())
        self.assertIn(f"loaded from {self.archive} after the archive server failed", out.getvalue())
        self.assertEqual(loaded, ["load", "image"])
        checked.assert_called_once()

    def test_without_the_handoff_the_double_outage_fails(self):
        """Negative twin of the planted case: no artifact, so both sources fail by name."""
        self.listing = []
        self.assertIn("SKIP: no unexpired artifact", self.run_handoff())
        stall = container.FetchError("docker pull exited 1 connect: connection timed out")
        out = io.StringIO()
        with patch.object(container, "build", side_effect=stall), patch.object(
            container, "pick_engine", return_value=("docker", None)
        ), contextlib.redirect_stdout(out):
            code = container.fetch(container.FLOOR_PIN, self.environ, sleep=lambda _: None)
        self.assertEqual(code, 1)
        self.assertIn("both sources failed", out.getvalue())


if __name__ == "__main__":
    unittest.main()
