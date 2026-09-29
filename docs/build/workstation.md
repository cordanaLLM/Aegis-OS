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
evidence; re-measurement inside an Aegis image is a later acceptance. Boot
time and memory footprint are not measured here: under decision D71 they are
M22's, each naming the monitor that produced it. Venus GPU pricing (E21-3,
REQ-P10-02) is deferred to M12 and not claimed.

## What is tracked

| Path | Role |
| :--- | :--- |
| `crates/aegis-tellus/src/rapl.rs` | the counter reading, the rollover delta and `MeasuredWattage`, the second implementation of M05's `WattageSource` seam; reads nothing |
| `crates/aegis-tellus-rapl` | the readings document and the RAPL cases; `aegis-tellus-rapl run <document>` is the binary the gate starts |
| `crates/aegis-vesta-sandbox/src/vm.rs` | one Firecracker process: started from a controller row, reached over the hybrid vsock, killed, reaped and its sockets removed |
| `crates/aegis-vesta-sandbox/src/run.rs` | the host run and its five cases; `aegis-vesta-sandbox` starts it |
| `crates/aegis-vesta-sandbox/src/guest.rs` | the evaluation service `aegis-vesta-guest` runs as the microVM's init |
| `crates/aegis-vesta-sandbox/src/config.rs` | the per-microVM Firecracker configuration file |
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

1. The cached Firecracker binary, its LICENSE, the guest kernel and its
   configuration are hashed against `build/sandbox/firecracker.pin.json`; a
   mismatch is a FAIL. `--version` must print `Firecracker v1.17.0`, and the
   kernel configuration must carry `CONFIG_BLK_DEV_INITRD=y`,
   `CONFIG_KVM_GUEST=y`, `CONFIG_SERIAL_8250_CONSOLE=y`,
   `CONFIG_VIRTIO_MMIO=y` and `CONFIG_VIRTIO_VSOCKETS=y`.
2. `aegis-vesta-sandbox` is built, and `aegis-vesta-guest` is built with
   `--release --target x86_64-unknown-linux-gnu` and
   `RUSTFLAGS=-C target-feature=+crt-static` in that child's environment
   only. The gate refuses a guest whose ELF program headers name an
   interpreter, then writes a deterministic newc initramfs holding `/dev` and
   `/init`; nothing else is in the guest.
3. `aegis-vesta-sandbox run` runs five cases, one microVM at a time, in its
   own session, under a 600 s deadline.
4. The gate looks for anything the run left behind: a Firecracker or
   `aegis-vesta-sandbox` process whose command line names the run directory,
   a socket file under it, or a change in `/sys/class/net`
   (`sandbox/nothing-left-behind`, a gate-side case). A shell, `tail` or
   pager that merely names the run directory is not the run's and is left
   alone.

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
| `sandbox/sixty-fourth-accepted` | E21-2 boundary: microVMs 2 to 64 are admitted, booted and each answers, and all 64 Firecracker processes are running at once |
| `sandbox/sixty-fifth-refused` | E21-2 boundary: the 65th request is the controller's `MicroVmTableFull`, and no process is started for it |
| `sandbox/teardown` | every row terminated, every process killed and reaped within 5 s, every socket file removed |

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

## What is not claimed

- **Boot time and footprint.** No figure of either is recorded here; D71 makes
  them M22's, each naming its monitor, and the scaffold's literals stay
  `Unmeasured` in `crates/aegis-vesta`.
- **A meter.** The energy counter is AMD's model; see the accuracy statement.
  Only the package zone is read: `core` is recorded in the enumeration but
  its counter is not read, because the decision admits one command.
- **`AF_VSOCK` pricing.** REQ-P10-03 asks every `AF_VSOCK` message to be
  priced; this run proves the transport path and prices nothing.
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
