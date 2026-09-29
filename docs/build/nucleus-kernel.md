<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The Nucleus kernel in a guest, and the hook it cannot attach

Status: recorded observations from milestone M10, reference profile,
2026-09-29

On 2026-09-29 cordanaLLM/nucleus published the first kernel it built with an
`imago.nucleus.kernel-artifact.v1` manifest, release
`v7.2.8-realtime-lusoris1`. M10's first exit criterion had waited for exactly
that. `make verify-nucleus-kernel` verifies and records the release, boots
the kernel in a guest, checks the Aegis kernel requirement against the
configuration the kernel reports from inside the guest, and only then repeats
the M19 loads on it. Run `r20260929T195935-62a3` was the first to reach every
case, and passed seventeen of eighteen. Review then tightened the gate: the
signature now binds the tag commit, the release record is written before the
first boot, every case of a stage gates the next stage, cosign is held below
3, and the requirement check refuses unknown keys and checks
`abi.module-abi`. Run `r20260929T203240-505f` started after the last edit of
the gate's files and gave the same eighteen outcomes:

```text
PASS nucleus/assets-pinned
PASS nucleus/signature-verified
PASS nucleus/imago-accepts-and-recorded
PASS nucleus/digest-mismatch-refused-before-boot
PASS nucleus/minimum-release-boundary
PASS nucleus/guest-identity-and-config
PASS nucleus/capabilities-read-in-guest-diffed-with-host
PASS nucleus/requirement-satisfied-before-any-load
PASS nucleus/requirement-unsatisfied-rejected
PASS nucleus/requirement-module-boundary
PASS nucleus/load-guest-config-matches-checked
FAIL action_gate/nucleus-loads-and-attaches
PASS action_gate/nucleus-unchecked-pointer-rejected
PASS scx_cake/nucleus-struct-ops-attaches
PASS scx_cake/nucleus-unbounded-rejected
PASS kepler_power/nucleus-tracepoint-attaches
PASS kepler_power/nucleus-missing-tracepoint-reported
PASS kepler_power/nucleus-idle-zero-delta-recorded
```

**The one failure is a finding about the kernel, not a defect in the gate.**
The Nucleus kernel's verifier accepts `action_gate`, but the program cannot
attach to its LSM hook: the kernel is built without `CONFIG_FUNCTION_TRACER`,
so the BPF trampoline an LSM program attaches through has no entry point to
patch. The Aegis kernel requirement asks for `CONFIG_BPF_LSM` and the kernel
has it; the requirement does not ask for the trampoline, and neither does
M26's own kernel have it. [The finding](#the-finding-bpf-lsm-present-but-not-attachable)
below records the controlled comparison that confirms the cause. M10 is
therefore not done.

**Nothing here is a release artifact and nothing here qualifies hardware.**
No package was installed, no module was loaded and no boot entry was written
on the host. A pass on this profile is development evidence only.

## What is tracked, and what is not

| Path | Role |
| :--- | :--- |
| `build/kernel/nucleus-artifact.pin.json` | the release, every asset by sha256 and size, the tag commit, the keyless signer |
| `tools/verify_nucleus_kernel.py` | the gate, run as `make verify-nucleus-kernel`; `--fetch` is `make nucleus-kernel-fetch` |
| `tools/kernel_requirement_check.py` | the D94 check of `build/kernel-requirement.json` against one configuration |
| `tools/guest/aegis-nucleus-init.sh` | PID 1 of both guests |
| `tools/guest/aegis-nucleus-probe.sh` | the sched_ext, BPF LSM and BTF probe, run in the guest and on the host |
| `bpf/loader/aegis_bpf_probe.c` | M19's loader, with M10's `tp-attach` mode and `--map-set` option |
| `tools/test_nucleus_kernel.py`, `tools/test_kernel_requirement_check.py` | the half of the gate that runs inside `make verify-all` |

Not tracked: the release, the guest trees, the reports, the verifier logs and
each run's `summary.json`. They live under `AEGIS_NUCLEUS_KERNEL_DIR`, default
`${XDG_CACHE_HOME:-$HOME/.cache}/aegis-nucleus-kernel`, one directory per run
id under `runs/`.

## Running it

```sh
make contract-fetch          # M09's imago, if its cache is absent
make nucleus-kernel-fetch    # the one networked step
make verify-nucleus-kernel
```

`make nucleus-kernel-fetch` downloads the pinned assets with curl, refuses any
whose sha256 or size is not the pin's, and runs `cosign verify-blob` on
`SHA256SUMS`. The gate never touches the network: cosign runs `--offline`
against the bundle's own transparency-log entry.

