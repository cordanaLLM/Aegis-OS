SHELL := /bin/sh
PRAETORCTL ?= praetorctl

.PHONY: verify-all verify-rust verify-sources verify-reuse readiness test build boot release

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
	$(PRAETORCTL) compile-context --verify
	$(PRAETORCTL) audit
	@if [ -f Cargo.toml ]; then \
		printf '%s\n' 'Workspace manifest present: running the crate gate.'; \
		$(MAKE) --no-print-directory verify-rust; \
	else \
		printf '%s\n' 'SKIP: no Cargo.toml at the repository root; no crate gate to run.'; \
	fi
	@printf '%s\n' 'PASS: preparation/governance only; OS build, boot and release remain blocked.'

# The crate gate for every activated workspace member, and the direct entry
# point when only the Rust half needs re-running. It requires the
# rustup-installed toolchain that rust-toolchain.toml pins (D61); the toolchain
# is reported before the gates run, so the evidence names the rustc that
# actually executed rather than a version string.
verify-rust:
	rustup show active-toolchain
	rustup which rustc
	cargo fmt --check
	cargo build --locked
	cargo test --locked --all-features
	cargo clippy --locked --all-targets --all-features -- -D warnings
	RUSTDOCFLAGS='-D warnings' cargo doc --locked --no-deps
	@printf '%s\n' 'PASS: workspace crate gate on the pinned toolchain; native build, image and boot remain blocked.'

verify-sources:
	python3 tools/verify_preparation.py --sources

verify-reuse:
	reuse lint

readiness:
	python3 tools/verify_preparation.py --readiness

# test == the preparation gate, which now carries the crate gate itself.
test: verify-all

build boot release:
	@printf '%s\n' '$@ blocked: this is a planning repository. See docs/integration/stack.md and make readiness.' >&2
	@exit 1
