// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P05 Forum shell on gpui (M28).
//!
//! gpui enters the workspace through this member and no other (M28 exit
//! criterion 2), git-pinned to one zed-industries/zed commit in
//! `[workspace.dependencies]` (D102). The M16 model crate `aegis-forum-shell`
//! stays toolkit-free, so its tests keep running with no display, GPU or bus.
//!
//! | Module | Holds |
//! | :--- | :--- |
//! | [`capture`] | live KWin ScreenShot2 reader capturing raw pixels over D-Bus |
//! | [`fixture`] | accessibility test surface, defect variants, and seeded node tree |
//! | [`judge`] | pure rules evaluating AT-SPI accessibility trees against acceptance criteria |
//! | [`layer`] | the P17 `MOUNT_SHELL_CANVAS` parameters mapped onto gpui's `LayerShellOptions` |
//! | [`pixels`] | pure measurement of focus ring coverage, band width, and contrast ratio |
//! | [`readback`] | live AT-SPI reader connecting over D-Bus to inspect and interact with the tree |
//! | [`ring`] | pure focus-ring geometry in screen space and layer space, lines and bounds |
//! | [`ring_fixture`] | focus-ring test surface, scale variants, and occluding panels on gpui |
//! | [`surface`] | maps the surface on a compositor and decides the first configure outcome |
//! | [`theme`] | the generated Concordia token constants as gpui types, the crate's only colours |

pub mod capture;
pub mod fixture;
pub mod judge;
pub mod layer;
pub mod pixels;
pub mod readback;
pub mod ring;
pub mod ring_fixture;
pub mod surface;
pub mod theme;

pub use layer::{LayerMapError, layer_shell_options};