A host that cannot run the gate prints
`SKIP: <reason>; the Nucleus kernel gate did not run.` and exits 0: not
Linux (macOS and Windows name their platform), no read-write `/dev/kvm`, no
`/proc/config.gz`, a tool missing or outside its admitted range, no M09
contract cache, or no fetched release. cosign is the one tool with a ceiling:
the reference profile carries 2.6.3 and the distribution's 3.1.3, and a
`PATH` that names 3.1.3 is refused by the gate and by the fetch alike. An
exit 0 is evidence only when the case lines are above it. A cache that is
present but wrong is a FAIL: a symlinked or altered asset fails
`nucleus/assets-pinned`, observed on 2026-09-29 with a hand-edited
`SHA256SUMS`. The gate is not part of `make verify-all`, for the
reasons `make verify-latency` is not; `docs/roadmap/toolchain-admission.md`
records every tool it runs.

## The order is the contract

Nothing boots before the release is verified and recorded (E10-4), and no
M19 object loads before the requirement has been checked against the
configuration the kernel reported from inside the guest (D94, E10-5). Each
stage runs all of its cases, and the next stage starts only when every one of
them passed, the negatives and boundaries included. `OrderTests`,
`ReadbackPhaseTests` and `LoadPhaseTests` in `tools/test_nucleus_kernel.py`
hold that order inside `make verify-all`.

1. **The release.** Every cached asset hashes to the pin. cosign verifies
   `SHA256SUMS` against the exact signer identity the pin records, with the
   tag commit and the tag as the certificate's workflow SHA and ref. A second
   identity under the same repository is refused, and so is a commit that
   differs in its last digit, so both are load-bearing. imago, the binary
   M09's fetch built at `16f964b`, whose sha256 the gate checks against the
   fetch's identity record, runs `imago kernel artifact verify` with the
   stream, tag and version the pin expects, and returns what M10 records. The
   record is written to the run's `release-record.json` when this stage ends.
2. **The readback boot.** M23's `boot()` in `tools/verify_latency_fixture.py`
   runs unchanged, with this kernel image and an M10 initramfs: the emulator
   argument vector and the kernel command line are M23's, and the initramfs is
   built through M23's `install_programs` and `archive_tree`. The kernel image
   is hashed against the pin immediately before each boot, and a mismatch
   raises before the emulator starts.
3. **The load boot**, only when the readback cases passed. PID 1 hashes its
   own `/proc/config.gz` again and loads nothing unless the sha256 is the one
   the gate checked.

## E10-4: the release, verified and recorded

imago accepted the manifest against the downloaded assets. The values it
returned, each equal to the pin, were written to the run's
`release-record.json` at the end of the release stage. In run
`r20260929T203240-505f` that file was written at 18:32:41 UTC, before the
readback boot started, and the guest's report is dated three seconds later;
`summary.json` repeats the values when the run ends.

| Field | Recorded value |
| :--- | :--- |
| `kernel.release` | `7.2.8-lusoris1-realtime` |
| `kernel.config_digest` | `sha256:e9510d2d1928d4e153b19cf3312c4184ce44b4186ec6fde286e001060ffd731a` |
| artifact digest (`SHA256SUMS`) | `sha256:62ceccf9b6730fce5790df8803375b3c1c32f67f1a6ca6bc94f7ba12b2c9c90f` |
| `vmlinuz-7.2.8-lusoris1-realtime` | `50ec65c5d6b77de9b14c1f2c0c2f07d668edcff3b44ce1a44196659cdc66d626` |
| provenance revision | `9e050cf8fc2fb62de943229f22ab104dce455e92` |

Both files record all seven artifact digests. imago reports the manifest
file itself as an unlisted file beside the assets, which it lists rather than
rejects.

The revision is bound to the signature, not only to the pin. The manifest is
not listed in `SHA256SUMS`, so its `provenance.revision` field is unsigned.
The Fulcio certificate in `SHA256SUMS.bundle` does carry the workflow's
commit and ref, and the gate passes `--certificate-github-workflow-sha` with
the pinned commit and `--certificate-github-workflow-ref` with
`refs/tags/v7.2.8-realtime-lusoris1`. cosign verified both. It refused
`9e050cf8fc2fb62de943229f22ab104dce455e90`, the pinned commit with its last
digit changed, with `expected GitHub Workflow SHA not found in certificate`.

Negative: a copy of the release whose kernel image differs from the published
one in a single byte, same size, was refused twice, and no guest was started
for it. The gate's pre-boot hash refused it with the two digests, and imago
refused it with
`artifacts[vmlinuz-7.2.8-lusoris1-realtime]: digest mismatch`.
`PreBootRefusalTests` holds that the boot path raises before M23's `boot()`
is called.

