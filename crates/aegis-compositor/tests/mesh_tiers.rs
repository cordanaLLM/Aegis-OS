// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Epics E07-1 and E07-2, the mesh halves: the Tier-1 client bound and the
//! mocked Tier-2 transport.
//!
//! Positive: 64 clients are admitted, a publication round-trips to a
//! subscriber on the same key. **Negative: the 65th client is not admitted**,
//! a subscriber cannot publish, and an empty key expression is rejected.
//! Boundary: the client table is exact at 64, and the key-expression rules are
//! exact at every shape a separator can take.
//!
//! [`MockedMesh`] is a mock and is documented as one: no session, no network,
//! no shared memory. Nothing here is evidence about any transport's latency,
//! throughput or retroactivity behaviour.

mod common;

use aegis_compositor::{
    ClientId, ClientTable, CompositorError, IdError, KeyExpr, MAX_IPC_CLIENTS, MAX_KEY_EXPR_LEN,
    MAX_MESH_TOPICS, MAX_SAMPLE_BYTES, MAX_SUBSCRIPTIONS, MeshRole, MockedMesh,
    SCAFFOLD_SHM_BUFFER_BYTES, Sample, Subscription, SubscriptionId,
};

use common::{FOCUS_KEY, Fallible, METRICS_KEY, filled_clients, key};

// --- Positive -------------------------------------------------------------

/// Positive: the Tier-1 table admits its full complement of clients.
#[test]
fn the_client_table_admits_sixty_four_clients() -> Fallible {
    let clients = filled_clients(MAX_IPC_CLIENTS)?;
    assert_eq!(clients.count(), MAX_IPC_CLIENTS);
    assert_eq!(MAX_IPC_CLIENTS, 64);
    assert!(!clients.is_empty());
    assert!(clients.holds(ClientId::new(1)));
    assert!(clients.holds(ClientId::new(64)));
    Ok(())
}

/// Positive: a publication round-trips to a subscriber on the same key.
#[test]
fn a_publication_round_trips_to_a_subscriber() -> Fallible {
    let mut mesh = MockedMesh::new();
    assert!(mesh.is_empty());
    let focus = key(FOCUS_KEY)?;
    let reader: Subscription = mesh.declare_subscriber(focus)?;
    assert_eq!(reader.key, focus);
    assert_eq!(reader.id, SubscriptionId::new(1));
    mesh.publish(MeshRole::Publisher, focus, b"focus-change")?;
    let sample: Sample = mesh
        .read(reader)?
        .ok_or("the subscriber must see the publication")?;
    assert_eq!(sample.key(), focus);
    assert_eq!(sample.payload(), b"focus-change");
    assert_eq!(sample.len(), 12);
    assert!(!sample.is_empty());
    assert_eq!(mesh.topics(), 1);
    assert_eq!(mesh.subscriptions(), 1);
    Ok(())
}

/// Positive: republishing on a key replaces the retained sample rather than
/// adding a topic, which is the scaffold's own map behaviour.
#[test]
fn republishing_replaces_the_retained_sample() -> Fallible {
    let mut mesh = common::seeded_mesh()?;
    let focus = key(FOCUS_KEY)?;
    assert_eq!(mesh.topics(), 1);
    mesh.publish(MeshRole::Publisher, focus, b"second")?;
    assert_eq!(mesh.topics(), 1);
    let retained = mesh.retained(focus).ok_or("a sample must be retained")?;
    assert_eq!(retained.payload(), b"second");
    Ok(())
}

/// Positive: the key expressions the report's own topic hierarchy uses parse,
/// and count their segments.
#[test]
fn the_recorded_topic_hierarchy_parses() -> Fallible {
    let focus = key(FOCUS_KEY)?;
    assert_eq!(focus.to_string(), FOCUS_KEY);
    assert_eq!(focus.segments(), 3);
    assert_eq!(focus.len(), FOCUS_KEY.len());
    assert_eq!(focus.as_bytes(), FOCUS_KEY.as_bytes());
    assert!(!focus.is_empty());
    let metrics = key(METRICS_KEY)?;
    assert_eq!(metrics.segments(), 5);
    assert!(KeyExpr::parse("aegis/agents/*/metrics").is_ok());
    assert_eq!(aegis_compositor::KEY_EXPR_SEPARATOR, b'/');
    Ok(())
}

