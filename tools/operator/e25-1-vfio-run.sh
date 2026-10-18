#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# E25-1 positive run (ledger aegis-os-m25-vfio-bind = a): bind the Arc A380 to
# vfio-pci, map one BAR through VFIO as the unprivileged user with
# `aegis-vulcan-gpu map-bar`, rebind it to xe and read the state back.
# No address is pinned here: the Arc is found at run time as the one PCI device
# with its vendor and device id, and its IOMMU group is read from sysfs. The
# script refuses when there is no such device or more than one. NVIDIA display
# devices are listed and never written to.
#
# Dry run by default: prints the preflight facts and the writes it would make.
#   bash tools/operator/e25-1-vfio-run.sh            preview
#   bash tools/operator/e25-1-vfio-run.sh --apply    bind, map, rebind, read back
# The rebind runs from an EXIT trap, so a failed map still returns the device
# to xe. Evidence goes to the checkout's .workingdir/evidence/.
#
# AEGIS_SYSFS, AEGIS_DEV, AEGIS_SUDO, AEGIS_BIN, AEGIS_EVID, AEGIS_PROC, AEGIS_NVSMI and
# AEGIS_ENVD exist only for the stub test; the card line sets none of them.
set -euo pipefail

VENDOR=0x8086
DEVICE=0x56a5
NVIDIA_VENDOR=0x10de
HOME_DRIVER=xe
WT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
BIN=${AEGIS_BIN:-$WT/target/debug/aegis-vulcan-gpu}
SYS=${AEGIS_SYSFS:-/sys}
DEV=${AEGIS_DEV:-/dev}
SUDO=${AEGIS_SUDO:-sudo}
EVID=${AEGIS_EVID:-$WT/.workingdir/evidence}
PROC=${AEGIS_PROC:-/proc}
NVSMI=${AEGIS_NVSMI:-nvidia-smi}
ENVD=${AEGIS_ENVD:-${XDG_CONFIG_HOME:-$HOME/.config}/environment.d}
APPLY=0
[ "${1:-}" = --apply ] && APPLY=1
if [ $# -gt 1 ] || { [ $# = 1 ] && [ "$1" != --apply ]; }; then
    echo "usage: bash $0 [--apply]" >&2
    exit 64
fi

mkdir -p "$EVID"
LOG=$EVID/e25-1-vfio-$(date -u +%Y%m%dT%H%M%SZ).log
exec > >(tee -a "$LOG") 2>&1
TEE_PID=$!
APPLIED=0
ADDR=""
PCI=""
GROUP=""
NVIDIA=""

# The one PCI device with the Arc's ids, or a refusal; prints its address.
find_arc() {
    local f found=()
    for f in "$SYS"/bus/pci/devices/*; do
        [ -r "$f/vendor" ] && [ -r "$f/device" ] || continue
        [ "$(cat "$f/vendor"):$(cat "$f/device")" = "$VENDOR:$DEVICE" ] && found+=("$(basename "$f")")
    done
    [ "${#found[@]}" = 1 ] || {
        echo "REFUSE: ${#found[@]} devices with id $VENDOR:$DEVICE, want exactly one" >&2
        return 2
    }
    echo "${found[0]}"
}
# NVIDIA display-class devices (class 0x03xxxx); listed, never written.
nvidia_gpus() {
    local f
    for f in "$SYS"/bus/pci/devices/*; do
        [ -r "$f/vendor" ] && [ "$(cat "$f/vendor")" = "$NVIDIA_VENDOR" ] || continue
        case "$(cat "$f/class" 2>/dev/null)" in 0x03*) basename "$f" ;; esac
    done
}
nvidia_ok() {
    local n
    for n in $NVIDIA; do [ "$(driver_of "$n")" = nvidia ] || return 1; done
}

driver_of() {
    local l=$SYS/bus/pci/devices/$1/driver
    [ -L "$l" ] && basename "$(readlink "$l")" || echo none
}
members() { find "$PCI/iommu_group/devices/" -mindepth 1 -maxdepth 1 -printf '%f\n' | sort | tr '\n' ' ' | sed 's/ $//'; }
connected() {
    local c out=""
    for c in "$PCI"/drm/card*/card*-*; do
        [ -e "$c/status" ] || continue
        case "$(basename "$c")" in *-Writeback-*) continue ;; esac
        [ "$(cat "$c/status")" = disconnected ] || out="$out $(basename "$c")"
    done
    echo "${out# }"
}
wait_driver() {
    local want=$1 _
    for _ in $(seq 1 40); do
        [ "$(driver_of "$ADDR")" = "$want" ] && return 0
        sleep 0.25
    done
    return 1
}
wsys() { echo "$1" | $SUDO tee "$2" >/dev/null; }

