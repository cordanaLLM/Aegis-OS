// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P12 `aegis-concordia`: the Concordia design tokens as Rust theme
//! constants (milestone M28, epic E28-5; D05, D101).
//!
//! `ui/concordia-tokens/concordia-tokens.css` stays the one token source.
//! `tools/concordia_theme.py` reads it and writes [`tokens`], one constant per
//! custom property, typed by its value: a colour is an [`Rgb`], a length in
//! pixels a [`Px`], one in rem a [`Rem`], a duration [`Millis`], a unitless
//! value a [`Ratio`], an easing a [`CubicBezier`] and a stroke style a
//! [`LineStyle`]. The media overrides become a second set: `REDUCED_MOTION_*`
//! for `prefers-reduced-motion: reduce` and `FORCED_COLORS_*`, whose colours
//! are [`SystemColor`]s, for `forced-colors: active`.
//!
//! The generator refuses a focus ring narrower than the D16 floor, and so
//! does this crate at compile time ([`FOCUS_RING_FLOOR`]). A constant edited
//! by hand, or a token changed without regenerating, fails `make verify-all`
//! through `tools/test_concordia_theme.py`.
//!
//! # What this crate does not do
//!
//! It depends on nothing, paints nothing and reads nothing at run time. It
//! does not compute contrast; `tools/test_a11y.py` holds the token file to
//! its contrast figures. Which toolkit type a constant becomes is the shell's
//! business (M28's gpui member), not this crate's.

pub mod tokens;

/// The narrowest focus ring D16 admits, in pixels.
pub const FOCUS_RING_FLOOR: Px = Px(2);

// D16, checked when the crate compiles as well as when the file is generated:
// a ring below the floor makes this array length 0 and the type mismatch a
// compile error, with no panic in library code.
const _: [(); 1] = [(); (tokens::FOCUS_RING_WIDTH.0 >= FOCUS_RING_FLOOR.0) as usize];

/// An opaque sRGB colour, one byte per channel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Rgb {
    /// The red channel.
    pub red: u8,
    /// The green channel.
    pub green: u8,
    /// The blue channel.
    pub blue: u8,
}

/// A CSS system colour (CSS Color 4), which forced-colours mode resolves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SystemColor {
    /// `Canvas`: the background of application content.
    Canvas,
    /// `CanvasText`: text on `Canvas`.
    CanvasText,
    /// `Highlight`: the background of selected or focused items.
    Highlight,
    /// `HighlightText`: text on `Highlight`.
    HighlightText,
    /// `ButtonFace`: the background of push buttons.
    ButtonFace,
    /// `ButtonText`: text on `ButtonFace`.
    ButtonText,
    /// `LinkText`: unvisited links.
    LinkText,
    /// `GrayText`: disabled text.
    GrayText,
}

/// A length in whole CSS pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Px(pub u16);

/// A length in thousandths of a rem: `Rem(375)` is `0.375rem`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Rem(pub u32);

/// A unitless value in thousandths: `Ratio(1500)` is `1.5`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ratio(pub u32);

/// A duration in whole milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Millis(pub u32);

/// A `cubic-bezier()` easing, each control value in thousandths.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CubicBezier {
    /// The first control point's x, within `0..=1000`.
    pub x1: i32,
    /// The first control point's y.
    pub y1: i32,
    /// The second control point's x, within `0..=1000`.
    pub x2: i32,
    /// The second control point's y.
    pub y2: i32,
}

/// A stroke style.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LineStyle {
    /// `solid`.
    Solid,
    /// `dashed`.
    Dashed,
    /// `dotted`.
    Dotted,
    /// `double`.
    Double,
}
