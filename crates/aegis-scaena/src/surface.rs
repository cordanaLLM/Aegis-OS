// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The layer surface's state machine, as the client half sees it.
//!
//! `zwlr_layer_surface_v1` requires the client to commit once without a
//! buffer, wait for `configure`, and answer it with `ack_configure` before the
//! first buffer is attached; attaching earlier is a protocol error the
//! compositor may raise. This model refuses it client-side, so the refusal
//! does not rest on one compositor's conformance: [`LayerSurfaceModel::attach`]
//! fails with [`SurfaceError::NotConfigured`] until the first acknowledged
//! configure, and with [`SurfaceError::Closed`] once the compositor closed the
//! surface. The model is plain data, so it is tested without a compositor.

use crate::error::SurfaceError;

/// Where the surface is in its lifecycle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceState {
    /// Created and committed without a buffer; no configure seen yet.
    Created,
    /// A configure arrived and is not yet acknowledged.
    Configured {
        /// The configure's serial.
        serial: u32,
        /// Whether an earlier configure was already acknowledged.
        acked_before: bool,
    },
    /// The last configure is acknowledged; buffers may be attached.
    Ready {
        /// The acknowledged serial.
        serial: u32,
    },
    /// The compositor sent `closed`.
    Closed,
}

/// The size the compositor configured, in surface-local pixels; 0 lets the
/// client choose.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ConfiguredSize {
    /// Width.
    pub width: u32,
    /// Height.
    pub height: u32,
}

/// The client half's model of one layer surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LayerSurfaceModel {
    state: SurfaceState,
    size: ConfiguredSize,
}

impl LayerSurfaceModel {
    /// A surface just created and committed without a buffer.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: SurfaceState::Created,
            size: ConfiguredSize {
                width: 0,
                height: 0,
            },
        }
    }

    /// The current state.
    #[must_use]
    pub const fn state(&self) -> SurfaceState {
        self.state
    }

    /// The size of the last configure.
    #[must_use]
    pub const fn size(&self) -> ConfiguredSize {
        self.size
    }

    /// Records a `configure` event.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceError::Closed`] after `closed`.
    pub fn configure(&mut self, serial: u32, size: ConfiguredSize) -> Result<(), SurfaceError> {
        let acked_before = match self.state {
            SurfaceState::Closed => return Err(SurfaceError::Closed),
            SurfaceState::Created => false,
            SurfaceState::Configured { acked_before, .. } => acked_before,
            SurfaceState::Ready { .. } => true,
        };
        self.state = SurfaceState::Configured {
            serial,
            acked_before,
        };
        self.size = size;
        Ok(())
    }

    /// Records the client's `ack_configure`.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceError::UnknownSerial`] for a serial other than the
    /// last configure's, [`SurfaceError::NotConfigured`] before any configure
    /// and [`SurfaceError::Closed`] after `closed`.
    pub fn ack(&mut self, serial: u32) -> Result<(), SurfaceError> {
        match self.state {
            SurfaceState::Configured { serial: last, .. } if last == serial => {
                self.state = SurfaceState::Ready { serial };
                Ok(())
            }
            SurfaceState::Configured { .. } | SurfaceState::Ready { .. } => {
                Err(SurfaceError::UnknownSerial { serial })
            }
            SurfaceState::Created => Err(SurfaceError::NotConfigured),
            SurfaceState::Closed => Err(SurfaceError::Closed),
        }
    }

    /// Admits a buffer attach: only once a configure has been acknowledged.
    ///
    /// # Errors
    ///
    /// Returns [`SurfaceError::NotConfigured`] before the first
    /// `ack_configure` and [`SurfaceError::Closed`] after `closed`.
    pub const fn attach(&self) -> Result<ConfiguredSize, SurfaceError> {
        match self.state {
            SurfaceState::Ready { .. }
            | SurfaceState::Configured {
                acked_before: true, ..
            } => Ok(self.size),
            SurfaceState::Created | SurfaceState::Configured { .. } => {
                Err(SurfaceError::NotConfigured)
            }
            SurfaceState::Closed => Err(SurfaceError::Closed),
        }
    }

    /// Records the compositor's `closed`.
    pub fn close(&mut self) {
        self.state = SurfaceState::Closed;
    }
}

impl Default for LayerSurfaceModel {
    fn default() -> Self {
        Self::new()
    }
}
