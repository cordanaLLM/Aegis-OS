#!/bin/sh
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# PID 1 of the M10 guest, which runs the Nucleus-built kernel under test.
#
# tools/verify_nucleus_kernel.py boots this guest twice through M23's
# direct-kernel boot, with only the kernel image and the initramfs substituted.
# The phase is a file the gate writes into the initramfs:
#
#   readback  reports the kernel's identity, the sha256 of its own
#             /proc/config.gz, that configuration in full, and the output of
#             aegis-nucleus-probe.sh. The gate checks the Aegis kernel
#             requirement against this configuration (D94) and boots the
#             second phase only when every feature is satisfied.
#   load      re-reads the sha256 of /proc/config.gz and refuses to load
#             anything unless it equals the one the gate checked, then sources
#             the case list the gate wrote. No M19 object is loaded before
#             that comparison.
#
# The per-run nonce on the kernel command line is echoed back, so a report
# left by an earlier boot is not evidence for this one. Everything goes to the
# second emulated serial line; the first carries the kernel's own console.
#
# Every load runs under setpriv as an unprivileged uid with exactly the M19
# capability set; the argument vectors are written by the gate. Every loop is
# bounded, nothing here recurses, and the caller's deadline covers the guest.

set -eu

PATH=/bin:/usr/bin
export PATH

MARK='AEGIS-M10'
REPORT=/dev/ttyS1
STAGE=/aegis-m10
IDLE=/sys/fs/cgroup/aegis-idle
# Upper bound on the wait for a loader to report its attach: 50 polls of 0.1 s.
ATTACH_POLLS=50

/bin/mount -n -t proc proc /proc
/bin/mount -n -t devtmpfs dev /dev
/bin/mount -n -t sysfs sys /sys
/bin/mount -n -t securityfs securityfs /sys/kernel/security
# tracefs is group-owned by the loads' gid, so the unprivileged loader can read
# a tracepoint's id; CAP_PERFMON alone does not bypass file permissions.
/bin/mount -n -t tracefs -o mode=0750,gid=65534 tracefs /sys/kernel/tracing
/bin/mount -n -t cgroup2 cgroup2 /sys/fs/cgroup

nonce=$(/bin/grep -oE 'aegis\.nonce=[A-Za-z0-9._-]+' /proc/cmdline || echo 'aegis.nonce=absent')
phase=$(/bin/cat "$STAGE/phase")
# shellcheck disable=SC2046 # the split is the point: sha256sum prints "<hex>  -".
set -- $(/bin/gzip -dc /proc/config.gz | /bin/sha256sum)
config_sha256=$1

# Emit one retained verifier log, gzip-compressed and base64-encoded, so a
# multi-megabyte rejection trace crosses the serial line intact.
aegis_log() {
	echo "${MARK}-LOG-BEGIN $1"
	if [ -f "$2" ]; then
		/bin/gzip -c "$2" | /bin/base64
	else
		echo "${MARK}-LOG-ABSENT"
	fi
	echo "${MARK}-LOG-END $1"
}

# aegis_case NAME LOG ARGV...: run one load and frame its output and its log.
aegis_case() {
	name=$1
	log=$2
	shift 2
	echo "${MARK}-CASE-BEGIN ${name}"
	if "$@" >"$STAGE/out" 2>&1; then status=0; else status=$?; fi
	/bin/cat "$STAGE/out"
	echo "${MARK}-CASE-STATUS ${name} ${status}"
	aegis_log "$name" "$log"
	echo "${MARK}-CASE-END ${name}"
}

# aegis_idle_case NAME LOG ARGV...: the P13 idle interval. The loader attaches
# and holds; once it has printed its attach, one task enters an otherwise
# empty cgroup and sleeps through the interval the loader then reads twice.
aegis_idle_case() {
	name=$1
	log=$2
	shift 2
	/bin/mkdir -p "$IDLE"
	echo "${MARK}-IDLE-CGROUP ${name} $(/bin/stat -c %i "$IDLE")"
	echo "${MARK}-CASE-BEGIN ${name}"
	"$@" >"$STAGE/out" 2>&1 &
	loader=$!
	polls=0
	while [ "$polls" -lt "$ATTACH_POLLS" ] && ! /bin/grep -q '^attached=' "$STAGE/out"; do
		/bin/sleep 0.1
		polls=$((polls + 1))
	done
	# The child moves itself: $$ inside the new shell is its own pid.
	# shellcheck disable=SC2016 # expanded by that child, not here.
	/bin/sh -c 'echo "$$" >"$1/cgroup.procs"; exec /bin/sleep 30' aegis-idle "$IDLE" &
	idle=$!
	if wait "$loader"; then status=0; else status=$?; fi
	kill "$idle" 2>/dev/null || true
	/bin/cat "$STAGE/out"
	echo "${MARK}-CASE-STATUS ${name} ${status}"
	aegis_log "$name" "$log"
	echo "${MARK}-CASE-END ${name}"
}

{
	echo "${MARK}-BEGIN"
	echo "${MARK}-NONCE ${nonce#aegis.nonce=}"
	echo "${MARK}-PHASE ${phase}"
	echo "${MARK}-UNAME-R $(/bin/uname -r)"
	echo "${MARK}-UNAME-V $(/bin/uname -v)"
	echo "${MARK}-CONFIG-SHA256 ${config_sha256}"
} >>"$REPORT"

if [ "$phase" = readback ]; then
	{
		echo "${MARK}-PROBE-BEGIN"
		if /bin/sh /aegis-nucleus-probe.sh 2>&1; then probe=0; else probe=$?; fi
		echo "${MARK}-PROBE-END"
		echo "${MARK}-PROBE-STATUS ${probe}"
		echo "${MARK}-CONFIG-BEGIN"
		/bin/gzip -dc /proc/config.gz
		echo "${MARK}-CONFIG-END"
	} >>"$REPORT"
elif [ "$config_sha256" = "$(/bin/cat "$STAGE/expected-config-sha256")" ]; then
	echo "${MARK}-CONFIG-CHECK match" >>"$REPORT"
	# shellcheck source=/dev/null # written by the gate into the initramfs.
	{ . "$STAGE/cases.sh"; } >>"$REPORT" 2>&1
else
	echo "${MARK}-CONFIG-CHECK mismatch; no object was loaded" >>"$REPORT"
fi

echo "${MARK}-END" >>"$REPORT"
echo "${MARK}: reported $(/bin/uname -r) on ${REPORT}"

# Let the emulated UART drain before the machine goes away.
/bin/sleep 2

# Orderly power-off through magic SysRq, so the emulator exits on its own.
echo o >/proc/sysrq-trigger

# Unreachable on a working SysRq; stops here instead of returning from PID 1.
/bin/sleep 60
