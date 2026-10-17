"""Ranking controller regressions: value per cost, tiers, overrides and the DAG."""

import json
import tempfile
import unittest
from pathlib import Path
from unittest.mock import patch

import rank_roadmap as ranker


def milestone(mid, rank, cost="small", blocked_by=(), unblocks=(), **extra):
    row = {
        "id": mid,
        "title": f"milestone {mid}",
        "rank": rank,
        "state": "blocked",
        "cost": cost,
        "blocked_by": list(blocked_by),
        "unblocks": list(unblocks),
        "needs_hardware": False,
        "needs_external_contract": False,
    }
    row.update(extra)
    return row


class RankingTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory()
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.patch = patch.object(ranker, "ROOT", self.root)
        self.patch.start()
        self.addCleanup(self.patch.stop)
        (self.root / "planning").mkdir()

    def write(self, rows):
        (self.root / "planning/roadmap.json").write_text(json.dumps({"milestones": rows}))

    def test_cheaper_milestone_with_equal_value_ranks_first(self):
        rows = [
            milestone("M00", 0, state="done", unblocks=["M01", "M02"]),
            milestone("M01", 1, cost="trivial", blocked_by=["M00"]),
            milestone("M02", 2, cost="large", blocked_by=["M00"]),
        ]
        self.write(rows)
        loaded, absent = ranker.load()
        _, order, diffs, _ = ranker.drift(loaded, absent)
        self.assertEqual(order, ["M00", "M01", "M02"])
        self.assertEqual(diffs, [])

    def test_more_unblocking_wins_at_equal_cost(self):
        rows = [
            milestone("M00", 0, state="done", unblocks=["M01", "M02"]),
            milestone("M01", 1, blocked_by=["M00"]),
            milestone("M02", 2, blocked_by=["M00"], unblocks=["M03"]),
            milestone("M03", 3, blocked_by=["M02"]),
        ]
        self.write(rows)
        loaded, absent = ranker.load()
        _, order, _, reach = ranker.drift(loaded, absent)
        self.assertEqual(order[1], "M02")
        self.assertEqual(reach["M02"], 1)

    def test_an_outside_node_waiting_on_a_milestone_moves_it_up(self):
        """Planted: one external_unblocks entry turns the order of two equal milestones."""
        rows = [
            milestone("M00", 0, state="done", unblocks=["M01", "M02"]),
            milestone("M01", 1, blocked_by=["M00"]),
            milestone("M02", 2, blocked_by=["M00"]),
        ]
        self.write(rows)
        loaded, absent = ranker.load()
        _, order, diffs, _ = ranker.drift(loaded, absent)
        self.assertEqual((order, diffs), (["M00", "M01", "M02"], []))
        rows[2]["external_unblocks"] = [{"node": "K4", "source": "an issue"}]
        self.write(rows)
        loaded, absent = ranker.load()
        _, order, diffs, reach = ranker.drift(loaded, absent)
        self.assertEqual(order, ["M00", "M02", "M01"])
        self.assertEqual(reach["M02"], 1)
        self.assertTrue(diffs)

    def test_an_outside_node_counts_once_through_every_path(self):
        """Boundary: reached directly and through a child, a node is one unit of reach."""
        outside = [{"node": "K4", "source": "an issue"}]
        rows = [
            milestone("M01", 0, unblocks=["M02"], external_unblocks=outside),
            milestone("M02", 1, blocked_by=["M01"], external_unblocks=outside),
        ]
        reach = ranker.transitive_unblocks(rows)
        self.assertEqual(reach, {"M01": 2, "M02": 1})

    def test_an_outside_node_named_like_a_milestone_stays_apart(self):
        """Negative: an outside node never merges with the milestone of the same name."""
        rows = [
            milestone(
                "M01", 0, unblocks=["M02"], external_unblocks=[{"node": "M02", "source": "x"}]
            ),
            milestone("M02", 1, blocked_by=["M01"]),
        ]
        self.assertEqual(ranker.transitive_unblocks(rows)["M01"], 2)

    def test_tiers_order_local_before_hardware_external_and_release(self):
        rows = [
            milestone("M00", 0, state="done", unblocks=["M01", "M02", "M03", "M04"]),
            milestone("M04", 1, blocked_by=["M00"], title="Release signing and delivery"),
            milestone("M03", 2, blocked_by=["M00"], needs_external_contract=True),
            milestone("M02", 3, blocked_by=["M00"], needs_hardware=True),
            milestone("M01", 4, blocked_by=["M00"]),
        ]
        self.write(rows)
        loaded, absent = ranker.load()
        _, order, diffs, _ = ranker.drift(loaded, absent)
        self.assertEqual(order, ["M00", "M01", "M02", "M03", "M04"])
        self.assertEqual(len(diffs), 4)

    def test_a_recorded_override_is_accepted_and_an_unrecorded_one_is_not(self):
        rows = [
            milestone("M00", 0, state="done", unblocks=["M01", "M02"]),
            milestone("M01", 1, cost="large", blocked_by=["M00"]),
            milestone("M02", 2, cost="trivial", blocked_by=["M00"]),
        ]
        self.write(rows)
        loaded, absent = ranker.load()
        _, _, diffs, _ = ranker.drift(loaded, absent)
        self.assertTrue(diffs)
        rows[1]["rank_override"] = {"reason": "the maintainer wants the large item first"}
        self.write(rows)
        loaded, absent = ranker.load()
        _, _, diffs, _ = ranker.drift(loaded, absent)
        self.assertEqual(diffs, [])
        rows[1]["rank_override"] = {"reason": ""}
        self.write(rows)
        loaded, absent = ranker.load()
        _, _, diffs, _ = ranker.drift(loaded, absent)
        self.assertTrue(diffs)

    def test_a_blocker_cycle_is_reported_rather_than_ordered(self):
        rows = [
            milestone("M01", 0, blocked_by=["M02"]),
            milestone("M02", 1, blocked_by=["M01"]),
        ]
        self.write(rows)
        loaded, absent = ranker.load()
        with self.assertRaises(ValueError):
            ranker.computed_order(loaded, absent)

    def test_an_unknown_blocker_is_reported_rather_than_ordered(self):
        """Negative: a blocker id that names no milestone is refused, not placed."""
        rows = [
            milestone("M00", 0, state="done", unblocks=["M01"]),
            milestone("M01", 1, blocked_by=["M00", "M99"]),
        ]
        self.write(rows)
        loaded, absent = ranker.load()
        with self.assertRaises(ValueError):
            ranker.computed_order(loaded, absent)

    def test_a_lone_milestone_without_blockers_is_ordered(self):
        """Boundary: one open milestone and no done rows orders to itself."""
        self.write([milestone("M01", 0)])
        loaded, absent = ranker.load()
        self.assertEqual(ranker.computed_order(loaded, absent)[0], ["M01"])

    def test_absent_capability_demotes_a_milestone(self):
        (self.root / "planning/hardware-profile.json").write_text(
            json.dumps(
                {"capabilities": {"kvm": {"present": True}, "realtime_kernel": {"present": False}}}
            )
        )
        rows = [
            milestone("M00", 0, state="done", unblocks=["M01", "M02"]),
            milestone(
                "M01",
                1,
                blocked_by=["M00"],
                needs_hardware=True,
                reference_profile={"support": "full", "requires": ["kvm"]},
            ),
            milestone(
                "M02",
                2,
                blocked_by=["M00"],
                needs_hardware=True,
                reference_profile={"support": "partial", "requires": ["realtime_kernel"]},
            ),
        ]
        self.write(rows)
        loaded, absent = ranker.load()
        self.assertEqual(absent, {"realtime_kernel"})
        _, order, diffs, _ = ranker.drift(loaded, absent)
        self.assertEqual(order, ["M00", "M01", "M02"])
        self.assertEqual(diffs, [])


if __name__ == "__main__":
    unittest.main()
