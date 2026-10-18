#!/bin/sh
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# The M24 boot harness's in-guest read-back, shipped in the Aegis image.
#
# Run by aegis-boot-report.service only when harness passed the credential;
# nonce read from "${CREDENTIALS_DIRECTORY}/aegis.boot-report.nonce".

set -eu

PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH

MARK='AEGIS-M24'
REPORT=/dev/ttyS1
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

NONCE=$(head -c 64 "${CREDENTIALS_DIRECTORY:?}/aegis.boot-report.nonce")
if [ "${#NONCE}" -ne 42 ]; then
    echo "nonce length is not 42: ${#NONCE}" >&2
    exit 1
fi
case "${NONCE}" in
aegis-m24-*[!0-9a-f]*)
    echo "nonce contains non-hex characters: ${NONCE}" >&2
    exit 1
    ;;
aegis-m24-*)
    ;;
*)
    echo "nonce does not start with aegis-m24-: ${NONCE}" >&2
    exit 1
    ;;
esac

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
