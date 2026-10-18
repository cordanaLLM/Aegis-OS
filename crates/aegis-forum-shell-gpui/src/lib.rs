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
//! | [`layer`] | the P17 `MOUNT_SHELL_CANVAS` parameters mapped onto gpui's `LayerShellOptions` |
//! | [`surface`] | maps the surface on a compositor and decides the first configure outcome |

pub mod layer;
pub mod surface;

pub use layer::{LayerMapError, layer_shell_options};
