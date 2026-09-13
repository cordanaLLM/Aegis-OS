<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# crates

Rust crates for the daemons proposed for P03, P04, P06, P07, P08, P09, P10,
P11, P13, P14, P15 and P16. A crate is activated only with its own `Cargo.toml`,
a workspace lock, its interface contract, and positive, negative and boundary
tests.

## Activated

| Crate | Component | Milestone | Scope |
| :--- | :--- | :--- | :--- |
| `aegis-justitia` | P06 decision engine | M02, M14 | Risk tiers, maker-checker oversight, Annex III escalation, approval timeouts, a one-way killswitch, and a hash-linked audit ledger behind a hashing trait (M02). Versioned consumer schemas for the P09, P05 and P16 edges and the reviewed hardened-unit contract (M14). Library only. |
| `aegis-fabrica-defs` | P01/P02 declarative inputs | M03, M18 | The definition parser D15 records: `repart.d(5)` drop-ins and the `sysupdate.d(5)` transfer, split into what systemd refuses and what the recorded requirement refuses (M03). The Aegis product input manifest and the kernel requirement schemas, on bounded field types that validate while decoding (M18). Library only. |
| `aegis-janus-lifecycle` | P02 A/B candidate lifecycle | M15 | The A/B lifecycle D15 records: one candidate from declaration through the signature check, delta acquisition, slot swap and boot watchdog to bless or rollback, plus the D13 reopening. Pure state machine with an injected clock, stubbed systemd effects and a machine-readable transition trace. Library only. |
| `aegis-vulcan` | P03 direct-DMA validation slice | M17 | The validation arithmetic and the bounds of export-038: page-aligned BAR windows, the `1..=8192` block count, the lock-less submission-ring index and a bounded VFIO device table, plus the versioned descriptors P03 hands to P09 and to P15, each carrying the DMA-BUF export path M25 binds. Library only. |
| `aegis-hestia` | P15 store and overlay validation slice | M17 | The Rust half of decision D09: the vector-store initialisation gate and `1..=100` query bound, the picture-in-picture overlay controller, the versioned registration P15 hands to P04, and `HestiaView`, the typed boundary payload the Svelte package would read. Library only. |

All five are members of the workspace root `Cargo.toml`. The member list is
written out rather than globbed, so the remaining reserved directories stay
inactive until they meet the same bar. M17 extended the list to five and
thereby settled open decision D22, which asked whether the P03 and P15 Rust
candidates join it at all: REQ-WS-01 had recorded their absence from the
*proposed* workspace as a defect, and a workspace-wide `cargo` invocation now
reaches both.

`aegis-fabrica-defs` owns the declarative inputs in `build/`, not the P01 or P02
component daemon. M18 widened it from a parser to the P01/P02 input contracts:
the product input manifest in `src/manifest.rs`, the kernel requirement payload
in `src/kernel.rs`, and the bounded field types both are built from in
`src/field.rs`. The manifest is validated against the reviewed definition files
rather than against a description of them, and the kernel requirement is checked
against the measured profile in `planning/hardware-profile.json`.

`aegis-janus-lifecycle` models what happens to a candidate release moving
through those inputs. Neither crate is the P01 or P02 component daemon.

P01 `aegis-fabrica` and P02 `aegis-janus-vallum` remain proposals in
`planning/components.json`: their activation blockers are image, kernel and
hardware evidence this crate does not produce.

`aegis-vulcan` and `aegis-hestia` carry their components' names and sit at their
components' proposed paths, and **P03 and P15 remain proposals all the same**.
Neither crate is the component it is named after: the P03 subsystem is a
direct-DMA driver and this crate maps no BAR and moves no byte; the P15
subsystem is a local-first store behind a shell, and this crate starts no store
and connects to no compositor. P15 has a second reason: D09 makes it two halves,
and the Svelte half has no package manifest, lockfile or accessibility test, so
`verify_candidates()` in `tools/verify_preparation.py` -- which requires every
candidate row of an activated component to record a manifest and a lock --
would refuse the claim. The `manifest_present` flag in
`planning/candidates.json` stays false for both for the same reason: that gate
reads it as a component-activation claim rather than as a filesystem fact, and
each row's note names the tracked workspace manifest and lock that do exist.
Activation covers the crate gate only: `cargo fmt`, `cargo build --locked`,
`cargo test` and `cargo clippy -D warnings` on the pinned toolchain. No native
pass is claimed by this directory.

The consumer contracts live in `aegis-justitia/src/contracts/` and the hardened
unit is a contract file in `aegis-justitia/contracts/`, reviewed by library code
and installed nowhere.

Deliberately outside `aegis-fabrica-defs`: every producer. Nothing in it opens a
file, contacts a repository, runs mkosi, builds a kernel, or produces or
verifies a digest or a signature. An artifact digest and an artifact signature
are field encodings; the caller supplies the definition text, and the mkosi gate
in `tools/verify_mkosi_definitions.py` is what runs a tool.

Deliberately outside `aegis-janus-lifecycle`: every effect. Nothing in it runs
`systemd-sysupdate`, opens a device, computes a dm-verity hash, verifies a
signature, changes a boot order or reboots. `SysupdateCall` describes the calls
a real implementation would make and `StubSysupdate` answers them from a script;
`tests/stubbed_effects.rs` sweeps the crate's own sources for twenty recorded
identifiers that would be needed to do any of it and fails if one appears -- a
regression gate over an enumeration, not a proof over every such identifier.
The clock is injected for the same reason: the watchdog boundary is only
meaningful if a test owns the time.

Deliberately outside `aegis-vulcan`: every effect and every transport. Nothing
in it maps a window, opens a VFIO container or a render node, issues an NVMe or
peer-to-peer transfer, exports a DMA-BUF or calls a driver. "Mapped" is a state
in a struct and a transfer is arithmetic; `tests/stubbed_effects.rs` sweeps the
crate's own sources for twenty-two recorded identifiers that would be needed to
do any of it and fails if one appears -- a regression gate over an enumeration,
not a proof over every such identifier. `src/mandate.rs` records the 20 W power
budget, the six protection boundaries and the 10 microsecond latency budget as a
register and enforces none of them.

Deliberately outside `aegis-hestia`: every effect, every transport and the whole
user interface. Nothing in it starts a PGlite instance, loads WebAssembly, opens
a database connection, touches a Btrfs subvolume, reads a file, connects to a
compositor or imports a DMA-BUF; `tests/stubbed_effects.rs` sweeps for twenty
recorded identifiers on the same terms. The D09 boundary is a payload rather
than an interface: `HestiaView` is a `Copy` snapshot carrying no method, handle
or file descriptor, so neither half can reach into the other.
`src/register.rs` records the P15 claims this repository cannot check, including
the one REQ-P15-06 itself marks as an unverified proposal figure.

Deliberately outside `aegis-justitia` at this milestone: every transport. There
is no D-Bus connection, no socket, no eBPF compilation or verifier load, no TPM2
signing and no file-backed ledger sink. A signature field in a schema is a field
encoding; the crate neither produces nor verifies a signature. The signing and
sink boundaries ship as traits so those deferrals are typed holes rather than
placeholders; the unbound signer fails closed.

## Reserved

The remaining directories hold no manifest and are not workspace members.
Candidates are indexed under `.workingdir/prepared/scaffold/crates/` (private,
gitignored; not present in a clone); they are inactive proposal data. See
`planning/components.json` and `docs/integration/stack.md` for activation
prerequisites and shared ownership.
