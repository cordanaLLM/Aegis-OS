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
//!
//! Mapping a surface on a compositor, the first-configure deadline and
//! `LayerShellNotSupportedError` are not here yet; they need a running
//! compositor and stay open in E28-2's acceptance.

pub mod layer;

pub use layer::{LayerMapError, layer_shell_options};
