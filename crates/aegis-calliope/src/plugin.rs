// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! The bounded sandboxed-plugin table and its typed tolerance lifecycle
//! (REQ-P08-03, REQ-P08-08).
//!
//! This is the scaffold's `PipeWirePipelineManager` plugin half with the two
//! things it lacked: the capacity refusal is a typed error rather than a
//! string, and the "immune system" lifecycle the P08 report describes is a
//! transition table rather than a `is_sandboxed: bool` that is written `true`
//! at every call site.
//!
//! # What REQ-P08-03 becomes here
//!
//! The report requires a third-party plugin to pass through a rate-limited,
//! restricted **Quarantine** before it is activated into the real-time graph.
//! That is stated as [`PluginStage::may_advance_to`]: a slot is admitted in
//! [`PluginStage::Recognition`], and the only step out of Recognition is
//! Quarantine. [`PluginStage::Recognition`] to [`PluginStage::Activation`] is
//! [`CalliopeError::IllegalStageTransition`], which is the executable form of
//! the requirement.
//!
//! # What this module does not do
//!
//! **No plugin is loaded, bridged, rate-limited or executed.** No subprocess
//! is started, no bridge binary is named, no `VST3`, `LV2` or `CLAP` library is
//! opened and no real-time graph exists. A slot is a row in a fixed array and
//! a stage is a value of [`PluginStage`]; "rate-limited" and "restricted" are
//! properties of a sandbox this crate does not build, and
//! [`P08_RECORDED_CLAIMS`](crate::register::P08_RECORDED_CLAIMS) says so.

use crate::error::CalliopeError;
use crate::id::Label;

/// Scalar upper bound on the sandboxed-plugin table.
///
/// The scaffold's `MAX_PLUGIN_SLOTS`, labelled there as an explicit NASA JPL
/// P10-2 upper bound (export-026 `c2f1e433cd32`).
pub const MAX_PLUGIN_SLOTS: usize = 32;

/// The plugin formats the report names.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PluginFormat {
    /// A Steinberg `VST3` plugin.
    Vst3,
    /// An `LV2` plugin.
    Lv2,
    /// A `CLAP` plugin.
    Clap,
}

impl PluginFormat {
    /// All three formats, in the order the scaffold declares them.
    pub const ALL: [Self; 3] = [Self::Vst3, Self::Lv2, Self::Clap];

    /// Returns the stable tag the wire form carries.
    #[must_use]
    pub const fn tag(self) -> &'static str {
        match self {
            Self::Vst3 => "vst3",
            Self::Lv2 => "lv2",
            Self::Clap => "clap",
        }
    }

    /// Returns the format a wire tag names, when it names one.
    #[must_use]
    pub fn from_tag(tag: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|format| format.tag() == tag)
    }
}

/// The stages of the typed tolerance lifecycle the P08 report describes.
///
/// Recorded from export-017 `afbc0af8056d`, which lists Recognition,
/// Quarantine, Anergic/Suppressed, Activation and Contraction and states that
/// "inactive is not one state": a suppressed plugin and a contracted one are
/// different, and only Contraction is terminal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum PluginStage {
    /// Provenance verification and identification.
    Recognition,
    /// Execution in a rate-limited, restricted environment.
    Quarantine,
    /// Reversible disablement, short of ejection.
    Anergic,
    /// Integration into the real-time graph.
    Activation,
    /// Audited deactivation and resource cleanup.
    Contraction,
}

impl PluginStage {
    /// All five stages, in the order the report lists them.
    pub const ALL: [Self; 5] = [
        Self::Recognition,
        Self::Quarantine,
        Self::Anergic,
        Self::Activation,
        Self::Contraction,
    ];

    /// Returns the stable name this stage is recorded under.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Recognition => "recognition",
            Self::Quarantine => "quarantine",
            Self::Anergic => "anergic",
            Self::Activation => "activation",
            Self::Contraction => "contraction",
        }
    }

    /// Returns `true` when nothing may follow this stage.
    #[must_use]
    pub const fn is_terminal(self) -> bool {
        matches!(self, Self::Contraction)
    }

    /// Returns `true` when a plugin in this stage is in the real-time graph.
    #[must_use]
    pub const fn in_realtime_graph(self) -> bool {
        matches!(self, Self::Activation)
    }

    /// Returns `true` when the lifecycle admits a step from `self` to `next`.
    ///
    /// A step to the stage already held is refused: the transition table
    /// describes movement, and a no-op that reports success would hide a
    /// caller that lost track of its own state.
    #[must_use]
    pub const fn may_advance_to(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::Recognition, Self::Quarantine)
                | (
                    Self::Quarantine,
                    Self::Anergic | Self::Activation | Self::Contraction
                )
                | (
                    Self::Anergic,
                    Self::Quarantine | Self::Activation | Self::Contraction
                )
                | (Self::Activation, Self::Anergic | Self::Contraction)
        )
    }
}

