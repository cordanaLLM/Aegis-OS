// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The platform interfaces, which this crate reaches only through traits
//! (D77, REQ-P05-05).
//!
//! | Interface | Transport in the product | Here |
//! | :-- | :-- | :-- |
//! | `SYNC_DESKTOP_SHELL` from P04 | the Unix socket stream the graph records (D32) | [`CompositorLink`] |
//! | the `StatusNotifierWatcher` | D-Bus | [`StatusNotifierWatcher`] |
//! | the portal settings (REQ-P12-02) | D-Bus, `org.freedesktop.portal.Settings` | [`PortalSettings`] |
//! | AT-SPI2 (REQ-P12-03) | D-Bus, through `accesskit_unix` at M28 | [`AccessibilityBus`] |
//!
//! The tests implement every trait with an in-memory mock; this crate opens
//! no socket and contacts no bus, and M29 runs the real daemons in a live
//! session. [`mount`] is the shell's start-up in the order REQ-P05-05 gives:
//! the compositor first, then the watcher, then the preferences, and last the
//! first accessibility tree -- which is published only if it passes
//! [`crate::a11y::check_tree`], so a failing tree fails the mount instead of
//! reaching assistive technology.

use accesskit::TreeUpdate;
use thiserror::Error;

use crate::a11y::{self, TreeError};
use crate::state::{Preferences, ShellState};

/// The shell's own bus name, which it registers as a tray host under.
pub const HOST_SERVICE: &str = "org.aegisos.Forum1";

/// The portal settings namespace the preferences are read from.
pub const APPEARANCE_NAMESPACE: &str = "org.freedesktop.appearance";

/// The portal key for the preferred contrast: 0 no preference, 1 higher
/// contrast; an unknown value reads as 0 (Settings interface version 2).
pub const CONTRAST_KEY: &str = "contrast";

/// The portal key for reduced motion: 0 no preference, 1 reduced motion; an
/// unknown value reads as 0 (Settings interface version 2).
pub const REDUCED_MOTION_KEY: &str = "reduced-motion";

/// Why a platform interface refused a call.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
pub enum PlatformError {
    /// The interface is not there.
    #[error("{0} is not available")]
    Unavailable(&'static str),
    /// The interface answered with a refusal.
    #[error("{0} refused the call")]
    Refused(&'static str),
}

/// The compositor end of `SYNC_DESKTOP_SHELL` (stubbed).
pub trait CompositorLink {
    /// Synchronises the desktop shell with the compositor.
    ///
    /// # Errors
    ///
    /// Returns the link's refusal.
    fn sync_desktop_shell(&mut self) -> Result<(), PlatformError>;
}

/// The `StatusNotifierWatcher` (a D-Bus mock in the tests).
pub trait StatusNotifierWatcher {
    /// Registers the shell as a tray host.
    ///
    /// # Errors
    ///
    /// Returns the watcher's refusal.
    fn register_host(&mut self, service: &str) -> Result<(), PlatformError>;

    /// The tray items registered with the watcher.
    ///
    /// # Errors
    ///
    /// Returns the watcher's refusal.
    fn registered_items(&self) -> Result<Vec<String>, PlatformError>;
}

/// The portal settings (a D-Bus mock in the tests).
pub trait PortalSettings {
    /// Reads one unsigned setting; `None` when the portal does not have it.
    ///
    /// # Errors
    ///
    /// Returns the portal's refusal.
    fn read_u32(&self, namespace: &str, key: &str) -> Result<Option<u32>, PlatformError>;
}

/// The AT-SPI2 side the tree is published to (a D-Bus mock in the tests).
pub trait AccessibilityBus {
    /// Publishes one tree update.
    ///
    /// # Errors
    ///
    /// Returns the bus's refusal.
    fn publish(&mut self, update: &TreeUpdate) -> Result<(), PlatformError>;
}

/// The four interfaces mount reaches.
pub struct Platform<'a> {
    /// The compositor link.
    pub compositor: &'a mut dyn CompositorLink,
    /// The tray watcher.
    pub watcher: &'a mut dyn StatusNotifierWatcher,
    /// The portal settings.
    pub portal: &'a dyn PortalSettings,
    /// The accessibility bus.
    pub bus: &'a mut dyn AccessibilityBus,
}

/// Why the mount failed, and at which step.
#[derive(Debug, Clone, PartialEq, Error)]
pub enum MountError {
    /// The compositor link refused.
    #[error("compositor: {0}")]
    Compositor(PlatformError),
    /// The tray watcher refused.
    #[error("StatusNotifierWatcher: {0}")]
    Watcher(PlatformError),
    /// The portal refused.
    #[error("portal settings: {0}")]
    Portal(PlatformError),
    /// The first tree failed the tree check and was not published.
    #[error("the first accessibility tree failed its check and was not published: {0}")]
    Tree(TreeError),
    /// The accessibility bus refused the tree.
    #[error("accessibility bus: {0}")]
    Bus(PlatformError),
}

/// What the mount did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MountReport {
    /// Tray items the watcher reported.
    pub tray_items: usize,
    /// The preferences read from the portal.
    pub preferences: Preferences,
    /// Nodes in the published tree.
    pub published_nodes: usize,
}

/// Reads one preference flag: exactly 1 is set; 0, an unknown value or a
/// missing key is no preference.
fn flag(portal: &dyn PortalSettings, key: &str) -> Result<bool, PlatformError> {
    Ok(portal.read_u32(APPEARANCE_NAMESPACE, key)? == Some(1))
}

/// Mounts the shell; see the module documentation for the order.
///
/// # Errors
///
/// Returns the first step's refusal as a [`MountError`]; a tree that fails
/// the check is [`MountError::Tree`] and is not published.
pub fn mount(
    state: &mut ShellState,
    platform: &mut Platform<'_>,
) -> Result<MountReport, MountError> {
    platform
        .compositor
        .sync_desktop_shell()
        .map_err(MountError::Compositor)?;
    platform
        .watcher
        .register_host(HOST_SERVICE)
        .map_err(MountError::Watcher)?;
    let tray_items = platform
        .watcher
        .registered_items()
        .map_err(MountError::Watcher)?
        .len();
    state.preferences = Preferences {
        high_contrast: flag(platform.portal, CONTRAST_KEY).map_err(MountError::Portal)?,
        reduced_motion: flag(platform.portal, REDUCED_MOTION_KEY).map_err(MountError::Portal)?,
    };
    let update = a11y::export(state);
    let checked = a11y::check_tree(&update).map_err(MountError::Tree)?;
    platform.bus.publish(&update).map_err(MountError::Bus)?;
    Ok(MountReport {
        tray_items,
        preferences: state.preferences,
        published_nodes: checked.nodes,
    })
}
