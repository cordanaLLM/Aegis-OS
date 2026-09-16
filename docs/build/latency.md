<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The latency fixture, and why the faster machine is the one that fails

Status: recorded observations from milestone M23, reference profile, 2026-09-13

Milestone M07 produced no timing figure on purpose. Every target in
`aegis-lictor` and `aegis-calliope` is a `Declared<T>` whose `Display` says
`5 (declared, unmeasured)`, and `tests/declared_literals.rs` stops compiling if
one becomes a bare integer. This milestone measures, and the first thing
measuring produced was an awkward result:

```text
guest (7.2.5-aegis-m26, PREEMPT_RT)      50000 cycles   max 273969 ns
host  (7.2.5-1-cachyos, PREEMPT_DYNAMIC) 50000 cycles   max 267461 ns
```

**The non-realtime machine measured the lower worst case.** That is not a fault
in the measurement, it is what the arrangement has to produce: the guest's
virtual CPU is a thread on the host, so the guest's lateness includes the
host's. Nor is it a one-off. Five runs recorded on 2026-09-13 gave these
figures; they are a sample of that day's runs, not all of them:

| Run | Guest mean | Guest max | Host mean | Host max | Lower max |
| ---: | ---: | ---: | ---: | ---: | :--- |
| 1 (recorded) | 9664 ns | 273969 ns | 1449 ns | 267461 ns | host |
| 2 | 8922 ns | 214129 ns | 1782 ns | 192784 ns | host |
| 3 | 8329 ns | 139079 ns | 1048 ns | 151594 ns | guest |
| 4 | 8761 ns | 203358 ns | 1745 ns | 184724 ns | host |
| 5 | 9484 ns | 290108 ns | 2701 ns | 257986 ns | host |

The host's worst case was the lower one four times out of five and the higher
one once, so ranking the two kernels by their worst case is not only wrong in
principle -- it does not even give a stable answer. The *means* never crossed:
the guest's stayed between 8329 and 9664 nanoseconds and the host's between 1048
and 2701, which is the virtualisation cost and not a property of either
kernel's preemption model. This page records what was measured, how, and the rule
that stops the figure deciding anything on its own.

**Nothing here is a release artifact and nothing here qualifies hardware.** No
package was installed, no module was loaded, no bootloader entry was written and
no firmware setting was touched. A pass is development evidence on one
workstation.

## What is tracked, and what is not

| Path | Role |
| :--- | :--- |
| `crates/aegis-calliope/src/measured.rs` | `Measured<T>`, `KernelIdentity`, and the verdict rule |
| `crates/aegis-lictor/src/determinism.rs` | the same rule applied to the P07 tier edges |
| `tools/guest/aegis-preempt-probe.sh` | the `CONFIG_PREEMPT_RT` probe, run on both machines |
| `tools/guest/aegis-latency-init.sh` | PID 1 of the latency guest |
| `tools/verify_latency_fixture.py` | the gate, run as `make verify-latency` |
| `tools/test_latency_fixture.py` | the half of it that runs inside `make verify-all` |

Not tracked: the guest tree, the initramfs, the console log and the retained
JSON. They live under `AEGIS_LATENCY_BUILD_DIR`, default
`${XDG_CACHE_HOME:-$HOME/.cache}/aegis-latency`. The kernel image is M26's, from
`AEGIS_KERNEL_BUILD_DIR`; nothing here builds a kernel.

## Where the realtime kernel came from (D57 and D70)

D57 asked where the `PREEMPT_RT` kernel for P07 and P08 work comes from and
recommended a distribution `linux-rt` package booted as a guest kernel, with no
host boot entry. D70 then put kernel construction in this repository while
Nucleus is a scaffold, and M26 built `linux-7.2.5` with `CONFIG_PREEMPT_RT=y`
from a pinned, signature-verified source.

This milestone consumes that image. **No distribution realtime package is
downloaded, installed or booted**, so D57's constraint -- the reference host is
not modified -- is satisfied by construction rather than by a careful
download-only procedure. The kernel is a file in a build directory. The gate
records the three things that can be checked for it without root on this
profile:

```text
$ pacman -Qo .../aegis-kernel/out/positive/arch/x86/boot/bzImage
error: No package owns .../bzImage

$ pacman -Qq linux-rt
error: package 'linux-rt' was not found

/lib/modules holds ['6.18.50-1-cachyos-lts', '7.2.4-arch1-2', '7.2.5-1-cachyos']
7.2.5-aegis-m26 is not among them
```

