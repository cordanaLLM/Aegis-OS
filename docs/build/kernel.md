<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The Aegis-built kernel, and what the guest said about it

Status: recorded observations from milestone M26, reference profile, 2026-09-13

Decision D70 puts kernel construction in this repository while Nucleus is a
scaffold, exactly as D56 keeps image-definition validation here while Imago is a
scaffold: a repository cannot defer construction to a producer that cannot yet
produce. This page records what was built, how the source was verified, what the
guest reported from inside itself, and what a pass here does **not** mean.

**Nothing on this page is a release artifact.** No image was packaged, nothing
was signed, nothing was installed, no bootloader entry was written, no module
was loaded on the host, and no device was touched. Construction returns to
Nucleus, which owns it under `docs/integration/stack.md`, once Nucleus returns
real artifacts against the M09 contract. Until then this is development evidence
on one workstation, and it closes no boot, hardware or release gate.

## What is tracked, and what is not

| Path | Role |
| :--- | :--- |
| `build/kernel/source.pin.json` | the pin: version, tarball digest, signature URLs and key fingerprints, base configuration, fragment order, expected release |
| `build/kernel/10-base-support.config` | the Kconfig and guest-boot prerequisites; states no product requirement |
| `build/kernel/50-aegis-requirement.config` | the product requirement, byte-identical to what `KernelRequirement::config_fragment` renders |
| `tools/guest/aegis-readback-init.sh` | PID 1 of the read-back guest |
| `tools/verify_kernel_build.py` | the gate, run as `make verify-kernel` |

Not tracked, and deliberately so: the source tree, the object tree, the produced
`.config`, `bzImage`, the modules, the initramfs and the guest console. They
live under `AEGIS_KERNEL_BUILD_DIR`, default
`${XDG_CACHE_HOME:-$HOME/.cache}/aegis-kernel`. A full `.config` is 5,520 lines
of which thirteen are the requirement; tracking it would bury the requirement in
the base configuration and make every upstream default a reviewed decision.

## The pinned source, and how it was verified

`linux-7.2.5`, the stable release current at the revision date, from
`https://cdn.kernel.org/pub/linux/kernel/v7.x/linux-7.2.5.tar.xz`.

```text
$ sha256sum linux-7.2.5.tar.xz
55ddf0df8325d9dad96fcff7bd93977d22e3f50af06527572af59b77c7632b78  linux-7.2.5.tar.xz

$ grep linux-7.2.5.tar.xz sha256sums.asc
55ddf0df8325d9dad96fcff7bd93977d22e3f50af06527572af59b77c7632b78  linux-7.2.5.tar.xz

$ gpg --verify sha256sums.asc
gpg: Good signature from "Kernel.org checksum autosigner <autosigner@kernel.org>"
Primary key fingerprint: B886 8C80 BA62 A1FF FAF5  FDA9 632D 3A06 589D A6B1

$ xz -dc linux-7.2.5.tar.xz | gpg --verify linux-7.2.5.tar.sign -
gpg: Good signature from "Greg Kroah-Hartman <gregkh@linuxfoundation.org>"
Primary key fingerprint: 647F 2865 4894 E3BD 4571  99BE 38DB BDC8 6092 693E
```

Both keys are fetched once into a keyring under the build directory. Neither is
certified by a local trust path, so gpg also prints its "not certified with a
trusted signature" warning; the pin asserts the fingerprint, not a web of trust.
The gate refuses a source whose signature is good but made by some other key.

`CONFIG_PREEMPT_RT` needs no out-of-tree patch here, and that was read out of the
pinned source rather than assumed: `kernel/Kconfig.preempt` carries
`config PREEMPT_RT` with `depends on EXPERT && ARCH_SUPPORTS_RT && !COMPILE_TEST`,
and `arch/x86/Kconfig` selects `ARCH_SUPPORTS_RT`.

## The configuration: a fragment on a named base

The base is `x86_64_defconfig`. The two tracked fragments are applied in order
by the kernel's own `scripts/kconfig/merge_config.sh -m`, then resolved with
`make olddefconfig`.

`50-aegis-requirement.config` is **generated**, not written. It is what
`KernelRequirement::config_fragment` renders from `build/kernel-requirement.json`
-- one assignment per feature row with its requirement identifier on the line
above -- and `crates/aegis-fabrica-defs/tests/kernel_fragment.rs` fails if the
tracked file and the renderer's output differ by a byte. That is what makes the
M18 schema the source of the requirement rather than a description of a
hand-written file.

