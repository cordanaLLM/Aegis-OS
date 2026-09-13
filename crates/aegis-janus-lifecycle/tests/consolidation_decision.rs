// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E15-2: the D13 decision is recorded, and a reopened slot still requires a
//! verity match before `Bless`.
//!
//! Two things are checked here and they are deliberately separate. The first
//! is that the register in
//! [`decision`](aegis_janus_lifecycle::decision) says what the roadmap says:
//! reversible structural consolidation, closed, reconciling REQ-P02-05 with
//! REQ-P02-01 and REQ-P16-05, against dispute DSP-27. The second is that the
//! machine behaves as the register claims, which is the half a register cannot
//! assert about itself: reopening discards the measurement, and a re-bless
//! without a fresh matching one fails closed.
//!
//! # Scope
//!
//! No dm-verity hash is computed anywhere in this crate. These tests show when
//! a match is required and what a mismatch does; whether a real reopened slot
//! re-verifies is M24 work over a real transfer.

mod common;

use aegis_janus_lifecycle::{
    Citation, ConsolidationModel, D13_CONSOLIDATION, DecisionRecord, DecisionState, Event,
    LifecycleError, Machine, SignatureVerdict, State, Tick,
};

use common::{DUE_AT, Fallible, WINDOW, blessed, drifted, refusal, signed, timeout};

// --- Positive -------------------------------------------------------------

/// Positive: D13 is recorded as closed, with the model that was chosen, the
/// model that was not, the requirements it reconciles and its sources.
#[test]
fn the_d13_decision_is_recorded() {
    let record: DecisionRecord = D13_CONSOLIDATION;
    assert_eq!(record.id, "D13");
    assert_eq!(record.state, DecisionState::Closed);
    assert_eq!(record.state.name(), "closed");
    assert_eq!(record.chosen, ConsolidationModel::ReversibleConsolidation);
    assert_eq!(record.rejected, ConsolidationModel::PermanentHardening);
    assert_eq!(
        record.reconciles,
        ["REQ-P02-05", "REQ-P02-01", "REQ-P16-05"]
    );
    assert_eq!(record.dispute, "DSP-27");
    assert!(
        record
            .question
            .contains("Reversible structural consolidation")
    );
    assert!(record.reconciliation.contains("fresh measurement"));
}

/// Positive: D13's sources are cited by export identifier and digest prefix
/// only, which is how this repository cites private material.
#[test]
fn the_d13_sources_are_cited_by_export_and_digest_prefix() {
    for citation in D13_CONSOLIDATION.sources {
        let cited: Citation = citation;
        assert_eq!(cited, citation);
        assert!(citation.export.starts_with("export-"));
        assert_eq!(citation.sha256_prefix.len(), 12);
        assert!(
            citation
                .sha256_prefix
                .chars()
                .all(|c| c.is_ascii_hexdigit())
        );
    }
}

/// Positive: a reopened slot that presents a fresh matching measurement is
/// blessed again, which is what makes the consolidation reversible.
#[test]
fn a_reopened_slot_is_blessed_again_after_a_fresh_matching_measurement() -> Fallible {
    let (mut machine, clock) = blessed()?;
    machine.step(&clock, Event::Reopen(timeout(WINDOW)?))?;
    assert_eq!(machine.state(), State::Reopened);
    machine.step(&clock, Event::Remeasure(signed()?))?;
    assert_eq!(machine.measured_root_hash(), Some(signed()?));
    machine.step(&clock, Event::Bless)?;
    assert_eq!(machine.state(), State::Blessed);
    assert_eq!(machine.watchdog(), None, "re-blessing closes the window");
    Ok(())
}

/// Positive: the rejected model stays representable, so the register records a
/// choice between two readings rather than the only one it can express.
#[test]
fn both_consolidation_models_are_representable() {
    assert_eq!(ConsolidationModel::ALL.len(), 2);
    assert!(ConsolidationModel::ReversibleConsolidation.permits_reopening());
    assert!(!ConsolidationModel::PermanentHardening.permits_reopening());
    assert_eq!(
        ConsolidationModel::ReversibleConsolidation.name(),
        "reversible-structural-consolidation"
    );
    assert_eq!(
        ConsolidationModel::PermanentHardening.name(),
        "permanent-hardening"
    );
    assert_eq!(DecisionState::Unresolved.name(), "unresolved");
}

