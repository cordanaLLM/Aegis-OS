<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# Product input manifest, kernel requirement, and what the host said

Status: recorded observations from milestone M18, reference profile, 2026-09-13

This page records the two Aegis-owned schemas, the payloads this repository
ships under them, and what the reference machine answered when each asserted
feature was probed. Everything below is an observation with a command. Nothing
here builds an image, builds a kernel, boots anything, or contacts a producer
repository, and a pass is development evidence on one workstation that closes
no image, kernel, boot, hardware or release gate.

## What is shipped

| File | Role | Schema | Requirements |
| :--- | :--- | :--- | :--- |
| `build/product-input.json` | the product input manifest Aegis hands to an image builder | `aegis.p01.product-input.v1` | REQ-P01-01, REQ-P01-06, REQ-P01-08, REQ-GOV-02 |
| `build/kernel-requirement.json` | the kernel a conforming product must be built with | `aegis.p01-nucleus.kernel-requirement.v1` | REQ-P01-09, REQ-P06-05, REQ-P07-01, REQ-P07-06, REQ-P13-02, REQ-P03-06, REQ-BOOT-01 |
| `build/kernel-requirement.reference.json` | what the reference profile must keep providing for the local milestones to stay runnable | the same schema | the same set, minus the realtime row |
| `build/mkosi.conf` | the P01 image definition `mkosi summary` parses | mkosi 27 configuration | REQ-P01-01, REQ-P01-06, REQ-P02-06 |

The reviewed drop-ins in `build/repart.d` and `build/sysupdate.d` cite their
private sources in their own header comments. JSON has no comment syntax, so the
three payload files cite theirs differently and in-band: every feature row
carries a `required-by` identifier that resolves in
`docs/roadmap/requirements.md`, and that page carries the export id and digest
of the source each requirement came from. No private path and no source
fragment appears in any of them.

## The two schemas

`crates/aegis-fabrica-defs` owns both. The field primitives are in `src/field.rs`
and every one of them validates while the payload is being decoded, through
`#[serde(try_from = "String")]`, so an ill-formed field is a refusal rather than
a value that is checked later or not at all.

The manifest states what `docs/integration/stack.md` asks of every crossing of
the Aegis boundary: a schema, a correlation identifier, an exact revision,
bounded retries and a recorded result. It adds the pinned distribution snapshot
(D18), the four configuration references and the boot kernel identity (D07).

Validation is against the real files. `validate_definitions` takes the text of
the reviewed `repart.d` drop-ins and the `sysupdate.d` transfer and runs them
through the same parsers the M03 gate uses; the crate opens nothing itself.
A definition set that does not parse is a `DefinitionRefused`, and one that
parses but does not meet a recorded requirement -- dropping the alternate root
slot, for instance -- is a `RequirementNotMet` naming the finding.

The kernel requirement is a payload rather than a fixed symbol list. That
distinction is the milestone's, and it is checkable: the crate names no Kconfig
symbol in code at all. Every symbol lives in a JSON row of the form

```json
{
  "symbol": "CONFIG_BPF_LSM",
  "state": "built-in",
  "probe": "lsm-list",
  "required-by": "REQ-P06-05"
}
```

so a consumer adds a feature by adding a row, not by changing this repository.
The private readiness matrix records that Nucleus currently validates a fixed
symbol list; this payload is the shape Aegis proposes instead, and M09 is where
that proposal is made.

## The fragment M26 consumes

`KernelRequirement::config_fragment` renders the payload as a Kconfig fragment:
one assignment per feature row, with the recorded requirement identifier on the
line above it, and no line that is neither a comment nor an assignment. It
validates before it renders and returns the refusal otherwise, because every
field of the payload is public and a caller that assembled one without decoding
it would otherwise receive a fragment silently short of the rows past the
feature bound -- and M26 applies this fragment to a base configuration.