`10-base-support.config` carries only prerequisites, each with the Kconfig
dependency it satisfies named above it: `CONFIG_EXPERT` for `PREEMPT_RT`,
`BPF_JIT`/`SECURITY`/`SECURITYFS`/`KPROBES`/`PERF_EVENTS` for `BPF_LSM`, the
"Debug information" choice for `DEBUG_INFO_BTF`, `PCI`/`IOSF_MBI` for
`INTEL_RAPL`, an IOMMU driver so the non-selectable `IOMMU_API` can be `y`,
`IKCONFIG_PROC` for the read-back, the serial and initrd options for the guest,
and `CONFIG_LOCALVERSION="-aegis-m26"` with `LOCALVERSION_AUTO` off. It assigns
no symbol the payload demands, and a test fails if it ever does -- otherwise a
hand-written file could satisfy a schema row while the generated fragment said
nothing.

## What was built

```text
$ make O=<build>/out/positive -j32 bzImage modules
Kernel: arch/x86/boot/bzImage is ready  (#1)
```

69 seconds on the reference profile's 32 threads, from a freshly configured
tree. The artifacts are a 16,438,272-byte `bzImage` and 19 modules, among them
`vfio.ko`, `vfio-pci.ko`, `vfio_iommu_type1.ko`, `intel_rapl_common.ko` and
`intel_rapl_msr.ko` -- the module-state rows of the payload, compiled rather
than merely configured.

One honest detail about `CONFIG_KVM=m`: the produced configuration carries it,
which is what the requirement row asks about, but no `kvm.ko` is produced.
`arch/x86/kvm/Kconfig` makes the module that actually gets built `KVM_X86`,
`def_tristate KVM if (KVM_INTEL != n || KVM_AMD != n)`, and `x86_64_defconfig`
selects neither vendor module. The row is satisfied as a Kconfig state; a KVM
module object is not part of this evidence.

## The read-back, from inside the guest

The guest is a minimal initramfs booted by
`qemu-system-x86_64 -enable-kvm -cpu max -kernel <bzImage> -initrd <cpio>`. Its
`/init` mounts its own `/proc`, and reports on a second serial line so that a
kernel `printk` cannot splice itself into the configuration being reported.
`CONFIG_IKCONFIG_PROC` is why `/proc/config.gz` exists at all: what the guest
prints is the configuration compiled into the image that is executing the
script, not a file from the build tree.

The guest's own output, verbatim, first five lines and last two:

```text
AEGIS-M26-BEGIN
AEGIS-M26-UNAME-R 7.2.5-aegis-m26
AEGIS-M26-UNAME-V #1 SMP PREEMPT_RT Sun Sep 13 18:32:20 CEST 2026
AEGIS-M26-UNAME-M x86_64
AEGIS-M26-CONFIG-BEGIN
...
AEGIS-M26-CONFIG-END
AEGIS-M26-END
```

and the required options as the guest printed them, with the line number each
sits on in the guest's own dump:

```text
 49:CONFIG_LOCALVERSION="-aegis-m26"
134:CONFIG_BPF_SYSCALL=y
140:CONFIG_BPF_LSM=y
147:CONFIG_PREEMPT_RT=y
153:CONFIG_SCHED_CLASS_EXT=y
190:CONFIG_IKCONFIG_PROC=y
238:CONFIG_CGROUP_BPF=y
503:CONFIG_HZ_1000=y
504:CONFIG_HZ=1000
740:CONFIG_KVM=m
4146:CONFIG_VFIO=m
4159:CONFIG_VFIO_PCI=m
4328:CONFIG_IOMMU_API=y
4482:CONFIG_POWERCAP=y
4484:CONFIG_INTEL_RAPL=m
5197:CONFIG_DEBUG_INFO_BTF=y
```

The host's own kernel at the time was `7.2.4-1-cachyos` with
`# CONFIG_PREEMPT_RT is not set`. Three separate things therefore have to hold
before the read-back passes, and each is checked:

1. the release the guest reports equals the release this build produced
   (`include/config/kernel.release`, `7.2.5-aegis-m26`) and is not the host's;
2. every one of the thirteen requirement rows holds in the text the guest
   printed;
3. that text is the produced `.config` **byte for byte** -- 5,520 lines. A
   read-back that came from anywhere but the running guest kernel could not
   reproduce it.

The guest then powers itself off through magic SysRq (`reboot: Power down` on
the console), so the gate's deadline is a failure signal rather than the normal
exit path.

## The cases, and what each proves

