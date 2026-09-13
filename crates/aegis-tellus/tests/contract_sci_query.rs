// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E05-4, the P16-to-P13 edge: the query and the response it is answered with.

mod common;

use aegis_tellus::{
    CandidateId, CandidateList, CandidateSciQuery, CandidateSciResponse, ContractError,
    Correlation, EdgeId, EnergyKwh, GridIntensity, MAX_CONTRACT_PAYLOAD_BYTES,
    MAX_QUERY_CANDIDATES, PayloadBuffer, RateEntry, RateList, SchemaId, SciEngine, SciQueryVersion,
    SciRate, SciResponseVersion,
};

use common::{CANDIDATE, CORRELATION, EPSILON, Fallible, candidate, correlation, query, response};

// --- Positive -------------------------------------------------------------

/// Positive: a query round-trips through its own encoder and decoder.
#[test]
fn a_query_round_trips() -> Fallible {
    let payload = query()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?;
    assert!(text.contains(CORRELATION));
    assert!(text.contains(CANDIDATE));
    assert!(text.contains(SchemaId::CandidateSciQuery.tag()));
    assert_eq!(CandidateSciQuery::decode(text)?, payload);
    assert_eq!(CandidateSciQuery::SCHEMA, SchemaId::CandidateSciQuery);
    assert_eq!(CandidateSciQuery::EDGE, EdgeId::EvaluateCandidateCarbonSci);
    Ok(())
}

/// Positive: a response round-trips and answers the query it names.
#[test]
fn a_response_round_trips_and_answers_its_query() -> Fallible {
    let asked = query()?;
    let answer = response()?;
    let mut buffer = PayloadBuffer::default();
    let text = answer.encode_into(&mut buffer)?;
    let decoded = CandidateSciResponse::decode(text)?;
    assert_eq!(decoded, answer);
    decoded.answers(&asked)?;
    assert_eq!(CandidateSciResponse::SCHEMA, SchemaId::CandidateSciResponse);
    assert_eq!(
        CandidateSciResponse::EDGE,
        EdgeId::EvaluateCandidateCarbonSci
    );

    let entry = decoded
        .rates
        .get(0)
        .ok_or("the response must carry a rate")?;
    assert_eq!(entry.candidate, candidate()?);
    assert!((entry.rate.get() - 0.6).abs() < EPSILON);
    assert!(!entry.fell_back);
    assert_eq!(decoded.intensity, GridIntensity::DEFAULT);
    assert!(!decoded.deferred);
    Ok(())
}

