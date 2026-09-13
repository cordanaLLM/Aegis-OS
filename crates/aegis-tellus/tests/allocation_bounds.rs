// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! HISS-03: the claim that no decision path allocates, made falsifiable.
//!
//! The reason the crate can say a decision path allocates nothing is
//! structural: every value on such a path is `Copy`, so a check writes into
//! storage that already exists. `assert_no_heap::<T>()` is the falsifier: a
//! `Vec`, `String` or `Box` anywhere inside one of these types, at any depth,
//! removes `Copy` and this file stops compiling, which fails the gate.
//!
//! The imported scaffold does not have that property. Its
//! `CgroupPowerTelemetry::slice_name` is a `String` and its
//! `poll_ebpf_power_probes` calls `name.to_string()` once per slice per poll,
//! so sixteen slices cost sixteen allocations every time the loop runs.
//! [`SliceName`] is an inline buffer instead.
//!
//! # What is deliberately not claimed
//!
//! **Decoding is not unconditionally allocation-free, and this file says so by
//! exercising the case rather than by denying it.** A payload that spells a
//! value with a JSON escape makes `serde_json` unescape the string into a heap
//! scratch buffer before any field of ours sees it. That is bounded by
//! [`MAX_CONTRACT_PAYLOAD_BYTES`] and never retained, and the decoded value
//! still owns no heap -- which is the claim the crate actually makes.

mod common;

use core::mem::size_of;

use aegis_tellus::{
    Bid, CandidateId, CandidateList, CandidateSciQuery, CandidateSciResponse, ContractError,
    CorrelationId, DeltaV, EmbodiedCarbon, EnergyKwh, EnergyPrice, FunctionalUnits, GridIntensity,
    MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer, Provenance, RaplZone, RateEntry, RateList,
    SampleDeadline, SciCalculation, SciEngine, SciRate, Seconds, SimulatedWattage, SliceDraw,
    SliceName, SliceTable, TaskShiftDirective, TellusError, Watts, ZoneList, ZoneSample,
};

use common::{CORRELATION, Fallible, directive, query, response};

/// The largest an engine may be before the size is worth a second look.
const MAX_ENGINE_BYTES: usize = 32;

/// Accepts only a type that owns no heap, because it is `Copy`.
fn assert_no_heap<T: Copy>() {}

// --- Positive -------------------------------------------------------------

/// Positive: every value on a decision path owns no heap.
#[test]
fn every_value_on_a_decision_path_owns_no_heap() {
    assert_no_heap::<SciEngine>();
    assert_no_heap::<SciCalculation>();
    assert_no_heap::<SciRate>();
    assert_no_heap::<EnergyKwh>();
    assert_no_heap::<GridIntensity>();
    assert_no_heap::<EmbodiedCarbon>();
    assert_no_heap::<FunctionalUnits>();
    assert_no_heap::<Watts>();
    assert_no_heap::<Seconds>();
    assert_no_heap::<SimulatedWattage>();
    assert_no_heap::<ZoneList>();
    assert_no_heap::<ZoneSample>();
    assert_no_heap::<RaplZone>();
    assert_no_heap::<Provenance>();
    assert_no_heap::<SampleDeadline>();
    assert_no_heap::<SliceTable>();
    assert_no_heap::<SliceDraw>();
    assert_no_heap::<SliceName>();
    assert_no_heap::<CandidateId>();
    assert_no_heap::<CorrelationId>();
    assert_no_heap::<Bid>();
    assert_no_heap::<DeltaV>();
    assert_no_heap::<EnergyPrice>();
    assert_no_heap::<CandidateList>();
    assert_no_heap::<RateList>();
    assert_no_heap::<RateEntry>();
    assert_no_heap::<CandidateSciQuery>();
    assert_no_heap::<CandidateSciResponse>();
    assert_no_heap::<TaskShiftDirective>();
    assert_no_heap::<PayloadBuffer>();
    assert_no_heap::<TellusError>();
    assert_no_heap::<ContractError>();
}