```text
# aegis.p01-nucleus.kernel-requirement.v1
# correlation-id: aegis-m18-kernel-requirement-0001
# minimum kernel release: 6.12
# REQ-P07-01
CONFIG_PREEMPT_RT=y
# REQ-P07-01
CONFIG_HZ_1000=y
# REQ-P07-06
CONFIG_SCHED_CLASS_EXT=y
```

`built-in` renders `=y`, `module` renders `=m`, `absent` renders
`# CONFIG_X is not set`, and `present` -- "either way" -- renders `=y`, because
a fragment states one assignment and built-in is the stronger of the two. That
is the form milestone M26 applies to a base configuration under decision D70.

## What the host answered

Each row of a requirement names the probe that checks it, and every probe was
run on the reference profile before the payloads were written. The commands and
their output:

| Command | Output |
| :--- | :--- |
| `zgrep CONFIG_PREEMPT_RT /proc/config.gz` | `# CONFIG_PREEMPT_RT is not set` |
| `zgrep CONFIG_PREEMPT_DYNAMIC /proc/config.gz` | `CONFIG_PREEMPT_DYNAMIC=y` |
| `zgrep CONFIG_HZ_1000 /proc/config.gz` | `CONFIG_HZ_1000=y` |
| `zgrep CONFIG_SCHED_CLASS_EXT /proc/config.gz` | `CONFIG_SCHED_CLASS_EXT=y` |
| `zgrep CONFIG_BPF_LSM /proc/config.gz` | `CONFIG_BPF_LSM=y` |
| `zgrep '^CONFIG_DEBUG_INFO_BTF=' /proc/config.gz` | `CONFIG_DEBUG_INFO_BTF=y` |
| `zgrep '^CONFIG_INTEL_RAPL=' /proc/config.gz` | `CONFIG_INTEL_RAPL=m` |
| `cat /sys/kernel/security/lsm` | `capability,landlock,lockdown,yama,bpf` |
| `ls /sys/kernel/btf/vmlinux` | `/sys/kernel/btf/vmlinux` |
| `ls /sys/class/powercap` | `intel-rapl`, `intel-rapl:0`, `intel-rapl:0:0` |
| `ls /sys/kernel/iommu_groups \| wc -l` | `38` |
| `uname -r` | `7.2.4-1-cachyos` |
| `uname -m` | `x86_64` |

Those values are transcribed into `planning/hardware-profile.json`, which is
what the schema is checked against.

The traceability is asserted rather than asserted-to. Each `ProbeSource` carries
the exact path it reads, and the test compares that path against the
`evidence_command` the profile records for the same capability, so a probe that
drifted away from the command the machine was measured with fails the gate
instead of reading plausibly.

## Positive, negative and boundary on the measured profile

- **Positive.** `build/kernel-requirement.reference.json` is satisfied by
  `planning/hardware-profile.json` with no unmet row: architecture `x86-64`,
  kernel release `7.2.4-1-cachyos`, the module ABI, thirteen Kconfig symbol
  states and the four runtime capabilities the probes read.
- **Negative, bound to a measured absence.**
  `build/kernel-requirement.json` -- the product requirement -- is refused by
  the same profile, with exactly one unmet row: `CONFIG_PREEMPT_RT` is required
  built-in and the profile records it as not set. This workstation runs
  `PREEMPT_DYNAMIC`, so it is not a kernel the product admits. That is the case
  the reference profile uniquely makes falsifiable: the refusal comes from a
  measured value, not from an invented one.
- **Boundary.** A payload demanding only `CONFIG_HZ_1000` is accepted by the
  very profile that refuses the product requirement, and relaxing only the
  `CONFIG_PREEMPT_RT` row makes the product requirement pass with nothing else
  changed. The schema therefore discriminates per feature, not per kernel
  flavour.

The distribution ships a realtime kernel (`extra/linux-rt`), so the absence is
an operator action rather than a missing capability; `docs/roadmap/hardware.md`
records it as one, and D57 assigns the realtime work to a guest kernel at M23.

