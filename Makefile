SHELL := /bin/sh
PRAETORCTL ?= praetorctl

.PHONY: verify-all verify-rust verify-systemd verify-mkosi verify-kernel verify-sources \
	verify-reuse readiness test build boot release

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

verify-sources:
	python3 tools/verify_preparation.py --sources

verify-reuse:
	reuse lint

readiness:
	python3 tools/verify_preparation.py --readiness

# test == the preparation gate, which now carries the crate gate itself.
test: verify-all

build boot release:
	@printf '%s\n' '$@ blocked: no image build, boot or release gate is open in this repository. See docs/integration/stack.md and make readiness.' >&2
	@exit 1
