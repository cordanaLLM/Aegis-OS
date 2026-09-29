SHELL := /bin/sh
PRAETORCTL ?= praetorctl

.PHONY: verify-all verify-rust verify-systemd verify-mkosi verify-a11y a11y-fetch verify-contract \
	contract-fetch verify-kernel verify-bpf verify-latency verify-boot verify-sources verify-reuse \
	readiness test build boot release install-praetor-bump uninstall-praetor-bump

# The gate every agent runs before concluding a turn. It carries the crate gate
# too: a repository whose instructions say "run make verify-all" must not have a
# second gate that only CI remembers to run.
#
# The crate gate is guarded on the workspace manifest rather than on the
# toolchain. A checkout with no Cargo.toml has nothing to build and says so; a
# checkout that does have one needs the pinned toolchain, and a missing rustup
# is then a failure to report, not a step to skip quietly.
verify-all:
	python3 -B -m unittest discover -s tools -p 'test_*.py'
	python3 tools/verify_preparation.py
	$(MAKE) --no-print-directory verify-systemd
	$(MAKE) --no-print-directory verify-mkosi
	$(MAKE) --no-print-directory verify-a11y
	$(MAKE) --no-print-directory verify-contract
	$(PRAETORCTL) compile-context --verify
	$(PRAETORCTL) audit
	@if [ -f Cargo.toml ]; then \
		printf '%s\n' 'Workspace manifest present: running the crate gate.'; \
		$(MAKE) --no-print-directory verify-rust || exit 1; \
	else \
		printf '%s\n' 'SKIP: no Cargo.toml at the repository root; no crate gate to run.'; \
	fi
	@printf '%s\n' 'PASS: every gate above reported its own scope; a SKIP line means that gate did not run here. Image build, boot, hardware, accessibility and release remain blocked.'

# The crate gate for every workspace member listed in Cargo.toml, including
# crates whose component planning/components.json still records as a proposal,
# and the direct entry point when only the Rust half needs re-running. It
# requires the rustup-installed toolchain that rust-toolchain.toml pins (D61);
# the toolchain is reported before the gates run, so the evidence names the
# rustc that actually executed rather than a version string.
verify-rust:
	rustup show active-toolchain
	rustup which rustc
	cargo fmt --check
	cargo build --locked
	cargo test --locked --all-features
	cargo clippy --locked --all-targets --all-features -- -D warnings
	RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
	@printf '%s\n' 'PASS: workspace crate gate on the pinned toolchain; native build, image and boot remain blocked.'

# The P01/P02 definition gate (M03): the reviewed repart and sysupdate
# definitions are run through the host's own systemd, with the recorded
# negative and boundary cases. The script guards itself: a host without
# systemd-repart/systemd-sysupdate, or one below the admitted floor, prints why
# it did not run instead of reporting a pass. It never suppresses a failure.
verify-systemd:
	python3 tools/verify_systemd_definitions.py

# The P01 image-definition gate (M18): build/mkosi.conf is parsed by the host's
# own mkosi, with the floor cases run from scratch copies. It guards itself the
# same way verify-systemd does: a host without mkosi, or one below the admitted
# floor, prints why it did not run instead of reporting a pass. `mkosi summary`
# resolves configuration and prints it; it downloads nothing and builds nothing,
# so this gate validates a definition and constructs no image (D56).
verify-mkosi:
	python3 tools/verify_mkosi_definitions.py

