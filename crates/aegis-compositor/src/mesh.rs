// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The tri-tier agent mesh, as far as it can be checked without a transport
//! (REQ-P04-02, REQ-P04-03, REQ-P04-06).
//!
//! The P04 report describes three tiers: `AF_UNIX` streams for shell
//! synchronisation, an Eclipse Zenoh shared-memory pub/sub core with
//! immutable-read controls, and Model Context Protocol sidecars. This module
//! carries the two halves a milestone with no transport can state:
//!
//! * **Tier 1** is [`ClientTable`], a bounded table of admitted clients. The
//!   scaffold accepts a connection past its bound and then drops it silently;
//!   here the 65th admission is [`CompositorError::ClientTableFull`].
//! * **Tier 2** is [`MockedMesh`], an in-process retained-sample table. It is
//!   a **mock**, said plainly: there is no session, no network, no shared
//!   memory segment and no Zenoh dependency. What it models is the one
//!   behaviour the requirement set can be tested against without a transport
//!   -- that a publication reaches a subscription on the same key, and that
//!   attaching a subscriber gives it no way to act back on the stream.
//!
//! Tier 3 is not modelled at all. A sidecar dispatcher is a process launcher,
//! and dispute DSP-21 leaves it unclear whether P04 even owns that dispatch;
//! [`crate::register`] records it rather than inventing an owner.
//!
//! # Immutable reads, and what the test can show
//!
//! REQ-P04-03 requires immutable-read controls so that a newly attached agent
//! cannot back-act on the upstream sensorimotor state. [`MeshRole`] is that
//! control: [`MockedMesh::publish`] takes the role of the caller, and a
//! [`MeshRole::Subscriber`] is refused with
//! [`CompositorError::RetroactivityRefused`]. A [`Subscription`] hands back
//! only [`MeshRole::Subscriber`], so a subscriber has no route to a publishing
//! role through this API.
//!
//! What that shows is that *this* mesh refuses the write. It is not evidence
//! about Zenoh, about shared memory, or about any real transport's
//! retroactivity behaviour; [`crate::decision`] records why no such transport
//! is admitted here.
//!
//! # Bounds
//!
//! The scaffold declares a 4096-byte shared-memory payload limit, recorded as
//! [`SCAFFOLD_SHM_BUFFER_BYTES`]. This mock's own retained-sample bound is
//! smaller, [`MAX_SAMPLE_BYTES`], because it stores every retained sample by
//! value in a fixed array and the whole table is carried by value. That bound
//! is this crate's and is not a recorded one.

use crate::error::CompositorError;
use crate::id::KeyExpr;

/// Scalar upper bound on the Tier-1 client table.
///
/// The scaffold's `MAX_IPC_CLIENTS` (export-027 `f19640d7a7da`).
pub const MAX_IPC_CLIENTS: usize = 64;

/// The shared-memory payload limit the scaffold declares, in bytes.
///
/// Recorded from export-027 `f19640d7a7da`, where `BUFFER_SIZE` is 4096 and a
/// longer payload is refused. Recorded, not used: this mock's own bound is
/// [`MAX_SAMPLE_BYTES`].
pub const SCAFFOLD_SHM_BUFFER_BYTES: usize = 4096;

/// Scalar upper bound on the bytes one retained sample may carry.
///
/// This crate's bound, not a recorded one. See the module documentation.
pub const MAX_SAMPLE_BYTES: usize = 1024;

/// Scalar upper bound on the topics the mock retains a sample for.
pub const MAX_MESH_TOPICS: usize = 8;

/// Scalar upper bound on the subscriptions the mock holds.
pub const MAX_SUBSCRIPTIONS: usize = 8;

/// A Tier-1 client identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ClientId(u32);

impl ClientId {
    /// Names a client by its raw identifier.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// The bounded Tier-1 client table.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClientTable {
    admitted: [Option<ClientId>; MAX_IPC_CLIENTS],
    count: usize,
    next: u32,
}

impl Default for ClientTable {
    fn default() -> Self {
        Self::new()
    }
}

