<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Workstation slices: RAPL energy and KVM sandboxing

Status: recorded observations from milestone M21, reference profile, 2026-09-29

Milestone M21 takes two stubs M05 and M06 left behind and runs them on the
reference profile. The SCI engine's simulated wattage gets a measured
alternative: two reads of the package energy counter, a rollover-safe delta,
and the unchanged engine computing SCI from it (epic E21-1). The P10 sandbox
table gets a monitor behind it: Firecracker boots microVMs the table
admitted, and one candidate evaluation round-trips over `AF_VSOCK` (epic
E21-2). The half that needs no hardware runs inside `make verify-all`; the
half that needs the reference profile runs here:

```sh
make workstation-fetch    # once: the one networked step
make verify-workstation
```

**A pass is development evidence on the reference profile.** It qualifies no
hardware, closes no hardware, isolation or release gate, and is not release
evidence; re-measurement inside an Aegis image is a later acceptance. The memory
footprint is M22's (E22-1) under decision D71 and names the monitor that
produced it, as is the boot time, read from Firecracker's boot timer. Venus
GPU pricing (E21-3, REQ-P10-02) is deferred to M12 and not claimed.

## What is tracked

| Path | Role |
| :--- | :--- |
| `crates/aegis-tellus/src/rapl.rs` | the counter reading, the rollover delta and `MeasuredWattage`, the second implementation of M05's `WattageSource` seam; reads nothing |
| `crates/aegis-tellus-rapl` | the readings document and the RAPL cases; `aegis-tellus-rapl run <document>` is the binary the gate starts |
| `crates/aegis-vesta-sandbox/src/vm.rs` | the evaluation exchange over a running microVM; the Firecracker process itself (started from a controller row, reached over the hybrid vsock, killed, reaped and its sockets removed) is `saeptum-microvm`'s `vm` module since 2026-10-16 |
| `crates/aegis-vesta-sandbox/src/run.rs` | the host run and its eight cases; `aegis-vesta-sandbox` starts it |
| `crates/aegis-vesta-sandbox/src/guest.rs` | the evaluation answer `aegis-vesta-guest` gives as the microVM's init, served through `saeptum-microvm`'s listener |
| `saeptum-microvm` (plurano/saeptum, pinned in `Cargo.toml`) | the per-microVM Firecracker configuration file, the line framing and the guest listener, re-exported by `aegis-vesta-sandbox` |
| `build/sandbox/firecracker.pin.json` | the pinned Firecracker release and guest kernel |
| `tools/verify_workstation.py` | the gate, run as `make verify-workstation` and `make workstation-fetch` |
| `tools/test_workstation_slices.py` | the gate's half that runs inside `make verify-all` |

## The privileged read

`/sys/class/powercap/intel-rapl:0/energy_uj` is mode `0400`, owner root, on
the reference profile: the CVE-2020-8694 mitigation, which keeps the counter
from unprivileged processes. The criterion asks which privileged path the
reader takes. The maintainer decided on 2026-09-29: **the gate reads the
counter as root through passwordless `sudo -n`, read-only and scoped.** It
runs exactly one command under sudo, each time under a 10 s deadline:

```sh
sudo -n cat /sys/class/powercap/intel-rapl:0/energy_uj
```

Nothing else ever runs under sudo. The zone enumeration, the `name`
attributes and `max_energy_range_uj` are world-readable and are read
unprivileged. The gate probes nothing with sudo first; if `sudo -n` would
have to ask, the first read says so and the RAPL half prints a SKIP. Both
wordings are recognised: classic sudo's "a password is required" and
sudo-rs's "interactive authentication is required". The gate's own
unprivileged read of `energy_uj`, in its own process, is kept as the
negative test: it must fail. The gate refuses any other sudo argument vector
before it starts, whether sudo is named bare or by a path such as
`/usr/bin/sudo`; `tools/test_workstation_slices.py` holds that and the number
of places the gate names sudo.

The reader itself, `aegis-tellus-rapl`, never runs as root and reads no file:
it takes the gate's reads as text. Every other process the gate starts --
cargo, rustc, Firecracker, both binaries -- runs as the invoking user.

## The RAPL half

1. `ls -d /sys/class/powercap/*` in Python: every entry's `name`,
   `max_energy_range_uj` and the mode and owner of its `energy_uj` are
   recorded to `powercap.json` and compared with the recorded enumeration
   (`rapl/zone-enumeration`, a gate-side case).