# The accessibility gate (M04, D16, D65, D76, D81): the P12 Concordia token
# file and its one Svelte 5 component are installed from the lockfile, linted
# for HISS-01, HISS-04 and HISS-08 with ESLint (M16, D100: praetorctl scans no
# JavaScript or Svelte until cordanaLLM/praetor#589), built and scanned with
# Playwright and axe-core inside the official Playwright image, named by digest
# in ui/concordia-tokens/toolchain.pin.json, with networking disabled and the
# repository mounted read-only (REQ-P12-04). Node and pnpm are the pinned
# releases, not the image's own Node.
#
# It IS part of verify-all, unlike verify-boot, because it runs where CI runs:
# the ubuntu-24.04 runner has a container engine, and CI runs `make a11y-fetch`
# before `make verify-all`. It guards itself the way verify-systemd and
# verify-mkosi do: with no podman or docker, without the pinned image, Node
# archive, pnpm binary or offline store in its cache, or with a store filled for
# another pnpm-lock.yaml, it prints
# 'SKIP: <reason>; the accessibility gate did not run.' and exits 0, so an exit 0
# is evidence only when the case lines are above it. A cached archive that no
# longer hashes to its pin is a FAIL, not a skip. The gate never pulls and never
# downloads; it never suppresses a failure.
#
# `make a11y-fetch` is the one networked step: it pulls the image by digest,
# downloads the Node tarball and the pnpm binary, refuses either if it does not
# hash to its pin, and fills the offline pnpm store from the lockfile, recording
# that lockfile's sha256 beside it once the fill succeeded. The cache
# and the retained logs live under AEGIS_A11Y_DIR, default
# ${XDG_CACHE_HOME:-$HOME/.cache}/aegis-a11y. Nothing is written into the
# repository. A pass covers the P12 token component and the lint; it closes no
# accessibility gate for the shell or the image. The native P05 shell's tree
# check is a cargo test inside the crate gate, not a case here (D101). See
# docs/build/accessibility-harness.md.
verify-a11y:
	python3 tools/verify_a11y.py

a11y-fetch:
	python3 tools/verify_a11y.py --fetch

# The contract pair gate (M09, D92, D93, D106): Aegis's two M18 payloads,
# build/product-input.json and build/kernel-requirement.json, are run through
# cordanaLLM/imago built from the commit build/contract/producers.pin.json names.
# Each is accepted, a tampered copy is refused with an error carrying the
# payload's correlation id, and each bound imago enforces is exercised at the
# bound and one above it; an empty feature list is refused explicitly. A stand-in
# binary rebuilt from other sources that prints imago's output byte for byte is
# refused because it lacks the pinned build provenance. Since D106 the kernel
# requirement also runs through cordanaLLM/nucleus's
# scripts/verify_kernel_requirement.py at its pinned commit, with this python3
# and -I: the gate asserts on its --report-json -- PASS on the bound stream, a
# planted unset symbol refused with the correlation id, an empty list rejected,
# a digest or correlation id of another document rejected -- never on its text.
#
# It IS part of verify-all, the D88 placement D93 applies: the gate never touches
# the network, and CI runs `make contract-fetch` before `make verify-all`. With no
# go or git on PATH, or without a cached checkout, the binary or the identity record, it
# prints 'SKIP: <reason>; the contract pair gate did not run.' and exits 0, so an
# exit 0 is evidence only when the case lines are above it. A cache that is
# present but wrong is a FAIL naming `make contract-fetch`, not a skip: an
# identity record fetched for another pin (read before any absent piece, so a
# pin edit that adds a producer cannot hide it), a checkout with anything the
# pinned commit lacks (a tracked change, an untracked or ignored file, a hidden
# index entry), a binary whose sha256 is not the one the fetch recorded or whose
# `go version -m` is not the pinned build. Every git and go command runs with the
# system and user git configuration shut out and no inherited GIT_* variable. It
# never suppresses a failure.
#
# `make contract-fetch` is the one networked step: `git ls-remote --heads` against
# both producers, a depth-1 fetch of imago and of nucleus at their pinned commits
# into fresh directories, `go mod download` and `go mod verify`, and GOTOOLCHAIN=local
# CGO_ENABLED=0 go build -trimpath -buildvcs=true, refusing a binary whose
# `go version -m` is not the pinned build and recording the sha256 of the one it
# keeps. The cache and the retained logs live under AEGIS_CONTRACT_DIR, default
# ${XDG_CACHE_HOME:-$HOME/.cache}/aegis-contract; nothing is written into the
# repository and nothing is sent to either producer. A pass is consumption
# evidence: no product result, image or kernel comes back (M11, M10). See
# docs/build/contract-pair.md.
verify-contract:
	python3 tools/verify_contract_pair.py

contract-fetch:
	python3 tools/verify_contract_pair.py --fetch

