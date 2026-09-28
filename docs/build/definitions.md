<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# P01/P02 definitions and what systemd said about them

Status: recorded observations from milestone M03, reference profile,
2026-09-13; re-run on systemd 262 on 2026-09-28

This page records what the reviewed definitions in `build/` are, what the host's
own systemd did with them, and what is deliberately left open. Everything below
is an observation with an exit code. The repart gate does write a throwaway
sparse scratch image inside a temporary directory that is removed when it exits
-- see "`--dry-run=yes` is inert under `--empty=create`" below -- and beyond
that image nothing is published, signed, written to a device or booted. A pass
here is development evidence on one machine and closes no image, boot, hardware
or release gate.

## What is shipped

| File | Role | Derived from | Requirements |
| :--- | :--- | :--- | :--- |
| `build/repart.d/00-esp.conf` | EFI system partition, 512M..1G | export-053 | REQ-P01-10 |
| `build/repart.d/10-root-a.conf` | root slot A: EROFS with dm-verity data | export-054 | REQ-P01-02, REQ-P02-07 |
| `build/repart.d/10-root-b.conf` | root slot B: the empty alternate slot | export-063 (the A/B target) | REQ-P01-04, REQ-P02-04 |
| `build/repart.d/11-root-verity.conf` | dm-verity hash partition for slot A | export-054 (its `Verity=data` pair) | REQ-P01-02, REQ-P02-07 |
| `build/repart.d/20-var.conf` | TPM2-sealed Btrfs `/var` | export-055 | REQ-P01-03, REQ-P02-03 |
| `build/sysupdate.d/10-root.transfer` | dual-slot A/B root transfer | export-063 | REQ-P01-04, REQ-P02-04 |

Each file carries its export id and the sha256 of that export in its own header
comment. No private path appears in any tracked file, and no file reproduces
more than a fragment of its source.

## The two gates

`make verify-all` runs `make verify-systemd`, which is
`tools/verify_systemd_definitions.py`. There is no `|| true` anywhere in it: a
case that does not match its recorded outcome fails the gate, and the gate's
exit code is the target's exit code.

The script guards itself twice before it reports anything:

1. each of `systemd-repart` and `systemd-sysupdate` is looked up on `PATH`
   first and then in systemd's libexec directory, `/usr/lib/systemd`, and the
   gate prints the copy it will run and where it came from. A tool in neither
   place prints a `SKIP:` line naming the tool and both places searched.
2. each tool it found is held to the admitted floor by its own `--version`. A
   tool below the floor, or one that prints no systemd banner, gives a `SKIP:`
   line naming the tool, its path and its banner, the floor and the recorded
   reference profile, and the gate does not run. A copy on `PATH` can be a
   different release from the systemd running as PID 1, so `systemctl
   --version` is printed beside the tools' banners as a cross-check only and
   does not decide admission.

Both print why; neither reports a pass.

The libexec step exists because systemd 262 moved `systemd-sysupdate` from
`/usr/bin` back to its libexec directory and calls that path "the most
compatible way to call it" (`NEWS`, "CHANGES WITH 262"); the 261.3 package had
installed `/usr/bin/systemd-sysupdate`. Until 2026-09-28 the gate looked on
`PATH` only, so it printed
`SKIP: systemd-sysupdate not on PATH; the systemd definition gate did not run.`
on the reference profile from the systemd 262 upgrade on 2026-09-25 onward, and
on CI since M03 merged: the ubuntu-24.04 runner's systemd 255.4-1ubuntu8.17
ships the tool only at `/usr/lib/systemd/systemd-sysupdate` (its
packages.ubuntu.com file list), and the CI logs at the M03 merge (`61f2fe1`)
and at `020bcd5` both show that line. With the libexec step the runner should
find the tool and skip on the floor instead, because 255 is below 261, so CI
would still not run this gate. That is expected from the code and from the
mocked test `test_a_libexec_copy_below_the_floor_still_skips`; no CI run of the
change has been read yet, and its first log on this branch is what confirms it.

## Recorded outcomes on systemd 261 (261.3-1-arch)