preflight() {
    echo "PREFLIGHT $(date -Is) host=$(uname -n) kernel=$(uname -r)"
    kwin_fresh
    ADDR=$(find_arc) || exit 2
    PCI=$SYS/bus/pci/devices/$ADDR
    NVIDIA=$(nvidia_gpus | tr '\n' ' ' | sed 's/ $//')
    local v d g m conn
    v=$(cat "$PCI/vendor")
    d=$(cat "$PCI/device")
    g=$(basename "$(readlink "$PCI/iommu_group")")
    m=$(members)
    conn=$(connected)
    echo "  $ADDR vendor=$v device=$d driver=$(driver_of "$ADDR") group=$g members=[$m]"
    for n in $NVIDIA; do echo "  $n driver=$(driver_of "$n") (NVIDIA, never written)"; done
    [ "$v:$d" = "$VENDOR:$DEVICE" ] || {
        echo "REFUSE: $ADDR is $v:$d, not the Arc A380"
        exit 2
    }
    [ "$(driver_of "$ADDR")" = "$HOME_DRIVER" ] || {
        echo "REFUSE: $ADDR not on $HOME_DRIVER"
        exit 2
    }
    [ -n "$g" ] && [ "$m" = "$ADDR" ] || {
        echo "REFUSE: group $g holds [$m], want [$ADDR] alone"
        exit 2
    }
    GROUP=$g
    [ -z "$conn" ] || {
        echo "REFUSE: connected display output: $conn"
        exit 2
    }
    [ -x "$BIN" ] || {
        echo "REFUSE: $BIN not built"
        exit 2
    }
    holders
}

# Unbinding a device a compositor holds takes the device from under the
# compositor, which may have opened the Arc as a hotplug GPU. Refuse while any
# process holds the Arc's card or render node, named by PCI path (cardN numbers
# move between boots), and name each holder. fuser sees the caller's own
# processes, which covers a desktop session of the same user.
holders() {
    command -v fuser >/dev/null || {
        echo "REFUSE: fuser not found; cannot check the DRM node holders"
        exit 2
    }
    local s node held pids p who=""
    for s in card render; do
        node=$DEV/dri/by-path/pci-$ADDR-$s
        [ -e "$node" ] || {
            echo "REFUSE: no $node; cannot check its holders"
            exit 2
        }
        # fuser exits 1 when no process holds the node: that is "none", not an error.
        if ! held=$(fuser "$node" 2>/dev/null); then held=""; fi
        read -ra pids <<<"$held"
        echo "  holders of $node -> $(readlink "$node"): ${pids[*]:-none}"
        for p in "${pids[@]}"; do who="$who $p ($(cat "/proc/$p/comm" 2>/dev/null || echo gone))"; done
        [ -z "$who" ] || {
            echo "REFUSE: pci-$ADDR-$s held by${who}; close it or move the session off this GPU first"
            exit 2
        }
    done
}

# A KWin DRM pin (KWIN_DRM_DEVICES in one environment.d file) keeps the
# compositor on the NVIDIA GPU, but only for a kwin_wayland started after the
# pin file was written; an older session may still open the Arc as a hotplug
# GPU. The pin is found by content, never by file name: exactly one file in
# environment.d must set the variable. Refuse unless every kwin_wayland started
# strictly after the pin's mtime and nvidia-smi lists it, so the bind never
# races a compositor on the old setting.
kwin_fresh() {
    local pins=() KWIN_PIN pin btime tck c pid line f start apps n=0
    for c in "$ENVD"/*.conf; do
        grep -qs "^KWIN_DRM_DEVICES=" "$c" && pins+=("$c")
    done
    [ "${#pins[@]}" -gt 0 ] || {
        echo "REFUSE: no file in $ENVD sets KWIN_DRM_DEVICES; pin KWin to the NVIDIA GPU first"
        exit 2
    }
    [ "${#pins[@]}" = 1 ] || {
        echo "REFUSE: ${#pins[@]} files in $ENVD set KWIN_DRM_DEVICES, want exactly one: ${pins[*]}"
        exit 2
    }
    KWIN_PIN=${pins[0]}
    pin=$(stat -c %Y "$KWIN_PIN")
    btime=$(sed -n 's/^btime //p' "$PROC/stat")
    tck=$(getconf CLK_TCK)
    if ! apps=$(timeout 30 "$NVSMI" --query-compute-apps=pid,process_name --format=csv,noheader 2>&1); then
        echo "REFUSE: nvidia-smi failed: $apps"
        exit 2
    fi
    for c in "$PROC"/[0-9]*/comm; do
        [ "$(cat "$c" 2>/dev/null)" = kwin_wayland ] || continue
        pid=$(basename "$(dirname "$c")")
        line=$(cat "$PROC/$pid/stat")
        read -ra f <<<"${line##*) }"
        start=$((btime + f[19] / tck))
        [ "$start" -gt "$pin" ] || {
            echo "REFUSE: kwin_wayland $pid started $start, not after the KWin DRM pin $KWIN_PIN ($pin); log out and back in first"
            exit 2
        }
        grep -q "^$pid, " <<<"$apps" || {
            echo "REFUSE: nvidia-smi does not list kwin_wayland $pid; the compositor is not on the NVIDIA GPU"
            exit 2
        }
        echo "  kwin_wayland $pid started $start, after the KWin DRM pin ($pin); nvidia-smi lists it"
        n=$((n + 1))
    done
    [ "$n" -gt 0 ] || {
        echo "REFUSE: no kwin_wayland running; log in to the desktop session first"
        exit 2
    }
}