# The kernel build gate (M26, D70): the pinned linux source is verified, the
# tracked fragments are applied to x86_64_defconfig, the result is compiled and
# booted in a guest, and the guest reports its own configuration back.
#
# It is deliberately NOT part of verify-all, and the omission is the decision
# rather than an oversight:
#
#   * verify-all is the per-turn gate. This one downloads 160 MB, extracts 1.4 GB
#     and compiles a kernel; on the reference profile's 32 threads that is
#     about a minute and a half of wall clock and several gigabytes of scratch,
#     which is the wrong cost for a gate that runs before every conclusion;
#   * the CI runner has no kernel toolchain, no /dev/kvm and no emulator, so
#     wiring it into verify-all would put a step into CI that can only skip.
#     A gate that always skips is not a gate;
#   * nothing this target produces is an input to anything verify-all checks.
#     The binding that does need checking everywhere -- that the tracked
#     fragment is what the M18 schema renders -- is a crate test
#     (crates/aegis-fabrica-defs/tests/kernel_fragment.rs) and a Python test
#     (tools/test_kernel_build.py), and both already run inside verify-all.
#
# So: verify-all proves the fragment still states the requirement; this target
# proves a kernel built from it satisfies the requirement. Run it by hand when
# the fragment, the pin or the payload changes. It never suppresses a failure.
#
# A host that cannot run it has two outcomes, and they are not the same claim:
#
#   * a tool that is missing, or below the floor the pinned source declares,
#     prints 'SKIP: <reason>; the kernel build gate did not run.' and exits 0,
#     which is the convention verify-systemd and verify-mkosi already use. An
#     exit 0 from this target is therefore evidence only when the case lines
#     are above it; with no toolchain it means nothing was verified, not that
#     a kernel was built. Observed with pahole off PATH: one SKIP line, exit 0;
#   * a host that has the toolchain but cannot obtain the pinned source -- no
#     network and no cached tarball -- prints 'FAIL: the gate could not run:'
#     and exits 1, so make fails. An unverifiable source is a refusal, not a
#     skip: the supply-chain check is the point of the target.
#
# The build tree is AEGIS_KERNEL_BUILD_DIR, default
# ${XDG_CACHE_HOME:-$HOME/.cache}/aegis-kernel. Nothing is written into the
# repository, nothing is installed, and the output is never a release artifact.
verify-kernel:
	python3 tools/verify_kernel_build.py

# The eBPF verifier gate (M19): the tracked objects under bpf/ are compiled with
# clang -target bpf and put through the RUNNING kernel's verifier, with the
# verifier log retained for every load, positive and negative.
#
# It is deliberately NOT part of verify-all, for the same reason verify-kernel is
# not, plus one this target has of its own:
#
#   * it needs CAP_BPF and CAP_PERFMON. The gate drops to the caller's own uid
#     with exactly that capability set through `sudo setpriv`, so it needs
#     passwordless sudo; a CI runner has neither that nor the privilege;
#   * it needs the running kernel to carry CONFIG_BPF_LSM, CONFIG_DEBUG_INFO_BTF
#     and an exported /sys/kernel/btf/vmlinux, because the objects are compiled
#     CO-RE against the BTF of the machine they are then loaded on. A runner
#     kernel is a different kernel, so a pass there would be a different claim;
#   * one case attaches a sched_ext struct_ops, which takes over the machine's
#     CPU scheduler. That case is additionally behind
#     --allow-scheduler-takeover and says why it did not run without it (D67).
#
# So a gate wired into verify-all could only ever skip on the runner, and a gate
# that always skips is not a gate. What CI does check is the half that needs no
# kernel: tools/test_bpf_objects.py asserts that the pinned versions, the
# mutation markers the negative cases delete, the stub's declared handler set and
# the unmeasured TDP literal are all what this gate expects, and that test runs
# inside verify-all.
#
# A host that cannot run it prints 'SKIP: <reason>; the eBPF verifier gate did
# not run.' and exits 0, the convention verify-systemd and verify-mkosi use. An
# exit 0 from this target is therefore evidence only when the case lines are
# above it. It never suppresses a failure. That skip is reachable for every tool
# because the gate checks PATH before it invokes the first one: invoking an
# absent binary is an error inside the gate, so with the two in the other order
# a host without clang, bpftool, pkg-config or llvm-strip failed here instead of
# standing down.
#
# Objects, logs and the generated vmlinux.h go to AEGIS_BPF_BUILD_DIR, default
# ${XDG_CACHE_HOME:-$HOME/.cache}/aegis-bpf. Nothing is written into the
# repository and nothing is installed.
#
# A pass is a NON-QUALIFYING LOCAL FIXTURE: the host kernel is neither built nor
# configured here, so it cannot close M10's Nucleus-kernel verification.
verify-bpf:
	python3 tools/verify_bpf_objects.py

