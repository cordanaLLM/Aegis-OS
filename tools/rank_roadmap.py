#!/usr/bin/env python3
"""Recompute the roadmap order from unblocking value per cost and report drift."""

import argparse
import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
COST_WEIGHT = {"trivial": 1, "small": 2, "medium": 4, "large": 8}
# Ordering tiers, cheapest resistance first. A milestone is placed in the first
# tier whose condition it meets, and ranked inside the tier by value per cost.
TIER_LOCAL = 0
TIER_LOCAL_HARDWARE = 1
TIER_ABSENT_CAPABILITY = 2
TIER_EXTERNAL = 3
TIER_RELEASE = 4


def transitive_unblocks(rows):
    """Count what each milestone makes reachable, with a bounded sweep."""
    direct = {row["id"]: set(row.get("unblocks", [])) for row in rows}
    reach = {key: set(value) for key, value in direct.items()}
    for _ in range(len(rows)):
        changed = False
        for key, value in reach.items():
            grown = set(value)
            for child in value:
                grown |= direct.get(child, set())
            if grown != value:
                reach[key] = grown
                changed = True
        if not changed:
            break
    return {key: len(value) for key, value in reach.items()}


def tier_of(row, profile_absent):
    if row["id"] == "M13" or "release" in row.get("title", "").lower():
        return TIER_RELEASE
    if row.get("needs_external_contract"):
        return TIER_EXTERNAL
    support = (row.get("reference_profile") or {}).get("support", "not-hardware")
    requires = set((row.get("reference_profile") or {}).get("requires", []))
    if requires & profile_absent or support.startswith("partial"):
        return TIER_ABSENT_CAPABILITY
    if row.get("needs_hardware"):
        # D68: hardware work whose capability is measured on the reference
        # profile ranks with local work; only unmeasured hardware is demoted.
        return TIER_LOCAL if support.startswith("full") else TIER_LOCAL_HARDWARE
    return TIER_LOCAL


def score(row, reach):
    """Unblocking value per unit cost; higher sorts earlier."""
    return (1 + reach[row["id"]]) / COST_WEIGHT[row.get("cost", "small")]


def computed_order(rows, profile_absent):
    """Order milestones by tier then value per cost, respecting the blocker DAG."""
    reach = transitive_unblocks(rows)
    done = [row for row in rows if row["state"] == "done"]
    remaining = [row for row in rows if row["state"] != "done"]
    order = sorted(done, key=lambda row: row["rank"])
    placed = {row["id"] for row in order}
    for _ in range(len(remaining)):
        ready = [
            row
            for row in remaining
            if row["id"] not in placed and set(row.get("blocked_by", [])) <= placed
        ]
        if not ready:
            break
        ready.sort(key=lambda row: (tier_of(row, profile_absent), -score(row, reach), row["id"]))
        chosen = ready[0]
        order.append(chosen)
        placed.add(chosen["id"])
    if len(order) != len(rows):
        missing = sorted({row["id"] for row in rows} - placed)
        raise ValueError(f"Milestones unreachable in the blocker graph: {missing}")
    return [row["id"] for row in order], reach


def drift(rows, profile_absent):
    """Return the committed order, the computed order and the unexplained differences.

    A milestone may sit away from its computed position only when it records
    rank_override with a reason; that keeps human judgement possible and audited.
    """
    order, reach = computed_order(rows, profile_absent)
    committed = [row["id"] for row in sorted(rows, key=lambda row: row["rank"])]
    overridden = {row["id"] for row in rows if (row.get("rank_override") or {}).get("reason")}
    diffs = [
        (index, was, now)
        for index, (was, now) in enumerate(zip(committed, order))
        if was != now and was not in overridden and now not in overridden
    ]
    return committed, order, diffs, reach


def load(path=None):
    source = path or (ROOT / "planning/roadmap.json")
    rows = json.loads(source.read_text())["milestones"]
    profile = ROOT / "planning/hardware-profile.json"
    absent = set()
    if profile.is_file():
        capabilities = json.loads(profile.read_text())["capabilities"]
        absent = {name for name, row in capabilities.items() if not row["present"]}
    return rows, absent


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--explain", action="store_true")
    args = parser.parse_args()
    rows, absent = load()
    committed, order, diffs, reach = drift(rows, absent)
    if args.explain:
        by_id = {row["id"]: row for row in rows}
        for position, key in enumerate(order):
            row = by_id[key]
            print(
                f"{position:>2} {key:<4} tier {tier_of(row, absent)} "
                f"value {1 + reach[key]:>2} cost {COST_WEIGHT[row['cost']]} "
                f"score {score(row, reach):.2f}  {row['title'][:48]}"
            )
    if diffs:
        print(
            f"Roadmap order drifts from the computed order at {len(diffs)} "
            "positions with no recorded override:"
        )
        for index, was, now in diffs[:10]:
            print(f"  rank {index}: committed {was}, computed {now}")
        return 1
    print(f"Roadmap order matches the computed order ({len(order)} milestones)")
    return 0


if __name__ == "__main__":
    sys.exit(main())
