#!/bin/sh
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# The M24 boot harness's in-guest read-back, delivered as NoCloud user-data.
#
# tools/verify_boot_harness.py copies this file into a per-run cidata seed and
# replaces the one placeholder below with a fresh nonce. cloud-init runs it as
# root in its final stage; it is configuration handed to an unmodified image,
# not a change to that image, and the image's own bytes are hashed against the
# pin before and after every boot.
#
# It reports what the booted guest reads about itself, and nothing else: the
# nonce it was handed, its kernel release, PCR 0, 4, 7 and 11 from the sha256
# bank of the TPM the guest sees (the swtpm instance the harness attached), the
# guest firmware's SecureBoot and SetupMode variables, and whether the guest's
# own TPM SRK setup unit failed. The nonce is why the report proves anything:
# the harness refuses a report that does not carry the value it generated for
# this boot, so a stale file or a reading taken on the host is not evidence.
#
# The report goes to the second serial line. The first carries the guest's
# console, where the harness looks for the login prompt; a console message
# landing mid-report would otherwise splice into the lines being parsed.
#
# Everything is bounded: one fixed list of four PCRs, no network, no write
# outside the serial line. The harness applies its own deadline to the guest.

set -u

PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH

MARK='AEGIS-M24'
REPORT=/dev/ttyS1
NONCE='@AEGIS_NONCE@'
EFIVARS=/sys/firmware/efi/efivars
GLOBAL=8be4df61-93ca-11d2-aa0d-00e098032b8c
PCRS=/sys/class/tpm/tpm0/pcr-sha256

efi_value() {
	if [ -r "${EFIVARS}/$1-${GLOBAL}" ]; then
		od -An -t u1 "${EFIVARS}/$1-${GLOBAL}" | tr -s ' ' | sed 's/^ //'
	else
		echo 'absent'
	fi
}

{
	echo "${MARK}-BEGIN"
	echo "${MARK}-NONCE ${NONCE}"
	echo "${MARK}-UNAME-R $(uname -r)"
	for index in 0 4 7 11; do
		echo "${MARK}-PCR-${index} $(cat "${PCRS}/${index}" 2>/dev/null || echo absent)"
	done
	echo "${MARK}-SECUREBOOT $(efi_value SecureBoot)"
	echo "${MARK}-SETUPMODE $(efi_value SetupMode)"
	echo "${MARK}-TPM-MAJOR $(cat /sys/class/tpm/tpm0/tpm_version_major 2>/dev/null || echo absent)"
	echo "${MARK}-TPM2-SETUP $(systemctl is-failed systemd-tpm2-setup.service 2>/dev/null)"
	echo "${MARK}-CMDLINE $(cat /proc/cmdline)"
	echo "${MARK}-END"
} >"${REPORT}"