| Case | Definitions | Exit | Decisive diagnostic |
| :--- | :--- | :---: | :--- |
| repart, positive | `build/repart.d` | 0 | none; no `Unknown key` |
| repart, negative | `tests/systemd/repart-missing-type` | 1 | `50-no-type.conf:1: Type= not defined, refusing.` |
| repart, negative | `tests/systemd/repart-inverted-size` | 1 | `00-esp.conf:1: SizeMinBytes= larger than SizeMaxBytes=, refusing.` |
| repart, negative | `tests/systemd/repart-unknown-key` | **0** | `00-esp.conf:14: Unknown key 'Subsystem' in section [Partition], ignoring.` |
| repart, boundary | `tests/systemd/repart-equal-size` | 0 | none; `SizeMinBytes=512M` equal to `SizeMaxBytes=512M` is accepted |
| sysupdate, positive | `build/sysupdate.d` | 0 | lists both slots: `{"current":"b","all":["b","a"]}` |
| sysupdate, negative | `tests/systemd/sysupdate-unsupported-keys` | 1 | six `Unknown key` lines, then `10-root.transfer:1: Source specification lacks MatchPattern=.` |
| sysupdate, boundary | `tests/systemd/sysupdate-single-slot` | **0** | `10-root.transfer:26: InstancesMax= value must be at least 2, bumping: 1` |
| sysupdate, `--root=` tree | the reviewed transfer in a scratch tree | 1 | `10-root.transfer:1: Source Type= must be one of url-file, url-tar, tar, regular-file, directory, subvolume.` |

The two bold zeroes are the reason the gate does not read exit codes alone. An
unknown key and a single-slot transfer both leave systemd reporting success
while doing something other than what the file says. REQ-CI-01 names exactly
this failure mode in the imported CI workflow, which suppressed the repart
validation step's failures outright.

## Re-run on systemd 262 (262-1-arch), 2026-09-28

With the tool found in `/usr/lib/systemd`, eight of the nine cases reproduce the
table above exactly on systemd 262: the same exit codes and the same decisive
diagnostics. The `--root=` case did not. As first staged it exited 1 on
`Failed to resolve '<scratch>/positive.raw': No such file or directory`, a third
outcome, so the gate failed, which is its job. The next finding records what
changed and how the case was restaged; after that, all nine cases pass:

<!-- markdownlint-disable MD013 -->
```text
systemd-repart: /usr/bin/systemd-repart (from PATH)
systemd-sysupdate: /usr/lib/systemd/systemd-sysupdate (from the systemd libexec directory /usr/lib/systemd, not PATH)
systemd definition gate on systemd-repart 'systemd 262 (262-1-arch)' and systemd-sysupdate 'systemd 262 (262-1-arch)' (floor systemd 261).
systemctl reports 'systemd 262 (262-1-arch)': a cross-check only; the floor applies to the tools.
PASS sysupdate/root-tree: exit 0
PASS: repart and sysupdate definition gates on the reference profile; development evidence only, no image, boot, hardware or release gate is closed.
```
<!-- markdownlint-enable MD013 -->

The excerpt keeps the resolution lines, the banners, the `--root=` case and
the verdict; the other eight case lines read `PASS` with the exit codes in the
table above.

The same gate was then run with the systemd 261.3 `systemd-repart` and
`systemd-sysupdate`, extracted from the cached `systemd-261.3-1` package into a
scratch directory and put first on `PATH` with `LD_LIBRARY_PATH` pointing at
that package's `libsystemd-shared-261.3-1.so`. It printed both tools as
`(from PATH)` with the banner `systemd 261 (261.3-1-arch)`, printed `systemctl`'s
`systemd 262 (262-1-arch)` as the cross-check, passed all nine cases and ended
the `--root=` case on its recorded exit 1. So the restaged case holds on both
releases.

The first run of that comparison predates the per-tool floor. The gate then
read the floor from `systemctl --version` alone, so it admitted the 261.3 tools
on the 262 banner and printed that banner, which named a systemd that had not
run the cases. The floor now reads each tool's own `--version`. A stand-in
`systemd-sysupdate` first on `PATH` that prints `systemd 255
(255.4-1ubuntu8.17)` is refused on this host with
`SKIP: systemd-sysupdate at <path> reports 'systemd 255 (255.4-1ubuntu8.17)',
below the admitted floor systemd 261`, although `systemctl` reports 262.

## Five findings worth writing down

### `--dry-run=yes` is inert under `--empty=create`

The gate passes `--dry-run=yes`, the spelling M03's exit criteria name, and
systemd 261 ignores it. `systemd-repart(8)` says the opposite:

