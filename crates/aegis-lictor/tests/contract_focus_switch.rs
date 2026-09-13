// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-5, the focus half: the `FOCUS_SWITCH_NOTIFY` report.
//!
//! Positive: a report encodes and decodes to the value it started as, carrying
//! the control-group slice REQ-P04-07 requires. Negative: a malformed report
//! is refused, and a slice name that is not a control group is refused even
//! though it is a perfectly good identifier. Boundary: the process identifier
//! is exact on the wire at both ends of its range.

mod common;

use aegis_lictor::{
    ContractError, Correlation, CorrelationId, EdgeId, FocusSwitchReport, FocusSwitchVersion,
    IdError, MAX_CONTRACT_PAYLOAD_BYTES, MAX_PID, PayloadBuffer, SLICE_SUFFIX, SchemaId, SliceName,
};

use common::{COMPOSITOR_PID, Fallible, encoded_report, other_edge, report, tamper};

// --- Positive -------------------------------------------------------------

/// Positive: a report round-trips to the value it started as.
#[test]
fn a_report_round_trips() -> Fallible {
    let original = report(COMPOSITOR_PID)?;
    let decoded = FocusSwitchReport::decode(&encoded_report(&original)?)?;
    assert_eq!(decoded, original);
    assert_eq!(FocusSwitchReport::SCHEMA, SchemaId::FocusSwitchReport);
    assert_eq!(SchemaId::ALL.len(), 1);
    Ok(())
}

/// Positive: every field reads back, the control-group slice among them.
#[test]
fn every_field_of_a_report_reads_back() -> Fallible {
    let decoded = FocusSwitchReport::decode(&encoded_report(&report(COMPOSITOR_PID)?)?)?;
    let fields = (
        decoded.schema,
        decoded.edge,
        decoded.correlation_id,
        decoded.app_id.to_string(),
        decoded.cgroup_slice.to_string(),
        decoded.pid.get(),
        decoded.surface_id,
    );
    assert_eq!(
        fields,
        (
            FocusSwitchVersion::V1,
            EdgeId::FocusSwitchNotify,
            CorrelationId::parse(common::CORRELATION)?,
            common::COMPOSITOR.to_owned(),
            common::SLICE.to_owned(),
            COMPOSITOR_PID,
            1,
        )
    );
    assert_eq!(decoded.edge, FocusSwitchReport::EDGE);
    Ok(())
}

/// Positive: the schema knows its own tag and edge, and a refusal would name
/// both.
#[test]
fn the_schema_names_its_tag_and_edge() -> Fallible {
    assert_eq!(
        SchemaId::FocusSwitchReport.tag(),
        "aegis.p04-p07.focus-switch.v1"
    );
    assert_eq!(
        SchemaId::FocusSwitchReport.edge(),
        EdgeId::FocusSwitchNotify
    );
    assert_eq!(
        SchemaId::FocusSwitchReport.to_string(),
        "aegis.p04-p07.focus-switch.v1"
    );
    let correlated: Correlation = report(COMPOSITOR_PID)?.correlation();
    assert_eq!(correlated.schema(), SchemaId::FocusSwitchReport);
    assert!(correlated.to_string().contains(common::CORRELATION));
    let anonymous = Correlation::new(SchemaId::FocusSwitchReport, None);
    assert_eq!(anonymous.id(), None);
    assert!(anonymous.to_string().contains("uncorrelated"));
    Ok(())
}

/// Positive: the encoder writes into a caller-supplied buffer.
#[test]
fn the_encoder_writes_into_the_callers_buffer() -> Fallible {
    let mut buffer = PayloadBuffer::default();
    let empty = (buffer.is_empty(), buffer.len(), buffer.as_bytes().to_vec());
    assert_eq!(empty, (true, 0, Vec::new()));
    let text = report(COMPOSITOR_PID)?.encode_into(&mut buffer)?.to_owned();
    let written = (
        buffer.is_empty(),
        buffer.len(),
        buffer.as_str().map(str::to_owned),
    );
    assert_eq!(written, (false, text.len(), Some(text.clone())));
    assert!(text.len() <= MAX_CONTRACT_PAYLOAD_BYTES);
    Ok(())
}

/// Positive: the recorded edges name their recorded transports, and the two
/// DSP-19 lists this milestone can source are stated as membership.
#[test]
fn the_edges_name_their_recorded_transports() {
    assert_eq!(EdgeId::ALL.len(), 4);
    assert_eq!(
        EdgeId::FocusSwitchNotify.recorded_transport(),
        "Eclipse Zenoh Zero-Copy Shared Memory"
    );
    assert_eq!(
        EdgeId::PrioritizeCompositorThread.recorded_transport(),
        "Linux sched_ext struct_ops / scx_cake"
    );
    assert_eq!(
        EdgeId::SpatiotemporalTaskShift.recorded_transport(),
        "D-Bus / kepler_power.bpf"
    );
}

