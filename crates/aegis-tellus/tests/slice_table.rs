// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-1, the slice bound: a cgroup slice table never exceeds sixteen entries.
//!
//! The scaffold declares `MAX_CGROUP_SLICES: usize = 16` against NASA rule 2
//! and then writes into a fixed array with an index guard. Here the bound is
//! the array length and a push past it is a refusal, so the guard cannot be
//! removed without the type changing.

mod common;

use aegis_tellus::{
    MAX_CGROUP_SLICES, Provenance, SliceDraw, SliceName, SliceTable, TellusError, Watts,
};

use common::{EPSILON, Fallible};

/// Builds a draw of `watts` for a slice named `name`.
fn draw(name: &str, watts: f64) -> Result<SliceDraw, Box<dyn std::error::Error>> {
    Ok(SliceDraw::new(
        SliceName::parse(name)?,
        Watts::new(watts)?,
        Provenance::Simulated,
    ))
}

/// Fills `table` with `count` distinct slices of one watt each.
fn fill(table: &mut SliceTable, count: usize) -> Fallible {
    for index in 0..count {
        table.push(draw(&format!("slice-{index}"), 1.0)?)?;
    }
    Ok(())
}

// --- Positive -------------------------------------------------------------

/// Positive: the four slices the scaffold simulates are recorded and summed.
///
/// The scaffold's four names are `system.slice`, `app.slice`, `agent.slice`
/// and `build.slice`, and its simulated per-slice totals are 3.8, 6.0, 20.1
/// and 10.4 watts, which sum to 40.3.
#[test]
fn the_four_scaffold_slices_are_recorded_and_summed() -> Fallible {
    let mut table = SliceTable::new();
    for (name, watts) in [
        ("system.slice", 3.8),
        ("app.slice", 6.0),
        ("agent.slice", 20.1),
        ("build.slice", 10.4),
    ] {
        table.push(draw(name, watts)?)?;
    }
    assert_eq!(table.len(), 4);
    assert!(!table.is_empty());
    assert!(
        (table.total()?.get() - 40.3).abs() < 1.0e-9,
        "the simulated system draw is {}, not the recorded 40.3",
        table.total()?.get()
    );
    let agent = table
        .get(SliceName::parse("agent.slice")?)
        .ok_or("the table must return a slice it recorded")?;
    assert!((agent.watts.get() - 20.1).abs() < EPSILON);
    assert_eq!(agent.provenance, Provenance::Simulated);
    Ok(())
}

/// Positive: an empty table totals zero and reports the weakest provenance.
#[test]
fn an_empty_table_totals_zero() -> Fallible {
    let table = SliceTable::default();
    assert!(table.is_empty());
    assert_eq!(table.len(), 0);
    assert_eq!(table.total()?, Watts::ZERO);
    assert_eq!(
        table.provenance(),
        Provenance::Simulated,
        "an empty table has measured nothing, so it claims the least"
    );
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: the seventeenth slice is refused, and refusing changes nothing.
#[test]
fn the_seventeenth_slice_is_refused() -> Fallible {
    let mut table = SliceTable::new();
    fill(&mut table, MAX_CGROUP_SLICES)?;
    let before = table;
    let refusal = table.push(draw("one-too-many", 1.0)?);
    assert_eq!(
        refusal,
        Err(TellusError::SliceTableFull {
            bound: MAX_CGROUP_SLICES
        })
    );
    assert_eq!(table, before, "a refused push must change nothing");
    Ok(())
}

/// Negative: a table holding one modelled draw does not claim to be measured.
#[test]
fn one_modelled_draw_weakens_the_whole_table() -> Fallible {
    let mut table = SliceTable::new();
    table.push(SliceDraw::new(
        SliceName::parse("system.slice")?,
        Watts::new(1.0)?,
        Provenance::Measured,
    ))?;
    assert_eq!(table.provenance(), Provenance::Measured);
    table.push(SliceDraw::new(
        SliceName::parse("app.slice")?,
        Watts::new(1.0)?,
        Provenance::Modelled,
    ))?;
    assert_eq!(
        table.provenance(),
        Provenance::Modelled,
        "a total is only as good as its worst input"
    );
    assert!(!table.provenance().is_measured());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: sixteen slices are accepted and the bound is exactly sixteen.
#[test]
fn sixteen_slices_are_accepted_and_the_bound_is_sixteen() -> Fallible {
    assert_eq!(MAX_CGROUP_SLICES, 16);
    let mut table = SliceTable::new();
    assert_eq!(table.bound(), MAX_CGROUP_SLICES);
    fill(&mut table, MAX_CGROUP_SLICES)?;
    assert_eq!(table.len(), MAX_CGROUP_SLICES);
    assert!(
        (table.total()?.get() - 16.0).abs() < EPSILON,
        "sixteen one-watt slices total sixteen watts"
    );
    Ok(())
}

/// Boundary: the fifteenth and sixteenth are accepted, the seventeenth is not.
#[test]
fn the_bound_turns_between_the_sixteenth_and_the_seventeenth() -> Fallible {
    let mut table = SliceTable::new();
    fill(&mut table, MAX_CGROUP_SLICES.saturating_sub(1))?;
    assert_eq!(table.len(), 15);
    table.push(draw("the-sixteenth", 1.0)?)?;
    assert_eq!(table.len(), MAX_CGROUP_SLICES);
    assert!(table.push(draw("the-seventeenth", 1.0)?).is_err());
    assert_eq!(
        table.len(),
        MAX_CGROUP_SLICES,
        "a refused push must not grow the table"
    );
    Ok(())
}

/// Boundary: a draw at the wattage ceiling sums into a refusal rather than a
/// wrap, so the total is a `Result` for a reachable reason.
#[test]
fn a_total_past_the_wattage_ceiling_is_refused() -> Fallible {
    let mut table = SliceTable::new();
    table.push(draw("near-the-ceiling", Watts::MAX)?)?;
    assert!((table.total()?.get() - Watts::MAX).abs() < EPSILON);
    table.push(draw("one-watt-more", 1.0)?)?;
    assert!(
        table.total().is_err(),
        "a total past the ceiling must be refused, not wrapped"
    );
    Ok(())
}
