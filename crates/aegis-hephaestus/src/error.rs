// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! What the P14 validators refuse, and why.
//!
//! The imported scaffold (export-029 `427996186520`) returns
//! `Box<dyn Error>` built from a string literal for every refusal, so a caller
//! can only match on prose and cannot read the bound that was hit. Each
//! refusal is a typed variant carrying its bound instead, which is what
//! HISS-07 asks for. The enum is `tornus-geometry`'s `GeometryError` since the
//! bounds moved there (cut unit CUT-1632-01); this name keeps every Aegis path
//! resolving.

pub use tornus_geometry::GeometryError as HephaestusError;