// --- Negative -------------------------------------------------------------

/// Negative: a reopened slot cannot be blessed on the strength of the
/// measurement it held before it was reopened.
#[test]
fn a_reopened_slot_holds_no_measurement() -> Fallible {
    let (mut machine, clock) = blessed()?;
    assert_eq!(machine.measured_root_hash(), Some(signed()?));
    machine.step(&clock, Event::Reopen(timeout(WINDOW)?))?;
    assert_eq!(
        machine.measured_root_hash(),
        None,
        "reopening must discard the measurement"
    );
    let error = refusal(&mut machine, clock, Event::Bless)?;
    assert_eq!(error, LifecycleError::VerityUnmeasured);
    assert_eq!(machine.state(), State::Reopened);
    Ok(())
}

/// Negative: a reopened slot that measures to anything but the signed release
/// is refused, with both hashes named.
#[test]
fn a_reopened_slot_that_drifted_is_refused() -> Fallible {
    let (mut machine, clock) = blessed()?;
    machine.step(&clock, Event::Reopen(timeout(WINDOW)?))?;
    machine.step(&clock, Event::Remeasure(drifted()?))?;
    let error = refusal(&mut machine, clock, Event::Bless)?;
    assert_eq!(
        error,
        LifecycleError::VerityMismatch {
            expected: signed()?,
            measured: drifted()?,
        }
    );
    assert_eq!(machine.state(), State::Reopened);
    Ok(())
}

/// Negative: the same gate applies on the first bless, not only after a
/// reopening: a delta that measured to the wrong hash is never blessed.
#[test]
fn a_first_bless_over_a_drifted_delta_is_refused() -> Fallible {
    let clock = common::armed()?.1;
    let mut machine = Machine::new();
    machine.step(&clock, Event::Declare(common::candidate()?))?;
    machine.step(&clock, Event::CheckSignature(SignatureVerdict::Valid))?;
    machine.step(&clock, Event::AcquireDelta(drifted()?))?;
    machine.step(&clock, Event::SwapSlot)?;
    machine.step(&clock, Event::ArmWatchdog(timeout(WINDOW)?))?;
    let error = refusal(&mut machine, clock, Event::Bless)?;
    assert_eq!(
        error,
        LifecycleError::VerityMismatch {
            expected: signed()?,
            measured: drifted()?,
        }
    );
    assert_eq!(machine.state(), State::WatchdogArmed);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the drifted hash differs from the signed one in a single nibble,
/// and that is enough to refuse the bless.
#[test]
fn a_single_nibble_of_drift_is_a_mismatch() -> Fallible {
    let signed_bytes = *signed()?.as_bytes();
    let drifted_bytes = *drifted()?.as_bytes();
    let differing = signed_bytes
        .iter()
        .zip(drifted_bytes.iter())
        .filter(|(left, right)| left != right)
        .count();
    assert_eq!(differing, 1, "the fixtures differ in exactly one byte");
    assert_ne!(signed()?, drifted()?);
    Ok(())
}

/// Boundary: maintenance is bounded. A tick exactly at the end of the reopened
/// window rolls the slot back; one tick before it does not.
#[test]
fn the_maintenance_window_is_bounded_at_exactly_its_timeout() -> Fallible {
    let (mut machine, mut clock) = blessed()?;
    clock.set(Tick::new(DUE_AT));
    machine.step(&clock, Event::Reopen(timeout(WINDOW)?))?;
    let window_due = DUE_AT.saturating_add(u64::from(WINDOW));

    let mut early = machine;
    let mut early_clock = clock;
    early_clock.set(Tick::new(window_due.saturating_sub(1)));
    early.step(&early_clock, Event::Tick)?;
    assert_eq!(early.state(), State::Reopened);

    let mut exact = machine;
    let mut exact_clock = clock;
    exact_clock.set(Tick::new(window_due));
    exact.step(&exact_clock, Event::Tick)?;
    assert_eq!(
        exact.state(),
        State::RolledBack,
        "an unfinished maintenance window rolls back at its deadline"
    );
    Ok(())
}
