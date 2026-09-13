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

`aegis-justitia` is the first and, for now, the only member of the workspace
root `Cargo.toml`. The member list is written out rather than globbed, so the
remaining reserved directories stay inactive until they meet the same bar.
Activation covers the crate gate only: `cargo fmt`, `cargo build --locked`,
`cargo test` and `cargo clippy -D warnings` on the pinned toolchain. No native
pass is claimed by this directory.

The consumer contracts live in `aegis-justitia/src/contracts/` and the hardened
unit is a contract file in `aegis-justitia/contracts/`, reviewed by library code
and installed nowhere.

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
