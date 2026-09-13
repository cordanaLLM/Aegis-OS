#!/bin/sh
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# PID 1 of the M23 latency guest.
#
# It reports four things about the kernel that is executing it, and nothing
# else: the per-run nonce it was handed on its own command line, its identity,
# the CONFIG_PREEMPT_RT line from its own compiled-in configuration, and one
# cyclictest run.
#
# The nonce is why the report proves anything. tools/verify_latency_fixture.py
# deletes the report file, generates a fresh nonce, passes it as
# aegis.nonce=<value> on the kernel command line, and refuses a report that
# does not carry that exact value. A stale file from an earlier boot, or a
# hand-written one, is therefore not evidence for the current run.
#
# The measurement arguments are NOT written here. They are written into
# /aegis-latency-run.sh by the gate from one Python tuple, and the gate runs the
# same tuple on the host, so the guest and the host cannot drift apart. The two
# runs then record their own argument vectors in their JSON output and the gate
# compares those, which is evidence rather than an assertion.
#
# Everything is bounded. There is no loop, no recursion, no network and no write
# outside the initramfs. The caller applies its own deadline to the whole guest,
# so a guest that never reaches the end marker fails the gate instead of
# hanging it.

set -eu

PATH=/bin:/usr/bin
export PATH

MARK='AEGIS-M23'
# The second emulated serial line. The first carries the kernel's own console,
# and a printk landing mid-report would splice kernel text into the JSON being
# parsed.
REPORT=/dev/ttyS1

/bin/mount -n -t proc proc /proc
/bin/mount -n -t devtmpfs dev /dev
/bin/mount -n -t sysfs sys /sys

nonce=$(/bin/grep -oE 'aegis\.nonce=[A-Za-z0-9._-]+' /proc/cmdline || echo 'aegis.nonce=absent')

{
	echo "${MARK}-BEGIN"
	echo "${MARK}-NONCE ${nonce#aegis.nonce=}"
	echo "${MARK}-UNAME-R $(/bin/uname -r)"
	echo "${MARK}-UNAME-V $(/bin/uname -v)"
	echo "${MARK}-PROBE-BEGIN"
} >>"$REPORT"

if /bin/sh /aegis-preempt-probe.sh >>"$REPORT" 2>>"$REPORT"; then
	probe=0
else
	probe=$?
fi

{
	echo "${MARK}-PROBE-END"
	echo "${MARK}-PROBE-STATUS ${probe}"
} >>"$REPORT"

if /bin/sh /aegis-latency-run.sh >/dev/null 2>&1; then
	measured=0
else
	measured=$?
fi

{
	echo "${MARK}-MEASURE-STATUS ${measured}"
	echo "${MARK}-JSON-BEGIN"
	/bin/cat /aegis-latency.json 2>/dev/null || echo '{}'
	echo ""
	echo "${MARK}-JSON-END"
	echo "${MARK}-END"
} >>"$REPORT"

echo "${MARK}: reported $(/bin/uname -r) on ${REPORT}"

# Let the emulated UART drain before the machine goes away.
/bin/sleep 2

# Orderly power-off through magic SysRq, so the emulator exits on its own and
# the gate's deadline is a failure signal rather than the normal path.
echo o >/proc/sysrq-trigger

# Unreachable on a working SysRq. Kept so that a kernel without it stops here
# instead of returning from PID 1 into a panic that looks like a boot failure.
/bin/sleep 60
