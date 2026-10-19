#!/bin/sh
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# The M29 live session gate's in-guest read-back (E29-4).
#
# tools/verify_live_session.py hands this file to the pinned live guest as a
# systemd credential, together with a unit that runs it and the list of
# services to probe, and replaces the placeholders below first. The guest's own
# bytes stay unchanged: everything arrives through SMBIOS type 11 credentials
# and the kernel command line of a direct kernel boot.
#
# It runs as root once the graphical target is reached. For every service line
# it waits, under that line's own deadline, until the name has an owner on the
# live user's session bus (or on the accessibility bus that bus hands out),
# and reads back the package version of the process that owns it. A name with
# no owner at its deadline is reported as missing, with the name, so the
# harness can fail the case and say which service it was. It also reads the
# compositor's Wayland globals, the portal's Settings interface version, one
# setting through it, and the watcher's protocol version.
#
# It runs with set -eu: every command whose failure is a finding, not an
# error, is tested in an if or carries its own fallback.
#
# Everything is bounded: each wait counts seconds up to its deadline, every
# bus call carries busctl's own timeout, and the service list is cut at
# MAX_SERVICES lines. The report goes to the second serial line, the console
# to the first. The guest powers itself off at the end.

set -eu

PATH=/usr/bin:/bin:/usr/sbin:/sbin
export PATH

MARK='AEGIS-M29'
REPORT=/dev/ttyS1
NONCE='@AEGIS_NONCE@'
LIVE_USER='@AEGIS_LIVE_USER@'
SESSION_DEADLINE='@AEGIS_SESSION_DEADLINE@'
SERVICES="${CREDENTIALS_DIRECTORY:-/nonexistent}/aegis.session-report.services"
MAX_SERVICES=16
CALL_TIMEOUT=5s
A11Y_ADDRESS=''

say() {
    echo "${MARK}-$*" >"${REPORT}"
}

as_user() {
    runuser -u "${LIVE_USER}" -- env XDG_RUNTIME_DIR="${RUNTIME}" \
        DBUS_SESSION_BUS_ADDRESS="unix:path=${RUNTIME}/bus" "$@"
}

session_call() {
    as_user busctl --user --timeout="${CALL_TIMEOUT}" "$@"
}

a11y_call() {
    as_user busctl --address="${A11Y_ADDRESS}" --timeout="${CALL_TIMEOUT}" "$@"
}

bus_call() {
    if [ "$1" = a11y ]; then
        shift
        # The accessibility bus address is asked for again until one is handed out.
        if [ -z "${A11Y_ADDRESS}" ]; then
            A11Y_ADDRESS=$(a11y_address) || A11Y_ADDRESS=''
        fi
        a11y_call "$@"
    else
        shift
        session_call "$@"
    fi
}

# Print the PID that owns name $2 on bus $1, or fail.
owner_pid() {
    bus_call "$1" call org.freedesktop.DBus /org/freedesktop/DBus org.freedesktop.DBus \
        GetConnectionUnixProcessID s "$2" 2>/dev/null | sed -n 's/^u \([0-9][0-9]*\)$/\1/p' | grep .
}

# Print the package and version that installed the executable of PID $1.
package_of() {
    exe=$(readlink "/proc/$1/exe" 2>/dev/null) || exe=''
    if [ -z "${exe}" ]; then
        echo 'unknown-executable unknown'
        return
    fi
    rpm -qf --qf '%{NAME} %{VERSION}-%{RELEASE}\n' "${exe}" 2>/dev/null | head -n 1 | grep . ||
        echo "unpackaged ${exe}"
}

# Wait until name $3 on bus $2 has an owner, at most $4 seconds; report label $1.
probe() {
    waited=0
    while [ "${waited}" -le "$4" ]; do
        # A method call starts an activatable service. An absent name makes it
        # fail, which is no error here: the owner check below decides.
        if ! bus_call "$2" call "$3" / org.freedesktop.DBus.Peer Ping >/dev/null 2>&1; then
            :
        fi
        if pid=$(owner_pid "$2" "$3"); then
            say "SERVICE $1 $3 ${waited} $(package_of "${pid}")"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    say "MISSING $1 $3 $4"
}

wait_for_session() {
    waited=0
    while [ "${waited}" -le "${SESSION_DEADLINE}" ]; do
        if [ -S "${RUNTIME}/bus" ] && [ -S "${RUNTIME}/wayland-0" ]; then
            say "SESSION ${LIVE_USER} ${waited}"
            return 0
        fi
        sleep 1
        waited=$((waited + 1))
    done
    say "SESSION-MISSING ${LIVE_USER} ${SESSION_DEADLINE}"
    return 1
}

a11y_address() {
    session_call call org.a11y.Bus /org/a11y/bus org.a11y.Bus GetAddress 2>/dev/null |
        sed -n 's/^s "\(unix:[^"]*\)"$/\1/p' | grep .
}

wayland_globals() {
    as_user wayland-info 2>/dev/null | sed -n "s/^.*interface: '\([a-z0-9_]*\)',[[:space:]]*version:[[:space:]]*\([0-9]*\).*$/\1 \2/p" |
        head -n 128 | while read -r name version; do
        say "GLOBAL ${name} ${version}"
    done
}

portal_facts() {
    version=$(session_call get-property org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop \
        org.freedesktop.portal.Settings version 2>/dev/null | sed -n 's/^u \([0-9]*\)$/\1/p')
    say "PORTAL-SETTINGS-VERSION ${version:-absent}"
    scheme=$(session_call call org.freedesktop.portal.Desktop /org/freedesktop/portal/desktop \
        org.freedesktop.portal.Settings ReadOne ss org.freedesktop.appearance color-scheme 2>/dev/null |
        sed -n 's/^v u \([0-9]*\)$/\1/p')
    say "PORTAL-COLOR-SCHEME ${scheme:-absent}"
}

watcher_facts() {
    version=$(session_call get-property org.kde.StatusNotifierWatcher /StatusNotifierWatcher \
        org.kde.StatusNotifierWatcher ProtocolVersion 2>/dev/null | sed -n 's/^i \([0-9]*\)$/\1/p')
    say "WATCHER-PROTOCOL-VERSION ${version:-absent}"
}

probe_services() {
    head -n "${MAX_SERVICES}" "${SERVICES}" | while read -r label bus name deadline; do
        probe "${label}" "${bus}" "${name}" "${deadline}"
    done
}

os_field() {
    sed -n "s/^$1=//p" /etc/os-release | tr -d '"' | head -n 1
}

say 'BEGIN'
say "NONCE ${NONCE}"
say "UNAME-R $(uname -r)"
say "OS $(os_field ID)-$(os_field VERSION_ID)"
RUNTIME="/run/user/$(id -u "${LIVE_USER}" 2>/dev/null || echo none)"
if wait_for_session; then
    probe_services
    wayland_globals
    portal_facts
    watcher_facts
fi
say 'END'
systemctl poweroff --no-block
