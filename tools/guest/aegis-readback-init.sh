#!/bin/sh
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# PID 1 of the M26 read-back guest.
#
# It reports the running kernel's own identity and the running kernel's own
# configuration, and nothing else. /proc/config.gz is the copy CONFIG_IKCONFIG
# compiled into the image being tested, so what is printed here cannot come
# from the build tree or from the host: it is the kernel that is executing this
# script describing itself.
#
# Everything is bounded. There is no loop, no recursion, no network and no
# write outside /proc. The caller
# (tools/verify_kernel_build.py) applies its own deadline to the whole guest,
# so a guest that never reaches the end marker fails the gate instead of
# hanging it.

set -eu

MARK='AEGIS-M26'
# The second emulated serial line. The first one carries the kernel's own
# console, and a printk that lands mid-dump would splice kernel text into the
# configuration being reported. Reporting on a line of its own is what makes
# the captured text comparable to the produced .config byte for byte.
REPORT=/dev/ttyS1

/bin/mount -n -t proc proc /proc
/bin/mount -n -t devtmpfs dev /dev

{
	echo "${MARK}-BEGIN"
	echo "${MARK}-UNAME-R $(/bin/uname -r)"
	echo "${MARK}-UNAME-V $(/bin/uname -v)"
	echo "${MARK}-UNAME-M $(/bin/uname -m)"
	echo "${MARK}-CONFIG-BEGIN"
	/bin/gzip -dc /proc/config.gz
	echo "${MARK}-CONFIG-END"
	echo "${MARK}-END"
} > "$REPORT"

echo "${MARK}: reported $(/bin/uname -r) on ${REPORT}"

# Let the emulated UART drain before the machine goes away.
/bin/sleep 2

# Orderly power-off through magic SysRq, so the emulator exits on its own and
# the deadline is a failure signal rather than the normal path.
echo o > /proc/sysrq-trigger

# Unreachable on a working SysRq. Kept so that a kernel without it stops here
# instead of returning from PID 1 into a panic that looks like a boot failure.
/bin/sleep 60
