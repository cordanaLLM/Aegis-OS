// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! E27-2 boundary: the layer surface's state machine, without a compositor.
//!
//! Positive: configure, then `ack_configure` with its serial, then attach.
//! Negative: an acknowledgement of a serial the compositor did not send last
//! is refused, and nothing may be attached after `closed`. Boundary: an
//! attach before the first `ack_configure` is refused, and a reconfigure
//! after the first acknowledgement does not take attaching away.

use aegis_scaena::{ConfiguredSize, LayerSurfaceModel, SurfaceError, SurfaceState};

/// The size the fixture compositor configures.
const SIZE: ConfiguredSize = ConfiguredSize {
    width: 64,
    height: 35,
};

// --- Positive -------------------------------------------------------------

/// Positive: the protocol's order admits an attach at the configured size.
#[test]
fn configure_then_ack_then_attach() -> Result<(), SurfaceError> {
    let mut surface = LayerSurfaceModel::new();
    surface.configure(7, SIZE)?;
    surface.ack(7)?;
    assert_eq!(surface.state(), SurfaceState::Ready { serial: 7 });
    assert_eq!(surface.attach()?, SIZE);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a serial other than the last configure's is refused, and a
/// closed surface refuses every step.
#[test]
fn a_wrong_serial_and_a_closed_surface_are_refused() -> Result<(), SurfaceError> {
    let mut surface = LayerSurfaceModel::new();
    surface.configure(7, SIZE)?;
    assert_eq!(
        surface.ack(6),
        Err(SurfaceError::UnknownSerial { serial: 6 })
    );
    surface.ack(7)?;
    assert_eq!(
        surface.ack(7),
        Err(SurfaceError::UnknownSerial { serial: 7 })
    );
    surface.close();
    assert_eq!(surface.attach(), Err(SurfaceError::Closed));
    assert_eq!(surface.configure(8, SIZE), Err(SurfaceError::Closed));
    assert_eq!(surface.ack(8), Err(SurfaceError::Closed));
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: before the first acknowledgement nothing may be attached --
/// neither on a fresh surface nor after a configure that is not yet
/// acknowledged.
#[test]
fn an_attach_before_the_first_ack_configure_is_refused() -> Result<(), SurfaceError> {
    let mut surface = LayerSurfaceModel::new();
    assert_eq!(surface.attach(), Err(SurfaceError::NotConfigured));
    assert_eq!(surface.ack(1), Err(SurfaceError::NotConfigured));
    surface.configure(1, SIZE)?;
    assert_eq!(surface.attach(), Err(SurfaceError::NotConfigured));
    surface.ack(1)?;
    assert_eq!(surface.attach()?, SIZE);
    Ok(())
}

/// Boundary: a reconfigure after the first acknowledgement keeps attaching
/// admitted, at the new size, until the client acknowledges it.
#[test]
fn a_reconfigure_after_the_first_ack_keeps_attach_admitted() -> Result<(), SurfaceError> {
    let mut surface = LayerSurfaceModel::new();
    surface.configure(1, SIZE)?;
    surface.ack(1)?;
    let larger = ConfiguredSize {
        width: 128,
        height: 70,
    };
    surface.configure(2, larger)?;
    assert_eq!(
        surface.state(),
        SurfaceState::Configured {
            serial: 2,
            acked_before: true
        }
    );
    assert_eq!(surface.attach()?, larger);
    surface.ack(2)?;
    assert_eq!(surface.attach()?, larger);
    Ok(())
}
