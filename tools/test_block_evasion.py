"""Positive, negative and boundary checks for the registered anti-evasion hook.

Payload shapes mirror the per-client documentation cited in
.config/agent/hooks/block_evasion.py; no agent client is executed here.
"""

import json
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parent.parent
HOOK = ROOT / ".config/agent/hooks/block_evasion.py"
BLOCK_EXIT = 2
NO_VERIFY = "git commit --no-verify -m wip"
LEFTHOOK_PUSH = "LEFTHOOK=0 git push origin main"
ALLOWED = "git status --short"

CLIENT_PAYLOADS = {
    "claude-code": lambda cmd: {
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": cmd},
    },
    "codex-cli": lambda cmd: {
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": cmd},
    },
    "gemini-cli": lambda cmd: {
        "hook_event_name": "BeforeTool",
        "tool_name": "run_shell_command",
        "tool_input": {"command": cmd, "description": "run"},
    },
    "copilot-pascal": lambda cmd: {
        "hook_event_name": "PreToolUse",
        "tool_name": "Bash",
        "tool_input": {"command": cmd},
    },
    "copilot-camel": lambda cmd: {"toolName": "bash", "toolArgs": json.dumps({"command": cmd})},
    "cursor": lambda cmd: {
        "hook_event_name": "beforeShellExecution",
        "command": cmd,
        "cwd": "/repo",
        "sandbox": False,
    },
    "windsurf": lambda cmd: {
        "agent_action_name": "pre_run_command",
        "tool_info": {"command_line": cmd, "cwd": "/repo"},
    },
}


def run_hook(payload_text, env=None):
    return subprocess.run(
        ["python3", "-B", str(HOOK)],
        input=payload_text,
        text=True,
        capture_output=True,
        cwd=str(ROOT),
        env=env,
        timeout=30,
    )


class RegisteredClientPayloadTests(unittest.TestCase):
    def test_blocks_no_verify_for_every_registered_client(self):
        for client, build in CLIENT_PAYLOADS.items():
            with self.subTest(client=client):
                result = run_hook(json.dumps(build(NO_VERIFY)))
                self.assertEqual(result.returncode, BLOCK_EXIT)
                self.assertIn("BLOCKED BY HISS-16", result.stderr)
                self.assertEqual(result.stdout, "")

    def test_blocks_lefthook_disable_for_every_registered_client(self):
        for client, build in CLIENT_PAYLOADS.items():
            with self.subTest(client=client):
                result = run_hook(json.dumps(build(LEFTHOOK_PUSH)))
                self.assertEqual(result.returncode, BLOCK_EXIT)

    def test_allows_read_only_command_for_every_registered_client(self):
        for client, build in CLIENT_PAYLOADS.items():
            with self.subTest(client=client):
                result = run_hook(json.dumps(build(ALLOWED)))
                self.assertEqual(result.returncode, 0)
                self.assertEqual(result.stdout, "")
                self.assertEqual(result.stderr, "")

    def test_registration_files_reference_the_hook(self):
        registrations = {
            ".claude/settings.json": ("hooks", "PreToolUse"),
            ".codex/hooks.json": ("hooks", "PreToolUse"),
            ".gemini/settings.json": ("hooks", "BeforeTool"),
            ".cursor/hooks.json": ("hooks", "beforeShellExecution"),
            ".github/hooks/hiss-16-block-evasion.json": ("hooks", "PreToolUse"),
            ".windsurf/hooks.json": ("hooks", "pre_run_command"),
        }
        for name, (root_key, event) in registrations.items():
            with self.subTest(registration=name):
                path = ROOT / name
                self.assertTrue(path.is_file(), name)
                data = json.loads(path.read_text())
                self.assertIn(event, data[root_key])
                self.assertIn("block_evasion.py", json.dumps(data[root_key][event]))


class BoundaryTests(unittest.TestCase):
    def test_empty_payload_is_allowed(self):
        self.assertEqual(run_hook("").returncode, 0)

    def test_whitespace_payload_is_allowed(self):
        self.assertEqual(run_hook("   \n\t ").returncode, 0)

    def test_unmapped_payload_shape_is_allowed(self):
        payload = json.dumps({"tool_name": "Read", "tool_input": {"file_path": "/etc/hosts"}})
        self.assertEqual(run_hook(payload).returncode, 0)

    def test_malformed_payload_is_still_pattern_scanned(self):
        result = run_hook('{"tool_input": {"command": "git commit --no-verify"')
        self.assertEqual(result.returncode, BLOCK_EXIT)

    def test_non_object_json_is_still_pattern_scanned(self):
        self.assertEqual(run_hook('["git commit --no-verify"]').returncode, BLOCK_EXIT)

    def test_argv_array_command_is_flattened(self):
        payload = json.dumps({"tool_input": {"command": ["bash", "-lc", NO_VERIFY]}})
        self.assertEqual(run_hook(payload).returncode, BLOCK_EXIT)

    def test_lefthook_disabled_in_environment_blocks_any_command(self):
        import os

        env = dict(os.environ, LEFTHOOK="0")
        payload = json.dumps(CLIENT_PAYLOADS["claude-code"](ALLOWED))
        self.assertEqual(run_hook(payload, env=env).returncode, BLOCK_EXIT)


if __name__ == "__main__":
    unittest.main()
