<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The P05 Forum shell model, and what its tests hold

Status: recorded observations from milestone M16, reference profile, 2026-09-29

ADR-0004 makes the whole P05 shell one native Rust program on gpui. Milestone
M16 builds the half of it that needs no toolkit, no display and no bus: the
crate `crates/aegis-forum-shell`. It holds the process lifecycle, the framing
of the two inbound edges, the consumers typed on the producers' own Rust
contracts, and the canvas model whose accessibility tree it exports as an
`accesskit::TreeUpdate`. Its tests run inside `make verify-all` through the
crate gate; to run them alone:

```sh
cargo test --locked -p aegis-forum-shell
cargo test --locked -p aegis-tellus --test contract_carbon_telemetry
```

**A pass is model-level evidence.** The crate paints nothing, creates no
window or surface, opens no real socket and contacts no D-Bus daemon. It closes
no accessibility gate for the running shell: M28 puts the model on gpui and
checks the tree over AT-SPI, and M29 runs the shell in a live session. P05
stays a proposal in `planning/components.json`.

## What is tracked

| Path | Role |
| :--- | :--- |
| `crates/aegis-forum-shell/src/lifecycle.rs` | the five states, D96's nine edges and `QUARANTINE_LIMIT` |
| `crates/aegis-forum-shell/src/processes.rs` | the managed process table and the stubbed P04, P06 and P13 inputs |
| `crates/aegis-forum-shell/src/jsonrpc.rs` | line framing under a byte bound and deadlines, and the Request checks |
| `crates/aegis-forum-shell/src/consumers.rs` | the two edges, decoded by `aegis-justitia` and `aegis-tellus` |
| `crates/aegis-forum-shell/src/session.rs` | one inbound stream: lines in, state updated, Responses out |
| `crates/aegis-forum-shell/src/endpoint.rs` | D32's endpoint and per-hop target, recorded and never asserted |
| `crates/aegis-forum-shell/src/canvas/` | the node registry, the camera, the `QuadTree` and the seeded canvas |
| `crates/aegis-forum-shell/src/focus.rs`, `walk.rs` | the keyboard model and the keyboard-only walk |
| `crates/aegis-forum-shell/src/a11y.rs` | the `TreeUpdate` export and the tree check |
| `crates/aegis-forum-shell/src/platform.rs` | the compositor link and the D-Bus interfaces, as traits |
| `crates/aegis-forum-shell/src/tokens.rs` | the P05/P12 token edge with its D05 direction |
| `crates/aegis-tellus/src/contracts/telemetry.rs` | the `CarbonTelemetry` payload, `aegis.p13-p05.carbon-telemetry.v1` |

The crate depends on accesskit 0.25.1, admitted on
`docs/roadmap/toolchain-admission.md`, and on the two producer crates by
path; nothing else is new in `Cargo.lock` but uuid 1.26.1, which accesskit
resolves with its features off.

## The recorded run

The crate's 77 tests passed on the pinned toolchain, rustc 1.98.1, on
2026-09-29, with clippy's `-D warnings` over every target:

| Test file | Tests | What it holds |
| :--- | :--- | :--- |
| `tests/lifecycle.rs` | 14 | E16-1: the D96 table, the forward chain, both recovery edges, Deleted terminal, the limit at 2 and 3 |
| `tests/consumers.rs` | 8 | E16-2: both payloads into the producers' types, the typed unknown-version refusal, zero watts, the D05 record |
| `tests/framing.rs` | 11 | D77 over a `socketpair(2)`: one line each, the JSON-RPC errors and the ids they carry, the byte bound, the deadlines, the discard bound |
| `tests/canvas_export.rs` | 12 | REQ-P05-11: 1,000 nodes exported with 10 in view, the dropped, hidden and clipped negatives, the missing empty state, the cull boundary, the camera |
| `tests/keyboard_walk.rs` | 12 | REQ-P05-09, REQ-P05-10: the walk against the pointer path, Escape and Tab, the trap, pointer-only, unrun-binding and unrevealable-node negatives, the widest revealable node, shortcuts |
| `tests/tree_check.rs` | 8 | E16-4: role and name on every node, the planted unlabelled node named, the empty state |
| `tests/platform_mocks.rs` | 3 | REQ-P05-05 on mocks: mount order, a missing watcher, the portal values |
| `tests/manifest_hygiene.rs` | 7 | criterion 1: the member, the lock, accesskit declared once, the D78 closure |
| `tests/stubbed_effects.rs` | 2 | the scope: no socket, bus, process, clock or file call in the sources |

