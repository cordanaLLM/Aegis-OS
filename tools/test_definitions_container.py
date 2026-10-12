"""Regressions for the pinned definition-gate container: pin, selection, command line."""

import contextlib
import io
import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import definitions_container as container
import gate_policy
import verify_mkosi_definitions
import verify_systemd_definitions


def capture(function, *args, **kwargs):
    """Return (result, stdout) of `function` called with `args` and `kwargs`."""
    out = io.StringIO()
    with contextlib.redirect_stdout(out):
        result = function(*args, **kwargs)
    return result, out.getvalue()


class PinTests(unittest.TestCase):
    def test_the_recorded_pin_loads(self):
        """Positive: the committed pin has its keys, a digest and the two gate tools."""
        pin, raw = container.load_pin()
        self.assertIn("@sha256:", pin["image"]["reference"])
        self.assertIn("systemd", pin["packages"])
        self.assertIn("mkosi", pin["packages"])
        self.assertTrue(raw)

    def test_the_pinned_tools_meet_the_gate_floors(self):
        """Boundary: the image may not hold a tool the gate would refuse."""
        pin, _ = container.load_pin()
        systemd = int(pin["packages"]["systemd"].split("-")[0].split(".")[0])
        mkosi = int(pin["packages"]["mkosi"].split("-")[0].split(".")[0])
        self.assertGreaterEqual(systemd, verify_systemd_definitions.SYSTEMD_FLOOR)
        self.assertGreaterEqual(mkosi, verify_mkosi_definitions.MKOSI_FLOOR)

    def test_the_archive_server_is_the_recorded_snapshot(self):
        """Boundary: the mirror line names the same snapshot date as the pin."""
        pin, _ = container.load_pin()
        self.assertIn(f"/repos/{pin['archive']['snapshot']}/", pin["archive"]["server"])

    def test_an_unexpected_key_is_refused(self):
        """Negative: a pin with a key outside the schema is not read."""
        pin, _ = container.load_pin()
        pin["extra"] = 1
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "pin.json"
            path.write_text(json.dumps(pin))
            with self.assertRaises(container.FetchError):
                container.load_pin(path)

    def test_a_repeated_package_is_refused(self):
        """Negative: a second systemd line cannot silently replace the first."""
        _, raw = container.load_pin()
        text = raw.decode().replace(
            '"systemd": "262-1",', '"systemd": "262-1",\n    "systemd": "255-1",'
        )
        with tempfile.TemporaryDirectory() as base:
            path = Path(base) / "pin.json"
            path.write_text(text)
            with self.assertRaises(container.FetchError):
                container.load_pin(path)

    def test_the_tag_follows_the_pin_bytes(self):
        """Positive and boundary: same bytes, same tag; one byte changed, another tag."""
        tag = container.image_tag(b"{}")
        self.assertEqual(tag, container.image_tag(b"{}"))
        self.assertNotEqual(tag, container.image_tag(b"{ }"))
        self.assertTrue(tag.startswith(f"{container.IMAGE_NAME}:"))
        self.assertEqual(len(tag.split(":")[1]), container.TAG_LENGTH)


