// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epic E06-4: the capsule request P09 Minerva sends to P10 Vesta.
//!
//! Positive, negative and boundary coverage of the schema: a well-formed
//! request round-trips, a malformed one is refused with a correlated error,
//! and the slot bound holds on the wire as well as in the table. The monitor
//! identity decision D58 requires is a field, and both monitors round-trip.
//!
//! No transport is exercised, because none exists.

mod common;

use aegis_justitia::SandboxAdmissionPath;
use aegis_vesta::{
    AdmittedRuntime, Capability, CapabilitySet, CapsuleRequest, ContractError, Correlation,
    CorrelationId, EdgeId, MAX_CAPABILITY_ENTRIES, MAX_CONTRACT_PAYLOAD_BYTES, MAX_WASM_CAPSULES,
    PayloadBuffer, SchemaId, VmmIdentity,
};

use common::{CORRELATION, Fallible, capsule_request, encoded_request, other_edge, tamper};

/// The exact payload the fixture request encodes to.
///
/// A golden text is the strongest available statement that the field encoding
/// is stable: a renamed field, a reordered struct, a changed enum tag or a
/// different capability rendering all fail here rather than silently ship.
const GOLDEN: &str = r#"{"schema":"aegis.p09-p10.capsule-request.v1","edge":"EXECUTE_WASMED_CAPSULE","admission":"transitive-through-minerva","runtime":"rust-native-component-model","vmm":"firecracker","correlation-id":"m06-fixture-0001","capsule":"aegis-extism-capsule-01","slot":1,"memory-limit-bytes":16777216,"capabilities":["filesystem-read","zenoh-ipc"]}"#;

/// Returns the correlation a refusal of the fixture request must carry.
fn expected_correlation() -> Result<Correlation, Box<dyn std::error::Error>> {
    Ok(Correlation::new(
        SchemaId::CapsuleRequest,
        Some(CorrelationId::parse(CORRELATION)?),
    ))
}

// --- Positive -------------------------------------------------------------

/// Positive: a well-formed request round-trips, and the encoding is stable.
#[test]
fn a_capsule_request_round_trips() -> Fallible {
    let original = capsule_request(1, VmmIdentity::Firecracker)?;
    let text = encoded_request(&original)?;
    assert_eq!(text, GOLDEN, "the field encoding is stable");
    let decoded = CapsuleRequest::decode(&text)?;
    assert_eq!(decoded, original, "serialise then deserialise is identity");
    assert_eq!(encoded_request(&decoded)?, text);
    assert_eq!(decoded.validate(), Ok(()));
    assert_eq!(decoded.correlation(), expected_correlation()?);
    assert_eq!(decoded.correlation().schema(), SchemaId::CapsuleRequest);
    assert_eq!(
        decoded.correlation().id(),
        Some(CorrelationId::parse(CORRELATION)?)
    );
    Ok(())
}

/// Positive: both monitors decision D58 admits round-trip, so a figure taken
/// from either sandbox can always be attributed.
#[test]
fn both_monitors_round_trip_on_the_request() -> Fallible {
    for vmm in VmmIdentity::BOTH {
        let request = capsule_request(2, vmm)?;
        let decoded = CapsuleRequest::decode(&encoded_request(&request)?)?;
        assert_eq!(decoded.vmm, vmm);
        assert!(encoded_request(&decoded)?.contains(vmm.tag()));
    }
    Ok(())
}

/// Positive: both readings of decision D04 are admitted, and the payload says
/// which one it arrived on.
#[test]
fn both_admission_paths_are_admitted() -> Fallible {
    for admission in SandboxAdmissionPath::BOTH {
        let mut request = capsule_request(3, VmmIdentity::Firecracker)?;
        request.admission = admission;
        let decoded = CapsuleRequest::decode(&encoded_request(&request)?)?;
        assert_eq!(decoded.admission, admission);
        assert_eq!(
            decoded.admission_is_in_graph_of_record(),
            admission == SandboxAdmissionPath::TransitiveThroughMinerva
        );
    }
    Ok(())
}

