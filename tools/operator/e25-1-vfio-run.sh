#!/usr/bin/env bash
# SPDX-FileCopyrightText: 2026 Lusoris <lusoris@proton.me>
# SPDX-License-Identifier: EUPL-1.2
#
# E25-1 positive run (ledger aegis-os-m25-vfio-bind = a): bind the Arc A380 at
# 0000:03:00.0 to vfio-pci, map one BAR through VFIO as the unprivileged user
# with `aegis-vulcan-gpu map-bar`, rebind it to xe and read the state back.
# The NVIDIA device at 0000:06:00.0 is never written to; the script refuses to
# run when the address it would bind is anything but the expected Arc.
#
# Dry run by default: prints the preflight facts and the writes it would make.
#   bash tools/operator/e25-1-vfio-run.sh            preview
#   bash tools/operator/e25-1-vfio-run.sh --apply    bind, map, rebind, read back
# The rebind runs from an EXIT trap, so a failed map still returns the device
# to xe. Evidence goes to the checkout's .workingdir/evidence/.
#
# AEGIS_SYSFS, AEGIS_DEV, AEGIS_SUDO, AEGIS_BIN and AEGIS_EVID exist only for the stub test; the card
# line sets none of them.
set -euo pipefail

ADDR=0000:03:00.0
VENDOR=0x8086
DEVICE=0x56a5
GROUP=17
HOME_DRIVER=xe
NVIDIA=0000:06:00.0
WT=$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)
BIN=${AEGIS_BIN:-$WT/target/debug/aegis-vulcan-gpu}
SYS=${AEGIS_SYSFS:-/sys}
DEV=${AEGIS_DEV:-/dev}
SUDO=${AEGIS_SUDO:-sudo}
PCI=$SYS/bus/pci/devices/$ADDR
EVID=${AEGIS_EVID:-$WT/.workingdir/evidence}
APPLY=0
[ "${1:-}" = --apply ] && APPLY=1
if [ $# -gt 1 ] || { [ $# = 1 ] && [ "$1" != --apply ]; }; then echo "usage: bash $0 [--apply]" >&2; exit 64; fi

mkdir -p "$EVID"
LOG=$EVID/e25-1-vfio-$(date -u +%Y%m%dT%H%M%SZ).log
exec > >(tee -a "$LOG") 2>&1
TEE_PID=$!
APPLIED=0

driver_of() { local l=$SYS/bus/pci/devices/$1/driver; [ -L "$l" ] && basename "$(readlink "$l")" || echo none; }
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
  [ -d "$PCI" ] || { echo "REFUSE: $ADDR absent"; exit 2; }
  local v d g m conn
  v=$(cat "$PCI/vendor"); d=$(cat "$PCI/device")
  g=$(basename "$(readlink "$PCI/iommu_group")"); m=$(members); conn=$(connected)
  echo "  $ADDR vendor=$v device=$d driver=$(driver_of "$ADDR") group=$g members=[$m]"
  echo "  $NVIDIA driver=$(driver_of "$NVIDIA") (never written)"
  [ "$v:$d" = "$VENDOR:$DEVICE" ] || { echo "REFUSE: $ADDR is $v:$d, not the Arc A380"; exit 2; }
  [ "$(driver_of "$ADDR")" = "$HOME_DRIVER" ] || { echo "REFUSE: $ADDR not on $HOME_DRIVER"; exit 2; }
  [ "$g" = "$GROUP" ] && [ "$m" = "$ADDR" ] || { echo "REFUSE: group $g holds [$m], want $GROUP=[$ADDR]"; exit 2; }
  [ -z "$conn" ] || { echo "REFUSE: connected display output: $conn"; exit 2; }
  [ -x "$BIN" ] || { echo "REFUSE: $BIN not built"; exit 2; }
  echo "  holders of the Arc DRM nodes (kept open across unbind; not signalled):"
  local n held pids
  for n in "$PCI"/drm/card* "$PCI"/drm/renderD*; do
    [ -e "$n" ] || continue
    # fuser exits 1 when no process holds the node: that is "none", not an error.
    if ! held=$(fuser "$DEV/dri/$(basename "$n")" 2>/dev/null); then held=""; fi
    read -ra pids <<< "$held"
    echo "    /dev/dri/$(basename "$n"): ${pids[*]:-none}"
  done
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
  echo "READBACK $NVIDIA driver=$(driver_of "$NVIDIA")"
  if [ -d "$PCI/drm" ]; then
    find "$PCI/drm/" -mindepth 1 -maxdepth 1 -name 'renderD*' -printf 'READBACK render node: %f\n'
  else
    echo "READBACK render node: none ($PCI/drm absent)"
  fi
  if [ "$ok" = 1 ] && [ "$(driver_of "$NVIDIA")" = nvidia ]; then
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
wait_driver vfio-pci || { echo "FAIL: $ADDR did not bind vfio-pci"; exit 1; }
for _ in $(seq 1 40); do [ -e "$DEV/vfio/$GROUP" ] && break; sleep 0.25; done
$SUDO chown "$(id -un)" "$DEV/vfio/$GROUP"
echo "BOUND $ADDR driver=$(driver_of "$ADDR") node=$(ls -l "$DEV/vfio/$GROUP")"
set +e
timeout 60 "$BIN" map-bar --device "$ADDR"
mrc=$?
set -e
echo "MAP-BAR rc=$mrc"
[ "$mrc" = 0 ] || exit 1
