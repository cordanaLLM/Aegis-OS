<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The P05 Forum shell on gpui: mapping the layer surface

Status: recorded observations from M28 epic E28-2, reference profile,
2026-10-18

Milestone M28 puts the P05 Forum shell on gpui (ADR-0004). Epic E28-2 maps
the shell's `zwlr_layer_shell_v1` surface from a `MOUNT_SHELL_CANVAS` request
and waits for the compositor's first configure (ADR-0003 decision 3). The unit
tests for the request line, the window options and the outcome contract run
inside `make verify-all`. Mapping the surface needs a Wayland compositor, so
it runs here:

```sh
make verify-shell
```

**A pass is development evidence for the client half only (D79).** The gate
starts its own headless KWin (`kwin_wayland --virtual`) in a private
`XDG_RUNTIME_DIR` under `dbus-run-session`, and it never connects to the host
session's compositor. KWin is not P04: nothing here is evidence that P04
serves `zwlr_layer_shell_v1`. A pass closes no hardware, accessibility or
release gate.

## What is tracked

| Path | Role |
| :--- | :--- |
| `crates/aegis-forum-shell-gpui/src/surface.rs` | request decoding, gpui window options for the layer surface, the event loop, outcomes and exit codes |
| `crates/aegis-forum-shell-gpui/src/bin/aegis-forum-shell-gpui.rs` | the binary the gate drives: `aegis-forum-shell-gpui surface <request-line>` |
| `crates/aegis-forum-shell-gpui/tests/surface.rs` | Rust positive, negative and boundary tests that need no display |
| `tools/verify_shell.py` | the gate: builds the binary, starts KWin and two filter proxies, runs and judges the cases |
| `tools/wayland_filter.py` | a Wayland wire proxy that hides `zwlr_layer_shell_v1` or drops `zwlr_layer_surface_v1.configure` |
| `tools/test_verify_shell.py`, `tools/test_wayland_filter.py` | unit tests for both tools, run inside `make verify-all` |

## What the gate does

1. If the host lacks Linux, `AF_UNIX` with `SCM_RIGHTS`, `kwin_wayland`,
   `dbus-run-session`, `wayland-info` or `cargo`, it prints
   `SKIP: <reasons>; the native shell gate did not run.` and exits 0. A SKIP is
   not a pass.
2. It builds `aegis-forum-shell-gpui` with `cargo build --locked` at two jobs,
   unless `CARGO_BUILD_JOBS` says otherwise.
3. It starts KWin at 1280x720 and two `wayland_filter.py` proxies in front of
   it, one per mode.
4. It runs the five cases below. Every child has a 30 s timeout, and the
   binary waits at most `FIRST_CONFIGURE_DEADLINE` (5 s) for the first
   configure.
5. It prints one `CASE` line per case, then the D79 note, then
   `RESULT pass` only when all five cases ran and passed (`summary()` in
   `tools/verify_shell.py`).

The binary takes exactly one newline-terminated line, as the socket would
carry it. Its exit status names the outcome: 0 configured, 3 open or write
failure, 4 layer shell missing, 5 configure deadline, 6 request refused,
64 usage.

## The cases

| Case | Connection | Passes when |
| :--- | :--- | :--- |
| `surface/first-configure` | KWin directly | exit 0 and `OUTCOME configured 1280x48`: the full output width, anchored top, left and right, at the shell height |
| `surface/filter-control` | `wayland-info` through both proxies | the hide proxy lists `xdg_wm_base` but not `zwlr_layer_shell_v1`, and the drop proxy still lists `zwlr_layer_shell_v1`, so the two negative cases below test what they claim |
| `surface/layer-shell-missing` | hide proxy | exit 4 and an `OUTCOME layer-shell-missing:` line naming `LayerShellNotSupportedError` |
| `surface/first-configure-deadline` | drop proxy | exit 5 and an `OUTCOME deadline:` line naming `zwlr_layer_surface_v1.configure` |
| `surface/refused-line` | none | exit 6 and an `OUTCOME refused:` line naming `jsonrpc`, for a request whose `jsonrpc` member is `"1.0"` |

The refused case must name `jsonrpc`. A refusal for any other reason, such as
a missing newline, fails the case.

## The filter proxy

`tools/wayland_filter.py` relays one client to the compositor and passes file
descriptors through as `SCM_RIGHTS`. It parses only what it needs: registry
globals and binds, `get_layer_surface`, and `wl_display.delete_id`. When the
compositor deletes an id, the proxy forgets it, because the client reuses
freed ids for new objects. Without this, the proxy read a `wl_callback.done`
on a reused registry id as a truncated `wl_registry.global` and dropped the
connection.

A framing error ends that one relay and is reported on the proxy's stderr. The
client then sees the end of the stream, so the error cannot pass as a case
result.

## The recorded run

Reference profile, 2026-10-18T05:56+02:00, kernel 7.2.10-1-cachyos, packages
`kwin 6.8.0-1.1`, `wayland-utils 1.3.0-1.1` and `dbus 1.16.2-1.1`, gpui at the
revision pinned in `docs/roadmap/toolchain-admission.md`:

```text
CASE surface/first-configure pass
CASE surface/filter-control pass
CASE surface/layer-shell-missing pass
CASE surface/first-configure-deadline pass
CASE surface/refused-line pass
RESULT pass
```

Between the last `CASE` line and `RESULT pass`, the run printed its D79 note:
`NOTE D79: headless KWin kwin 6.8.0 (kwin_wayland --virtual); development
evidence for the client half only, not evidence that P04 serves
zwlr_layer_shell_v1` (one line in the output).