/// Positive: the schema and edge constants agree with the register.
#[test]
fn the_schema_and_edge_constants_agree() {
    assert_eq!(CapsuleRequest::SCHEMA, SchemaId::CapsuleRequest);
    assert_eq!(CapsuleRequest::EDGE, EdgeId::ExecuteWasmedCapsule);
    assert_eq!(CapsuleRequest::RUNTIME, AdmittedRuntime::SETTLED);
    assert_eq!(SchemaId::CapsuleRequest.edge(), CapsuleRequest::EDGE);
    assert_eq!(
        SchemaId::CapsuleRequest.tag(),
        "aegis.p09-p10.capsule-request.v1"
    );
    assert_eq!(
        SchemaId::CapsuleRequest.to_string(),
        SchemaId::CapsuleRequest.tag()
    );
}

/// Positive: the edge is recorded verbatim from the graph, transport string
/// included, even where a decision superseded what that string names.
#[test]
fn the_edge_is_recorded_verbatim_from_the_graph() {
    assert_eq!(
        EdgeId::ExecuteWasmedCapsule.name(),
        "EXECUTE_WASMED_CAPSULE"
    );
    assert!(EdgeId::ExecuteWasmedCapsule.in_graph_of_record());
    assert!(
        EdgeId::ExecuteWasmedCapsule
            .recorded_transport()
            .contains("Wazero"),
        "the graph string is recorded verbatim and superseded by D06"
    );
    assert_eq!(EdgeId::ALL.len(), 2);
    assert_eq!(SchemaId::ALL.len(), 2);
}

// --- Negative -------------------------------------------------------------

/// Negative: a payload naming another contract version is refused, and the
/// refusal still names the payload it was about.
#[test]
fn another_contract_version_is_refused() -> Fallible {
    let text = encoded_request(&capsule_request(1, VmmIdentity::Firecracker)?)?;
    let tampered = tamper(&text, "capsule-request.v1", "capsule-request.v2")?;
    let refused = CapsuleRequest::decode(&tampered);
    assert_eq!(
        refused,
        Err(ContractError::UnknownVersion {
            correlation: expected_correlation()?
        })
    );
    Ok(())
}

/// Negative: the runtime decision D06 rejected does not decode.
#[test]
fn the_rejected_runtime_does_not_decode() -> Fallible {
    let text = encoded_request(&capsule_request(1, VmmIdentity::Firecracker)?)?;
    for rejected in ["wazero", "go-wasm-runtime", "out-of-process-adapter"] {
        let tampered = tamper(&text, "rust-native-component-model", rejected)?;
        assert_eq!(
            CapsuleRequest::decode(&tampered),
            Err(ContractError::Malformed {
                correlation: expected_correlation()?
            }),
            "a payload naming {rejected} must not decode"
        );
    }
    Ok(())
}