# The latency fixture gate (M23, D57, D70): the kernel M26 built is booted in a
# guest, cyclictest measures wakeup latency inside it and again on the reference
# host with the identical argument vector, and every figure is reported against
# the P07 tier edges and the P08 target with the kernel that produced it.
#
# It is deliberately NOT part of verify-all, for the reasons verify-kernel is
# not, plus two of its own:
#
#   * it consumes verify-kernel's output. Without a built bzImage under
#     AEGIS_KERNEL_BUILD_DIR there is nothing to boot, so on a checkout that has
#     not run `make verify-kernel` this target can only stand down;
#   * the CI runner has no /dev/kvm, no emulator and no rt-tests, so wiring it
#     into verify-all would put a step into CI that can only skip. A gate that
#     always skips is not a gate.
#
# What CI does re-run is the half that needs no guest: tools/test_latency_fixture.py
# holds the verdict rule, the recorded admission, the thresholds the gate reads
# out of the crates and the set of programs the gate may invoke at all, and
# crates/aegis-calliope/tests/measured_figures.rs plus
# crates/aegis-lictor/tests/determinism_fixture.rs hold what a measured figure
# may and may not be read as. Both run inside verify-all.
#
# A host that cannot run it prints 'SKIP: <reason>; the latency fixture gate did
# not run.' and exits 0, the convention verify-systemd, verify-mkosi,
# verify-kernel and verify-bpf already use. An exit 0 from this target is
# therefore evidence only when the case lines are above it. It never suppresses
# a failure.
#
# It modifies neither machine. cyclictest runs with --default-system, so it does
# not write the power-management latency target into /dev/cpu_dma_latency that it
# otherwise would; nothing is installed, no module is loaded, no bootloader entry
# is written and /boot is never read.
#
# The guest tree and the retained JSON go to AEGIS_LATENCY_BUILD_DIR, default
# ${XDG_CACHE_HOME:-$HOME/.cache}/aegis-latency. The kernel image comes from
# AEGIS_KERNEL_BUILD_DIR. Nothing is written into the repository.
#
# A pass is development evidence on the reference profile. It closes no hardware
# gate: the guest's virtual CPU is scheduled by a host that is not realtime, so
# the guest figure is a composite rather than an isolated measurement of what
# PREEMPT_RT buys. See docs/build/latency.md.
verify-latency:
	python3 tools/verify_latency_fixture.py

# The boot harness gate (M24, D62, D72, D84): an externally supplied artifact --
# today the pinned Fedora Cloud UEFI-UKI 44 image, at M11 an Imago return -- is
# verified against its signed CHECKSUM and booted headless under QEMU with KVM,
# OVMF with Secure Boot and a swtpm TPM, and the guest reads PCR 0, 4, 7 and 11
# back from inside itself with a per-boot nonce.
#
# It is deliberately NOT part of verify-all, for the reasons verify-latency is
# not, plus one of its own:
#
#   * it boots five guests, which on the reference profile is about forty-five
#     seconds of wall clock, and it needs a 630 MB artifact in its cache;
#   * the CI runner has no /dev/kvm, and the harness has no TCG fallback: a boot
#     without KVM is a different claim, so on the runner it could only skip. A
#     gate that always skips is not a gate.
#
# What CI does re-run is the half that needs no guest: tools/test_boot_harness.py
# holds the pin, the producer-version floor, the inclusive timeout rule and its
# boundary, the guest report parser, the argument vectors, the recorded admission
# and the set of programs the gate may start at all. It runs inside verify-all.
#
# A host that cannot run it prints 'SKIP: <reason>; the boot harness gate did not
# run.' and exits 0 -- a missing tool, OVMF image, read-write /dev/kvm or cached
# artifact -- the convention the other hardware gates use. An exit 0 from this
# target is therefore evidence only when the case lines are above it. The cache
# is filled with `python3 tools/verify_boot_harness.py --fetch`, which downloads
# the pinned artifact, its signed CHECKSUM and the signing keys; a failed or
# mismatched download is a FAIL, not a skip. It never suppresses a failure.
#
# It modifies neither the host firmware nor the pinned bytes. The host's Secure
# Boot variables are read from efivarfs, never written; every guest writes to a
# throwaway qcow2 overlay and a per-run variable store, and the artifact is
# hashed before every boot and after the last one. swtpm and QEMU are ended with
# their whole session on every exit the harness can catch -- a normal end, an
# exception, SIGINT, SIGTERM, SIGHUP or SIGQUIT -- and QEMU runs with
# exit-with-parent, so a SIGKILL of the harness takes QEMU down and swtpm ends
# with its client. gpg runs with --no-autostart, so no gpg-agent is left behind.
#
# The cache and the retained logs live under AEGIS_BOOT_HARNESS_DIR, default
# ${XDG_CACHE_HOME:-$HOME/.cache}/aegis-boot-harness. Nothing is written into
# the repository. A pass is development evidence on the reference profile: it
# closes no image, boot, hardware or release gate and is not evidence for M11.
# See docs/build/boot-harness.md.
verify-boot:
	python3 tools/verify_boot_harness.py

