#!/bin/sh
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# The sched_ext, BPF LSM and BTF probe, run on both machines (M10).
#
# Milestone M10's criterion asks for sched_ext, BPF LSM and BTF availability to
# be read from the Nucleus kernel's own configuration inside the virtual
# machine, and for each value to be diffed against the reference profile's
# host value, so that a capability the host happened to supply is never
# assumed of the kernel under test. The sibling of aegis-preempt-probe.sh, and
# "the same probe" is taken as literally: this one file is copied into the M10
# guest's initramfs and run there, and tools/verify_nucleus_kernel.py runs it
# directly on the host.
#
# It prints readings, not verdicts: the configuration lines verbatim, the
# active LSM list verbatim, and whether the BTF blob and the sched_ext state
# file exist. The gate compares the two machines' output.
#
# Bounded and effect-free: no loop, no recursion, no network, no write
# anywhere. It exits non-zero only when /proc/config.gz cannot be read.

set -eu

if [ ! -r /proc/config.gz ]; then
	echo "aegis-nucleus-probe: /proc/config.gz is not readable on this kernel" >&2
	exit 2
fi

# FUNCTION_TRACER and DYNAMIC_FTRACE_WITH_DIRECT_CALLS are read beside the
# three the criterion names: a BPF LSM program attaches through a BPF
# trampoline, which patches the 5-byte nop -mfentry leaves at the hook's entry,
# and a kernel built without the function tracer has no such nop.
gzip -dc /proc/config.gz |
	grep -E '^(# )?CONFIG_(SCHED_CLASS_EXT|BPF_LSM|BPF_SYSCALL|DEBUG_INFO_BTF|FUNCTION_TRACER|DYNAMIC_FTRACE_WITH_DIRECT_CALLS)[ =]' ||
	true

if [ -r /sys/kernel/security/lsm ]; then
	echo "LSM $(cat /sys/kernel/security/lsm)"
else
	echo "LSM <unreadable>"
fi

if [ -f /sys/kernel/btf/vmlinux ]; then
	echo "BTF-VMLINUX present"
else
	echo "BTF-VMLINUX absent"
fi

if [ -r /sys/kernel/sched_ext/state ]; then
	echo "SCHED-EXT-STATE $(cat /sys/kernel/sched_ext/state)"
else
	echo "SCHED-EXT-STATE absent"
fi
