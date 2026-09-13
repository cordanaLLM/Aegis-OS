// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-4, the plugin half: the slot bound and the tolerance lifecycle.
//!
//! Positive: 32 slots are accepted and every field of a row reads back.
//! Negative: the 33rd slot is refused, and so is every step the lifecycle does
//! not admit -- REQ-P08-03's Quarantine-before-Activation among them.
//! Boundary: the 32nd slot is accepted and the 33rd is not, and the transition
//! table is checked at every one of its 25 ordered pairs rather than at the
//! handful a reader might think of.

mod common;

use aegis_calliope::{
    CalliopeError, Label, MAX_PLUGIN_SLOTS, PluginFormat, PluginHost, PluginSandboxSlot,
    PluginSlotId, PluginStage,
};

use common::{Fallible, filled_host, host_at, label};

// --- Positive -------------------------------------------------------------

/// Positive: the host admits its full complement of slots.
#[test]
fn the_host_admits_thirty_two_plugins() -> Fallible {
    let host = filled_host(MAX_PLUGIN_SLOTS)?;
    assert_eq!(host.count(), MAX_PLUGIN_SLOTS);
    assert_eq!(MAX_PLUGIN_SLOTS, 32);
    assert!(!host.is_empty());
    assert_eq!(host.in_realtime_graph(), 0);
    Ok(())
}

/// Positive: an admitted row reads back with every field it was given.
#[test]
fn an_admitted_row_reads_back() -> Fallible {
    let mut host = PluginHost::new();
    assert!(host.is_empty());
    let name = label(common::PLUGIN)?;
    let slot = host.admit(name, PluginFormat::Lv2)?;
    let row: PluginSandboxSlot = host
        .get(slot)
        .ok_or("the host must hold the slot it just admitted")?;
    assert_eq!(row.slot, slot);
    assert_eq!(row.name, name);
    assert_eq!(row.format, PluginFormat::Lv2);
    assert_eq!(row.stage, PluginStage::Recognition);
    assert_eq!(slot.get(), 0);
    assert_eq!(PluginSlotId::new(0), slot);
    Ok(())
}

/// Positive: the recorded path Recognition to Quarantine to Activation runs,
/// and only an activated plugin counts as being in the real-time graph.
#[test]
fn the_recorded_path_reaches_the_realtime_graph() -> Fallible {
    let (mut host, slot) = host_at(PluginStage::Quarantine)?;
    assert_eq!(host.in_realtime_graph(), 0);
    assert_eq!(
        host.advance(slot, PluginStage::Activation)?,
        PluginStage::Activation
    );
    assert_eq!(host.in_realtime_graph(), 1);
    assert_eq!(
        host.advance(slot, PluginStage::Anergic)?,
        PluginStage::Anergic
    );
    assert_eq!(host.in_realtime_graph(), 0);
    assert_eq!(
        host.advance(slot, PluginStage::Contraction)?,
        PluginStage::Contraction
    );
    Ok(())
}

/// Positive: every format and every stage carries a distinct stable name, and
/// a format round-trips through its wire tag.
#[test]
fn formats_and_stages_carry_distinct_names() {
    let mut format_tags: Vec<&str> = PluginFormat::ALL.iter().map(|f| f.tag()).collect();
    format_tags.sort_unstable();
    format_tags.dedup();
    assert_eq!(format_tags.len(), PluginFormat::ALL.len());
    for format in PluginFormat::ALL {
        assert_eq!(PluginFormat::from_tag(format.tag()), Some(format));
    }
    assert_eq!(PluginFormat::from_tag("au"), None);

    let mut stage_names: Vec<&str> = PluginStage::ALL.iter().map(|s| s.name()).collect();
    stage_names.sort_unstable();
    stage_names.dedup();
    assert_eq!(stage_names.len(), 5);
    assert!(PluginStage::Contraction.is_terminal());
    assert!(!PluginStage::Anergic.is_terminal());
    assert!(PluginStage::Activation.in_realtime_graph());
    assert!(!PluginStage::Quarantine.in_realtime_graph());
}

// --- Negative -------------------------------------------------------------

