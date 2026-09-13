// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P04 `aegis-compositor`: the compositor's registers, without a
//! display.
//!
//! The imported P04 scaffold (export-027 `f19640d7a7da`) is a daemon that
//! prints that it is initialising a `wlroots` backend it never links, binds a
//! Unix socket, retains "shared-memory" publications in a `HashMap<String,
//! Vec<u8>>`, and sleeps its event loop to a 100 microsecond period under a
//! comment that says 144 Hz. Milestone M07 extracts the part that can be
//! checked without a compositor library, a display, an accelerator or a
//! transport -- the registry, the bounds, the key expressions, the pinned
//! pacing constant and the payloads -- and turns each string refusal into a
//! typed one, which is what HISS-07 asks for.
//!
//! # The four things a reviewer should look at
//!
//! * [`SurfaceRegistry`] holds the registry bound: [`MAX_SURFACES`] surfaces,
//!   the 257th refused, a duplicate identifier refused, and focusing a surface
//!   the registry does not hold refused rather than clearing every focus flag.
//! * [`ClientTable`] holds the Tier-1 bound: [`MAX_IPC_CLIENTS`] clients and
//!   the 65th [`CompositorError::ClientTableFull`], where the scaffold accepts
//!   the connection and drops it in silence.
//! * [`pacing`] pins the frame-pacing constant REQ-P04-08 asks for, states
//!   which of the three recorded figures it is, and says why -- without
//!   claiming any of them was observed.
//! * [`decision`] records D08, settled by ADR-0001 on a **pure Rust**
//!   compositor, and beside it the two upstream dependency checks the ADR and
//!   the milestone ask for: [`ZENOH_CHECK`] and [`COMPOSITOR_LIBRARY_CHECK`],
//!   each with its date, its command, the version upstream reported, and the
//!   reason this milestone admits neither.
//!
//! # What is deliberately not claimed
//!
//! **No latency, jitter, throughput or frame-time figure is produced by this
//! crate.** [`MockedMesh`] is an in-process table, not a transport: it says
//! nothing about the 5 to 35 microseconds REQ-P04-02 targets, and the
//! reference kernel is `PREEMPT_DYNAMIC` rather than `PREEMPT_RT` besides.
//! [`PINNED_FRAME_PERIOD_US`] is a [`DeclaredPeriodUs`] whose `Display` says
//! it is unmeasured, and `tests/declared_literals.rs` fails to compile if it
//! becomes a bare integer. Milestone M12 is the display path and M23 the
//! latency fixtures.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion -- **checked by review, not by a gate**: every function here
//!   is a flat check, a bounded sweep or a field access, and no function in
//!   the crate calls itself directly or through another. This is the one
//!   bullet in this list with no falsifier behind it. The M07 verification
//!   planted direct recursion and mutual recursion in this crate and both
//!   passed `cargo clippy -- -D warnings` and `praetorctl audit`, which
//!   reported no violation, while planted controls for the other invariants
//!   failed as they should. Read it as a reviewed statement about the code as
//!   written, not as a property this repository can currently enforce;
//! * every loop carries a scalar upper bound: [`MAX_SURFACES`],
//!   [`MAX_IPC_CLIENTS`], [`MAX_MESH_TOPICS`], [`MAX_SUBSCRIPTIONS`],
//!   [`MAX_SAMPLE_BYTES`], [`MAX_LABEL_LEN`] and [`MAX_KEY_EXPR_LEN`];
//! * every value on a decision path is `Copy`, so no such path allocates;
//!   `tests/allocation_bounds.rs` is the falsifier, and the one place that is
//!   deliberately not claimed -- a JSON string carrying an escape, which
//!   `serde_json` unescapes into a heap scratch buffer before any field of ours
//!   sees it -- is tested rather than denied;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or unchecked arithmetic,
//!   and no `unsafe` (forbidden at the workspace root).
//!
//! # What this crate does not do
//!
//! It composites nothing and connects to nothing. There is no compositor
//! library, C or Rust; no `wlroots`, `libweston` or `Smithay` dependency; no
//! Wayland connection, `libinput` device, `XKB` map or layer-shell binding; no
//! accelerator, `DRM` device or buffer; no `Zenoh` session, socket, shared
//! memory segment or subprocess; and no clock read anywhere in it.
//! `tests/stubbed_effects.rs` sweeps the crate's own sources for a recorded
//! list of identifiers that would be needed to do any of it and fails if one
//! appears -- a regression gate over an enumeration, not a proof over every
//! such identifier.
//!
//! A pass of this crate's tests is evidence about the registry, the bounds,
//! the key expressions, the pinned constant and the payloads. It closes no
//! display, accelerator, latency or transport gate.
//!
//! # Example
//!
//! ```
//! use aegis_compositor::{
//!     CompositorError, KeyExpr, Label, MeshRole, MockedMesh, PINNED_FRAME_PERIOD_US,
//!     SurfaceGeometry, SurfaceId, SurfaceLayer, SurfaceRegistry, SurfaceRequest,
//!     frame_period_us,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let mut registry = SurfaceRegistry::new();
//! let shell = SurfaceId::new(1);
//! registry.register(SurfaceRequest {
//!     id: shell,
//!     title: Label::parse("forum-desktop-shell")?,
//!     geometry: SurfaceGeometry::new(1920, 1080),
//!     layer: SurfaceLayer::Top,
//!     pid: 1001,
//! })?;
//! assert_eq!(registry.focus(shell)?.pid, 1001);
//! assert!(registry.focus(SurfaceId::new(9)).is_err());
//!
//! // The pacing constant is pinned to the refresh-rate reading.
//! assert_eq!(PINNED_FRAME_PERIOD_US.micros(), frame_period_us(144)?);
//!
//! // A subscriber sees a publication, and cannot make one.
//! let mut mesh = MockedMesh::new();
//! let key = KeyExpr::parse("aegis/compositor/focus")?;
//! let reader = mesh.declare_subscriber(key)?;
//! mesh.publish(MeshRole::Publisher, key, b"focus-change")?;
//! assert_eq!(mesh.read(reader)?.map(|s| s.len()), Some(12));
//! assert!(matches!(
//!     mesh.publish(reader.role(), key, b"back-action"),
//!     Err(CompositorError::RetroactivityRefused)
//! ));
//! assert!(KeyExpr::parse("").is_err());
//! # Ok(())
//! # }
//! ```

