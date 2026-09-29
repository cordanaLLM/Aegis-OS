# Reference development profile

Status: measured planning data

## Purpose and limits

This page records the capabilities of the reference development machine and what
each roadmap milestone can therefore verify locally. It exists because a
milestone that needs hardware is not blocked when the hardware is on the
developer's desk.

A check that passes on this profile is development evidence. It does not qualify
hardware, does not close a blocked gate and is not release evidence. Hardware
qualification needs more than one machine and the acceptance recorded in the
milestone itself. The machine-readable record is
`planning/hardware-profile.json`, validated by `verify_hardware_profile()`
inside `make verify-all`; every capability there carries the command that
evidences it.

## Capabilities

| Capability | Present | Note |
| :--- | :--- | :--- |
| `kvm` | yes | |
| `tpm2` | yes | discrete TPM 2.0; swtpm also installed for virtual machines; re-read 2026-09-28: the developer account is in group tss and an unprivileged `tpm2_pcrread sha256:7` exits 0, where on 2026-09-13 it ran privileged |
| `iommu` | yes | AMD-Vi (ivhd0); VFIO is available as a module |
| `rapl_energy_counters` | yes | package counters exist; /sys/class/powercap/intel-rapl:0/energy_uj is mode 0400 and root-readable only, so P13 telemetry needs a privileged daemon or a root test |
| `sched_ext` | yes | the host kernel runs a sched_ext scheduler at probe time; scx_cake, the scheduler P07 proposes, is installed as a distribution package alongside scx_rusty, scx_bpfland and scx_lavd |
| `bpf_lsm` | yes | bpf appears in the active LSM list |
| `btf` | yes | CO-RE eBPF programs can be compiled against the host kernel |
| `pci_p2pdma` | yes | kernel support is compiled in; device-level support still needs proving per device |
| `erofs_dm_verity` | yes | both available as modules |
| `realtime_kernel` | no | the running kernel is PREEMPT_DYNAMIC, but the distribution ships linux-rt 7.2.8.rt3.arch1-1 (7.2.5.rt3.arch1-1 on 2026-09-13); this is an operator action, not a missing capability |
| `secure_boot_enrolment` | no | disabled in the running firmware and not enrolled on the host; re-read 2026-09-28: the firmware is in setup mode (SetupMode 1) with no PK, KEK, db or dbx, which the maintainer reports clearing before a firmware update (no exact date given), where on 2026-09-13 it carried vendor keys with SetupMode 0; guest enrolment is proven with a virt-fw-vars variable store and OVMF_CODE.secboot (D62); host enrolment is an operator action, not a missing capability |
| `resizable_bar` | yes | already enabled in firmware: 32 GiB BAR1 on the discrete NVIDIA card and an 8 GiB prefetchable region on the Intel card, which is the firmware prerequisite for peer DMA |
| `gpu_dma_buf_and_peer_memory` | yes | NVIDIA 615.71.09 open kernel modules (Dual MIT/GPL) with nvidia_drm modeset and the nvidia_peermem peer-memory module installed, CUDA 13.4.2 present (13.3.1 on 2026-09-13); the Intel and AMD cards provide DMA-BUF through their in-tree drivers |
| `vaapi_decode` | yes | recorded 2026-09-28: with LIBVA_DRIVER_NAME=iHD, vainfo on the Arc A380 (renderD129) reports iHD 26.2.4 with VLD for MPEG-2, JPEG, H.264, HEVC, VP9 and AV1; the session sets LIBVA_DRIVER_NAME=nvidia, and with that session value the same call on renderD129 loads the NVIDIA driver (with the variable unset, libva selects iHD), so every run pins the driver; no committed gate has decoded a frame yet (M27) |
| `wayland_layer_shell` | yes | recorded 2026-09-28: the host session's KDE KWin 6.7.5 advertises zwlr_layer_shell_v1 version 5; it is not P04, so a run against it is the client half only (D79) |
| `compositor_dmabuf_import` | yes | recorded 2026-09-28: KWin advertises zwp_linux_dmabuf_v1 version 5 with main device renderD129 (the Arc A380) and NV12 and P010, each with LINEAR, INTEL_X_TILED, INTEL_4_TILED and DRM_FORMAT_MOD_INVALID (implicit modifier), and neither with INTEL_4_TILED_DG2_RC_CCS or INTEL_4_TILED_DG2_RC_CCS_CC; no committed gate exercises it yet (M27 will) |