/// Negative: the 33rd plugin is refused with the bound it hit.
#[test]
fn the_thirty_third_plugin_is_refused() -> Fallible {
    let mut host = filled_host(MAX_PLUGIN_SLOTS)?;
    let refusal = host.admit(label(common::PLUGIN)?, PluginFormat::Vst3);
    assert_eq!(
        refusal,
        Err(CalliopeError::PluginTableFull {
            max: MAX_PLUGIN_SLOTS
        })
    );
    assert_eq!(host.count(), MAX_PLUGIN_SLOTS);
    Ok(())
}

/// Negative: REQ-P08-03, as a refusal. Quarantine is not optional.
#[test]
fn activation_without_quarantine_is_refused() -> Fallible {
    let (mut host, slot) = host_at(PluginStage::Recognition)?;
    assert_eq!(
        host.advance(slot, PluginStage::Activation),
        Err(CalliopeError::IllegalStageTransition {
            from: PluginStage::Recognition,
            to: PluginStage::Activation,
        })
    );
    let row = host
        .get(slot)
        .ok_or("the row must survive a refused step")?;
    assert_eq!(row.stage, PluginStage::Recognition);
    Ok(())
}

/// Negative: a contracted plugin is finished, and a slot the host does not
/// hold cannot be stepped at all.
#[test]
fn a_terminal_stage_and_an_unknown_slot_are_refused() -> Fallible {
    let (mut host, slot) = host_at(PluginStage::Contraction)?;
    assert_eq!(
        host.advance(slot, PluginStage::Activation),
        Err(CalliopeError::IllegalStageTransition {
            from: PluginStage::Contraction,
            to: PluginStage::Activation,
        })
    );
    let stranger = PluginSlotId::new(31);
    assert_eq!(
        host.advance(stranger, PluginStage::Quarantine),
        Err(CalliopeError::UnknownSlot { slot: 31 })
    );
    assert_eq!(host.get(stranger), None);
    Ok(())
}

/// Negative: the scaffold's own plugin name is refused rather than repaired.
#[test]
fn the_scaffold_plugin_name_is_refused() {
    assert!(Label::parse("FabFilter Pro-Q 3").is_err());
    assert!(Label::parse("").is_err());
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the 32nd slot is accepted and the 33rd is not.
#[test]
fn the_bound_is_exact_at_thirty_two() -> Fallible {
    let mut host = filled_host(MAX_PLUGIN_SLOTS.saturating_sub(1))?;
    assert_eq!(host.count(), 31);
    let last = host.admit(label(common::PLUGIN)?, PluginFormat::Clap)?;
    assert_eq!(last.get(), 31);
    assert_eq!(host.count(), 32);
    assert!(
        host.admit(label(common::PLUGIN)?, PluginFormat::Clap)
            .is_err()
    );
    Ok(())
}

/// Boundary: the transition table is checked at all 25 ordered pairs.
///
/// A test that listed only the interesting steps would pass while an
/// unintended one crept in beside them. This one enumerates the whole product
/// of the stage set and compares it against the recorded lifecycle, so a new
/// edge in the table is a failure here rather than a silent widening.
#[test]
fn every_ordered_pair_of_stages_matches_the_recorded_lifecycle() {
    let admitted: [(PluginStage, PluginStage); 9] = [
        (PluginStage::Recognition, PluginStage::Quarantine),
        (PluginStage::Quarantine, PluginStage::Anergic),
        (PluginStage::Quarantine, PluginStage::Activation),
        (PluginStage::Quarantine, PluginStage::Contraction),
        (PluginStage::Anergic, PluginStage::Quarantine),
        (PluginStage::Anergic, PluginStage::Activation),
        (PluginStage::Anergic, PluginStage::Contraction),
        (PluginStage::Activation, PluginStage::Anergic),
        (PluginStage::Activation, PluginStage::Contraction),
    ];
    let mut seen = 0usize;
    for from in PluginStage::ALL {
        for to in PluginStage::ALL {
            seen = seen.saturating_add(1);
            let expected = admitted.contains(&(from, to));
            assert_eq!(
                from.may_advance_to(to),
                expected,
                "the pair {from:?} -> {to:?} disagrees with the recorded lifecycle"
            );
        }
    }
    assert_eq!(seen, 25);
    for stage in PluginStage::ALL {
        assert!(!stage.may_advance_to(stage), "a step to the same stage");
        assert!(!PluginStage::Contraction.may_advance_to(stage));
    }
}