What the gate does **not** check, said rather than implied: `/boot` is mode
`0700 root` on this profile, so it cannot be enumerated by the developer
account and no claim is made about bootloader entries. What stands in for that
is narrower and harder to evade than an enumeration would be. `ALLOWED_PROGRAMS`
in `tools/test_latency_fixture.py` is the complete set of programs the gate may
start -- `qemu-system-x86_64`, `cyclictest`, `cpio`, `ldd`, `bash`, `mount`,
`cat`, `grep`, `gzip` and `pacman` -- resolved from the gate's own syntax tree
and asserted as an **equality**, so the list can carry neither an unlisted
program nor a dead entry. `pacman` appears only as `-Qo` and `-Qq`, which are
queries.

The first form of that sweep was wrong, and wrong in the direction that hides
call sites rather than reporting them. It resolved `argv` bindings across the
whole module, and two functions each bind a local named `argv`, so the
toolchain version calls resolved to another function's literal vector and looked
decided -- silently dropping four of the ten programs. Scoped to one function it
reports what it cannot name instead: two call sites, `run()` (the gate's own
deadline wrapper, which starts whatever its caller hands it, so every real call
site is one of the decidable ones that go through it) and `read_version()`
(which runs one `TOOLCHAIN` row, and the table names those). The test asserts
both by name and resolves the second from the table.

D70 applies unchanged: the guest kernel is an interim source for this fixture.
The kernel the product ships is built by Nucleus against the M18 schema once
Nucleus is real.

## The same probe, on both machines

The criterion asks for the guest's configuration to be read back from inside the
virtual machine, and for the same probe on the host to report the option not
set. "The same probe" is one file,
`tools/guest/aegis-preempt-probe.sh`, copied into the guest's initramfs and
executed there, and executed on the host by the gate. It decompresses the
running kernel's own `/proc/config.gz` and prints the matching line verbatim
rather than a verdict, so the two readings compare as text. The interpreter is
the same program too: the guest's `/bin/sh` is a copy of this host's `bash`.

```text
guest, from inside the virtual machine
  AEGIS-M23-UNAME-R 7.2.5-aegis-m26
  AEGIS-M23-UNAME-V #1 SMP PREEMPT_RT Sun Sep 13 18:55:36 CEST 2026
  CONFIG_PREEMPT_RT=y

host, same script, same interpreter
  uname -r: 7.2.5-1-cachyos
  # CONFIG_PREEMPT_RT is not set
```

The guest's report carries a per-run nonce the gate generates and passes as
`aegis.nonce=<value>` on the kernel command line, and the gate deletes the
report file before the emulator starts. Both halves are needed and both were
falsified: with the guest patched to print a fixed nonce the gate reported

```text
FAIL latency/guest-preempt-rt
     the report carries nonce 'hand-typed-0001', not this run's
     'aegis-1539807-34e0b85bea9f557fa3fdf0c09731e848'; it was not written by
     this boot and proves nothing about it
```

## The measurement, precisely enough to dispute

The tool is `cyclictest` 2.10 from `rt-tests 2.10-1.1`, the standard rt-tests
latency tool, admitted in `docs/roadmap/toolchain-admission.md`. Nothing in this
repository writes its own timing loop.

One argument vector runs on both machines. The guest runs it from a script the
gate writes; the gate runs it directly on the host; and both runs record their
own arguments in their JSON output, which the gate compares after dropping the
program path and the output path. That is evidence that the two runs matched,
not an assertion that they did.

```text
--default-system --mlockall --priority=95 --interval=200 --distance=0
--threads=1 --affinity=1 --loops=50000 --nsecs --quiet
```

Three of those are decisions rather than defaults:

- `--default-system` stops cyclictest writing a power-management latency target
  into `/dev/cpu_dma_latency`. Without it the tool tunes the machine it runs on,
  which on the host would be a modification (D57). With it the tool prints
  `WARN: not setting cpu_dma_latency from cyclictest` and changes nothing. The
  cost is stated rather than hidden: idle states are **not** suppressed, so
  these figures include C-state exit latency and are worse than a tuned run
  would give.
- `--priority=95` is REQ-P08-02's `RLIMIT_RTPRIO`, the priority the P08 report
  calls non-negotiable for the audio threads, rather than cyclictest's customary
  99. The reference account's `ulimit -r` reads 99, so 95 is reachable without
  privilege.
- `--nsecs` puts min, average and maximum in nanoseconds, the unit the P07 tier
  edges use. The gate refuses a payload whose `resolution_in_ns` is not 1, so a
  microsecond payload cannot be read as a nanosecond one.

**What cyclictest measures**: one `SCHED_FIFO` thread sleeps on a periodic
`clock_nanosleep` at a fixed interval and records, each cycle, the difference
between the wakeup it was programmed for and the wakeup it got. The maximum over
the run is the worst-case wakeup latency. That sentence is carried in the type
itself, as `MeasurementTool::measures`, so a figure cannot be read as something
else at its use site.

