<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The eBPF fixtures, and what the host kernel's verifier said about them

Milestone M19 is the first work in this repository that reaches a real kernel.
Four objects under `bpf/` are compiled with `clang -target bpf` and handed to the
**running** kernel's BPF verifier; one of them is attached to an LSM hook and one
is attached as a sched_ext scheduler and detached again. `make verify-bpf` runs
the whole set and keeps the verifier's own log for every load.

Read the scope limit first, because it is the point: **this is a non-qualifying
local fixture.** The kernel that accepted these objects is the maintainer's
workstation kernel, which this repository neither builds nor configures. A pass
here does not close M10's Nucleus-kernel verification, it is not evidence for any
image, boot, hardware, accessibility or release gate, and it measures no
scheduling, power or policy behaviour whatsoever. What it establishes is narrow
and real: these objects are accepted by a current verifier, the bounds they carry
are load-bearing, and the attach and detach paths work.

## What is tracked, and what is not

Tracked, under `bpf/`:

| File | What it is |
| :--- | :--- |
| `aegis_bpf_abi.h` | the record layout and the declared bounds both sides share |
| `action_gate.bpf.c` | P06, a BPF LSM program on `lsm/bprm_check_security` |
| `kepler_power.bpf.c` | P13, a tracepoint program on `sched/sched_switch` |
| `scx_cake.bpf.c` | P07, a sched_ext `struct_ops`. Loaded, never attached |
| `scx_cake_stub.bpf.c` | P07 boundary, a `struct_ops` whose nine handlers are empty |
| `loader/aegis_bpf_probe.c` | the loader that retains the verifier log and holds the deadlines |

Not tracked, and generated on every run into
`${XDG_CACHE_HOME:-$HOME/.cache}/aegis-bpf`: `vmlinux.h`, produced by
`bpftool btf dump file /sys/kernel/btf/vmlinux format c`; the compiled objects;
the three negative variants; and the verifier logs. Nothing is written into the
repository and nothing is installed.

The negative variants are **not** tracked as separate broken sources. Each is
produced by deleting exactly one region of the positive source, delimited by
`// AEGIS-MUTATE-BEGIN <name>` and `// AEGIS-MUTATE-END <name>`. A region that is
absent, or present more than once, fails the gate rather than producing an object
identical to the positive one, so the negative case cannot quietly become
vacuous. That is also why the negatives prove something about *the deleted
bound*: the two objects differ in that region and nothing else.

## The host, as observed

Every line below was read back from the machine, with the command beside it. The
gate re-reads all of them before it compiles anything and refuses to run if any
is missing.

| Fact | Value | Command |
| :--- | :--- | :--- |
| Kernel | `7.2.4-1-cachyos #1 SMP PREEMPT_DYNAMIC` | `uname -a` |
| BPF syscall | `CONFIG_BPF_SYSCALL=y` | `zcat /proc/config.gz` |
| BPF LSM | `CONFIG_BPF_LSM=y` | `zcat /proc/config.gz` |
| BPF LSM active | `capability,landlock,lockdown,yama,bpf` | `cat /sys/kernel/security/lsm` |
| sched_ext | `CONFIG_SCHED_CLASS_EXT=y`, state `enabled` | `zcat /proc/config.gz`, `cat /sys/kernel/sched_ext/state` |
| Kernel BTF | `CONFIG_DEBUG_INFO_BTF=y`, `/sys/kernel/btf/vmlinux` present | `zcat /proc/config.gz`, `ls -l /sys/kernel/btf/vmlinux` |
| Unprivileged BPF | `kernel.unprivileged_bpf_disabled = 2` | `sysctl kernel.unprivileged_bpf_disabled` |

`CONFIG_BPF_LSM=y` is not the same claim as "BPF LSM is active": the symbol says
the kernel can carry BPF LSM programs, and the `lsm` file says `bpf` is in the
list the kernel actually initialised. Both are checked, because a kernel built
with the symbol but without `bpf` in `lsm=` would accept the program and refuse
the attach.

## Capabilities: CAP_BPF alone was not enough

The exit criterion says the objects load "with CAP_BPF". On this kernel they do
not. Every load runs under