class AdmissionPageTests(unittest.TestCase):
    HEADING = "## The definition gates' container (M03, M18)"

    def _rows(self):
        page = (container.ROOT / "docs" / "roadmap" / "toolchain-admission.md").read_text()
        section = page.split(self.HEADING, 1)[1].split("\n## ", 1)[0]
        rows = {}
        for line in section.splitlines():
            cells = [cell.strip() for cell in line.strip().strip("|").split("|")]
            if len(cells) == 3 and cells[0] not in {"Input", ":---"}:
                rows[cells[0].strip("`")] = cells[1]
        return rows

    def test_every_pinned_package_is_admitted_at_its_version(self):
        """Positive: the page and the pin name the same packages and versions."""
        pin, _ = container.load_pin()
        rows = self._rows()
        packages = {name: rows[name].strip("`") for name in rows if name in pin["packages"]}
        self.assertEqual(packages, pin["packages"])

    def test_the_page_names_no_package_the_pin_lacks(self):
        """Negative: a row for a package the image does not install is a stale row."""
        pin, _ = container.load_pin()
        extra = (
            set(self._rows()) - set(pin["packages"]) - {"base image", "Arch Linux Archive snapshot"}
        )
        self.assertEqual(extra, set())

    def test_the_image_and_snapshot_match_the_pin(self):
        """Boundary: the digest and the snapshot date are the pin's, byte for byte."""
        pin, _ = container.load_pin()
        rows = self._rows()
        self.assertIn(f"`{pin['image']['reference']}`", rows["base image"])
        for digest in pin["image"]["platform-digests"].values():
            self.assertIn(f"`{digest}`", rows["base image"])
        self.assertEqual(rows["Arch Linux Archive snapshot"], f"`{pin['archive']['snapshot']}`")


class ContainerfileTests(unittest.TestCase):
    def test_every_pinned_input_reaches_the_build(self):
        """Positive: the base by digest, the snapshot server and every package."""
        pin, _ = container.load_pin()
        text = container.containerfile(pin)
        self.assertTrue(text.startswith(f"FROM {pin['image']['reference']}\n"))
        self.assertIn(f"'{pin['archive']['server']}'", text)
        for name in pin["packages"]:
            self.assertIn(f" {name}", text)

    def test_no_other_mirror_can_serve_a_package(self):
        """Negative: the mirror list is replaced, not appended to."""
        pin, _ = container.load_pin()
        self.assertIn("> /etc/pacman.d/mirrorlist", container.containerfile(pin))
        self.assertNotIn(">> /etc/pacman.d/mirrorlist", container.containerfile(pin))


class VersionTests(unittest.TestCase):
    PINNED = {"systemd": "262-1", "mkosi": "27.1-1"}

    def test_exact_versions_pass(self):
        """Positive: pacman reports every package at its pinned version."""
        self.assertEqual(
            container.version_problems(self.PINNED, "mkosi 27.1-1\nsystemd 262-1\n"), []
        )

    def test_a_different_version_is_reported(self):
        """Negative: a newer package is a drift, not an upgrade."""
        problems = container.version_problems(self.PINNED, "systemd 262-2\nmkosi 27.1-1\n")
        self.assertEqual(problems, ["systemd: pinned 262-1, image has 262-2"])

    def test_a_similar_name_does_not_stand_in(self):
        """Boundary: systemd-libs at the right version is not systemd."""
        problems = container.version_problems(self.PINNED, "systemd-libs 262-1\nmkosi 27.1-1\n")
        self.assertEqual(problems, ["systemd: pinned 262-1, image has nothing"])


class SelectionTests(unittest.TestCase):
    def test_one_selects_the_container(self):
        """Positive: the documented value."""
        self.assertEqual(container.selected({container.VARIABLE: "1"}), (True, None))

    def test_unset_and_empty_select_the_host(self):
        """Boundary: an empty value is the same as an unset one."""
        self.assertEqual(container.selected({}), (False, None))
        self.assertEqual(container.selected({container.VARIABLE: ""}), (False, None))

    def test_any_other_value_is_refused(self):
        """Negative: neither yes nor 0 nor padded 1 is guessed at."""
        for value in ("yes", "0", " 1"):
            chosen, problem = container.selected({container.VARIABLE: value})
            self.assertIsNone(chosen)
            self.assertIn(repr(value), problem)