**What it does not measure, and what this fixture therefore does not claim:**

- it is not a burst duration. `Tier::classify` sorts tasks by how long they ran;
  this measures how late a thread was woken. The two are related by a floor, not
  by identity: a tier whose edge is 100 microseconds is a commitment at that
  time scale, and a kernel whose own worst-case wakeup latency exceeds it cannot
  keep that commitment however the classifier sorts, because the task is not
  running yet. Reading a measured worst case as *the smallest tier edge the
  kernel could honour* is the interpretation this milestone makes, and it is
  stated here so it can be disputed;
- it is not the P08 round-trip latency, which also includes the device and the
  graph. No `PipeWire` graph was started and no audio device was opened;
- it is not a measurement of `scx_cake`. No eBPF program was compiled, loaded or
  attached here; M19 is where an object goes through a verifier;
- **it is not a bound.** Ten seconds of samples on a machine that was also
  carrying a development session is a reading of that run, not a worst case over
  all time. The five recorded runs in the table above span 139079 to 290108
  nanoseconds
  in the guest, roughly a factor of two. **The verdicts did not change across
  any of them**; the integers did. A figure that has to hold must come from a
  long run on a quiet, tuned machine, and this is neither;
- **the guest figure is a composite.** The guest's virtual CPU is scheduled by a
  host that is not realtime, so what the guest measures is its own kernel plus
  the host's scheduling of it. It is a lower bound on how good a `PREEMPT_RT`
  kernel can be here, not a measurement of what `PREEMPT_RT` buys. The means say
  so plainly: 9664 ns in the guest against 1449 ns on the host, roughly seven
  times, and the means were the one thing that was stable: 8329 to 9664 ns in
  the guest against 1048 to 2701 ns on the host across all five runs. That gap
  is the virtualisation cost showing through, and it is why the maxima of the
  two machines land in the same range despite one of them being a realtime
  kernel.

## The figures, and what each one satisfies

The recorded run, with every figure naming the kernel that produced it:

| Kernel | Preemption | Cycles | Min | Mean | Max |
| :--- | :--- | ---: | ---: | ---: | ---: |
| 7.2.5-aegis-m26 (guest) | PREEMPT_RT | 50000 | 1510 ns | 9664 ns | 273969 ns |
| 7.2.5-1-cachyos (host) | not PREEMPT_RT | 50000 | 440 ns | 1449 ns | 267461 ns |

Against the three P07 tier edges and the P08 target:

| Threshold | Value | Guest verdict | Host verdict |
| :--- | ---: | :--- | :--- |
| `BURST_CRITICAL_NS` | 100000 ns | worst-case-exceeds | kernel-not-realtime |
| `BURST_INTERACTIVE_NS` | 2000000 ns | satisfied | kernel-not-realtime |
| `BURST_FRAME_NS` | 8000000 ns | satisfied | kernel-not-realtime |
| `TARGET_RTL_LATENCY_NS` | 5000000 ns | satisfied | kernel-not-realtime |

Two readings of that table matter more than the numbers.

**The critical tier was not attained and is reported as not attained.** The
guest's worst case is 2.7 times the 100 microsecond edge. REQ-P07-01's
deterministic tier-0 response is therefore still not demonstrated, and the
milestone says so rather than choosing a friendlier threshold. On this
arrangement it could hardly be otherwise: the composite includes an untuned,
non-realtime host.

**The host's column is not a column of numbers.** Every host verdict is
`kernel-not-realtime`, decided before its figure is looked at. The host's
figures are below the guest's at three of the four edges by arithmetic, and that
buys nothing.

## How a measured figure is kept apart from a declared one

`Declared<T>` (M07) and `Measured<T>` (M23) are separate types in
`aegis-calliope` with no conversion between them in either direction.
`Measured::new` demands a `KernelIdentity` and a `MeasurementTool`; a
`Declared<T>` has neither to give. Two checks hold the separation:

- `crates/aegis-calliope/tests/declared_literals.rs` binds each M07 constant to
  a `Declared<u32>` pattern, so rewriting one as a bare integer stops the file
  compiling, and sweeps the crate's own sources for six spellings of a
  conversion between the wrappers. A planted
  `impl From<Declared<u64>> for Measured<u64>` was reported as
  `measured.rs:513: impl From<Declared` and failed the suite;
- `crates/aegis-calliope/tests/measured_figures.rs` does the mirror image with
  `require_measured`, which takes a `Measured<u64>`.

Their renderings differ in the same direction:

```text
5 (declared, unmeasured)
273969 (measured by cyclictest on 7.2.5-aegis-m26, PREEMPT_RT)
```