Boundary: `abi.minimum-release` is `6.12`. The release rule of
`KernelRelease::at_least`, reimplemented in `tools/kernel_requirement_check.py`,
admits `6.12` and `6.12.0-lusoris1` and refuses `6.11.999`, the highest
release below the floor. The kernel's own `7.2.8-lusoris1-realtime` is
admitted, and a kernel reporting `6.11.999` is rejected by the requirement
check with the correlation id named. These are rule evaluations; the only
kernel booted is the pinned one.

## The guest's own configuration, and what the host supplied

The readback guest reported the release `7.2.8-lusoris1-realtime` and
`#lusoris1 SMP PREEMPT_RT`, and its own `/proc/config.gz` hashed to
`e9510d2d...ffd731a`, the manifest's `kernel.config_digest`. The configuration
checked below is therefore byte for byte the one Nucleus published.

Criterion 6 reads sched_ext, BPF LSM and BTF from the kernel under test and
diffs each against the host. The same probe script runs in both places:

| Reading | Guest | Host | Reference profile |
| :--- | :--- | :--- | :--- |
| `CONFIG_SCHED_CLASS_EXT` | `y` | `y` | `y` |
| `bpf` in the active LSM list | yes | yes | yes |
| `/sys/kernel/btf/vmlinux` | present | present | present |
| `CONFIG_FUNCTION_TRACER` (recorded, not required) | not set | `y` | not recorded |
| `CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS` (recorded, not required) | absent | `y` | not recorded |

