// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! REQ-P05-05 on mocks: the mount reaches the compositor link, the
//! `StatusNotifierWatcher`, the portal settings and the accessibility bus,
//! each an in-memory mock of what M29 runs live (D77).
//!
//! Positive: the mount runs in order and publishes a tree that passes the
//! tree check. Negative: a watcher that is not there fails the mount before
//! anything is published. Boundary: portal values 0, 1, an unknown value and
//! a missing key read as the Settings interface says.

mod common;

use accesskit::TreeUpdate;
use aegis_forum_shell::a11y::check_tree;
use aegis_forum_shell::canvas::seed;
use aegis_forum_shell::platform::{
    APPEARANCE_NAMESPACE, AccessibilityBus, CONTRAST_KEY, CompositorLink, HOST_SERVICE, MountError,
    Platform, PlatformError, PortalSettings, REDUCED_MOTION_KEY, StatusNotifierWatcher, mount,
};
use aegis_forum_shell::state::{Preferences, ShellState};

use common::Fallible;

/// Records every call, in order.
#[derive(Default)]
struct Mocks {
    calls: Vec<String>,
    watcher_present: bool,
    contrast: Option<u32>,
    reduced_motion: Option<u32>,
    published: Vec<TreeUpdate>,
}

/// The compositor mock.
struct Compositor<'a>(&'a std::cell::RefCell<Mocks>);
/// The watcher mock.
struct Watcher<'a>(&'a std::cell::RefCell<Mocks>);
/// The portal mock.
struct Portal<'a>(&'a std::cell::RefCell<Mocks>);
/// The bus mock.
struct Bus<'a>(&'a std::cell::RefCell<Mocks>);

impl CompositorLink for Compositor<'_> {
    fn sync_desktop_shell(&mut self) -> Result<(), PlatformError> {
        self.0.borrow_mut().calls.push("compositor".to_owned());
        Ok(())
    }
}

impl StatusNotifierWatcher for Watcher<'_> {
    fn register_host(&mut self, service: &str) -> Result<(), PlatformError> {
        let mut mocks = self.0.borrow_mut();
        mocks.calls.push(format!("register {service}"));
        if mocks.watcher_present {
            Ok(())
        } else {
            Err(PlatformError::Unavailable("watcher"))
        }
    }

    fn registered_items(&self) -> Result<Vec<String>, PlatformError> {
        self.0.borrow_mut().calls.push("items".to_owned());
        Ok(vec![
            "org.example.Tray1".to_owned(),
            "org.example.Tray2".to_owned(),
        ])
    }
}

impl PortalSettings for Portal<'_> {
    fn read_u32(&self, namespace: &str, key: &str) -> Result<Option<u32>, PlatformError> {
        let mut mocks = self.0.borrow_mut();
        mocks.calls.push(format!("read {namespace} {key}"));
        Ok(match key {
            CONTRAST_KEY => mocks.contrast,
            REDUCED_MOTION_KEY => mocks.reduced_motion,
            _ => None,
        })
    }
}

impl AccessibilityBus for Bus<'_> {
    fn publish(&mut self, update: &TreeUpdate) -> Result<(), PlatformError> {
        let mut mocks = self.0.borrow_mut();
        mocks.calls.push("publish".to_owned());
        mocks.published.push(update.clone());
        Ok(())
    }
}

/// Mounts a seeded shell against `mocks`.
fn mount_with(
    mocks: &std::cell::RefCell<Mocks>,
) -> Fallible<(
    ShellState,
    Result<aegis_forum_shell::platform::MountReport, MountError>,
)> {
    let mut state = ShellState::new(seed::grid(10, 2)?);
    let (mut compositor, mut watcher, portal, mut bus) =
        (Compositor(mocks), Watcher(mocks), Portal(mocks), Bus(mocks));
    let mut platform = Platform {
        compositor: &mut compositor,
        watcher: &mut watcher,
        portal: &portal,
        bus: &mut bus,
    };
    let result = mount(&mut state, &mut platform);
    Ok((state, result))
}

// --- Positive -------------------------------------------------------------

/// Positive: compositor, watcher, portal, then the tree, which passes the
/// check before it is published.
#[test]
fn the_mount_runs_in_order_and_publishes_a_checked_tree() -> Fallible {
    let mocks = std::cell::RefCell::new(Mocks {
        watcher_present: true,
        contrast: Some(1),
        reduced_motion: Some(0),
        ..Mocks::default()
    });
    let (state, result) = mount_with(&mocks)?;
    let report = result?;
    let mocks = mocks.into_inner();
    assert_eq!(
        mocks.calls,
        vec![
            "compositor".to_owned(),
            format!("register {HOST_SERVICE}"),
            "items".to_owned(),
            format!("read {APPEARANCE_NAMESPACE} {CONTRAST_KEY}"),
            format!("read {APPEARANCE_NAMESPACE} {REDUCED_MOTION_KEY}"),
            "publish".to_owned(),
        ]
    );
    assert_eq!(report.tray_items, 2);
    assert_eq!(
        report.preferences,
        Preferences {
            high_contrast: true,
            reduced_motion: false
        }
    );
    assert_eq!(state.preferences, report.preferences);
    let published = mocks.published.first().ok_or("nothing was published")?;
    assert_eq!(check_tree(published)?.nodes, report.published_nodes);
    Ok(())
}

// --- Negative -------------------------------------------------------------

/// Negative: a watcher that is not there fails the mount, and no tree is
/// published.
#[test]
fn a_missing_watcher_fails_the_mount_before_anything_is_published() -> Fallible {
    let mocks = std::cell::RefCell::new(Mocks::default());
    let (_, result) = mount_with(&mocks)?;
    assert_eq!(
        result.err(),
        Some(MountError::Watcher(PlatformError::Unavailable("watcher")))
    );
    assert!(mocks.into_inner().published.is_empty());
    Ok(())
}

// --- Boundary -------------------------------------------------------------

/// Boundary: exactly 1 is a preference; 0, an unknown value and a missing
/// key are none (`org.freedesktop.portal.Settings`, version 2).
#[test]
fn portal_values_read_as_the_settings_interface_says() -> Fallible {
    let cases = [
        (
            Some(1),
            Some(1),
            Preferences {
                high_contrast: true,
                reduced_motion: true,
            },
        ),
        (Some(0), Some(0), Preferences::default()),
        (Some(2), Some(7), Preferences::default()),
        (None, None, Preferences::default()),
    ];
    for (contrast, reduced_motion, expected) in cases {
        let mocks = std::cell::RefCell::new(Mocks {
            watcher_present: true,
            contrast,
            reduced_motion,
            ..Mocks::default()
        });
        let (_, result) = mount_with(&mocks)?;
        assert_eq!(
            result?.preferences, expected,
            "{contrast:?} {reduced_motion:?}"
        );
    }
    Ok(())
}
