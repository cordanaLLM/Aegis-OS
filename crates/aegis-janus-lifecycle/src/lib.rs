// SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
// SPDX-License-Identifier: EUPL-1.2

//! Aegis OS P02 `aegis-janus-lifecycle`: the A/B candidate lifecycle (D15).
//!
//! The reviewed definitions in `build/` say what an A/B update *is*: two root
//! slots (`build/repart.d/10-root-a.conf`, `10-root-b.conf`), a dm-verity hash
//! partition for the root (`11-root-verity.conf`), and one dual-slot transfer
//! (`build/sysupdate.d/10-root.transfer`) with `InstancesMax=2` and a
//! read-only target. `aegis-fabrica-defs` reads those files. This crate models
//! what *happens* to a candidate release as it moves through them: declared,
//! signature-checked, acquired, swapped in, watched, and then blessed or rolled
//! back.
//!
//! It is a model and nothing more. Nothing here runs `systemd-sysupdate`,
//! opens a device, computes a hash, changes a boot order or reboots; see
//! [`ports`] for the seam where a real implementation would attach, and
//! `tests/stubbed_effects.rs` for the sweep that keeps it that way.
//!
//! # The three things a reviewer should look at
//!
//! * [`Machine`] is the state machine. [`State`] is the vocabulary, [`Event`]
//!   is what may be offered, and [`LifecycleError`] is every refusal. The
//!   watchdog boundary REQ-P02-08 asks about is the half-open interval in
//!   [`Watchdog`]: expiry is `now >= due_at`, so exactly at the timeout rolls
//!   back and one tick earlier does not.
//! * [`Trace`] is the machine-readable transition trace milestone M24 diffs a
//!   real QEMU transfer against. It renders as JSON Lines, header first,
//!   ordered by a dense sequence number, with every line carrying
//!   [`TRACE_SCHEMA`]. Rendering is a fixed point of [`Trace::parse`], which is
//!   what makes a byte-for-byte comparison sound.
//! * [`decision`] records D13 -- reversible structural consolidation -- against
//!   the dm-verity requirement, and [`Machine`] holds the rule that reconciles
//!   them: reopening a blessed slot discards its measurement, so re-blessing
//!   needs a fresh verity match.
//!
//! # Design invariants (see `AGENTS.md`)
//!
//! * no recursion: planning is a flat dispatch over [`State`] into one small
//!   handler per state, and no handler calls another handler;
//! * every loop carries a scalar upper bound: [`MAX_TRANSITIONS`],
//!   [`MAX_RECORDED_CALLS`], [`MAX_VERSION_LEN`] and [`ROOT_HASH_HEX_LEN`];
//! * the clock is a parameter, never ambient: [`Machine::step`] reads it
//!   through a [`Clock`], the only implementation is [`StubClock`], and no
//!   module names `SystemTime`;
//! * a transition allocates nothing: every value the machine holds is `Copy`
//!   and the trace is a fixed-size array. Rendering a trace to text does
//!   allocate, and is not the lifecycle path;
//! * no `unwrap`, `expect`, `panic!`, slice indexing or unchecked arithmetic,
//!   and no `unsafe` (forbidden at the workspace root).
//!
//! # What this crate does not do
//!
//! It does not verify a signature, compute or check a dm-verity root hash,
//! install or start a unit, touch a partition, or claim that a lifecycle it
//! accepts will boot. Every value that would come from the host arrives as
//! data. A pass of this crate's tests is evidence about the model, and closes
//! no image, boot, hardware or release gate.
//!
//! # Example
//!
//! ```
//! use aegis_janus_lifecycle::{
//!     Candidate, Event, Machine, RootHash, SignatureVerdict, Slot, State, StubClock, Tick,
//!     Timeout, Version,
//! };
//!
//! # fn main() -> Result<(), Box<dyn std::error::Error>> {
//! let signed = RootHash::parse_hex(&"ab".repeat(32))?;
//! let candidate = Candidate::new(Version::parse("1.2.3")?, Slot::B, signed);
//! let timeout = Timeout::from_ticks(600).ok_or("a watchdog timeout must be positive")?;
//! let clock = StubClock::new(Tick::new(1_000));
//! let mut machine = Machine::new();
//!
//! machine.step(&clock, Event::Declare(candidate))?;
//! machine.step(&clock, Event::CheckSignature(SignatureVerdict::Valid))?;
//! machine.step(&clock, Event::AcquireDelta(signed))?;
//! machine.step(&clock, Event::SwapSlot)?;
//! machine.step(&clock, Event::ArmWatchdog(timeout))?;
//! machine.step(&clock, Event::Bless)?;
//!
//! assert_eq!(machine.state(), State::Blessed);
//! let rendered = machine.trace().render()?;
//! assert!(rendered.starts_with("{\"schema\":\"aegis.p02.ab-transition.v1\""));
//! assert_eq!(rendered.lines().count(), 7);
//! # Ok(())
//! # }
//! ```

pub mod candidate;
pub mod clock;
pub mod decision;
pub mod error;
pub mod machine;
pub mod ports;
pub mod slot;
pub mod state;
pub mod trace;
pub mod verity;

pub use crate::candidate::{Candidate, MAX_VERSION_LEN, Version, VersionError};
pub use crate::clock::{
    Clock, ClockError, MonotonicGuard, StubClock, Tick, Timeout, Watchdog, WatchdogError,
    WatchdogStatus,
};
pub use crate::decision::{
    Citation, ConsolidationModel, D13_CONSOLIDATION, DecisionRecord, DecisionState,
};
pub use crate::error::LifecycleError;
pub use crate::machine::Machine;
pub use crate::ports::{
    CallKind, CallOutcome, MAX_RECORDED_CALLS, PortError, StubSysupdate, SysupdateCall,
    SysupdatePort,
};
pub use crate::slot::Slot;
pub use crate::state::{Event, EventKind, SignatureVerdict, State};
pub use crate::trace::{
    MAX_TRANSITIONS, RecordKind, TRACE_SCHEMA, Trace, TraceError, TraceHeader, TraceLine,
    Transition,
};
pub use crate::verity::{ROOT_HASH_BYTES, ROOT_HASH_HEX_LEN, RootHash, VerityError};