class GateArgvTests(unittest.TestCase):
    def test_the_gate_runs_offline_and_read_only(self):
        """Positive: no pull, no network, read-only root and repository."""
        argv = container.gate_argv("podman", "localhost/x:1", "verify_mkosi_definitions.py", {})
        for flag in ("--pull=never", "--network=none", "--read-only"):
            self.assertIn(flag, argv)
        self.assertIn(f"type=bind,source={container.ROOT},target=/repo,readonly", argv)
        self.assertEqual(
            argv[-4:], ["localhost/x:1", "python3", "-B", "tools/verify_mkosi_definitions.py"]
        )

    def test_the_inner_run_does_not_select_the_container_again(self):
        """Negative: inside the image the gate runs on the image's tools."""
        environ = {container.VARIABLE: "1", gate_policy.VARIABLE: "systemd,mkosi"}
        argv = container.gate_argv("podman", "localhost/x:1", "s.py", environ)
        self.assertIn(f"{container.VARIABLE}=", argv)
        self.assertIn(f"{gate_policy.VARIABLE}=systemd,mkosi", argv)

    def test_docker_runs_as_the_invoking_user(self):
        """Boundary: docker gets --user, podman (rootless) does not."""
        with patch.object(container, "uid", return_value=(1001, None)), patch.object(
            container, "gid", return_value=(127, None)
        ):
            docker = container.gate_argv("docker", "t", "s.py", {})
            podman = container.gate_argv("podman", "t", "s.py", {})
        self.assertIn("1001:127", docker)
        self.assertNotIn("--user", podman)


class DispatchTests(unittest.TestCase):
    def _dispatch(self, environ, present):
        with patch.object(container, "pick_engine", return_value=("podman", None)), patch.object(
            container, "image_present", return_value=present
        ), patch.object(container, "run", return_value=(0, "", "")) as run:
            code, printed = capture(
                container.dispatch, "mkosi", "verify_mkosi_definitions.py", environ
            )
        return code, printed, run

    def test_a_host_run_is_left_alone(self):
        """Positive: without the variable the gate runs on the host as before."""
        code, printed, run = self._dispatch({}, True)
        self.assertIsNone(code)
        self.assertEqual(printed, "")
        run.assert_not_called()

    def test_a_selected_container_without_its_image_fails(self):
        """Negative: the planted CI defect, the container asked for and never fetched."""
        code, printed, run = self._dispatch({container.VARIABLE: "1"}, False)
        self.assertEqual(code, 1)
        self.assertIn("make definitions-fetch", printed)
        self.assertTrue(printed.startswith("FAIL"))
        run.assert_not_called()

    def test_a_present_image_runs_the_gate_and_names_it(self):
        """Positive: the run names the image, the engine and the snapshot it used."""
        code, printed, run = self._dispatch({container.VARIABLE: "1"}, True)
        pin, raw = container.load_pin()
        self.assertEqual(code, 0)
        self.assertIn(container.image_tag(raw), printed)
        self.assertIn(pin["archive"]["snapshot"], printed)
        self.assertEqual(run.call_args.args[0][:2], ["podman", "run"])

    def test_a_malformed_selection_fails_before_any_engine_call(self):
        """Boundary: a refused value never reaches podman."""
        code, printed, run = self._dispatch({container.VARIABLE: "true"}, True)
        self.assertEqual(code, 1)
        self.assertIn("'true'", printed)
        run.assert_not_called()


class GateRoutingTests(unittest.TestCase):
    def test_each_gate_hands_a_selected_run_to_the_container(self):
        """Positive: main returns the container's exit code and never runs the guard."""
        for gate in (verify_systemd_definitions, verify_mkosi_definitions):
            with patch.dict(gate.os.environ, {gate_policy.VARIABLE: ""}), patch.object(
                gate.definitions_container, "dispatch", return_value=7
            ) as dispatch, patch.object(gate, "guard") as guard, patch.object(
                gate.sys, "argv", ["gate"]
            ):
                self.assertEqual(gate.main(), 7)
            guard.assert_not_called()
            self.assertEqual(dispatch.call_args.args[0], gate.GATE)


if __name__ == "__main__":
    unittest.main()