## The mkosi gate

`make verify-mkosi` runs `tools/verify_mkosi_definitions.py`, which parses
`build/mkosi.conf` with the host's own mkosi. There is no `|| true` anywhere in
it, and it guards itself twice before reporting anything: mkosi missing from
`PATH`, or a host below the admitted floor, prints why it did not run and
reports no pass.

Three cases, all observed on mkosi 27:

| Case | Command | Outcome |
| :--- | :--- | :--- |
| positive | `mkosi --no-pager --directory build summary` | exit 0; the main image's Output stanza parses |
| negative | the same, on a scratch copy with `MinimumVersion=28` | exit 1, `mkosi 28 or newer is required by this configuration (found 27)` |
| boundary | the same, with `MinimumVersion=27`, the floor itself | exit 0 |

The floor cases run from a copy of `build/` in a temporary directory with
exactly one line rewritten; a definition with no `MinimumVersion=` line, or with
more than one, fails the gate rather than silently running both cases at the
same version.

What the positive case reads back from the `IMAGE: main` stanzas:

```text
Output Format: disk
Output:        aegis-os.raw
Image ID:      aegis-os
Distribution:  arch
Architecture:  x86-64
Snapshot:      2026/09/13
Repart Directories: <repository>/build/repart.d
```

The last line is the one that ties this milestone to M03: mkosi resolves
`RepartDirectories=repart.d` to the reviewed drop-in set, so the image
definition and the partition definitions are the same set of files rather than
two descriptions that happen to agree.

mkosi also reads a `mkosi.repart/` directory beside the configuration if one
exists, and a `mkosi.conf.d/` drop-in can add a further `RepartDirectories=`
row, so the resolved list is not necessarily the reviewed set alone. Neither of
those is a tracked file: git cannot record an empty directory, so a definition
tree can exist in a checkout, be read by mkosi and never appear in a diff. The
gate therefore requires two things rather than one, and the second is what keeps
the first honest: exactly one listed directory must be the reviewed one, and no
other listed directory may hold a partition definition. Identity is the resolved
path, not the end of the path, because a second set below another
`build/repart.d` is the case that matters. An empty extra directory is
tolerated; one holding a `.conf` fails the gate by name, and so does a second
row that resolves to the reviewed set itself, so a second definition set cannot
join the image without being seen.

`mkosi summary` resolves configuration and prints it. It downloads nothing and
builds nothing: `unshare -rn mkosi --no-pager --directory build summary` exits 0
with no network namespace at all, and the run leaves no file behind in the
repository.

## Where the pins came from

Every mkosi setting name was read from `mkosi documentation` on the installed
mkosi 27, not from memory. Two values needed the installed version's own source
rather than its manual:

- the Arch snapshot identifier is formatted `%Y/%m/%d` and is joined onto
  `https://archive.archlinux.org` as `repos/<snapshot>/$repo/os/$arch`;
- `linux-rt` is one of the names mkosi treats as a kernel package.

No mirror or archive was contacted to establish either. The snapshot is a
configuration pin, and whether that snapshot builds is M11 work.

The package set is the one the recorded requirements name and nothing more:
`linux-rt` (REQ-P01-09, the D07 default), `systemd` (which ships
`systemd-repart`) and `systemd-ukify` (REQ-P02-06, which on this distribution is
its own package). The M01 register's `remaining image packages` row stays open
for M11; guessing a content set no requirement asks for would be the same kind
of unpinned inheritance this milestone exists to remove.

## Scope limit

Two schemas, three reviewed payloads, one reviewed image definition and a
configuration parse. No image, UKI, kernel, artefact, signature or boot evidence
is produced or claimed. No producer repository is contacted, and neither the
Rust gate nor the mkosi gate needs a network: both were re-run inside an empty
network namespace and exited 0. `make build`, `make boot` and `make release`
remain blocked, and M09, M11 and M26 hold the evidence this page does not.