pub mod chain;
pub mod decision;
pub mod error;
pub mod id;
pub mod mesh;
pub mod pacing;
pub mod register;
pub mod surface;

pub use crate::chain::{FocusSwitchDraft, OverlayRegistration, accept_dma_buf_stream};
pub use crate::decision::{
    AdmittedBackend, COMPOSITOR_LIBRARY_CHECK, Citation, CompositorImplementation,
    D08_COMPOSITOR_IMPLEMENTATION, DecisionRecord, DecisionState, M07_UPSTREAM_CHECKS,
    UpstreamCheck, ZENOH_CHECK,
};
pub use crate::error::CompositorError;
pub use crate::id::{IdError, KEY_EXPR_SEPARATOR, KeyExpr, Label, MAX_KEY_EXPR_LEN, MAX_LABEL_LEN};
pub use crate::mesh::{
    ClientId, ClientTable, MAX_IPC_CLIENTS, MAX_MESH_TOPICS, MAX_SAMPLE_BYTES, MAX_SUBSCRIPTIONS,
    MeshRole, MockedMesh, SCAFFOLD_SHM_BUFFER_BYTES, Sample, Subscription, SubscriptionId,
};
pub use crate::pacing::{
    DeclaredPeriodUs, MAX_REFRESH_HZ, MIN_REFRESH_HZ, PACING_COMMENT_REFRESH_HZ,
    PINNED_FRAME_PERIOD_US, PacingSource, RENDER_IPC_BUDGET_US, SCAFFOLD_EVENT_LOOP_CYCLES,
    SCAFFOLD_LOOP_PERIOD_US, budget_fits_in_period, frame_period_us,
};
pub use crate::register::{ClaimSource, ClaimStatus, P04_RECORDED_CLAIMS, RecordedClaim};
pub use crate::surface::{
    MAX_SURFACES, SurfaceGeometry, SurfaceId, SurfaceLayer, SurfaceRegistry, SurfaceRequest,
    UNIT_MEMORY_MAX_MIB, WaylandSurface,
};