> Takes a boolean. If this switch is not specified, `--dry-run=yes` is the
> implied default. Controls whether systemd-repart executes the requested
> re-partition operations or whether it should only show what it would do.
> Unless `--dry-run=no` is specified systemd-repart will not actually touch
> the device's partition table.

Running the gate's own argv by hand exits 0 and ends on:

```text
Successfully formatted future partition 0.
Syncing future partition 0 contents to disk.
Adding new partition 0 to partition table.
Adding new partition 1 to partition table.
Adding new partition 2 to partition table.
Adding new partition 3 to partition table.
Writing new partition table.
Partition table written.
```

Earlier in the same run systemd reports formatting an erofs filesystem for
`10-root-a.conf` and a vfat one labelled `ESP`. `sfdisk -l` on the result
reports `Disklabel type: gpt` and four partitions -- 1G EFI System, two 6.2G
Linux root (x86-64) and one 6.2G Linux root verity (x86-64) -- and `du -h`
reports 2.2M real against 32G apparent. The identical command with
`--dry-run=no` produces the same table.

So this gate does not perform a dry run. It writes a real GPT, formats a vfat
ESP and an erofs root with its verity hash pair, into a sparse image inside a
`TemporaryDirectory` that is removed when the gate exits. Nothing is published,
signed, written to a device or booted.

This is the same class of divergence as the `Unknown key` rows above, a flag
systemd accepts and then does not honour, and it is the reason the flag was
kept rather than quietly dropped: `repart_argv()` in
`tools/verify_systemd_definitions.py` carries an inline comment saying it is
inert, so the next reader of that argv is not misled the way this page was.

### `Encrypt=tpm2` cannot be exercised by an unprivileged gate

Running the reviewed set without `--defer-partitions=var` reaches the encryption
step and stops. Under `LC_ALL=C` the tail is, verbatim apart from a trailing
space on the `ERROR:tcti` line that this repository's `trim_trailing_whitespace`
policy removes:

<!-- markdownlint-disable MD013 -->
```text
Successfully formatted future partition 4.
Encrypting future partition 4...
ERROR:tcti:src/tss2-tcti/tcti-device.c:421:Tss2_Tcti_Device_Init() Failed to open specified TCTI device file /dev/tpmrm0: Permission denied
Failed to create TPM2 context: State not recoverable
Failed to encrypt device: State not recoverable
```
<!-- markdownlint-enable MD013 -->

Exit 1. `/dev/tpmrm0` is `crw-rw---- root tss` and the gate did not run as
root or in `tss` when this was observed. The gate therefore passes
`--defer-partitions=var`, which is systemd's own mechanism for a partition that
is created in the table now and populated later. That is also the truthful
sequence: TPM2 sealing binds to the target machine's PCR state, so it belongs to
first boot on the target, not to the image build. Exercising the enrolment
itself is M20 work on swtpm.

Since then the developer account has joined `tss` (re-read 2026-09-28,
`planning/hardware-profile.json`), so the permission half of this finding no
longer holds on the reference profile. The un-deferred run was not repeated, and
the deferral stays: it rests on the first-boot sequence, which does not depend
on group membership.

### `systemd-sysupdate --root=` reads no transfer on systemd 261

