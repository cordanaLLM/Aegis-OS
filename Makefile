SHELL := /bin/sh
PRAETORCTL ?= praetorctl

.PHONY: verify-all verify-sources readiness test build boot release

verify-all:
	python3 -B -m unittest discover -s tools -p 'test_*.py'
	python3 tools/verify_preparation.py
	$(PRAETORCTL) compile-context --verify
	$(PRAETORCTL) audit
	@printf '%s\n' 'PASS: preparation/governance only; OS build, boot and release remain blocked.'

verify-sources:
	python3 tools/verify_preparation.py --sources

readiness:
	python3 tools/verify_preparation.py --readiness

test: verify-all

build boot release:
	@printf '%s\n' '$@ blocked: this is a planning repository. See docs/integration/stack.md and make readiness.' >&2
	@exit 1
