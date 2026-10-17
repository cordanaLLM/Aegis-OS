// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The P17 mount parameters mapped onto gpui's layer-shell options: every
//! member reaches gpui by name, a canvas the contract refuses is refused here
//! too, and the extents convert losslessly at both ends of their range.

use aegis_forum_shell_gpui::{LayerMapError, layer_shell_options};
use aegis_scaena::MountError;
use aegis_scaena::mount::{
    Anchor, Edge, KeyboardInteractivity, Layer, MAX_EXTENT, Margin, MountCanvas, MountSchema,
    Namespace,
};
use gpui::layer_shell;
use gpui::px;

fn canvas() -> Result<MountCanvas, MountError> {
    Ok(MountCanvas {
        schema: MountSchema::V1,
        namespace: Namespace::parse(b"aegis-forum")?,
        layer: Layer::Top,
        anchor: Anchor::NONE
            .with(Edge::Top)
            .with(Edge::Left)
            .with(Edge::Right),
        exclusive_zone: Some(32),
        exclusive_edge: Some(Edge::Top),
        margin: Some(Margin {
            top: 1,
            right: 2,
            bottom: 3,
            left: 4,
        }),
        keyboard_interactivity: KeyboardInteractivity::OnDemand,
    })
}

// --- Positive -------------------------------------------------------------

/// Positive: every member reaches gpui with its value.
#[test]
fn every_member_reaches_gpui() -> Result<(), Box<dyn std::error::Error>> {
    let options = layer_shell_options(&canvas()?)?;
    assert_eq!(options.namespace, "aegis-forum");
    assert_eq!(options.layer, layer_shell::Layer::Top);
    assert_eq!(
        options.anchor,
        layer_shell::Anchor::TOP | layer_shell::Anchor::LEFT | layer_shell::Anchor::RIGHT
    );
    assert_eq!(options.exclusive_zone, Some(px(32.0)));
    assert_eq!(options.exclusive_edge, Some(layer_shell::Anchor::TOP));
    assert_eq!(options.margin, Some((px(1.0), px(2.0), px(3.0), px(4.0))));
    assert_eq!(
        options.keyboard_interactivity,
        layer_shell::KeyboardInteractivity::OnDemand
    );
    Ok(())
}

/// Positive: each enumerated value maps to gpui's value of the same name, and
/// each edge to gpui's bit of the same name.
#[test]
fn each_enumerated_value_maps_by_name() -> Result<(), Box<dyn std::error::Error>> {
    let mut c = canvas()?;
    c.exclusive_edge = None;
    for (layer, want) in [
        (Layer::Background, layer_shell::Layer::Background),
        (Layer::Bottom, layer_shell::Layer::Bottom),
        (Layer::Top, layer_shell::Layer::Top),
        (Layer::Overlay, layer_shell::Layer::Overlay),
    ] {
        c.layer = layer;
        assert_eq!(layer_shell_options(&c)?.layer, want);
    }
    for (keyboard, want) in [
        (
            KeyboardInteractivity::None,
            layer_shell::KeyboardInteractivity::None,
        ),
        (
            KeyboardInteractivity::Exclusive,
            layer_shell::KeyboardInteractivity::Exclusive,
        ),
        (
            KeyboardInteractivity::OnDemand,
            layer_shell::KeyboardInteractivity::OnDemand,
        ),
    ] {
        c.keyboard_interactivity = keyboard;
        assert_eq!(layer_shell_options(&c)?.keyboard_interactivity, want);
    }
    for (edge, want) in [
        (Edge::Top, layer_shell::Anchor::TOP),
        (Edge::Bottom, layer_shell::Anchor::BOTTOM),
        (Edge::Left, layer_shell::Anchor::LEFT),
        (Edge::Right, layer_shell::Anchor::RIGHT),
    ] {
        c.anchor = Anchor::NONE.with(edge);
        assert_eq!(layer_shell_options(&c)?.anchor, want);
        assert_eq!(want.bits(), edge.bit());
    }
    Ok(())
}

/// Positive: absent optional members stay absent, and no anchor is no anchor.
#[test]
fn absent_members_stay_absent() -> Result<(), Box<dyn std::error::Error>> {
    let mut c = canvas()?;
    c.anchor = Anchor::NONE;
    c.exclusive_zone = None;
    c.exclusive_edge = None;
    c.margin = None;
    let options = layer_shell_options(&c)?;
    assert!(options.anchor.is_empty());
    assert_eq!(options.exclusive_zone, None);
    assert_eq!(options.exclusive_edge, None);
    assert_eq!(options.margin, None);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: an exclusive edge the anchor does not hold is refused with the
/// contract's own error, before gpui would hand it to the compositor.
#[test]
fn an_unanchored_exclusive_edge_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let mut c = canvas()?;
    c.exclusive_edge = Some(Edge::Bottom);
    assert_eq!(
        layer_shell_options(&c),
        Err(LayerMapError::Contract(MountError::ExclusiveEdge))
    );
    Ok(())
}

/// Negative: an exclusive zone below -1 is refused by the contract.
#[test]
fn an_exclusive_zone_below_minus_one_is_refused() -> Result<(), Box<dyn std::error::Error>> {
    let mut c = canvas()?;
    c.exclusive_zone = Some(-2);
    assert_eq!(
        layer_shell_options(&c),
        Err(LayerMapError::Contract(MountError::ExclusiveZone))
    );
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: the extents at both ends of the contract's range convert
/// exactly; one past them is refused.
#[test]
fn extents_convert_exactly_at_both_ends() -> Result<(), Box<dyn std::error::Error>> {
    let mut c = canvas()?;
    for zone in [-1, 0, MAX_EXTENT] {
        c.exclusive_zone = Some(zone);
        let want = px(f32::from(i16::try_from(zone)?));
        assert_eq!(layer_shell_options(&c)?.exclusive_zone, Some(want));
    }
    c.exclusive_zone = Some(MAX_EXTENT.saturating_add(1));
    assert!(layer_shell_options(&c).is_err());
    c.exclusive_zone = None;
    c.margin = Some(Margin {
        top: MAX_EXTENT,
        right: -MAX_EXTENT,
        bottom: 0,
        left: -1,
    });
    let want = (px(16_384.0), px(-16_384.0), px(0.0), px(-1.0));
    assert_eq!(layer_shell_options(&c)?.margin, Some(want));
    c.margin = Some(Margin {
        top: (-MAX_EXTENT).saturating_sub(1),
        right: 0,
        bottom: 0,
        left: 0,
    });
    assert!(layer_shell_options(&c).is_err());
    Ok(())
}

/// Boundary: the 64-byte namespace passes through unchanged.
#[test]
fn the_longest_namespace_passes_unchanged() -> Result<(), Box<dyn std::error::Error>> {
    let mut c = canvas()?;
    let longest = "a".repeat(aegis_scaena::mount::MAX_NAMESPACE_BYTES);
    c.namespace = Namespace::parse(longest.as_bytes())?;
    assert_eq!(layer_shell_options(&c)?.namespace, longest);
    Ok(())
}