M03's exit criterion 5 first named `systemd-sysupdate --root=<tree> --offline
list`. On this build that invocation reads no transfer: it exits 1 on
`10-root.transfer:1: Source Type= must be one of url-file, url-tar, tar,
regular-file, directory, subvolume.` even though `Type=url-file` is plainly
assigned, so the criterion is not met as it was written.

No mechanism is claimed. No systemd source and no upstream bug report was read
here, so what follows is what happens, not why. The falsifying experiment: put
a file that is not a config at all at
`<tree>/usr/lib/sysupdate.d/10-root.transfer`, put a valid copy at
`<tree><tree>/usr/lib/sysupdate.d/10-root.transfer`, and the same command exits
0 and lists the slots. The file under the tree is never read.

Because the named invocation cannot prove the parse, the authoritative proof is
the `--definitions=<tree>/usr/lib/sysupdate.d` form, which exits 0 with
`{"current":"b","all":["b","a"],"appstreamUrls":[]}`. At M03 it read the same
scratch tree's directory; since 2026-09-28 the two cases read separate staged
trees, `reviewed` for `--definitions=` and `sysroot` for `--root=`, both staged
from the same reviewed transfer (see the systemd 262 finding below). The
substitution is recorded in M03's evidence in `planning/roadmap.json` and the
criterion text there has been amended to describe what is actually proven.

The gate still keeps the named invocation as a case and accepts exactly two
outcomes: exit 0, or exit 1 with that diagnostic. A third outcome fails the
gate, so a systemd release that fixes this is a visible diff rather than a
silent change.

### systemd 262 reads `--root=` as a root, which the case was not staged for

systemd 262 produced that visible diff. It reads the transfer through `--root=`
and reports it relative to the tree (`usr/lib/sysupdate.d/10-root.transfer:48`
where 261 printed the root-prefixed absolute path), and it then resolves the
transfer's `Path=` inside the tree as well. The case had pointed `Path=` at the
scratch image outside the tree, so 262 exited 1 on `Failed to resolve ...: No
such file or directory`. Mirroring the image inside the tree was enough for exit
0 with `{"current":"b","all":["b","a"],"appstreamUrls":[]}`.

A second diagnostic in the same run was a silent drop of the kind REQ-CI-01 is
about: `10-root.transfer:48: Failed to expand specifiers in ProtectVersion=,
ignoring: %A`. Under `--root=`, `SYSTEMD_OS_RELEASE` pointing at a file outside
the tree left `%A` unexpanded, with the tree's own `usr/lib/os-release` present
or not. Without the variable, the tree's own `os-release` expanded it and the
line was gone. The `--definitions=` cases, which run without `--root=`, never
printed it.

So `stage_root_tree()` in `tools/verify_systemd_definitions.py` now builds a
self-contained tree: the repart gate writes its scratch image into it, the
transfer's `Path=` names that image as the tree does (`/positive.raw`), and the
identity is the tree's own `usr/lib/os-release`, with no `SYSTEMD_OS_RELEASE`.
Its exit-0 outcome forbids `Failed to expand specifiers` as well as
`Unknown key`, and must list both slots, as the positive case must; the positive
case forbids the specifier diagnostic too. Its exit-1 outcome is unchanged. No
systemd source or upstream report was read, so this records behaviour, not its
cause.

### `Subvolumes=` exists, but not in the imported spelling

The imported `/var` definition carried `BtrfsSubvolumes=@var @var-log @flatpak
@containers @pglite`, which systemd 261 ignores. `repart.d(5)` does define a
`Subvolumes=` key, but it takes absolute paths inside the new file system rather
than `@name` subvolume names. Translating the five names into mount paths is a
storage-layout decision, so REQ-P02-03's subvolume list stays open rather than
being guessed into a supported key.

## The Rust half

`crates/aegis-fabrica-defs` (D15) parses the same files without systemd. It
answers two questions and keeps them apart: `DefinitionError` for what systemd
refuses, `Finding` for what systemd accepts while the recorded requirement does
not. It is deliberately stricter in one direction -- an unknown key, a repeated
section and a repeated key are refusals, not warnings -- and it reports the
single-slot transfer that systemd silently bumps.

The two halves can disagree, which is the point: the gate runs the tool, the
crate runs the rules, and a divergence is a failure in one of them rather than a
quiet agreement.

## Acceptance target for PCR measurement (E03-4, not claimed here)

REQ-P01-05 records the UKI chain of trust as a TPM2 measurement strategy:

| PCR | What it measures |
| :--- | :--- |
| 0 | UEFI firmware |
| 4 | the UKI binary |
| 7 | Secure Boot policy |
| 11 | the dedicated UKI verification slot |

This is written down here as the **acceptance target for M11 boot evidence**,
not as a claim. Nothing in M03 reads a PCR, signs a roothash or boots anything.
The `/var` enrolment that binds to PCR 0, 4, 7 and 11 (REQ-P02-02) is M20 work
on swtpm, and the UKI measurement itself needs a real artefact from M11.

## Scope limit

The definitions parse, the gate runs, and the parser agrees with it on the
reference profile. What the gate writes is one throwaway sparse scratch image
inside a temporary directory that is removed when it exits, carrying a real GPT,
a formatted vfat ESP and an erofs root with its verity hash pair. Beyond that
image nothing is published, no artefact is signed, no partition is written to a
real device, no TPM is enrolled and nothing is booted. `make build`, `make boot`
and `make release` remain blocked, and M11, M20 and M24 hold the evidence this
page does not.
