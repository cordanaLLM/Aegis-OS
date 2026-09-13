#!/usr/bin/env python3
"""Print the blocking state of one roadmap milestone; used by the CI gates."""

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
MAX_BYTES = 1 << 20


def milestone_state(milestone_id, path=None):
    """Return the recorded state of a milestone, or raise ValueError."""
    source = path or (ROOT / "planning/roadmap.json")
    if source.is_symlink() or not source.is_file() or source.stat().st_size > MAX_BYTES:
        raise ValueError(f"Invalid or oversized roadmap: {source.name}")
    rows = json.loads(source.read_text())["milestones"]
    for row in rows:
        if row["id"] == milestone_id:
            return row["state"]
    raise ValueError(f"Unknown milestone: {milestone_id}")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("milestone")
    args = parser.parse_args()
    try:
        print(milestone_state(args.milestone))
    except (ValueError, KeyError) as error:
        print(error, file=sys.stderr)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main())