impl ClientTable {
    /// Builds an empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            admitted: [None; MAX_IPC_CLIENTS],
            count: 0,
            next: 1,
        }
    }

    /// Admits one client and returns its identifier.
    ///
    /// # Errors
    ///
    /// Returns [`CompositorError::ClientTableFull`] at [`MAX_IPC_CLIENTS`].
    /// The scaffold accepts the connection first and drops it inside an `if`,
    /// so the peer sees a successful connect and then nothing.
    pub fn admit(&mut self) -> Result<ClientId, CompositorError> {
        if self.count >= MAX_IPC_CLIENTS {
            return Err(CompositorError::ClientTableFull {
                max: MAX_IPC_CLIENTS,
            });
        }
        let id = ClientId::new(self.next);
        let cell = self
            .admitted
            .get_mut(self.count)
            .ok_or(CompositorError::ClientTableFull {
                max: MAX_IPC_CLIENTS,
            })?;
        *cell = Some(id);
        self.count = self.count.saturating_add(1);
        self.next = self.next.saturating_add(1);
        Ok(id)
    }

    /// Returns `true` when the table holds `id`.
    #[must_use]
    pub fn holds(&self, id: ClientId) -> bool {
        self.admitted
            .iter()
            .take(MAX_IPC_CLIENTS)
            .flatten()
            .any(|held| *held == id)
    }

    /// Returns how many clients the table holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns `true` when the table holds no client.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }
}

/// What a mesh participant is allowed to do.
///
/// REQ-P04-03's immutable-read control, as a type. A subscriber holds only
/// [`Self::Subscriber`], and this API offers no way to turn one into the
/// other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum MeshRole {
    /// May publish on a key expression.
    Publisher,
    /// May read a retained sample and nothing else.
    Subscriber,
}

impl MeshRole {
    /// Both roles.
    pub const ALL: [Self; 2] = [Self::Publisher, Self::Subscriber];

    /// Returns the stable name this role is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Publisher => "publisher",
            Self::Subscriber => "subscriber",
        }
    }

    /// Returns `true` when the role may publish.
    #[must_use]
    pub const fn may_publish(self) -> bool {
        matches!(self, Self::Publisher)
    }
}

/// A subscription identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct SubscriptionId(u32);

impl SubscriptionId {
    /// Names a subscription by its raw identifier.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One declared subscription.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Subscription {
    /// The subscription identifier.
    pub id: SubscriptionId,
    /// The key expression the subscription reads.
    pub key: KeyExpr,
}

impl Subscription {
    /// Returns the role a subscription holds, which is never a publishing one.
    #[must_use]
    pub const fn role(self) -> MeshRole {
        MeshRole::Subscriber
    }
}

/// One retained sample.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Sample {
    key: KeyExpr,
    bytes: [u8; MAX_SAMPLE_BYTES],
    len: usize,
}

impl Sample {
    /// Returns the key expression the sample was published on.
    #[must_use]
    pub const fn key(&self) -> KeyExpr {
        self.key
    }

    /// Returns the payload bytes.
    #[must_use]
    pub fn payload(&self) -> &[u8] {
        self.bytes.get(..self.len).unwrap_or(&[])
    }

    /// Returns the payload length in bytes.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    /// Returns `true` when the sample carries no payload.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }
}

/// An in-process mock of the Tier-2 retained-sample mesh.
///
/// **Not a transport.** See the module documentation.
#[derive(Debug, Clone, Copy)]
pub struct MockedMesh {
    samples: [Option<Sample>; MAX_MESH_TOPICS],
    topics: usize,
    subscriptions: [Option<Subscription>; MAX_SUBSCRIPTIONS],
    subscription_count: usize,
    next_subscription: u32,
}

impl Default for MockedMesh {
    fn default() -> Self {
        Self::new()
    }
}