The two capabilities recorded as absent are operator actions, not missing
hardware: host Secure Boot needs keys enrolled in the firmware and the setting
enabled, and the distribution ships a realtime kernel package. Both are listed
under operator actions in the machine-readable record together with the packages
that the remaining milestones need.

## What each milestone can verify here

| Milestone | Local support | What becomes possible | What still cannot be shown |
| :--- | :--- | :--- | :--- |
| M09 | partial | A positive, a negative (rejected payload with correlated error) and a boundary (empty requirement list rejected explicitly) result can all be produced against the two local checkou | Acceptance is recorded only when each producer consumes the payload |
| M10 | partial | The mechanics are fully local: booting an arbitrary kernel under QEMU/KVM with a minimal initramfs and repeating the three verifier loads needs nothing this machine lacks | The kernel under test |
| M11 | partial | Everything except the signature half | Firmware-verified Secure Boot of the UKI |
| M12 | partial | Two of the three paths become local work | The P2PDMA path, honestly and completely |
| M13 | partial | The cheapest_exit half is local and read-only: record hosted ruleset and label readback for the existing origin, keep the release workflow inactive | Release evidence |
| M19 | full | ANSWER TO Q1 AND Q2: yes to both | Two honest limits |
| M20 | partial | Once an M11 image exists, the full slice runs here: a real swtpm PCR quote signing one M14 audit record, and /var unsealing under an enrolled PCR policy | Attestation rooted in a firmware-verified boot chain, for the PCR 7 reason above |
| M21 | full | Measured RAPL energy deltas can replace the simulated wattage in the M05 engine today, and an AF_VSOCK candidate evaluation can round-trip | ACCURACY CAVEAT, and it is a real one |
| M27 | full | VA-API decode on the Arc A380, the DMA-BUF passed over SCM_RIGHTS, and the host compositor importing and presenting it on a layer surface | That P04 serves either protocol: the compositor is KDE KWin, so a pass is the client half only (D79) |
| M28 | full | A gpui layer surface on the host compositor, which advertises zwlr_layer_shell_v1 version 5; its AccessKit tree read back over AT-SPI on a private session bus (at-spi2-core 2.60.7 is installed, read 2026-09-29); and the 1,000-node frame and tree-update times measured on this GPU and driver | That P04 serves layer shell, since the compositor is KDE KWin and a pass is the client half only (D79), or any frame-time figure for another GPU, driver or compositor |
| M29 | full | A pinned distribution guest booted under QEMU with KVM, running the M28 shell with a portal, an at-spi2 registry and a StatusNotifierWatcher on its session bus | That the Aegis image or P04 serves that session: the guest is neither |

Milestones not listed need nothing from the profile: they are Rust, schema or
documentation work that runs on any developer machine.

## Toolchain gaps

The tools the roadmap needs are installed on the reference profile as of
2026-09-13: mkosi 27, Firecracker 1.17.0 with its jailer, sbsigntools 0.9.5,
tpm2-tools 5.8, erofs-utils 1.9.4, ukify 261 and virt-firmware 26.9, beside the
QEMU 11.1.1, swtpm 0.10.2, clang, bpftool, Rust 1.98.1, Node, pnpm and cosign
that were already present. Each is still admitted through the template matrix
with a pinned version before the gate that uses it runs; installation is not
admission.

Two remain deliberately absent, and a third changed: `rustup` 1.29.1 replaced
the distribution Rust package on 2026-09-13 and materialises the pinned
toolchain (D61).

- `sbctl` is an alternative to virt-firmware for the OVMF variable store, which
  is already installed.
- `nvidia-fs` would not make GPUDirect Storage demonstrable, because that path
  is gated to datacentre cards.

Re-read on 2026-09-28: `systemctl`, `systemd-repart`, `bootctl` and `ukify`
report 262 (262-1-arch). systemd 262 installs `systemd-sysupdate` only at
`/usr/lib/systemd/systemd-sysupdate`, off `PATH`, which is where
`make verify-systemd` now finds it. Node reads v26.10.0 and CUDA 13.4.2. A
second cosign, the distribution package 3.1.3 at `/usr/bin/cosign`, was
installed on 2026-09-16 beside the user-local v2.6.3; a bare `cosign` runs
v2.6.3 only because `~/go/bin` comes first on the developer account's `PATH`.
`planning/hardware-profile.json` records each beside the value it replaced.