```text
sudo setpriv --reuid=<caller> --regid=<caller> --clear-groups \
  --bounding-set=-all,+bpf,+perfmon --inh-caps=-all,+bpf,+perfmon \
  --ambient-caps=-all,+bpf,+perfmon -- <probe>
```

which is an unprivileged uid whose entire bounding set is those two capabilities;
`capsh --print` inside it reads `Current: cap_bpf,cap_perfmon=eip`. With
`+bpf` alone, all three program types were refused before the verifier ever ran:

```text
libbpf: prog 'action_gate_exec': BPF program load failed: -EPERM
libbpf: prog 'aegis_kepler_sched_switch': BPF program load failed: -EPERM
libbpf: prog 'aegis_cake_select_cpu': BPF program load failed: -EPERM
```

Adding `CAP_PERFMON` made all three load. So the recorded capability set is
`CAP_BPF + CAP_PERFMON`, because that is what was used; the register's
"with CAP_BPF" was an assumption and is corrected in `planning/roadmap.json`.
The loads never run as root and never inherit the caller's privileges.

## The verifier log, which is the evidence

The loader gives **every program its own log buffer at log level 1 before the
object is loaded**, so a log comes back whether the load succeeds or fails, and
a multi-program object does not overwrite its own evidence. `bpftool prog load`
would have given one shared buffer, which is the main reason there is a loader
here at all.

One retained log is truncated, and it says so. The rejected unbounded loop
traces 90,113 instructions, which outgrows the probe's 4 MiB per-program
buffer; libbpf then reports `ENOSPC`, which in this context means the verifier
log outgrew the buffer and not that a disk filled. The kernel's verifier log is
a rotating buffer, so what survives is the **tail** -- which is where the
verdict and the instruction count are, and the gate requires the verdict to be
there. The retained file marks the cut with `(TRUNCATED: this program's log
filled the buffer; the kernel retained the tail)` and the header explains the
errno, so a truncated log is never mistaken for a complete one. Every other log
in the set is complete.

Register-state annotations in the excerpts below are trimmed to fit the
80-column rule and the cut is marked with a trailing `…`; the untrimmed logs are
the files the gate names under `${XDG_CACHE_HOME:-$HOME/.cache}/aegis-bpf/logs`.

That design paid for itself on the first run of `scx_cake`, where the object
failed to load and the log said exactly why:

```text
=== program aegis_cake_init (section struct_ops/aegis_cake_init) ===
0: R1=ctx() R10=fp0
; created = scx_bpf_create_dsq(AEGIS_CAKE_DSQ_BASE + index, -1); @ scx_cake.bpf.c:189
0: (b7) r1 = 1095059273               ; R1=0x41454749
1: (b4) w2 = -1                       ; R2=0xffffffff
2: (85) call scx_bpf_create_dsq#131638
program must be sleepable to call sleepable kfunc scx_bpf_create_dsq
```

`scx_bpf_create_dsq` is a sleepable kfunc, so `ops.init` has to be declared
`SEC("struct_ops.s/...")`. The `.s` suffix in `bpf/scx_cake.bpf.c` is there
because of that rejection, not because a header said so.

### Positive loads

Each retained log opens with `load errno 0 (load reported success)` and carries
one section per program.

| Object | Programs verified | Instructions processed |
| :--- | ---: | :--- |
| `action_gate` | 1 | 50 |
| `kepler_power` | 1 | 54 |
| `scx_cake` | 7 | 10, 19, 86, 27, 40, 22, 5 |
| `scx_cake_stub` | 9 | 2, 1, 1, 1, 1, 1, 2, 2, 1 |

The one-instruction programs are the empty stub handlers; a handler that does
nothing compiles to a single `exit`. That is what "stubbed" means here, and it is
visible in the log rather than asserted in prose.

### Negative loads, proven by the log

Each negative deletes one region and must produce a **named verifier
rejection**. A non-zero exit code is not accepted on its own, and the gate
additionally requires that the corresponding positive log does *not* contain
the same string -- otherwise the diagnostic would say nothing about the deleted
bound.