`make verify-kernel` runs seven cases. None of them suppresses a failure and
none uses `|| true`. Each is its own unit: a run that fails two of them reports
two failures. Once the gate is past its toolchain check it runs all seven, with
one exception: `kernel/readback-negative-missing-option` needs the host's own
`/proc/config.gz` and is skipped, by name, on a kernel that publishes none. The
skip takes nothing else with it -- `kernel/readback-boundary-host-kernel` reads
only `uname -r`, so it still runs and is still counted.

| Case | What it does | Outcome |
| :--- | :--- | :--- |
| `kernel/positive` | the two tracked fragments on `x86_64_defconfig` | every requirement row satisfied |
| `kernel/negative-contradictory` | appends `# CONFIG_PREEMPT_RT is not set` | refused: `CONFIG_PREEMPT_RT: REQ-P07-01 requires built-in, observed 'n'` |
| `kernel/boundary-builtin-where-module` | appends `CONFIG_VFIO=y` where the schema demands a module | refused: `CONFIG_VFIO: REQ-P03-06 requires module, observed 'y'` |
| `kernel/boundary-module-where-builtin` | appends `CONFIG_BPF_SYSCALL=m` where the schema demands built-in | kconfig resolved the request to `'n'`; refused, naming `CONFIG_BPF_SYSCALL` and the two rows that fell with it |
| `kernel/readback-positive` | boots the built image, reads `/proc/config.gz` in the guest | release, rows and byte-identity all hold |
| `kernel/readback-negative-missing-option` | the same check over the host's own `/proc/config.gz` | refused: `CONFIG_PREEMPT_RT: REQ-P07-01 requires built-in, observed 'n'` |
| `kernel/readback-boundary-host-kernel` | the host's release through the identity check | refused twice: not the built release, and it is the host's own |

The fourth case is the interesting one and is worth stating plainly, because it
is the failure mode this whole gate exists for. Every symbol the payload demands
built-in is a `bool` in the pinned source, so `=m` is not a state any of them can
reach. kconfig neither honours the line nor complains about it: it resolves
`CONFIG_BPF_SYSCALL` to `n`. A fragment asking for a module therefore yields a
kernel **without** the feature rather than one carrying it as a module, and
`CONFIG_BPF_LSM` and `CONFIG_SCHED_CLASS_EXT` disappear with it. Nothing in the
build says so; only reading the produced configuration back does.

## Scope, and what a pass here does not mean

- Development evidence on the reference profile recorded in
  `planning/hardware-profile.json`. It qualifies no hardware.
- It is **not** a substitute for the Nucleus contract in M09. M09 pins one
  request/result pair against a real producer; this milestone builds a kernel
  locally because that producer is a scaffold.
- No boot gate is closed. The guest boot here is a configuration read-back, not
  a measured boot: no UKI, no Secure Boot, no TPM measurement, no
  `bootctl status` evidence.
- No release gate is closed. Nothing is packaged, signed or published.
- The guest's userspace is the host's own `bash`, `mount`, `uname`, `gzip` and
  `sleep` with their shared-library closure. Only the kernel under test is built
  here; the initramfs is transport, and the read-back would be equally valid
  with any userspace that can read `/proc/config.gz`.
- No latency, jitter or determinism figure is claimed. `CONFIG_PREEMPT_RT` is
  confirmed as a configuration state and `uname -v` reports `PREEMPT_RT`;
  measuring what that buys is milestone M23.
- A host that cannot run the gate has two outcomes, and they say different
  things. A missing tool, or one below the floor the pinned source declares,
  prints `SKIP: <reason>; the kernel build gate did not run.` and exits 0 --
  the same convention `verify-systemd` and `verify-mkosi` use -- so an exit 0
  from `make verify-kernel` is evidence only when the case lines are above it.
  A host that has the toolchain but cannot obtain the pinned source, with no
  network and no cached tarball, prints `FAIL: the gate could not run:` and
  exits 1 instead: an unverifiable source is a refusal, not a skip.
- `make verify-all` does **not** run this gate, and the omission is deliberate.
  See the comment on the `verify-kernel` target in the `Makefile`: it downloads
  160 MB, extracts 1.4 GB and compiles a kernel, and the CI runner has no kernel
  toolchain, no `/dev/kvm` and no emulator, so wiring it in would add a step
  that can only skip. What CI does re-run is the binding that matters
  everywhere: `crates/aegis-fabrica-defs/tests/kernel_fragment.rs` fails if the
  tracked fragment stops being what the M18 schema renders, and
  `tools/test_kernel_build.py` fails if the pin, the fragments or the recorded
  toolchain drift apart. CI never compiles a kernel and never boots one.