`crates/aegis-tellus/tests/contract_carbon_telemetry.rs` adds nine tests for
the telemetry payload itself: the golden encoding, the version, edge and
provenance, every provenance by name, the unknown version, a negative draw,
an unknown provenance or field, a payload on another edge, zero watts, the
draw bound, and every golden fitting one line within the payload bound.

## The lifecycle (D96)

REQ-P05-03 names five states and no edge; D96 supplies them. The forward chain
is Eligible but Inactive, Activated, Rate-Limited, Quarantined, Deleted, in the
source's order. Rate-Limited returns to Activated when a window is back within
budget, and Quarantined returns to Eligible but Inactive on release. Every live
state may go to Deleted, Deleted is terminal, and no other edge exists: the
test compares all 25 pairs with the nine D96 lists. A process is quarantined
by the third consecutive over-budget window, `QUARANTINE_LIMIT = 3`; the
second leaves it rate-limited, and a window within budget between two
over-budget ones returns it to Activated and restarts the count.

The inputs come from stubs, never from a transport: P04 maps a surface or
reports an exit, P13 reports a budget window, P06 releases or deletes. Each
process carries its control-group slice, which is what the compositor is
handed on focus (REQ-P04-07), and only Activated and Rate-Limited processes
count as active capacity (REQ-P05-07). The lifecycle holds no clock.

## The two inbound edges (D77)

`DISPATCH_DECISION_REQUEST` and `EMIT_CARBON_TELEMETRY` each arrive as one
line-delimited JSON-RPC 2.0 message whose method is the graph edge and whose
`params` is the producer's payload. The consumer hands that object, as JSON
text, to `aegis_justitia::DecisionRequest::decode` or
`aegis_tellus::CarbonTelemetry::decode`; no schema file, generated binding or
copy in a second language exists. `CarbonTelemetry` is new in `aegis-tellus`,
where P13's schemas live: `power-watts`, `sci-rate`, `grid-intensity` and a
`provenance` that says whether the draw was measured, modelled or simulated,
with neither an emission interval nor a delivery deadline, because D31 is open.

The framing's bounds, each a named constant in `src/jsonrpc.rs` or
`src/session.rs`:

| Bound | Value | Test |
| :--- | :--- | :--- |
| `MAX_LINE_BYTES` | 8,192 bytes before the newline | a line of 8,192 bytes is applied; 8,193 is answered with `-32000` and the next line is read |
| read and write deadline | 250 ms by default, armed before every read and every write | an in-memory stream counts one armed deadline per read and per write; a socket with no line ends the session after the deadline and within 5 s, below the 10 s backstop the test fixture sets on both socket ends, so a read left without its deadline fails rather than hangs |
| `MAX_LINES_PER_SESSION` | 1,024 lines | a session bounded to 3 lines stops after 3 |
| `MAX_DISCARD_BYTES` | 1 MiB of one over-long line | the reader gives the stream up past it |

Answers follow the JSON-RPC 2.0 specification as read on 2026-09-29: invalid
JSON is `-32700` with id `null`; a batch, a non-object, a `jsonrpc` other than
`"2.0"`, a missing method, a scalar `params` or an object id is `-32600`,
carrying the Request's id when it could be read and `null` otherwise; an unknown
method is `-32601` and a refused payload `-32602`, each with the call's id. A
Notification is never answered, so a refused one is reported in the session's
report with the producer's typed error instead. After every refusal the next
line is read.

`SYNC_DESKTOP_SHELL` keeps the Unix socket stream the graph records. D32
adopts `/run/aegis/compositor.sock` as its endpoint and under 100
microseconds as its per-hop target; `src/endpoint.rs` records both as
`COMPOSITOR_ENDPOINT` and `COMPOSITOR_HOP_TARGET`, and no test reads either,
because no real P04 socket exists. Until one is measured, the figure is a
target, not a claim (ADR-0001).

## The canvas model and its tree (D74)

The seeded canvas is 1,000 nodes of 100 canvas units on a 50 by 20 grid with
a 200-unit stride, in reading order, which is also focus order. The seeded
camera looks at `(450, 150)` at scale 1.0 through a 1,000 by 400 pixel
viewport, so its visible region, `(-50, -50)` to `(950, 350)`, holds exactly
ten nodes. The `QuadTree` answers which nodes intersect that region, without
recursion and without allocating past the caller's buffer, and it agrees with
a scan of every node over five views.

