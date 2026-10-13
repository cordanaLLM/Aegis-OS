// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The accessibility tree's id scheme: `level_id` and `decision_id`.
//!
//! The scheme, as the source documents it: bits 40 and up carry a tag (1 =
//! canvas node, 2 = instrument level, 3 = pending decision); a level id adds
//! `key * 8 + (depth mod 8)` below the tag, a decision id adds the index.
//! The tags are `1 << 40 = 1_099_511_627_776`, `2 << 40 = 2_199_023_255_552`
//! and `3 << 40 = 3_298_534_883_328`; every expected id below is that sum
//! worked by hand.
//!
//! Positive: a level id and a decision id land in their tag's range at the
//! hand-computed value, and `describe` reads them back. Negative: the three
//! id ranges never meet, and ids of different keys, depths or indexes differ.
//! Boundary: depth 7 against depth 8 (the slot wraps), key 0 and `u32::MAX`,
//! depth `usize::MAX`, decision index 0 and a large one.

use accesskit::NodeId;
use aegis_forum_shell::a11y::{decision_id, describe, level_id, node_id};
use aegis_forum_shell::canvas::registry::NodeKey;

/// `2 << 40`: the instrument-level tag.
const LEVEL_BASE: u64 = 2_199_023_255_552;

/// `3 << 40`: the pending-decision tag.
const DECISION_BASE: u64 = 3_298_534_883_328;

// --- Positive -------------------------------------------------------------

/// Positive: level 3 of node 5 is the level tag plus `5 * 8 + 3 = 43`.
#[test]
fn a_level_id_is_the_tag_plus_key_times_eight_plus_depth() {
    assert_eq!(level_id(NodeKey(5), 3), NodeId(2_199_023_255_595));
    assert_eq!(level_id(NodeKey(1), 0), NodeId(2_199_023_255_560));
    assert_eq!(level_id(NodeKey(0), 2), NodeId(2_199_023_255_554));
}

/// Positive: decision 3 is the decision tag plus 3, and `describe` names both
/// ids by what they stand for.
#[test]
fn a_decision_id_is_the_tag_plus_the_index() {
    assert_eq!(decision_id(3), NodeId(3_298_534_883_331));
    assert_eq!(decision_id(1), NodeId(3_298_534_883_329));
    assert_eq!(
        describe(&decision_id(3)),
        "pending decision 3 (#3298534883331)"
    );
    assert_eq!(
        describe(&level_id(NodeKey(5), 3)),
        "instrument level 3 of canvas node 5 (#2199023255595)"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: no two of the node, level and decision ranges produce the same
/// id, and ids that differ in key, depth or index differ.
#[test]
fn the_id_ranges_never_meet() {
    let node = node_id(NodeKey(0));
    assert_ne!(node, level_id(NodeKey(0), 0));
    assert_ne!(node, decision_id(0));
    assert_ne!(level_id(NodeKey(0), 0), decision_id(0));
    assert_ne!(level_id(NodeKey(0), 1), level_id(NodeKey(0), 2));
    assert_ne!(level_id(NodeKey(1), 0), level_id(NodeKey(2), 0));
    assert_ne!(level_id(NodeKey(1), 7), level_id(NodeKey(2), 0));
    assert_ne!(decision_id(0), decision_id(1));
}

// --- Boundary -------------------------------------------------------------

/// Boundary: depth 7 is the last slot; depth 8 wraps to slot 0 of the same
/// key, so it equals depth 0 (the instrument is at most four levels deep, so
/// the wrap is never reached by a real node).
#[test]
fn a_level_slot_wraps_at_eight() {
    assert_eq!(
        level_id(NodeKey(0), 7),
        NodeId(LEVEL_BASE.saturating_add(7))
    );
    assert_eq!(level_id(NodeKey(0), 8), NodeId(LEVEL_BASE));
    assert_eq!(
        level_id(NodeKey(0), 9),
        NodeId(LEVEL_BASE.saturating_add(1))
    );
    assert_eq!(level_id(NodeKey(3), 8), level_id(NodeKey(3), 0));
}

/// Boundary: key 0 contributes nothing; the largest key contributes
/// `4_294_967_295 * 8 = 34_359_738_360`, which still stays below the next tag;
/// the largest depth is slot `usize::MAX mod 8 = 7`.
#[test]
fn a_level_id_holds_at_the_extreme_keys_and_depths() {
    assert_eq!(level_id(NodeKey(0), 0), NodeId(LEVEL_BASE));
    assert_eq!(level_id(NodeKey(u32::MAX), 0), NodeId(2_233_382_993_912));
    assert_eq!(level_id(NodeKey(0), usize::MAX), NodeId(2_199_023_255_559));
    assert_eq!(
        describe(&level_id(NodeKey(u32::MAX), 5)),
        "instrument level 5 of canvas node 4294967295 (#2233382993917)"
    );
}

/// Boundary: index 0 is the bare tag; index `1 << 39` is the tag plus
/// `549_755_813_888`, still inside the decision range.
#[test]
fn a_decision_id_holds_at_index_zero_and_a_large_index() {
    assert_eq!(decision_id(0), NodeId(DECISION_BASE));
    assert_eq!(decision_id(4_095), NodeId(3_298_534_887_423));
    assert_eq!(decision_id(1 << 39), NodeId(3_848_290_697_216));
    assert_eq!(
        describe(&decision_id(1 << 39)),
        "pending decision 549755813888 (#3848290697216)"
    );
}