2. One unprivileged read of `energy_uj`; its error is recorded.
3. Two reads through the scoped sudo, 5 s apart, each stamped at the midpoint
   of its call on the monotonic clock. With `--observe-wrap` the reads
   continue every 10 s until the counter is seen to wrap, one read after the
   wrap, at most 25 minutes.
4. The reads go to `aegis-tellus-rapl` as one readings document
   (`aegis.m21.rapl-readings.v1`), kept as `rapl-readings.json`.

The cases, all of which must pass:

| Case | What it holds |
| :--- | :--- |
| `rapl/recorded-range` | the live `max_energy_range_uj` is the recorded 65532610987, the figure the boundary tests use |
| `rapl/measured-sci` | E21-1 positive: two readings become a `MeasuredWattage` (provenance `measured`), the M05 seam hands its draw back, the draw over the interval is the delta's energy to within 1e-9, and the unchanged `SciEngine` computes SCI from it |
| `rapl/unprivileged-read-fails-closed` | E21-1 negative and the privileged-read criterion: the unprivileged read failed, and the reader refuses what it left with an error, never a default value |
| `rapl/dram-zone-refused`, `rapl/psys-zone-refused` | D60: a `dram` or `psys` source is refused outright, and the package source refuses to answer for either, instead of returning zero |
| `rapl/wrap-at-live-range` | E21-1 boundary on the live range: the live delta, placed so that it straddles the wrap, comes back unchanged and marked wrapped |
| `rapl/observed-wrap` | with `--observe-wrap`: a wrap seen between two real readings gives a draw within a factor of two of the median unwrapped draw |

### Rollover, and what the delta is accurate to

The counter is the processor's 32-bit energy register scaled into
microjoules by the kernel, and it wraps to zero after `max_energy_range_uj`.
A second reading below the first is treated as exactly one wrap:
`delta = (range - before) + after`. Three facts bound it, each checked on
2026-09-29:

- The kernel scales by an integer unit: `drivers/powercap/intel_rapl_common.c`
  sets `energy_unit = (ENERGY_UNIT_SCALE * MICROJOULE_PER_JOULE) >> value`,
  15258 thousandths of a microjoule for this CPU's 2^-16 J unit, and the range
  is `0xffffffff * 15.258`, which is exactly the recorded 65532610987. The
  true modulus is one raw unit, 15.258 uJ, above the range, so a wrapped delta
  is at most one unit short, never negative. From the range itself to zero
  the formula gives zero where one unit passed; `tests/measured_energy.rs`
  records that edge rather than hiding it.
- A second wrap inside one interval would be invisible. `MeasuredWattage`
  therefore refuses an interval over which a draw of 1000 W could reach the
  range (65.53 s on the reference range) and any draw above 1000 W, which is
  what a counter reset read as a wrap produces.
- The midpoint stamp leaves each end of the interval uncertain by half of its
  read's latency; the run prints the slowest read, about 22 ms, so a 5 s
  interval is known to within about 0.4 per cent.

**Accuracy, stated where the number is used.** On this AMD package the
counter is a model-based estimate the processor derives from activity
counters, not a measured power rail. Only a delta within one zone is treated
as trustworthy; absolute watts carry the vendor's error, and the kernel's
truncated unit understates every reading by a further 0.0052 per cent (15.258
against 15.2587890625 uJ). The `measured` provenance says the number came
from a counter, not that the counter is a meter. Every run prints this
statement.

## The sandbox half

1. The cached Firecracker binary, its LICENSE, its API spec, the guest kernel
   and its configuration are hashed against
   `build/sandbox/firecracker.pin.json`; a mismatch is a FAIL. `--version`
   must print `Firecracker v1.17.0`, and the kernel configuration must carry `CONFIG_BLK_DEV_INITRD=y`,
   `CONFIG_KVM_GUEST=y`, `CONFIG_SERIAL_8250_CONSOLE=y`,
   `CONFIG_VIRTIO_MMIO=y` and `CONFIG_VIRTIO_VSOCKETS=y`.
2. `aegis-vesta-sandbox` is built, and `aegis-vesta-guest` is built with
   `--release --target x86_64-unknown-linux-gnu` and
   `RUSTFLAGS=-C target-feature=+crt-static` in that child's environment
   only. The gate refuses a guest whose ELF program headers name an
   interpreter, then writes a deterministic newc initramfs holding `/dev`,
   `/dev/mem` (character device 1:1, mode 0600) and `/init`; nothing else is
   in the guest. The kernel mounts no devtmpfs over an initramfs, so the
   guest's one device node is packed into the archive.