/// Positive: a role says what it may do, and both are named.
#[test]
fn a_role_says_what_it_may_do() {
    assert!(MeshRole::Publisher.may_publish());
    assert!(!MeshRole::Subscriber.may_publish());
    assert_eq!(MeshRole::ALL.len(), 2);
    assert_eq!(MeshRole::Publisher.name(), "publisher");
    assert_eq!(MeshRole::Subscriber.name(), "subscriber");
}

// --- Negative -------------------------------------------------------------

/// Negative: the 65th client is not admitted.
///
/// The scaffold accepts the connection and then drops the stream inside an
/// `if`, so the peer sees a successful connect followed by nothing.
#[test]
fn the_sixty_fifth_client_is_not_admitted() -> Fallible {
    let mut clients = filled_clients(MAX_IPC_CLIENTS)?;
    let refusal = clients.admit();
    assert_eq!(
        refusal,
        Err(CompositorError::ClientTableFull {
            max: MAX_IPC_CLIENTS
        })
    );
    assert_eq!(clients.count(), MAX_IPC_CLIENTS);
    assert!(!clients.holds(ClientId::new(65)));
    Ok(())
}

/// Negative: a subscriber cannot publish.
///
/// REQ-P04-03's immutable-read control: attaching a reader must not give it a
/// way to act back on the stream it is reading.
#[test]
fn a_subscriber_cannot_publish() -> Fallible {
    let mut mesh = common::seeded_mesh()?;
    let focus = key(FOCUS_KEY)?;
    let reader = mesh.declare_subscriber(focus)?;
    assert_eq!(reader.role(), MeshRole::Subscriber);
    assert!(!reader.role().may_publish());
    assert_eq!(
        mesh.publish(reader.role(), focus, b"back-action"),
        Err(CompositorError::RetroactivityRefused)
    );
    let retained = mesh.retained(focus).ok_or("the sample must be unchanged")?;
    assert_eq!(retained.payload(), b"focus-change");
    Ok(())
}

/// Negative: an empty key expression is rejected.
///
/// The scaffold inserts a topic whose name is the empty string and reports
/// success.
#[test]
fn an_empty_key_expression_is_rejected() {
    assert_eq!(KeyExpr::parse(""), Err(IdError::Empty));
    assert_eq!(KeyExpr::parse("/"), Err(IdError::EmptyKeySegment));
    assert_eq!(KeyExpr::parse("//"), Err(IdError::EmptyKeySegment));
    assert!(
        IdError::EmptyKeySegment
            .to_string()
            .contains("empty segment")
    );
}

/// Negative: a sample larger than the mock retains is refused, and a
/// subscription the mesh does not hold cannot be read.
#[test]
fn an_over_large_sample_and_an_unknown_subscription_are_refused() -> Fallible {
    let mut mesh = MockedMesh::default();
    let focus = key(FOCUS_KEY)?;
    let payload = vec![0u8; MAX_SAMPLE_BYTES.saturating_add(1)];
    assert_eq!(
        mesh.publish(MeshRole::Publisher, focus, &payload),
        Err(CompositorError::SampleTooLarge {
            bytes: MAX_SAMPLE_BYTES.saturating_add(1),
            max: MAX_SAMPLE_BYTES,
        })
    );
    assert!(mesh.is_empty());

    let stranger = Subscription {
        id: SubscriptionId::new(99),
        key: focus,
    };
    assert!(!mesh.holds_subscription(stranger.id));
    assert_eq!(
        mesh.read(stranger),
        Err(CompositorError::UnknownSubscription { id: 99 })
    );
    Ok(())
}