## Component requirements against the profile

| Component | Requirement | Status | Evidence |
| :--- | :--- | :--- | :--- |
| P01 | TPM2 chip (or swtpm emulator for CI) | satisfied | cat /sys/class/tpm/tpm0/tpm_version_major -> 2; swtpm --version -> 0.10.2; swtpm_setup pre |
| P01 | UEFI Secure Boot firmware (PK/KEK/db/MOK hierarchy) | partial | re-read 2026-09-28: efivars list no PK, KEK, db or dbx, only the firmware's PKDefault, KEKDefault, dbDefault and dbxDefault; od `SecureBoot-*` -> 0 (disabled) and `SetupMode-*` -> 1 (setup mode; the maintainer reports clearing the keys before a firmware update, no exact date); on 2026-09-13 PK, KEK, db and dbx were enrolled and `SetupMode-*` read 0 |
| P01 | x86-64 CPU with KVM virtualization support for headless QEMU/OVMF boot | satisfied | ls -la /dev/kvm; lsmod grep kvm_amd (loaded); /sys/module/kvm_amd/parameters/nested -> 1 |
| P01 | erofs/btrfs/dm-verity-capable Linux kernel/storage stack for root-a (e | satisfied | zgrep /proc/config.gz -> CONFIG_EROFS_FS=m, CONFIG_BTRFS_FS=y, CONFIG_DM_VERITY=m (+FEC, R |
| P02 | TPM2 security chip | satisfied | /sys/class/tpm/tpm0 present, version 2, pcr-sha256 banks populated; CONFIG_TCG_TPM=y, CONF |
| P02 | UEFI Secure Boot chain | partial | bootctl status -> 'Secure Boot: disabled (setup)', 'Measured UKI: no', 'Measured OS: no' (2026-09-28); `SetupMode-*` -> 1 with no PK, KEK, db or dbx enrolled |
| P02 | Hardware watchdog timer | partial | ls /sys/class/watchdog -> empty and no /dev/watchdog; /proc/cmdline contains 'nowatchdog'; |
| P02 | NVRAM variable support for A/B slot priority swap | satisfied | grep efivarfs /proc/mounts -> mounted rw; bootctl reports UEFI 2.90 firmware; BootOrder/Lo |
| P03 | IOMMU (Intel VT-d / AMD-Vi) enabling VFIO group isolation and IOMMU do | satisfied | ls /sys/kernel/iommu_groups wc -l -> 38; AMD-Vi ivhd0; modinfo vfio_pci/vfio_iommu_type1 |
| P03 | NVMe PCIe SSD controller as DMA source | partial | 3 controllers present (990 PRO 2TB, 960 EVO 1TB, 980 PRO 1TB) so ordinary DMA is fine, but |
| P03 | GPU (NVIDIA/AMD) with P2P DMA / CUDA GPUDirect support as DMA target | missing | ls -d /sys/bus/pci/devices/*/p2pdma -> no matches (no p2pmem provider anywhere despite CON |
| P03 | PCIe BAR mapping, page-aligned; Resizable BAR / SmartAccess Memory for | satisfied | sudo lspci -vvs 06:00.0 -> 'Physical Resizable BAR', BAR1 current size 32GB (supported 64M |
| P03 | TPM2 | satisfied | /sys/class/tpm/tpm0 version 2 |
| P04 | GPU (NVIDIA or AMD) for the wlroots rendering backend | satisfied | 3 DRM cards: i915 card0, nvidia card1, amdgpu card2 (lspci -nnk). Note D08/ADR-0001 supers |
| P04 | cgroups v2 system.slice + Seccomp BPF isolation for the compositor pro | satisfied | stat -fc %T /sys/fs/cgroup -> cgroup2fs; cgroup.controllers -> cpuset cpu io memory hugetl |
| P04 | systemd sandboxing directives requiring a populated /run and /var tree | satisfied | systemctl --version -> systemd 262 (262-1-arch) running as PID 1 (261.3-1-arch on 2026-09-13); capsh --print shows th |
| P04 | TPM2 chip | satisfied | /sys/class/tpm/tpm0 version 2 |
| P05 | cgroups v2 kernel support for the app.slice hierarchy enforcing the 20 | partial | cgroup v2 hierarchy and controllers confirmed (cgroup2fs, memory+cpu+cpuset present), so s |
| P05 | Linux namespace/seccomp support for Bubblewrap sandboxing | satisfied | command -v bwrap -> /usr/bin/bwrap; CONFIG_USER_NS=y; CONFIG_SECCOMP=y |
| P06 | TPM2 security chip (hardware or swtpm emulator for dev/QEMU) | satisfied | Both available: /sys/class/tpm/tpm0 version 2 and swtpm 0.10.2 |
| P06 | Linux kernel with eBPF LSM support (lsm/bprm_check_security attach poi | satisfied | zgrep -> CONFIG_BPF_LSM=y; cat /sys/kernel/security/lsm -> capability,landlock,lockdown,ya |
| P07 | PREEMPT_RT-capable kernel (CONFIG_PREEMPT_RT, CONFIG_HZ_1000) for dete | partial | zgrep -> CONFIG_HZ_1000=y and CONFIG_HZ=1000 satisfied, but '# CONFIG_PREEMPT_RT is not se |
| P07 | x86-64-v3/v4 CPU compilation target | satisfied | /lib64/ld-linux-x86-64.so.2 --help -> 'x86-64-v4 (supported)'; /proc/cpuinfo has avx512f, |
| P07 | isolcpus-pinnable physical cores for Wayland/Compositor threads | partial | nproc -> 32, but /proc/cmdline has no isolcpus= parameter, so isolation would need a boot- |
| P07 | GPU with Linux 7.2+ DRM scheduler (DRM_SCHED_PRIORITY_HIGH/NORMAL/LOW) | partial | Kernel is 7.2.4-1-cachyos and amdgpu/i915/xe all use drm_sched (radeonsi reports DRM 3.64) |
| P07 | cgroups v2 controller for resource+risk brokerage | satisfied | cgroup2fs with cpuset cpu io memory hugetlb pids rdma misc dmem |
| P08 | Real-time-capable CPU scheduling class support (RLIMIT_RTPRIO=95, CPUS | satisfied | ulimit -r -> 99 (above the required 95); sysctl kernel.sched_rt_runtime_us=1000000 with ke |
| P08 | GPU with NVENC (or equivalent hardware video encode) implied by 'Produ | satisfied | libnvidia-encode.so.1 present and nvidia-smi reports encoder session counters on the RTX 4 |
| P08 | Memory locking capability (RLIMIT_MEMLOCK = infinity) so real-time aud | partial | ulimit -l -> 8192 (KB) for the current session. The capability exists (CAP_IPC_LOCK is in |
| P09 | GPU with PCIe P2PDMA / CUDA GPUDirect support | missing | Same evidence as P03: no /sys/bus/pci/devices/*/p2pdma entries, nvidia_fs module not found |
| P09 | NVMe PCIe SSD | satisfied | ls /sys/class/nvme -> nvme0/1/2 (Samsung 990 PRO 2TB, 960 EVO 1TB, 980 PRO 1TB) |
| P09 | Sufficient VRAM/host-pinned DRAM to hold swapped SLM expert weights | satisfied | nvidia-smi -> 24564 MiB on the RTX 4090, plus 60.5 GiB system RAM; BAR1 resized to 32 GB e |
| P09 | Power-metering capability to enforce/observe the 20.0W BIOMIMETIC_WATT | partial | RAPL package-0 and core zones are live (delta 241073516 uJ over 2 s), but scope is socket- |
| P10 | KVM virtualization extensions on the host CPU for Firecracker microVMs | satisfied | /dev/kvm present mode 0666, kvm_amd loaded, nested=1. Firecracker itself is absent but is |
| P10 | GPU/NPU accelerator when is_gpu_enabled is set on a MicroVmInstance | partial | Three GPUs present with clean IOMMU groups (17, 20, 31) and vfio-pci available, so passthr |
| P10 | AF_VSOCK-capable kernel/hypervisor transport for inter-VM and P09<->P1 | satisfied | zgrep -> CONFIG_VSOCKETS=m, CONFIG_VSOCKETS_LOOPBACK=m, CONFIG_VHOST_VSOCK=m; ls -la /dev/ |
| P11 | TPM2 security chip: PCR-sealed credentials and hash-linked, TPM2-signe | satisfied | /sys/class/tpm/tpm0 version 2 with pcr-sha256 banks; swtpm 0.10.2 for the VM path. tpm2-to |
| P11 | FIDO2-capable authenticator | missing | lsusb grep -iE 'yubi fido solo token nitro' -> no match; no security-key device enumerat |
| P11 | GPU with DMA-BUF / zero-copy export support (SPA_DATA_DmaBuf) | satisfied | CONFIG_DMA_SHARED_BUFFER=y, CONFIG_UDMABUF=y with /dev/udmabuf, CONFIG_DMABUF_HEAPS=y with |
| P12 | (no hardware_requirements entries; needs_hardware false) | satisfied | jq '.components[] select(.id=="P12") .hardware_requirements' -> empty; Node v26.10.0 (v26.8.2 on 2026-09-13), p |
| P13 | Intel RAPL (Running Average Power Limit) MSR/sysfs counters for real ( | partial | Package and core ARE real and live: sudo cat energy_uj twice 2 s apart gave 3489923310 -> |
| P13 | ACPI power interfaces as a fallback/alternative to RAPL | missing | No ACPI power_meter (ACPI000D) device; hwmon names are amdgpu, asus, asusec, enp15s0, i915 |
| P13 | cgroups v2 kernel support for per-slice attribution (system.slice/app. | satisfied | stat -fc %T /sys/fs/cgroup -> cgroup2fs; controllers include cpu, cpuset, memory, io for a |
| P14 | (no hardware_requirements entries; needs_hardware false) | satisfied | jq '.components[] select(.id=="P14") .hardware_requirements' -> empty; z3 is present a |
| P15 | NVMe SSD with PCIe BAR / memory-mapped DMA path (P03 Vulcan dependency | partial | 3 NVMe controllers present and BAR resources readable via /sys/bus/pci/devices/*/resource, |
| P15 | GPU with VA-API or NVDEC hardware decode support for the zero-copy vid | satisfied | Verified by running vainfo per render node: Intel iHD 26.2.4 on renderD129 with VLD for MP |
| P15 | Wayland compositor with wlr-layer-shell protocol support for pinned Pi | satisfied (host compositor, D79) | wayland-info -i zwlr_layer_shell_v1 -> version 5 from the host session's KDE KWin 6.7.5 (2026-09-28); development evidence only, and P04 serves no Wayland protocol |
| P15 | GPU scheduler exposing drm_sched priority tiers so Lictor can guarante | partial | Kernel 7.2.4 with amdgpu/i915/xe on drm_sched (DRM 3.64 reported by radeonsi) satisfies th |
| P15 | Btrfs-capable block storage for the @pglite subvolume snapshot/rollbac | satisfied | findmnt -no FSTYPE,SOURCE / -> btrfs /dev/nvme0n1p2[/@] (already a subvolume layout); CONF |
| P16 | (no hardware_requirements entries; needs_hardware false) | satisfied | jq '.components[] select(.id=="P16") .hardware_requirements' -> empty; pure Rust logic |
| P17 | a GPU whose VA-API driver decodes and exports DMA-BUF on the DRM device the compositor names as its main device | partial | LIBVA_DRIVER_NAME=iHD vainfo on renderD129 -> iHD 26.2.4 with MPEG-2, JPEG, H.264, HEVC, VP9 and AV1 VLD; KWin's zwp_linux_dmabuf_v1 main device 0xE281 is renderD129; decode advertised, export not yet exercised by a committed gate (M27) |
| P17 | a Wayland compositor exposing zwlr_layer_shell_v1 and zwp_linux_dmabuf_v1 version 4 or later, which refuses a format and modifier pair it did not advertise | partial (host compositor, D79) | wayland-info -> zwlr_layer_shell_v1 version 5 and zwp_linux_dmabuf_v1 version 5 advertised by KDE KWin 6.7.5; refusal of an unadvertised pair not exercised by a committed gate (M27 asserts the client-side refusal only) |
| P17 | a kernel with DMA-BUF support and AF_UNIX SCM_RIGHTS | satisfied | zgrep /proc/config.gz -> CONFIG_DMA_SHARED_BUFFER=y, CONFIG_UNIX=y, CONFIG_SYNC_FILE=y (7.2.8-1-cachyos, 2026-09-28) |