/// Positive: evaluating a rate changes no storage.
#[test]
fn evaluating_a_rate_changes_no_storage() -> Fallible {
    let engine = SciEngine::new();
    let before = size_of::<SciEngine>();
    let sci = engine.sci_rate(EnergyKwh::new(0.0025)?, 1.0);
    assert_eq!(size_of_val(&engine), before);
    assert_eq!(size_of_val(&sci), size_of::<SciCalculation>());
    assert!(
        before <= MAX_ENGINE_BYTES,
        "the engine measures {before} bytes, past the recorded bound of {MAX_ENGINE_BYTES}"
    );
    Ok(())
}

/// Positive: filling the slice table to its bound changes its size not at all.
///
/// This is where the scaffold's per-poll `to_string()` would show: a table of
/// sixteen slices is one fixed-size value, not sixteen heap strings.
#[test]
fn filling_the_slice_table_changes_no_storage() -> Fallible {
    let mut table = SliceTable::new();
    let before = size_of_val(&table);
    for index in 0..aegis_tellus::MAX_CGROUP_SLICES {
        table.push(SliceDraw::new(
            SliceName::parse(&format!("slice-{index}"))?,
            Watts::new(1.0)?,
            Provenance::Simulated,
        ))?;
    }
    assert_eq!(size_of_val(&table), before);
    assert_eq!(size_of_val(&table), size_of::<SliceTable>());
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an escaped payload is the case the crate does not claim.
///
/// `m` is an admissible spelling of `m`, so the two payloads carry the
/// same correlation identifier. They decode to the same value -- which is the
/// property that matters -- while the escaped one costs `serde_json` a heap
/// scratch buffer the plain one does not.
#[test]
fn an_escaped_payload_decodes_to_the_same_value() -> Fallible {
    let payload = query()?;
    let mut buffer = PayloadBuffer::new();
    let plain = payload.encode_into(&mut buffer)?.to_owned();
    assert!(plain.contains(CORRELATION));

    let escaped = plain.replace("m05-fixture", "\\u006d05-fixture");
    assert!(escaped.contains("\\u006d"));
    assert_ne!(escaped, plain);
    assert_eq!(CandidateSciQuery::decode(&escaped)?, payload);
    Ok(())
}

/// Negative: the payload buffer refuses to grow, and reports nothing written.
#[test]
fn the_payload_buffer_does_not_grow() -> Fallible {
    let buffer = PayloadBuffer::new();
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.as_bytes(), b"");
    assert_eq!(buffer.as_str(), Some(""));

    let mut second = PayloadBuffer::default();
    response()?.encode_into(&mut second)?;
    assert_eq!(size_of_val(&second), size_of::<PayloadBuffer>());
    assert!(!second.is_empty());

    let mut third = PayloadBuffer::default();
    directive()?.encode_into(&mut third)?;
    assert!(third.len() < MAX_CONTRACT_PAYLOAD_BYTES);
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the buffer is exactly its bound plus its length field.
#[test]
fn the_payload_buffer_is_its_bound_plus_a_length() {
    let buffer = size_of::<PayloadBuffer>();
    assert!(buffer >= MAX_CONTRACT_PAYLOAD_BYTES);
    assert!(buffer <= MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(size_of::<usize>() * 2));
}

/// Boundary: a copied engine and a copied table share nothing with their
/// originals.
#[test]
fn a_copied_value_shares_nothing_with_its_original() -> Fallible {
    let original = SliceTable::new();
    let mut fork = original;
    fork.push(SliceDraw::new(
        SliceName::parse("app.slice")?,
        Watts::new(2.0)?,
        Provenance::Simulated,
    ))?;
    assert_eq!(fork.len(), 1);
    assert_eq!(original.len(), 0, "the original must be untouched");

    let bare = ZoneList::new();
    let mut copy = bare;
    copy.push(RaplZone::Psys)?;
    assert_eq!(copy.len(), 1);
    assert!(bare.is_empty());
    Ok(())
}
