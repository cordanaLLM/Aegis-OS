// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P01/P02 `aegis-fabrica-defs`: the declarative inputs (D15).
//!
//! The crate owns the P01/P02 declarative inputs this repository ships in
//! `build/`, in two layers.
//!
//! # The definition parser (milestone M03)
//!
//! It reads the `repart.d(5)` partition drop-ins and the `sysupdate.d(5)`
//! transfer, and answers two different questions about them, keeping the
//! answers apart:
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
//! # The schemas (milestone M18)
//!
//! [`manifest`] is the Aegis product input manifest: the correlation
//! identifier, exact revision, pinned distribution snapshot, configuration
//! references, package set, boot kernel identity and bounded retry budget that
//! `docs/integration/stack.md` asks of every crossing of the Aegis boundary.
//! It is validated against the reviewed definition files rather than against a
//! description of them.
//!
//! [`kernel`] is the kernel requirement schema: a payload of Kconfig symbol
//! rows, each with the state it must be in, the probe that checks it on a
//! running system and the requirement it comes from. It renders as a Kconfig
//! fragment for the kernel build decision D70 puts in this repository (M26),
//! and it is checked against the measured reference profile by
//! [`kernel::ReferenceProfile`].
//!
//! Neither schema contacts a producer repository, opens a file, runs a builder
//! or verifies a signature. A digest or signature field is a field encoding.
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
//!   [`MAX_DEFINITIONS`], [`MAX_FINDINGS`], [`MAX_SIZE_DIGITS`],
//!   [`MAX_PAYLOAD_BYTES`], [`manifest::MAX_PACKAGES`],
//!   [`kernel::MAX_FEATURES`], [`kernel::MAX_ARCHITECTURES`] and
//!   [`kernel::MAX_UNMET`];
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
pub mod field;
pub mod finding;
pub mod kernel;
pub mod manifest;
pub mod payload;
pub mod repart;
pub mod size;
pub mod sysupdate;
pub mod unit;

pub use crate::error::DefinitionError;
pub use crate::field::FieldError;
pub use crate::finding::{Finding, MAX_FINDINGS};
pub use crate::kernel::{KernelError, KernelRequirement, ReferenceProfile, Unmet};
pub use crate::manifest::{ManifestError, ProductInputManifest};
pub use crate::payload::MAX_PAYLOAD_BYTES;
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
