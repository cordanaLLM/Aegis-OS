// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! `SYNC_DESKTOP_SHELL`, the P04 to P05 edge: its endpoint and its budget,
//! recorded and not asserted (decision D32, 2026-09-29).
//!
//! The graph of record draws the edge as a Unix socket stream with no path
//! and no budget; export-003 alone names both, and D32 adopts them. Nothing in
//! this crate opens the path or times a hop, and no test here asserts either
//! value, because no real P04 socket exists: a contract test asserts both once
//! one does. Until the hop is measured, [`COMPOSITOR_HOP_TARGET`] is a target,
//! not a claim (ADR-0001).

use core::time::Duration;

/// The Unix socket the compositor serves `SYNC_DESKTOP_SHELL` on (D32).
pub const COMPOSITOR_ENDPOINT: &str = "/run/aegis/compositor.sock";

/// The per-hop target for `SYNC_DESKTOP_SHELL`: under 100 microseconds
/// (D32). A target, not a measured figure.
pub const COMPOSITOR_HOP_TARGET: Duration = Duration::from_micros(100);
