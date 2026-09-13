#!/bin/sh
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# The CONFIG_PREEMPT_RT probe, run on both machines.
#
# Milestone M23's criterion asks for the guest kernel's configuration to be read
# back from inside the virtual machine and for the same probe on the reference
# host to report the option not set. "The same probe" is taken literally here:
# this one file is copied into the guest's initramfs and executed as PID 1's
# child there, and executed directly by tools/verify_latency_fixture.py on the
# host. Neither reading comes from a build tree, a package or a document; each
# comes from the running kernel's own compiled-in configuration through
# CONFIG_IKCONFIG_PROC.
#
# It prints the matching line verbatim rather than a verdict, so the two
# readings can be compared as text and so a reader can see which spelling each
# kernel produced.
#
# Bounded and effect-free: no loop, no recursion, no network, no write
# anywhere. It exits non-zero when /proc/config.gz cannot be read or carries no
# CONFIG_PREEMPT_RT line at all, and the caller reports that as a failed probe
# rather than as an answer.

set -eu

if [ ! -r /proc/config.gz ]; then
	echo "aegis-preempt-probe: /proc/config.gz is not readable on this kernel" >&2
	exit 2
fi

gzip -dc /proc/config.gz | grep -E '^(# )?CONFIG_PREEMPT_RT[ =]'