A log is evidence only when the load that wrote it stamped it. Each log is
deleted immediately before the load that produces it, the gate passes that load
a nonce, and the loader writes it into the header as `# run-nonce <value>`; a
file without this run's nonce is refused rather than read. The loader also exits
`6` when it cannot *write* a log, which is a different outcome from the `1` it
exits when the verifier *rejects* an object. Without those three, a probe that
failed for a non-verifier reason -- an unwritable log file -- left whatever was
already on disk in place, and a non-zero exit over a stale file containing the
recorded string read as a verifier rejection.

`action_gate`, with the `bpf_ringbuf_reserve` NULL check deleted (11 lines):

```text
7: (85) call bpf_ringbuf_reserve#131   ; R0=ringbuf_mem_or_null(id=2,sz=48) …
8: (bf) r7 = r0                        ; R7=ringbuf_mem_or_null(id=2,sz=48) …
; event->pid_tgid = bpf_get_current_pid_tgid(); @ action_gate_negative.bpf.c:44
9: (85) call bpf_get_current_pid_tgid#14      ; R0=scalar() refs=2
10: (7b) *(u64 *)(r7 +0) = r0
R7 invalid mem access 'ringbuf_mem_or_null'
```

`kepler_power`, with the map-value index mask deleted (7 lines):

```text
; slot = (__u32)ctx->next_pid; @ kepler_power_negative.bpf.c:62
38: (61) r1 = *(u32 *)(r7 +56)  ; R1=scalar(smin=0,smax=umax=0xffffffff) …
; sample->runtime_ns[slot] += delta_ns; @ kepler_power_negative.bpf.c:63
39: (67) r1 <<= 3
41: (0f) r2 += r1
42: (79) r1 = *(u64 *)(r2 +16)
R2 unbounded memory access, make sure to bounds check any such access
```

`scx_cake`, with the dispatch loop's clamp deleted (10 lines):

```text
26: (bf) r1 = r7                      ; R1=0x41456747 R7=0x41456747
27: (07) r1 += -1095059272            ; R1=8191
28: (07) r7 += 1                      ; R7=0x41456748
29: (ad) if r1 < r6 goto pc-10
The sequence of 8193 jumps is too complex.
processed 90113 insns (limit 1000000) max_states_per_insn 4 total_states 1233 …
```

The third one is worth naming precisely, because it is easy to overclaim: the
verifier did not print "infinite loop". With the clamp removed the loop bound
comes from a map value, the verifier explores the whole range, and it refuses the
program as too complex after 90,113 instructions. That is a rejection caused by
removing the bound, and it is the rejection that was observed; it is not a
general statement about how the verifier treats unbounded loops.

The three recorded diagnostics are **an enumeration of three mutations**, not a
proof that the verifier rejects unchecked pointers, unbounded indices or
unbounded loops as categories.

## E19-1: an exec event reaching the consumer

The `action_gate` program is attached to `lsm/bprm_check_security`, a marker
binary is executed, and the event must arrive in the ring buffer:

```text
attached=lsm ringbuf=action_ringbuf
event filename=/…/scratch/aegis_exec_prob comm=aegis_bpf_probe kind=1
  verdict=0 argc=1 cgroup_id=14943
detached=lsm marker_seen=1
```

`comm` is `aegis_bpf_probe` and not the marker's name, and that is not a defect:
at `bprm_check_security` the task's `comm` is still the name of the process that
called `execve`, because the new name is installed later in `begin_new_exec`. A
consumer keyed on `comm` matches the caller, not the program being executed. The
fixture therefore records `bprm->filename`, and the consumer matches on that. The
first version of this fixture did key on `comm`, saw nothing, and timed out.

**The fixture cannot deny an exec.** `bprm_check_security` runs on every exec on
the machine, so the program returns only `AEGIS_ACTION_ALLOW`, which is zero, or
passes through a denial an earlier LSM already decided. `tools/test_bpf_objects.py`
asserts that the set of return expressions in the source is exactly
`{ret, AEGIS_ACTION_ALLOW}` and that `AEGIS_ACTION_ALLOW` is `0`. The
pre-execution approval gate REQ-P06-05 describes is **not** implemented: this
object produces the event a gate would decide on, and nothing else.