/// Positive: a correlation renders both with and without an identifier.
#[test]
fn a_correlation_renders_with_and_without_an_identifier() -> Fallible {
    let named = Correlation::new(SchemaId::CandidateSciQuery, Some(correlation()?));
    assert_eq!(named.schema(), SchemaId::CandidateSciQuery);
    assert_eq!(named.id(), Some(correlation()?));
    assert!(format!("{named}").contains(CORRELATION));

    let anonymous = Correlation::new(SchemaId::TaskShiftDirective, None);
    assert!(format!("{anonymous}").contains("uncorrelated"));
    assert_eq!(SchemaId::ALL.len(), 3);
    for schema in SchemaId::ALL {
        assert!(!schema.tag().is_empty());
        assert!(!schema.edge().name().is_empty());
    }
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a malformed query payload is refused, and the refusal is correlated.
#[test]
fn a_malformed_query_is_refused() -> Fallible {
    let payload = query()?;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?.to_owned();

    let wrong_version = text.replace(
        SchemaId::CandidateSciQuery.tag(),
        "aegis.p16-p13.sci-query.v2",
    );
    let refusal = CandidateSciQuery::decode(&wrong_version);
    assert!(matches!(refusal, Err(ContractError::UnknownVersion { .. })));
    assert_eq!(
        refusal.err().map(|error| error.correlation().id()),
        Some(Some(correlation()?)),
        "a refusal must name the conversation it belonged to"
    );

    for bad in [
        "{}",
        "not json at all",
        "[]",
        &text.replace("\"energy-kwh\"", "\"energy_kwh\""),
        &text.replace(&format!("\"{CANDIDATE}\""), "\"\""),
        &text.replace("\"energy-kwh\":0.0025", "\"energy-kwh\":-1.0"),
        &text.replace('}', ",\"extra\":1}"),
    ] {
        assert!(
            CandidateSciQuery::decode(bad).is_err(),
            "the decoder accepted {bad}"
        );
    }
    Ok(())
}

/// Negative: a query naming another edge is refused.
#[test]
fn a_query_naming_another_edge_is_refused() -> Fallible {
    let mut payload = query()?;
    payload.edge = EdgeId::EmitCarbonTelemetry;
    let refusal = payload.validate();
    assert!(matches!(refusal, Err(ContractError::WrongEdge { .. })));

    let mut buffer = PayloadBuffer::new();
    assert!(payload.encode_into(&mut buffer).is_err());
    Ok(())
}

/// Negative: a response that does not answer its query is refused.
#[test]
fn a_response_that_does_not_answer_its_query_is_refused() -> Fallible {
    let mut asked = query()?;
    asked
        .candidates
        .push(CandidateId::parse("candidate-002")?)?;
    let answer = response()?;
    let refusal = answer.answers(&asked);
    assert!(matches!(
        refusal,
        Err(ContractError::UnansweredQuery {
            asked: 2,
            answered: 1,
            ..
        })
    ));

    let mut mismatched = response()?;
    mismatched.correlation_id = aegis_tellus::CorrelationId::parse("m05-fixture-0002")?;
    assert!(mismatched.answers(&query()?).is_err());
    Ok(())
}

/// Negative: a response whose rate names another candidate is refused.
#[test]
fn a_response_answering_the_wrong_candidate_is_refused() -> Fallible {
    let engine = SciEngine::new();
    let sci = engine.sci_rate(EnergyKwh::new(0.0025)?, 1.0);
    let mut rates = RateList::new();
    rates.push(RateEntry::new(
        CandidateId::parse("candidate-999")?,
        sci.as_rate()?,
        false,
    ))?;
    let answer = CandidateSciResponse {
        schema: SciResponseVersion::V1,
        edge: EdgeId::EvaluateCandidateCarbonSci,
        correlation_id: correlation()?,
        intensity: engine.intensity(),
        rates,
        deferred: false,
    };
    assert!(matches!(
        answer.answers(&query()?),
        Err(ContractError::UnansweredQuery { .. })
    ));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: an empty candidate list is handled explicitly, not answered.
///
/// An empty answer and "no candidate exceeded its bound" are the same bytes to
/// a consumer that does not look, and P16 promotes on that answer, so the
/// question is refused at the boundary instead.
#[test]
fn an_empty_candidate_list_is_refused_explicitly() -> Fallible {
    let mut payload = query()?;
    payload.candidates = CandidateList::new();
    assert!(payload.candidates.is_empty());
    assert_eq!(payload.candidates.len(), 0);

    let refusal = payload.validate();
    assert!(
        matches!(refusal, Err(ContractError::EmptyCandidateList { .. })),
        "an empty list must be its own refusal, not a malformed payload"
    );

    let mut buffer = PayloadBuffer::new();
    assert!(payload.encode_into(&mut buffer).is_err());

    let empty_on_the_wire = r#"{"schema":"aegis.p16-p13.sci-query.v1","edge":"EVALUATE_CANDIDATE_CARBON_SCI","correlation-id":"m05-fixture-0001","candidates":[],"energy-kwh":0.0025,"functional-units":1.0}"#;
    assert!(matches!(
        CandidateSciQuery::decode(empty_on_the_wire),
        Err(ContractError::EmptyCandidateList { .. })
    ));

    let mut answer = response()?;
    answer.rates = RateList::new();
    assert!(matches!(
        answer.validate(),
        Err(ContractError::EmptyCandidateList { .. })
    ));
    Ok(())
}

/// Boundary: sixteen candidates are carried and the seventeenth is refused.
#[test]
fn the_candidate_list_turns_at_its_bound() -> Fallible {
    let mut list = CandidateList::default();
    for index in 0..MAX_QUERY_CANDIDATES {
        list.push(CandidateId::parse(&format!("candidate-{index:03}"))?)?;
    }
    assert_eq!(list.len(), MAX_QUERY_CANDIDATES);
    assert!(list.push(CandidateId::parse("one-too-many")?).is_err());
    assert!(list.get(MAX_QUERY_CANDIDATES).is_none());

    let mut payload = query()?;
    payload.candidates = list;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?;
    assert!(text.len() < MAX_CONTRACT_PAYLOAD_BYTES);
    assert_eq!(CandidateSciQuery::decode(text)?.candidates.len(), 16);
    Ok(())
}

/// Boundary: a list of seventeen on the wire is refused rather than truncated.
#[test]
fn seventeen_candidates_on_the_wire_are_refused() {
    let names: Vec<String> = (0..=MAX_QUERY_CANDIDATES)
        .map(|index| format!("\"candidate-{index:03}\""))
        .collect();
    let text = format!(
        r#"{{"schema":"aegis.p16-p13.sci-query.v1","edge":"EVALUATE_CANDIDATE_CARBON_SCI","correlation-id":"m05-fixture-0001","candidates":[{}],"energy-kwh":0.0025,"functional-units":1.0}}"#,
        names.join(",")
    );
    assert!(
        CandidateSciQuery::decode(&text).is_err(),
        "a list past the bound must be refused, not truncated to it"
    );
}

/// Boundary: the byte bound is 4096, a payload of exactly that length is not
/// refused for its length, and one byte more is.
///
/// The refusal is written against the constant rather than against a literal,
/// which is what a refusal should be -- but it means the refusal alone holds
/// for any value of the constant. The recorded figure is asserted here so the
/// number in `planning/roadmap.json` has the same falsifier every other
/// recorded literal in this milestone has.
#[test]
fn a_payload_past_the_byte_bound_is_refused_before_parsing() {
    assert_eq!(
        MAX_CONTRACT_PAYLOAD_BYTES, 4096,
        "the recorded payload bound is 4096 bytes"
    );

    let at_bound = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES);
    assert_eq!(at_bound.len(), MAX_CONTRACT_PAYLOAD_BYTES);
    assert!(
        !matches!(
            CandidateSciQuery::decode(&at_bound),
            Err(ContractError::PayloadTooLong { .. })
        ),
        "exactly the bound is inside it; the payload is refused as malformed, not as too long"
    );

    let oversized = "x".repeat(MAX_CONTRACT_PAYLOAD_BYTES.saturating_add(1));
    let refusal = CandidateSciQuery::decode(&oversized);
    assert!(matches!(
        refusal,
        Err(ContractError::PayloadTooLong {
            max: MAX_CONTRACT_PAYLOAD_BYTES,
            ..
        })
    ));
}

/// Boundary: a functional-unit count of zero travels and falls back on arrival.
///
/// The wire carries a bare `f64` precisely so the tolerated case is
/// representable; a schema that refused it could never exercise the fallback
/// the recorded acceptance asks for.
#[test]
fn a_zero_functional_unit_count_travels_and_falls_back() -> Fallible {
    let mut payload = query()?;
    payload.functional_units = 0.0;
    let mut buffer = PayloadBuffer::new();
    let text = payload.encode_into(&mut buffer)?;
    let decoded = CandidateSciQuery::decode(text)?;
    assert!((decoded.functional_units - 0.0).abs() < EPSILON);

    let sci = SciEngine::new().sci_rate(decoded.energy_kwh, decoded.functional_units);
    assert!(sci.fell_back());
    assert!(!sci.rate().is_nan());
    let rate: SciRate = sci.as_rate()?;
    assert!((rate.get() - 0.6).abs() < EPSILON);
    assert_eq!(SciQueryVersion::V1, decoded.schema);
    Ok(())
}