/// Negative: a key expression carrying a byte the mesh does not admit is
/// refused, never repaired.
#[test]
fn a_key_expression_outside_the_set_is_refused() {
    for raw in ["aegis compositor/focus", "aegis/focus!", "aegis/caf\u{e9}"] {
        assert_eq!(KeyExpr::parse(raw), Err(IdError::Charset), "{raw}");
    }
    assert_eq!(
        KeyExpr::parse(&"a".repeat(MAX_KEY_EXPR_LEN.saturating_add(1))),
        Err(IdError::TooLong {
            max: MAX_KEY_EXPR_LEN,
            actual: MAX_KEY_EXPR_LEN.saturating_add(1),
        })
    );
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the client table is exact at 64.
#[test]
fn the_client_bound_is_exact_at_sixty_four() -> Fallible {
    let mut clients = filled_clients(MAX_IPC_CLIENTS.saturating_sub(1))?;
    assert_eq!(clients.count(), 63);
    let last = clients.admit()?;
    assert_eq!(last, ClientId::new(64));
    assert_eq!(clients.count(), MAX_IPC_CLIENTS);
    assert!(clients.admit().is_err());
    assert!(ClientTable::new().is_empty());
    Ok(())
}

/// Boundary: the key-expression rules are exact at every shape a separator can
/// take, and at the length bound.
#[test]
fn the_key_expression_rules_are_exact_at_every_separator_shape() {
    for bad in ["/aegis", "aegis/", "aegis//focus", "/", "//"] {
        assert_eq!(
            KeyExpr::parse(bad),
            Err(IdError::EmptyKeySegment),
            "{bad} must be refused as an empty segment"
        );
    }
    assert!(KeyExpr::parse("a").is_ok());
    assert!(KeyExpr::parse("a/b").is_ok());
    let longest = "k".repeat(MAX_KEY_EXPR_LEN);
    assert_eq!(
        KeyExpr::parse(&longest).map(|k| k.len()),
        Ok(MAX_KEY_EXPR_LEN)
    );
    assert_eq!(MAX_KEY_EXPR_LEN, 96);
}

/// Boundary: the retained-sample table is exact at its bound, and the mock's
/// own payload bound is deliberately smaller than the one the scaffold
/// declares.
#[test]
fn the_mesh_bounds_are_exact_and_distinct() -> Fallible {
    let mut mesh = MockedMesh::new();
    for index in 0..MAX_MESH_TOPICS {
        let expression = format!("aegis/topic/n{index}");
        mesh.publish(MeshRole::Publisher, key(&expression)?, b"x")?;
    }
    assert_eq!(mesh.topics(), MAX_MESH_TOPICS);
    assert_eq!(
        mesh.publish(MeshRole::Publisher, key("aegis/topic/overflow")?, b"x"),
        Err(CompositorError::MeshTopicTableFull {
            max: MAX_MESH_TOPICS
        })
    );
    // The mock retains less than the scaffold declares, and says which is
    // which: the smaller figure is this crate's own storage bound and the
    // larger one is the recorded payload limit.
    assert_eq!((MAX_SAMPLE_BYTES, SCAFFOLD_SHM_BUFFER_BYTES), (1024, 4096));
    assert_eq!(
        MAX_SAMPLE_BYTES.min(SCAFFOLD_SHM_BUFFER_BYTES),
        MAX_SAMPLE_BYTES
    );
    assert_eq!(MAX_SUBSCRIPTIONS, 8);
    Ok(())
}

/// Boundary: a payload of exactly the retained bound is accepted and reads
/// back whole.
#[test]
fn a_payload_at_the_retained_bound_is_accepted() -> Fallible {
    let mut mesh = MockedMesh::new();
    let focus = key(FOCUS_KEY)?;
    let payload = vec![7u8; MAX_SAMPLE_BYTES];
    mesh.publish(MeshRole::Publisher, focus, &payload)?;
    let sample = mesh.retained(focus).ok_or("a sample must be retained")?;
    assert_eq!(sample.len(), MAX_SAMPLE_BYTES);
    assert_eq!(sample.payload(), payload.as_slice());
    Ok(())
}