verify-sources:
	python3 tools/verify_preparation.py --sources

verify-reuse:
	reuse lint

readiness:
	python3 tools/verify_preparation.py --readiness

# The local praetor pin auto-bump (docs/build/praetor-bump.md). It installs
# three systemd user units for the invoking user, never root:
#
#   * aegis-praetor-bump.service runs tools/praetor_bump.py from THIS checkout,
#     so @AEGIS_REPO@ in the templates under tools/praetor-bump/ becomes
#     $(CURDIR). Run the target from the primary checkout, not a worktree that
#     will be deleted;
#   * aegis-praetor-bump.path starts it when ~/.local/bin/praetorctl changes,
#     which Praetor's `make dev-install` does;
#   * aegis-praetor-bump.timer starts it once a day as the fallback.
#
# The path unit and the timer are enabled and started; the service only ever
# runs when one of them starts it or by hand. Neither target is part of
# verify-all: they change the workstation, not the repository.
# uninstall-praetor-bump stops and removes all three.
SYSTEMD_USER_DIR ?= $(HOME)/.config/systemd/user
PRAETOR_BUMP_UNITS := aegis-praetor-bump.service aegis-praetor-bump.path aegis-praetor-bump.timer

install-praetor-bump:
	@if [ "$$(id -u)" -eq 0 ]; then \
		printf '%s\n' 'install-praetor-bump: refusing to run as root; the bump runs as the maintainer.' >&2; \
		exit 1; \
	fi
	mkdir -p '$(SYSTEMD_USER_DIR)'
	for unit in $(PRAETOR_BUMP_UNITS); do \
		sed 's|@AEGIS_REPO@|$(CURDIR)|g' "tools/praetor-bump/$$unit" > '$(SYSTEMD_USER_DIR)'/"$$unit" || exit 1; \
	done
	systemctl --user daemon-reload
	systemctl --user enable --now aegis-praetor-bump.path aegis-praetor-bump.timer

uninstall-praetor-bump:
	-systemctl --user disable --now aegis-praetor-bump.path aegis-praetor-bump.timer
	-systemctl --user stop aegis-praetor-bump.service
	for unit in $(PRAETOR_BUMP_UNITS); do rm -f '$(SYSTEMD_USER_DIR)'/"$$unit"; done
	systemctl --user daemon-reload

# test == the preparation gate, which now carries the crate gate itself.
test: verify-all

build boot release:
	@printf '%s\n' '$@ blocked: no image build, boot or release gate is open in this repository. See docs/integration/stack.md and make readiness.' >&2
	@exit 1

# BEGIN praetor documentation gate
.PHONY: docs-lint docs-figures
verify-all: docs-lint docs-figures
docs-lint:
	@node tools/markdownlint/verify.mjs
docs-figures:
	@node tools/figures/build.mjs check
	@node tools/figures/build.mjs sources
# END praetor documentation gate