The guest's active LSM list is `capability,lockdown,apparmor,bpf`; the host's
is `capability,landlock,lockdown,yama,bpf`. Each capability is decided from
the guest's reading alone. `CapabilityTests` holds that a guest without `bpf`
fails although the host has it (E10-1's boundary), and that a guest without
sched_ext is recorded as a Nucleus requirement defect (E10-2's boundary).
The last two rows are the ones the host silently supplied: they are why the
M19 attach passed on the host and the M10 attach does not.

## D94: the requirement against the built kernel

`tools/kernel_requirement_check.py` decides every row of
`build/kernel-requirement.json` by the M18 rules: built-in by `y` only, module
by `m` only, present by either, absent by `not set`, and an unobserved symbol
by nothing. It also mirrors the identity half of `KernelRequirement::unmet`:
every listed architecture, the release floor, and `abi.module-abi` as an
exact release when a payload fixes one. It refuses any key the schema does
not name, as the Rust decoders' `deny_unknown_fields` does. It does not
decide the capability rows the Rust half reads from a profile. All thirteen
features were satisfied by the configuration the guest printed, and each
symbol's state is in `summary.json`. The four module
rows, `CONFIG_INTEL_RAPL`, `CONFIG_VFIO`, `CONFIG_VFIO_PCI` and `CONFIG_KVM`,
read `m` in the guest.

Negative and boundary, each planted into a copy of the guest's own
configuration and each rejected alone with
`correlation-id aegis-m18-kernel-requirement-0001` and the symbol named:
`CONFIG_SCHED_CLASS_EXT=n`, `# CONFIG_BPF_LSM is not set`, a removed
`CONFIG_HZ_1000` line, `CONFIG_BPF_SYSCALL=m` against a built-in row, and
`CONFIG_KVM=y` against a module row.

The runtime probes the requirement rows name for powercap and the IOMMU groups
are not read in the guest: QEMU presents neither RAPL counters nor an IOMMU
here, so a reading would describe the emulator. D94 is a check of the
configuration.

## The loads

Every load runs as uid 65534 under `setpriv` with exactly M19's capability
set, `-all,+bpf,+perfmon`. The objects are M19's sources, compiled by M19's
own gate functions against a `vmlinux.h` dumped from the Nucleus kernel's
BTF, which the gate extracts from the image it verified. Every verifier log
carries its load's nonce and is kept in full under the run's `load/logs/`.

| Epic | Positive | Negative | Boundary |
| :--- | :--- | :--- | :--- |
| E10-1 `action_gate` | FAIL: loads, `load_rc=0`, then `failed to attach: -EBUSY` | PASS: `R7 invalid mem access 'ringbuf_mem_or_null'` | see criterion 6 above; the attach failure carries its recorded reason |
| E10-2 `scx_cake` | PASS: `sched_ext_ops_during=aegis_cake`, `switch_all=1`, still attached after 1 s, `disabled (unregistered from user space)` | PASS: `The sequence of 8193 jumps is too complex.` | see criterion 6 above |
| E10-3 `kepler_power` | PASS: `sched/sched_switch` attached; runtime grew in the root cgroup | PASS: `tracepoint_attach_failed tracepoint=sched/aegis_no_such_tracepoint errno=2` | PASS: an idle cgroup's delta over 3 s recorded as 0 uJ and 0 ns |

The negatives reuse M19's recorded mutations: the gate deletes the same
marked region and requires the same rejection string, from a log this load
wrote, absent from the unmutated object's log. The kepler value is the
object's declared runtime-times-TDP figure, `pseudo_energy_uj_unmeasured`,
not a meter reading. The idle cgroup holds one task that enters it after the
attach and sleeps through the interval, so its row exists at both ends and
its delta is a reading of zero, not an absent line.

## The finding: BPF LSM present but not attachable

`action_gate` loads, and its attach fails:

```text
program=action_gate_exec section=lsm/bprm_check_security type=29 fd_valid=1
load_rc=0
libbpf: prog 'action_gate_exec': failed to attach: -EBUSY
lsm attach failed: Device or resource busy
```

A BPF LSM program attaches through a BPF trampoline. On x86 the kernel
installs it by patching the five-byte nop that `-mfentry` leaves at the hook
function's entry, and `__bpf_arch_text_poke` in
`arch/x86/net/bpf_jit_comp.c` returns `-EBUSY` when the bytes there are not
that nop. A kernel built without `CONFIG_FUNCTION_TRACER` is not compiled
with `-mfentry`, so the nop is absent whatever `CONFIG_BPF_LSM` says, and
`CONFIG_BPF_LSM` does not depend on the function tracer in Kconfig.

That was a hypothesis, and it was checked with a controlled comparison on
2026-09-29. Two kernels were built from M26's verified linux 7.2.5 source and
M26's configuration, one unchanged and one with `CONFIG_FTRACE`,
`CONFIG_FUNCTION_TRACER` and `CONFIG_DYNAMIC_FTRACE` switched on. The
configuration diff is those options and the ones they select, plus
`CONFIG_PAHOLE_VERSION`, which records the host's pahole and moved from 131
to 132 since M26 built its kernel.
Both booted through the same `boot()` with the same load initramfs, objects,
loader and capability set. The unchanged kernel failed with the same
`-EBUSY`; the other attached, and the marker exec reached the ring buffer
(the event line is cut after its first two fields here):

```text
load_rc=0
attached=lsm ringbuf=action_ringbuf
event filename=/aegis-m10/aegis_exec_prob comm=aegis_bpf_probe ...
detached=lsm marker_seen=1
```

The comparison, its configuration diff and both reports are kept under
`control-function-tracer-20260929/` in the gate's cache. It is not part of
the gate: the gate boots only the pinned kernel.

What would close E10-1 is a kernel requirement that states the trampoline
prerequisite and a Nucleus release built to it. The row would name
`CONFIG_DYNAMIC_FTRACE_WITH_DIRECT_CALLS` or `CONFIG_FUNCTION_TRACER`. That
changes the M18 payload that imago and nucleus both vendor and M09 pins, so it
is a recorded decision for the maintainer, not a change made here; see the
M10 entry in `docs/roadmap/README.md`.

## Two things M19 never exercised

M19 loaded `scx_cake` and never attached it, and loaded `kepler_power` and
never attached it. Attaching both on the Nucleus kernel found two gaps, each
closed on the loader side and recorded here:

- **scx_cake stalled every task until the watchdog ejected it.** Its
  `dispatch` walks as many tiers as the one-row array `cake_tier_budget`
  holds, and the row reads zero until a loader writes it. Run
  `r20260929T194509-ada2` attached it with the zero budget: the guest console
  recorded `disabled (runnable task stall)` after 30 seconds. The loader's new
  `--map-set cake_tier_budget=4` writes `AEGIS_CAKE_TIERS`, read from
  `bpf/aegis_bpf_abi.h`, after the load and before the attach. The source is
  unchanged, so the budget stays a value the verifier does not know and M19's
  unbounded negative keeps its meaning.
- **The tracepoint attach needs tracefs access.** CAP_PERFMON does not bypass
  file permissions, and libbpf reads the tracepoint's id from tracefs. PID 1
  mounts tracefs group-owned by the loads' gid, 65534.

The loader's `tp-attach` mode is new: it attaches the tracepoint program, to
the tracepoint its section names or to one given with `--tracepoint`, reads
the per-cgroup map at both ends of an interval inside the attach window, and
prints a failed attach as `tracepoint_attach_failed` with its errno. M19's
four modes are unchanged, and `tools/test_bpf_objects.py` still passes.

## What this does not claim

- It does not close M10. E10-1's positive fails on this kernel, as recorded
  above.
- It is development evidence on the reference profile, closing neither the
  hardware nor the release gate. The M19 host-kernel fixture closes nothing
  here either: it is exactly the fixture whose host supplied the trampoline.
- The guest's userspace, libbpf included, is the host's own; only the kernel
  is under test.
- No SBOM content is checked; the two SBOMs are verified by digest only.