impl MockedMesh {
    /// Builds an empty mesh.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            samples: [None; MAX_MESH_TOPICS],
            topics: 0,
            subscriptions: [None; MAX_SUBSCRIPTIONS],
            subscription_count: 0,
            next_subscription: 1,
        }
    }

    /// Publishes `payload` on `key`, as `role`.
    ///
    /// A publication on a key that already has a retained sample replaces it,
    /// which is the scaffold's own behaviour: it inserts into a map keyed by
    /// topic.
    ///
    /// # Errors
    ///
    /// Returns [`CompositorError::RetroactivityRefused`] when `role` may not
    /// publish, [`CompositorError::SampleTooLarge`] past
    /// [`MAX_SAMPLE_BYTES`], and [`CompositorError::MeshTopicTableFull`] when
    /// a new key would exceed [`MAX_MESH_TOPICS`].
    pub fn publish(
        &mut self,
        role: MeshRole,
        key: KeyExpr,
        payload: &[u8],
    ) -> Result<(), CompositorError> {
        if !role.may_publish() {
            return Err(CompositorError::RetroactivityRefused);
        }
        if payload.len() > MAX_SAMPLE_BYTES {
            return Err(CompositorError::SampleTooLarge {
                bytes: payload.len(),
                max: MAX_SAMPLE_BYTES,
            });
        }
        let mut bytes = [0u8; MAX_SAMPLE_BYTES];
        for (target, byte) in bytes.iter_mut().zip(payload.iter()) {
            *target = *byte;
        }
        let sample = Sample {
            key,
            bytes,
            len: payload.len(),
        };
        if let Some(slot) = self.slot_of(key) {
            let cell = self
                .samples
                .get_mut(slot)
                .ok_or(CompositorError::MeshTopicTableFull {
                    max: MAX_MESH_TOPICS,
                })?;
            *cell = Some(sample);
            return Ok(());
        }
        if self.topics >= MAX_MESH_TOPICS {
            return Err(CompositorError::MeshTopicTableFull {
                max: MAX_MESH_TOPICS,
            });
        }
        let cell =
            self.samples
                .get_mut(self.topics)
                .ok_or(CompositorError::MeshTopicTableFull {
                    max: MAX_MESH_TOPICS,
                })?;
        *cell = Some(sample);
        self.topics = self.topics.saturating_add(1);
        Ok(())
    }

    /// Declares a subscription on `key`.
    ///
    /// # Errors
    ///
    /// Returns [`CompositorError::MeshTopicTableFull`] at
    /// [`MAX_SUBSCRIPTIONS`].
    pub fn declare_subscriber(&mut self, key: KeyExpr) -> Result<Subscription, CompositorError> {
        if self.subscription_count >= MAX_SUBSCRIPTIONS {
            return Err(CompositorError::MeshTopicTableFull {
                max: MAX_SUBSCRIPTIONS,
            });
        }
        let subscription = Subscription {
            id: SubscriptionId::new(self.next_subscription),
            key,
        };
        let cell = self.subscriptions.get_mut(self.subscription_count).ok_or(
            CompositorError::MeshTopicTableFull {
                max: MAX_SUBSCRIPTIONS,
            },
        )?;
        *cell = Some(subscription);
        self.subscription_count = self.subscription_count.saturating_add(1);
        self.next_subscription = self.next_subscription.saturating_add(1);
        Ok(subscription)
    }

    /// Returns the sample a subscription can read, when there is one.
    ///
    /// # Errors
    ///
    /// Returns [`CompositorError::UnknownSubscription`] when the mesh holds no
    /// such subscription, which is what distinguishes "nothing published yet"
    /// from "you are reading a subscription that does not exist".
    pub fn read(&self, subscription: Subscription) -> Result<Option<Sample>, CompositorError> {
        if !self.holds_subscription(subscription.id) {
            return Err(CompositorError::UnknownSubscription {
                id: subscription.id.get(),
            });
        }
        Ok(self.retained(subscription.key))
    }

    /// Returns the retained sample on `key`, when there is one.
    #[must_use]
    pub fn retained(&self, key: KeyExpr) -> Option<Sample> {
        self.samples
            .iter()
            .take(MAX_MESH_TOPICS)
            .flatten()
            .find(|sample| sample.key == key)
            .copied()
    }

    /// Returns `true` when the mesh holds subscription `id`.
    #[must_use]
    pub fn holds_subscription(&self, id: SubscriptionId) -> bool {
        self.subscriptions
            .iter()
            .take(MAX_SUBSCRIPTIONS)
            .flatten()
            .any(|held| held.id == id)
    }

    /// Returns how many keys hold a retained sample.
    #[must_use]
    pub const fn topics(&self) -> usize {
        self.topics
    }

    /// Returns how many subscriptions the mesh holds.
    #[must_use]
    pub const fn subscriptions(&self) -> usize {
        self.subscription_count
    }

    /// Returns `true` when no key holds a retained sample.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.topics == 0
    }

    /// Returns the slot `key` already occupies, when it occupies one.
    fn slot_of(&self, key: KeyExpr) -> Option<usize> {
        self.samples
            .iter()
            .take(MAX_MESH_TOPICS)
            .position(|cell| cell.is_some_and(|sample| sample.key == key))
    }
}
