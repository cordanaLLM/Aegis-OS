// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The two root slots the reviewed definitions declare.
//!
//! The names are not invented here. `build/repart.d/10-root-a.conf` and
//! `build/repart.d/10-root-b.conf` declare `Label=root-a` and `Label=root-b`,
//! `build/sysupdate.d/10-root.transfer` selects them with
//! `MatchPattern=root-@v` and `InstancesMax=2`, and the M03 transfer gate read
//! back `{"current":"b","all":["b","a"]}` from `systemd-sysupdate --offline
//! --json=short list`. [`Slot::name`] is the short form that appeared in that
//! output and [`Slot::label`] is the partition label, so a trace rendered here
//! can be compared with a real transfer at M24 without a translation table.

/// Which of the two root slots a value refers to.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, serde::Serialize, serde::Deserialize,
)]
#[serde(rename_all = "kebab-case")]
#[non_exhaustive]
pub enum Slot {
    /// Root slot A, `Label=root-a`.
    A,
    /// Root slot B, `Label=root-b`.
    B,
}

impl Slot {
    /// Both slots, in declaration order.
    ///
    /// Exported rather than re-declared by each sweep that needs one:
    /// `#[non_exhaustive]` stops a consumer crate from matching the enum
    /// exhaustively, so a sweep holding its own copy of the list could not be
    /// broken by a new variant and would quietly stop testing what it claims.
    pub const ALL: [Self; 2] = [Self::A, Self::B];

    /// Returns the other slot: the A/B pair has exactly one alternate.
    #[must_use]
    pub const fn other(self) -> Self {
        match self {
            Self::A => Self::B,
            Self::B => Self::A,
        }
    }

    /// Returns the short name `systemd-sysupdate list` prints.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::A => "a",
            Self::B => "b",
        }
    }

    /// Returns the partition label the repart drop-in declares.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::A => "root-a",
            Self::B => "root-b",
        }
    }
}

impl core::fmt::Display for Slot {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(self.name())
    }
}
