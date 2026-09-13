// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E07-5: the `ENFORCE_REALTIME_RTPRIO` grant, round-tripped and refused.
//!
//! Positive: a grant encodes and decodes to the value it started as.
//! Negative: a malformed payload is refused, and each refusal names what it
//! was about. Boundary: a priority at the source value decodes and one above
//! it does not.

mod common;

use aegis_calliope::{
    ContractError, Correlation, EdgeId, GrantedRtPrio, MAX_CONTRACT_PAYLOAD_BYTES, PayloadBuffer,
    RealtimeGrant, RealtimeGrantVersion, SchedPolicy, SchemaId,
};

use common::{Fallible, correlation, encoded_grant, grant, other_edge, tamper};

// --- Positive -------------------------------------------------------------

/// Positive: a grant round-trips to the value it started as.
#[test]
fn a_grant_round_trips() -> Fallible {
    let original = grant(95)?;
    let decoded = RealtimeGrant::decode(&encoded_grant(&original)?)?;
    assert_eq!(decoded, original);
    assert_eq!(RealtimeGrant::SCHEMA, SchemaId::RealtimeGrant);
    Ok(())
}

/// Positive: every field of a round-tripped grant reads back.
#[test]
fn every_field_of_a_grant_reads_back() -> Fallible {
    let decoded = RealtimeGrant::decode(&encoded_grant(&grant(95)?)?)?;
    let fields = (
        decoded.schema,
        decoded.edge,
        decoded.policy,
        decoded.rtprio,
        decoded.correlation_id,
        decoded.thread.to_string(),
    );
    assert_eq!(
        fields,
        (
            RealtimeGrantVersion::V1,
            EdgeId::EnforceRealtimeRtprio,
            SchedPolicy::RoundRobin,
            GrantedRtPrio::REQUIRED,
            correlation()?,
            common::THREAD.to_owned(),
        )
    );
    assert_eq!(decoded.edge, RealtimeGrant::EDGE);
    assert_eq!(decoded.policy, RealtimeGrant::POLICY);
    assert!(decoded.is_source_priority());
    Ok(())
}

/// Positive: the encoder writes into a caller-supplied buffer and reports what
/// it wrote.
#[test]
fn the_encoder_writes_into_the_callers_buffer() -> Fallible {
    let mut buffer = PayloadBuffer::new();
    let empty = (buffer.is_empty(), buffer.len(), buffer.as_bytes().to_vec());
    assert_eq!(empty, (true, 0, Vec::new()));
    let value = grant(80)?;
    let text = value.encode_into(&mut buffer)?.to_owned();
    let written = (
        buffer.is_empty(),
        buffer.len(),
        buffer.as_str().map(str::to_owned),
    );
    assert_eq!(written, (false, text.len(), Some(text.clone())));
    assert_eq!(buffer.as_bytes(), text.as_bytes());
    assert!(text.len() <= MAX_CONTRACT_PAYLOAD_BYTES);
    assert!(!value.is_source_priority());
    Ok(())
}

/// Positive: the schema knows its own tag and edge, and the correlation a
/// refusal would carry names both.
#[test]
fn the_schema_names_its_tag_and_edge() -> Fallible {
    assert_eq!(
        SchemaId::RealtimeGrant.tag(),
        "aegis.p07-p08.realtime-grant.v1"
    );
    assert_eq!(
        SchemaId::RealtimeGrant.edge(),
        EdgeId::EnforceRealtimeRtprio
    );
    assert_eq!(SchemaId::ALL.len(), 2);
    assert_eq!(
        SchemaId::RealtimeGrant.to_string(),
        "aegis.p07-p08.realtime-grant.v1"
    );
    let value = grant(95)?;
    let correlated: Correlation = value.correlation();
    assert_eq!(correlated.schema(), SchemaId::RealtimeGrant);
    assert_eq!(correlated.id(), Some(correlation()?));
    assert!(correlated.to_string().contains(common::CORRELATION));
    let anonymous = Correlation::new(SchemaId::RealtimeGrant, None);
    assert!(anonymous.to_string().contains("uncorrelated"));
    assert_eq!(anonymous.id(), None);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a payload naming another contract version is refused as such.
#[test]
fn another_contract_version_is_refused() -> Fallible {
    let text = encoded_grant(&grant(95)?)?;
    let tampered = tamper(&text, "realtime-grant.v1", "realtime-grant.v2")?;
    let refusal = RealtimeGrant::decode(&tampered);
    assert!(matches!(refusal, Err(ContractError::UnknownVersion { .. })));
    let correlated = refusal.err().map(|error| error.correlation());
    assert_eq!(correlated.and_then(|c| c.id()), Some(correlation()?));
    Ok(())
}

/// Negative: an unknown field, a policy the unit does not name and an empty
/// thread label are each refused.
#[test]
fn malformed_grants_are_refused() -> Fallible {
    let text = encoded_grant(&grant(95)?)?;
    for (from, to) in [
        ("\"thread\"", "\"thread-group\""),
        ("\"sched-rr\"", "\"sched-fifo\""),
        (common::THREAD, ""),
        (common::CORRELATION, "not a correlation"),
    ] {
        let tampered = tamper(&text, from, to)?;
        assert!(
            matches!(
                RealtimeGrant::decode(&tampered),
                Err(ContractError::Malformed { .. } | ContractError::UnknownVersion { .. })
            ),
            "the decoder accepted {from:?} replaced by {to:?}"
        );
    }
    Ok(())
}

/// Negative: a payload naming the other edge this crate carries is refused as
/// travelling on the wrong crossing, not merely as malformed.
#[test]
fn the_other_edge_is_refused_as_the_wrong_edge() -> Fallible {
    let text = encoded_grant(&grant(95)?)?;
    let tampered = tamper(
        &text,
        EdgeId::EnforceRealtimeRtprio.tag(),
        other_edge(EdgeId::EnforceRealtimeRtprio).tag(),
    )?;
    assert!(matches!(
        RealtimeGrant::decode(&tampered),
        Err(ContractError::WrongEdge { .. })
    ));
    Ok(())
}

/// Negative: a payload past the byte bound is refused before it is parsed.
#[test]
fn an_over_long_payload_is_refused_before_parsing() {
    let padding = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(1));
    let refusal = RealtimeGrant::decode(&padding);
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

/// Boundary: 95 decodes and 96 does not.
///
/// The refusal happens inside the decoder, through the same constructor the
/// library uses, so an unauthorised priority never becomes a value even
/// transiently.
#[test]
fn the_wire_priority_is_exact_at_the_source_value() -> Fallible {
    let text = encoded_grant(&grant(95)?)?;
    assert!(text.contains("\"rtprio\":95"));
    assert!(RealtimeGrant::decode(&text).is_ok());

    let above = tamper(&text, "\"rtprio\":95", "\"rtprio\":96")?;
    assert!(matches!(
        RealtimeGrant::decode(&above),
        Err(ContractError::Malformed { .. })
    ));

    let zero = tamper(&text, "\"rtprio\":95", "\"rtprio\":0")?;
    assert!(matches!(
        RealtimeGrant::decode(&zero),
        Err(ContractError::Malformed { .. })
    ));

    let one = tamper(&text, "\"rtprio\":95", "\"rtprio\":1")?;
    assert_eq!(RealtimeGrant::decode(&one)?.rtprio.get(), 1);
    Ok(())
}
