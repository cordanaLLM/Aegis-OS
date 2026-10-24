// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Test fixtures shared across aegis-forum-shell-gpui integration tests.

#![allow(dead_code)]

use aegis_scaena::MountError;
use aegis_scaena::mount::{
    Anchor, Edge, KeyboardInteractivity, Layer, Margin, MountCanvas, MountSchema, Namespace,
};

/// A test canvas anchored Top, Left, and Right with configurable exclusive zone and margin.
pub fn sample_mount_canvas(
    exclusive_zone: Option<i32>,
    margin: Option<Margin>,
) -> Result<MountCanvas, MountError> {
    Ok(MountCanvas {
        schema: MountSchema::V1,
        namespace: Namespace::parse(b"aegis-forum")?,
        layer: Layer::Top,
        anchor: Anchor::NONE
            .with(Edge::Top)
            .with(Edge::Left)
            .with(Edge::Right),
        exclusive_zone,
        exclusive_edge: Some(Edge::Top),
        margin,
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
    })
}