/// A sandbox slot identifier, zero-based as in the scaffold.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, serde::Serialize, serde::Deserialize,
)]
pub struct PluginSlotId(u32);

impl PluginSlotId {
    /// Names a slot by its raw identifier.
    #[must_use]
    pub const fn new(raw: u32) -> Self {
        Self(raw)
    }

    /// Returns the raw identifier.
    #[must_use]
    pub const fn get(self) -> u32 {
        self.0
    }
}

/// One sandbox slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PluginSandboxSlot {
    /// The slot identifier.
    pub slot: PluginSlotId,
    /// The plugin name.
    pub name: Label,
    /// The plugin format.
    pub format: PluginFormat,
    /// Where the plugin stands in the typed tolerance lifecycle.
    pub stage: PluginStage,
}

/// The bounded sandboxed-plugin table.
///
/// `Copy`, like every value on a decision path in this crate: the whole table
/// is a fixed array, so admitting a plugin allocates nothing.
#[derive(Debug, Clone, Copy)]
pub struct PluginHost {
    slots: [Option<PluginSandboxSlot>; MAX_PLUGIN_SLOTS],
    count: usize,
}

impl Default for PluginHost {
    fn default() -> Self {
        Self::new()
    }
}

impl PluginHost {
    /// Builds an empty table.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            slots: [None; MAX_PLUGIN_SLOTS],
            count: 0,
        }
    }

    /// Admits one plugin into [`PluginStage::Recognition`].
    ///
    /// There is no other way to put a row in the table, which is what makes
    /// the Quarantine step unavoidable: every row starts at Recognition, and
    /// [`PluginStage::may_advance_to`] is the only way out of it.
    ///
    /// # Errors
    ///
    /// Returns [`CalliopeError::PluginTableFull`] at [`MAX_PLUGIN_SLOTS`].
    pub fn admit(
        &mut self,
        name: Label,
        format: PluginFormat,
    ) -> Result<PluginSlotId, CalliopeError> {
        if self.count >= MAX_PLUGIN_SLOTS {
            return Err(CalliopeError::PluginTableFull {
                max: MAX_PLUGIN_SLOTS,
            });
        }
        let index = self.count;
        let slot = PluginSlotId::new(u32::try_from(index).unwrap_or(u32::MAX));
        let row = PluginSandboxSlot {
            slot,
            name,
            format,
            stage: PluginStage::Recognition,
        };
        let cell = self
            .slots
            .get_mut(index)
            .ok_or(CalliopeError::PluginTableFull {
                max: MAX_PLUGIN_SLOTS,
            })?;
        *cell = Some(row);
        self.count = self.count.saturating_add(1);
        Ok(slot)
    }

    /// Steps one slot to `next`, returning the stage it now holds.
    ///
    /// # Errors
    ///
    /// Returns [`CalliopeError::UnknownSlot`] when the table holds no such
    /// slot, and [`CalliopeError::IllegalStageTransition`] when the lifecycle
    /// does not admit the step -- which is where REQ-P08-03 is enforced.
    pub fn advance(
        &mut self,
        slot: PluginSlotId,
        next: PluginStage,
    ) -> Result<PluginStage, CalliopeError> {
        for cell in self.slots.iter_mut().take(MAX_PLUGIN_SLOTS) {
            let Some(row) = cell.as_mut() else { continue };
            if row.slot != slot {
                continue;
            }
            if !row.stage.may_advance_to(next) {
                return Err(CalliopeError::IllegalStageTransition {
                    from: row.stage,
                    to: next,
                });
            }
            row.stage = next;
            return Ok(next);
        }
        Err(CalliopeError::UnknownSlot { slot: slot.get() })
    }

    /// Returns the row for `slot`, when the table holds one.
    #[must_use]
    pub fn get(&self, slot: PluginSlotId) -> Option<PluginSandboxSlot> {
        self.slots
            .iter()
            .take(MAX_PLUGIN_SLOTS)
            .flatten()
            .find(|row| row.slot == slot)
            .copied()
    }

    /// Returns how many slots the table holds.
    #[must_use]
    pub const fn count(&self) -> usize {
        self.count
    }

    /// Returns `true` when the table holds no plugin.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.count == 0
    }

    /// Returns how many rows are in the real-time graph.
    #[must_use]
    pub fn in_realtime_graph(&self) -> usize {
        self.slots
            .iter()
            .take(MAX_PLUGIN_SLOTS)
            .flatten()
            .filter(|row| row.stage.in_realtime_graph())
            .count()
    }
}