/// Negative: an unknown field, an unknown capability and an unknown monitor
/// are each refused rather than ignored.
#[test]
fn unknown_fields_and_values_are_refused() -> Fallible {
    let text = encoded_request(&capsule_request(1, VmmIdentity::Firecracker)?)?;
    let cases = [
        tamper(&text, r#""slot":1"#, r#""slot":1,"gpu":true"#)?,
        tamper(&text, r#""filesystem-read""#, r#""filesystem-everything""#)?,
        tamper(&text, r#""firecracker""#, r#""cloud-hypervisor""#)?,
        tamper(&text, r#""edge""#, r#""egde""#)?,
    ];
    for tampered in cases {
        assert_eq!(
            CapsuleRequest::decode(&tampered),
            Err(ContractError::Malformed {
                correlation: expected_correlation()?
            })
        );
    }
    assert!(CapsuleRequest::decode("not json at all").is_err());
    Ok(())
}

/// Negative: a request that names the other edge this crate carries is
/// refused by `validate`, not by the decoder.
#[test]
fn a_request_on_the_other_edge_is_refused() -> Fallible {
    let mut request = capsule_request(1, VmmIdentity::Firecracker)?;
    request.edge = other_edge(CapsuleRequest::EDGE);
    assert_eq!(
        request.validate(),
        Err(ContractError::WrongEdge {
            correlation: expected_correlation()?
        })
    );
    let mut buffer = PayloadBuffer::new();
    let refused = request.encode_into(&mut buffer);
    assert!(refused.is_err(), "an invalid request does not encode");
    assert!(buffer.is_empty());
    assert_eq!(buffer.len(), 0);
    assert_eq!(buffer.as_bytes(), &[] as &[u8]);
    assert_eq!(buffer.as_str(), Some(""));
    let error = ContractError::WrongEdge {
        correlation: expected_correlation()?,
    };
    assert_eq!(error.correlation(), expected_correlation()?);
    assert!(error.to_string().contains("does not travel"));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: a request at the capsule bound is accepted and one over is
/// rejected, on the wire as well as in the constructor.
#[test]
fn a_request_at_the_capsule_bound_is_accepted_and_one_over_is_not() -> Fallible {
    let at_bound = capsule_request(MAX_WASM_CAPSULES, VmmIdentity::Firecracker)?;
    let text = encoded_request(&at_bound)?;
    assert!(text.contains(r#""slot":128"#));
    let decoded = CapsuleRequest::decode(&text)?;
    assert_eq!(decoded.slot.get(), MAX_WASM_CAPSULES);

    let over = tamper(&text, r#""slot":128"#, r#""slot":129"#)?;
    assert_eq!(
        CapsuleRequest::decode(&over),
        Err(ContractError::Malformed {
            correlation: expected_correlation()?
        }),
        "the slot bound holds on the wire, not only in the table"
    );
    let under = tamper(&text, r#""slot":128"#, r#""slot":0"#)?;
    assert!(CapsuleRequest::decode(&under).is_err());
    Ok(())
}

/// Boundary: a capability list at its entry bound is admitted and one entry
/// past it is refused, so a payload cannot make the decoder walk an unbounded
/// sequence.
#[test]
fn the_capability_list_is_bounded_at_its_entry_count() -> Fallible {
    let mut all = capsule_request(1, VmmIdentity::Firecracker)?;
    all.capabilities = Capability::ALL
        .into_iter()
        .fold(CapabilitySet::new(), CapabilitySet::with);
    let text = encoded_request(&all)?;
    let decoded = CapsuleRequest::decode(&text)?;
    assert_eq!(decoded.capabilities.count(), MAX_CAPABILITY_ENTRIES);
    assert_eq!(MAX_CAPABILITY_ENTRIES, Capability::ALL.len());

    let over = tamper(
        &text,
        r#"["network","filesystem-read","filesystem-write","zenoh-ipc"]"#,
        r#"["network","filesystem-read","filesystem-write","zenoh-ipc","network"]"#,
    )?;
    assert_eq!(
        CapsuleRequest::decode(&over),
        Err(ContractError::Malformed {
            correlation: expected_correlation()?
        }),
        "a fifth entry is refused even though it repeats a granted capability"
    );

    let mut none = capsule_request(1, VmmIdentity::Firecracker)?;
    none.capabilities = CapabilitySet::NONE;
    let empty = encoded_request(&none)?;
    assert!(empty.contains(r#""capabilities":[]"#));
    assert!(CapsuleRequest::decode(&empty)?.capabilities.is_empty());
    Ok(())
}

/// Boundary: a payload one byte past the contract bound is refused before it
/// is parsed, and the refusal is anonymous because nothing was read.
#[test]
fn a_payload_past_the_byte_bound_is_refused_unparsed() -> Fallible {
    let filler = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(1));
    let refused = CapsuleRequest::decode(&filler);
    assert_eq!(
        refused,
        Err(ContractError::PayloadTooLong {
            correlation: Correlation::new(SchemaId::CapsuleRequest, None),
            max: MAX_CONTRACT_PAYLOAD_BYTES,
        })
    );
    if let Err(error) = refused {
        assert_eq!(error.correlation().id(), None);
        assert!(error.to_string().contains("uncorrelated"));
    }
    let inside = encoded_request(&capsule_request(1, VmmIdentity::Firecracker)?)?;
    assert!(inside.len() < MAX_CONTRACT_PAYLOAD_BYTES);
    Ok(())
}
