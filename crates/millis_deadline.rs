// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! D116: Millisecond deadline newtypes backed by `NonZeroU32`.
//!
//! One macro, included by path into every crate that defines a millisecond
//! deadline newtype (`#[path = "<relative>/millis_deadline.rs"]`), so the
//! accessor methods and constructor have one definition rather than one per
//! crate. It is not a crate: a member enters every dependency closure and
//! carries a permanent name.

#![allow(unused_macros)]
#![allow(dead_code)]

macro_rules! millis_deadline {
    (
        $(#[$meta:meta])*
        $vis:vis struct $name:ident;
    ) => {
        $(#[$meta])*
        $vis struct $name(::core::num::NonZeroU32);

        impl $name {
            /// Builds a deadline of `millis` milliseconds.
            #[must_use]
            pub const fn from_millis(millis: ::core::num::NonZeroU32) -> Self {
                Self(millis)
            }

            /// Builds a deadline of `millis` milliseconds, or `None` when it is zero.
            #[must_use]
            pub const fn try_from_millis(millis: u32) -> Option<Self> {
                match ::core::num::NonZeroU32::new(millis) {
                    Some(value) => Some(Self(value)),
                    None => None,
                }
            }

            /// Returns the deadline in milliseconds.
            #[must_use]
            pub const fn millis(self) -> u32 {
                self.0.get()
            }
        }
    };
}
