<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# crates

Rust crates for the daemons proposed for P03, P04, P06, P07, P08, P09, P10,
P11, P13, P14, P15 and P16. A crate joins the workspace only with its own
`Cargo.toml`, a workspace lock entry, its interface contract, and positive,
negative and boundary tests. Joining the workspace is not the same as
activating the component: `planning/components.json` records which components
have left `proposal`, and several crates below carry a component's name while
the component stays a proposal.

## Activated

| Crate | Component | Milestone | Scope |
| :--- | :--- | :--- | :--- |
| `aegis-justitia` | P06 decision engine | M02, M14 | Risk tiers, maker-checker oversight, Annex III escalation, approval timeouts, a one-way killswitch, and a hash-linked audit ledger behind a hashing trait (M02). Versioned consumer schemas for the P09, P05 and P16 edges and the reviewed hardened-unit contract (M14). Library only. |
| `aegis-fabrica-defs` | P01/P02 declarative inputs | M03, M18 | The definition parser D15 records: `repart.d(5)` drop-ins and the `sysupdate.d(5)` transfer, split into what systemd refuses and what the recorded requirement refuses (M03). The Aegis product input manifest and the kernel requirement schemas, on bounded field types that validate while decoding (M18). Library only. |
| `aegis-janus-lifecycle` | P02 A/B candidate lifecycle | M15 | The A/B lifecycle D15 records: one candidate from declaration through the signature check, delta acquisition, slot swap and boot watchdog to bless or rollback, plus the D13 reopening. Pure state machine with an injected clock, stubbed systemd effects and a machine-readable transition trace. Library only. |
| `aegis-vulcan` | P03 direct-DMA validation slice | M17 | The validation arithmetic and the bounds of export-038: page-aligned BAR windows, the `1..=8192` block count, the lock-less submission-ring index and a bounded VFIO device table, plus the versioned descriptors P03 hands to P09 and to P15, each carrying the DMA-BUF export path M25 binds. Library only. |
| `aegis-hestia` | P15 store and overlay validation slice | M17 | The Rust half of decision D09: the vector-store initialisation gate and `1..=100` query bound, the picture-in-picture overlay controller, the versioned registration P15 hands to P04, and `HestiaView`, the typed boundary payload the Svelte package would read. Library only. |
| `aegis-tellus` | P13 carbon slice | M05 | The ISO/IEC 21031:2024 SCI rate `((E * I) + M) / R`, the 300 gCO2eq/kWh spatiotemporal defer threshold compared strictly, the sixteen-slice cgroup table, the `Delta V` bidding contract, and the zone-list wattage seam M21 substitutes a measured RAPL delta into. Versioned payloads for the P16 and P07 edges. Library only. |
| `aegis-athena` | P16 evolution loop | M05 | The seven-stage candidate lifecycle with only Invalidate terminal, the four-objective Pareto promotion gate, the nine structural maturity gates, and the SHA-256 hash-chained checkpoint ledger over the D02 trait. The versioned trigger P16 hands to P02, and the M14 audit record consumed rather than redefined. Library only. |
| `aegis-minerva` | P09 agent-chain middle link | M06 | The bounded Alps expert table and its 20 W envelope, the `AgentHER` trajectory buffer whose relabeller flips only failed steps, the constraint screen that can refuse and cannot accept, the versioned request P09 submits to P14, and the two payloads P09 builds from other crates' types rather than redefining. Library only. |
| `aegis-vesta` | P10 sandbox-controller validation slice | M06 | The bounded microVM and capsule tables of export-037, decision D06's Rust-native runtime with the rejected runtime unrepresentable on the wire, the capsule request P09 sends and the candidate evaluation P16 receives, each carrying the D58 VMM identity, and the recorded boot-time literal carried as an `Unmeasured` value. Library only. |

Every crate above is a member of the workspace root `Cargo.toml`. The member
list is written out rather than globbed, so the remaining reserved directories
stay inactive until they meet the same bar, and the crate tests check the
invariant rather than the count: a crate directory that has a manifest is a
member, and nothing else is. M17 settled open decision D22, which asked whether
the P03 and P15 Rust candidates join the list at all -- REQ-WS-01 had recorded
their absence from the *proposed* workspace as a defect. M05 added the two
crates REQ-P16-07 and REQ-P13-08 name, and M06 the two REQ-P09-01 and
REQ-P10-01 name.

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

`aegis-tellus` and `aegis-athena` carry their components' names and sit at their
components' proposed paths, and **P13 and P16 remain proposals all the same**.
The P13 subsystem is a Kepler eBPF telemetry daemon and this crate compiles no
probe and reads no counter; the P16 subsystem is an evolution daemon that runs
candidates and deploys updates, and this crate runs nothing and deploys nothing.
Their `manifest_present` flags in `planning/candidates.json` stay false for the
same reason the P03 and P15 flags do.

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

Deliberately outside `aegis-tellus`: every measurement. Nothing in it reads a
RAPL counter, opens `/sys/class/powercap`, compiles or loads an eBPF program,
walks a cgroup, connects to D-Bus, spawns a thread or sleeps. Every wattage it
reports is a recorded constant labelled `Simulated` or `Modelled`, and nothing
it produces carries `Measured`; `tests/stubbed_effects.rs` sweeps the crate's
own sources for twenty-four recorded identifiers on two lists -- one that is a
finding anywhere, one that is a finding only outside a string literal, because
the register records the very paths M21 will read -- and fails if one appears
where it would mean the effect exists. That is a regression gate over an
enumeration, not a proof over every such identifier, so a second check is held
beside it: no line of code under `src/` names `std::` at all. Neither sees the
standard macro prelude, which writes to a file descriptor without naming
`std::`, so `print_stdout`, `print_stderr` and `dbg_macro` are denied in
`[workspace.lints.clippy]` and clippy is what refuses those. `src/power.rs` is
where the substitution happens: the arithmetic receives a draw and an interval
and never learns where the number came from. A zone the reference profile
exposes takes the weakest of its figure's ingredients, so `package-0`, which
folds the modelled DRAM figure into a simulated core constant, is `Simulated`;
a zone it exposes no counter for is `Modelled` by D60 whatever figure stands in,
so `dram` and `psys` are both `Modelled` even though `psys` carries the
`package-0` number. Every zone's label is pinned in both directions.

Deliberately outside `aegis-athena`: every effect. Nothing in it opens a file,
runs `systemd-sysupdate`, connects to D-Bus, starts a microVM, opens an
`AF_VSOCK` socket, writes a partition or reboots; `tests/stubbed_effects.rs`
sweeps for twenty recorded identifiers on the same two-list terms, including the
`OpenOptions` call and the `SystemTime::now()` the imported scaffold actually
makes, and holds the same second check that no line of code under `src/` names
`std::`; the prelude's I/O macros escape both and are denied by the same three
workspace lints. The checkpoint ledger is in memory and unsigned: a chain
detects an edit, it does not establish who wrote it, and TPM2 sealing is M20.
The M14 audit record is decoded through `aegis_justitia::SignedAuditRecord`
rather than redefined, so a consumer's reading of it cannot drift from the
producer's.

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