The first live runs failed, and each failure was a defect that is now fixed:

| Observed | Cause | Fix |
| :--- | :--- | :--- |
| three cases exit 6 | the gate passed the request without its newline | `REFERENCE_LINE` ends in `\n` |
| `surface/refused-line` passed anyway | that refusal was for the newline, not for `jsonrpc` | the judge requires `jsonrpc` |
| drop case exit 101, a wgpu EGL panic | the proxy dropped the connection on a reused id | `wl_display.delete_id` handling |
| hide case timed out (124) | gpui runs the launch callback before calloop's `EventLoop::run`, which clears an earlier stop request | `quit_from_loop` in `surface.rs` quits from inside the running loop |

## The AT-SPI cases

Milestone M28 epic E28-3 verifies that the shell's AccessKit tree reaches
AT-SPI live and a reader built on `atspi` and `zbus` reads it back on a
private D-Bus session.

### The private bus recipe

The gate never connects to the host session bus or operator compositor. It
provisions an isolated AT-SPI environment in `XDG_RUNTIME_DIR`:

1. Starts `dbus-daemon --session --nofork --print-address=1` with address
   `unix:path=<rt>/a11y-session-bus` and records `DBUS_SESSION_BUS_ADDRESS`.
2. Starts `at-spi-bus-launcher` with `GSETTINGS_BACKEND=memory`.
3. Sets accessibility status via `gdbus call --session -d org.a11y.Bus`
   `-o /org/a11y/bus -m org.freedesktop.DBus.Properties.Set`
   `org.a11y.Status IsEnabled '<true>'`.
4. Starts `at-spi2-registryd` directly in the same environment, because D-Bus
   service activation for the registry fails on a private non-systemd bus.
5. Queries the a11y bus address with `gdbus call --session -d org.a11y.Bus`
   `-o /org/a11y/bus -m org.a11y.Bus.GetAddress` and polls
   `Accessible.GetChildren` on `/org/a11y/atspi/accessible/root` until the
   registry responds.

### Cases and variants

The verification binary `aegis-atspi-readback` and fixture binary
`aegis-forum-shell-gpui a11y <request-line> <variant>` run across six cases:

| Case | Variant | Passes when |
| :--- | :--- | :--- |
| `atspi/sound` | `sound` | The fixture button has role button, name, and click action; focus is on the button; all 1,000 seeded nodes are reachable and named; hidden subtree is hidden; clicking the button mutates its name to "Forum opened" and emits `EVENT click`. |
| `atspi/late-client` | `sound` | A client started 0.5 s after the surface maps still receives the complete 1,000-node accessibility tree. |
| `atspi/negative-button-as-container` | `button-as-container` | Readback exits 1, fails only `atspi/button-exposed`, and names role "panel". |
| `atspi/negative-unlabelled` | `unlabelled` | Readback exits 1, fails only `atspi/every-node-named` (naming path index 517) and `atspi/seeded-reachable` (the row lost its "Node 517" name). |
| `atspi/negative-culled` | `culled` | Readback exits 1 and fails only `atspi/seeded-reachable` with "10 of 1000". |
| `atspi/negative-hidden-shown` | `hidden-shown` | Readback exits 1 and fails only `atspi/hidden-subtree-hidden`, naming "Hidden panel". |

`NEGATIVE_EXPECT` in `tools/verify_shell.py` holds the failing set per variant.
A negative that fails for any other reason, a read error (exit 3) included,
fails its gate case.

### AccessKit filtering and tree structure

- **Hidden subtree semantics**: `accesskit_consumer` (0.38.0 `src/filters.rs:22`)
  excludes hidden subtrees entirely from the exported AT-SPI tree. "Reported
  hidden" means no reachable node with a hidden label reports `State::Showing`.
  The fixture renders a visible sibling ("Shown panel") so the check proves
  the reader traversed that section of the tree.
- **Clips children finding**: `accesskit_consumer` (`src/filters.rs:54-90`) only
  culls off-screen list items when the parent element sets `clips_children()`.
  Because gpui never sets this property, all 1,000 seeded list items remain
  reachable in the AT-SPI hierarchy even though only 10 rows fit the viewport.
  The planted `culled` variant simulates incorrect culling by rendering only
  `0..IN_VIEW` rows.
- **Deadline unit fixture**: Subprocess calls against a hanging D-Bus interface
  risk blocking or destabilizing the live test harness. In accordance with
  HISS-02, `crates/aegis-forum-shell-gpui/tests/readback_deadline.rs` verifies
  `with_deadline` behavior against a mock `Silent` peer on a peer-to-peer
  `UnixStream`, ensuring zero-deadline, hanging calls, and responsive calls
  are tested deterministically without touching the live bus.
- **D79 note**: Live execution outputs
  `NOTE versions: at-spi2-core <v>, dbus <v>` and the D79 note verifying
  development evidence for the client half only.

### Evidence run

`make verify-shell` on the reference profile, 2026-10-18: all 11 cases passed,
with `NOTE versions: at-spi2-core 2.62.1, dbus 1.16.2` and headless KWin 6.8.0.
The readback read 1,005 nodes from the sound fixture: the application, the
window, the button, the list, 1,000 rows and the shown panel. The hidden
subtree was absent. The `hidden-shown` variant read 1,008 nodes, three more:
the panel and its two children. The `culled` variant read 15 nodes.

## What is not claimed

- That P04 serves `zwlr_layer_shell_v1`. The compositor here is KWin.
- Anything about pixels or frame timing. Those join `make verify-shell` later.
- A run on any host but the reference profile, or on any KWin version but the
  one recorded above.