The export lists every one of the 1,000 nodes as a child of the canvas, in focus
order, each with a role, a name and screen bounds; a culled node's bounds lie
outside the viewport. `check_canvas_export` refuses an update that drops the
culled nodes (it counts 10 of 1,000), one that hides them, one out of focus
order, and a canvas that sets `clips_children`; for an empty canvas it passes
only the labelled empty state as the canvas's one child. The refusal of a
clipping canvas comes from reading `accesskit_consumer` 0.39.1, the release that
pairs with accesskit 0.25: its `common_filter` (`src/filters.rs`) excludes the
subtree of a child whose bounds miss a clipping parent's, unless a neighbouring
sibling is inside them, and excludes every hidden node and every
`GenericContainer`. A clipping canvas would therefore hide most culled nodes
from AT-SPI at M28, so the export does not clip and the tree check refuses the
other two.

Focusing a culled node moves the camera until the node is fully in view, and
focusing one already in view leaves the camera still. A node whose edge lies
exactly on the cull boundary and one straddling it are both painted and
exported; one a unit further out is exported and not painted.

## The keyboard walk (REQ-P05-09, REQ-P05-10)

Tab and Shift+Tab move between the three shell regions, status bar, canvas and
decisions, and entering the canvas lands on the node focused last. Inside it,
Next and Previous move through focus order with the camera following, Enter
opens a node's instrument and then each nested level, Escape returns to the
owner, and each action is bound to Space or a Control chord. Single-letter
shortcuts are global commands that can be turned off or remapped, so no action
depends on one.

The walk visits all 200 nodes of a 10 by 20 grid and all 1,000 of the seeded
canvas, invokes every action once, opens every level of the three-deep
instruments (an instrument, a surrogate, a transcluded fragment), climbs out
with exactly as many Escape presses as levels, and checks from every level
that Tab reaches the decisions region and Shift+Tab returns to the node. The
canvas it walked and a copy walked by the pointer path end in the same state.
A level with a pointer-only action fails the walk naming the level and the
action, a fragment that swallows Tab fails as a trap, a binding that runs
another action fails naming the action it never ran, and a node wider than
the viewport shows at the smallest scale, 0.125, fails as not in view after
the camera zoomed out that far; a node exactly 8,000 units wide, the widest
the 1,000-pixel viewport shows at that scale, is revealed and walked. A
disabled shortcut no longer fires.

## The tree check (E16-4)

`check_tree` walks the exported tree from its root with an explicit stack and
requires every node to be reached exactly once, to carry a role an adapter
keeps and a non-blank name, and not to be hidden, and focus to rest on a node
of the tree; an empty canvas must export its labelled empty state, "The canvas
is empty". The default state and the seeded canvas with a pending decision,
telemetry and an open three-level instrument pass, 1,009 nodes. A planted
unlabelled node fails with `canvas node 4242 (#1099511632018) (Group) has no
accessible name`, and a blank name, `Role::Unknown`, `GenericContainer` and a
hidden node each fail. The model refuses an unlabelled node before it reaches
the tree, so the planted node stands for an exporter's defect.

This check is narrower than the axe-core scan D91 first planned, and ADR-0004
records it as a negative consequence: it asserts a role and a name on every
node and the labelled empty state, not the WCAG 2.2 AA rule set.

## What the dependency closure holds (D78)

`tests/manifest_hygiene.rs` runs `cargo metadata --format-version 1 --locked
--offline --all-features` under a 120-second deadline, starts at the one
workspace member named `aegis-forum-shell`, follows every resolved edge, and
refuses gpui, winit, wayland-client, zbus, atspi, accesskit_unix and tokio in
the closure, matching names exactly. On 2026-09-29 the closure held 28
packages: the crate, accesskit 0.25.1 and uuid 1.26.1, the two producer
crates, and the 23 packages of the M02 stack they already resolved (base16ct,
sha2, serde, serde_json, thiserror and the graph those five pull in).
Synthetic metadata shows the check refusing each forbidden name, directly and
one hop down, and failing closed on a missing start node, two of them or a
null resolve.

## Scope, and what a pass here does not mean

- The shell is not built: gpui, accesskit_unix, the layer surface, the Rust
  theme constants and the focus ring on rendered pixels are M28's; the portal,
  AT-SPI2 and the StatusNotifierWatcher run live at M29.
- The D-Bus interfaces are traits with in-memory mocks, and the compositor
  link is a stub; nothing here reads a portal setting or reaches a bus.
- No figure is measured: the D32 hop target is recorded, not asserted, and no
  frame or tree-update time is claimed.
- REQ-P05-12, REQ-P05-14 and REQ-P05-15 are planned by no milestone.
- The D100 lint that M16 also delivers covers the JavaScript and Svelte under
  `ui/`; it is recorded on
  [the accessibility harness page](accessibility-harness.md).
