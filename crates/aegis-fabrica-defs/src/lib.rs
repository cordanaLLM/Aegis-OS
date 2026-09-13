// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P01/P02 `aegis-fabrica-defs`: the definition parser (D15).
//!
//! The crate reads the declarative inputs this repository ships in `build/`:
//! the `repart.d(5)` partition drop-ins and the `sysupdate.d(5)` transfer. It
//! answers two different questions about them, and keeps the answers apart:
//!
//! * **would systemd refuse this?** Those are [`DefinitionError`] values. A
//!   missing `Type=`, an inverted size pair and an unreadable size are
//!   refusals systemd itself makes, so a reviewer gets the same verdict
//!   without building an image.
//! * **does this meet the recorded requirement?** Those are [`Finding`]
//!   values, each naming the requirement it comes from.
//!
//! The parser is deliberately stricter than systemd in one direction, and the
//! milestone exists because of it: systemd prints `Unknown key '...' in
//! section [...], ignoring` and **exits 0**. An ignored key is a definition
//! that says one thing and does another, so this crate refuses it
//! ([`DefinitionError::UnknownKey`]) instead of warning. The same applies to a
//! repeated section or key, which systemd merges or resolves last-wins, and to
//! `InstancesMax=1`, which systemd silently bumps to 2
//! ([`Finding::SingleSlotTransfer`]).
//!
//! The key vocabularies in [`repart`] and [`sysupdate`] were read from the
//! `repart.d(5)` and `sysupdate.d(5)` manual pages of the admitted systemd
//! floor recorded in `docs/roadmap/toolchain-admission.md`, not from memory.
//! `tools/verify_systemd_definitions.py` runs the same files through the host's
//! systemd, so the two checks can disagree and be caught.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: the drop-in reader is a single forward pass over lines and
//!   the set checks are flat loops over a bounded slice;
//! * every loop carries a scalar upper bound, declared as a constant:
//!   [`MAX_LINES`], [`MAX_LINE_BYTES`], [`MAX_SECTIONS`], [`MAX_ENTRIES`],
//!   [`MAX_DEFINITIONS`], [`MAX_FINDINGS`] and [`MAX_SIZE_DIGITS`];
//! * no `unwrap`, `expect`, `panic!`, slice indexing or wrapping arithmetic;
//!   [`parse_size`] returns `None` on overflow rather than a wrapped product;
//! * no dynamic execution and no `unsafe` (forbidden at the crate root).
//!
//! # What this crate does not do
//!
//! It does not build an image, run systemd, verify a roothash, or claim that a
//! definition it accepts will produce a bootable system. It reads text and
//! reports what the recorded rules say about it.
//!
//! # Example
//!
//! ```
//! use aegis_fabrica_defs::{DefinitionError, RepartDefinition};
//!
//! let accepted = RepartDefinition::parse("00-esp.conf", "[Partition]\nType=esp\n")?;
//! assert_eq!(accepted.partition_type(), "esp");
//!
//! let ignored_by_systemd = RepartDefinition::parse(
//!     "00-esp.conf",
//!     "[Partition]\nType=esp\nSubsystem=block\n",
//! );
//! assert!(matches!(
//!     ignored_by_systemd,
//!     Err(DefinitionError::UnknownKey { .. })
//! ));
//! # Ok::<(), DefinitionError>(())
//! ```

pub mod error;
pub mod finding;
pub mod repart;
pub mod size;
pub mod sysupdate;
pub mod unit;

pub use crate::error::DefinitionError;
pub use crate::finding::{Finding, MAX_FINDINGS};
pub use crate::repart::{
    ESP_TYPE, KNOWN_KEYS, MAX_DEFINITIONS, PARTITION_SECTION, ROOT_TYPE, RepartDefinition,
    VAR_TYPE, check_repart_set,
};
pub use crate::size::{
    ESP_MAX_BYTES, ESP_MIN_BYTES, GIBIBYTE, KIBIBYTE, MAX_SIZE_DIGITS, MEBIBYTE, parse_size,
};
pub use crate::sysupdate::{
    AB_INSTANCES, SOURCE_KEYS, SOURCE_SECTION, TARGET_KEYS, TARGET_SECTION, TRANSFER_KEYS,
    TRANSFER_SECTION, TransferDefinition,
};
pub use crate::unit::{Entry, MAX_ENTRIES, MAX_LINE_BYTES, MAX_LINES, MAX_SECTIONS, UnitFile};

/// The contract version the reviewed definitions in `build/` are read under.
///
/// It changes when a key vocabulary or a finding rule changes, so a consumer
/// can tell a re-read from a re-interpretation.
pub const CONTRACT_VERSION: u32 = 1;