/// Positive: the two edges dispute DSP-19 lists as graph-only are the two
/// outbound ones, and only the inbound schema is declared in this crate.
///
/// Membership of a recorded list, and nothing wider: `false` here means the
/// edge is not on the DSP-19 list, not that two documents attest it.
#[test]
fn the_dsp_19_membership_is_stated_as_membership() {
    assert!(EdgeId::PrioritizeCompositorThread.listed_graph_only_by_dsp_19());
    assert!(EdgeId::EnforceRealtimeRtprio.listed_graph_only_by_dsp_19());
    assert!(!EdgeId::FocusSwitchNotify.listed_graph_only_by_dsp_19());
    assert!(!EdgeId::SpatiotemporalTaskShift.listed_graph_only_by_dsp_19());
    assert!(EdgeId::FocusSwitchNotify.declared_here());
    assert!(!EdgeId::SpatiotemporalTaskShift.declared_here());
    assert_eq!(
        EdgeId::EnforceRealtimeRtprio.tag(),
        "P07_Lictor->P08_Calliope:ENFORCE_REALTIME_RTPRIO"
    );
}

// --- Negative -------------------------------------------------------------

/// Negative: a payload naming another contract version is refused as such.
#[test]
fn another_contract_version_is_refused() -> Fallible {
    let text = encoded_report(&report(COMPOSITOR_PID)?)?;
    let tampered = tamper(&text, "focus-switch.v1", "focus-switch.v2")?;
    let refusal = FocusSwitchReport::decode(&tampered);
    assert!(matches!(refusal, Err(ContractError::UnknownVersion { .. })));
    Ok(())
}

/// Negative: a slice name that is not a control group is refused, even when it
/// is a perfectly good identifier.
///
/// REQ-P04-07's whole point is that the report carries the slice the broker
/// will re-allocate. A name the broker cannot resolve is worse than none.
#[test]
fn a_name_that_is_not_a_control_group_slice_is_refused() -> Fallible {
    assert_eq!(
        SliceName::parse("aegis-compositor"),
        Err(IdError::NotASlice)
    );
    assert_eq!(SliceName::parse(SLICE_SUFFIX), Err(IdError::NotASlice));
    assert_eq!(SliceName::parse(""), Err(IdError::NotASlice));
    let text = encoded_report(&report(COMPOSITOR_PID)?)?;
    let tampered = tamper(&text, common::SLICE, "app-aegis-compositor.service")?;
    assert!(matches!(
        FocusSwitchReport::decode(&tampered),
        Err(ContractError::Malformed { .. })
    ));
    Ok(())
}

/// Negative: an unknown field, an empty application identifier and a process
/// identifier of zero are each refused.
#[test]
fn malformed_reports_are_refused() -> Fallible {
    let text = encoded_report(&report(COMPOSITOR_PID)?)?;
    for (from, to) in [
        ("\"app-id\"", "\"application-id\""),
        (common::COMPOSITOR, ""),
        ("\"pid\":1001", "\"pid\":0"),
        ("\"surface-id\":1", "\"surface-id\":\"one\""),
    ] {
        let tampered = tamper(&text, from, to)?;
        assert!(
            matches!(
                FocusSwitchReport::decode(&tampered),
                Err(ContractError::Malformed { .. })
            ),
            "the decoder accepted {from:?} replaced by {to:?}"
        );
    }
    Ok(())
}

/// Negative: a payload naming another of this crate's edges is refused as
/// travelling on the wrong crossing.
#[test]
fn another_edge_is_refused_as_the_wrong_edge() -> Fallible {
    let text = encoded_report(&report(COMPOSITOR_PID)?)?;
    let tampered = tamper(
        &text,
        EdgeId::FocusSwitchNotify.tag(),
        other_edge(EdgeId::FocusSwitchNotify).tag(),
    )?;
    assert!(matches!(
        FocusSwitchReport::decode(&tampered),
        Err(ContractError::WrongEdge { .. })
    ));
    Ok(())
}

/// Negative: a payload past the byte bound is refused before it is parsed.
#[test]
fn an_over_long_payload_is_refused_before_parsing() {
    let padding = "y".repeat(MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(1));
    let refusal = FocusSwitchReport::decode(&padding);
    assert!(matches!(
        refusal,
        Err(ContractError::PayloadTooLong {
            max: MAX_CONTRACT_PAYLOAD_BYTES,
            ..
        })
    ));
    if let Err(error) = refusal {
        assert_eq!(error.correlation().id(), None);
    }
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the process identifier is exact on the wire at both ends.
#[test]
fn the_wire_process_identifier_is_exact_at_both_ends() -> Fallible {
    let text = encoded_report(&report(COMPOSITOR_PID)?)?;
    let at_ceiling = tamper(&text, "\"pid\":1001", &format!("\"pid\":{MAX_PID}"))?;
    assert_eq!(FocusSwitchReport::decode(&at_ceiling)?.pid.get(), MAX_PID);
    let above = tamper(
        &text,
        "\"pid\":1001",
        &format!("\"pid\":{}", MAX_PID.saturating_add(1)),
    )?;
    assert!(matches!(
        FocusSwitchReport::decode(&above),
        Err(ContractError::Malformed { .. })
    ));
    let one = tamper(&text, "\"pid\":1001", "\"pid\":1")?;
    assert_eq!(FocusSwitchReport::decode(&one)?.pid.get(), 1);
    Ok(())
}
