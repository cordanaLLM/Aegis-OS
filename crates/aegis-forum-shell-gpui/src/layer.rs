// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P17 `MOUNT_SHELL_CANVAS` parameters mapped onto gpui's layer-shell
//! options (`crates/gpui/src/platform/layer_shell.rs` at the pinned commit).
//!
//! The mapping is total over a canvas that passed
//! [`MountCanvas::check`](aegis_scaena::mount::MountCanvas::check): it runs the
//! check again first and refuses what fails it, so no value the decoder would
//! refuse reaches the compositor through this path. Each extent is narrowed
//! through `i16`, which holds `-1..=MAX_EXTENT` exactly, so the conversion to
//! gpui's `f32` pixels is lossless.

use aegis_scaena::mount::{self, Edge, MountCanvas};
use gpui::layer_shell::{Anchor, KeyboardInteractivity, Layer, LayerShellOptions};
use gpui::{Pixels, px};

/// Why a canvas could not be mapped.
#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum LayerMapError {
    /// The canvas fails the contract's own check.
    #[error("the canvas fails the mount contract: {0}")]
    Contract(#[from] aegis_scaena::MountError),
    /// An extent does not fit the lossless `i16` path.
    #[error("the extent {0} does not fit the lossless pixel conversion")]
    Extent(i32),
}

/// Maps `canvas` onto gpui's layer-shell options, after re-running its check.
///
/// # Errors
///
/// [`LayerMapError::Contract`] when the canvas fails
/// [`MountCanvas::check`]; [`LayerMapError::Extent`] when an extent does not
/// fit `i16`, which a checked canvas never yields.
pub fn layer_shell_options(canvas: &MountCanvas) -> Result<LayerShellOptions, LayerMapError> {
    canvas.check()?;
    let exclusive_zone = canvas.exclusive_zone.map(pixels).transpose()?;
    let margin = match canvas.margin {
        Some(m) => Some((
            pixels(m.top)?,
            pixels(m.right)?,
            pixels(m.bottom)?,
            pixels(m.left)?,
        )),
        None => None,
    };
    Ok(LayerShellOptions {
        namespace: canvas.namespace.as_str().to_owned(),
        layer: layer(canvas.layer),
        anchor: anchor(canvas.anchor),
        exclusive_zone,
        exclusive_edge: canvas.exclusive_edge.map(edge),
        margin,
        keyboard_interactivity: keyboard(canvas.keyboard_interactivity),
    })
}

/// One extent as gpui pixels, through `i16` so the conversion is exact.
fn pixels(value: i32) -> Result<Pixels, LayerMapError> {
    let narrow = i16::try_from(value).map_err(|_| LayerMapError::Extent(value))?;
    Ok(px(f32::from(narrow)))
}

/// The contract's layer as gpui's.
const fn layer(value: mount::Layer) -> Layer {
    match value {
        mount::Layer::Background => Layer::Background,
        mount::Layer::Bottom => Layer::Bottom,
        mount::Layer::Top => Layer::Top,
        mount::Layer::Overlay => Layer::Overlay,
    }
}

/// One contract edge as gpui's single-edge anchor.
const fn edge(value: Edge) -> Anchor {
    match value {
        Edge::Top => Anchor::TOP,
        Edge::Bottom => Anchor::BOTTOM,
        Edge::Left => Anchor::LEFT,
        Edge::Right => Anchor::RIGHT,
    }
}

/// The contract's anchor set as gpui's. Both carry the wlr-layer-shell bit
/// values, and every edge is mapped by name so neither side's bit layout is
/// assumed.
fn anchor(value: mount::Anchor) -> Anchor {
    [Edge::Top, Edge::Bottom, Edge::Left, Edge::Right]
        .into_iter()
        .filter(|&e| value.contains(e))
        .fold(Anchor::empty(), |set, e| set | edge(e))
}

/// The contract's keyboard interactivity as gpui's.
const fn keyboard(value: mount::KeyboardInteractivity) -> KeyboardInteractivity {
    match value {
        mount::KeyboardInteractivity::None => KeyboardInteractivity::None,
        mount::KeyboardInteractivity::Exclusive => KeyboardInteractivity::Exclusive,
        mount::KeyboardInteractivity::OnDemand => KeyboardInteractivity::OnDemand,
    }
}
