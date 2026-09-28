<!--
SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
SPDX-License-Identifier: CC-BY-SA-4.0
-->

# The boot harness, and what a booted upstream image told it

Status: recorded observations from milestone M24, reference profile, 2026-09-28

Decision D72 gives milestone M24 the boot harness and the real boot evidence,
over whatever bootable artifact is supplied: a pinned upstream distribution
image now, an Imago return at M11. `make verify-boot` verifies the artifact
against its signed checksum, boots it headless under QEMU with KVM, OVMF with
Secure Boot and a swtpm TPM, and makes the guest read PCR 0, 4, 7 and 11 back
from inside itself. Decision D84 narrowed epic E24-2 to what a pinned upstream
image can prove, and recorded the four implementation choices this page follows.

**M24 is `done`, for the upstream kind of artifact it proves (D85).**
Verifying an Imago return's signature before boot moved to M11, which adds that
pin scheme once the signature form of the Imago result is pinned; under D92 that
form is pinned in M11 with the result itself, not in M09. The section
[What moved to M11](#what-moved-to-m11) says why.

**Aegis constructs no image here, and nothing here is release evidence.** The
booted bytes are Fedora's, hashed against the pin before every boot and after
the last one. No host firmware variable was written, nothing was installed, and
nothing was published. A pass is development evidence on one workstation: it
closes no image, boot, hardware or release gate, and it is not evidence for M11.

## What is tracked, and what is not

| Path | Role |
| :--- | :--- |
| `build/boot/artifact.pin.json` | the pin: compose URL, sha256, size, producer version and floor, the signed CHECKSUM and its key fingerprint, the recorded timeout |
| `tools/guest/aegis-boot-report.sh` | the in-guest read-back, handed to the guest as NoCloud user-data |
| `tools/verify_boot_harness.py` | the gate, run as `make verify-boot` |
| `tools/test_boot_harness.py` | the half of it that runs inside `make verify-all` |

Not tracked: the 630 MB artifact, its CHECKSUM, the signing keyring and every
run's logs. They live under `AEGIS_BOOT_HARNESS_DIR`, default
`${XDG_CACHE_HOME:-$HOME/.cache}/aegis-boot-harness`; `*.qcow2` is ignored by
`.gitignore`. `python3 tools/verify_boot_harness.py --fetch` fills the cache.
Each run keeps its console logs, its guest reports, the two text files of each
seed and a `result.json` per boot under `runs/`. When it ends, including on
SIGINT, SIGTERM, SIGHUP or SIGQUIT, it deletes the overlays, variable stores,
TPM state, seed images, monitor sockets, the private key and the UKI copies. A
SIGKILL runs no clean-up, so those files stay until the run is pruned. The
newest sixteen runs are kept.

## The artifact, and how it is pinned

The pin names `Fedora-Cloud-Base-UEFI-UKI-44-1.7.x86_64.qcow2` at its compose
URL under `/releases/44/`, not a moving path. D84 chose it over Ubuntu 26.04
because it boots a systemd-stub UKI, which exercises PCR 11 and the signing
criterion; Ubuntu's image boots shim and GRUB. The digest is not trusted
because this page repeats it. The gate verifies Fedora's clearsigned
`Fedora-Cloud-44-1.7-x86_64-CHECKSUM` and requires the `VALIDSIG` status line
to name the pinned primary key; a good signature by any other key is refused.
It then reads the digest and the producer version out of the **signed** text
only, which gpg writes to a separate file, so a line outside the signature
cannot supply either. From the recorded run, with two long lines wrapped and
two values shortened:

```text
PASS boot/artifact-verified
     gpg status: GOODSIG DBFCF71C6D9F90A6 Fedora (44)
       <fedora-44-primary@fedoraproject.org>
     signing key: 36F612DCF27F7D1A48A835E4DBFCF71C6D9F90A6 (pinned)
     signed record: SHA256 (Fedora-Cloud-Base-UEFI-UKI-44-1.7.x86_64.qcow2)
       = 2b0af3e6bf4add3695e52df5db3b2eb48d081ab38963ae48c35fca45d5c31b64
     producer version 44-1.7 against floor 44-1.7: admitted
     cached bytes: sha256 2b0af3e6...5c31b64, 630784000 bytes: match the pin
```

The key came from `https://fedoraproject.org/fedora.gpg`, which carries the
Fedora 43 to 46 keys; `https://fedoraproject.org/security/` lists
`36F6 12DC F27F 7D1A 48A8 35E4 DBFC F71C 6D9F 90A6` for Fedora 44, read on
2026-09-28. gpg reports the key as uncertified because no local trust path
exists; what the pin asserts is the fingerprint.

The producer version is `44-1.7`, ordered as `(44, 1, 7)`, and the pin's floor
is the same value. A refresh to a later compose is a reviewed pin bump (D69),
and a compose below the floor is refused whatever its signature says.

## How one boot runs

Every Secure Boot guest is
`qemu-system-x86_64 -machine q35,smm=on,accel=kvm -cpu host`; the contrast guest
on `OVMF_CODE.4m.fd` uses `q35,smm=off,accel=kvm`. Each has 2 GiB and two vCPUs,
`-nodefaults -display none -no-reboot -nic none` and
`-run-with exit-with-parent=on`.
`accel=kvm` has no TCG fallback, so a host without a read-write `/dev/kvm` skips
the gate with that reason instead of booting a different machine, and QMP's
`query-kvm` answers `enabled: true` in the evidence of every guest that reaches
its prompt. Before each boot the harness hashes the pinned file and refuses it
on a mismatch, before it runs a program or writes a file for that boot. Then it
writes, in the boot's own directory:

- a NoCloud `cidata` seed with xorriso, holding the instance id (a fresh nonce)
  and `tools/guest/aegis-boot-report.sh` with that nonce in its one placeholder;
- a guest variable store generated by virt-fw-vars from the shipped
  `OVMF_VARS.4m.fd`: this run's certificate as PK and KEK, Microsoft's
  third-party UEFI CAs of 2011 and 2023 in db, a placeholder dbx,
  `SecureBootEnable` on, and shim's fallback told not to reset
  (`FB_NO_REBOOT`), so one boot is one measurement chain;
- a TPM state manufactured by `swtpm_setup --createek --pcr-banks sha256`;
- after all three, the sha256 of the pinned file a second time, and then a
  qcow2 overlay whose read-only backing file is that same path. This second
  hash is the one taken immediately before the boot.

swtpm then starts with `--terminate`, QEMU starts on `OVMF_CODE.secboot.4m.fd`
with the overlay and the seed, and the harness polls the first serial line for a
login prompt. Both processes lead their own session and are ended with it on
every way out the harness can catch: a normal end, a missed timeout, an
exception, SIGINT, SIGTERM, SIGHUP and SIGQUIT. A SIGKILL cannot be caught, so
QEMU's `exit-with-parent=on` ends QEMU when the harness dies, and swtpm ends
through `--terminate` when QEMU is gone. Both were checked on this host against
a live `--measure 1` run: after SIGHUP the harness exited 129, and after SIGKILL
it was killed; each time no swtpm or QEMU process was left. gpg runs with
`--no-autostart`, so no gpg-agent outlives the gate.

**Why the seed is configuration, not image construction.** It is an ISO 9660
volume holding two text files: an instance id and a shell script. It is the
transport the NoCloud datasource reads, the same mechanism Imago's own Packer
build uses, and it carries no kernel, no root file system and nothing the guest
boots from. The OS that boots is the pinned image, byte for byte: its digest is
checked before the boot, the guest writes only to the overlay, and the file
hashes the same after every boot. The variable store and the TPM state are
firmware and device state of the virtual machine, not of the image.

## What the guest reported about itself

The report arrives on the second serial line, so a console message cannot
splice into it. From the recorded run (`r20260928T195251-1a11`), with the one
line longer than 80 columns wrapped:

```text
AEGIS-M24-BEGIN
AEGIS-M24-NONCE aegis-m24-9702a8b95c44964bad256d39d35dd9ae
AEGIS-M24-UNAME-R 6.19.10-300.fc44.x86_64
AEGIS-M24-PCR-0 246BB9CEDB66A7DA85235E267E29CF0E1F98A6F2A79D33631E423595ECE181AC
AEGIS-M24-PCR-4 AD79C1CDFF7B610FB694731BE9EE4CA05C9B81916F6A6AD301BC471E1DA4CCC9
AEGIS-M24-PCR-7 1CED24D4BCF8294F882B7546CD01DE48CBE75C246F0C4A4319F9FF3C1AC3B199
AEGIS-M24-PCR-11
  7A15F54DE99214EB54DB1AFA5B04AA79B32359081C6A12229FE3379130CD7B8C
AEGIS-M24-SECUREBOOT 6 0 0 0 1
AEGIS-M24-SETUPMODE 6 0 0 0 0
AEGIS-M24-TPM-MAJOR 2
AEGIS-M24-TPM2-SETUP failed
AEGIS-M24-CMDLINE console=tty0 console=ttyS0
AEGIS-M24-END
```

The nonce is the one the harness generated for that boot and wrote into its
seed; a report carrying any other value is refused, so a stale file or a
reading taken on the host proves nothing. The first serial line of the same
boot, abridged to the lines that matter here:

```text
BdsDxe: starting Boot0002 "UEFI Misc Device" from PciRoot(0x0)/Pci(0x1,0x0)
[    0.000000] Linux version 6.19.10-300.fc44.x86_64 (mockbuild@...) ...
[    0.000000] Command line: console=tty0 console=ttyS0
[    0.000000] secureboot: Secure boot enabled
[    0.778659] tpm_crb MSFT0101:00: Disabling hwrng
Welcome to Fedora Linux 44 (Cloud Edition)!
[   38.966343] cloud-init[885]: Cloud-init v. 25.3 finished at ...
  Datasource DataSourceNoCloud [seed=/dev/vdb].  Up 38.96 seconds
aegis-m24-guest login: [  OK  ] Removed slice system-modprobe.slice ...
[   42.701305] reboot: Power down
```

The harness saw the prompt 47.4 s after QEMU started, on a host whose load
average was 85 when the run began (the timeout section below has the margins),
then asked QMP for `query-kvm` and `system_powerdown`; the shutdown messages
follow the prompt on the same console line, and QEMU exited 0. Every PCR the
guest read is annotated by what it is, not only by its value:

| PCR | Owner (UAPI PCR registry) | Reading | What it records here |
| :--- | :--- | :--- | :--- |
| 0 | firmware code | extended | the OVMF image booted; it differs between the two firmware builds |
| 4 | boot loader and binaries it loads | extended | the boot applications of shim's path to the UKI; the event log was not read, so which binaries is not itemised; the same on both firmware builds |
| 7 | Secure Boot policy | extended | the guest firmware's state: Secure Boot on, this run's PK and KEK, the db; it changes every run because the key does |
| 11 | kernel boot: the UKI's sections, then boot phases | extended | systemd-stub and systemd-pcrphase measured it; the reading does not separate the two |

A reading equal to the reset value would be printed as
`reset (never extended)`; none was. The reset value was read once by hand, not
by the gate: a fresh swtpm 0.10.2 manufactured the same way and started with
`--flags startup-clear` answers `tpm2_pcrread sha256:0,4,7,11,16,17` with 64
zeros for PCR 0 to 16 and 64 `F`s for PCR 17. PCR 11 in particular is
**extended**, which D84 asked to be confirmed on a real boot rather than
assumed: the same value, `7a15f54d...30cd7b8c`, was read on both firmware
builds and in every run, which fits a measurement of the UKI and the boot
phases rather than of the firmware.

**The host is recorded, not tested.** Its efivarfs reads `SecureBoot 6 0 0 0 0`
and `SetupMode 6 0 0 0 1`, with no PK, KEK, db or dbx present: Secure Boot is
disabled and the firmware sits in setup mode, which matches
`planning/hardware-profile.json`. So PCR 7 above documents the **guest**
firmware's state; it attests no trusted chain on this host (D62). The host's own
PCR 0 reads `1b2dd7c5...58e8ae18`, which the case requires to differ from the
guest's.

## The cases, and what each proves

| Case | What it does | Outcome on the reference profile |
| :--- | :--- | :--- |
| `boot/host-secure-boot` | reads SecureBoot, SetupMode and the key variables from efivarfs | Secure Boot disabled, setup mode, no keys; nothing written |
| `boot/artifact-verified` | CHECKSUM signature by the pinned key, the signed digest and version against the pin, the cached bytes | all four match; version `44-1.7` at the floor |
| `boot/artifact-tampered-refused` | a copy with one flipped bit, fed to the verification and to the boot entry | both refuse at the digest; the boot entry runs no program and writes no file: no seed, store, TPM state, overlay, swtpm or QEMU |
| `boot/checksum-bad-signature-refused` | one hex digit changed inside the signed CHECKSUM | gpg reports `BADSIG`; refused at the signature, before any digest check |
| `boot/producer-version-boundary` | the floor rule at `44-1.7` and at `44-1.6` | admitted, refused; rule evaluations, not downloads |
| `boot/positive` | Secure Boot firmware, swtpm, the login prompt within 180 s, the PCR report, a clean power-off | prompt at 47.4 s at load average 85 (about 15.5 s on a quiet host), four PCRs extended, exit status 0, KVM enabled |
| `boot/firmware-contrast` | the same bytes on `OVMF_CODE.4m.fd`, without Secure Boot | SecureBoot variable absent, PCR 0 and PCR 7 differ, PCR 4 and 11 equal |
| `boot/login-timeout-negative` | the same boot with a recorded timeout of 5 s | `missed` with neither a prompt nor an exit by the deadline; the harness stops QEMU, exit status -9, mid kernel start-up; a QEMU that exited on its own would fail the case |
| `boot/timeout-boundary` | the timeout rule at 179, 180 and 181 s | reached, reached, missed: two outcomes, inclusive at the edge |
| `boot/uki-signed` | the artifact's UKI signed with this run's key and booted on a store holding only that key | signed copy boots (kernel reports Secure Boot enabled); unsigned copy refused by the firmware |
| `boot/pinned-bytes-unchanged` | the cached artifact hashed after every boot | `2b0af3e6...5c31b64`, mode `0444` |

The recorded run, `r20260928T195251-1a11`, on 2026-09-28, took 80.7 s of wall
clock and printed eleven case lines, all `PASS`, and the closing line; the
detail lines under each are abridged here:

```text
PASS boot/host-secure-boot
PASS boot/artifact-verified
PASS boot/artifact-tampered-refused
PASS boot/checksum-bad-signature-refused
PASS boot/producer-version-boundary
PASS boot/positive
     login prompt: 47.4 s against the recorded 180 s -> reached
PASS boot/firmware-contrast
     login prompt: 15.5 s against the recorded 180 s -> reached
PASS boot/login-timeout-negative
PASS boot/timeout-boundary
     recorded timeout 180 s, inclusive; these are rule evaluations, not boots
     prompt after 179 s -> reached
     prompt after 180 s -> reached
     prompt after 181 s -> missed
PASS boot/uki-signed
PASS boot/pinned-bytes-unchanged
PASS: boot harness on the reference profile (M24, D62, D72, D84). The pinned
  artifact booted under KVM with OVMF and swtpm; this is development evidence
  only. It closes no image, boot, hardware or release gate and is not evidence
  for M11.
```

After it, `pgrep -a 'swtpm|qemu-system'` found nothing and no socket was left
under `runs/`. The two negatives, from the same run:

```text
PASS boot/artifact-tampered-refused
     verification: refused at digest: Fedora-Cloud-Base-UEFI-UKI-44-1.7
       .x86_64.qcow2 hashes to 64f40e32...431445e6, not 2b0af3e6...5c31b64
     boot entry: refused at digest: (the same digest pair)
     files the boot entry wrote for the tampered copy: none (no seed, store,
       TPM state or overlay); swtpm or QEMU sessions started: 0
PASS boot/login-timeout-negative
     recorded timeout for this run: 5 s (deliberately short)
     harness decision: missed; seen by the deadline: neither a prompt nor an
       exit
     QEMU exit status after the harness ended its session: -9
     last console line when stopped: [    0.898664] ata2: SATA link down
       (SStatus 0 SControl 300)
```

## The recorded timeout, and the margin it keeps

"Reaches the login prompt within the recorded timeout" is inclusive: a prompt
seen at exactly the timeout passes, one second later fails. The boundary is
checked as a rule on synthetic times, 179, 180 and 181 s, never on a live boot:
host load moves a live boot by seconds, and D73 recorded what happens to a gate
that decides on a live edge. Live boots measure the margin instead. The recorded
timeout is 180 s, in the pin; it was 120 s until the maintainer raised it on
2026-09-28 (recorded with D84 in `docs/roadmap/README.md`) for the margins
below. `--measure N` re-derives the figures:

| Series (2026-09-28) | Host load | Boots | Prompt seen after | Under 120 s | Under 180 s |
| :--- | :--- | ---: | :--- | ---: | ---: |
| `--measure 10` | load average 11 | 10 | min 15.3 s, median 15.5 s, max 15.8 s | 104.2 s | 164.2 s |
| `--measure 3` with all 32 threads busy (`yes` x 32) | load average 35 | 3 | min 26.9 s, median 27.4 s, max 27.6 s | 92.4 s | 152.4 s |
| `--measure 3` | load average 14 to 18 | 3 | min 16.3 s, median 16.3 s, max 16.5 s | 103.5 s | 163.5 s |
| `make verify-boot`, run `r20260928T192850-2cf7` | load average 13 | 2 | 15.5 s (Secure Boot), 15.0 s (contrast) | 104.5 s | 164.5 s |
| `make verify-boot`, run `r20260928T193744-9c81`, with other work on the host (a local model server at about 700 % CPU and other sessions' test runs) | load average 89 at the start | 2 | 65.7 s (Secure Boot), 17.0 s (contrast) | 54.3 s | 114.3 s |
| `make verify-boot`, the recorded run `r20260928T195251-1a11` of the 180 s revision, with other sessions' work on the host | load average 85 at the start, 47 at the end | 2 | 47.4 s (Secure Boot), 15.5 s (contrast) | 72.6 s | 132.6 s |
| `make verify-boot`, run `r20260928T201443-e93e`, the same gate after the rebase onto `main` at `d3f5c3a`, which changes none of its files | load average 10 at the start | 2 | 15.3 s (Secure Boot), 14.3 s (contrast) | 104.7 s | 164.7 s |

Each margin is taken from the slowest boot of its series. Thirty-two `yes`
processes added about twelve seconds. Other work that drove the load average to
89 delayed one prompt to 65.7 s, the slowest boot measured, which left 54.3 s
under the 120 s recorded then: less than half the timeout, against a prompt at
about 15.5 s on a quiet host. That is why the recorded timeout is now 180 s,
under which the slowest boot measured keeps 114.3 s. The recorded run of the
180 s revision began at load average 85, again from other sessions' work, and
saw the Secure Boot prompt after 47.4 s, 132.6 s inside the timeout. The margin
is still load-dependent: a host loaded further than anything measured here
could miss a prompt that a quiet host reaches in sixteen seconds. That is
recorded rather than tuned away.

A live figure is the time the harness **saw** the prompt, polled every 0.25 s,
so it can be up to one poll late; the decision rule is applied to that figure.
The negative case's 5 s is below the firmware and kernel start-up alone.

## Guest Secure Boot, and the signed UKI

The positive boot shows the distribution's own chain verified by the guest
firmware: shim is signed by Microsoft's third-party UEFI CA, which the store
carries in db, and shim verifies Fedora's UKI. The store's PK and KEK are the
run's own certificate, so nothing in the guest's key hierarchy is Microsoft's or
Fedora's except the db entries the image needs.

`boot/uki-signed` exercises the signing path criterion 9 names. It reads the
pinned image's GPT with `qemu-img dd` into a file, copies the MiB-aligned ESP
out the same way, and takes `EFI/Linux/6.19.10-300.fc44.x86_64.efi` out of that
copy with mcopy; the pinned image is read and never written. sbsign adds this
run's signature beside Fedora's, and sbverify with this run's certificate
accepts the signed copy and refuses the original:

```text
PASS boot/uki-signed
     UKI copied from the artifact's ESP: EFI/Linux/6.19.10-300.fc44.x86_64.efi,
       sha256 e6c5d0104346f168a3f81bb578368281672d6c7d6a46de36d2006b12a8ab9c33
     sbverify --cert <run key> signed: exit 0 (['Signature verification OK'])
     sbverify --cert <run key> unsigned: exit 1 (['Signature verification
       failed'])
     custom-only store, signed UKI: kernel started with Secure Boot enabled
       after 1.5 s
     custom-only store, same UKI unsigned: firmware refused the image (Access
       Denied) after 1.0 s
```

Both boots use a second store whose PK, KEK and db are this run's certificate
alone, and a directory that QEMU presents to the guest as a FAT disk, holding
the UKI as `EFI/BOOT/BOOTX64.EFI`; no disk image is written for them. The
unsigned boot ends at the firmware:

```text
BdsDxe: failed to load Boot0002 "UEFI Misc Device" from PciRoot(0x0)/Pci(0x1,
  0x0): Access Denied -- rejected probably by Secure Boot
```

The signed boot is stopped once the kernel reports
`secureboot: Secure boot enabled`: its root file system is not on that disk, so
it would not reach a prompt, and the case needs only the firmware's acceptance.
ukify 262 and erofs-utils 1.9.4 are installed, as criterion 8 records, and this
milestone runs neither: it signs the UKI the image ships rather than building
one, and builds no EROFS root.

## Observations for the milestones that follow

- The guest's `systemd-tpm2-setup.service` fails on every boot, on both firmware
  builds, with a TPM manufactured by `swtpm_setup` and with one that was not.
  The cause was not read: this harness has no shell in the guest. M20, whose
  unseal needs an SRK, meets this first.
- M24 does not pre-prove D63's tpm2-tools quote path; it reads PCRs through
  sysfs. The criterion of M20 that records that pre-proof is still open (D84).
- The A/B transition trace of M15 is not compared here: a pinned upstream image
  has no A/B slots. Under D72 that comparison is M11's E11-4, on this harness.

## What moved to M11

Criterion 2 asked for a harness "parameterised over an externally supplied
bootable artifact: a pinned upstream distribution image or an Imago return",
and E24-1's positive half asked that the artifact's digest and signature verify
before boot "whether it is a pinned upstream distribution image or an Imago
return". This harness proves that for the upstream kind only:

- The pin schema, `aegis.m24.boot-artifact-pin.v1`, implements one signature
  scheme, `gpg-clearsigned-checksum`, a producer version of the form
  `MAJOR-MINOR.RESPIN` and Fedora's `SHA256 (<file>) = <digest>` CHECKSUM line.
  `load_pin` refuses any other scheme, and a unit test holds that refusal.
- The Imago return's signature form is not pinned. Imago's proposed result
  schema, `imago.p01.product-result.v1` (its ADR-0020), carries an
  `image-digest` and a `signature-ref` string. It was to be agreed in M09;
  under D92 (2026-09-28) M09 closed on the consumption contract and the result
  moved to M11, because nothing produces a result yet (cordanaLLM/imago#46). A
  scheme added here would guess a contract nobody has pinned.

Decision D85 (2026-09-28) closes M24 on the upstream kind. Criterion 2 and
E24-1 are restated for it, and verifying an Imago return's signature before
boot moved to M11: an exit criterion and the acceptance of E11-1, whose
requirements are E24-1's. M11 adds the pin scheme for the Imago result's
signature form once that form is pinned, in M11 itself since D92, with its own
positive, negative and boundary
cases, and the `gpg-clearsigned-checksum` pin recorded here must keep verifying
unchanged. Adding a scheme changes the harness, which M11's E11-2 and sixth
criterion forbade; both now except that one scheme and still fail M11 on any
other change. Both milestones disclose their edits in `planning/roadmap.json`.

## Scope, and what a pass here does not mean

- A pass is development evidence on the reference profile, one workstation. It
  closes no image, boot, hardware or release gate, and it is not evidence for
  M11: M11 needs its own artifact through this harness, changed only by the pin
  scheme D85 moves there.
- The guest's Secure Boot is the guest's. The host firmware stays in setup mode
  with Secure Boot off; nothing here enrols a key on it or writes a variable.
- PCR values from swtpm prove the measurement path, not a hardware root of
  trust.
- P01 and P02 stay proposals in `planning/components.json`: the harness is a
  development gate, not the component daemon of either.
- `make verify-boot` is not part of `make verify-all`. The CI runner has no
  `/dev/kvm`, and a boot without KVM would be a different claim, so it could
  only skip there; the reasoning is on the target in the `Makefile`.
  `tools/test_boot_harness.py` runs inside `make verify-all` and holds the pin,
  the floor, the timeout rule, the report parser, the argument vectors, the
  admission and the programs the gate may start.
- A host that cannot run the gate prints `SKIP: <reason>; the boot harness gate
  did not run.` and exits 0, so an exit 0 is evidence only when the case lines
  are above it. A failed or mismatched download is a `FAIL`, not a skip.