on_exit() {
    local rc=$?
    trap - EXIT
    [ "$APPLIED" = 1 ] && rc=$(restore "$rc")
    exec 1>&- 2>&-
    # A log writer that failed leaves an incomplete log: a clean run then exits 4.
    if ! wait "$TEE_PID" 2>/dev/null && [ "$rc" = 0 ]; then rc=4; fi
    exit "$rc"
}

restore() {
    local rc=$1
    exec 3>&1 1>&2
    echo "RESTORE"
    [ "$(driver_of "$ADDR")" = vfio-pci ] && wsys "$ADDR" "$SYS/bus/pci/drivers/vfio-pci/unbind"
    wsys "" "$PCI/driver_override"
    [ "$(driver_of "$ADDR")" = none ] && wsys "$ADDR" "$SYS/bus/pci/drivers_probe"
    local ok=1
    wait_driver "$HOME_DRIVER" || ok=0
    echo "READBACK $ADDR driver=$(driver_of "$ADDR") override=[$(cat "$PCI/driver_override")]"
    for n in $NVIDIA; do echo "READBACK $n driver=$(driver_of "$n")"; done
    if [ -d "$PCI/drm" ]; then
        find "$PCI/drm/" -mindepth 1 -maxdepth 1 -name 'renderD*' -printf 'READBACK render node: %f\n'
    else
        echo "READBACK render node: none ($PCI/drm absent)"
    fi
    if [ "$ok" = 1 ] && nvidia_ok; then
        echo "RESTORED: $ADDR back on $HOME_DRIVER; log $LOG"
        if [ "$rc" = 0 ]; then echo "RESULT pass: map-bar rc=0, $ADDR restored to $HOME_DRIVER"; fi
    else
        echo "NOT RESTORED: $ADDR on $(driver_of "$ADDR"); reboot returns it to $HOME_DRIVER; log $LOG"
        rc=3
    fi
    echo "$rc" >&3
}

trap on_exit EXIT
preflight
if [ "$APPLY" = 0 ]; then
    echo "DRY RUN: would modprobe vfio-pci, set $PCI/driver_override=vfio-pci,"
    echo "  unbind from $HOME_DRIVER, drivers_probe, chown $DEV/vfio/$GROUP to $(id -un),"
    echo "  run $BIN map-bar --device $ADDR, then rebind $HOME_DRIVER. Re-run with --apply."
    exit 0
fi

APPLIED=1
$SUDO modprobe vfio-pci
wsys vfio-pci "$PCI/driver_override"
wsys "$ADDR" "$SYS/bus/pci/drivers/$HOME_DRIVER/unbind"
wsys "$ADDR" "$SYS/bus/pci/drivers_probe"
wait_driver vfio-pci || {
    echo "FAIL: $ADDR did not bind vfio-pci"
    exit 1
}
for _ in $(seq 1 40); do
    [ -e "$DEV/vfio/$GROUP" ] && break
    sleep 0.25
done
$SUDO chown "$(id -un)" "$DEV/vfio/$GROUP"
echo "BOUND $ADDR driver=$(driver_of "$ADDR") node=$(ls -l "$DEV/vfio/$GROUP")"
set +e
timeout 60 "$BIN" map-bar --device "$ADDR"
mrc=$?
set -e
echo "MAP-BAR rc=$mrc"
[ "$mrc" = 0 ] || exit 1
