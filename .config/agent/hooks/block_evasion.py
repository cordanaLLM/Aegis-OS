#!/usr/bin/env python3
"""Agent pre-tool-use evasion interceptor (HISS-16).

Registered as a committed pre-tool-use hook in every agent client that supports
one (AGENTS.md rule 5). The client passes the pending tool call as JSON on stdin;
argv is also accepted for manual testing. Exit code 2 blocks the call and every
registered client reports the stderr reason back to the agent.

Payload field verified against each vendor's current documentation on 2026-09-13:

  client        registration file        event / matcher                field
  Claude Code   .claude/settings.json    PreToolUse / Bash              tool_input.command
  Codex CLI     .codex/hooks.json        PreToolUse / ^Bash$            tool_input.command
  Gemini CLI    .gemini/settings.json    BeforeTool / run_shell_command tool_input.command
  Copilot       .github/hooks/*.json     PreToolUse / Bash              tool_input.command
  Copilot       (camelCase preToolUse)   preToolUse                     toolArgs.command
  Cursor        .cursor/hooks.json       beforeShellExecution           command
  Windsurf      .windsurf/hooks.json     pre_run_command                tool_info.command_line

Boundary behaviour: an empty or non-JSON payload carries no command to inspect
and cannot be produced by the model, so it is allowed (exit 0) rather than
bricking every tool call; text that fails JSON parsing is still pattern-scanned
verbatim, so an evasive command survives a payload shape this script cannot map.

A git hook cannot observe --no-verify because git skips hooks entirely, so this
script is deliberately not part of lefthook.yml.
"""

import json
import os
import re
import sys

BLOCKED_PATTERNS = [
    r"--no-verify\b",
    r"\bgit\s+commit\b.*\s-n\b",
    r"LEFTHOOK=0\b",
    r"SKIP=.*git",
    r"core\.hooksPath\s*=\s*/dev/null",
    r"rm\s+(-rf?\s+)?\.git/hooks",
]

# (container field, command field); an empty container field means the top level.
COMMAND_FIELDS = (
    ("tool_input", "command"),
    ("toolArgs", "command"),
    ("tool_info", "command_line"),
    ("", "command"),
    ("", "command_line"),
)

BLOCK_EXIT = 2
MAX_PAYLOAD_BYTES = 1 << 20
MAX_COMMAND_CHARS = 1 << 16
MAX_ARGV_PARTS = 64


def as_text(value):
    """Flatten one command value; an argv array joins into a single line."""
    if isinstance(value, str):
        return value
    if isinstance(value, (list, tuple)):
        return " ".join(part for part in value[:MAX_ARGV_PARTS] if isinstance(part, str))
    return ""


def as_mapping(value):
    """Return a mapping for a container that may arrive JSON-encoded as a string."""
    if isinstance(value, dict):
        return value
    if not isinstance(value, str) or not value.startswith("{"):
        return {}
    try:
        decoded = json.loads(value[:MAX_PAYLOAD_BYTES])
    except ValueError:
        return {}
    return decoded if isinstance(decoded, dict) else {}


def command_from_payload(payload):
    """Return the shell command a registered client placed in its hook payload."""
    for container, field in COMMAND_FIELDS:
        source = payload if container == "" else as_mapping(payload.get(container))
        text = as_text(source.get(field))
        if text:
            return text
    return ""


def pending_command():
    """Return the command under review from argv or the JSON hook payload."""
    if len(sys.argv) > 1:
        return " ".join(sys.argv[1:])[:MAX_COMMAND_CHARS]
    if sys.stdin.isatty():
        return ""
    raw = sys.stdin.read(MAX_PAYLOAD_BYTES)
    if not raw.strip():
        return ""
    try:
        payload = json.loads(raw)
    except ValueError:
        return raw[:MAX_COMMAND_CHARS]
    if not isinstance(payload, dict):
        return raw[:MAX_COMMAND_CHARS]
    return command_from_payload(payload)[:MAX_COMMAND_CHARS]


def main():
    if os.environ.get("LEFTHOOK") == "0":
        sys.stderr.write("[BLOCKED BY HISS-16] LEFTHOOK=0 detected in environment.\n")
        sys.exit(BLOCK_EXIT)
    cmd = pending_command()
    for pattern in BLOCKED_PATTERNS:
        if re.search(pattern, cmd):
            sys.stderr.write(f"[BLOCKED BY HISS-16] Verification evasion prohibited: {pattern}\n")
            sys.exit(BLOCK_EXIT)
    sys.exit(0)


if __name__ == "__main__":
    main()