3. The gate reads the device limits from the release itself
   (`sandbox/passthrough-limits`, a gate-side case; see
   [The device passthrough limit](#the-device-passthrough-limit-m22-e22-1)).
4. `aegis-vesta-sandbox run` runs eight cases, one microVM at a time, in its
   own session, under a 600 s deadline.
5. The gate looks for anything the run left behind: a Firecracker or
   `aegis-vesta-sandbox` process whose command line names the run directory,
   a socket file under it, or a tun or tap device in `/sys/class/net` that
   was not there before (`sandbox/nothing-left-behind`, a gate-side case). A
   shell, `tail` or pager that merely names the run directory is not the
   run's and is left alone. A tap device is the only network device
   Firecracker attaches, and only tun and tap devices carry a `tun_flags`
   attribute; the `veth` and bridge devices other workloads' containers add
   and remove are not the run's, which four runs on 2026-10-17 failed on
   (r20261017T022819-f20b, r20261017T031922-b721, r20261017T154108-f92a,
   r20261017T175934-056a).

Every microVM is a row the `aegis-vesta` controller admitted first, and the
guest context identifier Firecracker is given is the one the controller
derived, `3 + vm_id`. Each is booted with `--no-api --config-file`, so no API
socket exists; the configuration names the kernel, the initramfs, one vCPU,
64 MiB and one vsock device, with `drives` and `network-interfaces` empty, so
there is no block device and no tap device. The guest listens on `AF_VSOCK`
port 5210; the host reaches it through Firecracker's hybrid vsock
(`docs/vsock.md` at v1.17.0): an `AF_UNIX` connection to the device's socket,
`CONNECT 5210`, an `OK <port>` acknowledgement, then one request line and one
answer line. Firecracker's own virtio-vsock device model carries the bytes;
the host's `vhost_vsock` is not in the path. The guest binds each request to
the sandbox it arrived in by its own context identifier, judges the metrics
with P16's Pareto gate and answers with the
`aegis.p10-p16.candidate-evaluation.v1` payload M06 typed. It answers at most
16 connections and waits at most 120 s for the next, then restarts; with
`reboot=k` Firecracker answers the restart by exiting. The run kills every
microVM long before either bound, so a probe on 2026-09-29 checked that end
separately: one microVM answered 16 connections, printed `answered 16
connection(s); restarting`, and Firecracker exited 0.

| Case | What it holds |
| :--- | :--- |
| `sandbox/over-limit-memory-refused` | E21-2 negative: 1025 MiB, one above the 1024 MiB guest limit, is refused by `GuestMemoryMib` before any row or process exists |
| `sandbox/boot-and-round-trip` | E21-2 positive: the first admitted microVM boots, a superior candidate comes back `passed` and one at the 1.5 ms latency bound comes back `failed`, each decoded and checked against the host's own Pareto verdict |
| `sandbox/vsock-transport` | M22 / E22-2: the first microVM answers 12 more evaluations, each timed from the `AF_UNIX` connect to the close; the figure names Firecracker 1.17.0, the host's `/dev/vhost-vsock` and `vhost_vsock` build state are recorded, and a `vhost_vsock` reference count that rose between the first boot and the timed run fails the case, since Firecracker's transport bypasses vhost |
| `sandbox/sixty-fourth-accepted` | E21-2 boundary: microVMs 2 to 64 are admitted, booted and each answers, and all 64 Firecracker processes are running at once |
| `sandbox/footprint` | M22 / E22-1: the resident set (`VmRSS` of `/proc/<pid>/status`) of each of the 64 Firecracker processes after its first answer, as minimum, median and maximum; every process's `/proc/<pid>/exe` must be the pinned binary, and a process naming no monitor or another one fails the case rather than entering the figure (D58) |
| `sandbox/boot-time` | M22 / E22-1: each of the 64 microVMs is started with Firecracker's boot timer on; its guest, as pid 1, writes the boot-complete byte 123 to the timer at `0xc000_0000` through `/dev/mem` (`saeptum_microvm::guest::signal_boot_complete`), and Firecracker logs one `Guest-boot-time` line to the microVM's `console.log`; wall clock and CPU time are printed as minimum, median and maximum, and a microVM without exactly one line fails the case, naming it |
| `sandbox/sixty-fifth-refused` | E21-2 boundary: the 65th request is the controller's `MicroVmTableFull`, and no process is started for it |
| `sandbox/teardown` | every row terminated, every process killed and reaped within 5 s, every socket file removed |
| `sandbox/passthrough-limits` | M22 / E22-1 boundary, gate-side: every endpoint the pinned release's API spec offers is one the pin reviewed and lists, and every listed one is offered; an endpoint nobody reviewed fails the case, because it could attach a host device |

Guest memory is 64 MiB, the smallest power of two the pinned kernel boots in
with this initramfs: on 2026-09-29 a probe booted 60 MiB and 64 MiB guests
and saw 48 MiB and 56 MiB guests panic out of memory before init. It is a
configuration value, not a footprint measurement. Sixty-four such guests
need about 4 GiB, so the gate skips unless the host reports at least
9216 MiB available. The pinned Firecracker binary, guest kernel and guest
init are x86_64, so on any other machine the sandbox half and
`make workstation-fetch` print a SKIP naming the architecture instead of
starting a binary the host cannot execute.

## The pins

`make workstation-fetch` is the one networked step. Nothing it downloads is
written into the repository; it lives under `AEGIS_WORKSTATION_DIR`, default
`${XDG_CACHE_HOME:-$HOME/.cache}/aegis-workstation`.

| Artifact | Pin | Source |
| :--- | :--- | :--- |
| Firecracker release archive | `firecracker-v1.17.0-x86_64.tgz`, 7464385 bytes, sha256 `06094a1108ae9e82aa4c23a775aa92758f53f1175d422270d9d6162cb9ade558` | the v1.17.0 GitHub release of 2026-09-10; the fetch also requires the release's `.sha256.txt` to name the same digest |
| `firecracker` binary | `release-v1.17.0-x86_64/firecracker-v1.17.0-x86_64`, sha256 `99ad0f5cd0514a88aad0e9ae8cfdb3cc3b4ab9d190e1194602406c786b5de7a5` | extracted from the archive; the archive's own `SHA256SUMS` names the same digest |
| Firecracker licence | `LICENSE`, Apache-2.0, sha256 `cfc7749b96f63bd31c3c42b5c471bf756814053e847c10f3eb003417bc523d30` | extracted beside the binary |
| Firecracker API spec | `release-v1.17.0-x86_64/firecracker_spec-v1.17.0.yaml`, sha256 `473a0a392927b407834b341bf5fbc7925524fedaf9d925e1bbbafb6a1f48c671` | extracted beside the binary; the archive's own `SHA256SUMS` names the same digest |
| Guest kernel | `vmlinux-6.18.44`, 27846248 bytes, sha256 `d8ced68bd61e27b6813e2c993cc53a4029c59e13210672180591c84109684fe4` | Firecracker's CI artifacts, `firecracker-ci/20260909-a8e1c3830545-0/x86_64/` |
| Guest kernel configuration | `vmlinux-6.18.44.config`, sha256 `9fb2be18303d2f6e8ec35b3a20ecf1209f54a4ece10114a893cb37beabee7030` | the same prefix |

The jailer is not used: it needs root to chroot and change user, and the
decision above admits no privilege beyond the one read. The guest kernel is
Firecracker's own CI build because it is the kernel the release is validated
against: its `docs/kernel-policy.md` at v1.17.0 supports 6.18 guests from
v1.16.1 until 2028-06-01, and 6.1's minimum support ended on 2026-09-02. The
artifact set is the one current when v1.17.0 was published on 2026-09-10; it
is pinned by the sha256 computed at admission, because the bucket publishes
none. Neither the kernel M26 builds nor the Nucleus release M10 boots was
reused: both configurations read `# CONFIG_VSOCKETS is not set` and
`# CONFIG_VIRTIO_MMIO is not set` (checked on 2026-09-29), so neither guest
could reach a vsock device on Firecracker's virtio-mmio bus.

## The device passthrough limit (M22, E22-1)

E22-1's boundary asks for the sandbox VMM's device passthrough limits to be
read from the pinned release rather than assumed. The release archive ships
its own OpenAPI document, `firecracker_spec-v1.17.0.yaml`, and every device a
Firecracker microVM can carry is configured through one of its endpoints. The
pin lists all 26 of them. The device endpoints are `/drives/{drive_id}`,
`/network-interfaces/{iface_id}`, `/vsock`, `/pmem/{id}`, `/entropy`,
`/balloon`, `/serial` and `/hotplug/memory`; each configures a device
Firecracker emulates itself, and none names a host device. The document
contains no `vfio`, no `passthrough` and no `pci`.

The PCI support v1.17.0 carries is a transport, not passthrough: its
`CHANGELOG.md` (#5364) adds `--enable-pci`, which creates the same VirtIO
devices on a PCI VirtIO transport instead of MMIO, and `docs/device-hotplug.md`
hot-plugs those virtio devices only. Firecracker therefore has no PCI
passthrough, `VmmIdentity::Firecracker` reports none
(`crates/aegis-vesta/tests/decision_register.rs`), and passthrough work stays
with the QEMU-based harness (D58) rather than being worked around here.

`sandbox/passthrough-limits` reads the endpoints from the cached spec, after
the cache check has matched its digest, and compares them with the pin's list
in both directions. An endpoint the spec offers and the pin lacks fails the
case: a release that adds one must have it reviewed for device passthrough
before the pin admits it. An endpoint the pin lists and the spec lacks fails
too, because then the list no longer describes the release. The planted cases
are in `tools/test_workstation_slices.py` (`PassthroughLimitTests`).

## The recorded run

Run r20260929T200422-2476 of `make verify-workstation`, on 2026-09-29, on the
reference profile, the AMD Ryzen 9 9950X3D 16-Core Processor with kernel
7.2.8-1-cachyos; the readings, the enumeration and the child logs are kept
under `~/.cache/aegis-workstation/runs/r20260929T200422-2476`. The host model
and kernel were printed on that run's output and not kept in its directory;
from run r20260929T205659-3afc on, every run keeps them in `host.json` beside
`rapl-readings.json`, as the review revision below records. The gate printed
the
enumeration `intel-rapl, intel-rapl:0 (package-0), intel-rapl:0:0 (core)`,
`max_energy_range_uj` 65532610987 for both zones and `energy_uj` mode `0400`
owner root for both, and every case passed:

- `rapl/zone-enumeration`: as above; no `dram` and no `psys` zone.
- `rapl/recorded-range`: 65532610987.
- `rapl/measured-sci`: `energy_uj` 45514845753 then 46107552971, 5015051212 ns
  apart; delta 592707218 uJ; draw 118.186 W, provenance `measured`; energy
  0.000164640894 kWh through the M05 seam; SCI 0.0862209967 gCO2eq per
  functional unit, `((E * I) + M) / R` with I 220 gCO2eq/kWh, M 0.05 g and R
  1, the interval. `crates/aegis-tellus/tests/measured_energy.rs` recomputes
  these figures by hand from the two readings.
- `rapl/unprivileged-read-fails-closed`: `PermissionError: [Errno 13]
  Permission denied`, and the reader refused it: "an energy counter read of an
  unreadable counter was refused; no default value stands in for it".
- `rapl/dram-zone-refused` and `rapl/psys-zone-refused`: building either
  source over the live pair was refused, and the package source answered
  "does not carry the dram zone" and "does not carry the psys zone".
- `rapl/wrap-at-live-range`: the live delta placed from 65236257378 to
  296353609 came back as 592707218 uJ, wrapped once.
- The slowest of the two privileged reads took 22.3 ms.
- Sandbox: Firecracker v1.17.0 and the guest kernel matched their pins; the
  initramfs held the static `/init`.
- `sandbox/over-limit-memory-refused`: 1025 MiB refused before any row or
  process; 1024 MiB admissible as a request.
- `sandbox/boot-and-round-trip`: sandbox 100, vsock context 103, booted under
  Firecracker with 64 MiB and accepted on port 5210; the superior candidate
  came back `passed` and the latency-bound one `failed`, both decoded as
  `aegis.p10-p16.candidate-evaluation.v1`; the guest clock stamped
  1790712268.
- `sandbox/sixty-fourth-accepted`: the 64th request was admitted as sandbox
  163, booted and answered; 64 Firecracker processes ran at once.
- `sandbox/sixty-fifth-refused`: "the microVM table holds its maximum of 64
  sandboxes"; still 64 processes.
- `sandbox/teardown`: 64 rows terminated, 64 processes killed and reaped, 64
  socket files removed.
- `sandbox/nothing-left-behind`: no process named the run directory, no
  socket file remained, and the 12 network devices were the same before and
  after.

The whole run took 26 s of wall clock.

Run r20260929T201527-d65a, the same day with `--observe-wrap`, passed every
case again, the sandbox half with the same figures and the same initramfs
digest, and added the observed wrap. Seven privileged reads 10 s apart, the
slowest 26.0 ms, read 59374959060, 60770719871, 62074613409, 63426207666,
64527611843, then 121324200 and 1297006778:

- `rapl/observed-wrap`: pair 4, 64527611843 then 121324200, 10006796487 ns
  apart, is one wrap of 1126323344 uJ, a draw of 112.556 W against a median
  of 130.292 W over the 5 unwrapped pairs; one wrapped pair in six.
- `rapl/measured-sci` was computed over that wrapped pair: energy
  0.000312867596 kWh, SCI 0.1188308710 gCO2eq per functional unit.
- `crates/aegis-tellus/tests/measured_energy.rs` and
  `crates/aegis-tellus-rapl/tests/cases.rs` replay both runs' readings.

Run r20260929T202807-0aa5 passed every case again with the same sandbox
figures and initramfs digest, and printed the compiler, rustc 1.98.1
(48a229cea 2026-09-01): `energy_uj` 28650559757 then 29162279281, 5012018419 ns
apart, 511719524 uJ, 102.098 W and SCI 0.0812717487. The gate then came to skip
the RAPL half on a host without cargo as the sandbox half already did, and run
r20260929T203429-7568, on the final revision, passed every case once more:
20026052066 then 20579592196, 5014224403 ns apart, 553540130 uJ, 110.394 W and
SCI 0.0838274524, the slowest read 20.9 ms, with the same sandbox figures and
initramfs digest.

Review of that revision found that the run directory did not keep the host
model and kernel, that a path-qualified sudo passed the scope check, that
sudo-rs's "interactive authentication is required" was a FAIL where it must
be a SKIP, that the leftover sweep killed any process naming the run
directory, and that a non-x86_64 host failed where it must skip. On the
revision that fixed all five, run r20260929T205659-3afc passed every case once
more and keeps `host.json`, whose record reads the AMD Ryzen 9 9950X3D
16-Core Processor, kernel 7.2.8-1-cachyos and rustc 1.98.1 (48a229cea
2026-09-01), beside `rapl-readings.json`, and `gate.log`, everything the gate
printed, gate-side cases and the pinned Firecracker version line included:
`energy_uj` 56916637446 then 57506260472, 5012254117 ns apart, 589623026 uJ,
117.636 W and SCI 0.0860325183, the slowest read 18.2 ms, with the same sandbox
figures and initramfs digest, and no Firecracker or `aegis-vesta-sandbox`
process, no socket file and the same 12 network devices after the run.

Two probes the same day showed the other two outcomes. With
`AEGIS_WORKSTATION_DIR` pointing at an empty cache (r20260929T202843-93fa)
the RAPL half passed, the sandbox half printed four `SKIP` lines each naming
`make workstation-fetch`, and the gate exited 0. With one byte of the cached
kernel flipped (r20260929T202855-b942) the sandbox half failed, naming both
digests, before anything booted, and the gate exited 1.

## The recorded transport figure (M22, E22-2)

Run r20261017T022908-f3ef of `make verify-workstation`, on 2026-10-17, on the
reference profile with kernel 7.2.10-1-cachyos, passed every case, the
gate-side `sandbox/nothing-left-behind` included; its `gate.log` is kept under
`~/.cache/aegis-workstation/runs/r20261017T022908-f3ef`. The
`sandbox/vsock-transport` case printed:

- host: `/dev/vhost-vsock` present, `CONFIG_VHOST_VSOCK=m`; `vhost_vsock`
  references 0 before the first boot and 0 while sandbox 100 ran, so the
  transport did not use vhost, as Firecracker's `docs/vsock.md` at v1.17.0
  describes;
- Firecracker 1.17.0, 12 round trips on sandbox 100: min 188.7 us, median
  197.6 us, max 494.6 us.

A round trip is everything the host waits for: the `AF_UNIX` connect, the
`CONNECT`/`OK` handshake, one request line, the guest's Pareto judgement, one
answer line and the close. The figure is a wall-clock time on a shared
workstation, not a price and not a Firecracker figure for any other host; a
QEMU figure would be a separate record (D58). Twelve is the most the first
microVM can answer: the boot case's three exchanges and these stay below the
guest's 16-connection bound, and `tests/transport.rs` holds that sum.

The run before it, r20261017T022819-f20b (same build, a minute earlier),
measured a median of 266.5 us and a max of 949.3 us and
failed only `sandbox/nothing-left-behind`: a Docker `veth` device belonging to
another workload disappeared during the run. The microVMs configure no
network device.

## The recorded footprint (M22, E22-1)

Run r20261017T032010-9ff3 of `make verify-workstation`, on 2026-10-17, on the
same reference profile and kernel, with saeptum at `24eef8c`, passed every
case, the gate-side `sandbox/nothing-left-behind` included; its `gate.log` is
kept under `~/.cache/aegis-workstation/runs/r20261017T032010-9ff3`. The
`sandbox/footprint` case printed:

- Firecracker 1.17.0, 64 processes, each one's `/proc/<pid>/exe` the pinned
  binary;
- resident set (`VmRSS`) after each microVM's first answer: min 56556 KiB,
  median 59120 KiB, max 66244 KiB.

The figure is the monitor process's resident set: Firecracker's own memory
plus the part of the 64 MiB guest it has touched, read while all 64
processes ran. It is not the guest's view of its memory, it is compared with
no target, and a QEMU figure would be a separate record (D58). The run before
it, r20261017T031922-b721 (same build), printed min 56128 KiB, median
58896 KiB, max 66244 KiB and failed only `sandbox/nothing-left-behind`,
again because a Docker `veth` device of another workload disappeared.

## The recorded boot time (M22, E22-1)

Run r20261017T124857-0027 of `make verify-workstation`, on 2026-10-17, on the
same reference profile and kernel, with saeptum at `d5e6b94`, passed every
case, the gate-side `sandbox/nothing-left-behind` included; its `gate.log` is
kept under `~/.cache/aegis-workstation/runs/r20261017T124857-0027`. The
`sandbox/boot-time` case printed:

- Firecracker 1.17.0, 64 microVMs, each with one `Guest-boot-time` line
  after its guest's boot-complete write;
- wall clock from the start of the microVM build: min 254112 us, median
  306210 us, max 551196 us;
- CPU time of the boot: min 249661 us, median 300213 us, max 376637 us.

The same run's footprint was min 57972 KiB, median 61620 KiB, max
68292 KiB. The store that signals the timer is `unsafe` code; it lives in
`saeptum-microvm` under one dated exception (category D, expires
2027-02-14), and `aegis-vesta-sandbox` keeps `unsafe_code = "forbid"`.

## What is not claimed

- **A boot time target.** `sandbox/boot-time` records Firecracker's own
  timer, which starts when Firecracker begins building the microVM, not when
  the request arrives, and is compared with no target. The microVMs boot one
  after another while earlier ones keep running, so the spread reflects a
  host growing to 64 guests. REQ-P10-01's target of 125 ms is missed: the
  median, 306 ms, is now `saeptum-capsule`'s `MEASURED_BOOT_TIME_MS`, which
  names Firecracker 1.17.0 and run r20261017T124857-0027, and the target
  stays `Unmeasured`.
- **A footprint target.** `sandbox/footprint` records the monitor process's
  resident set, which includes the guest memory it has touched; it is not the
  guest's view of its memory and is compared with no target. REQ-P10-01's
  target of 5 MiB is missed: the median, 61620 KiB, is now
  `MEASURED_FOOTPRINT_KIB`, on the same terms, and the target stays
  `Unmeasured`.
- **A meter.** The energy counter is AMD's model; see the accuracy statement.
  Only the package zone is read: `core` is recorded in the enumeration but
  its counter is not read, because the decision admits one command.
- **`AF_VSOCK` pricing.** REQ-P10-03 asks every `AF_VSOCK` message to be
  priced; this run proves the transport path, times it, and prices nothing.
- **Isolation.** No jailer, no seccomp change, no cgroup and no namespace
  beyond what Firecracker applies by itself; every microVM runs as the
  invoking user. Nothing here is evidence about isolation strength.
- **Venus.** E21-3, REQ-P10-02, is deferred to M12.
- **CI.** The Verification gate's runner has no powercap counter, no
  passwordless sudo and no `/dev/kvm`, so `make verify-workstation` is not
  part of `make verify-all`; its hardware-free half is.

## Host safety

Nothing is installed; no GPU, driver, module or sysctl is touched; sudo runs
the one read above and nothing else. Every microVM has no drive and no
network interface, so no tap device exists, and the gate compares
`/sys/class/net` before and after. The host run lives in its own session: if
it overruns its deadline the gate kills the whole session, Firecracker
processes included, and a leftover Firecracker or `aegis-vesta-sandbox`
process naming the run directory is killed and fails the gate; no other
process is signalled. Every socket file is removed by the run and looked for
by the gate. Each run prints its id and keeps under that id `host.json` (the
CPU model, kernel and rustc), `gate.log` (everything the gate printed), the
readings document, the enumeration, the child logs, the initramfs and each
microVM's configuration and console; nothing is written into the repository
but cargo's target directory.

## VFIO BAR mapping (M25, E25-1)

`aegis-vulcan-gpu map-bar --device <PCI addr> [--sysfs ROOT] [--dev ROOT]`
maps one Base Address Register (BAR) of a GPU through VFIO as an unprivileged
user, once the operator has bound the device to `vfio-pci` and handed the user
the group device node.

Before it opens any device node, the tool reads sysfs and refuses the
candidate when:

- its driver is not `vfio-pci` (the refusal names the bound driver);
- it shares its IOMMU group with another PCI function;
- a display connector other than Writeback is not `disconnected`;
- it is the boot VGA device.

An admitted candidate gets `<dev>/vfio/vfio` and `<dev>/vfio/<group>` opened,
the API version and the type-1v2 IOMMU extension checked, the group's
viability checked, the group set on the container and the device descriptor
fetched. The tool queries BARs 0..=5 and maps 4 KiB of the first BAR that is
mmap-capable, at least 4 KiB long and prefetchable by the flags column of
sysfs `resource` (`IORESOURCE_PREFETCH`, `0x2000`). Register BARs are never
mapped, because a read of device registers can have side effects. On the
reference profile's Arc A380 that is BAR2, the 8 GiB VRAM aperture; BAR0, the
16 MiB register space, is skipped. The mapping is read-only, read with
volatile 32-bit loads, hashed with SHA-256 and unmapped.

`crates/aegis-vulcan-gpu/tests/vfio_tests.rs` holds the refusals, the BAR
choice against the Arc's recorded `resource` table and the UAPI ioctl numbers
and struct layouts.

### The operator run

Binding a device to `vfio-pci` is the operator's act; the tool, the tests and
the gates never do it. `tools/operator/e25-1-vfio-run.sh` is that act for the
Arc A380 at `0000:03:00.0`:

```sh
bash tools/operator/e25-1-vfio-run.sh            # preview, writes nothing
bash tools/operator/e25-1-vfio-run.sh --apply    # bind, map-bar, rebind xe
```

It refuses unless the address is the Arc (`8086:56a5`) on `xe`, alone in IOMMU
group 17, with no connected output. With `--apply` it binds `vfio-pci`, gives
the user `/dev/vfio/17`, runs `map-bar`, and from an exit trap unbinds,
clears `driver_override`, reprobes and reads back the driver of the Arc and of
the NVIDIA device at `0000:06:00.0`, which it never writes to. The log goes to
`.workingdir/evidence/e25-1-vfio-<UTC time>.log`.

It also refuses, in the preview as well, while any process holds the Arc's
card or render node, read by PCI path under `/dev/dri/by-path/` because the
`cardN` numbers move between boots, and it names each holder by pid and
command. It signals no process. The run of 2026-10-18 13:33 listed a holder
of `card2` and went on: the map passed and the Arc returned to `xe`, but KWin,
which used the Arc as a hotplug GPU, logged `Failed to open drm node
/dev/dri/renderD130` and hung the session until a reboot. On the reference
profile after that reboot, the preview refuses with
`REFUSE: pci-0000:03:00.0-card held by 5184 (Xwayland)`.

`tools/test_e25_vfio_card.py` runs the script's stub test
(`tools/operator/e25-1-vfio-run-stub-test.sh`) inside `make verify-all`: a
fake sysfs and a fake sudo, 20 cases, nothing under `/sys` or `/dev` touched.
A real process started by the test holds the stub card or render node, and
the preview and `--apply` must refuse and name it without a write.

### Limits

Recorded on the reference profile on 2026-10-18 with the Arc still on `xe`:
`map-bar --device 0000:03:00.0` prints
`FAIL: candidate: refused: 0000:03:00.0 driver is xe, expected vfio-pci` and
exits 1. The positive run waits for the operator's `--apply`. It maps a BAR
on the host through VFIO; it does not attach the device to a guest, so the
milestone's guest-passthrough criterion stays open.