## E19-2: attaching and detaching a scheduler (D67)

Only one sched_ext scheduler can hold `/sys/kernel/sched_ext/root/ops`, so the
boundary case takes over the machine's CPU scheduling for the duration. It is
opt-in behind `--allow-scheduler-takeover`; without the flag the gate prints why
it did not run. Confirmed directly: with the machine's scheduler attached, the
attach is refused and nothing changes.

```text
sched_ext_ops_before=rusty_1.1.3_x86_64_unknown_linux_gnu
struct_ops attach failed: Device or resource busy
sched_ext_ops_after_failed_attach=rusty_1.1.3_x86_64_unknown_linux_gnu
```

Three properties make the stub safe to attach, and all three are declared in
`bpf/scx_cake_stub.bpf.c` rather than assumed:

1. `SCX_OPS_SWITCH_PARTIAL` (value 8 in the running kernel's `enum
   scx_ops_flags`). With it set, sched_ext schedules only tasks whose policy is
   `SCHED_EXT`; `SCHED_NORMAL`, `SCHED_BATCH` and `SCHED_IDLE` stay on the
   fair-class scheduler. Nothing on the reference profile sets `SCHED_EXT`, so no
   task ever reaches the empty handlers. Without this flag an empty `enqueue`
   would stall every runnable task: the stub is safe **because of the flag**, not
   because doing nothing is harmless.
2. `timeout_ms = 5000`. sched_ext runs a watchdog and ejects a scheduler that
   leaves a runnable task un-run past that bound, reverting every task to the
   fair-class scheduler. This is why the step is recoverable without operator
   action, and it is why the bound is declared rather than defaulted.
3. The kernel documents `ops.name` as the only mandatory field, so the nine
   handlers are present and empty by choice. `tools/test_bpf_objects.py` pins the
   set to those nine names and the gate reads the same nine back out of the built
   object.

The sequence the gate runs, with the readings it took:

| Step | `/sys/kernel/sched_ext/root/ops` | `state` | `switch_all` | `nr_rejected` | `enable_seq` |
| :--- | :--- | :--- | ---: | ---: | ---: |
| Before | `rusty_1.1.3_x86_64_unknown_linux_gnu` | `enabled` | 1 | 0 | 19 |
| After `org.scx.Loader.StopScheduler` | absent | `disabled` | 0 | 0 | 19 |
| While the link is held | `aegis_cake_stub` | `enabled` | 0 | 0 | 20 |
| Still held, after the hold window | `aegis_cake_stub` | `enabled` | 0 | 0 | 20 |
| After `bpf_link__destroy` (rc 0) | absent | `disabled` | 0 | 0 | 20 |
| After `org.scx.Loader.RestoreDefault` | `rusty_1.1.3_x86_64_unknown_linux_gnu` | `enabled` | 1 | 0 | 21 |

Every counter column is a reading from one run, not a reconstruction. The three
middle rows are read by the probe itself, from inside the process that holds the
link, because a reading taken after the link is released is not a reading taken
during it; the first and last are read by the gate, which brackets the whole
case.

The attach window is one second, bounded by `--hold-ms`, with a `SIGALRM`
deadline around the whole probe. The restore is verified **against the recorded
value**, not against "something is attached": a different scheduler holding
`root/ops` afterwards fails the case. That check is not theoretical, see below.

The two `org.scx.Loader` calls run through `sudo -n`. That is not decoration:
the methods are polkit-gated, and an unprivileged `busctl` with no interactive
agent to answer the prompt gets `Call failed: Not allowed!` after the polkit
timeout. Recorded without `sudo`, the sequence above does not reproduce itself.

### The counters, and which pair of readings is a difference

`/sys/kernel/sched_ext/root/` holds exactly two files, `events` and `ops`. An
earlier revision of this page concluded from that listing that `switch_all` and
`nr_rejected` do not exist on this kernel. **They do.** They are one directory
up, as top-level `sched_ext` attributes; only `root/` had been listed. All are
world-readable:

| File | Across the takeover | What the kernel does with it |
| :--- | :--- | :--- |
| `/sys/kernel/sched_ext/switch_all` | `1` → `0` → `1` | not a counter: the live `scx_switching_all` of whatever holds `root/ops` |
| `/sys/kernel/sched_ext/nr_rejected` | `0` → `0` → `0` | set to `0` by every scheduler enable, so it belongs to the instance |
| `/sys/kernel/sched_ext/enable_seq` | `19` → `20` → `21` | incremented on every enable, never reset: monotonic since boot |
| `/sys/kernel/sched_ext/hotplug_seq` | `31`, unchanged | incremented on CPU hotplug, never reset |

"The counters reset per scheduler instance and are therefore not subtractable"
is not one claim but three, and they do not all hold:

- The `SCX_EV_*` lines inside `root/events` hang off the running scheduler's own
  kobject and go away with it. `SCX_EV_DISPATCH_KEEP_LAST` read 104,813,235 on
  the long-running instance, 201,783 on the instance that replaced it and
  430,596 twenty seconds later. A before and an after set is the right record;
  **subtracting them is not**.
- `nr_rejected` is per instance as well, for a different reason: the enable path
  itself zeroes it, `atomic_long_set(&scx_nr_rejected, 0)` in the upstream
  `kernel/sched/ext/ext.c`, and the same line in `kernel/sched/ext.c` at v6.12.
  It counts tasks whose `SCHED_EXT` policy was refused because `ops.init_task`
  set `disallow`, for the scheduler that is running. A pair spanning an attach
  is **not** a difference.
- `enable_seq` **is** monotonic since boot, and its pair **is** a difference. The
  kernel only ever increments it, `atomic_long_inc(&scx_enable_seq)`; the
  upstream documentation calls it a monotonically incrementing counter of BPF
  schedulers loaded since boot; and it was observed advancing `19 → 20 → 21`
  across the stub's enable and rusty's. The gate prints that delta and names it.

Those kernel-source lines are read from the upstream tree and not from the
source of the running kernel, which this repository neither builds nor
configures -- the same reason the whole gate is a non-qualifying local fixture.
The behaviour they describe was confirmed here only where it is observable:
`enable_seq` advanced by exactly one per enable across four enables.

### `switch_all` is the measurement, and it read `0`

`SCX_OPS_SWITCH_PARTIAL` in `bpf/scx_cake_stub.bpf.c` is what the stub *asks
for*. `switch_all` is what the kernel *answers*, and they are not the same
evidence: the first is a flag read out of the fixture's own source, the second
is `scx_switching_all` read out of the kernel while the fixture held the slot.
It read `1` under rusty, which switches every task, and `0` for the whole hold
window, which is what a partial scheduler reads.

The gate requires that `0`. A stub that lost its `SCX_OPS_SWITCH_PARTIAL` would
still attach, still detach cleanly and still satisfy every other check in the
case; `switch_all` reading `1` is the only thing that would catch it.

### A hazard this uncovered on the reference profile

The first detach freed `root/ops`, and one second later a *different* scheduler
held it: `/usr/local/bin/scx_ghostbrew`, from an enabled unit
`scx-ghostbrew.service` with `Restart=on-failure` and `RestartSec=5`. It had been
failing every five seconds for the whole session, because `scx_rusty` held the
slot, and it won the first moment the slot was free. `scx_loader` then could not
put `scx_rusty` back: `another sched_ext scheduler is already running`.

So the reference profile has **two scheduler supervisors enabled at once**, and
which one holds `root/ops` is decided by whoever retries first. That is a
pre-existing condition of the host, not something this milestone introduced, but
this milestone is what made it visible. It is also why the gate compares the
restored scheduler with the recorded one by name: accepting "something is
attached" would have recorded a restore that did not happen. An operator running
the boundary case on a host with a competing supervisor should stop it for the
duration.

The register's `ghostbrew` reading is thus explained rather than merely wrong: it
was taken during a window in which ghostbrew held the slot. It is corrected to
the value observed at M19 time, `rusty_1.1.3_x86_64_unknown_linux_gnu`, with the
race recorded here.

## E19-3: the TDP literal is not a measurement

`kepler_power` accumulates scheduler runtime per cgroup and multiplies it by
`AEGIS_KEPLER_TDP_MILLIWATT_UNMEASURED`, 15,000 mW. That product is not energy
and the field it lands in is called `pseudo_energy_uj_unmeasured`. No RAPL MSR,
no `/sys/class/powercap` file and no ACPI meter is read anywhere in the object;
the number is runtime in different units. The value is carried over from the
imported P13 proposal unchanged so that M21 replaces it with a counter read and
the diff shows exactly which figure changed. `tools/test_bpf_objects.py` pins the
literal to `15000` written as a literal in the test, not to the macro that holds
it.

## Provenance of `scx_cake.bpf.c` (D66)

`extra/scx-scheds 1.1.3-2` is installed on the reference profile and already
ships `/usr/bin/scx_cake`. The tracked `bpf/scx_cake.bpf.c` is **an Aegis
original that shares a name with that binary and derives from neither it nor its
upstream**: no line of `https://github.com/sched-ext/scx` is copied, no upstream
commit is vendored, and the object is not a fork. What it does descend from is
the imported P07 proposal sketch under `.workingdir/prepared/scaffold/` (private,
gitignored, immutable proposal data), which is rewritten here rather than
imported.

The rewrite was not cosmetic. The proposal's `enqueue` classified a task into a
tier and then dispatched it nowhere -- the dispatch call was commented out.
Attached, that scheduler stalls every runnable task until the watchdog ejects it.
The tracked object dispatches through `scx_bpf_dsq_insert` and consumes in
`dispatch`; it is still never attached by this milestone.

The kfunc prototypes in the object were read back from the running kernel's BTF
(`bpftool btf dump file /sys/kernel/btf/vmlinux format raw`) rather than copied
from an upstream header, which is why a disagreement with the kernel shows up as
a load failure with a verifier log rather than as a silent mismatch.

## How to run it, and what the outcomes mean

```bash
make verify-bpf                      # everything except the takeover
python3 tools/verify_bpf_objects.py --allow-scheduler-takeover   # and that too
```

`make verify-bpf` is **not** part of `make verify-all`, and the `Makefile`
records why: the gate needs CAP_BPF, CAP_PERFMON, passwordless sudo and a kernel
with BTF and BPF LSM, and CI has none of them, so a wired-in gate could only ever
skip. A gate that always skips is not a gate. What CI does run, inside
`verify-all`, is `tools/test_bpf_objects.py`, which checks the half that needs no
kernel: the pinned versions against their floors, the admission table against the
gate's own constants, each mutation marker's presence and bite, the stub handler
set, the unmeasured TDP literal, the fact that the LSM fixture cannot deny, that
a verifier log is read back only when the load that wrote it stamped it, and that
a missing tool reaches the SKIP instead of a failure.

A host that cannot run the gate prints `SKIP: <reason>; the eBPF verifier gate
did not run.` and exits 0. **An exit 0 is evidence only when the case lines are
above it.** The PATH check for every tool runs *before* the first tool is
invoked; with the two in the other order a host without `clang`, `bpftool`,
`pkg-config` or `llvm-strip` reported a failure rather than the documented skip,
because invoking an absent binary is an error inside the gate and not a reason to
stand down. Nothing is suppressed: a case that fails fails the target.

## Scope, and what a pass here does not mean

- It does not close M10. M10 verifies these objects against a **Nucleus-built**
  kernel whose configuration this project controls. A stock distribution kernel
  cannot stand in for that, however current its verifier.
- It does not build, boot or publish anything. Image, boot, hardware,
  accessibility and release remain separate blocked gates.
- It measures nothing. No burst, latency, frame time, energy or power figure
  exists anywhere in this milestone; every threshold in `bpf/aegis_bpf_abi.h`
  is a declared budget and the two that could be mistaken for readings say
  `_UNMEASURED` in their names.
- It says nothing about correctness at runtime. `action_gate` was attached for
  seconds and observed one exec; `scx_cake` was never attached at all; the stub
  was attached with `SCX_OPS_SWITCH_PARTIAL` and `switch_all` read `0` for the
  whole window, so no task was switched to it. The verifier accepted all four
  objects; only `action_gate` was executed, for seconds, as an observer.