## The rule, and why the kernel is checked first

`DeterminismVerdict::of` takes a `KernelIdentity` and a `TierAttainment` and
refuses a non-realtime kernel **before** looking at the attainment:

```rust
pub const fn of(kernel: KernelIdentity, attainment: TierAttainment) -> Self {
    if !kernel.is_realtime() {
        return Self::KernelNotRealtime;
    }
    match attainment {
        TierAttainment::Attained => Self::Satisfied,
        TierAttainment::AtEdge => Self::WorstCaseAtEdge,
        TierAttainment::Exceeded => Self::WorstCaseExceeds,
    }
}
```

This is the shape M05 gave the wattage seam in `crates/aegis-tellus/src/power.rs`,
where a figure carries how it was obtained and the rule deciding the label is
written down and tested: there, a zone the profile exposes no counter for is
`Modelled` whatever figure stands in; here, a kernel without
`CONFIG_PREEMPT_RT` is `KernelNotRealtime` whatever figure stands in. The
falsifier is a host reading of **zero nanoseconds**, the best conceivable
figure, which `a_perfect_reading_on_a_non_realtime_kernel_is_still_refused`
pins as refused. If the rule ever ordered the two kernels by figure, that is the
case that would flip.

`TierAttainment::evaluate` is the layer below and carries no provenance at all.
That is deliberate: the boundary cases evaluate the comparison rule at three
points, and building a `Measured` for them would mean minting three figures
nobody observed.

## The edge, and why it has a name of its own

The comparison is strict, because the classifier it reports against is strict.
`scx_cake.bpf.c` writes `if (ewma < BURST_CRITICAL_NS)`, so a burst of exactly
100000 nanoseconds is not critical; `Tier::classify` reproduces that rather than
rounding it. A fixture that called a worst case exactly on the edge "attained"
would claim a tier the classifier would not agree with, so the edge is its own
outcome:

```text
BURST_CRITICAL_NS = 100000 ns; these are rule evaluations, not measurements
worst case  99999 ns -> attained, satisfied
worst case 100000 ns -> at-edge,  worst-case-at-edge
worst case 100001 ns -> exceeded, worst-case-exceeds
```

Three values, three outcomes, three names. The threshold is pinned to `100_000`
with `assert_eq!` in both crates and in `tools/test_latency_fixture.py` before
anything is evaluated against it, and
`the_classifier_places_the_edge_the_way_the_fixture_reports_it` checks that
`Tier::classify` places the same three values the same way, so the fixture and
the classifier cannot disagree about what "exactly 100 microseconds" means.

## One reading the tool gets wrong, and why it is not used

`cyclictest`'s JSON carries `"realtime": 0` on **both** machines, including the
guest that reports `CONFIG_PREEMPT_RT=y` from its own configuration. It is not a
contradiction and it is not evidence of anything: `cyclictest` reads
`/sys/kernel/realtime`, a file the out-of-tree realtime patch set used to add,
and mainline does not. Read out of the pinned source, the `kernel_attrs[]` array
in `kernel/ksysfs.c` is the complete attribute list of `/sys/kernel` and holds
nine entries, four of them unconditional and five behind a `CONFIG_` guard --

```text
fscaps, uevent_seqnum, cpu_byteorder, address_bits, uevent_helper, profiling,
vmcoreinfo, rcu_expedited, rcu_normal
```

-- and none of them is `realtime`; a whole-tree search of `linux-7.2.5` for
`KERNEL_ATTR_RO(realtime)`, `KERNEL_ATTR_RW(realtime)` and `__ATTR_RO(realtime)`
returns nothing. So the field reports the absence of a file, not the absence of
`PREEMPT_RT`, and this fixture ignores it. The reading that is used is the
kernel's own compiled-in configuration, through the probe both machines run.

## Where a host kernel change would show up

`REFERENCE_HOST` in `crates/aegis-calliope/src/measured.rs` records
`7.2.5-1-cachyos`. The gate compares that with `os.uname().release` on every run
and fails if they differ, saying the recorded host reading describes a kernel
that is no longer running. That is deliberate: a recorded negative is about one
kernel, and a host update should make it stale loudly rather than leave a figure
attributed to a kernel nobody is running. The cost is that a routine host kernel
update fails `make verify-latency` until the constant and this page are
re-recorded.

## What a pass here does not close

The image, kernel-artifact, boot, hardware and release gates remain blocked. No
hardware is qualified: the reference profile is one machine and
`planning/hardware-profile.json` records its evidence class as development
evidence only. `planning/hardware-profile.json` still records
`realtime_kernel: present false`, which remains the correct reading of the
**host** -- this milestone did not change it, and the negative half of the
fixture depends on it staying true.
